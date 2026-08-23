// This is free and unencumbered software released into the public domain.

use crate::ID_LEN;
use thiserror::Error;

/// An error decoding an [`Id`](crate::Id) from a string representation.
#[derive(Clone, Debug, Error)]
pub enum IdError {
    /// The input wasn't a valid hexadecimal string.
    #[error("{0}")]
    DecodeHex(blake3::HexError),

    /// The input wasn't a valid Base58 string.
    #[cfg(feature = "base58")]
    #[error("{0}")]
    DecodeBase58(bs58::decode::Error),

    /// The input didn't decode to exactly [`ID_LEN`] bytes.
    #[error("expected {ID_LEN} bytes, but got {}{}", .0.unwrap_or(ID_LEN), if .0.is_none() { "+" } else { "" })]
    InvalidLength(Option<usize>),
}

impl From<blake3::HexError> for IdError {
    fn from(input: blake3::HexError) -> Self {
        Self::DecodeHex(input)
    }
}

#[cfg(feature = "base58")]
impl From<bs58::decode::Error> for IdError {
    fn from(input: bs58::decode::Error) -> Self {
        Self::DecodeBase58(input)
    }
}
