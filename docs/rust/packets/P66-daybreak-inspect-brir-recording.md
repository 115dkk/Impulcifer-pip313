# P66 (Daybreak): `inspect_brir_recording`, the sweeps in one recording before the job

ADR 0016 lets a BRIR come from one two-ear recording in which a program played one
sweep per speaker. The user types the speakers in playing order
(`brir_recording_speakers`). Today the number of sweeps in the file is known only when
the job reaches the binaural step (`BrirError::RecordingSweepCount`), so the user
cannot write the list in playing order without running the job. This packet adds a
read-only IPC method that answers, right after the file is chosen, how many sweeps
the recording holds and where each one starts. The screen (Claude) builds a per-sweep
list from it.

Work in `E:\wt\recording-sweep-disclosure` only. Do not touch `apps/impulcifer-app/ui/`
except `apps/impulcifer-app/ui/ipc.d.ts`, and do not touch any `locales/*.json`.

## 1. DSP: locating without deconvolving

`crates/impulcifer-dsp/src/sweep_import.rs`

Split the start-finding part of `import_sweeps` into a public function

```rust
/// Starts (in samples) of every sweep in a two-ear recording, sorted,
/// with the reference's analysis band.
pub fn locate_sweeps(
    left: &[f64],
    right: &[f64],
    reference: &ReferenceSweep,
) -> Result<SweepLocations, DspError>;

#[derive(Clone, Debug, PartialEq)]
pub struct SweepLocations { pub picks: Vec<usize>, pub band: (f64, f64) }
```

It contains everything `import_sweeps` does up to and including the
`"overlapping sweeps"` check (the length checks, logarithmic test, band, correlation
energy, picking, sorting, overlap check), with the same error messages. `import_sweeps`
must call `locate_sweeps` and continue from its result, so the two can never disagree.
`import_sweeps`' output must stay bit-identical: the existing tests in that file and
`cargo test -p impulcifer-dsp` must pass unchanged, and `brir-3x` hashes depend on it.
Add one unit test `locate_sweeps_matches_import_picks` on the existing `synthetic()`
fixture: `locate_sweeps(..).picks == import_sweeps(..).picks` and the bands are equal.

## 2. Service: the inspection

New module `crates/impulcifer-service/src/brir/recording_inspect.rs` (declare it in
`brir/mod.rs`), function `pub(crate) fn inspect(request: &Value, data: &Path) -> Result<Value, Value>`.

Request object (all strings trimmed; empty string = absent):

| field | meaning |
|---|---|
| `recording_file` | required. The two-ear WAV. |
| `recording_sweep` | optional. Another program's sweep WAV (the job's `brir_recording_sweep`). |
| `test_signal` | optional. The job's `test_signal` value (`auto`, `default`, `generate:<s>s@<fs>`, a path, or absent). Used only without `recording_sweep`. |
| `dir_path` | optional. The job's folder, for the `test.wav` sidecar under `auto`. A missing folder is not an error; it just has no sidecar. |

Reference sweep:
- with `recording_sweep`: read it and `trim_reference` it, exactly as
  `recording_file.rs::located_method` does (same `RecordingSweepRate` and
  `RecordingSweepFile` errors, same "no tracks" handling);
- without: the test signal the job would open. Build the `MeasurementDir` the job
  would build (or an empty one when `dir_path` is absent/missing; read how `run.rs`
  builds it) and call `estimator::open_with_events(dir, test_signal, data, true, None, true, None)`
  (`recording_sweep = true` skips the folder detection, as in the job with a sweep
  file; the result is the sidecar, the explicit signal, the generated one or the
  bundled default). Its `test_signal` samples and `fs` are the reference; pass them
  through `trim_reference`. If its fs differs from the recording's, answer the same
  `RecordingSweepRate` error (with `path` = the test signal path or the spec string).

Recording: read with the same `read_recording` helper (missing file → its
`BrirError::Missing` envelope), require two tracks like `recording_file::load`, use
tracks 0 and 1.

