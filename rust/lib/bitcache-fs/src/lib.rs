// This is free and unencumbered software released into the public domain.

//! Bitcache is a distributed content-addressable storage (CAS) system.
//!
//! This crate provides a filesystem-backed repository ([`FsRepository`]).

#![no_std]
//#![allow(unused)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

//#[doc = include_str!("../README.md")]
//#[cfg(doctest)]
//pub struct ReadmeDoctests;

pub use cap_std::fs_utf8::{Dir, camino::Utf8Path};

mod adapter;
pub use adapter::*;

mod blob_encoding;
pub use blob_encoding::*;

mod blob_file;
pub use blob_file::*;

mod dir_cursor;
pub use dir_cursor::*;

#[cfg(feature = "std")]
mod file_metadata;

#[cfg(feature = "std")]
mod repository;
#[cfg(feature = "std")]
pub use repository::*;

mod util;
