// This is free and unencumbered software released into the public domain.

use crate::OpenError;
use bitcache_core::{DynRepository, RepositoryError};

/// Opens a Bitcache repository based on the given URL.
pub fn open(
    url: impl AsRef<str>,
) -> Result<alloc::boxed::Box<DynRepository<'static, RepositoryError>>, OpenError> {
    let url = url.as_ref();

    #[cfg(feature = "heap")]
    if url.starts_with("memory:") {
        return Ok(DynRepository::new_box(bitcache_heap::HeapRepository::new()));
    }

    #[cfg(feature = "fs")]
    if let Some(path) = url.strip_prefix("file:") {
        return Ok(DynRepository::new_box(bitcache_fs::FsRepository::open(
            path,
        )?));
    }

    #[cfg(feature = "opendal")]
    if let Some(url) = url.strip_prefix("opendal+") {
        return Ok(DynRepository::new_box(
            bitcache_opendal::DalRepository::open(url)?,
        ));
    }

    Err(OpenError::NoAdapter)
}
