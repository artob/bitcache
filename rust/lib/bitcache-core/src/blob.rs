// This is free and unencumbered software released into the public domain.

use bytes::Bytes;

#[derive(Clone, Debug)]
pub struct Blob(Bytes);

impl Blob {
    pub fn len(&self) -> u64 {
        self.0.len() as _
    }
}
