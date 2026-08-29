// This is free and unencumbered software released into the public domain.

/// A compression scheme for physically storing blobs.
///
/// This is a storage-layer concern only: blob IDs are always derived from
/// the uncompressed contents, and repositories always return the original
/// bytes regardless of how they are stored on disk.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Compression {
    /// Store blobs uncompressed.
    #[default]
    #[cfg_attr(feature = "serde", serde(rename = "none"))]
    None,

    /// XZ (LZMA2) compression optimized for speed.
    ///
    /// `xz` is accepted as an alias for `xz:fast`.
    #[cfg_attr(feature = "serde", serde(rename = "xz:fast", alias = "xz"))]
    XzFast,

    /// XZ (LZMA2) compression optimized for size.
    #[cfg_attr(feature = "serde", serde(rename = "xz:best"))]
    XzBest,
}

impl Compression {
    /// The canonical string form (`none`, `xz:fast`, or `xz:best`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::XzFast => "xz:fast",
            Self::XzBest => "xz:best",
        }
    }
}

impl core::fmt::Display for Compression {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl core::str::FromStr for Compression {
    type Err = InvalidCompression;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "none" => Ok(Self::None),
            "xz" | "xz:fast" => Ok(Self::XzFast),
            "xz:best" => Ok(Self::XzBest),
            _ => Err(InvalidCompression),
        }
    }
}

/// The error returned when parsing an invalid [`Compression`] name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCompression;

impl core::fmt::Display for InvalidCompression {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("expected one of: none, xz, xz:fast, xz:best")
    }
}

impl core::error::Error for InvalidCompression {}
