# P13 CLI oracle fixtures

`export_goldens_cli.py` observes the actual 2.x `impulcifer.create_cli()` parser
with CPython 3.14. It wraps `ArgumentParser.parse_args` only to retain the parser
instance; argument registration, parsing and bass/decay post-processing all run
unchanged. No handwritten Python parser is used and no BRIR processing runs in
the exporter. Python bytecode writes are disabled.

Regenerate from any working directory with

```text
py -3.14 E:/Impulcifer/tests/migration/export_goldens_cli.py
```

## Files and comparison

- `goldens/p13_options.json` contains all 34 argparse actions, including the
  automatic help action, version, info, 30 config arguments and bass boost.
  Each entry retains option strings, destination, help, type, action class,
  choices, default (or `SUPPRESS`) and required status, in parser order.
- `goldens/p13_parse_*.json` contains exactly twelve cases. `argv` includes the
  program name, `kwargs` is the unmodified `create_cli()` result, and failures
  contain the exception message and CLI exit code instead. JSON comparison is
  exact, including omitted defaults and integer versus floating-point values.
- `goldens/p13_help.txt` is stdout from the actual `impulcifer.py --help` process.
  Python uses the script's filename in usage; the Rust executable uses
  `impulcifer`. Help prose is frozen, but clap's layout is not byte-identical to
  argparse's wrapping or metavariables.
- `goldens/p13_info_keys.json` contains labels observed from `_print_info()`.
  The Rust diagnostic block replaces Python/GIL/dependency diagnostics with
  Rust toolchain, audio backend and data directory, retaining version, OS and
  CPU count. These machine-dependent values are not golden values.

| Case | Additional arguments (all cases include `--dir_path measurements`) |
| --- | --- |
| defaults | none |
| every_flag | every processing option once, plus bass boost; help/version/info are early-exit options and tested separately |
| bass_gain | `--bass_boost=6` |
| bass_shelf | `--bass_boost=6,150,0.69` |
| decay_uniform | `--decay=300` |
| decay_pairs | `--decay=FL:300,FR:250` |
| head | `--c 2.5` |
| fs | `--fs 44100` |
| balance | `--channel_balance trend` |
| vbass | `--vbass --vbass_freq 200 --vbass_polarity invert` |
| store_false | `--no_room_correction --no_headphone_compensation --no_equalization` |
| bad_bass | `--bass_boost=1,2` (exit 1) |

## Rust entry points and execution

`impulcifer_cli::parse(&[String])` is independently testable and performs no I/O.
It returns help, version, info, or the kwargs map. `impulcifer_cli::run(argv,
out, err) -> i32` constructs `ProcessingConfig` via `from_kwargs`, invokes the
service's `run_brir` in a `JobRegistry` worker and waits synchronously, polling
every 100 ms. The executable only forwards argv and exits with the return code.
The headless `NoopHost` supplies the service's diagnostic method; BRIR execution
uses the host-independent `run_brir` function directly, not the IPC validator.

Settings are read without initializing or rewriting the user's settings file.
Saved languages use embedded catalogues with English fallback. The private
service settings implementation is not called because its read operation also
saves the settings. The CLI mirrors its locale normalization and its one
3.x-only plot-warning key. Data-directory diagnostics mirror the service's
private lookup order.

PROGRESS log events are paired with progress events by the service. The CLI
renders only the latter to avoid duplicate lines. Percentages already truncated
by the service are rounded back to integers when formatting. Other logger
levels retain the Python console prefixes. Successful runs also print the
written README text. Job failure messages are copied to stderr and return 1.
In particular, missing-directory `run_brir` failures carry the missing path;
the IPC validator's `Measurement directory does not exist.` is not that API's
message. `OUTPUT_MISSING` messages likewise propagate unchanged.

## Deliberate limits

- Ctrl-C cancellation is not installed; the packet excludes an additional
  signal-handling dependency.
- Non-finite floats are rejected rather than silently becoming JSON null.
  Integer parsing is bounded by Rust `i64`; `fs` and `vbass_freq` must also fit
  `ProcessingConfig`'s `u32`. Python argparse itself has unbounded integers.
- clap supplies help layout and general parsing diagnostics beyond the tested
  argparse error conventions. The CLI retains argparse-style usage/error
  prefixes, missing-value/unknown-option diagnostics, required-directory
  message and exit codes; it does not reproduce Python tracebacks.
- Plot generation and other service/DSP limitations are inherited unchanged.
  P13 is a parser/job/console implementation, not an additional DSP port.

## Verification

Run the packet commands in the repository root, in the foreground:

```text
py -3.14 E:/Impulcifer/tests/migration/export_goldens_cli.py
cargo fmt -p impulcifer-cli -- --check
cargo clippy -p impulcifer-cli --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-cli
cargo run -p impulcifer-cli -- --help
cargo test -p impulcifer-policy
```

The new `impulcifer-jobs` path dependency requires a lockfile update, which is
outside P13's allowed writes. The implementation worker first verified a
source snapshot under `crates/impulcifer-cli/target/p13-check/`. During concurrent
work, the shared root lockfile acquired that dependency entry. The reviewer
then ran all six commands against the actual shared checkout successfully;
the root lockfile diff was unchanged by that verification. The existing
lockfile change was not reverted or claimed as a P13 edit.

The integration test copies input WAV/CSV/TXT files to an independently created
OS temporary directory. Generated WAVs and README are excluded from inputs;
all outputs are written to the temporary directory, never repository `data/`.
The test also checks that stdout contains the selected-language writing-BRIR
message and the generated README. Temporary test directories are removed when
their guards are dropped.
