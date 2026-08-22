// This is free and unencumbered software released into the public domain.

use crate::{Blob, Id};

pub trait Repository {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        0
    }

    fn contains(&self, id: &Id) -> bool {
        self.get(id).is_some()
    }

    fn get(&self, id: &Id) -> Option<Blob> {
        None
    }

    fn get_len(&self, id: &Id) -> Option<u64> {
        self.get(id).map(|blob| blob.len())
    }
}
