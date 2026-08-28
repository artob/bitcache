$ bitcache list --help
List the IDs of the blobs in the repository, in ascending order.

With `--verbose` (repeatable), appends further tab-separated columns to each line: the blob's byte size, media type, creation timestamp, last-update timestamp, last-access timestamp, and expiration
timestamp.

Usage: bitcache list [OPTIONS]

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

          [default: hex]

  -d, --debug
          Enable debugging output

  -p, --prefix <PREFIX>
          List only IDs whose hexadecimal encoding begins with this prefix

  -a, --after <ID>
          List only IDs ordered strictly after this one

  -n, --limit <COUNT>
          List at most this many IDs

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
