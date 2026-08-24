// This is free and unencumbered software released into the public domain.

#[cfg(feature = "alloc")]
use alloc::borrow::Cow;

#[cfg(feature = "std")]
pub const EPOCH: std::time::SystemTime = std::time::SystemTime::UNIX_EPOCH;

/// A content-addressed blob's metadata.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct BlobMetadata {
    len: u64,
    #[cfg(feature = "alloc")]
    media_type: Option<Cow<'static, str>>,
    created: Option<u64>,
    accessed: Option<u64>,
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

    /// The creation time of the blob, if known.
    #[cfg(feature = "std")]
    pub fn created(&self) -> Option<std::time::SystemTime> {
        use std::time::Duration;
        self.created
            .and_then(|t| EPOCH.checked_add(Duration::from_nanos(t)))
    }

    /// The creation time of the blob, as nanoseconds since the Unix epoch,
    /// if known.
    pub fn created_nanos(&self) -> Option<u64> {
        self.created
    }

    /// The creation time of the blob, as milliseconds since the Unix epoch,
    /// if known.
    pub fn created_millis(&self) -> Option<u64> {
        self.created.map(|t| t / 1e6 as u64)
    }

    /// The creation time of the blob, as seconds since the Unix epoch,
    /// if known.
    pub fn created_secs(&self) -> Option<u64> {
        self.created.map(|t| t / 1e9 as u64)
    }

    /// The last-accessed time of the blob, if known.
    #[cfg(feature = "std")]
    pub fn accessed(&self) -> Option<std::time::SystemTime> {
        use std::time::Duration;
        self.accessed
            .and_then(|t| EPOCH.checked_add(Duration::from_nanos(t)))
    }

    /// The last-accessed time of the blob, as nanoseconds since the Unix epoch,
    /// if known.
    pub fn accessed_nanos(&self) -> Option<u64> {
        self.accessed
    }

    /// The last-accessed time of the blob, as milliseconds since the Unix epoch,
    /// if known.
    pub fn accessed_millis(&self) -> Option<u64> {
        self.accessed.map(|t| t / 1e6 as u64)
    }

    /// The last-accessed time of the blob, as seconds since the Unix epoch,
    /// if known.
    pub fn accessed_secs(&self) -> Option<u64> {
        self.accessed.map(|t| t / 1e9 as u64)
    }

    /// The expiration time of the blob, if known.
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

    /// Sets the creation time.
    #[cfg(feature = "std")]
    pub fn with_created(mut self, input: impl Into<Option<std::time::SystemTime>>) -> Self {
        self.created = input
            .into()
            .map(|t| t.duration_since(EPOCH).unwrap().as_nanos() as u64);
        self
    }

    /// Sets the creation time, as nanoseconds since the Unix epoch.
    pub fn with_created_nanos(mut self, input: impl Into<Option<u64>>) -> Self {
        self.created = input.into();
        self
    }

    /// Sets the last-accessed time.
    #[cfg(feature = "std")]
    pub fn with_accessed(mut self, input: impl Into<Option<std::time::SystemTime>>) -> Self {
        self.accessed = input
            .into()
            .map(|t| t.duration_since(EPOCH).unwrap().as_nanos() as u64);
        self
    }

    /// Sets the last-accessed time, as nanoseconds since the Unix epoch.
    pub fn with_accessed_nanos(mut self, input: impl Into<Option<u64>>) -> Self {
        self.accessed = input.into();
        self
    }

    /// Sets the expiration time.
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
