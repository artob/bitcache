// This is free and unencumbered software released into the public domain.

use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, Repository, RepositoryError, Stream,
    futures_util::stream,
};
#[cfg(unix)]
use cap_std::fs_utf8::PermissionsExt;
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, File, OpenOptions, camino::Utf8Path},
};
use std::{
    io::{Read, Write},
    string::String,
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
/// capability-scoped to the directory via [`cap_std`].
///
/// Note: except for [`FsRepository::put_file`], I/O is currently performed
/// synchronously within the async methods.
#[derive(Debug)]
pub struct FsRepository(Dir);

impl FsRepository {
    /// Creates or opens a new repository at the given directory path.
    pub fn create(path: impl AsRef<Utf8Path>) -> Result<Self, RepositoryError> {
        use std::io::ErrorKind;
        match Dir::open_ambient_dir(path.as_ref(), ambient_authority()) {
            Ok(dir) => Ok(Self(dir)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Dir::create_ambient_dir_all(path.as_ref(), ambient_authority())?;
                Ok(Self(Dir::open_ambient_dir(
                    path.as_ref(),
                    ambient_authority(),
                )?))
            },
            Err(err) => Err(err.into()),
        }
    }

    /// Opens the repository at the given directory path.
    pub fn open(path: impl AsRef<Utf8Path>) -> Result<Self, RepositoryError> {
        Ok(Self(Dir::open_ambient_dir(
            path.as_ref(),
            ambient_authority(),
        )?))
    }

    /// The storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        id.to_hex().as_str().into()
    }

    /// Derives blob metadata from the given file metadata.
    fn blob_metadata(metadata: &cap_std::fs::Metadata) -> BlobMetadata {
        BlobMetadata::new(metadata.len())
            .with_created(
                metadata
                    .created()
                    .or_else(|_| metadata.modified())
                    .ok()
                    .map(|time| time.into_std()),
            )
            .with_accessed(metadata.accessed().ok().map(|time| time.into_std()))
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

    /// Publishes a temporary file without ever replacing an existing blob.
    fn publish_temp(&self, temp_name: &str, id: &Id) -> Result<(), RepositoryError> {
        let temp_file = self.0.open(temp_name)?;
        if let Err(error) = Self::make_read_only(&temp_file) {
            drop(temp_file);
            let _ = self.0.remove_file(temp_name);
            return Err(error);
        }
        drop(temp_file);

        let path = Self::path(id);
        match self.0.hard_link(temp_name, &self.0, &path) {
            Ok(()) => {
                self.0.remove_file(temp_name)?;
                Ok(())
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.0.remove_file(temp_name)?;
                let existing_file = self.0.open(path)?;
                Self::make_read_only(&existing_file)
            },
            Err(error) => {
                let _ = self.0.remove_file(temp_name);
                Err(error.into())
            },
        }
    }

    /// Collects the IDs of the contained blobs, in ascending order.
    ///
    /// Directory iteration order is unspecified, so the IDs are materialized
    /// and sorted; this suffices for the current flat-directory layout.
    fn collect_ids(&self, options: &ListOptions) -> Result<Vec<Id>, RepositoryError> {
        let mut ids = Vec::new();
        for entry in self.0.entries()? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Ok(name) = entry.file_name() else {
                continue;
            };
            let Ok(id) = Id::from_hex(&name) else {
                continue;
            };
            if options.matches(&id) {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        if let Some(limit) = options.limit {
            ids.truncate(limit);
        }
        Ok(ids)
    }

    /// Opens the blob with the given ID for asynchronous streaming reads.
    ///
    /// Returns `Ok(None)` if the repository doesn't contain the blob.
    /// The returned file handle is capability-scoped to the repository
    /// directory, and its contents can be read incrementally without
    /// buffering the whole blob in memory.
    #[cfg(feature = "tokio")]
    pub async fn get_file(
        &self,
        id: &Id,
    ) -> Result<Option<bitcache_core::tokio::fs::File>, RepositoryError> {
        match self.0.open(Self::path(id)) {
            Ok(file) => Ok(Some(bitcache_core::tokio::fs::File::from_std(
                file.into_std(),
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Stores the file at the given path as a blob, returning its ID.
    ///
    /// The file's contents are streamed with asynchronous I/O in a single
    /// pass: each chunk is hashed with BLAKE3 while being written to a
    /// temporary file in the repository. Once the ID is known, a hard link
    /// publishes the file without replacing any existing blob. The file is
    /// never buffered wholly in memory.
    #[cfg(feature = "tokio")]
    pub async fn put_file(
        &mut self,
        input_path: impl AsRef<std::path::Path>,
    ) -> Result<Id, RepositoryError> {
        use bitcache_core::{
            Hasher,
            tokio::{
                fs::File,
                io::{AsyncReadExt, AsyncWriteExt},
            },
        };

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
                self.publish_temp(&temp_name, &id)?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                Err(error)
            },
        }
    }

    pub async fn put(&mut self, data: Bytes) -> Result<Id, RepositoryError> {
        let id = Id::of(&data);
        let (temp_name, mut temp_file) = self.create_temp_file()?;
        let result = temp_file.write_all(&data);
        drop(temp_file);

        match result {
            Ok(()) => {
                self.publish_temp(&temp_name, &id)?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(temp_name);
                Err(error.into())
            },
        }
    }
}

impl Repository for FsRepository {
    type Error = RepositoryError;

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.0.try_exists(Self::path(id))?)
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        match self.0.open(Self::path(id)) {
            Ok(mut file) => {
                let mut data = Vec::new();
                file.read_to_end(&mut data)?;
                let metadata = file.metadata()?;
                Ok(Some(
                    Blob::new_unchecked(id.clone(), data)
                        .with_metadata(Self::blob_metadata(&metadata)),
                ))
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        match self.0.metadata(Self::path(id)) {
            Ok(metadata) => Ok(Some(metadata.len())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        self.put(data).await
    }

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        match self.0.remove_file(Self::path(id)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        for id in self.collect_ids(&ListOptions::default())? {
            match self.0.remove_file(Self::path(&id)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        stream::iter(match self.collect_ids(&options) {
            Ok(ids) => ids.into_iter().map(Ok).collect(),
            Err(error) => std::vec![Err(error)],
        })
    }
}
