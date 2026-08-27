// This is free and unencumbered software released into the public domain.

use alloc::{borrow::Cow, boxed::Box, string::String, vec, vec::Vec};
use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, ListOrder, OpenError, PutOptions, Repository,
    RepositoryError, Stream,
    futures_util::{StreamExt, TryStreamExt, stream},
};
use core::time::Duration;
use turso::{Builder, Connection, Database, Row, Value};

const SCHEMA: &str = include_str!("../etc/schema.sql");
const SCHEMA_VERSION: i64 = 1;

/// A content-addressable repository backed by a local Turso database.
///
/// The database is initialized from the schema bundled with this crate when it
/// does not already contain a Bitcache schema. Existing databases are checked
/// for a compatible schema version and are never reinitialized destructively.
///
/// Clones share the underlying [`Database`], while each operation obtains its
/// own connection. This permits independent operations and transactions to run
/// concurrently.
#[derive(Clone, Debug)]
pub struct TursoRepository {
    database: Database,
    ttl: Option<Duration>,
}

impl TursoRepository {
    /// Opens or creates a local Turso database.
    ///
    /// The input may be a filesystem path, `:memory:`, or a `sqlite:`/`turso:`
    /// URL. For example: `bitcache.db`, `sqlite:bitcache.db`, or
    /// `sqlite:///var/lib/bitcache.db`.
    pub async fn open(input: &str) -> Result<Self, OpenError> {
        let path = local_path(input)?;
        let database = Builder::new_local(path).build().await.map_err(open_error)?;
        Self::new(database).await.map_err(OpenError::from)
    }

    /// Creates a repository from an existing Turso database.
    ///
    /// This initializes the Bitcache schema if necessary and validates the
    /// schema version otherwise.
    pub async fn new(database: Database) -> Result<Self, RepositoryError> {
        let repository = Self {
            database,
            ttl: None,
        };
        repository.initialize().await?;
        Ok(repository)
    }

    /// Configures a default time-to-live for stored blobs.
    ///
    /// An explicit TTL passed to [`Repository::put_with_ttl`] or
    /// [`Repository::put_with_options`] takes precedence over this default.
    pub fn with_ttl(mut self, ttl: impl Into<Option<Duration>>) -> Self {
        self.ttl = ttl.into();
        self
    }

    /// Returns the configured default time-to-live, if any.
    pub fn ttl(&self) -> Option<Duration> {
        self.ttl
    }

    /// Returns the underlying Turso database.
    pub fn database(&self) -> &Database {
        &self.database
    }

    fn connect(&self) -> Result<Connection, RepositoryError> {
        self.database.connect().map_err(repository_error)
    }

    async fn initialize(&self) -> Result<(), RepositoryError> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'bitcache_config')",
                (),
            )
            .await
            .map_err(repository_error)?;
        let exists = rows
            .next()
            .await
            .map_err(repository_error)?
            .ok_or_else(|| invalid_database("schema presence query returned no row"))?
            .get::<i64>(0)
            .map_err(repository_error)?;

        if exists == 0 {
            connection
                .execute_batch(SCHEMA)
                .await
                .map_err(repository_error)?;
            return Ok(());
        }

        let mut rows = connection
            .query("SELECT val FROM bitcache_config WHERE key = 'schema'", ())
            .await
            .map_err(repository_error)?;
        let version = rows
            .next()
            .await
            .map_err(repository_error)?
            .ok_or_else(|| invalid_database("Bitcache schema version is missing"))?
            .get::<i64>(0)
            .map_err(repository_error)?;
        if version != SCHEMA_VERSION {
            return Err(invalid_database("unsupported Bitcache schema version"));
        }
        Ok(())
    }

    async fn store(&mut self, data: Bytes, ttl: Option<Duration>) -> Result<Id, RepositoryError> {
        let id = Id::of(&data);
        let now = now_millis();
        let expires = ttl.or(self.ttl).map(|ttl| millis_after(now, ttl));
        let mut connection = self.connect()?;
        let transaction = connection.transaction().await.map_err(repository_error)?;

        transaction
            .execute(
                "INSERT INTO bitcache_blob (blake3) VALUES (?1) ON CONFLICT (blake3) DO NOTHING",
                [Value::Blob(id.as_slice().to_vec())],
            )
            .await
            .map_err(repository_error)?;

        let mut rows = transaction
            .query(
                "SELECT id FROM bitcache_blob WHERE blake3 = ?1",
                [Value::Blob(id.as_slice().to_vec())],
            )
            .await
            .map_err(repository_error)?;
        let database_id = rows
            .next()
            .await
            .map_err(repository_error)?
            .ok_or_else(|| invalid_database("stored blob has no database ID"))?
            .get::<i64>(0)
            .map_err(repository_error)?;
        drop(rows);

        transaction
            .execute(
                "INSERT INTO bitcache_data (id, encoding, data) VALUES (?1, NULL, ?2) \
                 ON CONFLICT (id) DO UPDATE SET encoding = excluded.encoding, data = excluded.data",
                vec![Value::Integer(database_id), Value::Blob(data.to_vec())],
            )
            .await
            .map_err(repository_error)?;
        transaction
            .execute(
                "INSERT INTO bitcache_meta (id, created, updated, accessed, expires, media_type) \
                 VALUES (?1, ?2, ?2, ?2, ?3, NULL) \
                 ON CONFLICT (id) DO UPDATE SET updated = excluded.updated, \
                 accessed = excluded.accessed, expires = excluded.expires, \
                 media_type = excluded.media_type",
                vec![
                    Value::Integer(database_id),
                    Value::Integer(now),
                    expires.map_or(Value::Null, Value::Integer),
                ],
            )
            .await
            .map_err(repository_error)?;
        transaction.commit().await.map_err(repository_error)?;
        Ok(id)
    }
}

