// This is free and unencumbered software released into the public domain.

use bitcache::IdEncoding;
use bitcache_core::{Id, ListOptions, Repository};
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
    #[clap(aliases = ["identify", "hash"])]
    Id {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT", default_value = "hex")]
        format: IdEncoding,

        /// The paths to the file(s) to hash.
        #[arg(value_name = "FILES")]
        paths: Vec<PathBuf>,
    },

    /// Initialize a new repository in `./.bitcache/`.
    Init {},

    /// List the IDs of the blobs in the repository, in ascending order.
    #[clap(alias = "ls")]
    List {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT", default_value = "hex")]
        format: IdEncoding,

        /// List only IDs whose hexadecimal encoding begins with this prefix.
        #[arg(short, long, value_name = "PREFIX")]
        prefix: Option<String>,

        /// List only IDs ordered strictly after this one.
        #[arg(short = 'a', long, value_name = "ID")]
        after: Option<Id>,

        /// List at most this many IDs.
        #[arg(short = 'n', long, value_name = "COUNT")]
        limit: Option<usize>,
    },

    /// Check whether the repository contains blob(s) with the given ID(s).
    ///
    /// With `--verbose`, prints `true` or `false` for each ID.
    ///
    /// Exits with a nonzero status unless all of the given blobs were found
    /// in the repository.
    #[clap(aliases = ["knows", "exists", "contains"])]
    Has {
        /// The IDs of the blob(s) to check for.
        #[arg(value_name = "IDS")]
        ids: Vec<Id>,
    },

    /// Fetch blob(s) from the repository, writing their contents to stdout.
    ///
    /// Exits with a nonzero status unless all of the given blobs were found
    /// in the repository.
    #[clap(alias = "cat")]
    Get {
        /// The IDs of the blob(s) to fetch.
        #[arg(value_name = "IDS")]
        ids: Vec<Id>,
    },

    /// Store the given file(s) into the repository as blob(s).
    Put {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT", default_value = "hex")]
        format: IdEncoding,

        /// The paths to the file(s) to store.
        #[arg(value_name = "FILES")]
        paths: Vec<PathBuf>,
    },

    /// Remove blob(s) with the given ID(s) from the repository.
    ///
    /// Exits with a nonzero status unless all of the given blobs were found
    /// in the repository.
    #[clap(aliases = ["remove", "delete", "del"])]
    Rm {
        /// The IDs of the blob(s) to remove.
        #[arg(value_name = "IDS")]
        ids: Vec<Id>,
    },

    /// Remove all blobs from the repository.
    #[clap(aliases = ["reset"])]
    Clear {
        /// Actually perform the operation; without this, nothing is removed.
        #[arg(short, long)]
        force: bool,
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
    let flags = options.flags;

    // Print the program version, if requested:
    if flags.version {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Print the program license, if requested:
    if flags.license {
        print!("{}", include_str!("../../../UNLICENSE"));
        return Ok(());
    }

    // Configure debug output:
    if flags.debug {}

    match options.command.unwrap() {
        Command::Id { format, paths } => {
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

        Command::Init {} => {
            let _repository = FsRepository::create(".bitcache")?;
            Ok(())
        },

        Command::List {
            format,
            prefix,
            after,
            limit,
        } => {
            use bitcache_core::futures_util::StreamExt;
            let mut options = ListOptions::new();
            if let Some(prefix) = prefix {
                if prefix.len() > 64 || !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
                    eprintln!("bitcache: invalid ID prefix: {}", prefix);
                    return Err(SysexitsError::EX_USAGE);
                }
                options = options.with_prefix(&prefix);
            }
            options.after = after;
            options.limit = limit;
            let repository = FsRepository::open(".bitcache")?;
            let mut ids = std::pin::pin!(repository.list(options));
            while let Some(id) = ids.next().await {
                let id = id?;
                match format {
                    IdEncoding::Hex => print!("{}", id.to_hex()),
                    #[cfg(feature = "base58")]
                    IdEncoding::Base58 => print!("{}", id.to_base58()),
                }
                if flags.verbose == 0 {
                    println!();
                    continue;
                }
                let blob = repository.get(&id).await?.unwrap();
                let metadata = blob.metadata();
                let len = metadata.len();
                let media_type = metadata.media_type().unwrap_or("application/octet-stream");
                let created = metadata
                    .created_secs()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                let accessed = metadata
                    .accessed_secs()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                match flags.verbose {
                    0 => unreachable!(),
                    1 => println!("\t{}", len),
                    2 => println!("\t{}\t{}", len, media_type),
                    3 => println!("\t{}\t{}\t{}", len, media_type, created),
                    4 | _ => println!("\t{}\t{}\t{}\t{}", len, media_type, created, accessed),
                }
            }
            Ok(())
        },

        Command::Has { ids } => {
            let repository = FsRepository::open(".bitcache")?;
            for id in ids {
                if !repository.contains(&id).await? {
                    eprintln!("bitcache: blob not found: {}", id.to_hex());
                    return Err(SysexitsError::from(std::io::Error::from(
                        std::io::ErrorKind::NotFound,
                    )));
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

        Command::Rm { ids } => {
            let mut repository = FsRepository::open(".bitcache")?;
            for id in ids {
                if !repository.remove(&id).await? {
                    eprintln!("bitcache: blob not found: {}", id.to_hex());
                    return Err(SysexitsError::from(std::io::Error::from(
                        std::io::ErrorKind::NotFound,
                    )));
                }
            }
            Ok(())
        },

        Command::Clear { force } => {
            if !force {
                eprintln!("bitcache: refusing to clear the repository without --force");
                return Err(SysexitsError::EX_USAGE);
            }
            let mut repository = FsRepository::open(".bitcache")?;
            repository.clear().await?;
            Ok(())
        },
    }
}
