// This is free and unencumbered software released into the public domain.

#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
use core::time::Duration;

/// Options for storing a blob in a repository.
///
/// See [`Repository::put_with_options`](crate::Repository::put_with_options).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PutOptions {
    /// Expire the blob this long after it is stored.
    ///
    /// Honored by repositories that support blob expiration; others store
    /// the blob persistently. See
    /// [`Repository::put_with_options`](crate::Repository::put_with_options).
    pub ttl: Option<Duration>,

    /// The explicit media type (MIME type) of the blob's contents.
    ///
    /// Honored by repositories that support media-type metadata; others store
    /// the blob without it.
    #[cfg(feature = "alloc")]
    pub media_type: Option<Cow<'static, str>>,
}

impl PutOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Expires the blob the given duration after it is stored.
    pub fn with_ttl(mut self, ttl: impl Into<Option<Duration>>) -> Self {
        self.ttl = ttl.into();
        self
    }

    /// Sets the explicit media type (MIME type) of the blob's contents.
    #[cfg(feature = "alloc")]
    pub fn with_media_type(mut self, media_type: impl Into<Option<Cow<'static, str>>>) -> Self {
        self.media_type = media_type.into();
        self
    }

    /// Returns the explicit media type (MIME type), if set.
    #[cfg(feature = "alloc")]
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    /// The requested expiration time as nanoseconds since the Unix epoch,
    /// resolved against the current time, when a TTL is set (and a clock is
    /// available, which requires the `std` feature).
    pub fn expires_nanos(&self) -> Option<u64> {
        self.ttl.and_then(expires_nanos_after)
    }
}

/// The current time plus the given duration, as nanoseconds since the Unix
/// epoch.
#[cfg(feature = "std")]
fn expires_nanos_after(ttl: Duration) -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .ok()?
        .checked_add(ttl)?
        .as_nanos()
        .try_into()
        .ok()
}

/// Unavailable without a clock (the `std` feature).
#[cfg(not(feature = "std"))]
fn expires_nanos_after(_ttl: Duration) -> Option<u64> {
    None
}
