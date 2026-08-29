// This is free and unencumbered software released into the public domain.

use crate::{Blob, CompactOptions, Id, ListOptions, PutOptions, RepositoryCapabilities};
use bytes::Bytes;
use core::{future::Future, time::Duration};
use futures_core::Stream;
use futures_util::{StreamExt, stream};

/// An asynchronous content-addressable blob repository.
///
/// The trait is runtime-agnostic: implementations may use any async runtime,
/// though Tokio is the default choice throughout the Bitcache ecosystem.
///
/// Methods are declared in the explicit `-> impl Future + Send` form (rather
/// than as `async fn`) so that returned futures are [`Send`] and repositories
/// can be used with multithreaded executors (e.g. `tokio::spawn`).
/// Implementations may nevertheless be written using plain `async fn`.
#[dynosaur::dynosaur(pub DynRepository = dyn(box) Repository, bridge(dyn))]
pub trait Repository: Send + Sync {
    /// The error type returned by repository operations.
    type Error: Send + Sync;

    /// Returns the optional functionality supported by this repository.
    ///
    /// Capability inspection is local and does not access the backing store.
    /// Clients should inspect capabilities before requesting metadata that they
    /// require the repository to preserve.
    fn capabilities(&self) -> RepositoryCapabilities {
        RepositoryCapabilities::NONE
    }

