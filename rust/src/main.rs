// This is free and unencumbered software released into the public domain.

use clientele::{
    StandardOptions, SysexitsError,
    crates::clap::{Parser, Subcommand},
};
use std::path::PathBuf;

/// Bitcache is a distributed content-addressable storage (CAS) system.
#[derive(Debug, Parser)]
#[command(name = "Bitcache", long_about)]
#[command(arg_required_else_help = true)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize a new repository.
    Init {},

    /// Compute the hash of a file.
    #[clap(aliases = ["id", "hash"])]
    Identify {
        /// The path to the file to hash.
        #[arg(value_name = "FILES")]
        paths: Vec<PathBuf>,
    },
}

pub fn main() -> Result<(), SysexitsError> {
    // Load environment variables from `.env`:
    clientele::dotenv().ok();

    // Expand wildcards and @argfiles:
    let args = clientele::args_os()?;

    // Parse command-line options:
    let options = Options::parse_from(args);

    // Print the program version, if requested:
    if options.flags.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Print the program license, if requested:
    if options.flags.license {
        print!("{}", include_str!("../UNLICENSE"));
        return Ok(());
    }

    // Configure debug output:
    if options.flags.debug {}

    match options.command.unwrap() {
        Command::Init {} => Ok(()),
        Command::Identify { paths } => {
            for path in paths {
                println!("{}", path.display());
            }
            Ok(())
        },
    }
}
