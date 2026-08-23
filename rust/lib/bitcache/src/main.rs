// This is free and unencumbered software released into the public domain.

use bitcache::IdEncoding;
use bitcache_core::Id;
use bitcache_fs::FsRepository;
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
    /// Compute the BLAKE3 hash of the given file(s).
    #[clap(aliases = ["id", "hash"])]
    Identify {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT", default_value = "hex")]
        format: IdEncoding,

        /// The paths to the file(s) to hash.
        #[arg(value_name = "FILES")]
        paths: Vec<PathBuf>,
    },

    /// Fetch blob(s) from the repository, writing their contents to stdout.
    #[clap(alias = "cat")]
    Get {
        /// The IDs of the blob(s) to fetch.
        #[arg(value_name = "IDS")]
        ids: Vec<Id>,
    },

    /// Initialize a new repository in `./.bitcache/`.
    Init {},

    /// Store the given file(s) into the repository.
    Put {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT", default_value = "hex")]
        format: IdEncoding,

        /// The paths to the file(s) to store.
        #[arg(value_name = "FILES")]
        paths: Vec<PathBuf>,
    },
}

/// The entry point for the `bitcache` command-line interface.
#[tokio::main]
pub async fn main() -> Result<(), SysexitsError> {
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
        print!("{}", include_str!("../../../UNLICENSE"));
        return Ok(());
    }

    // Configure debug output:
    if options.flags.debug {}

    match options.command.unwrap() {
        Command::Identify { format, paths } => {
            for path in paths {
                let id = bitcache_core::sync::identify_file(&path)?;
                match format {
                    IdEncoding::Hex => println!("{}", id.to_hex()),
                    #[cfg(feature = "base58")]
                    IdEncoding::Base58 => println!("{}", id.to_base58()),
                }
            }
            Ok(())
        },

        Command::Get { ids } => {
            let repository = FsRepository::open(".bitcache")?;
            let mut stdout = tokio::io::stdout();
            for id in ids {
                let Some(mut file) = repository.get_file(&id).await? else {
                    eprintln!("bitcache: blob not found: {}", id.to_hex());
                    return Err(SysexitsError::from(std::io::Error::from(
                        std::io::ErrorKind::NotFound,
                    )));
                };
                tokio::io::copy(&mut file, &mut stdout).await?;
            }
            use tokio::io::AsyncWriteExt;
            stdout.flush().await?;
            Ok(())
        },

        Command::Init {} => {
            let _repository = FsRepository::create(".bitcache")?;
            Ok(())
        },

        Command::Put { format, paths } => {
            let mut repository = FsRepository::open(".bitcache")?;
            for path in paths {
                let id = repository.put_file(&path).await?;
                match format {
                    IdEncoding::Hex => println!("{}", id.to_hex()),
                    #[cfg(feature = "base58")]
                    IdEncoding::Base58 => println!("{}", id.to_base58()),
                }
            }
            Ok(())
        },
    }
}
