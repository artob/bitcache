// This is free and unencumbered software released into the public domain.

/// A string encoding for [`Id`](crate::Id) values.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum IdEncoding {
    /// Hexadecimal (aka Base16)
    #[default]
    Hex,

    /// Base58
    #[cfg(feature = "base58")]
    Base58,
}
