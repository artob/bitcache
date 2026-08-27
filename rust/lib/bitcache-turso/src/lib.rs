// This is free and unencumbered software released into the public domain.

//! Bitcache is a distributed content-addressable storage (CAS) system.
//!
//! This crate provides a repository backed by [Turso](https://turso.tech),
//! a SQLite-compatible database implementation in Rust.

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

mod adapter;
pub use adapter::*;

#[cfg(feature = "std")]
mod repository;
#[cfg(feature = "std")]
pub use repository::*;
