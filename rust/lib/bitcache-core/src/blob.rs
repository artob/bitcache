// This is free and unencumbered software released into the public domain.

use crate::Id;
use bytes::Bytes;

/// A content-addressed blob.
///
/// A blob carries its metadata (its content-derived [`Id`] and size) and
/// provides access to its contents through the [`Blob::read`] method, rather
/// than exposing the underlying byte storage directly.
#[derive(Clone, Debug)]
pub struct Blob {
    id: Id,
    data: Bytes,
}

impl Blob {
    /// Creates a blob from the given data, computing its ID.
    pub fn compute(data: impl Into<Bytes>) -> Self {
        let data = data.into();
        Self {
            id: Id::of(&data),
            data,
        }
    }

    /// Creates a blob with a known ID.
    ///
    /// The caller is responsible for ensuring that `id` is in fact the
    /// content-derived ID of `data`; see [`Blob::compute`] otherwise.
    pub fn new(id: Id, data: impl Into<Bytes>) -> Self {
        Self {
            id,
            data: data.into(),
        }
    }

    /// The content-derived ID of this blob.
    pub fn id(&self) -> &Id {
        &self.id
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

/// An asynchronous reader over a blob's contents.
#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct BlobReader(Bytes);

#[cfg(feature = "std")]
impl futures_io::AsyncRead for BlobReader {
    fn poll_read(
        self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &mut [u8],
    ) -> core::task::Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        let n = this.0.len().min(buf.len());
        let chunk = this.0.split_to(n);
        buf[..n].copy_from_slice(&chunk);
        core::task::Poll::Ready(Ok(n))
    }
}

#[cfg(feature = "tokio")]
impl tokio::io::AsyncRead for BlobReader {
    fn poll_read(
        self: core::pin::Pin<&mut Self>,
        _cx: &mut core::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> core::task::Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let n = this.0.len().min(buf.remaining());
        let chunk = this.0.split_to(n);
        buf.put_slice(&chunk);
        core::task::Poll::Ready(Ok(()))
    }
}
