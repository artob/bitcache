// This is free and unencumbered software released into the public domain.

#[cfg(feature = "alloc")]
use alloc::borrow::Cow;

#[cfg(feature = "std")]
pub const EPOCH: std::time::SystemTime = std::time::SystemTime::UNIX_EPOCH;

/// Metadata associated with a content-addressed blob.
///
/// Timestamps are represented internally as nanoseconds since the Unix epoch.
/// Repositories should leave a timestamp unset when the backing store cannot
/// represent its semantics reliably.
///
/// # Timestamps
///
/// - [`created`](Self::created) records the first insertion. Filesystem stores
///   map it to creation time (`crtime` or `btime`).
/// - [`updated`](Self::updated) records the most recent insertion, including a
///   no-op reinsertion. Filesystem stores map it to status-change time (`ctime`).
/// - [`accessed`](Self::accessed) records the most recent identifier-based
///   retrieval of the blob's contents or metadata. Maintenance access should
///   not advance it; filesystem stores map it to last-access time (`atime`).
/// - [`expires`](Self::expires) defines when the blob should be treated as
///   absent. Filesystem stores may use extended attributes when available.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BlobMetadata {
    /// The size of the blob's contents, in bytes.
    len: u64,

    /// The media type (MIME type) of the blob's contents, when known.
    #[cfg(feature = "alloc")]
    media_type: Option<Cow<'static, str>>,

    /// When the blob was first inserted into the repository.
    ///
    /// Filesystem repositories map this to file creation time (`crtime` or
    /// `btime`) when the filesystem exposes it.
    created: Option<u64>,

    /// When the blob was most recently inserted into the repository.
    ///
    /// This advances when reinserting an already-present blob, even though the
    /// content write itself is a no-op. Filesystem repositories map this to
    /// inode status-change time (`ctime`).
    #[cfg_attr(feature = "serde", serde(default))]
    updated: Option<u64>,

    /// When the blob's contents or metadata were most recently retrieved by
    /// its identifier.
    ///
    /// Repository-internal maintenance should not advance this timestamp.
    /// Filesystem repositories map this to last-access time (`atime`).
    accessed: Option<u64>,

    /// The time after which the blob should be treated as absent.
    ///
    /// Filesystem repositories may store this in extended attributes when
    /// available. An unset value means the blob does not expire.
    expires: Option<u64>,
}

impl core::fmt::Display for BlobMetadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut result = f.debug_tuple("");
        result.field(&self.len);
        #[cfg(feature = "alloc")]
        result.field(&self.media_type);
        result
            .field(&self.created)
            .field(&self.updated)
            .field(&self.accessed)
            .field(&self.expires)
            .finish()
    }
}

impl BlobMetadata {
    /// Creates empty metadata, with all fields unset.
    pub fn new(len: u64) -> Self {
        Self::default().with_len(len)
    }

    /// The size of the blob, in bytes.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// The media type (aka MIME type) of the blob's contents, if known.
    #[cfg(feature = "alloc")]
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    /// When the blob was first inserted into the repository, if known.
    #[cfg(feature = "std")]
    pub fn created(&self) -> Option<std::time::SystemTime> {
        use std::time::Duration;
        self.created
            .and_then(|t| EPOCH.checked_add(Duration::from_nanos(t)))
    }

    /// The first-insertion time, as nanoseconds since the Unix epoch, if known.
    pub fn created_nanos(&self) -> Option<u64> {
        self.created
    }

    /// The first-insertion time, as milliseconds since the Unix epoch, if known.
    pub fn created_millis(&self) -> Option<u64> {
        self.created.map(|t| t / 1e6 as u64)
    }

    /// The first-insertion time, as seconds since the Unix epoch, if known.
    pub fn created_secs(&self) -> Option<u64> {
        self.created.map(|t| t / 1e9 as u64)
    }

    /// When the blob was most recently inserted into the repository, if known.
    #[cfg(feature = "std")]
    pub fn updated(&self) -> Option<std::time::SystemTime> {
        use std::time::Duration;
        self.updated
            .and_then(|t| EPOCH.checked_add(Duration::from_nanos(t)))
    }

    /// The last-insertion time, as nanoseconds since the Unix epoch, if known.
    pub fn updated_nanos(&self) -> Option<u64> {
        self.updated
    }

    /// The last-insertion time, as milliseconds since the Unix epoch, if known.
    pub fn updated_millis(&self) -> Option<u64> {
        self.updated.map(|t| t / 1e6 as u64)
    }

