// This is free and unencumbered software released into the public domain.

use bitcache_core::{Blob, Bytes, Id, ListOptions, Repository, Stream, futures_util::stream};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, camino::Utf8Path},
};
use std::{io::Result, string::String, vec::Vec};

/// The buffer size used when streaming file contents.
#[cfg(feature = "tokio")]
const BUFFER_LEN: usize = 65_536;

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
    pub fn create(path: impl AsRef<Utf8Path>) -> Result<Self> {
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
            Err(err) => Err(err),
        }
    }

    /// Opens the repository at the given directory path.
    pub fn open(path: impl AsRef<Utf8Path>) -> Result<Self> {
        Ok(Self(Dir::open_ambient_dir(
            path.as_ref(),
            ambient_authority(),
        )?))
    }

    /// The storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        id.to_hex().as_str().into()
    }

    /// Collects the IDs of the contained blobs, in ascending order.
    ///
    /// Directory iteration order is unspecified, so the IDs are materialized
    /// and sorted; this suffices for the current flat-directory layout.
    fn collect_ids(&self, options: &ListOptions) -> Result<Vec<Id>> {
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
    pub async fn get_file(&self, id: &Id) -> Result<Option<bitcache_core::tokio::fs::File>> {
        match self.0.open(Self::path(id)) {
            Ok(file) => Ok(Some(bitcache_core::tokio::fs::File::from_std(
                file.into_std(),
            ))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Stores the file at the given path as a blob, returning its ID.
    ///
    /// The file's contents are streamed with asynchronous I/O in a single
    /// pass: each chunk is hashed with BLAKE3 while being written to a
    /// temporary file in the repository, which is then renamed into place
    /// once the ID is known. The file is never buffered wholly in memory.
    #[cfg(feature = "tokio")]
    pub async fn put_file(&mut self, input_path: impl AsRef<std::path::Path>) -> Result<Id> {
        use bitcache_core::{
            Hasher,
            tokio::{
                fs::File,
                io::{AsyncReadExt, AsyncWriteExt},
            },
        };

        let mut input_file = File::open(input_path.as_ref()).await?;

        let temp_name = std::format!(".put-{}.tmp", std::process::id());
        let mut temp_file = File::from_std(self.0.create(&temp_name)?.into_std());

        let result: Result<Id> = async {
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
                self.0.rename(&temp_name, &self.0, Self::path(&id))?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                Err(error)
            },
        }
    }

    pub async fn put(&mut self, data: Bytes) -> Result<Id> {
        let id = Id::of(&data);
        self.0.write(Self::path(&id), &data)?;
        Ok(id)
    }
}

impl Repository for FsRepository {
    type Error = std::io::Error;

    async fn contains(&self, id: &Id) -> Result<bool> {
        self.0.try_exists(Self::path(id))
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>> {
        match self.0.read(Self::path(id)) {
            Ok(data) => Ok(Some(Blob::new(id.clone(), data))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>> {
        match self.0.metadata(Self::path(id)) {
            Ok(metadata) => Ok(Some(metadata.len())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn put(&mut self, data: Bytes) -> Result<Id> {
        self.put(data).await
    }

    async fn remove(&mut self, id: &Id) -> Result<bool> {
        match self.0.remove_file(Self::path(id)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn clear(&mut self) -> Result<()> {
        for id in self.collect_ids(&ListOptions::default())? {
            match self.0.remove_file(Self::path(&id)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id>> + Send {
        stream::iter(match self.collect_ids(&options) {
            Ok(ids) => ids.into_iter().map(Ok).collect(),
            Err(error) => std::vec![Err(error)],
        })
    }
}
