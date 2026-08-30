$ bitcache push --help
Copy blobs missing from the given remote repositories to them.

Every blob present in the current repository but absent from a remote repository is copied to that remote repository.

Usage: bitcache push [OPTIONS] [URLS]...

Arguments:
  [URLS]...
          The URLs of the remote repositories to push to

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
