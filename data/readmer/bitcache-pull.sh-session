$ bitcache pull --help
Copy blobs missing from the current repository from the given remotes.

Every blob present in a remote repository but absent from the current repository is copied into the current repository.

Usage: bitcache pull [OPTIONS] [URLS]...

Arguments:
  [URLS]...
          The URLs of the remote repositories to pull from

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
