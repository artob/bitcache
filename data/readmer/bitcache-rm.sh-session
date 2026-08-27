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
