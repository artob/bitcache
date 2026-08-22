// This is free and unencumbered software released into the public domain.

use crate::{Bytes, Id, Repository};
use hashbrown::HashMap;

#[derive(Clone, Debug, Default)]
pub struct HeapRepository(HashMap<Id, Bytes>);

impl HeapRepository {
    pub fn new() -> Self {
        Self(HashMap::default())
    }
}

impl Repository for HeapRepository {
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
