// This is free and unencumbered software released into the public domain.

use bitcache_core::{Blob, Id, Repository};
use hashbrown::HashMap;

#[derive(Clone, Debug, Default)]
pub struct HeapRepository(HashMap<Id, Blob>);

impl HeapRepository {
    pub fn new() -> Self {
        Self(HashMap::default())
    }
}

impl Repository for HeapRepository {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn contains(&self, id: &Id) -> bool {
        self.0.contains_key(id)
    }

    fn get(&self, id: &Id) -> Option<Blob> {
        self.0.get(id).cloned()
    }
}
