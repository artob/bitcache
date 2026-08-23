// This is free and unencumbered software released into the public domain.

//! Asynchronous (non-blocking) identification of streams and files.

use crate::{Hasher, Id};

const BUFFER_LEN: usize = 65_536;

/// Computes the [`Id`] of the given input stream by hashing it with BLAKE3.
///
/// The input is read to the end in a streaming fashion, so arbitrarily large
/// inputs can be identified without buffering them fully in memory.
///
/// # Errors
///
/// Returns any I/O error encountered while reading the input.
pub async fn identify_input(input: impl futures_io::AsyncRead) -> std::io::Result<Id> {
    use futures_util::AsyncReadExt;
    futures_util::pin_mut!(input);
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; BUFFER_LEN];
    loop {
        match input.read(&mut buffer).await? {
            0 => break,
            n => hasher.update(&buffer[..n]),
        };
    }
    Ok(Id(hasher.finalize()))
}

/// Computes the [`Id`] of the file at the given path by hashing its contents
/// with BLAKE3.
///
/// The file is read in a streaming fashion, so arbitrarily large files can be
/// identified without buffering them fully in memory.
///
/// # Errors
///
/// Returns any I/O error encountered while opening or reading the file.
#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub async fn identify_file(input_path: impl AsRef<std::path::Path>) -> std::io::Result<Id> {
    use tokio::io::AsyncReadExt;
    let mut input = tokio::fs::File::open(input_path).await?;
    let mut hasher = Hasher::new();
    let mut buffer = [0u8; BUFFER_LEN];
    loop {
        match input.read(&mut buffer).await? {
            0 => break,
            n => hasher.update(&buffer[..n]),
        };
    }
    Ok(Id(hasher.finalize()))
}
