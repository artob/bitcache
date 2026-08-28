# Bitcache

[![License](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://unlicense.org)
[![Compatibility](https://img.shields.io/badge/rust-1.91%2B-blue)](https://endoflife.date/rust)
[![Package on Crates.io](https://img.shields.io/crates/v/bitcache)](https://crates.io/crates/bitcache)
[![Documentation](https://img.shields.io/docsrs/bitcache?label=docs.rs)](https://docs.rs/bitcache)

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

- Available both as the command-line tool [`bitcache`] and a Rust library.
- 100% pure and safe Rust with minimal dependencies and no bloat.
- Designed for `#![no_std]` environment compatibility from the get-go.
- Supports opting out of any feature using comprehensive [feature flags].
- Adheres to the Rust API Guidelines in its [naming conventions].
- Polyglot software <sup><sub>(soon!)</sub></sup> available for Dart, Python, Ruby, and TypeScript.
- Cuts red tape: 100% free and unencumbered public domain software.

## 🛠️ Prerequisites

- [Rust] 1.91+

## ⬇️ Installation

### Installation of the CLI

#### Installation via [Cargo Binstall]

```bash
cargo binstall -y bitcache
```

<img width="100%" alt="Installation via cargo-binstall" src="{{ github.repository.link }}/raw/master/rust/etc/asciinema/install.gif"/>

#### Installation via [mise]

```bash
mise use -g github:artob/bitcache
```

#### Installation via [Cargo]

```bash
cargo install bitcache --locked --features=cli
```

#### Downloading Release Tarballs

```bash
wget https://github.com/artob/bitcache/releases/download/0.0.4/bitcache-aarch64-apple-darwin.tar.xz
wget https://github.com/artob/bitcache/releases/download/0.0.4/bitcache-aarch64-unknown-linux-gnu.tar.xz
wget https://github.com/artob/bitcache/releases/download/0.0.4/bitcache-x86_64-apple-darwin.tar.xz
wget https://github.com/artob/bitcache/releases/download/0.0.4/bitcache-x86_64-pc-windows-msvc.zip
wget https://github.com/artob/bitcache/releases/download/0.0.4/bitcache-x86_64-unknown-linux-gnu.tar.xz
```

### Installation of the Library

<details>
<summary>Installation from Crates.io</summary>

#### Installation from [Crates.io]

```bash
cargo add bitcache
```
</details>

<details>
<summary>Configuration in <code>Cargo.toml</code></summary>

#### Configuration in `Cargo.toml`

Enable all default features:

```toml
[dependencies]
bitcache = { version = "0" }
```

Enable only specific features:

```toml
[dependencies]
bitcache = { version = "0", default-features = false, features = ["alloc"] }
```
</details>

## 👉 Examples

### Importing the Library

```rust
use bitcache::*;
```

## 📚 Reference

[docs.rs/bitcache](https://docs.rs/bitcache)

### Command-Line Interface

```shellsession
$ bitcache --help
```

### Feature Flags

#### Interoperability

| Feature          | Version  | Summary |
| ---------------- | -------- | ------- |
| `serde`          | 1.0      | Derives `serde::{Serialize, Deserialize}` |

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
[feature flags]: https://docs.rs/crate/bitcache/latest/features
[naming conventions]: https://rust-lang.github.io/api-guidelines/naming.html

[Cargo]: https://rustup.rs
[Cargo Binstall]: https://crates.io/crates/cargo-binstall
[Rust]: https://rust-lang.org
[mise]: https://mise.jdx.dev
