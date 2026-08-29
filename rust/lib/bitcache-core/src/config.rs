// This is free and unencumbered software released into the public domain.

//! Repository configuration (`.bitcache/config.toml`).
//!
//! The configuration file contains a top-level `[bitcache]` section,
//! followed by optional named sections for each CLI command (e.g.,
//! `[bitcache.put]`, `[bitcache.compact]`) whose directives correspond to
//! that command's command-line options, plus a `[bitcache.remote.NAME]`
//! section per named remote repository:
//!
//! ```toml
//! [bitcache]
//! version = 0
//! hashing = "blake3"
//! capacity = "100M"      # optional: expected number of blobs
//! encoding = "hex"       # optional: default ID display encoding
//!
//! [bitcache.put]
//! compress = "none"      # none | xz | xz:fast | xz:best
//!
//! [bitcache.compact]
//! compress = "xz"        # none | xz | xz:fast | xz:best
//!
//! [bitcache.remote.github]
//! url = "https://github.com/example/repo"
//! ```

use crate::{Compression, IdEncoding};
use std::{
    collections::BTreeMap,
    path::Path,
    string::{String, ToString},
};

/// The name of the repository configuration file.
pub const CONFIG_FILE_NAME: &str = "config.toml";

/// The current configuration format version: the major version of this crate.
pub const CONFIG_VERSION: u64 = parse_major(env!("CARGO_PKG_VERSION_MAJOR"));

pub const CONFIG_HEADER: &str = "# See: https://bitcache.dev/#configuration-file\n\n";

/// The default configuration file contents.
pub const DEFAULT_CONFIG_TOML: &str = "[bitcache]\nversion = 0\nhashing = \"blake3\"\n";

const fn parse_major(major: &str) -> u64 {
    let bytes = major.as_bytes();
    let mut value = 0u64;
    let mut index = 0;
    while index < bytes.len() {
        value = value * 10 + (bytes[index] - b'0') as u64;
        index += 1;
    }
    value
}

/// A repository configuration, as stored in `.bitcache/config.toml`.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The top-level `[bitcache]` section.
    pub bitcache: ConfigSection,
}

/// The top-level `[bitcache]` configuration section.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigSection {
    /// The configuration format version (the crate's major version).
    pub version: u64,

    /// The content-hashing algorithm; only BLAKE3 is supported.
    pub hashing: Hashing,

    /// A capacity hint for how many blobs will be stored (e.g., `"100M"`).
    ///
    /// This will eventually be used to derive the sharding structure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<Capacity>,

    /// The default encoding for displaying blob IDs (e.g., in `bitcache list`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<IdEncoding>,

    /// The `[bitcache.put]` section: defaults for `bitcache put`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put: Option<PutConfig>,

    /// The `[bitcache.compact]` section: defaults for `bitcache compact`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compact: Option<CompactConfig>,

    /// The `[bitcache.remote.NAME]` sections: named remote repositories.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub remote: BTreeMap<String, RemoteConfig>,
}

impl Default for ConfigSection {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            hashing: Hashing::Blake3,
            capacity: None,
            encoding: None,
            put: None,
            compact: None,
            remote: BTreeMap::new(),
        }
    }
}

/// The content-hashing algorithm used to derive blob IDs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Hashing {
    /// The BLAKE3 cryptographic hash function.
    #[default]
    #[serde(rename = "blake3")]
    Blake3,
}

impl Hashing {
    /// The canonical string form (`blake3`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blake3 => "blake3",
        }
    }
}

impl core::fmt::Display for Hashing {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl core::str::FromStr for Hashing {
    type Err = ConfigError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "blake3" => Ok(Self::Blake3),
            _ => Err(ConfigError::UnsupportedHashing(input.to_string())),
        }
    }
}

/// Defaults for the `bitcache put` command (`[bitcache.put]`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PutConfig {
    /// The compression scheme for storing blobs (default: `none`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress: Option<Compression>,
}

/// Defaults for the `bitcache compact` command (`[bitcache.compact]`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CompactConfig {
    /// The target compression scheme for stored blobs (default: `xz`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compress: Option<Compression>,
}

/// A named remote repository (`[bitcache.remote.NAME]`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RemoteConfig {
    /// The URL of the remote repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses a configuration from its TOML representation.
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(input).map_err(ConfigError::Parse)?;
        if config.bitcache.version > CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(config.bitcache.version));
        }
        Ok(config)
    }

    /// Loads the configuration from the given file path.
    ///
    /// Returns `Ok(None)` if the file doesn't exist.
    pub fn load(path: impl AsRef<Path>) -> Result<Option<Self>, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(input) => Self::parse(&input).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(ConfigError::Io(error)),
        }
    }

    /// Loads the configuration from the given file path, falling back to the
    /// default configuration if the file doesn't exist.
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        Ok(Self::load(path)?.unwrap_or_default())
    }

    /// Serializes the configuration to its TOML representation.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string(self).map_err(ConfigError::Serialize)
    }

    /// Returns the URL of the named remote repository, if configured.
    pub fn remote_url(&self, name: &str) -> Option<&str> {
        self.bitcache.remote.get(name)?.url.as_deref()
    }
}

