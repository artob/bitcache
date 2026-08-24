// This is free and unencumbered software released into the public domain.

use super::RepositoryError;
use thiserror::Error;

/// An error when opening a repository.
#[derive(Debug, Error)]
pub enum OpenError {
    #[error("no adapter available for repository URL")]
    UnknownAdapter,

    #[error("invalid bytes in repository URL")]
    InvalidUrl,

    #[error(transparent)]
    Repository(#[from] RepositoryError),

    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "opendal")]
    #[error(transparent)]
    Opendal(opendal::Error),

    #[cfg(feature = "alloc")]
    #[error(transparent)]
    Other(#[from] crate::BoxError),
}

#[cfg(feature = "opendal")]
impl From<opendal::Error> for OpenError {
    fn from(input: opendal::Error) -> Self {
        use OpenError::*;
        use opendal::ErrorKind;
        match input.kind() {
            ErrorKind::Unsupported => UnknownAdapter,
            _ => Opendal(input),
        }
    }
}

#[cfg(feature = "clientele")]
impl From<OpenError> for clientele::SysexitsError {
    fn from(input: OpenError) -> Self {
        use OpenError::*;
        use clientele::SysexitsError::*;
        match input {
            UnknownAdapter => EX_UNAVAILABLE,
            InvalidUrl => EX_DATAERR,
            Repository(error) => error.into(),
            _ => EX_SOFTWARE,
        }
    }
}
