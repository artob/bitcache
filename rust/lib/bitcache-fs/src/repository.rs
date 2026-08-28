// This is free and unencumbered software released into the public domain.

use crate::file_metadata::{self, ExtendedMetadata};
use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, ListOrder, PutOptions, Repository, RepositoryError,
    Stream, futures_util::stream,
};
#[cfg(unix)]
use cap_std::fs_utf8::{MetadataExt, PermissionsExt};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File, OpenOptions, camino::Utf8Path},
};
use std::{
    io::{Read, Write},
    path::PathBuf,
    string::{String, ToString},
    sync::atomic::{AtomicU64, Ordering},
    vec::Vec,
};

/// The buffer size used when streaming file contents.
#[cfg(feature = "tokio")]
const BUFFER_LEN: usize = 65_536;

/// A process-local sequence used to make temporary filenames unique.
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// A repository backed by a local filesystem directory.
///
/// Blobs are stored as flat files named by their hexadecimal IDs. Access is
/// capability-scoped to the directory via [`cap_std`]. On Unix, extended
/// metadata is also accessed through file handles. Windows extended metadata
/// uses NTFS alternate data streams and therefore retains the canonical
/// repository path as a platform-specific fallback.
///
/// Note: except for [`FsRepository::put_file`], I/O is currently performed
/// synchronously within the async methods.
#[derive(Debug)]
pub struct FsRepository(Dir, #[cfg(windows)] PathBuf);

impl FsRepository {
    /// Creates or opens a new repository at the given directory path.
    pub fn create(path: impl AsRef<Utf8Path>) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        let dir = match Dir::open_ambient_dir(path, ambient_authority()) {
            Ok(dir) => dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Dir::create_ambient_dir_all(path, ambient_authority())?;
                Dir::open_ambient_dir(path, ambient_authority())?
            },
            Err(error) => return Err(error.into()),
        };
        Self::from_dir(path, dir)
    }

    /// Opens the repository at the given directory path.
    pub fn open(path: impl AsRef<Utf8Path>) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        let dir = Dir::open_ambient_dir(path, ambient_authority())?;
        Self::from_dir(path, dir)
    }

    #[cfg(not(windows))]
    fn from_dir(_path: &Utf8Path, dir: Dir) -> Result<Self, RepositoryError> {
        Ok(Self(dir))
    }

    #[cfg(windows)]
    fn from_dir(path: &Utf8Path, dir: Dir) -> Result<Self, RepositoryError> {
        Ok(Self(dir, std::fs::canonicalize(path.as_std_path())?))
    }

    /// The storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        id.to_hex().as_str().into()
    }

    #[cfg(not(windows))]
    fn extended_path(&self, _name: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(windows)]
    fn extended_path(&self, name: &str) -> Option<PathBuf> {
        Some(self.1.join(name))
    }

    /// Derives blob metadata from filesystem and extended metadata.
    fn blob_metadata(metadata: &cap_std::fs::Metadata, extended: ExtendedMetadata) -> BlobMetadata {
        BlobMetadata::new(metadata.len())
            .with_media_type(extended.media_type.map(Into::into))
            .with_created(metadata.created().ok().map(|time| time.into_std()))
            .with_updated_nanos(Self::updated_nanos(metadata))
            .with_accessed(metadata.accessed().ok().map(|time| time.into_std()))
            .with_expires_nanos(extended.expires)
    }

    /// Returns the inode status-change time as nanoseconds since the Unix epoch.
    #[cfg(unix)]
    fn updated_nanos(metadata: &cap_std::fs::Metadata) -> Option<u64> {
        let seconds = u64::try_from(metadata.ctime()).ok()?;
        let nanoseconds = u64::try_from(metadata.ctime_nsec()).ok()?;
        if nanoseconds >= 1_000_000_000 {
            return None;
        }
        seconds.checked_mul(1_000_000_000)?.checked_add(nanoseconds)
    }

    /// Status-change time is unavailable on non-Unix platforms.
    #[cfg(not(unix))]
    fn updated_nanos(_metadata: &cap_std::fs::Metadata) -> Option<u64> {
        None
    }

    fn now_nanos() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| duration.as_nanos().try_into().ok())
            .unwrap_or(u64::MAX)
    }

    fn read_extended(&self, name: &str, file: &File) -> Result<ExtendedMetadata, RepositoryError> {
        let path = self.extended_path(name);
        Ok(file_metadata::read(file, path.as_deref())?)
    }

    fn write_extended(
        &self,
        name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<bool, RepositoryError> {
        let path = self.extended_path(name);
        Ok(file_metadata::write(file, path.as_deref(), metadata)?)
    }

    fn mutate_read_only_file<T>(
        file: &File,
        operation: impl FnOnce() -> Result<T, RepositoryError>,
    ) -> Result<T, RepositoryError> {
        let original = file.metadata()?.permissions();
        let mut writable = original.clone();
        #[cfg(unix)]
        writable.set_mode(original.mode() | 0o200);
        #[cfg(not(unix))]
        writable.set_readonly(false);
        file.set_permissions(writable)?;

        let result = operation();
        let restore = file
            .set_permissions(original)
            .map_err(RepositoryError::from);
        match (result, restore) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn write_existing_extended(
        &self,
        name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<bool, RepositoryError> {
        Self::mutate_read_only_file(file, || self.write_extended(name, file, metadata))
    }

    fn is_expired(metadata: &ExtendedMetadata) -> bool {
        metadata
            .expires
            .is_some_and(|expires| expires <= Self::now_nanos())
    }

    /// Opens a physical blob and reads its extended metadata.
    fn open_physical(
        &self,
        id: &Id,
    ) -> Result<Option<(String, File, ExtendedMetadata)>, RepositoryError> {
        let name = Self::path(id);
        match self.0.open(&name) {
            Ok(file) => {
                let metadata = self.read_extended(&name, &file)?;
                Ok(Some((name, file, metadata)))
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Opens a blob only if it is logically present (not expired).
    fn open_live(
        &self,
        id: &Id,
    ) -> Result<Option<(String, File, ExtendedMetadata)>, RepositoryError> {
        Ok(self
            .open_physical(id)?
            .filter(|(_, _, metadata)| !Self::is_expired(metadata)))
    }

    /// Creates a uniquely named temporary file without replacing any file.
    fn create_temp_file(&self) -> Result<(String, File), RepositoryError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);

        loop {
            let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let name = std::format!(".put-{}-{}.tmp", std::process::id(), sequence);
            match self.0.open_with(&name, &options) {
                Ok(file) => return Ok((name, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Sets the file's permissions to read-only, updating its status-change time.
    fn make_read_only(file: &File) -> Result<(), RepositoryError> {
        let mut permissions = file.metadata()?.permissions();
        #[cfg(unix)]
        permissions.set_mode(0o444);
        #[cfg(not(unix))]
        permissions.set_readonly(true);
        file.set_permissions(permissions)?;
        Ok(())
    }

    fn prepare_temp(
        &self,
        temp_name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<(), RepositoryError> {
        self.write_extended(temp_name, file, metadata)?;
        Self::make_read_only(file)
    }

    /// Publishes a temporary file without ever replacing an existing blob.
    fn publish_temp(
        &self,
        temp_name: &str,
        id: &Id,
        metadata: &ExtendedMetadata,
    ) -> Result<(), RepositoryError> {
        let path = Self::path(id);
        for _ in 0..4 {
            match self.0.hard_link(temp_name, &self.0, &path) {
                Ok(()) => {
                    self.0.remove_file(temp_name)?;
                    return Ok(());
                },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    match self.0.open(&path) {
                        Ok(existing_file) => {
                            let result = self
                                .write_existing_extended(&path, &existing_file, metadata)
                                .map(|_| ());
                            let _ = self.0.remove_file(temp_name);
                            return result;
                        },
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(error) => {
                            let _ = self.0.remove_file(temp_name);
                            return Err(error.into());
                        },
                    }
                },
                Err(error) => {
                    let _ = self.0.remove_file(temp_name);
                    return Err(error.into());
                },
            }
        }
        let _ = self.0.remove_file(temp_name);
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "blob disappeared repeatedly while being stored",
        )
        .into())
    }

    fn store_metadata(expires: Option<u64>, media_type: Option<String>) -> ExtendedMetadata {
        ExtendedMetadata {
            expires,
            media_type,
        }
    }

    /// Collects all physical blob IDs, including expired blobs.
    fn collect_physical_ids(&self) -> Result<Vec<Id>, RepositoryError> {
        let mut ids = Vec::new();
        for entry in self.0.entries()? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Ok(name) = entry.file_name() else {
                continue;
            };
            if let Ok(id) = Id::from_hex(&name) {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    /// Collects logically present IDs, filtering before applying the limit.
    fn collect_live_ids(&self, options: &ListOptions) -> Result<Vec<Id>, RepositoryError> {
        let mut ids = Vec::new();
        for id in self.collect_physical_ids()? {
            if options.matches(&id) && self.open_live(&id)?.is_some() {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        if options.order == Some(ListOrder::Descending) {
            ids.reverse();
        }
        if let Some(limit) = options.limit {
            ids.truncate(limit);
        }
        Ok(ids)
    }

    /// Opens the blob with the given ID for asynchronous streaming reads.
    ///
    /// Returns `Ok(None)` if the repository doesn't contain the blob or it has
    /// expired. The returned file handle is capability-scoped to the repository
    /// directory, and its contents can be read incrementally without buffering
    /// the whole blob in memory.
    #[cfg(feature = "tokio")]
    pub async fn get_file(
        &self,
        id: &Id,
    ) -> Result<Option<bitcache_core::tokio::fs::File>, RepositoryError> {
        Ok(self
            .open_live(id)?
            .map(|(_, file, _)| bitcache_core::tokio::fs::File::from_std(file.into_std())))
    }

    /// Stores the file at the given path as a blob, returning its ID.
    #[cfg(feature = "tokio")]
    pub async fn put_file(
        &mut self,
        input_path: impl AsRef<std::path::Path>,
    ) -> Result<Id, RepositoryError> {
        self.put_file_with_options(input_path, PutOptions::default())
            .await
    }

    /// Stores the file at the given path as a blob with metadata options.
    ///
    /// Contents are streamed in one pass and published with a hard link, so an
    /// existing blob is never overwritten or replaced.
    #[cfg(feature = "tokio")]
    pub async fn put_file_with_options(
        &mut self,
        input_path: impl AsRef<std::path::Path>,
        options: PutOptions,
    ) -> Result<Id, RepositoryError> {
        use bitcache_core::{
            Hasher,
            tokio::{
                fs::File,
                io::{AsyncReadExt, AsyncWriteExt},
            },
        };

        let metadata = Self::store_metadata(
            options.expires_nanos(),
            options.media_type().map(ToString::to_string),
        );
        let mut input_file = File::open(input_path.as_ref()).await?;
        let (temp_name, temp_file) = self.create_temp_file()?;
        let mut temp_file = File::from_std(temp_file.into_std());

        let result: Result<Id, RepositoryError> = async {
            let mut hasher = Hasher::new();
            let mut buffer = std::vec![0u8; BUFFER_LEN];
            loop {
                match input_file.read(&mut buffer).await? {
                    0 => break,
                    n => {
                        hasher.update(&buffer[..n]);
                        temp_file.write_all(&buffer[..n]).await?;
                    },
                }
            }
            temp_file.flush().await?;
            Ok(Id(hasher.finalize()))
        }
        .await;

        drop(temp_file);
        match result {
            Ok(id) => {
                let temp_file = match self.0.open(&temp_name) {
                    Ok(file) => file,
                    Err(error) => {
                        let _ = self.0.remove_file(&temp_name);
                        return Err(error.into());
                    },
                };
                if let Err(error) = self.prepare_temp(&temp_name, &temp_file, &metadata) {
                    drop(temp_file);
                    let _ = self.0.remove_file(&temp_name);
                    return Err(error);
                }
                drop(temp_file);
                self.publish_temp(&temp_name, &id, &metadata)?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                Err(error)
            },
        }
    }

    async fn store_bytes(
        &mut self,
        data: Bytes,
        options: PutOptions,
    ) -> Result<Id, RepositoryError> {
        let id = Id::of(&data);
        let metadata = Self::store_metadata(
            options.expires_nanos(),
            options.media_type().map(ToString::to_string),
        );
        let (temp_name, mut temp_file) = self.create_temp_file()?;
        let result = temp_file.write_all(&data).map_err(RepositoryError::from);
        if result.is_ok() {
            if let Err(error) = self.prepare_temp(&temp_name, &temp_file, &metadata) {
                drop(temp_file);
                let _ = self.0.remove_file(&temp_name);
                return Err(error);
            }
        }
        drop(temp_file);

        match result {
            Ok(()) => {
                self.publish_temp(&temp_name, &id, &metadata)?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(temp_name);
                Err(error)
            },
        }
    }

    pub async fn put(&mut self, data: Bytes) -> Result<Id, RepositoryError> {
        self.store_bytes(data, PutOptions::default()).await
    }
}

impl Repository for FsRepository {
    type Error = RepositoryError;

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.open_live(id)?.is_some())
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        let Some((_, mut file, extended)) = self.open_live(id)? else {
            return Ok(None);
        };
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        let metadata = Self::blob_metadata(&file.metadata()?, extended);
        Ok(Some(
            Blob::new_unchecked(id.clone(), data).with_metadata(metadata),
        ))
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        let Some((_, file, _)) = self.open_live(id)? else {
            return Ok(None);
        };
        Ok(Some(file.metadata()?.len()))
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        self.put(data).await
    }

    async fn put_with_options(
        &mut self,
        data: Bytes,
        options: PutOptions,
    ) -> Result<Id, Self::Error> {
        self.store_bytes(data, options).await
    }

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        let Some((name, file, _)) = self.open_live(id)? else {
            return Ok(false);
        };
        drop(file);
        match self.0.remove_file(name) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    async fn set_expiry(
        &mut self,
        id: &Id,
        expires_nanos: Option<u64>,
    ) -> Result<bool, Self::Error> {
        let Some((name, file, mut metadata)) = self.open_live(id)? else {
            return Ok(false);
        };
        metadata.expires = expires_nanos;
        self.write_existing_extended(&name, &file, &metadata)
    }

    async fn set_media_type(
        &mut self,
        id: &Id,
        media_type: Option<&str>,
    ) -> Result<bool, Self::Error> {
        let Some((name, file, mut metadata)) = self.open_live(id)? else {
            return Ok(false);
        };
        metadata.media_type = media_type.map(ToString::to_string);
        self.write_existing_extended(&name, &file, &metadata)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        for id in self.collect_physical_ids()? {
            match self.0.remove_file(Self::path(&id)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        stream::iter(match self.collect_live_ids(&options) {
            Ok(ids) => ids.into_iter().map(Ok).collect(),
            Err(error) => std::vec![Err(error)],
        })
    }
}
