// This is free and unencumbered software released into the public domain.

use crate::Compression;

/// Options for compacting a repository's physical storage.
///
/// See [`Repository::compact_with_options`](crate::Repository::compact_with_options).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactOptions {
    /// The target compression scheme for stored blobs.
    ///
    /// Backends without physical compression support ignore this.
    pub compression: Compression,
}

impl Default for CompactOptions {
    fn default() -> Self {
        Self {
            compression: Compression::XzFast,
        }
    }
}

impl CompactOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the target compression scheme for stored blobs.
    pub fn with_compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }
}
