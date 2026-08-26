// This is free and unencumbered software released into the public domain.

use crate::{BlobMetadata, Id};
use bytes::Bytes;

/// An asynchronous reader over a blob's contents.
#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct BlobReader(pub(crate) Bytes);

impl BlobReader {
    pub fn into_bytes(self) -> Bytes {
        self.0
    }
}

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
