// This is free and unencumbered software released into the public domain.

use alloc::string::String;
use bitcache_core::{Blob, Bytes, Id, Repository};
use opendal::{ErrorKind, Operator};

#[derive(Clone, Debug)]
pub struct DalRepository(Operator);

impl DalRepository {
    /// Creates a new repository backed by the given operator.
    pub fn new(operator: Operator) -> Self {
        Self(operator)
    }

    /// The storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        id.to_hex().as_str().into()
    }
}

impl Repository for DalRepository {
    type Error = opendal::Error;

    async fn len(&self) -> Result<usize, Self::Error> {
        Ok(self
            .0
            .list("")
            .await?
            .iter()
            .filter(|entry| entry.metadata().is_file())
            .count())
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        self.0.exists(&Self::path(id)).await
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        match self.0.read(&Self::path(id)).await {
            Ok(buffer) => Ok(Some(Blob::new(id.clone(), buffer.to_bytes()))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        match self.0.stat(&Self::path(id)).await {
            Ok(metadata) => Ok(Some(metadata.content_length())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        let id = Id::of(&data);
        self.0.write(&Self::path(&id), data).await?;
        Ok(id)
    }
}
