// This is free and unencumbered software released into the public domain.

use alloc::{format, string::String, sync::Arc, vec::Vec};
use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, OpenError, Repository, RepositoryError, Stream,
    futures_util::{StreamExt, TryStreamExt, future, stream},
};
use fred::{
    clients::Client,
    interfaces::{ClientLike, KeysInterface, SortedSetsInterface, TransactionInterface},
    types::{Value, config::Config},
};
use tokio::sync::OnceCell;

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
/// The connection is established lazily on first use; cloning the repository
/// shares the underlying connection.
#[derive(Clone)]
pub struct ValkeyRepository {
    client: Client,
    connected: Arc<OnceCell<()>>,
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
    /// - `redis://localhost:6379`
    ///
    /// The connection itself is established lazily upon first use.
    pub fn open(url: &str) -> Result<Self, OpenError> {
        let config = Config::from_url(&Self::normalize_url(url))?;
        Ok(Self::new(Client::new(config, None, None, None)))
    }

    /// Creates a new repository backed by the given client.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            connected: Arc::new(OnceCell::new()),
        }
    }

    /// The underlying fred client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Normalizes `valkey:` URLs (and scheme-less `//host:port` forms) to
    /// the `redis:` scheme understood by the fred client.
    fn normalize_url(url: &str) -> String {
        if let Some(rest) = url.strip_prefix("valkey") {
            format!("redis{}", rest)
        } else if url.starts_with("redis") {
            url.into()
        } else {
            format!("redis:{}", url)
        }
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
        let data: Option<Bytes> = client.get(Self::blob_key(id)).await?;
        Ok(data.map(|data| {
            let metadata = BlobMetadata::new(data.len() as u64);
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
        let client = self.connect().await?;
        let trx = client.multi();
        let _: () = trx
            .set(Self::blob_key(&id), data, None, None, false)
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
