# Bitcache

[![License](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://unlicense.org)
[![Package on Crates.io](https://img.shields.io/crates/v/bitcache)](https://crates.io/crates/bitcache)
[![Package on NPM](https://img.shields.io/npm/v/bitcache.js)](https://npmjs.com/package/bitcache.js)
[![Package on Pub.dev](https://img.shields.io/pub/v/bitcache)](https://pub.dev/packages/bitcache)
[![Package on PyPI](https://img.shields.io/pypi/v/bitcache)](https://pypi.org/project/bitcache)
[![Package on RubyGems](https://img.shields.io/gem/v/bitcache)](https://rubygems.org/gems/bitcache)

**Bitcache is a distributed content-addressable storage (CAS) system.**

<sub>

[[Features](#-features)] |
[[Prerequisites](#%EF%B8%8F-prerequisites)] |
[[Installation](#%EF%B8%8F-installation)] |
[[Examples](#-examples)] |
[[Reference](#-reference)] |
[[Development](#%E2%80%8D-development)]

</sub>

<br/>

## ✨ Features

- Available both as the command-line tool [`bitcache`] and a polyglot library.
- Polyglot software also <sup><sub>(soon!)</sub></sup> for Dart, Python, Ruby, Rust, and TypeScript.
- Cuts red tape: 100% free and unencumbered public domain software.

## ⬇️ Installation

### Installation of the CLI

#### Installation via [Cargo Binstall]

```bash
cargo binstall -y bitcache
```

<img width="100%" alt="Installation via cargo-binstall" src="https://github.com/artob/bitcache/raw/master/rust/etc/asciinema/install.gif"/>

#### Installation via [mise]

```bash
mise use -g github:artob/bitcache
```

#### Installation via [Cargo]

```bash
cargo install bitcache --locked --features=cli
```

### Installation of the Library

<details>
<summary>Installation for Rust from Crates.io</summary>

#### Installation from [Crates.io]

```bash
cargo add bitcache
```
</details>

<details>
<summary>Installation for JavaScript/TypeScript from NPM</summary>

#### Installation from [NPM]

```bash
npm install bitcache.js
bun add bitcache.js
pnpm add bitcache.js
yarn add bitcache.js
```
</details>

<details>
<summary>Installation for Dart from Pub.dev</summary>

#### Installation from [Pub.dev]

```bash
dart pub add bitcache
flutter pub add bitcache
```
</details>

<details>
<summary>Installation for Python from PyPI</summary>

#### Installation from [PyPI]

```bash
pip install -U bitcache
uv add bitcache
poetry add bitcache
pdm add bitcache
```
</details>

<details>
<summary>Installation for Ruby from RubyGems</summary>

#### Installation from [RubyGems]

```bash
gem install bitcache
bundle add bitcache
```
</details>

## 👉 Examples

## 📚 Reference

### Command-Line Interface

```shellsession
$ bitcache --help
Bitcache is a distributed content-addressable storage (CAS) system.

Usage: bitcache [OPTIONS] [COMMAND]

General commands:
  id       Compute the BLAKE3 hash of the given file(s), or of stdin
  help     Print this message or the help of the given subcommand(s)

Current repository commands (`$BITCACHE_URL`, default `./.bitcache/`):
  init     Initialize a new repository in `./.bitcache/`
  list     List the IDs of the blobs in the repository, in ascending order
  has      Check whether the repository contains blob(s) with the given ID(s)
  get      Fetch blob(s) from the repository, writing their contents to stdout
  put      Store the given file(s) into the repository as blob(s)
  rm       Remove blob(s) with the given ID(s) from the repository
  clear    Remove all blobs from the repository
  export   Export all blobs in the repository into a tarball

Remote repository commands:
  push     Copy blobs missing from the given remote repositories to them
  pull     Copy blobs missing from the current repository from the given remotes
  sync     Synchronize with the given remote repositories, in both directions

Other commands:
  compact  Compact the repository's physical storage

Options::
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

      --license
          Show license information

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -V, --version
          Print version information

  -h, --help
          Print help (see a summary with '-h')
```

- [`bitcache clear`](#bitcache-clear) - Remove all blobs from the repository
- [`bitcache compact`](#bitcache-compact) - Compact the repository's physical storage
- [`bitcache export`](#bitcache-export) - Export all blobs in the repository into a tarball
- [`bitcache get`](#bitcache-get) - Fetch blob(s) from the repository, writing their contents to stdout
- [`bitcache has`](#bitcache-has) - Check whether the repository contains blob(s) with the given ID(s)
- [`bitcache id`](#bitcache-id) - Compute the BLAKE3 hash of the given file(s)
- [`bitcache init`](#bitcache-init) - Initialize a new repository in `./.bitcache/`
- [`bitcache list`](#bitcache-list) - List the IDs of the blobs in the repository, in ascending order
- [`bitcache pull`](#bitcache-pull) - Copy blobs missing from the current repository from the given remotes
- [`bitcache push`](#bitcache-push) - Copy blobs missing from the given remote repositories to them
- [`bitcache put`](#bitcache-put) - Store the given file(s) into the repository as blob(s)
- [`bitcache rm`](#bitcache-rm) - Remove blob(s) with the given ID(s) from the repository
- [`bitcache sync`](#bitcache-sync) - Synchronize with the given remote repositories, in both directions

#### `bitcache clear`

```shellsession
$ bitcache clear --help
Remove all blobs from the repository.

As a safety measure, this requires the `--force` flag; without it, nothing is removed and the command exits with a usage error.

Usage: bitcache clear [OPTIONS]

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -f, --force
          Actually perform the operation; without this, nothing is removed

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache compact`

```shellsession
$ bitcache compact --help
Compact the repository's physical storage.

Filesystem repositories rewrite stored blobs using the requested compression scheme and clean up orphaned temporary artifacts. Other repository backends may perform no maintenance.

Usage: bitcache compact [OPTIONS]

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

      --compress <SCHEME>
          The target compression scheme for stored blobs.

          One of `none`, `xz`, `xz:fast`, or `xz:best` (`xz` is an alias for `xz:fast`). Defaults to the `compress` directive of the `[bitcache.compact]` config section, or else `xz`.

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache export`

```shellsession
$ bitcache export --help
Export all blobs in the repository into a tarball.

Without `--output`, the tar stream is written to stdout, so it can be piped to `xz`, `bzip2`, `gzip`, etc.

Usage: bitcache export [OPTIONS]

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -o, --output <FILE>
          The path to the tarball file to create (default: stdout)

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache get`

```shellsession
$ bitcache get --help
Fetch blob(s) from the repository, writing their contents to stdout.

IDs may be given as unambiguous hexadecimal prefixes: each prefix resolves to the first matching blob ID in the repository.

Exits with a nonzero status unless all of the given blobs were found in the repository.

Usage: bitcache get [OPTIONS] [IDS]...

Arguments:
  [IDS]...
          The IDs (or unambiguous ID prefixes) of the blob(s) to fetch

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -n, --lines <COUNT>
          Print only the first COUNT lines of each blob

  -d, --debug
          Enable debugging output

  -f, --format <FORMAT>
          The output format: `raw` (the default) or `base64`

          Possible values:
          - raw:    The blob's raw contents
          - base64: ASCII-armored (Base64-encoded) contents, one line per blob

          [default: raw]

  -o, --output <FILE>
          Write the output to this file instead of stdout.

          With a single blob, raw output, and no line limit, filesystem repositories reflink uncompressed blobs to the output file on supporting filesystems, avoiding a data copy.

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache has`

```shellsession
$ bitcache has --help
Check whether the repository contains blob(s) with the given ID(s).

With `--verbose`, prints `true` or `false` for each ID.

Exits with a nonzero status unless all of the given blobs were found in the repository.

Usage: bitcache has [OPTIONS] [IDS]...

Arguments:
  [IDS]...
          The IDs of the blob(s) to check for

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache id`

```shellsession
$ bitcache id --help
Compute the BLAKE3 hash of the given file(s), or of stdin.

Prints the ID each file would have as a blob, one per line, without accessing or modifying any repository. With no files (or with `-`), reads from standard input.

Usage: bitcache id [OPTIONS] [FILES]...

Arguments:
  [FILES]...
          The paths to the file(s) to hash (`-` or none for stdin)

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -f, --format <FORMAT>
          The format to use for the hash output

          Possible values:
          - hex:    Hexadecimal (aka Base16)
          - base58: Base58

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache init`

```shellsession
$ bitcache init --help
Initialize a new repository in `./.bitcache/`.

Creates an empty repository in the `./.bitcache/` directory of the current working directory; `$BITCACHE_URL` is ignored. The given options are recorded in the created `.bitcache/config.toml`; an existing
configuration file is never overwritten.

Usage: bitcache init [OPTIONS]

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

      --hashing <ALGORITHM>
          The content-hashing algorithm to use (only `blake3`)

      --capacity <COUNT>
          A capacity hint for how many blobs will be stored.

          A count with an optional `K`, `M`, `B`, or `T` suffix (e.g., `100M` for one hundred million).

  -d, --debug
          Enable debugging output

      --encoding <FORMAT>
          The default encoding for displaying blob IDs

          Possible values:
          - hex:    Hexadecimal (aka Base16)
          - base58: Base58

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

      --without-git
          Skip creating the `.gitattributes` and `.gitignore` files

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache list`

```shellsession
$ bitcache list --help
List the IDs of the blobs in the repository, in ascending order.

With `--verbose` (repeatable), appends further tab-separated columns to each line: the blob's byte size, media type, creation timestamp, last-update timestamp, last-access timestamp, and expiration
timestamp.

Usage: bitcache list [OPTIONS] [PREFIX]

Arguments:
  [PREFIX]
          List only IDs whose hexadecimal encoding begins with this prefix

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -f, --format <FORMAT>
          The format to use for the hash output

          Possible values:
          - hex:    Hexadecimal (aka Base16)
          - base58: Base58

  -a, --after <ID>
          List only IDs ordered strictly after this one

  -d, --debug
          Enable debugging output

  -n, --limit <COUNT>
          List at most this many IDs

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache pull`

```shellsession
$ bitcache pull --help
Copy blobs missing from the current repository from the given remotes.

Every blob present in a remote repository but absent from the current repository is copied into the current repository.

Usage: bitcache pull [OPTIONS] [URLS]...

Arguments:
  [URLS]...
          The URLs of the remote repositories to pull from

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache push`

```shellsession
$ bitcache push --help
Copy blobs missing from the given remote repositories to them.

Every blob present in the current repository but absent from a remote repository is copied to that remote repository.

Usage: bitcache push [OPTIONS] [URLS]...

Arguments:
  [URLS]...
          The URLs of the remote repositories to push to

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache put`

```shellsession
$ bitcache put --help
Store the given file(s) into the repository as blob(s).

Prints the ID of each stored blob, one per line. Since blobs are content addressed, storing already-present content is harmless: the blob is simply retained with the same ID.

Usage: bitcache put [OPTIONS] [FILES]...

Arguments:
  [FILES]...
          The paths to the file(s) to store

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -f, --format <FORMAT>
          The format to use for the hash output

          Possible values:
          - hex:    Hexadecimal (aka Base16)
          - base58: Base58

      --compress <SCHEME>
          The compression scheme for physically storing the blob(s).

          One of `none`, `xz`, `xz:fast`, or `xz:best` (`xz` is an alias for `xz:fast`). Defaults to the `compress` directive of the `[bitcache.put]` config section, or else `none`.

  -d, --debug
          Enable debugging output

      --ttl <DURATION>
          Expire the stored blob(s) after the given duration.

          Accepts a plain number of seconds (e.g. "90") or a human-friendly duration (e.g. "90s", "2m30s", "1h", "7d").

          Requires a repository backend that supports blob expiration (e.g., filesystem, Turso, or Valkey); exits with an error otherwise.

      --media-type <TYPE>
          Store an explicit media type (MIME type) for the blob(s)

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache rm`

```shellsession
$ bitcache rm --help
Remove blob(s) with the given ID(s) from the repository.

Exits with a nonzero status unless all of the given blobs were found in the repository.

Usage: bitcache rm [OPTIONS] [IDS]...

Arguments:
  [IDS]...
          The IDs of the blob(s) to remove

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

#### `bitcache sync`

```shellsession
$ bitcache sync --help
Synchronize with the given remote repositories, in both directions.

Equivalent to a `pull` followed by a `push` for each given remote repository: afterwards, the current repository and every given remote repository all contain the union of their blobs.

Usage: bitcache sync [OPTIONS] [URLS]...

Arguments:
  [URLS]...
          The URLs of the remote repositories to synchronize with

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
```

### Configuration File

### Storage Adapters

| URL Scheme            | Adapter Crate     | `ls`  | `get`  | `put`  | `rm`  | `clear` |
| --------------------- | ----------------- | ----- | ------ | ------ | ----- | ------- |
| `file:`               | bitcache-fs       | ✓    | ✓    | ✓    | ✓    | ✓ |
| `git:`                | bitcache-git      | ✓    | ✓    | ✓    | ✓    | ✓ |
| `heap:`               | bitcache-heap     | ✓    | ✓    | ✓    | ✓    | ✓ |
| `opendal+azblob:`     | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | x |
| `opendal+fs:`         | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ✓ |
| `opendal+ftp:`        | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ✓ |
| `opendal+memcached:`  | bitcache-opendal  | x    | ✓    | ✓    | ✓    | x |
| `opendal+memory:`     | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ✓ |
| `opendal+mongodb:`    | bitcache-opendal  | x    | ✓    | ✓    | ✓    | x |
| `opendal+gcs:`        | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ✓ |
| `opendal+http:`       | bitcache-opendal  | x    | ✓    | x    | x    | x |
| `opendal+redis:`      | bitcache-opendal  | x    | ✓    | ✓    | ✓    | x |
| `opendal+s3:`         | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ✓ |
| `opendal+sftp:`       | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ? |
| `opendal+sled:`       | bitcache-opendal  | ✓    | ✓    | ✓    | ✓    | ✓ |
| `redis:`              | bitcache-valkey   | ✓    | ✓    | ✓    | ✓    | ✓ |
| `sqlite:`             | bitcache-turso    | ✓    | ✓    | ✓    | ✓    | ✓ |
| `valkey:`             | bitcache-valkey   | ✓    | ✓    | ✓    | ✓    | ✓ |

#### File System Adapter

```dotenv
BITCACHE_URL=file:.bitcache
BITCACHE_URL=file:/tmp/bitcache
```

#### Git Adapter

```dotenv
BITCACHE_URL=git://github.com/asimov-datasets/gutenberg.org.git
```

#### Heap (Memory) Adapter

```dotenv
BITCACHE_URL=heap:
```

#### OpenDAL Adapter

##### Azure Blob Storage ([`azblob`]) Service

```dotenv
BITCACHE_URL=opendal+azblob://my-container
```

<details>
<summary>Configuration for Floci AZ</summary>

###### Configuration for [Floci AZ](https://github.com/floci-io/floci-az)

```dotenv
BITCACHE_URL=opendal+azblob://my-container?endpoint=http://localhost:4577/devstoreaccount1&skip_signature=true
```
</details>

##### File System Service ([`fs`])

```dotenv
BITCACHE_URL=opendal+fs:///tmp/bitcache
```

##### FTP Service ([`ftp`])

```dotenv
BITCACHE_URL=opendal+ftp://localhost
```

<details>
<summary>Configuration for pyftpdlib</summary>

###### Configuration for [pyftpdlib](https://github.com/giampaolo/pyftpdlib)

```dotenv
BITCACHE_URL=opendal+ftp://127.0.0.1:2121?user=anonymous&password=jhacker@example.org
```
</details>

##### Google Cloud Storage Service ([`gcs`])

```dotenv
BITCACHE_URL=opendal+gcs://my-bucket/my-root
```

<details>
<summary>Configuration for Floci GCP</summary>

###### Configuration for [Floci GCP](https://github.com/floci-io/floci-gcp)

```dotenv
BITCACHE_URL=opendal+gcs://my-bucket/my-root?endpoint=http://localhost:4588&skip_signature=true
```
</details>

##### HTTP Service ([`http`])

```dotenv
BITCACHE_URL=opendal+http://localhost:8000
```

##### Memcached Service ([`memcached`])

```dotenv
BITCACHE_URL=opendal+memcached://localhost:11211
```

##### Memory Service ([`memory`])

```dotenv
BITCACHE_URL=opendal+memory://
```

##### MongoDB Service ([`mongodb`])

```dotenv
BITCACHE_URL=opendal+mongodb://localhost:27017/my-database/my-collection
```

##### Redis Service ([`redis`])

```dotenv
BITCACHE_URL=opendal+redis://localhost:6379
```

##### S3 Service ([`s3`])

```dotenv
BITCACHE_URL=opendal+s3://my-bucket
```

<details>
<summary>Configuration for Floci AWS</summary>

###### Configuration for [Floci AWS](https://github.com/floci-io/floci)

```dotenv
BITCACHE_URL=opendal+s3://my-bucket?region=us-east-1&endpoint=http://localhost:4566&skip_signature=true
```
</details>

##### SFTP Service ([`sftp`])

```dotenv
BITCACHE_URL=opendal+sftp://my-host
```

##### Sled Service ([`sled`])

```dotenv
BITCACHE_URL=opendal+sled:///tmp/bitcache
```

##### Miscellaneous Services

OpenDAL supports dozens more additional [services](https://opendal.apache.org/services/);
however, if we haven't validated them yet, we won't have a feature flag for
them nor URL scheme support in Bitcache directly. (Submit a pull request to add
support for your favorite service!)

[`azblob`]: https://opendal.apache.org/services/azblob/
[`fs`]: https://opendal.apache.org/services/fs/
[`ftp`]: https://opendal.apache.org/services/ftp/
[`gcs`]: https://opendal.apache.org/services/gcs/
[`http`]: https://opendal.apache.org/services/http/
[`memcached`]: https://opendal.apache.org/services/memcached/
[`memory`]: https://opendal.apache.org/services/memory/
[`mongodb`]: https://opendal.apache.org/services/mongodb/
[`redis`]: https://opendal.apache.org/services/redis/
[`s3`]: https://opendal.apache.org/services/s3/
[`sftp`]: https://opendal.apache.org/services/sftp/
[`sled`]: https://opendal.apache.org/services/sled/

#### Turso (aka SQLite) Adapter

```dotenv
BITCACHE_URL=sqlite:/tmp/bitcache.db
```

#### Valkey (fka Redis) Adapter

```dotenv
BITCACHE_URL=valkey://localhost:6379
```

## 👨‍💻 Development

```bash
git clone https://github.com/artob/bitcache.git
```

---

[![Share on X](https://img.shields.io/badge/share%20on-x-03A9F4?logo=x)](https://x.com/intent/post?url=https%3A%2F%2Fgithub.com%2Fartob%2Fbitcache&text=Bitcache)
[![Share on Reddit](https://img.shields.io/badge/share%20on-reddit-red?logo=reddit)](https://reddit.com/submit?url=https%3A%2F%2Fgithub.com%2Fartob%2Fbitcache&title=Bitcache)
[![Share on Hacker News](https://img.shields.io/badge/share%20on-hn-orange?logo=ycombinator)](https://news.ycombinator.com/submitlink?u=https%3A%2F%2Fgithub.com%2Fartob%2Fbitcache&t=Bitcache)
[![Share on Facebook](https://img.shields.io/badge/share%20on-fb-1976D2?logo=facebook)](https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Fgithub.com%2Fartob%2Fbitcache)
[![Share on LinkedIn](https://img.shields.io/badge/share%20on-linkedin-3949AB?logo=linkedin)](https://www.linkedin.com/sharing/share-offsite/?url=https%3A%2F%2Fgithub.com%2Fartob%2Fbitcache)

[`bitcache`]: https://github.com/artob/bitcache#command-line-interface

[Crates.io]: https://crates.io/crates/bitcache
[NPM]: https://npmjs.com/package/bitcache.js
[Pub.dev]: https://pub.dev/packages/bitcache
[PyPI]: https://pypi.org/project/bitcache
[RubyGems]: https://rubygems.org/gems/bitcache

[Cargo]: https://rustup.rs
[Cargo Binstall]: https://crates.io/crates/cargo-binstall
[mise]: https://mise.jdx.dev