Then `locate_sweeps`. Map `NO_SWEEP_FOUND` to a successful answer with `found: 0`,
`NOT_LOGARITHMIC_SWEEP` to `RecordingSweepFile`, other DSP errors to
`BrirError::Unsupported(error.to_string())`. Every error is `BrirError::envelope()`.

Answer (`json!`):

```json
{
  "found": 3,
  "starts_seconds": [1.5, 9.5, 17.5],
  "sweep_seconds": 6.15,
  "fs": 48000,
  "reference": "file"
}
```

`starts_seconds` are `pick / fs` rounded to hundredths (same rounding the
`RecordingSweepCount` envelope uses), `sweep_seconds` the trimmed reference length
rounded to hundredths, `reference` is `"file"` with `recording_sweep` and
`"test_signal"` without. With `found: 0`, `starts_seconds` is `[]`.

The count must equal what the job would find: the job's `located_method` uses
`import_sweeps`, which now goes through `locate_sweeps`, so this holds for the
sweep-file case by construction. For the no-sweep-file case the job uses the folder
method instead; that is fine, the screen only uses the answer as a guide there.

Factor the reference-reading code out of `located_method` into a helper both use,
rather than copying it.

## 3. IPC registration

- `crates/impulcifer-types/src/ipc.rs`: `IpcMethod::InspectBrirRecording` ↔
  `"inspect_brir_recording"` in every list/match there.
- `crates/impulcifer-service/src/lib.rs`: dispatch next to `InspectRoomMeasurements`:
  `args.count(1, 1)?; brir::recording_inspect::inspect(args.get(0), &self.data_dir)`
  (check what the service uses as its data dir for `open_with_events` in `run.rs` and
  use the same).
- `crates/impulcifer-policy/tests/gates.rs`: add to `CANONICAL_IPC`.
- `features.toml`: new entry `ipc.inspect_brir_recording`, `status = "implemented"`,
  `tests = [...]` naming the real tests below plus the dsp test.
- `apps/impulcifer-app/ui/ipc.d.ts`: request/answer interfaces and the `IpcApi`
  method, in the style of `inspect_room_measurements`.
- grep for every other place that enumerates IPC methods (bridge, Android bridge,
  CLI `--info`, key-count asserts in tests) and add the method where the others are.

## 4. Tests (`crates/impulcifer-service/tests/`, new file `brir_recording_inspect.rs` or next to `brir_ipc.rs`)

Build fixtures synthetically in a temp dir the way `brir_inputs.rs` builds its
single-recording fixtures (read those helpers first and reuse them):

1. `inspect_brir_recording_counts_the_sweeps_of_a_sweep_file`: a recording with N
   (e.g. 3) sweeps of a WAV sweep at irregular starts → `found == N`, starts within
   0.02 s of the planted starts, `reference == "file"`.
2. `inspect_brir_recording_uses_the_test_signal_without_a_sweep_file`: recording made
   with a `generate:` test signal, request with that `test_signal` and no sweep →
   correct count, `reference == "test_signal"`.
3. `inspect_brir_recording_reports_zero_and_errors`: silent recording → `found: 0`;
   missing recording → FILE_NOT_FOUND-style envelope the job also gives; sweep at a
   different rate → the `RecordingSweepRate` message; linear sweep → `RecordingSweepFile`
   message; missing `recording_file` / non-object request → invalid request.
4. Through the IPC dispatcher (`pywebview_api("inspect_brir_recording", [..])` as
   `brir_ipc.rs` does) at least once.

## 5. Gates (run in the foreground, report the output tail of each)

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp sweep_import
cargo test -p impulcifer-service
cargo test -p impulcifer-policy
cargo test -p impulcifer-types
```

The shell sets `HTTPS_PROXY`; if updater loopback tests fail with CONNECT errors,
rerun with `env -u HTTPS_PROXY` and report both. Do not commit. Do not bump versions
or edit CHANGELOG.

## Report

Changed files, the public signatures you added, the exact JSON shape, test names, and
gate output tails. Then the list for the screen: method name, request fields, answer
fields, error messages it can return.
