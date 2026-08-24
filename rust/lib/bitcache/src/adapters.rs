// This is free and unencumbered software released into the public domain.

use bitcache_core::AdapterRegistry;

pub fn register_default_adapters() {
    let registry: AdapterRegistry = ();

    #[cfg(feature = "fs")]
    bitcache_fs::register_adapter(&registry);

    #[cfg(feature = "heap")]
    bitcache_heap::register_adapter(&registry);

    #[cfg(feature = "iroh")]
    bitcache_iroh::register_adapter(&registry);

    #[cfg(feature = "opendal")]
    bitcache_opendal::register_adapter(&registry);
}
