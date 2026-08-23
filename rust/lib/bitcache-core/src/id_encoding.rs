// This is free and unencumbered software released into the public domain.

/// A string encoding for [`Id`](crate::Id) values.
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum IdEncoding {
    /// Hexadecimal (aka Base16)
    #[default]
    Hex,

    /// Base58
    #[cfg(feature = "base58")]
    Base58,
}