/// A capacity hint: a count with an optional `K`, `M`, `B`, or `T` suffix
/// (e.g., `"100M"` for one hundred million).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Capacity(pub u64);

impl Capacity {
    /// The capacity as a plain count.
    pub fn get(&self) -> u64 {
        self.0
    }
}

impl core::str::FromStr for Capacity {
    type Err = ConfigError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        let (digits, multiplier) = match input.as_bytes().last() {
            Some(b'K' | b'k') => (&input[..input.len() - 1], 1_000),
            Some(b'M' | b'm') => (&input[..input.len() - 1], 1_000_000),
            Some(b'B' | b'b') => (&input[..input.len() - 1], 1_000_000_000),
            Some(b'T' | b't') => (&input[..input.len() - 1], 1_000_000_000_000),
            _ => (input, 1),
        };
        digits
            .parse::<u64>()
            .ok()
            .and_then(|count| count.checked_mul(multiplier))
            .map(Self)
            .ok_or_else(|| ConfigError::InvalidCapacity(input.to_string()))
    }
}

impl core::fmt::Display for Capacity {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            n if n >= 1_000_000_000_000 && n % 1_000_000_000_000 == 0 => {
                write!(formatter, "{}T", n / 1_000_000_000_000)
            },
            n if n >= 1_000_000_000 && n % 1_000_000_000 == 0 => {
                write!(formatter, "{}B", n / 1_000_000_000)
            },
            n if n >= 1_000_000 && n % 1_000_000 == 0 => write!(formatter, "{}M", n / 1_000_000),
            n if n >= 1_000 && n % 1_000 == 0 => write!(formatter, "{}K", n / 1_000),
            n => write!(formatter, "{}", n),
        }
    }
}

impl serde::Serialize for Capacity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Capacity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = Capacity;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("a count, optionally with a K, M, B, or T suffix")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Capacity, E> {
                value.parse().map_err(E::custom)
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Capacity, E> {
                u64::try_from(value)
                    .map(Capacity)
                    .map_err(|_| E::custom("capacity must be nonnegative"))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Capacity, E> {
                Ok(Capacity(value))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

/// An error loading, parsing, or serializing a repository configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// The configuration file couldn't be read.
    Io(std::io::Error),

    /// The configuration file isn't valid TOML or has invalid directives.
    Parse(toml::de::Error),

    /// The configuration couldn't be serialized to TOML.
    Serialize(toml::ser::Error),

    /// The configuration format version is newer than this build supports.
    UnsupportedVersion(u64),

    /// The named content-hashing algorithm isn't supported.
    UnsupportedHashing(String),

    /// A capacity hint couldn't be parsed.
    InvalidCapacity(String),
}

impl core::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read configuration: {}", error),
            Self::Parse(error) => write!(formatter, "invalid configuration: {}", error),
            Self::Serialize(error) => {
                write!(formatter, "failed to serialize configuration: {}", error)
            },
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported configuration version {} (this build supports up to {})",
                version, CONFIG_VERSION
            ),
            Self::UnsupportedHashing(input) => write!(
                formatter,
                "unsupported hashing algorithm {:?} (expected \"blake3\")",
                input
            ),
            Self::InvalidCapacity(input) => write!(formatter, "invalid capacity: {:?}", input),
        }
    }
}

impl core::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_round_trips() {
        let config = Config::parse(DEFAULT_CONFIG_TOML).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.bitcache.version, 0);
        assert_eq!(config.bitcache.hashing, Hashing::Blake3);
    }

    #[test]
    fn full_config_parses() {
        let config = Config::parse(
            r#"
            # See: https://bitcache.dev/#configuration-file

            [bitcache]
            version = 0
            hashing = "blake3"
            capacity = "100M"
            encoding = "hex"

            [bitcache.put]
            compress = "none"

            [bitcache.compact]
            compress = "xz"

            [bitcache.remote.github]
            url = "https://github.com/example/repo"
            "#,
        )
        .unwrap();
        assert_eq!(config.bitcache.capacity, Some(Capacity(100_000_000)));
        assert_eq!(config.bitcache.encoding, Some(IdEncoding::Hex));
        assert_eq!(
            config.remote_url("github"),
            Some("https://github.com/example/repo")
        );
        assert_eq!(
            config.bitcache.put.unwrap().compress,
            Some(Compression::None)
        );
        assert_eq!(
            config.bitcache.compact.unwrap().compress,
            Some(Compression::XzFast)
        );
    }

    #[test]
    fn unsupported_version_is_rejected() {
        assert!(matches!(
            Config::parse("[bitcache]\nversion = 999\nhashing = \"blake3\"\n"),
            Err(ConfigError::UnsupportedVersion(999))
        ));
    }

    #[test]
    fn capacity_suffixes() {
        for (input, expected) in [
            ("500", 500),
            ("5K", 5_000),
            ("100M", 100_000_000),
            ("2B", 2_000_000_000),
            ("1T", 1_000_000_000_000),
        ] {
            assert_eq!(input.parse::<Capacity>().unwrap().get(), expected);
        }
        assert!("".parse::<Capacity>().is_err());
        assert!("x".parse::<Capacity>().is_err());
    }
}
