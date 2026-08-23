// This is free and unencumbered software released into the public domain.

use crate::{Blob, Id};
use bytes::Bytes;

/// An asynchronous content-addressable blob repository.
///
/// The trait is runtime-agnostic: implementations may use any async runtime,
/// though Tokio is the default choice throughout the Bitcache ecosystem.
#[allow(async_fn_in_trait)]
pub trait Repository {
    /// The error type returned by repository operations.
    type Error;

    /// Returns `true` if the repository contains no blobs.
    async fn is_empty(&self) -> Result<bool, Self::Error> {
        Ok(self.len().await? == 0)
    }

    /// Returns the number of blobs in the repository.
    async fn len(&self) -> Result<usize, Self::Error> {
        Ok(0)
    }

    /// Returns `true` if the repository contains the blob with the given ID.
    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.get(id).await?.is_some())
    }

    /// Fetches the blob with the given ID, if present.
    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        Ok(None)
    }

    /// Returns the size in bytes of the blob with the given ID, if present.
    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        Ok(self.get(id).await?.map(|blob| blob.len()))
    }

    /// Stores the given data as a blob, returning its content-derived ID.
    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error>;
}
