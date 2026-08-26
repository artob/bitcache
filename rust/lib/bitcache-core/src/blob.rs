// This is free and unencumbered software released into the public domain.

use crate::{BlobMetadata, BlobReader, Id};
use bytes::Bytes;

/// A content-addressed blob.
///
/// A blob carries its metadata (e.g., its content-derived [`Id`] and size)
/// and provides access to its contents through the [`Blob::read`] method,
/// rather than exposing the underlying byte storage directly.
#[derive(Clone, Debug)]
pub struct Blob {
    id: Id,
    data: Bytes,
    metadata: BlobMetadata,
}

impl Blob {
    /// Creates a blob from the given data, computing its ID.
    pub fn compute(data: impl Into<Bytes>) -> Self {
        let data = data.into();
        Self {
            id: Id::of(&data),
            metadata: BlobMetadata::new(data.len() as u64),
            data,
        }
    }

    /// Creates a blob with a known ID.
    ///
    /// The caller is responsible for ensuring that `id` is in fact the
    /// content-derived ID of `data`; see [`Blob::compute`] otherwise.
    pub fn new_unchecked(id: Id, data: impl Into<Bytes>) -> Self {
        let data = data.into();
        Self {
            id,
            metadata: BlobMetadata::new(data.len() as u64),
            data,
        }
    }

    /// Attaches the given metadata to this blob.
    pub fn with_metadata(mut self, metadata: BlobMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// The content-derived ID of this blob.
    pub fn id(&self) -> &Id {
        &self.id
    }

    /// The metadata of this blob.
    pub fn metadata(&self) -> &BlobMetadata {
        &self.metadata
    }

    /// Mutable access to the metadata of this blob.
    pub fn metadata_mut(&mut self) -> &mut BlobMetadata {
        &mut self.metadata
    }

    /// The size of this blob in bytes.
    pub fn len(&self) -> u64 {
        self.data.len() as _
    }

    /// Returns `true` if this blob is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns an asynchronous reader over this blob's contents.
    ///
    /// The returned reader implements [`futures_io::AsyncRead`] and, when the
    /// `tokio` feature is enabled (the default), [`tokio::io::AsyncRead`] as
    /// well.
    #[cfg(feature = "std")]
    pub fn read(&self) -> BlobReader {
        BlobReader(self.data.clone())
    }
}
