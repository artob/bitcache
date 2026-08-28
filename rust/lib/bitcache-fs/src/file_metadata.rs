// This is free and unencumbered software released into the public domain.

use cap_std::fs_utf8::File;
use std::{io, path::Path, string::String, vec::Vec};

#[cfg(unix)]
const CREATED_NAME: &str = "user.bitcache.created";
#[cfg(unix)]
const UPDATED_NAME: &str = "user.bitcache.updated";
#[cfg(unix)]
const EXPIRES_NAME: &str = "user.bitcache.expires";
#[cfg(unix)]
const MEDIA_TYPE_NAME: &str = "user.bitcache.media-type";

#[cfg(windows)]
const CREATED_NAME: &str = "bitcache.created";
#[cfg(windows)]
const UPDATED_NAME: &str = "bitcache.updated";
#[cfg(windows)]
const EXPIRES_NAME: &str = "bitcache.expires";
#[cfg(windows)]
const MEDIA_TYPE_NAME: &str = "bitcache.media-type";

#[cfg(not(any(unix, windows)))]
const CREATED_NAME: &str = "bitcache.created";
#[cfg(not(any(unix, windows)))]
const UPDATED_NAME: &str = "bitcache.updated";
#[cfg(not(any(unix, windows)))]
const EXPIRES_NAME: &str = "bitcache.expires";
#[cfg(not(any(unix, windows)))]
const MEDIA_TYPE_NAME: &str = "bitcache.media-type";

/// Bitcache metadata stored outside the blob's contents.
#[derive(Clone, Debug, Default)]
pub struct ExtendedMetadata {
    /// Creation-time override used when maintenance replaces the backing inode.
    pub created: Option<u64>,
    /// Update-time override used when maintenance replaces the backing inode.
    pub updated: Option<u64>,
    pub expires: Option<u64>,
    pub media_type: Option<String>,
}

/// Reads all supported extended metadata from a blob.
pub fn read(file: &File, path: Option<&Path>) -> io::Result<ExtendedMetadata> {
    Ok(ExtendedMetadata {
        created: decode_timestamp(read_attribute(file, path, CREATED_NAME)?, CREATED_NAME)?,
        updated: decode_timestamp(read_attribute(file, path, UPDATED_NAME)?, UPDATED_NAME)?,
        expires: decode_timestamp(read_attribute(file, path, EXPIRES_NAME)?, EXPIRES_NAME)?,
        media_type: decode_media_type(read_attribute(file, path, MEDIA_TYPE_NAME)?)?,
    })
}

/// Writes all extended metadata, returning whether the backing filesystem
/// supports it. Unset optional fields have their attributes removed.
pub fn write(file: &File, path: Option<&Path>, metadata: &ExtendedMetadata) -> io::Result<bool> {
    for (name, value) in [
        (CREATED_NAME, metadata.created),
        (UPDATED_NAME, metadata.updated),
        (EXPIRES_NAME, metadata.expires),
    ] {
        let value = value.map(u64::to_be_bytes);
        if !write_optional_attribute(
            file,
            path,
            name,
            value.as_ref().map(|bytes| bytes.as_slice()),
        )? {
            return Ok(false);
        }
    }
    write_optional_attribute(
        file,
        path,
        MEDIA_TYPE_NAME,
        metadata.media_type.as_deref().map(str::as_bytes),
    )
}

fn decode_timestamp(value: Option<Vec<u8>>, name: &str) -> io::Result<Option<u64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    let bytes: [u8; 8] = value.try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            std::format!("invalid {name} extended attribute"),
        )
    })?;
    Ok(Some(u64::from_be_bytes(bytes)))
}

fn decode_media_type(value: Option<Vec<u8>>) -> io::Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() {
        return Ok(None);
    }
    String::from_utf8(value).map(Some).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid UTF-8 in media-type extended attribute",
        )
    })
}

fn write_optional_attribute(
    file: &File,
    path: Option<&Path>,
    name: &str,
    value: Option<&[u8]>,
) -> io::Result<bool> {
    match value {
        Some(value) => write_attribute(file, path, name, value),
        None => remove_attribute(file, path, name),
    }
}

#[cfg(unix)]
fn read_attribute(file: &File, _path: Option<&Path>, name: &str) -> io::Result<Option<Vec<u8>>> {
    use xattr::FileExt;

    match file.try_clone()?.into_std().get_xattr(name) {
        Ok(value) => Ok(value),
        Err(error) if is_unsupported(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn read_attribute(_file: &File, path: Option<&Path>, name: &str) -> io::Result<Option<Vec<u8>>> {
    let path = required_path(path)?;
    match fsquirrel::get(path, name) {
        Ok(value) => Ok(value),
        Err(error) if is_unsupported(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn write_attribute(
    file: &File,
    _path: Option<&Path>,
    name: &str,
    value: &[u8],
) -> io::Result<bool> {
    use xattr::FileExt;

    match file.try_clone()?.into_std().set_xattr(name, value) {
        Ok(()) => Ok(true),
        Err(error) if is_unsupported(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn write_attribute(
    _file: &File,
    path: Option<&Path>,
    name: &str,
    value: &[u8],
) -> io::Result<bool> {
    match fsquirrel::set(required_path(path)?, name, value) {
        Ok(()) => Ok(true),
        Err(error) if is_unsupported(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn remove_attribute(file: &File, _path: Option<&Path>, name: &str) -> io::Result<bool> {
    use xattr::FileExt;

    match file.try_clone()?.into_std().remove_xattr(name) {
        Ok(()) => Ok(true),
        Err(error) if is_missing_attribute(&error) => Ok(true),
        Err(error) if is_unsupported(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
fn remove_attribute(_file: &File, path: Option<&Path>, name: &str) -> io::Result<bool> {
    match fsquirrel::remove(required_path(path)?, name) {
        Ok(()) => Ok(true),
        Err(error) if is_missing_attribute(&error) => Ok(true),
        Err(error) if is_unsupported(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(not(any(unix, windows)))]
fn read_attribute(_file: &File, _path: Option<&Path>, _name: &str) -> io::Result<Option<Vec<u8>>> {
    Ok(None)
}

#[cfg(not(any(unix, windows)))]
fn write_attribute(
    _file: &File,
    _path: Option<&Path>,
    _name: &str,
    _value: &[u8],
) -> io::Result<bool> {
    Ok(false)
}

#[cfg(not(any(unix, windows)))]
fn remove_attribute(_file: &File, _path: Option<&Path>, _name: &str) -> io::Result<bool> {
    Ok(false)
}

#[cfg(windows)]
fn required_path(path: Option<&Path>) -> io::Result<&Path> {
    path.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "blob path is unavailable"))
}

fn is_missing_attribute(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound || matches!(error.raw_os_error(), Some(61 | 87 | 93))
}

fn is_unsupported(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Unsupported
}
