// This is free and unencumbered software released into the public domain.

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = distrib_build::ruby();
    Ok(())
}
