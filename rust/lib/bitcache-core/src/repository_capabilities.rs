// This is free and unencumbered software released into the public domain.

/// Capabilities advertised by a blob repository.
///
/// Clients can inspect these capabilities before performing an operation that
/// depends on optional repository functionality. A capability indicates that
/// the repository adapter supports the feature; individual operations can
/// still fail because of backing-store errors or platform limitations.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct RepositoryCapabilities {
    blob_metadata: BlobMetadataCapabilities,
}

impl RepositoryCapabilities {
    /// No optional repository capabilities.
    pub const NONE: Self = Self::new();

    /// Creates an empty set of repository capabilities.
    pub const fn new() -> Self {
        Self {
            blob_metadata: BlobMetadataCapabilities::NONE,
        }
    }

    /// Returns the repository's blob metadata capabilities.
    pub const fn blob_metadata(self) -> BlobMetadataCapabilities {
        self.blob_metadata
    }

    /// Sets the repository's blob metadata capabilities.
    pub const fn with_blob_metadata(mut self, capabilities: BlobMetadataCapabilities) -> Self {
        self.blob_metadata = capabilities;
        self
    }
}

/// Capabilities for metadata associated with stored blobs.
///
/// Creation, update, and access timestamps are maintained automatically by
/// repositories that support them. Expiry and media type can additionally be
/// requested through [`PutOptions`](crate::PutOptions) and changed through the
/// corresponding [`Repository`](crate::Repository) methods.
///
/// Timestamp values can remain absent for an individual blob when its backing
/// store cannot provide a reliable value, even when the adapter advertises the
/// corresponding capability.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct BlobMetadataCapabilities(u8);

impl BlobMetadataCapabilities {
    const CREATED: u8 = 1 << 0;
    const UPDATED: u8 = 1 << 1;
    const ACCESSED: u8 = 1 << 2;
    const EXPIRES: u8 = 1 << 3;
    const MEDIA_TYPE: u8 = 1 << 4;

    /// No blob metadata capabilities.
    pub const NONE: Self = Self(0);

    /// All blob metadata capabilities.
    pub const ALL: Self =
        Self(Self::CREATED | Self::UPDATED | Self::ACCESSED | Self::EXPIRES | Self::MEDIA_TYPE);

    /// Creates an empty set of blob metadata capabilities.
    pub const fn new() -> Self {
        Self::NONE
    }

    /// Returns whether the repository records the blob's original insertion
    /// timestamp.
    pub const fn created(self) -> bool {
        self.contains(Self::CREATED)
    }

    /// Returns whether the repository records the blob's most recent insertion
    /// timestamp.
    pub const fn updated(self) -> bool {
        self.contains(Self::UPDATED)
    }

    /// Returns whether the repository records identifier-based blob accesses.
    pub const fn accessed(self) -> bool {
        self.contains(Self::ACCESSED)
    }

    /// Returns whether the repository can store and enforce blob expiration.
    pub const fn expires(self) -> bool {
        self.contains(Self::EXPIRES)
    }

    /// Returns whether the repository can store an explicit media (MIME) type.
    pub const fn media_type(self) -> bool {
        self.contains(Self::MEDIA_TYPE)
    }

    /// Adds support for the creation timestamp.
    pub const fn with_created(mut self) -> Self {
        self.0 |= Self::CREATED;
        self
    }

    /// Adds support for the most recent insertion timestamp.
    pub const fn with_updated(mut self) -> Self {
        self.0 |= Self::UPDATED;
        self
    }

    /// Adds support for the most recent identifier-based access timestamp.
    pub const fn with_accessed(mut self) -> Self {
        self.0 |= Self::ACCESSED;
        self
    }

    /// Adds support for blob expiration.
    pub const fn with_expires(mut self) -> Self {
        self.0 |= Self::EXPIRES;
        self
    }

    /// Adds support for an explicit media (MIME) type.
    pub const fn with_media_type(mut self) -> Self {
        self.0 |= Self::MEDIA_TYPE;
        self
    }

