// This is free and unencumbered software released into the public domain.

use bitcache_core::{Blob, Bytes, Id, Repository};
use core::convert::Infallible;
use hashbrown::HashMap;

#[derive(Clone, Debug, Default)]
pub struct HeapRepository(HashMap<Id, Bytes>);

impl HeapRepository {
    pub fn new() -> Self {
        Self(HashMap::default())
    }
}

impl Repository for HeapRepository {
    type Error = Infallible;

    async fn len(&self) -> Result<usize, Self::Error> {
        Ok(self.0.len())
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.0.contains_key(id))
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        Ok(self
            .0
            .get(id)
            .map(|data| Blob::new(id.clone(), data.clone())))
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        Ok(self.0.get(id).map(|data| data.len() as u64))
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        let id = Id::of(&data);
        self.0.insert(id.clone(), data);
        Ok(id)
    }
}
