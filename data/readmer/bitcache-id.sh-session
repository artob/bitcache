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
