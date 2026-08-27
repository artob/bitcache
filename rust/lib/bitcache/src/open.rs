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
    if url.is_empty() {
        return Ok(DynRepository::new_box(bitcache_heap::HeapRepository::new()));
    }

    // Bare filesystem paths (not URLs):
    #[cfg(feature = "fs")]
    if url.starts_with('.') || url.starts_with('/') || !url.contains(':') {
        return Ok(DynRepository::new_box(bitcache_fs::FsRepository::open(
            url,
        )?));
    }

    let parsed = url::Url::parse(url).map_err(|_| OpenError::InvalidUrl)?;
    match parsed.scheme() {
        #[cfg(feature = "heap")]
        "heap" | "memory" => Ok(DynRepository::new_box(bitcache_heap::HeapRepository::new())),

        // Note: dispatch on the parsed scheme, but pass down the original
        // path (rather than the parsed URL's, which WHATWG normalization
        // would have made absolute), so that relative paths keep working:
        #[cfg(feature = "fs")]
        "file" => Ok(DynRepository::new_box(bitcache_fs::FsRepository::open(
            url.strip_prefix("file:").unwrap(),
        )?)),

        #[cfg(feature = "opendal")]
        scheme if scheme.starts_with("opendal+") => Ok(DynRepository::new_box(
            bitcache_opendal::DalRepository::open(url.strip_prefix("opendal+").unwrap())?,
        )),

        #[cfg(feature = "valkey")]
        "valkey" | "valkeys" | "redis" | "rediss" => Ok(DynRepository::new_box(
            bitcache_valkey::ValkeyRepository::open(url)?,
        )),

        _ => Err(OpenError::UnknownAdapter),
    }
}
