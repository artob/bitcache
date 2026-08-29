// This is free and unencumbered software released into the public domain.

use crate::{
    BlobEncoding, BlobFile, Dir, DirCursor, Utf8Path,
    dir_cursor::SHARD_PREFIX_LEN,
    file_metadata::{self, ExtendedMetadata},
};
use bitcache_core::{
    Blob, BlobMetadata, BlobMetadataCapabilities, Bytes, Id, ListOptions, ListOrder, PutOptions,
    Repository, RepositoryCapabilities, RepositoryError, Stream,
    futures_util::StreamExt,
};
#[cfg(unix)]
use cap_std::fs_utf8::{MetadataExt, PermissionsExt};
use cap_std::{
    ambient_authority,
    fs_utf8::{File, OpenOptions},
};
use std::{
    path::PathBuf,
    string::{String, ToString},
    sync::atomic::{AtomicU64, Ordering},
    vec::Vec,
};

/// The buffer size used when streaming file contents.
#[cfg(feature = "tokio")]
const BUFFER_LEN: usize = 65_536;

/// The LZMA2 dictionary size used by liblzma's fastest (preset 0) mode.
#[cfg(feature = "tokio")]
const FASTEST_XZ_DICTIONARY_SIZE: u64 = 256 * 1024;

/// A process-local sequence used to make temporary filenames unique.
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct PhysicalBlob {
    name: String,
    file: File,
    extended: ExtendedMetadata,
    encoding: BlobEncoding,
}

