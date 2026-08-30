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

  -C, --cwd <DIR>
          Change to this directory before executing the command

  -h, --help
          Print help (see a summary with '-h')
