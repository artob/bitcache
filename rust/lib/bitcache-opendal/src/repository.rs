// This is free and unencumbered software released into the public domain.

use alloc::string::String;
use bitcache_core::{
    Blob, Bytes, Id, ListOptions, Repository, Stream,
    futures_util::{StreamExt, TryFutureExt, TryStreamExt, future},
};
use opendal::{ErrorKind, Operator};

/// A repository backed by an Apache OpenDAL [`Operator`].
///
/// # Enumeration order caveat
///
/// [`Repository::list`] yields IDs in whatever order the underlying service
/// natively lists them. The trait's ordering contract (ascending
/// lexicographic order, on which stable pagination relies) is only upheld on
/// services whose native listing is lexicographic by key — as is the case
/// for, e.g., S3, GCS, Azure Blob Storage, and OpenDAL's `memory` service.
/// On services without that guarantee (e.g. some filesystem-like backends),
/// enumeration still honors the prefix filter, cursor, and limit, but its
/// order — and thus pagination stability — is not guaranteed.
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

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        // OpenDAL's `delete` is idempotent and doesn't report whether the
        // path existed, so check for presence first.
        let path = Self::path(id);
        if !self.0.exists(&path).await? {
            return Ok(false);
        }
        self.0.delete(&path).await?;
        Ok(true)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        let options = opendal::options::DeleteOptions {
            recursive: true,
            ..Default::default()
        };
        self.0.delete_options("", options).await
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> + Send {
        // The cursor is passed down to the backend (e.g. S3 `start-after`) as
        // a pagination hint; `options.matches` below remains the source of
        // truth for backends that don't support it.
        let backend_options = opendal::options::ListOptions {
            start_after: options.start_after.as_ref().map(|id| Self::path(id)),
            // Passed down as a page-size hint; the authoritative cap is the
            // `take` below.
            limit: options.limit,
            ..Default::default()
        };
        let limit = options.limit.unwrap_or(usize::MAX);
        self.0
            .lister_options("", backend_options)
            .try_flatten_stream()
            .try_filter_map(move |entry| {
                let id = if entry.metadata().is_file() {
                    Id::from_hex(entry.name())
                        .ok()
                        .filter(|id| options.matches(id))
                } else {
                    None
                };
                future::ready(Ok(id))
            })
            .take(limit)
    }
}
