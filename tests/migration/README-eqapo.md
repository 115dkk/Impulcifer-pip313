# P12 EqualizerAPO oracle fixtures

## Regeneration

Run in the foreground with the repository's Python 3.14 oracle environment.

```sh
py -3.14 E:/Impulcifer/tests/migration/export_goldens_eqapo.py
```

The standalone exporter calls `core/eqapo.py` without patches. It uses
`FrequencyResponse.generate_frequencies(f_min=10, f_max=24000, f_step=1.01)`
and a 48000 Hz sample rate. The duplicate-node case additionally queries exact
node frequencies, zero and a negative frequency. JSON uses `allow_nan=False`.
Scalar exceptions have a separate `p12_arithmetic.json` fixture containing
exception class names rather than nonfinite numeric arrays.

`p12_manifest.json` records the oracle versions and parser fixture filenames.
Each parser fixture contains its input text, frequency grid, optional portable
base directory, decoded text map, WAV channel map, and the complete expected
left/right arrays, counters, preamp totals, channel split and all three report
lists. The arithmetic and detection fixtures have their own schemas. The total
P12 fixture budget is 12 MiB.

Includes and convolutions are evaluated by Python against real temporary files.
The serialized loader inputs use `/eqapo`; config text uses relative filenames,
so no expected report is rewritten. Included UTF-8 BOMs are decoded with
`utf-8-sig`, whereas a BOM passed directly to the parser is preserved. WAV inputs
are written as DOUBLE and read back as f64 channel arrays before serialization.
The exporter removes its temporary directory after completion.

Two existing EqualizerAPO-XT engine configs are copied verbatim into fixtures:
`tests/fixtures/eqapo/biquad_peaking_1khz.txt` and
`tests/fixtures/eqapo/iir_order2_lowpass.txt`. These fixtures verify the Python
magnitude parser, not a second execution of the C++ engine.

## Rust gates

```sh
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --test golden_eqapo --test properties_eqapo --test golden_stages
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --test golden_eqapo -- --nocapture --test-threads=1
```

Numeric comparisons use `atol=1e-9`, `rtol=1e-11`. Every counter, preamp total,
channel-split decision, description and complete bypass/skip report must match
exactly. The final command prints each corpus name, maximum absolute dB error,
and exact-report result. Environment-variable tests use a child test process
with an explicitly supplied environment; they do not mutate the test runner's
environment.

## Loader contract and platform boundaries

`stages::eqapo` does no filesystem I/O. `EqApoLoader::read_text` returns decoded
text; the caller must implement the Python UTF-8-sig then cp1252 decoding policy.
`read_wav` returns `(sample_rate, channels)`, with each channel containing equal
numbers of f64 samples. Errors and empty/malformed WAV arrays become the same
file-not-found bypass reasons as Python.

Paths are normalized lexically. Unlike Python `abspath`, the parser does not
consult the process working directory: callers should pass an absolute base or
make their loader resolve relative bases. Root-relative Windows paths do not
acquire an implicit current drive. The loader owns filesystem existence checks,
permissions, decoding and symlink traversal. No canonicalization occurs;
symlink aliases cannot be identified as one include by the parser, although the
depth-eight limit still applies. Windows include identity is lowercased using
Rust Unicode casing, whereas Python uses its native `normcase` implementation;
unusual Unicode filesystem names can differ between their Unicode versions.
The loader receives the original normalized spelling, not that lowercase key.

Convolution expands native environment-variable syntax, but Include does not.
The one-hertz sample-rate tolerance is preserved. File errors and sample-rate
mismatch take precedence over empty channel scope. Mono maps to both ears;
two or more channels use the first two. Arithmetic exceptions from Python's
scalar calculations become `DspError::InvalidArgument`; NumPy nonfinite array
arithmetic remains nonfinite rather than gaining an undocumented validity cap.

The old `read_eq_settings_csv` intentionally retains its legacy rejection string
because `golden_stages` asserts the exact text. Use `read_eq_settings` for the new
content-based routing and `EqApoLogReport`. CSV parsing itself is unchanged.
This packet does not implement a filesystem loader, register features, or run a
performance audit.
