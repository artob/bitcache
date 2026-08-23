// This is free and unencumbered software released into the public domain.

//! Bitcache is a distributed content-addressable storage (CAS) system.

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

#[doc(hidden)]
pub use blake3::Hasher;

pub use bytes::Bytes;

mod blob;
pub use blob::*;

mod id;
pub use id::*;

mod id_encoding;
pub use id_encoding::*;

mod id_error;
pub use id_error::*;

mod repository;
pub use repository::*;
