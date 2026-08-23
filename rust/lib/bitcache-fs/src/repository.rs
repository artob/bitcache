// This is free and unencumbered software released into the public domain.

use bitcache_core::{Blob, Bytes, Id, Repository};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, camino::Utf8Path},
};
use std::{io::Result, string::String};

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
}

impl Repository for FsRepository {
    type Error = std::io::Error;

    async fn len(&self) -> Result<usize> {
        let mut count = 0;
        for entry in self.0.entries()? {
            if entry?.file_type()?.is_file() {
                count += 1;
            }
        }
        Ok(count)
    }

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
}
