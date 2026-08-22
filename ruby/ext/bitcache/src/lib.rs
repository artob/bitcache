// This is free and unencumbered software released into the public domain.

use distrib::ruby::magnus;
use magnus::{Error, Ruby, function};

#[distrib::export]
pub fn square(n: i64) -> i64 {
    n * n
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    ruby.define_global_function("square", function!(square, 1));
    Ok(())
}
