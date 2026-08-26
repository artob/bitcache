// This is free and unencumbered software released into the public domain.

use crate::OpenError;
use bitcache_core::{DynRepository, RepositoryError};

#[cfg(feature = "std")]
pub fn open_env(
    name: impl AsRef<str>,
    default_value: impl AsRef<str>,
) -> Result<alloc::boxed::Box<DynRepository<'static, RepositoryError>>, OpenError> {
    use std::env::VarError;
    match std::env::var(name.as_ref()) {
        Ok(url) => open(url),
        Err(VarError::NotPresent) => open(default_value.as_ref()),
        Err(VarError::NotUnicode(_)) => Err(OpenError::InvalidUrl),
    }
}

/// Opens a Bitcache repository based on the given URL.
pub fn open(
    url: impl AsRef<str>,
) -> Result<alloc::boxed::Box<DynRepository<'static, RepositoryError>>, OpenError> {
    let url = url.as_ref();

    #[cfg(feature = "heap")]
    if url.is_empty() || url.starts_with("heap:") || url.starts_with("memory:") {
        return Ok(DynRepository::new_box(bitcache_heap::HeapRepository::new()));
    }

    #[cfg(feature = "fs")]
    if url.starts_with('.') || url.starts_with('/') || !url.contains(':') {
        return Ok(DynRepository::new_box(bitcache_fs::FsRepository::open(
            url,
        )?));
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

    Err(OpenError::UnknownAdapter)
}
