// This is free and unencumbered software released into the public domain.

use bitcache_core::{Blob, Id, Repository};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, camino::Utf8Path},
};
use std::io::Result;

#[derive(Debug)]
pub struct FsRepository(Dir);

impl FsRepository {
    pub fn open(path: impl AsRef<Utf8Path>) -> Result<Self> {
        Ok(Self(Dir::open_ambient_dir(
            path.as_ref(),
            ambient_authority(),
        )?))
    }
}

impl Repository for FsRepository {
    fn len(&self) -> usize {
        0 // TODO
    }

    fn contains(&self, id: &Id) -> bool {
        false // TODO
    }

    fn get(&self, id: &Id) -> Option<Blob> {
        None // TODO
    }
}
