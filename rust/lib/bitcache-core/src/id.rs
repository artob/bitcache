// This is free and unencumbered software released into the public domain.

use crate::IdError;
use arrayvec::ArrayString;
use blake3::Hash;

pub const ID_LEN: usize = 32;

/// A cryptographic hash identifier.
///
/// `Id` implements [`From`] and [`Into`] for `[u8; 32]`.
/// `Id` doesn't implement [`Deref`] or [`AsRef`], to preclude situations where
/// a type conversion might happen implicitly and the constant-time property
/// would be accidentally lost.
///
/// [`From`]: https://doc.rust-lang.org/std/convert/trait.From.html
/// [`Into`]: https://doc.rust-lang.org/std/convert/trait.Into.html
/// [`Deref`]: https://doc.rust-lang.org/stable/std/ops/trait.Deref.html
/// [`AsRef`]: https://doc.rust-lang.org/std/convert/trait.AsRef.html
#[derive(Clone, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Id(pub Hash);

impl core::fmt::Display for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::fmt::Debug for Id {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("Id").field(&self.to_hex().as_str()).finish()
    }
}

impl Id {
    /// Computes the ID of the given data.
    pub fn of(data: impl AsRef<[u8]>) -> Self {
        Self(blake3::hash(data.as_ref()))
    }

    pub fn from_bytes(input: [u8; ID_LEN]) -> Self {
        Self(Hash::from(input))
    }

    pub fn from_slice(input: &[u8]) -> Result<Self, core::array::TryFromSliceError> {
        Ok(Self::from_bytes(input.try_into()?))
    }

    pub fn from_hex(input: impl AsRef<[u8]>) -> Result<Self, IdError> {
        Hash::from_hex(input).map(Self).map_err(IdError::DecodeHex)
    }

    #[cfg(feature = "base58")]
    pub fn from_base58(input: impl AsRef<[u8]>) -> Result<Self, IdError> {
        use bs58::decode::Error::BufferTooSmall;
        let mut buffer = [0u8; ID_LEN];
        match bs58::decode(input.as_ref()).onto(buffer.as_mut_slice()) {
            Ok(ID_LEN) => Ok(Self::from_bytes(buffer)),
            Ok(len) => Err(IdError::InvalidLength(Some(len))),
            Err(BufferTooSmall) => Err(IdError::InvalidLength(None)),
            Err(error) => Err(IdError::DecodeBase58(error)),
        }
    }

    pub const fn as_bytes(&self) -> &[u8; ID_LEN] {
        self.0.as_bytes()
    }

    pub const fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn to_hex(&self) -> ArrayString<{ 2 * ID_LEN }> {
        self.0.to_hex()
    }

    #[cfg(feature = "base58")]
    pub fn to_base58(&self) -> ArrayString<{ 2 * ID_LEN }> {
        let mut bytes = [0u8; 2 * ID_LEN];
        let len = bs58::encode(self.0.as_bytes())
            .onto(bytes.as_mut_slice())
            .expect("buffer is large enough for any base58-encoded ID");
        let mut buffer = ArrayString::new();
        buffer.push_str(core::str::from_utf8(&bytes[..len]).expect("base58 output is ASCII"));
        buffer
    }
}

impl From<Hash> for Id {
    fn from(input: Hash) -> Self {
        Self(input)
    }
}

impl From<Id> for Hash {
    fn from(input: Id) -> Self {
        input.0
    }
}

impl From<[u8; ID_LEN]> for Id {
    fn from(input: [u8; ID_LEN]) -> Self {
        Self(Hash::from_bytes(input))
    }
}

impl From<Id> for [u8; ID_LEN] {
    fn from(input: Id) -> Self {
        input.0.into()
    }
}
