// This is free and unencumbered software released into the public domain.

use alloc::collections::BTreeMap;
use bitcache_core::{Blob, Bytes, Id, ListOptions, Repository, Stream, futures_util::stream};
use core::{convert::Infallible, ops::Bound};

/// An in-memory (heap-allocated) repository, useful for testing and caching.
///
/// Blobs are kept in a sorted map, so enumeration order and cursor seeks
/// come for free.
#[derive(Clone, Debug, Default)]
pub struct HeapRepository(BTreeMap<Id, Bytes>);

impl HeapRepository {
    /// Creates a new, empty repository.
    pub fn new() -> Self {
        Self(BTreeMap::default())
    }
}

impl Repository for HeapRepository {
    type Error = Infallible;

    /// An O(1) shortcut, equivalent to counting the [`Repository::list`]
    /// enumeration.
    async fn len(&self) -> Result<u64, Self::Error> {
        Ok(self.0.len() as u64)
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

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.0.remove(id).is_some())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.0.clear();
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> + Send {
        let start = match options.start_after.clone() {
            Some(id) => (Bound::Excluded(id), Bound::Unbounded),
            None => (Bound::Unbounded, Bound::Unbounded),
        };
        let limit = options.limit.unwrap_or(usize::MAX);
        stream::iter(
            self.0
                .range(start)
                .map(|(id, _)| id)
                .filter(move |id| options.matches(id))
                .take(limit)
                .map(|id| Ok(id.clone())),
        )
    }
}
