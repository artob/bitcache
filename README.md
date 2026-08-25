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
- Polyglot software <sup><sub>(soon!)</sub></sup> available for Dart, Python, Ruby, Rust, and TypeScript.
- Cuts red tape: 100% free and unencumbered public domain software.

## ⬇️ Installation

### Installation of the CLI

#### Installation via [Cargo Binstall]

```bash
cargo binstall -y bitcache
```

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
```

### Storage Adapters

| URL Scheme            | Adapter Crate     | `ls`  | `get`  | `put`  | `rm`  | `clear` |
| --------------------- | ----------------- | ----- | ------ | ------ | ----- | ------- |
| `file:`               | bitcache-fs       | ✓    | ✓    | ✓    | ✓    | ✓ |
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

#### File System Adapter

```dotenv
BITCACHE_URL=file:.bitcache
BITCACHE_URL=file:/tmp/bitcache
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
