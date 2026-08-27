// This is free and unencumbered software released into the public domain.

use bitcache::{
    Bytes, DynRepository, Id, IdEncoding, ListOptions, Repository, RepositoryError,
    futures_util::StreamExt,
};
use clientele::{
    ColorChoiceExt, StandardOptions, SysexitsError,
    crates::clap::{self, CommandFactory, FromArgMatches, Parser, Subcommand},
};
use std::path::PathBuf;

/// Bitcache is a distributed content-addressable storage (CAS) system.
#[derive(Debug, Parser)]
#[command(name = "Bitcache", long_about)]
#[command(arg_required_else_help = true)]
#[command(styles = clientele::HELP_STYLES)]
struct Options {
    #[clap(flatten)]
    flags: StandardOptions,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Compute the BLAKE3 hash of the given file(s).
    ///
    /// Prints the ID each file would have as a blob, one per line, without
    /// accessing or modifying any repository.
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
    ///
    /// Creates an empty repository in the `./.bitcache/` directory of the
    /// current working directory; `$BITCACHE_URL` is ignored.
    Init {},

    /// List the IDs of the blobs in the repository, in ascending order.
    ///
    /// With `--verbose` (repeatable), appends further tab-separated columns
    /// to each line: the blob's byte size, media type, creation timestamp,
    /// and last-access timestamp.
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
    ///
    /// Prints the ID of each stored blob, one per line. Since blobs are
    /// content addressed, storing already-present content is harmless: the
    /// blob is simply retained with the same ID.
    Put {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT", default_value = "hex")]
        format: IdEncoding,

        /// Expire the stored blob(s) after the given number of seconds.
        ///
        /// Requires a repository backend that supports blob expiration
        /// (e.g., Valkey); exits with an error otherwise.
        #[arg(long, value_name = "SECS", value_parser = clap::value_parser!(u64).range(1..))]
        ttl: Option<u64>,

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
    ///
    /// As a safety measure, this requires the `--force` flag; without it,
    /// nothing is removed and the command exits with a usage error.
    #[clap(aliases = ["reset"])]
    Clear {
        /// Actually perform the operation; without this, nothing is removed.
        #[arg(short, long)]
        force: bool,
    },

    /// Copy blobs missing from the given remote repositories to them.
    ///
    /// Every blob present in the current repository but absent from a remote
    /// repository is copied to that remote repository.
    Push {
        /// The URLs of the remote repositories to push to.
        #[arg(value_name = "URLS")]
        remotes: Vec<String>,
    },

    /// Copy blobs missing from the current repository from the given remotes.
    ///
    /// Every blob present in a remote repository but absent from the current
    /// repository is copied into the current repository.
    Pull {
        /// The URLs of the remote repositories to pull from.
        #[arg(value_name = "URLS")]
        remotes: Vec<String>,
    },

    /// Synchronize with the given remote repositories, in both directions.
    ///
    /// Equivalent to a `pull` followed by a `push` for each given remote
    /// repository: afterwards, the current repository and every given remote
    /// repository all contain the union of their blobs.
    #[clap(aliases = ["rsync"])]
    Sync {
        /// The URLs of the remote repositories to synchronize with.
        #[arg(value_name = "URLS")]
        remotes: Vec<String>,
    },
}

/// How subcommands are grouped into sections in the `--help` output.
/// Only the grouping is defined here; names and help texts are
/// introspected from the [`Command`] definition itself.
const COMMAND_SECTIONS: &[(&str, &[&str])] = &[
    ("General commands:", &["id", "help"]),
    (
        "Current repository commands (`$BITCACHE_URL`, default `./.bitcache/`):",
        &["init", "list", "has", "get", "put", "rm", "clear"],
    ),
    ("Remote repository commands:", &["push", "pull", "sync"]),
];

/// Renders the subcommand list grouped into [`COMMAND_SECTIONS`],
/// using the names and help summaries from the given [`clap::Command`].
fn subcommand_help_sections(command: &clap::Command) -> String {
    let subcommands: Vec<&clap::Command> = command
        .get_subcommands()
        .filter(|subcommand| !subcommand.is_hide_set())
        .collect();
    let name_width = subcommands
        .iter()
        .map(|subcommand| subcommand.get_name().len())
        .max()
        .unwrap_or_default();
    let render = |output: &mut String, subcommand: &clap::Command| {
        let about = subcommand
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default();
        output.push_str(&format!(
            "  {:name_width$}  {}\n",
            subcommand.get_name(),
            about
        ));
    };
    let mut output = String::new();
    for (heading, names) in COMMAND_SECTIONS {
        let heading = color_print::cformat!("<y>{}</y>", heading);
        output.push_str(&heading);
        output.push('\n');
        for name in *names {
            let subcommand = command
                .find_subcommand(name)
                .unwrap_or_else(|| panic!("unknown subcommand in COMMAND_SECTIONS: {}", name));
            render(&mut output, subcommand);
        }
        output.push('\n');
    }
    // Catch-all for subcommands not (yet) assigned to a section above:
    let orphans: Vec<&&clap::Command> = subcommands
        .iter()
        .filter(|subcommand| {
            !COMMAND_SECTIONS
                .iter()
                .any(|(_, names)| names.contains(&subcommand.get_name()))
        })
        .collect();
    if !orphans.is_empty() {
        output.push_str("Other commands:\n");
        for subcommand in orphans {
            render(&mut output, subcommand);
        }
        output.push('\n');
    }
    output
}

/// Parses command-line options, using a help template that groups the
/// subcommands into the sections defined by [`COMMAND_SECTIONS`].
fn parse_options(args: impl IntoIterator<Item = std::ffi::OsString>) -> Options {
    let mut command = Options::command();
    command.build(); // adds the implicit `help` subcommand
    let options_heading = color_print::cstr!("<y>Options:</y>");
    let template = format!(
        "{{before-help}}{{about-with-newline}}\n\
         {{usage-heading}} {{usage}}\n\n\
         {}\
         {options_heading}:\n{{options}}{{after-help}}",
        subcommand_help_sections(&command)
    );
    let matches = command.help_template(template).get_matches_from(args);
    Options::from_arg_matches(&matches).unwrap_or_else(|error| error.exit())
}

/// The entry point for the `bitcache` command-line interface.
#[tokio::main]
pub async fn main() -> Result<(), SysexitsError> {
    // Load environment variables from `.env`:
    clientele::dotenv().ok();

    // Expand wildcards and @argfiles:
    let args = clientele::args_os()?;

    // Determine the color output mode ahead of parsing, so that Clap's own
    // help/usage/error rendering honors `--color` (the default is "auto"):
    let color = clientele::color_choice(&args);
    let _use_color = color.to_bool();

    // Parse command-line options:
    let options = parse_options(args);
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
            let _repository = bitcache_fs::FsRepository::create(".bitcache")?;
            Ok(())
        },

