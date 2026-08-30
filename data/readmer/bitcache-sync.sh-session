$ bitcache sync --help
Synchronize with the given remote repositories, in both directions.

Equivalent to a `pull` followed by a `push` for each given remote repository: afterwards, the current repository and every given remote repository all contain the union of their blobs.

Usage: bitcache sync [OPTIONS] [URLS]...

Arguments:
  [URLS]...
          The URLs of the remote repositories to synchronize with

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -C, --cwd <DIR>
          Change to this directory before executing the command

  -h, --help
          Print help (see a summary with '-h')
