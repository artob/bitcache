// This is free and unencumbered software released into the public domain.

use crate::ID_LEN;
use thiserror::Error;

/// An error decoding an [`Id`](crate::Id) from a string representation.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(feature = "opendal")]
    #[error(transparent)]
    Opendal(#[from] opendal::Error),

    #[cfg(feature = "alloc")]
    #[error(transparent)]
    Other(#[from] crate::BoxError),
}

#[cfg(feature = "clientele")]
impl From<RepositoryError> for clientele::SysexitsError {
    fn from(input: RepositoryError) -> Self {
        use clientele::SysexitsError::*;
        match input {
            #[cfg(feature = "std")]
            RepositoryError::Io(_) => EX_IOERR,
            #[cfg(feature = "opendal")]
            RepositoryError::Opendal(_) => EX_IOERR,
            #[cfg(feature = "alloc")]
            RepositoryError::Other(_) => EX_SOFTWARE,
        }
    }
}
