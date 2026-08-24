// This is free and unencumbered software released into the public domain.

use bitcache_core::RepositoryError;
use thiserror::Error;

/// An error when opening a repository.
#[derive(Debug, Error)]
pub enum OpenError {
    #[error("no adapter available for URL scheme")]
    NoAdapter,

    #[error("invalid URL")]
    InvalidUrl,

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[cfg(feature = "alloc")]
    #[error(transparent)]
    Other(#[from] crate::BoxError),
}

#[cfg(feature = "clientele")]
impl From<OpenError> for clientele::SysexitsError {
    fn from(input: OpenError) -> Self {
        use clientele::SysexitsError::*;
        match input {
            OpenError::NoAdapter => EX_UNAVAILABLE,
            OpenError::InvalidUrl => EX_DATAERR,
            OpenError::Repository(error) => error.into(),
            #[cfg(feature = "alloc")]
            OpenError::Other(_) => EX_SOFTWARE,
        }
    }
}
