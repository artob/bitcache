$ bitcache clear --help
Remove all blobs from the repository.

As a safety measure, this requires the `--force` flag; without it, nothing is removed and the command exits with a usage error.

Usage: bitcache clear [OPTIONS]

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -f, --force
          Actually perform the operation; without this, nothing is removed

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -C, --cwd <DIR>
          Change to this directory before executing the command

  -h, --help
          Print help (see a summary with '-h')
