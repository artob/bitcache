// This is free and unencumbered software released into the public domain.

use bitcache::{
    CompactOptions, Compression, Config, DynRepository, Id, IdEncoding, ListOptions, ListOrder,
    PutOptions, Repository, RepositoryError, futures_util::StreamExt,
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
    /// Compute the BLAKE3 hash of the given file(s), or of stdin.
    ///
    /// Prints the ID each file would have as a blob, one per line, without
    /// accessing or modifying any repository. With no files (or with `-`),
    /// reads from standard input.
    #[clap(aliases = ["identify", "hash"])]
    Id {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT")]
        format: Option<IdEncoding>,

        /// The paths to the file(s) to hash (`-` or none for stdin).
        #[arg(value_name = "FILES")]
        paths: Vec<PathBuf>,
    },

    /// Initialize a new repository in `./.bitcache/`.
    ///
    /// Creates an empty repository in the `./.bitcache/` directory of the
    /// current working directory; `$BITCACHE_URL` is ignored. The given
    /// options are recorded in the created `.bitcache/config.toml`; an
    /// existing configuration file is never overwritten.
    Init {
        /// The content-hashing algorithm to use (only `blake3`).
        #[arg(long, alias = "hash", value_name = "ALGORITHM")]
        hashing: Option<bitcache::Hashing>,

        /// A capacity hint for how many blobs will be stored.
        ///
        /// A count with an optional `K`, `M`, `B`, or `T` suffix
        /// (e.g., `100M` for one hundred million).
        #[arg(long, value_name = "COUNT")]
        capacity: Option<bitcache::Capacity>,

        /// The default encoding for displaying blob IDs.
        #[arg(long, value_name = "FORMAT")]
        encoding: Option<IdEncoding>,

        /// Skip creating the `.gitattributes` and `.gitignore` files.
        #[arg(long)]
        without_git: bool,
    },

    /// List the IDs of the blobs in the repository, in ascending order.
    ///
    /// With `--verbose` (repeatable), appends further tab-separated columns
    /// to each line: the blob's byte size, media type, creation timestamp,
    /// last-update timestamp, last-access timestamp, and expiration timestamp.
    #[clap(alias = "ls")]
    List {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT")]
        format: Option<IdEncoding>,

        /// List only IDs ordered strictly after this one.
        #[arg(short = 'a', long, value_name = "ID")]
        after: Option<Id>,

        /// List at most this many IDs.
        #[arg(short = 'n', long, value_name = "COUNT")]
        limit: Option<usize>,

        /// List only IDs whose hexadecimal encoding begins with this prefix.
        #[arg(value_name = "PREFIX")]
        prefix: Option<String>,
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
    /// IDs may be given as unambiguous hexadecimal prefixes: each prefix
    /// resolves to the first matching blob ID in the repository.
    ///
    /// Exits with a nonzero status unless all of the given blobs were found
    /// in the repository.
    #[clap(alias = "cat")]
    Get {
        /// Print only the first COUNT lines of each blob.
        #[arg(short = 'n', long, value_name = "COUNT")]
        lines: Option<usize>,

        /// The output format: `raw` (the default) or `base64`.
        #[arg(short, long, value_name = "FORMAT", default_value = "raw")]
        format: GetFormat,

        /// Write the output to this file instead of stdout.
        ///
        /// With a single blob, raw output, and no line limit, filesystem
        /// repositories reflink uncompressed blobs to the output file on
        /// supporting filesystems, avoiding a data copy.
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// The IDs (or unambiguous ID prefixes) of the blob(s) to fetch.
        #[arg(value_name = "IDS")]
        ids: Vec<String>,
    },

    /// Store the given file(s) into the repository as blob(s).
    ///
    /// Prints the ID of each stored blob, one per line. Since blobs are
    /// content addressed, storing already-present content is harmless: the
    /// blob is simply retained with the same ID.
    Put {
        /// The format to use for the hash output.
        #[arg(short, long, value_name = "FORMAT")]
        format: Option<IdEncoding>,

        /// The compression scheme for physically storing the blob(s).
        ///
        /// One of `none`, `xz`, `xz:fast`, or `xz:best` (`xz` is an alias
        /// for `xz:fast`). Defaults to the `compress` directive of the
        /// `[bitcache.put]` config section, or else `none`.
        #[arg(long, value_name = "SCHEME")]
        compress: Option<Compression>,

        /// Expire the stored blob(s) after the given duration.
        ///
        /// Accepts a plain number of seconds (e.g. "90") or a
        /// human-friendly duration (e.g. "90s", "2m30s", "1h", "7d").
        ///
        /// Requires a repository backend that supports blob expiration
        /// (e.g., filesystem, Turso, or Valkey); exits with an error otherwise.
        #[arg(long, value_name = "DURATION", value_parser = parse_ttl)]
        ttl: Option<std::time::Duration>,

        /// Store an explicit media type (MIME type) for the blob(s).
        #[arg(long, value_name = "TYPE")]
        media_type: Option<String>,

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

    /// Compact the repository's physical storage.
    ///
    /// Filesystem repositories rewrite stored blobs using the requested
    /// compression scheme and clean up orphaned temporary artifacts. Other
    /// repository backends may perform no maintenance.
    Compact {
        /// The target compression scheme for stored blobs.
        ///
        /// One of `none`, `xz`, `xz:fast`, or `xz:best` (`xz` is an alias
        /// for `xz:fast`). Defaults to the `compress` directive of the
        /// `[bitcache.compact]` config section, or else `xz`.
        #[arg(long, value_name = "SCHEME")]
        compress: Option<Compression>,
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

    /// Export all blobs in the repository into a tarball.
    ///
    /// Without `--output`, the tar stream is written to stdout, so it can
    /// be piped to `xz`, `bzip2`, `gzip`, etc.
    Export {
        /// The path to the tarball file to create (default: stdout).
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
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
        &["init", "list", "has", "get", "put", "rm", "clear", "export"],
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

/// The output format for `bitcache get`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
enum GetFormat {
    /// The blob's raw contents.
    Raw,

    /// ASCII-armored (Base64-encoded) contents, one line per blob.
    Base64,
}

/// Parses a time-to-live duration, given either as a plain number of
/// seconds (e.g. "90") or in a human-friendly format (e.g. "2m30s").
fn parse_ttl(input: &str) -> Result<std::time::Duration, String> {
    let ttl = clientele::crates::duration_str::parse_std(input)?;
    if ttl.is_zero() {
        return Err(String::from("the duration must be nonzero"));
    }
    Ok(ttl)
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

    // Load the repository configuration (`.bitcache/config.toml`), if any:
    let config = match Config::load_or_default(CONFIG_PATH) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("bitcache: {}", error);
            return Err(SysexitsError::EX_CONFIG);
        },
    };
    let default_format = config.bitcache.encoding.unwrap_or_default();

    match options.command.unwrap() {
        Command::Id { format, paths } => {
            let format = format.unwrap_or(default_format);
            let stdin = PathBuf::from("-");
            let paths = if paths.is_empty() { vec![stdin.clone()] } else { paths };
            for path in paths {
                let id = if path == stdin {
                    bitcache_core::sync::identify_input(std::io::stdin().lock())?
                } else {
                    bitcache_core::sync::identify_file(&path)?
                };
                println!("{}", format_id(&id, format));
            }
            Ok(())
        },

        Command::Init {
            hashing,
            capacity,
            encoding,
            without_git,
        } => {
            let _repository = bitcache_fs::FsRepository::create_with_options(
                ".bitcache",
                bitcache_fs::CreateOptions::new().with_git(!without_git),
            )?;
            if !std::path::Path::new(CONFIG_PATH).exists() {
                let mut config = Config::default();
                config.bitcache.hashing = hashing.unwrap_or_default();
                config.bitcache.capacity = capacity;
                config.bitcache.encoding = encoding;
                let toml = config.to_toml().map_err(|error| {
                    eprintln!("bitcache: {}", error);
                    SysexitsError::EX_SOFTWARE
                })?;
                std::fs::write(CONFIG_PATH, toml)?;
            }
            Ok(())
        },

        Command::List {
            format,
            prefix,
            after,
            limit,
        } => {
            let format = format.unwrap_or(default_format);
            let mut list_options = ListOptions::default().with_order(ListOrder::Ascending);
            if let Some(prefix) = prefix {
                if prefix.len() > 64 || !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
                    eprintln!("bitcache: invalid ID prefix: {}", prefix);
                    return Err(SysexitsError::EX_USAGE);
                }
                list_options = list_options.with_prefix(&prefix);
            }
            list_options.after = after;
            list_options.limit = limit;
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            let mut ids = std::pin::pin!(repository.list(list_options));
            while let Some(id) = ids.next().await {
                let id = id?;
                print!("{}", format_id(&id, format));
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
                let updated = metadata
                    .updated_secs()
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
                    4 => println!("\t{}\t{}\t{}\t{}", len, media_type, created, updated),
                    5 => println!(
                        "\t{}\t{}\t{}\t{}\t{}",
                        len, media_type, created, updated, accessed
                    ),
                    6 | _ => println!(
                        "\t{}\t{}\t{}\t{}\t{}\t{}",
                        len, media_type, created, updated, accessed, expires
                    ),
                }
            }
            Ok(())
        },

        Command::Has { ids } => {
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
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

        Command::Get {
            lines,
            format,
            output,
            ids,
        } => {
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            let mut resolved = Vec::with_capacity(ids.len());
            for input in &ids {
                resolved.push(resolve_id(&repository, input).await?);
            }

            // Fast path: a single raw, unabridged blob written to a file can
            // be reflinked by the repository backend where supported:
            if let Some(path) = &output
                && resolved.len() == 1
                && format == GetFormat::Raw
                && lines.is_none()
            {
                let id = &resolved[0];
                if !repository.get_to_path(id, path).await? {
                    eprintln!("bitcache: blob not found: {}", id.to_hex());
                    return Err(SysexitsError::from(std::io::Error::from(
                        std::io::ErrorKind::NotFound,
                    )));
                }
                return Ok(());
            }

            let mut buffer: Vec<u8> = Vec::new();
            for id in &resolved {
                let Some(blob) = repository.get(id).await? else {
                    eprintln!("bitcache: blob not found: {}", id.to_hex());
                    return Err(SysexitsError::from(std::io::Error::from(
                        std::io::ErrorKind::NotFound,
                    )));
                };
                let data = blob.read().into_bytes();
                let data = match lines {
                    Some(count) => first_lines(&data, count),
                    None => &data,
                };
                match format {
                    GetFormat::Raw => buffer.extend_from_slice(data),
                    GetFormat::Base64 => {
                        buffer.extend_from_slice(data_encoding::BASE64.encode(data).as_bytes());
                        buffer.push(b'\n');
                    },
                }
            }
            match output {
                Some(path) => tokio::fs::write(&path, &buffer).await?,
                None => {
                    use tokio::io::AsyncWriteExt;
                    let mut stdout = tokio::io::stdout();
                    stdout.write_all(&buffer).await?;
                    stdout.flush().await?;
                },
            }
            Ok(())
        },

        Command::Put {
            format,
            compress,
            ttl,
            media_type,
            paths,
        } => {
            let format = format.unwrap_or(default_format);
            let compress = compress
                .or(config.bitcache.put.as_ref().and_then(|put| put.compress))
                .unwrap_or(Compression::None);
            let options = PutOptions::new()
                .with_ttl(ttl)
                .with_compression(compress)
                .with_media_type(media_type.map(std::borrow::Cow::Owned));
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            let metadata_capabilities = repository.capabilities().blob_metadata();
            if options.ttl.is_some() && !metadata_capabilities.expires() {
                eprintln!("bitcache: repository does not support blob expiration");
                return Err(SysexitsError::EX_UNAVAILABLE);
            }
            if options.media_type().is_some() && !metadata_capabilities.media_type() {
                eprintln!("bitcache: repository does not support media-type metadata");
                return Err(SysexitsError::EX_UNAVAILABLE);
            }
            for path in paths {
                let id = repository.put_from_path(&path, options.clone()).await?;
                println!("{}", format_id(&id, format));
            }
            Ok(())
        },

        Command::Rm { ids } => {
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
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

        Command::Compact { compress } => {
            let compress = compress
                .or(config
                    .bitcache
                    .compact
                    .as_ref()
                    .and_then(|compact| compact.compress))
                .unwrap_or(Compression::XzFast);
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            repository
                .compact_with_options(CompactOptions::new().with_compression(compress))
                .await?;
            Ok(())
        },

        Command::Clear { force } => {
            if !force {
                eprintln!("bitcache: refusing to clear the repository without --force");
                return Err(SysexitsError::EX_USAGE);
            }
            let mut repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            repository.clear().await?;
            Ok(())
        },

        Command::Export { output } => {
            let repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            let writer: std::pin::Pin<Box<dyn tokio::io::AsyncWrite + Send>> = match &output {
                Some(path) => Box::pin(tokio::fs::File::create(path).await?),
                None => Box::pin(tokio::io::stdout()),
            };
            let mut tarball = tokio_tar::Builder::new(writer);
            tarball.mode(tokio_tar::HeaderMode::Deterministic);
            let list_options = ListOptions::default().with_order(ListOrder::Ascending);
            let mut ids = std::pin::pin!(repository.list(list_options));
            while let Some(id) = ids.next().await {
                let id = id?;
                let Some(blob) = repository.get(&id).await? else {
                    continue; // unreachable by contract
                };
                let mut header = tokio_tar::Header::new_gnu();
                header.set_path(id.to_hex().to_string())?;
                header.set_size(blob.len());
                header.set_mode(0o444);
                if let Some(header) = header.as_gnu_mut() {
                    if let Some(changed) = blob.metadata().updated_secs() {
                        header.set_ctime(changed);
                    }
                    if let Some(accessed) = blob.metadata().accessed_secs() {
                        header.set_atime(accessed);
                    }
                }
                header.set_cksum();
                tarball.append(&header, blob.read()).await?;
            }
            use tokio::io::AsyncWriteExt;
            let mut writer = tarball.into_inner().await?; // finishes the archive
            writer.flush().await?;
            Ok(())
        },

        Command::Push { remotes } => {
            let local_repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            for remote in remotes {
                let remote = resolve_remote(&config, remote);
                let mut remote_repository = bitcache::open(&remote).await?;
                sync(&local_repository, &mut remote_repository).await?;
            }
            Ok(())
        },

        Command::Pull { remotes } => {
            let mut local_repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            for remote in remotes {
                let remote = resolve_remote(&config, remote);
                let remote_repository = bitcache::open(&remote).await?;
                sync(&remote_repository, &mut local_repository).await?;
            }
            Ok(())
        },

        Command::Sync { remotes } => {
            let mut local_repository = bitcache::open_env("BITCACHE_URL", "file:.bitcache").await?;
            for remote in remotes {
                let remote = resolve_remote(&config, remote);
                let mut remote_repository = bitcache::open(&remote).await?;
                sync(&remote_repository, &mut local_repository).await?;
                sync(&local_repository, &mut remote_repository).await?;
            }
            Ok(())
        },
    }
}

/// The path to the local repository configuration file.
const CONFIG_PATH: &str = ".bitcache/config.toml";

/// Formats a blob ID using the given encoding.
fn format_id(id: &Id, encoding: IdEncoding) -> String {
    match encoding {
        IdEncoding::Hex => id.to_hex().to_string(),
        #[cfg(feature = "base58")]
        IdEncoding::Base58 => id.to_base58().to_string(),
    }
}

/// Resolves a remote name from a `[bitcache.remote.NAME]` config section to
/// its URL; other arguments are returned unchanged (assumed to be URLs).
fn resolve_remote(config: &Config, remote: String) -> String {
    config
        .remote_url(&remote)
        .map(str::to_string)
        .unwrap_or(remote)
}

/// Resolves a full blob ID, or a hexadecimal ID prefix to the first
/// matching blob ID in the repository.
async fn resolve_id(
    repository: &DynRepository<'_, RepositoryError>,
    input: &str,
) -> Result<Id, SysexitsError> {
    if let Ok(id) = input.parse::<Id>() {
        return Ok(id);
    }
    if input.is_empty() || input.len() > 64 || !input.bytes().all(|b| b.is_ascii_hexdigit()) {
        eprintln!("bitcache: invalid blob ID or prefix: {}", input);
        return Err(SysexitsError::EX_USAGE);
    }
    let options = ListOptions::default()
        .with_order(ListOrder::Ascending)
        .with_prefix(input)
        .with_limit(1);
    let mut ids = std::pin::pin!(repository.list(options));
    match ids.next().await {
        Some(Ok(id)) => Ok(id),
        Some(Err(error)) => Err(error.into()),
        None => {
            eprintln!("bitcache: blob not found: {}", input);
            Err(SysexitsError::from(std::io::Error::from(
                std::io::ErrorKind::NotFound,
            )))
        },
    }
}

/// Truncates the given data to its first `count` lines (including their
/// trailing newlines), like `head -nCOUNT`.
fn first_lines(data: &[u8], count: usize) -> &[u8] {
    let mut remaining = count;
    if remaining == 0 {
        return &[];
    }
    for (index, byte) in data.iter().enumerate() {
        if *byte == b'\n' {
            remaining -= 1;
            if remaining == 0 {
                return &data[..=index];
            }
        }
    }
    data
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
