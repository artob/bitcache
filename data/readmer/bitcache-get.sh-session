$ bitcache get --help
Fetch blob(s) from the repository, writing their contents to stdout.

IDs may be given as unambiguous hexadecimal prefixes: each prefix resolves to the first matching blob ID in the repository.

Exits with a nonzero status unless all of the given blobs were found in the repository.

Usage: bitcache get [OPTIONS] [IDS]...

Arguments:
  [IDS]...
          The IDs (or unambiguous ID prefixes) of the blob(s) to fetch

Options:
      --color <COLOR>
          Set the color output mode

          [default: auto]
          [possible values: auto, always, never]

  -n, --lines <COUNT>
          Print only the first COUNT lines of each blob

  -d, --debug
          Enable debugging output

  -f, --format <FORMAT>
          The output format: `raw` (the default) or `base64`

          Possible values:
          - raw:    The blob's raw contents
          - base64: ASCII-armored (Base64-encoded) contents, one line per blob

          [default: raw]

  -o, --output <FILE>
          Write the output to this file instead of stdout.

          With a single blob, raw output, and no line limit, filesystem repositories reflink uncompressed blobs to the output file on supporting filesystems, avoiding a data copy.

  -v, --verbose...
          Enable verbose output (may be repeated for more verbosity)

  -C, --cwd <DIR>
          Change to this directory before executing the command

  -h, --help
          Print help (see a summary with '-h')
