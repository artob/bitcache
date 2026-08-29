// This is free and unencumbered software released into the public domain.

use cap_std::fs_utf8::File;
use std::io::Result;

const XZ_MAGIC: &[u8; 6] = b"\xfd7zXZ\0";

/// Reads the LZMA2 dictionary size from the first XZ block header.
pub fn read_xz_dict_size(file: &mut File) -> Result<Option<u64>> {
    use std::io::{Error, ErrorKind, Read};

    let mut stream_header = [0u8; 12];
    file.read_exact(&mut stream_header)?;
    if &stream_header[..XZ_MAGIC.len()] != XZ_MAGIC {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "invalid XZ stream header",
        ));
    }

    let mut header_size = [0u8; 1];
    file.read_exact(&mut header_size)?;
    if header_size[0] == 0 {
        return Ok(None);
    }
    let header_len = (usize::from(header_size[0]) + 1) * 4;
    let mut header = std::vec![0u8; header_len];
    header[0] = header_size[0];
    file.read_exact(&mut header[1..])?;

    let end = header_len
        .checked_sub(4)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "invalid XZ block header size"))?;
    let flags = *header
        .get(1)
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidData, "truncated XZ block header"))?;
    if flags & 0x3c != 0 {
        return Err(Error::new(ErrorKind::InvalidData, "invalid XZ block flags"));
    }

    let mut cursor = 2;
    if flags & 0x40 != 0 {
        read_xz_vli(&header, &mut cursor, end)?;
    }
    if flags & 0x80 != 0 {
        read_xz_vli(&header, &mut cursor, end)?;
    }

    let mut dictionary_size = None;
    for _ in 0..=flags & 0x03 {
        let filter_id = read_xz_vli(&header, &mut cursor, end)?;
        let properties_len =
            usize::try_from(read_xz_vli(&header, &mut cursor, end)?).map_err(|_| {
                Error::new(ErrorKind::InvalidData, "XZ filter properties are too large")
            })?;
        let properties_end = cursor
            .checked_add(properties_len)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "XZ filter properties overflow"))?;
        if properties_end > end {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "truncated XZ filter properties",
            ));
        }
        if filter_id == 0x21 {
            if properties_len != 1 || header[cursor] > 40 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "invalid LZMA2 dictionary property",
                ));
            }
            let property = header[cursor];
            dictionary_size = Some(if property == 40 {
                u64::from(u32::MAX)
            } else {
                u64::from(2 | (property & 1)) << (u32::from(property / 2) + 11)
            });
        }
        cursor = properties_end;
    }
    if header[cursor..end].iter().any(|byte| *byte != 0) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "invalid XZ block header padding",
        ));
    }
    Ok(dictionary_size)
}

fn read_xz_vli(data: &[u8], cursor: &mut usize, end: usize) -> Result<u64> {
    use std::io::{Error, ErrorKind};

    let mut value = 0u64;
    for index in 0..9 {
        let byte = *data
            .get(*cursor)
            .filter(|_| *cursor < end)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "truncated XZ block header"))?;
        *cursor += 1;
        if index == 8 && byte > 1 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "XZ variable-length integer overflow",
            ));
        }
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            if index > 0 && byte == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "non-canonical XZ variable-length integer",
                ));
            }
            return Ok(value);
        }
    }
    Err(Error::new(
        ErrorKind::InvalidData,
        "unterminated XZ variable-length integer",
    ))
}
