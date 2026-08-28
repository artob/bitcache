// This is free and unencumbered software released into the public domain.

use alloc::collections::{BTreeMap, btree_map::Entry};
use bitcache_core::{
    Blob, BlobMetadata, BlobMetadataCapabilities, BoxError, Bytes, Id, ListOptions, Repository,
    RepositoryCapabilities, RepositoryError, Stream, futures_util::stream,
};
use core::{convert::Infallible, ops::Bound};

/// An in-memory (heap-allocated) repository, useful for testing and caching.
///
/// Blobs are kept in a sorted map, so enumeration order and cursor seeks
/// come for free.
#[derive(Clone, Debug, Default)]
pub struct HeapRepository(BTreeMap<Id, HeapEntry>);

#[derive(Clone, Debug, Default)]
struct HeapEntry {
    data: Bytes,
    metadata: BlobMetadata,
}

impl HeapRepository {
    /// Creates a new, empty repository.
    pub fn new() -> Self {
        Self(BTreeMap::default())
    }
}

impl Repository for HeapRepository {
    //type Error = Infallible;
    type Error = RepositoryError;

    fn capabilities(&self) -> RepositoryCapabilities {
        let metadata = BlobMetadataCapabilities::new();
        #[cfg(feature = "std")]
        let metadata = metadata.with_created().with_updated();
        RepositoryCapabilities::new().with_blob_metadata(metadata)
    }

    /// An O(1) shortcut, equivalent to counting the [`Repository::list`]
    /// enumeration.
    async fn len(&self) -> Result<u64, Self::Error> {
        Ok(self.0.len() as u64)
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.0.contains_key(id))
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        Ok(self.0.get(id).map(|entry| {
            Blob::new_unchecked(id.clone(), entry.data.clone())
                .with_metadata(entry.metadata.clone())
        }))
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        Ok(self.0.get(id).map(|entry| entry.data.len() as u64))
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        let id = Id::of(&data);
        let timestamp = now();
        match self.0.entry(id.clone()) {
            Entry::Vacant(entry) => {
                let metadata = BlobMetadata::new(data.len() as u64)
                    .with_created_nanos(timestamp)
                    .with_updated_nanos(timestamp);
                entry.insert(HeapEntry { data, metadata });
            },
            Entry::Occupied(mut entry) => {
                let metadata = entry.get().metadata.clone().with_updated_nanos(timestamp);
                entry.get_mut().metadata = metadata;
            },
        }
        Ok(id)
    }

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.0.remove(id).is_some())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.0.clear();
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        let start = match options.after.clone() {
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

/// The current time as nanoseconds since the Unix epoch, when available.
#[cfg(feature = "std")]
fn now() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos() as u64)
}

/// The current time, unavailable without `std`.
#[cfg(not(feature = "std"))]
fn now() -> Option<u64> {
    None
}