    const fn contains(self, capability: u8) -> bool {
        self.0 & capability != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Blob, Bytes, Id, PutOptions, Repository};
    use core::{convert::Infallible, time::Duration};
    use futures_util::FutureExt;

    struct UnsupportedMetadataRepository;

    impl Repository for UnsupportedMetadataRepository {
        type Error = Infallible;

        async fn get(&self, _id: &Id) -> Result<Option<Blob>, Self::Error> {
            Ok(None)
        }

        async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
            Ok(Id::of(data))
        }

        async fn set_expiry(
            &mut self,
            _id: &Id,
            _expires_nanos: Option<u64>,
        ) -> Result<bool, Self::Error> {
            panic!("unsupported expiry setter must not be called")
        }

        async fn set_media_type(
            &mut self,
            _id: &Id,
            _media_type: Option<&str>,
        ) -> Result<bool, Self::Error> {
            panic!("unsupported media-type setter must not be called")
        }

        async fn remove(&mut self, _id: &Id) -> Result<bool, Self::Error> {
            Ok(false)
        }

        async fn clear(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn default_put_skips_unsupported_metadata_operations() {
        let mut repository = UnsupportedMetadataRepository;
        let options = PutOptions::new()
            .with_ttl(Duration::from_secs(60))
            .with_media_type(Some("text/plain".into()));
        let result = repository
            .put_with_options(Bytes::from_static(b"capabilities"), options)
            .now_or_never()
            .expect("the test repository future is immediately ready");
        assert!(result.is_ok());
    }

    struct SupportedMetadataRepository {
        expiry_set: bool,
        media_type_set: bool,
    }

    impl Repository for SupportedMetadataRepository {
        type Error = Infallible;

        fn capabilities(&self) -> RepositoryCapabilities {
            RepositoryCapabilities::new().with_blob_metadata(
                BlobMetadataCapabilities::new()
                    .with_expires()
                    .with_media_type(),
            )
        }

        async fn get(&self, _id: &Id) -> Result<Option<Blob>, Self::Error> {
            Ok(None)
        }

        async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
            Ok(Id::of(data))
        }

        async fn set_expiry(
            &mut self,
            _id: &Id,
            _expires_nanos: Option<u64>,
        ) -> Result<bool, Self::Error> {
            self.expiry_set = true;
            Ok(true)
        }

        async fn set_media_type(
            &mut self,
            _id: &Id,
            _media_type: Option<&str>,
        ) -> Result<bool, Self::Error> {
            self.media_type_set = true;
            Ok(true)
        }

        async fn remove(&mut self, _id: &Id) -> Result<bool, Self::Error> {
            Ok(false)
        }

        async fn clear(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn default_put_applies_supported_metadata_operations() {
        let mut repository = SupportedMetadataRepository {
            expiry_set: false,
            media_type_set: false,
        };
        let options = PutOptions::new()
            .with_ttl(Duration::from_secs(60))
            .with_media_type(Some("text/plain".into()));
        let result = repository
            .put_with_options(Bytes::from_static(b"capabilities"), options)
            .now_or_never()
            .expect("the test repository future is immediately ready");

        assert!(result.is_ok());
        assert!(repository.expiry_set);
        assert!(repository.media_type_set);
    }

    #[test]
    fn metadata_capability_queries_reflect_builders() {
        let metadata = BlobMetadataCapabilities::new()
            .with_created()
            .with_updated()
            .with_accessed()
            .with_expires()
            .with_media_type();
        let capabilities = RepositoryCapabilities::new().with_blob_metadata(metadata);

        assert!(capabilities.blob_metadata().created());
        assert!(capabilities.blob_metadata().updated());
        assert!(capabilities.blob_metadata().accessed());
        assert!(capabilities.blob_metadata().expires());
        assert!(capabilities.blob_metadata().media_type());
    }
}
