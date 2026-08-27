$ bitcache --help
Bitcache is a distributed content-addressable storage (CAS) system.

Usage: bitcache [OPTIONS] [COMMAND]

General commands:
  id     Compute the BLAKE3 hash of the given file(s)
  help   Print this message or the help of the given subcommand(s)

Current repository commands (`$BITCACHE_URL`, default `./.bitcache/`):
  init   Initialize a new repository in `./.bitcache/`
  list   List the IDs of the blobs in the repository, in ascending order
  has    Check whether the repository contains blob(s) with the given ID(s)
  get    Fetch blob(s) from the repository, writing their contents to stdout
  put    Store the given file(s) into the repository as blob(s)
  rm     Remove blob(s) with the given ID(s) from the repository
  clear  Remove all blobs from the repository

Remote repository commands:
  push   Copy blobs missing from the given remote repositories to them
  pull   Copy blobs missing from the current repository from the given remotes
  sync   Synchronize with the given remote repositories, in both directions

Options::
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -d, --debug
          Enable debugging output

      --license
          Show license information

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -V, --version
          Print version information

  -h, --help
          Print help (see a summary with '-h')
