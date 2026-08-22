# This is free and unencumbered software released into the public domain.

import importlib.metadata

try:
    __version__ = importlib.metadata.version("bitcache")
except importlib.metadata.PackageNotFoundError:
    # This acts as a fallback if the code is being run directly
    # without having been installed first (e.g., local development):
    __version__ = "unknown"
