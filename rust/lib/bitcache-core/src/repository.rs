// This is free and unencumbered software released into the public domain.

use crate::{Blob, Id, ListOptions};
use bytes::Bytes;
use core::future::Future;
use futures_core::Stream;

/// An asynchronous content-addressable blob repository.
///
/// The trait is runtime-agnostic: implementations may use any async runtime,
/// though Tokio is the default choice throughout the Bitcache ecosystem.
///
/// Methods are declared in the explicit `-> impl Future + Send` form (rather
/// than as `async fn`) so that returned futures are [`Send`] and repositories
/// can be used with multithreaded executors (e.g. `tokio::spawn`).
/// Implementations may nevertheless be written using plain `async fn`.
pub trait Repository {
    /// The error type returned by repository operations.
    type Error;

    /// Returns `true` if the repository contains no blobs.
    fn is_empty(&self) -> impl Future<Output = Result<bool, Self::Error>> + Send
    where
        Self: Sync,
    {
        async {
            let mut ids = core::pin::pin!(self.list(ListOptions::new().with_limit(1)));
            match next(&mut ids).await {
                None => Ok(true),
                Some(result) => result.map(|_| false),
            }
        }
    }

    /// Returns the number of blobs in the repository.
    ///
    /// This is implemented in terms of [`Repository::list`], and hence takes
    /// time linear in the number of contained blobs. Implementations may
    /// override it when they can count blobs more cheaply.
    fn len(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send
    where
        Self: Sync,
    {
        async {
            let mut ids = core::pin::pin!(self.list(ListOptions::default()));
            let mut count: u64 = 0;
            while let Some(result) = next(&mut ids).await {
                result?;
                count += 1;
            }
            Ok(count)
        }
    }

    /// Returns `true` if the repository contains the blob with the given ID.
    fn contains(&self, id: &Id) -> impl Future<Output = Result<bool, Self::Error>> + Send
    where
        Self: Sync,
    {
        async { Ok(self.get(id).await?.is_some()) }
    }

    /// Fetches the blob with the given ID, if present.
    fn get(&self, id: &Id) -> impl Future<Output = Result<Option<Blob>, Self::Error>> + Send;

    /// Returns the size in bytes of the blob with the given ID, if present.
    fn get_len(&self, id: &Id) -> impl Future<Output = Result<Option<u64>, Self::Error>> + Send
    where
        Self: Sync,
    {
        async { Ok(self.get(id).await?.map(|blob| blob.len())) }
    }

    /// Stores the given data as a blob, returning its content-derived ID.
    fn put(&mut self, data: Bytes) -> impl Future<Output = Result<Id, Self::Error>> + Send;

    /// Enumerates the IDs of the blobs contained in the repository.
    ///
    /// IDs are enumerated in ascending lexicographic order of their bytes
    /// (equivalently, of their hexadecimal encodings), so that repeated calls
    /// with a [`ListOptions::start_after`] cursor yield a stable paginated
    /// view even over very large repositories. See [`ListOptions`] for the
    /// supported prefix filter, cursor, and page-size limit.
    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> + Send;
}

/// Awaits the next item of a stream. (A dependency-free `StreamExt::next`.)
async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    core::future::poll_fn(|cx| core::pin::Pin::new(&mut *stream).poll_next(cx)).await
}
