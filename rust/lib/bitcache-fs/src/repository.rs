// This is free and unencumbered software released into the public domain.

use bitcache_core::{Blob, Bytes, Id, ListOptions, Repository, Stream, futures_util::stream};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, camino::Utf8Path},
};
use std::{io::Result, string::String, vec::Vec};

#[derive(Debug)]
pub struct FsRepository(Dir);

impl FsRepository {
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
        let id = Id::of(&data);
        self.0.write(Self::path(&id), &data)?;
        Ok(id)
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
