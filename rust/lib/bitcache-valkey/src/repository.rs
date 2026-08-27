// This is free and unencumbered software released into the public domain.

use alloc::{format, string::String, string::ToString, sync::Arc, vec, vec::Vec};
use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, OpenError, Repository, RepositoryError, Stream,
    futures_util::{StreamExt, TryStreamExt, future, stream},
};
use core::time::Duration;
use fred::{
    clients::Client,
    cmd,
    interfaces::{ClientLike, KeysInterface, SortedSetsInterface, TransactionInterface},
    types::{Expiration, Value, config::Config},
};
use tokio::sync::OnceCell;
use url::Url;

/// The sorted-set key indexing the IDs of all contained blobs.
const INDEX_KEY: &str = "bitcache:index";

/// The key prefix under which blob data is stored.
const BLOB_KEY_PREFIX: &str = "bitcache:blob:";

/// How many IDs to fetch from the index per round trip.
const PAGE_SIZE: usize = 256;

/// A repository backed by a [Valkey](https://valkey.io) (or Redis) server,
/// using the [`fred`] client.
///
/// Blob data is stored under string keys of the form `bitcache:blob:<hex>`,
/// with a sorted set at `bitcache:index` indexing the IDs of all contained
/// blobs. Since the index is sorted lexicographically, enumeration order and
/// cursor seeks come for free.
///
/// # Expiring blobs
///
/// Blobs may carry an expiration time, enforced server-side by the key's
/// TTL. A default time-to-live for stored blobs can be configured via
/// [`ValkeyRepository::with_ttl`] or a `ttl` URL query parameter (in
/// seconds), and the expiration of an individual blob can be set or cleared
/// with [`Repository::expire`]. A fetched blob's expiration time, if
/// any, is reported by its [`BlobMetadata::expires`](BlobMetadata).
///
/// Expired blobs disappear from [`Repository::contains`],
/// [`Repository::get`], and [`Repository::list`] immediately, though their
/// index entries are only pruned lazily (during enumeration), so
/// [`Repository::len`] may transiently overcount until then.
///
/// The connection is established lazily on first use; cloning the repository
/// shares the underlying connection.
#[derive(Clone)]
pub struct ValkeyRepository {
    client: Client,
    connected: Arc<OnceCell<()>>,
    ttl: Option<Duration>,
}

impl From<Client> for ValkeyRepository {
    fn from(client: Client) -> Self {
        Self::new(client)
    }
}

impl ValkeyRepository {
    /// Opens a repository for the given Valkey (or Redis) server URL.
    ///
    /// # URL examples
    ///
    /// - `valkey://localhost:6379`
    /// - `valkey://localhost:6379/0`
    /// - `valkey://username:password@localhost:6379`
    /// - `valkey://localhost:6379?ttl=3600`
    /// - `redis://localhost:6379`
    ///
    /// A `ttl` query parameter, in seconds, configures a default
    /// time-to-live for stored blobs; see [`ValkeyRepository::with_ttl`].
    ///
    /// The connection itself is established lazily upon first use.
    pub fn open(url: &str) -> Result<Self, OpenError> {
        // Accept scheme-less `//host:port` forms:
        let url = if url.starts_with("//") {
            format!("redis:{}", url)
        } else {
            url.into()
        };
        let mut url = Url::parse(&url).map_err(|_| OpenError::InvalidUrl)?;

        // Normalize the `valkey(s)` schemes to the `redis(s)` schemes
        // understood by the fred client:
        match url.scheme() {
            "valkey" => url.set_scheme("redis").map_err(|_| OpenError::InvalidUrl)?,
            "valkeys" => url
                .set_scheme("rediss")
                .map_err(|_| OpenError::InvalidUrl)?,
            _ => {},
        }

        // Extract (and strip) the `ttl` query parameter, in seconds:
        let mut ttl = None;
        if url.query().is_some() {
            let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();
            url.set_query(None);
            for (key, value) in pairs {
                if key == "ttl" {
                    ttl = match value.parse::<u64>() {
                        Ok(secs) if secs > 0 => Some(Duration::from_secs(secs)),
                        _ => return Err(OpenError::InvalidUrl),
                    };
                } else {
                    url.query_pairs_mut().append_pair(&key, &value);
                }
            }
        }

        let config = Config::from_url(url.as_str())?;
        Ok(Self::new(Client::new(config, None, None, None)).with_ttl(ttl))
    }