/// A repository backed by a local filesystem directory.
///
/// Blobs are stored in shard subdirectories named after the first
/// [`SHARD_PREFIX_LEN`] hexadecimal characters of the blob ID (`00` through
/// `ff` by default), created lazily as blobs are stored. Within a shard,
/// files are named by their full
/// hexadecimal IDs, with an `.xz` suffix when compressed. Access is
/// capability-scoped to the directory
/// via [`cap_std`]. On Unix, extended
/// metadata is also accessed through file handles. Windows extended metadata
/// uses NTFS alternate data streams and therefore retains the canonical
/// repository path as a platform-specific fallback.
///
/// Note: except for [`FsRepository::put_file`], I/O is currently performed
/// synchronously within the async methods.
#[derive(Debug)]
pub struct FsRepository(Dir, RepositoryCapabilities, #[cfg(windows)] PathBuf);

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
        let mut repository = Self(dir, RepositoryCapabilities::NONE);
        repository.1 = repository.detect_capabilities();
        Ok(repository)
    }

    #[cfg(windows)]
    fn from_dir(path: &Utf8Path, dir: Dir) -> Result<Self, RepositoryError> {
        let mut repository = Self(
            dir,
            RepositoryCapabilities::NONE,
            std::fs::canonicalize(path.as_std_path())?,
        );
        repository.1 = repository.detect_capabilities();
        Ok(repository)
    }

    /// Creates a cursor over the physical blobs in the repository directory.
    ///
    /// See [`DirCursor`] for ordering and memory-use guarantees.
    fn cursor(&self, descending: bool) -> DirCursor {
        DirCursor::open(&self.0, descending)
    }

    /// Creates the shard subdirectory for the given blob ID if it doesn't
    /// already exist.
    ///
    /// Shard subdirectories are created lazily, on first store of a blob
    /// whose ID falls within the shard.
    fn ensure_shard_dir(&self, id: &Id) -> Result<(), RepositoryError> {
        match self.0.create_dir(Self::shard_name(id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// The shard subdirectory name for the blob with the given ID.
    fn shard_name(id: &Id) -> String {
        let hex = id.to_hex();
        hex.as_str()[..SHARD_PREFIX_LEN].into()
    }

    /// The uncompressed storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        let hex = id.to_hex();
        let hex = hex.as_str();
        std::format!("{}/{}", &hex[..SHARD_PREFIX_LEN], hex)
    }

    /// The compressed storage path for the blob with the given ID.
    fn compressed_path(id: &Id) -> String {
        let hex = id.to_hex();
        let hex = hex.as_str();
        std::format!("{}/{}.xz", &hex[..SHARD_PREFIX_LEN], hex)
    }

    fn path_for(id: &Id, encoding: BlobEncoding) -> String {
        match encoding {
            BlobEncoding::Uncompressed => Self::path(id),
            BlobEncoding::Xz => Self::compressed_path(id),
        }
    }

    #[cfg(not(windows))]
    fn extended_path(&self, _name: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(windows)]
    fn extended_path(&self, name: &str) -> Option<PathBuf> {
        Some(self.2.join(name))
    }

    fn detect_capabilities(&self) -> RepositoryCapabilities {
        let metadata = BlobMetadataCapabilities::new()
            .with_created()
            .with_accessed();
        #[cfg(unix)]
        let metadata = metadata.with_updated();
        let metadata = if self.supports_extended_metadata() {
            metadata.with_expires().with_media_type()
        } else {
            metadata
        };
        RepositoryCapabilities::new().with_blob_metadata(metadata)
    }

    fn supports_extended_metadata(&self) -> bool {
        let Ok((name, file)) = self.create_temp_file() else {
            return false;
        };
        let metadata = ExtendedMetadata {
            expires: Some(1),
            media_type: Some(String::from("application/x-bitcache-capability-probe")),
            ..ExtendedMetadata::default()
        };
        let path = self.extended_path(&name);
        let supported = file_metadata::write(&file, path.as_deref(), &metadata).unwrap_or(false);
        drop(file);
        let _ = self.0.remove_file(name);
        supported
    }

    /// Derives blob metadata from filesystem and extended metadata.
    fn blob_metadata(
        metadata: &cap_std::fs::Metadata,
        extended: ExtendedMetadata,
        len: u64,
    ) -> BlobMetadata {
        let created = extended.created.or_else(|| {
            metadata
                .created()
                .ok()
                .and_then(|time| Self::system_time_nanos(time.into_std()))
        });
        let updated = extended.updated.or_else(|| Self::updated_nanos(metadata));
        BlobMetadata::new(len)
            .with_media_type(extended.media_type.map(Into::into))
            .with_created_nanos(created)
            .with_updated_nanos(updated)
            .with_accessed(metadata.accessed().ok().map(|time| time.into_std()))
            .with_expires_nanos(extended.expires)
    }

    fn system_time_nanos(time: std::time::SystemTime) -> Option<u64> {
        time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()?
            .as_nanos()
            .try_into()
            .ok()
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

    fn write_reinserted_extended(
        &self,
        name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<bool, RepositoryError> {
        let existing = self.read_extended(name, file)?;
        let metadata = ExtendedMetadata {
            created: existing.created,
            updated: None,
            expires: metadata.expires,
            media_type: metadata.media_type.clone(),
        };
        self.write_existing_extended(name, file, &metadata)
    }

    fn is_expired(metadata: &ExtendedMetadata) -> bool {
        metadata
            .expires
            .is_some_and(|expires| expires <= Self::now_nanos())
    }

    /// Opens a physical blob and reads its extended metadata.
    ///
    /// The uncompressed representation is preferred when both exist.
    fn open_physical(&self, id: &Id) -> Result<Option<PhysicalBlob>, RepositoryError> {
        for encoding in [BlobEncoding::Uncompressed, BlobEncoding::Xz] {
            let name = Self::path_for(id, encoding);
            match self.0.open(&name) {
                Ok(file) => {
                    let extended = self.read_extended(&name, &file)?;
                    return Ok(Some(PhysicalBlob {
                        name,
                        file,
                        extended,
                        encoding,
                    }));
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    /// Opens a blob only if it is logically present (not expired).
    fn open_live(&self, id: &Id) -> Result<Option<PhysicalBlob>, RepositoryError> {
        Ok(self
            .open_physical(id)?
            .filter(|blob| !Self::is_expired(&blob.extended)))
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

    #[cfg(feature = "tokio")]
    fn prepare_compacted_temp(
        &self,
        temp_name: &str,
        file: &File,
        extended: &ExtendedMetadata,
        source: &cap_std::fs::Metadata,
    ) -> Result<(), RepositoryError> {
        if !self.write_extended(temp_name, file, extended)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "filesystem cannot preserve blob metadata during compaction",
            )
            .into());
        }

        let mut times = std::fs::FileTimes::new();
        if let Ok(accessed) = source.accessed() {
            times = times.set_accessed(accessed.into_std());
        }
        if let Ok(modified) = source.modified() {
            times = times.set_modified(modified.into_std());
        }
        file.try_clone()?.into_std().set_times(times)?;
        file.set_permissions(source.permissions())?;
        Ok(())
    }

    /// Publishes a temporary file without ever replacing an existing blob.
    fn publish_temp(
        &self,
        temp_name: &str,
        id: &Id,
        metadata: &ExtendedMetadata,
        encoding: BlobEncoding,
    ) -> Result<(), RepositoryError> {
        use std::io::{Error, ErrorKind};

        let uncompressed_path = Self::path(id);
        match self.0.open(&uncompressed_path) {
            Ok(existing_file) => {
                let result = self
                    .write_reinserted_extended(&uncompressed_path, &existing_file, metadata)
                    .map(|_| ());
                let _ = self.0.remove_file(temp_name);
                return result;
            },
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => {
                let _ = self.0.remove_file(temp_name);
                return Err(error.into());
            },
        }

        let path = Self::path_for(id, encoding);
        if let Err(error) = self.ensure_shard_dir(id) {
            let _ = self.0.remove_file(temp_name);
            return Err(error);
        }
        for _ in 0..4 {
            match self.0.hard_link(temp_name, &self.0, &path) {
                Ok(()) => {
                    self.0.remove_file(temp_name)?;
                    return Ok(());
                },
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    match self.0.open(&path) {
                        Ok(existing_file) => {
                            let result = self
                                .write_reinserted_extended(&path, &existing_file, metadata)
                                .map(|_| ());
                            let _ = self.0.remove_file(temp_name);
                            return result;
                        },
                        Err(error) if error.kind() == ErrorKind::NotFound => continue,
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
        Err(Error::new(
            ErrorKind::NotFound,
            "blob disappeared repeatedly while being stored",
        )
        .into())
    }

    fn store_metadata(expires: Option<u64>, media_type: Option<String>) -> ExtendedMetadata {
        ExtendedMetadata {
            expires,
            media_type,
            ..ExtendedMetadata::default()
        }
    }

    /// Selects a blob that needs maximum XZ recompression.
    #[cfg(feature = "tokio")]
    fn compact_source(
        &self,
        id: &Id,
    ) -> Result<Option<(BlobEncoding, cap_std::fs::Metadata)>, RepositoryError> {
        use crate::util::read_xz_dict_size;
        use std::io::ErrorKind;

        match self.0.open(Self::path(id)) {
            Ok(file) => {
                let metadata = file.metadata()?;
                return Ok(Some((BlobEncoding::Uncompressed, metadata)));
            },
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }

        let mut file = match self.0.open(Self::compressed_path(id)) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let metadata = file.metadata()?;
        Ok(
            (read_xz_dict_size(&mut file)? == Some(FASTEST_XZ_DICTIONARY_SIZE))
                .then_some((BlobEncoding::Xz, metadata)),
        )
    }

    /// Rewrites an uncompressed or minimally compressed blob using maximum XZ compression.
    #[cfg(feature = "tokio")]
    async fn compact_blob(&self, id: &Id) -> Result<(), RepositoryError> {
        use async_compression::{Level, tokio::write::XzEncoder};
        use bitcache_core::tokio::{fs::File, io::AsyncWriteExt};

        std::eprintln!("Compacting {}...", id);

        let Some((source_encoding, source_metadata)) = self.compact_source(id)? else {
            return Ok(());
        };
        let source_name = Self::path_for(id, source_encoding);
        let source_file = self.0.open(&source_name)?;
        let mut extended = self.read_extended(&source_name, &source_file)?;
        extended.created = extended.created.or_else(|| {
            source_metadata
                .created()
                .ok()
                .and_then(|time| Self::system_time_nanos(time.into_std()))
        });
        extended.updated = extended
            .updated
            .or_else(|| Self::updated_nanos(&source_metadata));
        let mut source_file = BlobFile::new(source_file, source_encoding);
        let (temp_name, temp_file) = self.create_temp_file()?;
        let temp_file = File::from_std(temp_file.into_std());
        let mut encoder = XzEncoder::with_quality(temp_file, Level::Best);

        let result: Result<(), RepositoryError> = async {
            bitcache_core::tokio::io::copy(&mut source_file, &mut encoder).await?;
            encoder.shutdown().await?;
            Ok(())
        }
        .await;
        drop(encoder);
        drop(source_file);

        if let Err(error) = result {
            let _ = self.0.remove_file(&temp_name);
            return Err(error);
        }

        let temp_file = match self.0.open(&temp_name) {
            Ok(file) => file,
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                return Err(error.into());
            },
        };
        if let Err(error) =
            self.prepare_compacted_temp(&temp_name, &temp_file, &extended, &source_metadata)
        {
            drop(temp_file);
            let _ = self.0.remove_file(&temp_name);
            return Err(error);
        }
        drop(temp_file);

        let compressed_name = Self::compressed_path(id);
        if let Err(error) = self.ensure_shard_dir(id) {
            let _ = self.0.remove_file(&temp_name);
            return Err(error);
        }
        if let Err(error) = self.0.rename(&temp_name, &self.0, &compressed_name) {
            let _ = self.0.remove_file(&temp_name);
            return Err(error.into());
        }
        if source_encoding == BlobEncoding::Xz {
            return Ok(());
        }
        match self.0.remove_file(source_name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Opens the blob with the given ID for asynchronous streaming reads.
    ///
    /// Returns `Ok(None)` if the repository doesn't contain the blob or it has
    /// expired. The returned file handle is capability-scoped to the repository
    /// directory, and its contents can be read incrementally without buffering
    /// the whole blob in memory.
    #[cfg(feature = "tokio")]
    pub async fn get_file(&self, id: &Id) -> Result<Option<BlobFile>, RepositoryError> {
        Ok(self
            .open_live(id)?
            .map(|blob| BlobFile::new(blob.file, blob.encoding)))
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
        use async_compression::{Level, tokio::write::XzEncoder};
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
        let temp_file = File::from_std(temp_file.into_std());
        let mut encoder = XzEncoder::with_quality(temp_file, Level::Best);

        let result: Result<Id, RepositoryError> = async {
            let mut hasher = Hasher::new();
            let mut buffer = std::vec![0u8; BUFFER_LEN];
            loop {
                match input_file.read(&mut buffer).await? {
                    0 => break,
                    n => {
                        hasher.update(&buffer[..n]);
                        encoder.write_all(&buffer[..n]).await?;
                    },
                }
            }
            encoder.shutdown().await?;
            Ok(Id(hasher.finalize()))
        }
        .await;

        drop(encoder);
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
                self.publish_temp(&temp_name, &id, &metadata, BlobEncoding::Xz)?;
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
        let (temp_name, temp_file) = self.create_temp_file()?;

        #[cfg(feature = "tokio")]
        let (result, encoding) = {
            use async_compression::{Level, tokio::write::XzEncoder};
            use bitcache_core::tokio::{fs::File, io::AsyncWriteExt};

            let temp_file = File::from_std(temp_file.into_std());
            let mut encoder = XzEncoder::with_quality(temp_file, Level::Best);
            let result = async {
                encoder.write_all(&data).await?;
                encoder.shutdown().await?;
                Ok::<(), RepositoryError>(())
            }
            .await;
            drop(encoder);
            (result, BlobEncoding::Xz)
        };

        #[cfg(not(feature = "tokio"))]
        let (result, encoding) = {
            let mut temp_file = temp_file;
            (
                temp_file.write_all(&data).map_err(RepositoryError::from),
                BlobEncoding::Uncompressed,
            )
        };

        if result.is_ok() {
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
        }

        match result {
            Ok(()) => {
                self.publish_temp(&temp_name, &id, &metadata, encoding)?;
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

    fn capabilities(&self) -> RepositoryCapabilities {
        self.1
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.open_live(id)?.is_some())
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        let Some(blob) = self.open_live(id)? else {
            return Ok(None);
        };

        #[cfg(feature = "tokio")]
        {
            use bitcache_core::tokio::io::AsyncReadExt;

            let metadata_file = blob.file.try_clone()?;
            let mut reader = BlobFile::new(blob.file, blob.encoding);
            let mut data = Vec::new();
            reader.read_to_end(&mut data).await?;
            let metadata =
                Self::blob_metadata(&metadata_file.metadata()?, blob.extended, data.len() as u64);
            Ok(Some(
                Blob::new_unchecked(id.clone(), data).with_metadata(metadata),
            ))
        }

        #[cfg(not(feature = "tokio"))]
        {
            if blob.encoding == BlobEncoding::Xz {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "XZ blob reads require the tokio feature",
                )
                .into());
            }
            let mut file = blob.file;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            let metadata = Self::blob_metadata(&file.metadata()?, blob.extended, data.len() as u64);
            Ok(Some(
                Blob::new_unchecked(id.clone(), data).with_metadata(metadata),
            ))
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        let Some(blob) = self.open_live(id)? else {
            return Ok(None);
        };
        if blob.encoding == BlobEncoding::Uncompressed {
            return Ok(Some(blob.file.metadata()?.len()));
        }

        #[cfg(feature = "tokio")]
        {
            use bitcache_core::tokio::io::AsyncReadExt;

            let mut reader = BlobFile::new(blob.file, blob.encoding);
            let mut buffer = [0u8; BUFFER_LEN];
            let mut len = 0u64;
            loop {
                let read = reader.read(&mut buffer).await?;
                if read == 0 {
                    break;
                }
                len = len.checked_add(read as u64).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "blob length overflow")
                })?;
            }
            Ok(Some(len))
        }

        #[cfg(not(feature = "tokio"))]
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "XZ blob reads require the tokio feature",
        )
        .into())
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
        let Some(blob) = self.open_live(id)? else {
            return Ok(false);
        };
        drop(blob.file);

        let mut removed = false;
        for encoding in [BlobEncoding::Uncompressed, BlobEncoding::Xz] {
            match self.0.remove_file(Self::path_for(id, encoding)) {
                Ok(()) => removed = true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(removed)
    }

    async fn set_expiry(
        &mut self,
        id: &Id,
        expires_nanos: Option<u64>,
    ) -> Result<bool, Self::Error> {
        let Some(mut blob) = self.open_live(id)? else {
            return Ok(false);
        };
        blob.extended.expires = expires_nanos;
        blob.extended.updated = None;
        self.write_existing_extended(&blob.name, &blob.file, &blob.extended)
    }

    async fn set_media_type(
        &mut self,
        id: &Id,
        media_type: Option<&str>,
    ) -> Result<bool, Self::Error> {
        let Some(mut blob) = self.open_live(id)? else {
            return Ok(false);
        };
        blob.extended.media_type = media_type.map(ToString::to_string);
        blob.extended.updated = None;
        self.write_existing_extended(&blob.name, &blob.file, &blob.extended)
    }

    async fn compact(&mut self) -> Result<(), Self::Error> {
        #[cfg(feature = "tokio")]
        {
            let mut cursor = self.cursor(false);
            let mut previous: Option<Id> = None;
            while let Some(item) = cursor.next().await {
                let (id, _encoding) = item?;
                if previous.as_ref() == Some(&id) {
                    continue;
                }
                self.compact_blob(&id).await?;
                previous = Some(id);
            }
            Ok(())
        }

        #[cfg(not(feature = "tokio"))]
        Ok(())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        // Removes all blobs one shard batch at a time, preserving the shard
        // subdirectories themselves.
        let mut cursor = self.cursor(false);
        while let Some(item) = cursor.next().await {
            let (id, encoding) = item?;
            match self.0.remove_file(Self::path_for(&id, encoding)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        // The cursor visits one shard subdirectory at a time in ID order, so
        // the stream is ordered while memory use stays bounded by shard size.
        let descending = options.order == Some(ListOrder::Descending);
        let limit = options.limit.unwrap_or(usize::MAX);
        self.cursor(descending)
            .scan(
                (None::<Id>, false),
                move |(previous, failed), item| {
                    if *failed {
                        return core::future::ready(None);
                    }
                    let item = match item {
                        Err(error) => {
                            *failed = true;
                            Some(Err(error))
                        },
                        // Skip the duplicate entry when a blob exists in both
                        // encodings (entries for one ID are adjacent).
                        Ok((id, _encoding)) if previous.as_ref() == Some(&id) => None,
                        Ok((id, _encoding)) => {
                            *previous = Some(id.clone());
                            if !options.matches(&id) {
                                None
                            } else {
                                match self.open_live(&id) {
                                    Ok(Some(_)) => Some(Ok(id)),
                                    Ok(None) => None,
                                    Err(error) => {
                                        *failed = true;
                                        Some(Err(error))
                                    },
                                }
                            }
                        },
                    };
                    core::future::ready(Some(item))
                },
            )
            .filter_map(core::future::ready)
            .take(limit)
    }
}
