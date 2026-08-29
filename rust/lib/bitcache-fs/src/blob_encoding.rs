// This is free and unencumbered software released into the public domain.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BlobEncoding {
    Uncompressed,
    Xz,
}
