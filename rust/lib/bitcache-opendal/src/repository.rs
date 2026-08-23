// This is free and unencumbered software released into the public domain.

use crate::OpenOptions;
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
    pub fn open(url: &str) -> opendal::Result<Self> {
        Self::open_options(url, OpenOptions::new())
    }

    /// Opens a repository for the given OpenDAL service URL, with options.
    ///
    /// See [`OpenOptions`] for the supported service configuration options
    /// and layers, and [`DalRepository::open`] for the URL format.
    pub fn open_options(url: &str, options: OpenOptions) -> opendal::Result<Self> {
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
