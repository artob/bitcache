// This is free and unencumbered software released into the public domain.

//! Bitcache is a distributed content-addressable storage (CAS) system.
//!
//! This crate will provide a repository backed by [Iroh](https://iroh.computer).

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
