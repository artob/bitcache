// This is free and unencumbered software released into the public domain.

//! Bitcache is a distributed content-addressable storage (CAS) system.
//!
//! This crate provides the core types and traits: [`Id`], [`Blob`], and the
//! asynchronous [`Repository`] trait implemented by storage backends.

#![no_std]
#![allow(unused)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

//#[doc = include_str!("../README.md")]
//#[cfg(doctest)]
//pub struct ReadmeDoctests;

#[cfg(feature = "alloc")]
pub type BoxError = alloc::boxed::Box<dyn core::error::Error + Send + Sync>;

#[doc(hidden)]
pub use blake3::Hasher;

pub use bytes::Bytes;

// Re-exported for downstream crates (e.g. repository implementations), so
// that they needn't depend on the `futures-*` and `tokio` crates directly:
pub use futures_core;
pub use futures_core::Stream;
pub use futures_io;
pub use futures_util;
#[cfg(feature = "tokio")]
pub use tokio;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
#[path = "async.rs"]
pub mod r#async;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub mod sync;

mod adapter_registry;
pub use adapter_registry::*;

mod blob;
pub use blob::*;

mod blob_reader;
pub use blob_reader::*;

mod blob_metadata;
pub use blob_metadata::*;

mod id;
pub use id::*;

mod id_encoding;
pub use id_encoding::*;

mod id_error;
pub use id_error::*;

mod list_options;
pub use list_options::*;

mod open_error;
pub use open_error::*;

mod repository;
pub use repository::*;

mod repository_error;
pub use repository_error::*;

/// Computes the [`Id`] of the given in-memory input by hashing it with BLAKE3.
///
/// This is the most convenient option when the input already resides in
/// memory. For streaming inputs, see [`sync::identify_input`] and
/// [`r#async::identify_input`]; for files, see [`sync::identify_file`] and
/// [`r#async::identify_file`].
pub fn identify(input: impl AsRef<[u8]>) -> Id {
    Id(blake3::hash(input.as_ref()))
}
