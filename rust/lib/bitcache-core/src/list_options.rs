// This is free and unencumbered software released into the public domain.

use crate::{ID_LEN, Id};
use arrayvec::ArrayString;

/// Options for enumerating the blob IDs in a repository.
///
/// See [`Repository::list`](crate::Repository::list).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListOptions {
    /// Enumerate only IDs whose hexadecimal encoding begins with this prefix.
    pub prefix: Option<ArrayString<{ 2 * ID_LEN }>>,

    /// Enumerate only IDs ordered strictly after this one.
    ///
    /// This is an exclusive cursor: passing the last ID of one page as the
    /// cursor for the next yields a stable paginated view.
    pub after: Option<Id>,

    /// Enumerate at most this many IDs.
    ///
    /// Together with [`ListOptions::after`], this bounds one page of a
    /// paginated view.
    pub limit: Option<usize>,
}

impl ListOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts enumeration to IDs whose hexadecimal encoding begins with
    /// the given prefix.
    ///
    /// # Panics
    ///
    /// Panics if the prefix is longer than a full hexadecimal ID (64 bytes).
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = Some(ArrayString::from(prefix).expect("prefix no longer than a full ID"));
        self
    }

    /// Restricts enumeration to IDs ordered strictly after the given ID.
    pub fn with_after(mut self, id: Id) -> Self {
        self.after = Some(id);
        self
    }

    /// Restricts enumeration to at most the given number of IDs.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Returns `true` if the given ID satisfies these options.
    ///
    /// Note that this checks the prefix filter and cursor only; enforcing
    /// [`ListOptions::limit`] is up to the enumerator.
    pub fn matches(&self, id: &Id) -> bool {
        if let Some(prefix) = &self.prefix {
            if !id.to_hex().starts_with(prefix.as_str()) {
                return false;
            }
        }
        if let Some(after) = &self.after {
            if id <= after {
                return false;
            }
        }
        true
    }
}
