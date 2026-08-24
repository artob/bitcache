// This is free and unencumbered software released into the public domain.

use crate::{BoxError, ID_LEN};
use thiserror::Error;

/// An error decoding an [`Id`](crate::Id) from a string representation.
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[cfg(feature = "std")]
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] BoxError),
}

#[cfg(feature = "clientele")]
impl From<RepositoryError> for clientele::SysexitsError {
    fn from(input: RepositoryError) -> Self {
        use clientele::SysexitsError::*;
        match input {
            RepositoryError::Io(_) => EX_IOERR,
            RepositoryError::Other(err) => EX_SOFTWARE,
        }
    }
}