impl Repository for TursoRepository {
    type Error = RepositoryError;

    async fn is_empty(&self) -> Result<bool, Self::Error> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT NOT EXISTS(\
                    SELECT 1 FROM bitcache_blob AS b \
                    JOIN bitcache_data AS d ON d.id = b.id \
                    JOIN bitcache_meta AS m ON m.id = b.id \
                    WHERE m.expires IS NULL OR m.expires > ?1\
                )",
                [now_millis()],
            )
            .await
            .map_err(repository_error)?;
        Ok(rows
            .next()
            .await
            .map_err(repository_error)?
            .ok_or_else(|| invalid_database("empty query returned no row"))?
            .get::<i64>(0)
            .map_err(repository_error)?
            != 0)
    }

    async fn len(&self) -> Result<u64, Self::Error> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT count(*) FROM bitcache_blob AS b \
                 JOIN bitcache_data AS d ON d.id = b.id \
                 JOIN bitcache_meta AS m ON m.id = b.id \
                 WHERE m.expires IS NULL OR m.expires > ?1",
                [now_millis()],
            )
            .await
            .map_err(repository_error)?;
        let count = rows
            .next()
            .await
            .map_err(repository_error)?
            .ok_or_else(|| invalid_database("count query returned no row"))?
            .get::<i64>(0)
            .map_err(repository_error)?;
        u64::try_from(count).map_err(other_error)
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT EXISTS(\
                    SELECT 1 FROM bitcache_blob AS b \
                    JOIN bitcache_data AS d ON d.id = b.id \
                    JOIN bitcache_meta AS m ON m.id = b.id \
                    WHERE b.blake3 = ?1 AND (m.expires IS NULL OR m.expires > ?2)\
                )",
                vec![
                    Value::Blob(id.as_slice().to_vec()),
                    Value::Integer(now_millis()),
                ],
            )
            .await
            .map_err(repository_error)?;
        Ok(rows
            .next()
            .await
            .map_err(repository_error)?
            .ok_or_else(|| invalid_database("presence query returned no row"))?
            .get::<i64>(0)
            .map_err(repository_error)?
            != 0)
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        let connection = self.connect()?;
        let now = now_millis();
        let mut rows = connection
            .query(
                "SELECT b.id, d.encoding, d.data, m.created, m.accessed, m.expires, m.media_type \
                 FROM bitcache_blob AS b \
                 JOIN bitcache_data AS d ON d.id = b.id \
                 JOIN bitcache_meta AS m ON m.id = b.id \
                 WHERE b.blake3 = ?1 AND (m.expires IS NULL OR m.expires > ?2)",
                vec![Value::Blob(id.as_slice().to_vec()), Value::Integer(now)],
            )
            .await
            .map_err(repository_error)?;
        let Some(row) = rows.next().await.map_err(repository_error)? else {
            return Ok(None);
        };

        let database_id = integer(&row, 0)?;
        if !matches!(row.get_value(1).map_err(repository_error)?, Value::Null) {
            return Err(RepositoryError::UnsupportedOperation);
        }
        let data = blob(&row, 2)?;
        let created = optional_u64(&row, 3)?;
        let expires = optional_u64(&row, 5)?;
        let media_type = optional_text(&row, 6)?;
        drop(rows);

        connection
            .execute(
                "UPDATE bitcache_meta SET accessed = ?1 WHERE id = ?2",
                [now, database_id],
            )
            .await
            .map_err(repository_error)?;

        let metadata = BlobMetadata::new(data.len() as u64)
            .with_media_type(media_type.map(Cow::Owned))
            .with_created_nanos(created.map(|t| t * 1000))
            .with_accessed_nanos(u64::try_from(now).ok().map(|t| t * 1000))
            .with_expires_nanos(expires.map(|t| t * 1000));
        Ok(Some(
            Blob::new_unchecked(id.clone(), Bytes::from(data)).with_metadata(metadata),
        ))
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        let connection = self.connect()?;
        let mut rows = connection
            .query(
                "SELECT length(d.data) FROM bitcache_blob AS b \
                 JOIN bitcache_data AS d ON d.id = b.id \
                 JOIN bitcache_meta AS m ON m.id = b.id \
                 WHERE b.blake3 = ?1 AND (m.expires IS NULL OR m.expires > ?2)",
                vec![
                    Value::Blob(id.as_slice().to_vec()),
                    Value::Integer(now_millis()),
                ],
            )
            .await
            .map_err(repository_error)?;
        let Some(row) = rows.next().await.map_err(repository_error)? else {
            return Ok(None);
        };
        Ok(Some(u64::try_from(integer(&row, 0)?).map_err(other_error)?))
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        self.store(data, None).await
    }

    async fn put_with_options(
        &mut self,
        data: Bytes,
        options: PutOptions,
    ) -> Result<Id, Self::Error> {
        self.store(data, options.ttl).await
    }

    async fn put_with_ttl(
        &mut self,
        data: Bytes,
        ttl: Option<Duration>,
    ) -> Result<Id, Self::Error> {
        self.store(data, ttl).await
    }

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().await.map_err(repository_error)?;
        let mut rows = transaction
            .query(
                "SELECT b.id FROM bitcache_blob AS b \
                 JOIN bitcache_data AS d ON d.id = b.id \
                 JOIN bitcache_meta AS m ON m.id = b.id \
                 WHERE b.blake3 = ?1 AND (m.expires IS NULL OR m.expires > ?2)",
                vec![
                    Value::Blob(id.as_slice().to_vec()),
                    Value::Integer(now_millis()),
                ],
            )
            .await
            .map_err(repository_error)?;
        let Some(row) = rows.next().await.map_err(repository_error)? else {
            return Ok(false);
        };
        let database_id = integer(&row, 0)?;
        drop(rows);

        transaction
            .execute("DELETE FROM bitcache_meta WHERE id = ?1", [database_id])
            .await
            .map_err(repository_error)?;
        transaction
            .execute("DELETE FROM bitcache_data WHERE id = ?1", [database_id])
            .await
            .map_err(repository_error)?;
        transaction
            .execute("DELETE FROM bitcache_blob WHERE id = ?1", [database_id])
            .await
            .map_err(repository_error)?;
        transaction.commit().await.map_err(repository_error)?;
        Ok(true)
    }

    async fn set_expiry(
        &mut self,
        id: &Id,
        expires_nanos: Option<u64>,
    ) -> Result<bool, Self::Error> {
        let connection = self.connect()?;
        let now = now_millis();
        let expires = match expires_nanos {
            Some(value) => Value::Integer(i64::try_from(value / 1000).unwrap()),
            None => Value::Null,
        };
        let changed = connection
            .execute(
                "UPDATE bitcache_meta SET expires = ?1, updated = ?2 \
                 WHERE id = (SELECT id FROM bitcache_blob WHERE blake3 = ?3) \
                 AND (expires IS NULL OR expires > ?2)",
                vec![
                    expires,
                    Value::Integer(now),
                    Value::Blob(id.as_slice().to_vec()),
                ],
            )
            .await
            .map_err(repository_error)?;
        Ok(changed > 0)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction().await.map_err(repository_error)?;
        transaction
            .execute("DELETE FROM bitcache_meta", ())
            .await
            .map_err(repository_error)?;
        transaction
            .execute("DELETE FROM bitcache_data", ())
            .await
            .map_err(repository_error)?;
        transaction
            .execute("DELETE FROM bitcache_blob", ())
            .await
            .map_err(repository_error)?;
        transaction.commit().await.map_err(repository_error)
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        let repository = self.clone();
        let order = match options.order.unwrap_or_default() {
            ListOrder::Ascending => "ASC",
            ListOrder::Descending => "DESC",
        };
        let sql = alloc::format!(
            "SELECT b.blake3 FROM bitcache_blob AS b \
             JOIN bitcache_data AS d ON d.id = b.id \
             JOIN bitcache_meta AS m ON m.id = b.id \
             WHERE (m.expires IS NULL OR m.expires > ?1) \
             AND (?2 IS NULL OR b.blake3 > ?2) \
             AND (?3 IS NULL OR substr(lower(hex(b.blake3)), 1, length(?3)) = ?3) \
             ORDER BY b.blake3 {order} LIMIT ?4"
        );
        let after = options
            .after
            .map_or(Value::Null, |id| Value::Blob(id.as_slice().to_vec()));
        let prefix = options
            .prefix
            .map_or(Value::Null, |prefix| Value::Text(prefix.as_str().into()));
        let limit = options
            .limit
            .and_then(|limit| i64::try_from(limit).ok())
            .unwrap_or(-1);
        let params = vec![
            Value::Integer(now_millis()),
            after,
            prefix,
            Value::Integer(limit),
        ];

        stream::once(async move {
            let connection = repository.connect()?;
            connection
                .query(sql, params)
                .await
                .map_err(repository_error)
        })
        .map_ok(|rows| {
            stream::try_unfold(rows, |mut rows| async move {
                let Some(row) = rows.next().await.map_err(repository_error)? else {
                    return Ok(None);
                };
                let id = Id::from_slice(&blob(&row, 0)?).map_err(other_error)?;
                Ok(Some((id, rows)))
            })
        })
        .try_flatten()
        .boxed()
    }
}

