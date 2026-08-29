// This is free and unencumbered software released into the public domain.

use crate::BlobEncoding;
use alloc::boxed::Box;
use cap_std::fs_utf8::File;
use core::pin::Pin;

/// An asynchronous reader over an uncompressed blob's contents.
///
/// The underlying repository file may itself be either uncompressed or XZ
/// compressed; callers always receive the original blob bytes.
#[cfg(feature = "tokio")]
pub struct BlobFile(Pin<Box<dyn bitcache_core::tokio::io::AsyncRead + Send>>);

#[cfg(feature = "tokio")]
impl std::fmt::Debug for BlobFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("BlobFile").finish_non_exhaustive()
    }
}

#[cfg(feature = "tokio")]
impl BlobFile {
    pub fn new(file: File, encoding: BlobEncoding) -> Self {
        use async_compression::tokio::bufread::XzDecoder;
        use bitcache_core::tokio::{fs::File, io::BufReader};
        let file = File::from_std(file.into_std());
        match encoding {
            BlobEncoding::Uncompressed => Self(Box::pin(file)),
            BlobEncoding::Xz => Self(Box::pin(XzDecoder::new(BufReader::new(file)))),
        }
    }
}

#[cfg(feature = "tokio")]
impl bitcache_core::tokio::io::AsyncRead for BlobFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
        buffer: &mut bitcache_core::tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.0.as_mut().poll_read(context, buffer)
    }
}
