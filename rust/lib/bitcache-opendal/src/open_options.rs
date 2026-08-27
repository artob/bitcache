// This is free and unencumbered software released into the public domain.

use alloc::{boxed::Box, string::String, vec::Vec};
use opendal::{Operator, raw::Layer};

/// Options for opening a [`DalRepository`](crate::DalRepository): service
/// configuration and OpenDAL layers.
///
/// # Examples
///
/// ```no_run
/// use bitcache_core::OpenError;
/// use bitcache_opendal::{DalRepository, OpenOptions};
///
/// # fn main() -> Result<(), OpenError> {
/// let repository = DalRepository::open_options(
///     "s3://bucket/prefix",
///     OpenOptions::new().with_option("region", "us-east-1"),
/// )?;
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct OpenOptions {
    pub(crate) options: Vec<(String, String)>,
    pub(crate) layers: Vec<Box<dyn FnOnce(Operator) -> Operator + Send + Sync>>,
}

impl OpenOptions {
    /// Creates an empty set of options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a service configuration option (e.g. `region` for S3),
    /// overriding any equally-named option in the URL's query string.
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.push((key.into(), value.into()));
        self
    }

    /// Applies an OpenDAL [`Layer`] (e.g. retry, logging, or timeout) to the
    /// underlying operator. Layers are applied in the order they were added.
    pub fn with_layer(mut self, layer: impl Layer) -> Self {
        self.layers.push(Box::new(move |op| op.layer(layer)));
        self
    }
}

impl core::fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OpenOptions")
            .field("options", &self.options)
            .field("layers", &self.layers.len())
            .finish()
    }
}