fn local_path(input: &str) -> Result<&str, OpenError> {
    let path = if let Some(path) = input
        .strip_prefix("sqlite:")
        .or_else(|| input.strip_prefix("turso:"))
    {
        path.strip_prefix("//").unwrap_or(path)
    } else if input.contains("://") {
        return Err(OpenError::InvalidUrl);
    } else {
        input
    };
    if path.is_empty() {
        Err(OpenError::InvalidUrl)
    } else {
        Ok(path)
    }
}

fn now_millis() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    millis.min(i64::MAX as u128) as i64
}

fn millis_after(now: i64, ttl: Duration) -> i64 {
    let ttl = ttl.as_millis().min(i64::MAX as u128) as i64;
    now.saturating_add(ttl)
}

fn integer(row: &Row, index: usize) -> Result<i64, RepositoryError> {
    match row.get_value(index).map_err(repository_error)? {
        Value::Integer(value) => Ok(value),
        _ => Err(invalid_database("expected an integer column")),
    }
}

fn blob(row: &Row, index: usize) -> Result<Vec<u8>, RepositoryError> {
    match row.get_value(index).map_err(repository_error)? {
        Value::Blob(value) => Ok(value),
        _ => Err(invalid_database("expected a blob column")),
    }
}

fn optional_u64(row: &Row, index: usize) -> Result<Option<u64>, RepositoryError> {
    match row.get_value(index).map_err(repository_error)? {
        Value::Null => Ok(None),
        Value::Integer(value) => Ok(Some(u64::try_from(value).map_err(other_error)?)),
        _ => Err(invalid_database("expected an optional integer column")),
    }
}

fn optional_text(row: &Row, index: usize) -> Result<Option<String>, RepositoryError> {
    match row.get_value(index).map_err(repository_error)? {
        Value::Null => Ok(None),
        Value::Text(value) => Ok(Some(value)),
        _ => Err(invalid_database("expected an optional text column")),
    }
}

fn invalid_database(message: &str) -> RepositoryError {
    repository_error(turso::Error::Error(message.into()))
}

fn repository_error(error: turso::Error) -> RepositoryError {
    other_error(error)
}

fn open_error(error: turso::Error) -> OpenError {
    OpenError::Other(Box::new(error))
}

fn other_error(error: impl core::error::Error + Send + Sync + 'static) -> RepositoryError {
    RepositoryError::Other(Box::new(error))
}
