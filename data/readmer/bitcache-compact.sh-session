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

  -C, --cwd <DIR>
          Change to this directory before executing the command

  -h, --help
          Print help (see a summary with '-h')
