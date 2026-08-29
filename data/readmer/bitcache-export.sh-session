$ bitcache export --help
Export all blobs in the repository into a tarball.

Without `--output`, the tar stream is written to stdout, so it can be piped to `xz`, `bzip2`, `gzip`, etc.

Usage: bitcache export [OPTIONS]

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -o, --output <FILE>
          The path to the tarball file to create (default: stdout)

  -d, --debug
          Enable debugging output

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -h, --help
          Print help (see a summary with '-h')
