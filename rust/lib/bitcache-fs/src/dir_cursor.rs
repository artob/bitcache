// This is free and unencumbered software released into the public domain.

use crate::{BlobEncoding, Dir};
use bitcache_core::{Id, RepositoryError, Stream};
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use std::{string::String, vec::Vec};

/// The number of leading hexadecimal characters of the blob ID used to name
/// the shard subdirectory a blob is stored in.
pub(crate) const SHARD_PREFIX_LEN: usize = 2;

/// The number of shard subdirectories (`00` through `ff` for a prefix of 2).
pub(crate) const SHARD_COUNT: u32 = 1 << (4 * SHARD_PREFIX_LEN as u32);

/// A cursor over the physical blobs in a repository directory.
///
/// Implements [`Stream`], yielding `(Id, BlobEncoding)` tuples for every
/// physical blob file, in ID order (ascending by default, or descending). A
/// blob present in both encodings yields two consecutive tuples.
///
/// Entries are read one shard subdirectory at a time: only a single shard's
/// entries are ever materialized in memory, so memory use is proportional to
/// the largest shard rather than to the whole repository, and the first
/// entries are yielded without scanning every shard.
pub struct DirCursor {
    dir: Option<Dir>,
    descending: bool,
    /// An error to yield before terminating (e.g., from cloning the handle).
    error: Option<RepositoryError>,
    shards: std::vec::IntoIter<String>,
    batch: std::vec::IntoIter<(Id, BlobEncoding)>,
}

impl core::fmt::Debug for DirCursor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DirCursor")
            .field("descending", &self.descending)
            .finish_non_exhaustive()
    }
}

impl DirCursor {
    /// Creates a cursor over the given repository directory.
    pub fn new(dir: Dir, descending: bool) -> Self {
        let mut shards: Vec<String> = Self::shard_names().collect();
        if descending {
            shards.reverse();
        }
        Self {
            dir: Some(dir),
            descending,
            error: None,
            shards: shards.into_iter(),
            batch: Vec::new().into_iter(),
        }
    }

    /// Creates a cursor from a borrowed directory handle.
    ///
    /// If the handle can't be cloned, the cursor yields that error as its
    /// only item.
    pub fn open(dir: &Dir, descending: bool) -> Self {
        match dir.try_clone() {
            Ok(dir) => Self::new(dir, descending),
            Err(error) => Self {
                dir: None,
                descending,
                error: Some(error.into()),
                shards: Vec::new().into_iter(),
                batch: Vec::new().into_iter(),
            },
        }
    }

    /// The names of all shard subdirectories, in ascending order.
    pub(crate) fn shard_names() -> impl DoubleEndedIterator<Item = String> {
        (0..SHARD_COUNT).map(|shard| std::format!("{shard:0width$x}", width = SHARD_PREFIX_LEN))
    }

    /// Reads and sorts all blob entries of one shard subdirectory.
    fn read_shard(
        dir: &Dir,
        shard: &str,
        descending: bool,
    ) -> Result<Vec<(Id, BlobEncoding)>, RepositoryError> {
        let dir = match dir.open_dir(shard) {
            Ok(dir) => dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut entries = Vec::new();
        for entry in dir.entries()? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Ok(name) = entry.file_name() else {
                continue;
            };
            let (name, encoding) = match name.strip_suffix(".xz") {
                Some(name) => (name, BlobEncoding::Xz),
                None => (name.as_str(), BlobEncoding::Uncompressed),
            };
            if let Ok(id) = Id::from_hex(name) {
                entries.push((id, encoding));
            }
        }
        entries.sort_unstable();
        if descending {
            entries.reverse();
        }
        Ok(entries)
    }
}

impl Stream for DirCursor {
    type Item = Result<(Id, BlobEncoding), RepositoryError>;

    fn poll_next(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if let Some(error) = this.error.take() {
            this.dir = None;
            this.shards = Vec::new().into_iter();
            return Poll::Ready(Some(Err(error)));
        }
        loop {
            if let Some(item) = this.batch.next() {
                return Poll::Ready(Some(Ok(item)));
            }
            let Some(dir) = this.dir.as_ref() else {
                return Poll::Ready(None);
            };
            let Some(shard) = this.shards.next() else {
                this.dir = None;
                return Poll::Ready(None);
            };
            match Self::read_shard(dir, &shard, this.descending) {
                Ok(batch) => this.batch = batch.into_iter(),
                Err(error) => {
                    // Terminate the stream after yielding the error.
                    this.dir = None;
                    this.shards = Vec::new().into_iter();
                    return Poll::Ready(Some(Err(error)));
                },
            }
        }
    }
}
