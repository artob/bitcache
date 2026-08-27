// This is free and unencumbered software released into the public domain.

use crate::ID_LEN;
use thiserror::Error;

/// An error decoding an [`Id`](crate::Id) from a string representation.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("unsupported operation")]
    UnsupportedOperation,

    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "fred")]
    #[error(transparent)]
    Fred(#[from] fred::error::Error),

    #[cfg(feature = "opendal")]
    #[error(transparent)]
    Opendal(opendal::Error),

    #[cfg(feature = "alloc")]
    #[error(transparent)]
    Other(#[from] crate::BoxError),
}

#[cfg(feature = "opendal")]
impl From<opendal::Error> for RepositoryError {
    fn from(input: opendal::Error) -> Self {
        use RepositoryError::*;
        use opendal::ErrorKind;
        match input.kind() {
            ErrorKind::Unsupported => UnsupportedOperation,
            _ => Opendal(input),
        }
    }
}

#[cfg(feature = "clientele")]
impl From<RepositoryError> for clientele::SysexitsError {
    fn from(input: RepositoryError) -> Self {
        use clientele::SysexitsError::*;
        match input {
            #[cfg(feature = "std")]
            RepositoryError::Io(_) => EX_IOERR,
            #[cfg(feature = "fred")]
            RepositoryError::Fred(_) => EX_IOERR,
            #[cfg(feature = "opendal")]
            RepositoryError::Opendal(error) => EX_IOERR,
            _ => EX_SOFTWARE,
        }
    }
}
