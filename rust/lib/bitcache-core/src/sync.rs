// This is free and unencumbered software released into the public domain.

//! Synchronous (blocking) identification of streams and files.

use crate::{Hasher, Id};

/// Computes the [`Id`] of the given input stream by hashing it with BLAKE3.
///
/// The input is read to the end in a streaming fashion, so arbitrarily large
/// inputs can be identified without buffering them fully in memory.
///
/// # Errors
///
/// Returns any I/O error encountered while reading the input.
pub fn identify_input(input: impl std::io::Read) -> std::io::Result<Id> {
    let mut hasher = Hasher::new();
    hasher.update_reader(input)?;
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
pub fn identify_file(input_path: impl AsRef<std::path::Path>) -> std::io::Result<Id> {
    identify_input(std::fs::File::open(input_path)?)
}
