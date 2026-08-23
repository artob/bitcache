// This is free and unencumbered software released into the public domain.

use bytes::Bytes;

#[derive(Clone, Debug)]
pub struct Blob(Bytes);

impl Blob {
    pub fn new(bytes: impl Into<Bytes>) -> Self {
        Self(bytes.into())
    }

    pub fn len(&self) -> u64 {
        self.0.len() as _
    }
}

impl From<Bytes> for Blob {
    fn from(bytes: Bytes) -> Self {
        Self(bytes)
    }
}

impl From<Blob> for Bytes {
    fn from(blob: Blob) -> Self {
        blob.0
    }
}
