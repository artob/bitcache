// This is free and unencumbered software released into the public domain.

use crate::{Bytes, Id, Repository};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FsRepository(PathBuf);

impl FsRepository {
    pub fn open(path: impl AsRef<Path>) -> Self {
        Self(PathBuf::from(path.as_ref()))
    }
}

impl Repository for FsRepository {
    fn len(&self) -> usize {
        0 // TODO
    }

    fn contains(&self, id: &Id) -> bool {
        false // TODO
    }

    fn get(&self, id: &Id) -> Option<Bytes> {
        None // TODO
    }
}
