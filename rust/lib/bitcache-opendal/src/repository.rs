// This is free and unencumbered software released into the public domain.

use crate::OpenOptions;
use alloc::{
    borrow::{Cow, ToOwned},
    string::{String, ToString},
};
use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, OpenError, Repository, RepositoryError, Stream,
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

impl From<Operator> for DalRepository {
    fn from(operator: Operator) -> Self {
        Self(operator)
    }
}

impl DalRepository {
    /// Opens a repository for the given OpenDAL service URL.
    ///
    /// The URL scheme selects the service, the authority and path locate the
    /// storage within it, and query parameters supply service configuration
    /// options. The service must be compiled in by enabling the same-named
    /// feature flag on this crate (e.g. `s3`, `gcs`, `fs`; the in-memory
    /// service is always available), or a `services-*` feature on the
    /// `opendal` crate directly.
    ///
    /// # URL examples
    ///
    /// | Service              | URL |
    /// |----------------------|-----|
    /// | In-memory            | `memory://` |
    /// | Local filesystem     | `fs:///var/lib/bitcache` |
    /// | Amazon S3            | `s3://bucket/prefix?region=us-east-1` |
    /// | S3-compatible (e.g. MinIO) | `s3://bucket?endpoint=http://localhost:9000&region=us-east-1` |
    /// | Google Cloud Storage | `gcs://bucket/prefix` |
    /// | Azure Blob Storage   | `azblob://container/prefix?account_name=alice` |
    /// | Cloudflare R2        | `s3://bucket?endpoint=https://<account-id>.r2.cloudflarestorage.com` |
    /// | WebDAV               | `webdav://host/path` |
    /// | Redis                | `redis://localhost:6379/prefix` |
    ///
    /// Credentials can also be passed as query parameters (e.g.
    /// `access_key_id` and `secret_access_key` for S3), but prefer passing
    /// secrets via [`DalRepository::open_options`] — or the services'
    /// ambient/environment credentials — over embedding them in URLs.
    ///
    /// The full list of services and their configuration options is in the
    /// [OpenDAL service documentation](https://docs.rs/opendal/latest/opendal/services/index.html).
    pub fn open(url: &str) -> Result<Self, OpenError> {
        Ok(Self::open_options(url, OpenOptions::new())?)
    }

    /// Opens a repository for the given OpenDAL service URL, with options.
    ///
    /// See [`OpenOptions`] for the supported service configuration options
    /// and layers, and [`DalRepository::open`] for the URL format.
    pub fn open_options(url: &str, options: OpenOptions) -> Result<Self, OpenError> {
        // Ensure that all compiled-in services are registered for URL
        // scheme lookup (idempotent):
        opendal::init_default_registry();
        let mut operator = Operator::from_uri((url, options.options))?;
        for layer in options.layers {
            operator = layer(operator);
        }
        Ok(Self(operator))
    }

    /// Creates a new repository backed by the given operator.
    pub fn new(operator: Operator) -> Self {
        Self(operator)
    }

    /// The underlying OpenDAL operator.
    pub fn operator(&self) -> &Operator {
        &self.0
    }

    /// The storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        id.to_hex().as_str().into()
    }

    /// Derives blob metadata from the given OpenDAL entry metadata.
    fn blob_metadata(metadata: &opendal::Metadata) -> BlobMetadata {
        BlobMetadata::new(metadata.content_length())
            .with_media_type(metadata.content_type().map(|s| s.to_owned().into()))
            .with_created_nanos(
                metadata
                    .last_modified()
                    .map(|time| time.into_inner().as_nanosecond().max(0) as u64),
            )
            .with_expires(None) // TODO
    }
}

impl Repository for DalRepository {
    type Error = RepositoryError;

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.0.exists(&Self::path(id)).await?)
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        let path = Self::path(id);
        match self.0.read(&path).await {
            Ok(buffer) => {
                let mut blob = Blob::new_unchecked(id.clone(), buffer.to_bytes());
                // Best-effort metadata enrichment; failures are non-fatal.
                if let Ok(metadata) = self.0.stat(&path).await {
                    blob = blob.with_metadata(Self::blob_metadata(&metadata));
                }
                Ok(Some(blob))
            },
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        match self.0.stat(&Self::path(id)).await {
            Ok(metadata) => Ok(Some(metadata.content_length())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
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
        Ok(self.0.delete_options("", options).await?)
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        // The cursor is passed down to the backend (e.g. S3 `start-after`) as
        // a pagination hint; `options.matches` below remains the source of
        // truth for backends that don't support it.
        let backend_options = opendal::options::ListOptions {
            start_after: options.after.as_ref().map(|id| Self::path(id)),
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
            .map_err(|error| error.into())
            .take(limit)
            .boxed()
    }
}