    /// The last-insertion time, as seconds since the Unix epoch, if known.
    pub fn updated_secs(&self) -> Option<u64> {
        self.updated.map(|t| t / 1e9 as u64)
    }

    /// When the blob's contents or metadata were most recently retrieved by
    /// its identifier, if known.
    #[cfg(feature = "std")]
    pub fn accessed(&self) -> Option<std::time::SystemTime> {
        use std::time::Duration;
        self.accessed
            .and_then(|t| EPOCH.checked_add(Duration::from_nanos(t)))
    }

    /// The last-retrieval time, as nanoseconds since the Unix epoch, if known.
    pub fn accessed_nanos(&self) -> Option<u64> {
        self.accessed
    }

    /// The last-retrieval time, as milliseconds since the Unix epoch, if known.
    pub fn accessed_millis(&self) -> Option<u64> {
        self.accessed.map(|t| t / 1e6 as u64)
    }

    /// The last-retrieval time, as seconds since the Unix epoch, if known.
    pub fn accessed_secs(&self) -> Option<u64> {
        self.accessed.map(|t| t / 1e9 as u64)
    }

    /// The time after which the blob should be treated as absent, if set.
    #[cfg(feature = "std")]
    pub fn expires(&self) -> Option<std::time::SystemTime> {
        use std::time::Duration;
        self.expires
            .and_then(|t| EPOCH.checked_add(Duration::from_nanos(t)))
    }

    /// The expiration time of the blob, as nanoseconds since the Unix epoch,
    /// if known.
    pub fn expires_nanos(&self) -> Option<u64> {
        self.expires
    }

    /// The expiration time of the blob, as milliseconds since the Unix epoch,
    /// if known.
    pub fn expires_millis(&self) -> Option<u64> {
        self.expires.map(|t| t / 1e6 as u64)
    }

    /// The expiration time of the blob, as seconds since the Unix epoch,
    /// if known.
    pub fn expires_secs(&self) -> Option<u64> {
        self.expires.map(|t| t / 1e9 as u64)
    }

    /// Sets the size of the blob, in bytes.
    pub fn with_len(mut self, input: impl Into<u64>) -> Self {
        self.len = input.into();
        self
    }

    /// Sets the media type (aka MIME type) of the blob's contents.
    #[cfg(feature = "alloc")]
    pub fn with_media_type(mut self, input: Option<Cow<'static, str>>) -> Self {
        self.media_type = input;
        self
    }

    /// Sets the time when the blob was first inserted into the repository.
    #[cfg(feature = "std")]
    pub fn with_created(mut self, input: impl Into<Option<std::time::SystemTime>>) -> Self {
        self.created = input
            .into()
            .map(|t| t.duration_since(EPOCH).unwrap().as_nanos() as u64);
        self
    }

    /// Sets the first-insertion time, as nanoseconds since the Unix epoch.
    pub fn with_created_nanos(mut self, input: impl Into<Option<u64>>) -> Self {
        self.created = input.into();
        self
    }

    /// Sets the time when the blob was most recently inserted into the repository.
    #[cfg(feature = "std")]
    pub fn with_updated(mut self, input: impl Into<Option<std::time::SystemTime>>) -> Self {
        self.updated = input
            .into()
            .map(|t| t.duration_since(EPOCH).unwrap().as_nanos() as u64);
        self
    }

    /// Sets the last-insertion time, as nanoseconds since the Unix epoch.
    pub fn with_updated_nanos(mut self, input: impl Into<Option<u64>>) -> Self {
        self.updated = input.into();
        self
    }

    /// Sets when the blob's contents or metadata were last retrieved by identifier.
    #[cfg(feature = "std")]
    pub fn with_accessed(mut self, input: impl Into<Option<std::time::SystemTime>>) -> Self {
        self.accessed = input
            .into()
            .map(|t| t.duration_since(EPOCH).unwrap().as_nanos() as u64);
        self
    }

    /// Sets the last-retrieval time, as nanoseconds since the Unix epoch.
    pub fn with_accessed_nanos(mut self, input: impl Into<Option<u64>>) -> Self {
        self.accessed = input.into();
        self
    }

    /// Sets the time after which the blob should be treated as absent.
    #[cfg(feature = "std")]
    pub fn with_expires(mut self, input: impl Into<Option<std::time::SystemTime>>) -> Self {
        self.expires = input
            .into()
            .map(|t| t.duration_since(EPOCH).unwrap().as_nanos() as u64);
        self
    }

    /// Sets the expiration time, as nanoseconds since the Unix epoch.
    pub fn with_expires_nanos(mut self, input: impl Into<Option<u64>>) -> Self {
        self.expires = input.into();
        self
    }
}
