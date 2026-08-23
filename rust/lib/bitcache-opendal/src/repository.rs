// This is free and unencumbered software released into the public domain.

use alloc::string::String;
use bitcache_core::{Blob, Id, Repository};
use opendal::{Operator, blocking};

#[derive(Clone, Debug)]
pub struct DalRepository(blocking::Operator);

impl DalRepository {
    /// Creates a new repository backed by the given operator.
    ///
    /// Must be called from within a Tokio runtime context, since OpenDAL's
    /// blocking API dispatches operations onto the current runtime.
    pub fn new(operator: Operator) -> opendal::Result<Self> {
        Ok(Self(blocking::Operator::new(operator)?))
    }

    /// The storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        id.to_hex().as_str().into()
    }
}

impl Repository for DalRepository {
    fn len(&self) -> usize {
        self.0
            .list("")
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry.metadata().is_file())
                    .count()
            })
            .unwrap_or(0)
    }

    fn contains(&self, id: &Id) -> bool {
        self.0.exists(&Self::path(id)).unwrap_or(false)
    }

    fn get(&self, id: &Id) -> Option<Blob> {
        self.0
            .read(&Self::path(id))
            .ok()
            .map(|buffer| Blob::new(buffer.to_bytes()))
    }

    fn get_len(&self, id: &Id) -> Option<u64> {
        self.0
            .stat(&Self::path(id))
            .ok()
            .map(|metadata| metadata.content_length())
    }
}