    /// Returns `true` if the repository contains no blobs.
    fn is_empty(&self) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async {
            let mut ids = core::pin::pin!(self.list(ListOptions::default().with_limit(1)));
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
    fn len(&self) -> impl Future<Output = Result<u64, Self::Error>> + Send {
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
    fn contains(&self, id: &Id) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        async { Ok(self.get(id).await?.is_some()) }
    }

    /// Fetches the blob with the given ID, if present.
    fn get(&self, id: &Id) -> impl Future<Output = Result<Option<Blob>, Self::Error>> + Send;

    /// Returns the size in bytes of the blob with the given ID, if present.
    fn get_len(&self, id: &Id) -> impl Future<Output = Result<Option<u64>, Self::Error>> + Send {
        async { Ok(self.get(id).await?.map(|blob| blob.len())) }
    }

    /// Stores the given data as a blob, returning its content-derived ID.
    fn put(&mut self, data: Bytes) -> impl Future<Output = Result<Id, Self::Error>> + Send;

    /// Stores the given data as a blob, with options, returning its
    /// content-derived ID.
    ///
    /// When [`PutOptions::ttl`] or [`PutOptions::media_type`] is set,
    /// repositories that support the corresponding metadata arrange to store
    /// it — where possible atomically, as part of the store itself.
    ///
    /// The default implementation stores the blob with [`Repository::put`]
    /// and then applies supported metadata on a best-effort basis. It consults
    /// [`Repository::capabilities`] first and does not attempt metadata
    /// operations the repository reports as unsupported.
    fn put_with_options(
        &mut self,
        data: Bytes,
        options: PutOptions,
    ) -> impl Future<Output = Result<Id, Self::Error>> + Send {
        async move {
            let metadata_capabilities = self.capabilities().blob_metadata();
            let id = self.put(data).await?;
            if metadata_capabilities.expires()
                && let Some(expires_nanos) = options.expires_nanos()
            {
                self.set_expiry(&id, Some(expires_nanos)).await?;
            }
            #[cfg(feature = "alloc")]
            if metadata_capabilities.media_type()
                && let Some(media_type) = options.media_type()
            {
                self.set_media_type(&id, Some(media_type)).await?;
            }
            Ok(id)
        }
    }

    /// Stores the given data as a blob that expires after the given
    /// time-to-live, returning its content-derived ID.
    ///
    /// This is shorthand for [`Repository::put_with_options`] with
    /// [`PutOptions::ttl`] set; the same expiration-support caveats apply.
    fn put_with_ttl(
        &mut self,
        data: Bytes,
        ttl: Option<Duration>,
    ) -> impl Future<Output = Result<Id, Self::Error>> + Send {
        self.put_with_options(data, PutOptions::new().with_ttl(ttl))
    }

    /// Stores the file at the given path as a blob, returning its
    /// content-derived ID.
    ///
    /// Passing the path (rather than the file's contents) lets repository
    /// backends use filesystem shortcuts where possible: for example, the
    /// filesystem backend reflinks the file into the repository on
    /// filesystems that support it, avoiding a data copy entirely.
    ///
    /// The default implementation reads the whole file into memory and
    /// delegates to [`Repository::put_with_options`].
    #[cfg(feature = "std")]
    fn put_from_path(
        &mut self,
        path: &std::path::Path,
        options: PutOptions,
    ) -> impl Future<Output = Result<Id, Self::Error>> + Send
    where
        Self::Error: From<std::io::Error>,
    {
        async move {
            // Read asynchronously when built with Tokio; otherwise fall back
            // to a blocking read, keeping this default runtime-agnostic.
            #[cfg(feature = "tokio")]
            let data = tokio::fs::read(path).await?;
            #[cfg(not(feature = "tokio"))]
            let data = std::fs::read(path)?;
            self.put_with_options(Bytes::from(data), options).await
        }
    }

    /// Removes the blob with the given ID, if present.
    ///
    /// Returns `true` if a blob was removed, or `false` if no blob with the
    /// given ID was present.
    fn remove(&mut self, id: &Id) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Sets or clears the expiration time of the blob with the given ID.
    ///
    /// The expiration time is given in nanoseconds since the Unix epoch;
    /// passing `None` clears any expiration, making the blob persistent.
    /// The expiration time of a fetched blob is reported by its
    /// [`BlobMetadata::expires`](crate::BlobMetadata) metadata.
    ///
    /// Returns `true` if the blob's expiration was updated, or `false` if
    /// no blob with the given ID was present or if the repository does not
    /// support blob expiration (the default).
    fn set_expiry(
        &mut self,
        id: &Id,
        expires_nanos: Option<u64>,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let _ = (id, expires_nanos);
        async { Ok(false) }
    }

    /// Sets or clears the explicit media type (MIME type) of the blob with the
    /// given ID.
    ///
    /// Passing `None` clears the media type. Returns `true` if the blob's media
    /// type was updated, or `false` if no blob with the given ID was present or
    /// if the repository does not support media-type metadata (the default).
    fn set_media_type(
        &mut self,
        id: &Id,
        media_type: Option<&str>,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        let _ = (id, media_type);
        async { Ok(false) }
    }

    /// Performs backend-specific repository maintenance.
    ///
    /// The default implementation is a no-op. Backends may override this to
    /// compact or otherwise optimize their physical storage without changing
    /// the repository's logical contents.
    fn compact(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }

    /// Performs backend-specific repository maintenance, with options.
    ///
    /// The default implementation ignores the options and delegates to
    /// [`Repository::compact`]. Backends with physical compression support
    /// (e.g., the filesystem backend) honor [`CompactOptions::compression`].
    fn compact_with_options(
        &mut self,
        options: CompactOptions,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let _ = options;
        self.compact()
    }

    /// Removes all blobs, resetting the repository to an empty state.
    fn clear(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Enumerates the IDs of the blobs contained in the repository.
    ///
    /// IDs are enumerated in ascending lexicographic order of their bytes
    /// (equivalently, of their hexadecimal encodings), so that repeated calls
    /// with a [`ListOptions::after`] cursor yield a stable paginated view
    /// even over very large repositories. See [`ListOptions`] for the
    /// supported prefix filter, cursor, and page-size limit.
    fn list(
        &self,
        options: ListOptions,
    ) -> impl Stream<Item = Result<Id, Self::Error>> + Send + Unpin {
        stream::empty().boxed()
    }
}

/// Awaits the next item of a stream. (A dependency-free `StreamExt::next`.)
async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    core::future::poll_fn(|cx| core::pin::Pin::new(&mut *stream).poll_next(cx)).await
}