        Command::List {
            format,
            prefix,
            after,
            limit,
        } => {
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
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
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
                let expires = metadata
                    .expires_secs()
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                match flags.verbose {
                    0 => unreachable!(),
                    1 => println!("\t{}", len),
                    2 => println!("\t{}\t{}", len, media_type),
                    3 => println!("\t{}\t{}\t{}", len, media_type, created),
                    4 => println!("\t{}\t{}\t{}\t{}", len, media_type, created, accessed),
                    5 | _ => println!(
                        "\t{}\t{}\t{}\t{}\t{}",
                        len, media_type, created, accessed, expires
                    ),
                }
            }
            Ok(())
        },

        Command::Has { ids } => {
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
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
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
            let mut stdout = tokio::io::stdout();
            for id in ids {
                let Some(blob) = repository.get(&id).await? else {
                    eprintln!("bitcache: blob not found: {}", id.to_hex());
                    return Err(SysexitsError::from(std::io::Error::from(
                        std::io::ErrorKind::NotFound,
                    )));
                };
                tokio::io::copy(&mut blob.read(), &mut stdout).await?;
            }
            use tokio::io::AsyncWriteExt;
            stdout.flush().await?;
            Ok(())
        },

        Command::Put { format, ttl, paths } => {
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
            for path in paths {
                let buffer = tokio::fs::read(&path).await?;
                let bytes = Bytes::from(buffer);
                let id = repository.put(bytes).await?;
                if let Some(secs) = ttl {
                    let expires_nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64
                        + secs * 1_000_000_000;
                    if !repository.expire(&id, Some(expires_nanos)).await? {
                        eprintln!("bitcache: repository does not support blob expiration");
                        return Err(SysexitsError::EX_UNAVAILABLE);
                    }
                }
                match format {
                    IdEncoding::Hex => println!("{}", id.to_hex()),
                    #[cfg(feature = "base58")]
                    IdEncoding::Base58 => println!("{}", id.to_base58()),
                }
            }
            Ok(())
        },

        Command::Rm { ids } => {
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
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
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
            repository.clear().await?;
            Ok(())
        },

        Command::Push { remotes } => {
            let local_repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
            for remote in remotes {
                let mut remote_repository = bitcache::open(&remote)?;
                sync(&local_repository, &mut remote_repository).await?;
            }
            Ok(())
        },

        Command::Pull { remotes } => {
            let mut local_repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
            for remote in remotes {
                let remote_repository = bitcache::open(&remote)?;
                sync(&remote_repository, &mut local_repository).await?;
            }
            Ok(())
        },

        Command::Sync { remotes } => {
            let mut local_repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache")?;
            for remote in remotes {
                let mut remote_repository = bitcache::open(&remote)?;
                sync(&remote_repository, &mut local_repository).await?;
                sync(&local_repository, &mut remote_repository).await?;
            }
            Ok(())
        },
    }
}

async fn sync(
    source: &DynRepository<'_, RepositoryError>,
    target: &mut DynRepository<'_, RepositoryError>,
) -> Result<u64, RepositoryError> {
    let mut count = 0;
    let mut ids = std::pin::pin!(source.list(ListOptions::default()));
    while let Some(id) = ids.next().await {
        let id = id?;
        if !target.contains(&id).await?
            && let Some(blob) = source.get(&id).await?
        {
            let blob_data = blob.read();
            target.put(blob_data.into_bytes()).await?;
            count += 1;
        }
    }
    Ok(count)
}
