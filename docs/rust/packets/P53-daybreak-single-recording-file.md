# P53: BRIR measurements from one recording file (ADR 0016, items 1–4)

Repository root: the worktree you are started in. P52 (measured LFE) is already in the tree; this packet uses
its `PipelineInputs.lfe`. Read first:

- `CLAUDE.md` (3.x workspace section).
- `docs/rust/SINGLE_RECORDING.md`: the specification. Every number in it is normative. The Korean decision
  record is `docs/adr/0016-single-recording-file-and-measured-lfe.md`.
- `crates/impulcifer-service/src/brir/measurement_file.rs` (`load_brir`, `log_itd`) and
  `crates/impulcifer-service/src/brir/inputs.rs` (`open_measurements`, `ingest`): the two existing sources of
  BRIR measurements. The single recording is the third.

## Invariant

Without `brir_recording_file` every output byte and log line stays as it is.

## What to build

### 1. Configuration and CLI

Three extension fields, as the specification's table: `brir_recording_file`, `brir_recording_speakers`,
`brir_recording_sweep` (`Option<String>`, default `None`). Follow `brir_measurement_file` /
`brir_measurement_map` in `crates/impulcifer-types/src/config.rs` (field, default, `EXTENSION_FIELD_NAMES`
(40 → 43), `from_kwargs`), `crates/impulcifer-cli/src/options.rs` (`EXTENSION_OPTIONS`, help text) and
`crates/impulcifer-service/src/brir/validation.rs`. The validation rules and their order are in the
specification; an empty string counts as unset. Put the list parser in one place (a `pub fn` in
`impulcifer-types`, for example `config::parse_recording_speakers(&str) -> Result<Vec<String>, String>`) and use
it from validation and from the reader. The policy gate's field lists and the 2.x parity tests must keep
passing: the new fields belong to the extension lists, not to `FIELD_NAMES`/`OPTIONS`.

### 2. Locating and dividing: `crates/impulcifer-dsp/src/sweep_import.rs` (new module, file-free)

```rust
pub struct ReferenceSweep { pub samples: Vec<f64>, pub fs: u32 }          // the trimmed sweep r
pub struct SweepImport {
    pub picks: Vec<usize>,              // ascending sample indices
    pub band: (f64, f64),               // f_lo, f_hi
    pub pre: usize,
    pub responses: Vec<[Vec<f64>; 2]>,  // per pick: left, right, each pre + T samples
}
pub fn trim_reference(samples: &[f64], fs: u32) -> Result<ReferenceSweep, DspError>;
pub fn import_sweeps(left: &[f64], right: &[f64], reference: &ReferenceSweep) -> Result<SweepImport, DspError>;
```

`import_sweeps` implements "Spectrum, sweep type and band", "Sweep positions" steps 1–3 and 5, and "Impulse
responses" steps 1–3 for every pick (the caller compares the count with the list and drops the `X` entries).
Use `crate::fft::{rfft, irfft}`; check in `fft.rs` what lengths they accept and how `irfft` scales before
relying on it. Everything is sequential. Free the large buffers as soon as they are no longer needed: the
recording can be several minutes long (`N = 2^24`).

Errors are `DspError::InvalidArgument` with plain English messages: reference shorter than 50 ms or with no
active part, active part shorter than 0.5 s, not a logarithmic sweep covering 200–2000 Hz, recording shorter
than the sweep, no sweep found, overlapping sweeps, sweeps too close for a response.

### 3. Service: `crates/impulcifer-service/src/brir/recording_file.rs` (new) and `inputs.rs`

`open_measurements` gains the third branch (before the folder branch, after the measurement-file branch).

- Common: read the WAV (`Missing` when the file does not exist, with the path). Parse the list. When the
  folder has speaker recordings, log info `cli_brir_recording_replaces_folder` `{"count": n}`.
- Without a sweep: the folder method of the specification. `Hrir::open_recording_samples` skips names outside
  `SPEAKER_NAMES` (`crates/impulcifer-dsp/src/hrir.rs`, `ingest_recording`); let `LFE` and `LFE2` through there
  (no folder file name can contain them, so folder results do not change) and move those pairs to the LFE list
  afterwards. Then check that every listed entry other than `X` has both ears; the error text:
  `The recording holds {read} of the {listed} listed sweeps for this test signal. A recording made with another
  sweep needs brir_recording_sweep.`
- Test signal detection for this case: `crates/impulcifer-service/src/brir/estimator.rs` `open_with_events`
  scans the folder today. With `brir_recording_file` set and no `brir_recording_sweep`, detect from that file
  with the list's entry count instead (the per-file rule is in `sweep_grid.rs`; expose what you need as
  `pub(crate)`). With `brir_recording_sweep` set, behave as `external_brir` does today (no scan, bundled
  default, no fallback warning).
- With a sweep: read the sweep WAV, check the rates, `trim_reference`, `import_sweeps` on the first two tracks,
  compare the number of picks with the list (the error gives both numbers and the pick times:
  `Found {found} sweeps in the recording but the list has {listed} entries. Sweeps start at {times} s. Write X
  for a sweep to skip.`), build the HRIR in list order (speakers in `SLOTS` order as `load_brir` does, LFE
  pairs apart), resample to the job's rate when it differs, log `cli_brir_recording_loaded` and call `log_itd`
  (make it `pub(crate)`).
