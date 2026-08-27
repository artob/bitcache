// This is free and unencumbered software released into the public domain.

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
        .ok()
        .map(|now| (now + ttl).as_nanos() as u64)
}

/// Unavailable without a clock (the `std` feature).
#[cfg(not(feature = "std"))]
fn expires_nanos_after(_ttl: Duration) -> Option<u64> {
    None
}
