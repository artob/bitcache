// This is free and unencumbered software released into the public domain.

//! Bitcache is a distributed content-addressable storage (CAS) system.
//!
//! This is the umbrella crate: it re-exports the core types and traits from
//! [`bitcache_core`], and hosts the `bitcache` command-line interface.

#![no_std]
#![allow(unused)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[doc = include_str!("../../../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

mod adapters;
pub use adapters::*;

pub use bitcache_core::*;

#[cfg(feature = "fs")]
pub use bitcache_fs as fs;

#[cfg(feature = "heap")]
pub use bitcache_heap as heap;

#[cfg(feature = "iroh")]
pub use bitcache_iroh as iroh;

#[cfg(feature = "opendal")]
pub use bitcache_opendal as opendal;
