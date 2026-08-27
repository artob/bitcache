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

          [default: hex]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