- `load_inputs`: nothing else changes; the LFE pairs flow into `PipelineInputs.lfe` as in P52.
- Catalog keys (do not edit `crates/impulcifer-service/locales/*.json`: the catalog packet P54 writes them in
  all nine languages at the same time as you work; if a test needs them and they are not there yet, report it):
  - `cli_brir_recording_loaded` with `{count}`, `{file}`, `{sweeps}`, `{seconds}`, `{low}`, `{high}`
  - `cli_brir_recording_replaces_folder` with `{count}`

### 4. IPC

`start_brir` already forwards extension fields; confirm with a service IPC test that the three fields reach the
job and that the validation errors carry `details.field`. No new IPC method.

## Tests (must exist and pass; register them in `features.toml`)

- `impulcifer-types`: `recording_speaker_list_parses_and_rejects` (valid list with `X`, `LFE`, spaces; empty
  entry; unknown code; duplicate; missing FL), extension field count.
- `impulcifer-cli`: `cli_recording_file_options_parse`.
- `impulcifer-dsp` (`sweep_import`): build everything synthetically, no fixtures.
  - `reference_trim_keeps_the_active_part`: a log sweep with 3 s of −54 dB noise before and after is trimmed to
    within 60 ms of the sweep's ends; all-zero input is an error.
  - `import_finds_irregular_sweeps_and_recovers_the_responses`: a 10 Hz–20 kHz log sweep of 2 s at 48 kHz; a
    recording made of five copies at irregular starts (spacing 2.4–2.9 s), each convolved with a known response
    per ear (a delay, a gain and a second tap), levels spread over 15 dB, plus −60 dB noise. The picks are
    within one sample of the true starts plus the first-tap delay of the louder ear; each response's first tap
    has the right position and its level within 0.2 dB; the magnitude at 100 Hz, 1 kHz and 10 kHz is within
    0.3 dB of the known response.
  - `import_reports_the_sweeps_it_finds_not_the_number_asked_for`: the pick count for that recording is five
    whatever the caller expects, and a −40 dB burst of noise between sweeps is not a pick.
  - `linear_sweep_is_rejected` and `overlapping_or_too_close_sweeps_are_errors`.
  - `import_is_deterministic`: two calls give identical bytes.
- `impulcifer-service`:
  - `recording_file_without_sweep_matches_the_folder_recording`: copy `data/demo/FL,FR.wav` to an arbitrary
    name outside the measurement folder, run with `brir_recording_speakers = "FL,FR"`; hesuvi.wav is
    byte-identical to the job on a folder holding only `FL,FR.wav` (same other inputs).
  - `recording_file_with_sweep_reads_a_foreign_sequence`: a synthetic recording as in the dsp test with the list
    `FL,FC,X,FR,LFE` produces a BRIR with FL, FC, FR, logs `cli_brir_recording_loaded`, and (with
    `layout_files` naming a format that has an LFE channel) non-silent LFE tracks.
  - `recording_file_errors_name_the_cause`: wrong count (message has both numbers and the times), list missing
    FL, both `brir_recording_file` and `brir_measurement_file`, missing file, sweep with another rate, and the
    no-sweep case with a recording that does not match the test signal.
- `features.toml`: `config.brir_recording_file`, `config.brir_recording_speakers`,
  `config.brir_recording_sweep` and `input.brir_recording_file`, `status = "implemented"`, with the tests above.

## Allowed files

`crates/impulcifer-types/src/config.rs`, `crates/impulcifer-cli/src/options.rs`,
`crates/impulcifer-dsp/src/sweep_import.rs` (new), `crates/impulcifer-dsp/src/lib.rs`,
`crates/impulcifer-dsp/src/hrir.rs` (the `LFE` names in `ingest_recording` only),
`crates/impulcifer-service/src/brir/{recording_file,mod,inputs,estimator,sweep_grid,validation,measurement_file,run}.rs`,
tests of those crates, the policy gate's field
lists if they enumerate extension fields, `features.toml`. Do not touch `apps/`, `.github/`, `docs/`,
`Cargo.toml`, `Cargo.lock`, the 2.x tree, or any golden fixture. No new dependency.

## Verification (foreground; report the tail of each)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-cli -p impulcifer-dsp
cargo test -p impulcifer-service --lib
cargo test -p impulcifer-service --test brir_inputs --test brir_ipc --test config_parity --test ipc --test ui_catalog
cargo test -p impulcifer-policy
```

Then a manual run on the maintainer's recording (read-only inputs; the output folder is a scratch copy the
caller names in the task message) and report the log lines `cli_brir_recording_loaded` and the ITD list. The
shell sets `HTTPS_PROXY`; if a loopback test fails with CONNECT 400, rerun it with `env -u HTTPS_PROXY`. Run every
command in the foreground and end your turn only after reporting: the files changed, the public signatures, the
test names, and the command outputs. Do not commit. Do not use a type alias or any other trick to get past a
policy gate; if a gate or a parity test blocks the design, stop and report it.
