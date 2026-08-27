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
