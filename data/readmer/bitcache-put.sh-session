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

  -C, --cwd <DIR>
          Change to this directory before executing the command

  -h, --help
          Print help (see a summary with '-h')