    /// Creates a new repository backed by the given client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            connected: Arc::new(OnceCell::new()),
            ttl: None,
        }
    }

    /// Configures a default time-to-live for stored blobs.
    ///
    /// When set, every blob stored by [`Repository::put`] expires that long
    /// after it was (last) stored; storing an already-present blob resets
    /// its clock. When unset (the default), stored blobs are persistent,
    /// and storing an already-present blob clears any expiration it had.
    pub fn with_ttl(mut self, ttl: impl Into<Option<Duration>>) -> Self {
        self.ttl = ttl.into();
        self
    }

    /// The default time-to-live for stored blobs, if configured.
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    /// The underlying fred client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The storage key for the blob with the given ID.
    fn blob_key(id: &Id) -> String {
        format!("{}{}", BLOB_KEY_PREFIX, id.to_hex())
    }

    /// Returns the client, connecting to the server on first use.
    async fn connect(&self) -> Result<&Client, RepositoryError> {
        self.connected
            .get_or_try_init(|| async {
                let _handle = self.client.init().await?;
                Ok::<(), fred::error::Error>(())
            })
            .await?;
        Ok(&self.client)
    }
}

impl Repository for ValkeyRepository {
    type Error = RepositoryError;

    /// An O(1) shortcut, equivalent to counting the [`Repository::list`]
    /// enumeration.
    async fn len(&self) -> Result<u64, Self::Error> {
        let client = self.connect().await?;
        Ok(client.zcard(INDEX_KEY).await?)
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        let client = self.connect().await?;
        let count: u64 = client.exists(Self::blob_key(id)).await?;
        Ok(count > 0)
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        let client = self.connect().await?;
        let key = Self::blob_key(id);
        let pipeline = client.pipeline();
        let _: () = pipeline.get(&key).await?;
        let _: () = pipeline
            .custom(cmd!("PEXPIRETIME"), vec![key.clone()])
            .await?;
        let (data, expires_millis): (Option<Bytes>, i64) = pipeline.all().await?;
        Ok(data.map(|data| {
            let metadata = BlobMetadata::new(data.len() as u64).with_expires_nanos(
                (expires_millis > 0).then(|| expires_millis as u64 * 1_000_000),
            );
            Blob::new_unchecked(id.clone(), data).with_metadata(metadata)
        }))
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        let client = self.connect().await?;
        let key = Self::blob_key(id);
        let len: u64 = client.strlen(&key).await?;
        if len > 0 {
            return Ok(Some(len));
        }
        // STRLEN returns 0 for missing keys, so disambiguate empty blobs:
        let count: u64 = client.exists(&key).await?;
        Ok((count > 0).then_some(0))
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        let id = Id::of(&data);
        let expiration = self
            .ttl
            .map(|ttl| Expiration::PX(ttl.as_millis().max(1) as i64));
        let client = self.connect().await?;
        let trx = client.multi();
        let _: () = trx
            .set(Self::blob_key(&id), data, expiration, None, false)
            .await?;
        let _: () = trx
            .zadd(
                INDEX_KEY,
                None,
                None,
                false,
                false,
                (0.0, id.to_hex().as_str()),
            )
            .await?;
        let _: Value = trx.exec(true).await?;
        Ok(id)
    }

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        let client = self.connect().await?;
        let trx = client.multi();
        let _: () = trx.del(Self::blob_key(id)).await?;
        let _: () = trx.zrem(INDEX_KEY, id.to_hex().as_str()).await?;
        let (removed, _): (u64, u64) = trx.exec(true).await?;
        Ok(removed > 0)
    }

    async fn expire(&mut self, id: &Id, expires_nanos: Option<u64>) -> Result<bool, Self::Error> {
        let client = self.connect().await?;
        let key = Self::blob_key(id);
        let result: i64 = match expires_nanos {
            Some(nanos) => {
                let millis = nanos / 1_000_000;
                client
                    .custom(cmd!("PEXPIREAT"), vec![key, millis.to_string()])
                    .await?
            },
            None => client.persist(key).await?,
        };
        Ok(result > 0)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        let client = self.connect().await?;
        loop {
            let page: Vec<String> = client
                .zrangebylex(INDEX_KEY, "-", "+", Some((0, PAGE_SIZE as i64)))
                .await?;
            if page.is_empty() {
                return Ok(());
            }
            let keys: Vec<String> = page
                .iter()
                .map(|hex| format!("{}{}", BLOB_KEY_PREFIX, hex))
                .collect();
            let trx = client.multi();
            let _: () = trx.del(keys).await?;
            let _: () = trx.zrem(INDEX_KEY, page).await?;
            let _: Value = trx.exec(true).await?;
        }
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        let repository = self.clone();
        let limit = options.limit.unwrap_or(usize::MAX);
        let prefix = options.prefix.clone();
        // The exclusive lower bound to start the index scan from: the
        // greater of the cursor and the prefix's start.
        let start = {
            let after = options.after.as_ref().map(|id| id.to_hex());
            match (after, &options.prefix) {
                (Some(after), Some(prefix)) if prefix.as_str() > after.as_str() => {
                    format!("[{}", prefix)
                },
                (Some(after), _) => format!("({}", after),
                (None, Some(prefix)) => format!("[{}", prefix),
                (None, None) => String::from("-"),
            }
        };
        stream::try_unfold(
            (repository, start, false),
            |(repository, cursor, done)| async move {
                if done {
                    return Ok(None);
                }
                let client = repository.connect().await?;
                let page: Vec<String> = client
                    .zrangebylex(INDEX_KEY, cursor.as_str(), "+", Some((0, PAGE_SIZE as i64)))
                    .await?;
                let done = page.len() < PAGE_SIZE;
                let cursor = page
                    .last()
                    .map_or_else(|| String::from("+"), |hex| format!("({}", hex));
                // Filter out expired blobs whose index entries linger, and
                // opportunistically prune those stale entries:
                let page = if page.is_empty() {
                    page
                } else {
                    let pipeline = client.pipeline();
                    for hex in &page {
                        let _: () = pipeline
                            .exists(format!("{}{}", BLOB_KEY_PREFIX, hex))
                            .await?;
                    }
                    let exists: Vec<u64> = pipeline.all().await?;
                    let mut live = Vec::new();
                    let mut stale = Vec::new();
                    for (hex, count) in page.into_iter().zip(exists) {
                        if count > 0 { &mut live } else { &mut stale }.push(hex);
                    }
                    if !stale.is_empty() {
                        let _: u64 = client.zrem(INDEX_KEY, stale).await?;
                    }
                    live
                };
                Result::<_, RepositoryError>::Ok(Some((page, (repository, cursor, done))))
            },
        )
        .map_ok(|page| stream::iter(page.into_iter().map(Ok)))
        .try_flatten()
        .try_take_while(move |hex| {
            // The index is sorted, so stop as soon as the prefix range ends:
            future::ready(Ok(prefix
                .as_ref()
                .is_none_or(|prefix| hex.starts_with(prefix.as_str()))))
        })
        .try_filter_map(move |hex| {
            future::ready(Ok(Id::from_hex(&hex).ok().filter(|id| options.matches(id))))
        })
        .take(limit)
        .boxed()
    }
}

impl core::fmt::Debug for ValkeyRepository {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ValkeyRepository").finish_non_exhaustive()
    }
}
