# P40: ITD report, correction and left/right balance checks (ADR 0014)

Repository worktree: `E:\Impulcifer\.worktrees\itd` (Git Bash: `/e/Impulcifer/.worktrees/itd`), branch
`claude/itd-wizard`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\itd\CLAUDE.md`, section "3.x Rust 워크스페이스": `forbid(unsafe_code)`, no new
  dependencies, `features.toml` gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, brir-3x byte identity, "DSP를
  병렬화할 때는 map만" (no rayon reductions; everything here is sequential f64).
- `E:\Impulcifer\.worktrees\itd\docs\rust\ALIGNMENT.md`: **the specification**. Every formula, sign convention,
  name, JSON shape and log key below is defined there; when this packet and the spec differ, the spec wins.
- `E:\Impulcifer\.worktrees\itd\docs\adr\0014-itd-report-and-correction.md` (Korean; the decision and the reasons).

## What to build

A new module `crates/impulcifer-dsp/src/itd.rs` (`pub mod itd;` in `lib.rs`) holding

```rust
pub struct ItdEstimate { pub itd_us: f64, pub inverted: bool }
pub fn estimate(left: &[f64], right: &[f64], fs: u32, max_lag_ms: f64) -> Option<ItdEstimate>;
pub fn band_level_db(ir: &[f64], fs: u32, lo_hz: f64, hi_hz: f64) -> Option<f64>;
pub fn fractional_delay(x: &[f64], fs: u32, delay_us: f64) -> Vec<f64>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ItdMode { Off, Symmetric, Model }
impl ItdMode { pub fn parse(s: &str) -> Option<Self>; pub fn as_str(self) -> &'static str; }
#[derive(Clone, Debug, serde::Serialize)] pub struct SpeakerAlignment { speaker, measured_us, model_us, target_us, after_us, applied_us, lf_ild_db, hf_ild_db, skipped }
#[derive(Clone, Debug, serde::Serialize)] pub struct PairAlignment { left, right, sum_us, yaw_deg }
#[derive(Clone, Debug, serde::Serialize)] pub struct AlignmentReport { mode, head_radius_cm, head_radius_source, yaw_deg, common_offset_us, input_balance_db, hf_asymmetry_db, headphone_balance_db, predicted_balance_db, speakers: Vec<SpeakerAlignment>, pairs: Vec<PairAlignment> }
pub fn analyze(hrir: &Hrir, head_radius_cm: Option<f64>, max_lag_ms: f64) -> AlignmentReport;
pub struct Correction { pub speaker: String, pub target_us: f64, pub applied_us: f64, pub ear: Ear, pub skipped: bool }
pub fn correct(hrir: &mut Hrir, report: &mut AlignmentReport, mode: ItdMode) -> Vec<Correction>;
pub fn measure_after(hrir: &Hrir, report: &mut AlignmentReport, max_lag_ms: f64);
```

Field types follow the spec's JSON: `Option<f64>` for every value that can be `null`, `String` for the speaker
codes and `mode`, `&'static str` or `String` for `head_radius_source` (`"given"`, `"estimated"`, `"default"`).
`Ear` may be an existing enum if the dsp crate has one; otherwise a two-variant enum in `itd.rs`.
`serde` is already a dependency of `impulcifer-dsp` (check `Cargo.toml`; if it is not, derive nothing and build
the JSON in the service instead).

1. **`estimate`** is `crates/impulcifer-service/src/recording/checks.rs::estimate_itd_from_ir` moved into the
   dsp crate with `max_lag_ms` as a parameter (today's constant 1 ms). Move it, do not rewrite it: the recording
   checks must produce the same numbers (`crates/impulcifer-service/tests/recording_checks.rs` pins them). The
   service then calls `impulcifer_dsp::itd::estimate(.., 1.0)`; delete the private copy and its private
   `ItdEstimate`. `direct_arrival` is needed by both `estimate` and `impulse_metrics`; make it `pub fn
   direct_arrival` in `itd.rs` and call it from the service.
2. **`band_level_db`**, **`fractional_delay`**, **`analyze`**, **`correct`**, **`measure_after`** exactly as the
   spec says (sections "Band levels", "Pairs and centre slots", "The spherical head model", "The analysis",
   "Correction"). `analyze` fills `measured_us`, `model_us`, `lf_ild_db`, `hf_ild_db`, the pairs and the summary;
   `target_us`, `after_us`, `applied_us` start as `None` and `skipped` as false; `correct` fills `target_us`,
   `applied_us`, `skipped` and sets `mode`; `measure_after` fills `after_us`. `headphone_balance_db` and
   `predicted_balance_db` stay `None` in the dsp crate; the service fills them.
   Slot geometry comes from `impulcifer_types::layouts::SLOTS` (`azimuth`, `elevation`, ITU: positive azimuth is
   left). Use `fft::rfft`, `fft::irfft`, `fft::next_fast_len`, `filters::butter`, `filters::sosfilt` that the crate
   already has; add no dependency.
3. **Pipeline** (`crates/impulcifer-dsp/src/pipeline.rs`): `ProcessingConfig` gains `itd_mode: String` (default
   `"off"`) and `head_radius: Option<f64>` (cm) in `crates/impulcifer-types/src/config.rs`, registered in
   `EXTENSION_FIELD_NAMES` (27 → 29, and the length assert in `crates/impulcifer-types/tests/output_fill_config.rs`),
   validated in `validate_room_options` with the exact messages `invalid value for itd_mode: must be off, symmetric or
   model` and `invalid value for head_radius: must be between 5 and 15 cm` (follow the `invalid(..)` helper the
   other fields use so the wording matches), and read by `from_kwargs` (`head_radius` absent or `null` → `None`).
   `StageKey::CropAndAlign` becomes the sequence in the spec's "Where it runs": `crop_heads`, `analyze`, `correct`
   (when the mode is not `Off`), `align_ipsilateral_all`, `align_onset_groups_peak_leftref`, `measure_after`,
   `observer.on_alignment(&report, &corrections)?`, `crop_tails`, then the room tunings unchanged. Add
   `fn on_alignment(&mut self, _report: &AlignmentReport, _corrections: &[Correction]) -> Result<(), DspError> { Ok(()) }`
   to `StageObserver` with a default body, and `pub alignment: AlignmentReport` to `PipelineOutputs`.
   With `itd_mode = "off"` the HRIR data must be byte-identical to today after the stage (the analysis only reads).
4. **CLI** (`crates/impulcifer-cli/src/options.rs`, `EXTENSION_OPTIONS`): `--itd_mode` with choices
   `off|symmetric|model` and `--head_radius` (float, cm), both `CliDefault::Suppressed`, help texts from the spec's
   table; check `crates/impulcifer-cli/src/lib.rs` for any list a float option must join and `--info` output.
5. **Service** (`crates/impulcifer-service`):
   - `brir/validation.rs`: accept and validate the two fields like the other extension strings/numbers.
   - `brir/run.rs`: implement `on_alignment` on the `Events`/`BrirEvents` observer: fill
     `headphone_balance_db` and `predicted_balance_db` when headphone compensation is on (spec "The pairing
     check": 200 Hz–2 kHz mean of `left.raw − right.raw` of the `HeadphoneCompensation` the run already holds),
     then log the keys of the spec's table in that order with those placeholders (`.log("info", "cli_itd_report",
     json!({..}))` as the other notices do; numbers as the spec says: integers via `.round() as i64`, one-decimal
     values via `format!("{:.1}")`, `None` as the string `?`). `BrirRun` gains `alignment: serde_json::Value`
     (the spec's JSON object, built from the report) and `IpcMethod::StartBrir` in `lib.rs` returns
     `{"output_path": .., "alignment": ..}`.
   - `brir/outputs.rs`: the README section "Interaural time differences" with keys `cli_readme_itd_title`,
     `cli_readme_itd_row`, `cli_readme_itd_summary`, `cli_readme_itd_pairing` (placeholders in the spec), placed
     after the virtual room tuning section if one exists, else before the layouts section. Values formatted as in
     the log.
   - `lib.rs` `brir_defaults`: add `itd_mode: "off"` and `head_radius: null` beside the other extension defaults.
   - `recording/checks.rs`: per spec "Recording checks": `lf_ild_db` in `Measurement` and `ReferenceMeasurement`
     (serde `Option<f64>`, omitted/`null` accepted), `record_check_input_balance` for speaker takes,
     `headphone_balance_db` and `record_check_headphone_balance` for headphone takes. `check_recording` needs to
     know the mode: add a `headphones: bool` parameter (or the request `Mode`) and update the call site(s).
     `recording/request.rs`: the validator accepts `lf_ild_db` in `reference_measurements` entries and parses
     `reference_measurements` for `Mode::Headphones` as well (today it drops them for that mode).
   - Catalog keys, added to **all nine** `crates/impulcifer-service/locales/*.json` with the English text in every
     file (a translator replaces the other languages later; keep the files' key order sensible: next to the other
     `cli_room_tuning2*` keys for the `cli_itd_*` and `cli_readme_itd_*` keys, next to `record_check_level_changed`
     for the two `record_check_*` keys):

     | key | English |
     | --- | --- |
     | `cli_itd_report` | Interaural time differences: head turned {yaw}° to the left, centre offset {offset_us} µs, recording chain left minus right {balance} dB |
     | `cli_itd_pairing` | Left/right balance of the final BRIR: speaker recording {speaker_db} dB, headphone recording {headphone_db} dB, predicted {predicted_db} dB |
     | `cli_itd_pairing_warning` | The final BRIR will be {predicted_db} dB louder in the left ear than the right (speaker recording {speaker_db} dB, headphone recording {headphone_db} dB). The microphones were probably on different inputs in the two recordings. Record the headphones again with the same microphone on the same input, or set channel_balance. |
     | `cli_itd_head_turned` | The head was turned about {yaw}° to the left during the speaker recording (mirror pairs are asymmetric). itd_mode=symmetric removes the asymmetry. |
     | `cli_itd_corrected` | ITD correction ({mode}): {count} speakers adjusted, largest change {max_us} µs |
     | `cli_itd_skipped` | {speaker}: ITD correction skipped, the change of {us} µs is too large |
     | `cli_readme_itd_title` | Interaural time differences |
     | `cli_readme_itd_row` | {speaker}: measured {measured_us} µs, spherical head {model_us} µs, final {after_us} µs |
     | `cli_readme_itd_summary` | Head turned {yaw}° to the left, centre offset {offset_us} µs, recording chain left minus right {balance} dB, head radius {radius_cm} cm. |
     | `cli_readme_itd_pairing` | Final left/right balance: speaker recording {speaker_db} dB, headphone recording {headphone_db} dB, predicted {predicted_db} dB. |
     | `record_check_input_balance` | The left input is {db_diff} dB louder than the right below 120 Hz, where both ears should be equal. Set both input gains equal, or make sure the headphone recording uses the same microphone on the same input. |
     | `record_check_headphone_balance` | Headphone recording: left minus right {db_diff} dB, while the speaker recordings of this folder differ by the opposite amount. The final BRIR would be {predicted_db} dB off between the ears. Check that each microphone is on the same input as during the speaker recording. |

     `crates/impulcifer-service/tests/ui_catalog.rs::brir_job_log_keys_resolve_in_every_language` scans
     `src/brir/*.rs` for `cli_` keys, so every log key above must appear literally in a `.log("..", "cli_..."` call.
6. **Policy**: `crates/impulcifer-policy/tests/gates.rs` `CANONICAL_CONFIG` (50 → 52: `itd_mode`, `head_radius`);
   `features.toml` entries `config.itd_mode`, `config.head_radius` (`kind = "config"`), `alignment.itd_report`,
   `alignment.itd_correction` (`kind = "stage"`), `recording.input_balance_check` (`kind = "recording"`), all
   `status = "implemented"` with the real test paths below. Put the config entries next to `config.lfe_mode`.

## Tests (must exist and pass; names are yours to keep, list them in the report)

- `impulcifer-dsp`
  - `itd_estimate_recovers_integer_and_fractional_delays`: a band-limited pulse pair (e.g. a Hann-windowed 1 kHz
    burst or a low-passed click) with the right ear delayed by 7 samples and by 7.4 samples (made with
    `fractional_delay`): `itd_us` positive and within 2 µs of the delay in both cases; swapping the ears flips the
    sign; a polarity-inverted right ear sets `inverted`.
  - `fractional_delay_matches_integer_shift_and_keeps_length`: for an integer delay the result equals
    `ImpulseResponse::shift` (max abs difference < 1e-9 relative to the peak) and the length is unchanged;
    `delay_us = 0` returns the input within 1e-12.
  - `analyze_reports_yaw_offset_and_balance_on_a_synthetic_head`: build an `Hrir` with FL FR FC SL SR BL BR (and
    TFL TFR) from one short band-limited pulse, each ear delayed by the Woodworth model for a radius of 9 cm
    with a yaw of +5°, plus a common offset of −20 µs (right ear earlier), the left ear scaled by 10^(−6.5/20)
    and the left ear's high band tilted (so `hf_asymmetry_db` is not 0). Assert: `input_balance_db` within
    0.2 dB of −6.5; `common_offset_us` within 5 µs of −20; `yaw_deg` within 1° of 5; `head_radius_cm` within
    1 cm of 9 (estimated from SL/SR); every pair's `sum_us` sign as the spec's convention predicts.
  - `symmetric_correction_makes_pairs_symmetric_and_centres_zero`: on that synthetic head with
    `ItdMode::Symmetric`, after `correct` + `measure_after` every pair has `after_L + after_R` within 3 µs of 0,
    FC's `after_us` within 3 µs of 0, no speaker skipped, and no ear was advanced (the first non-zero sample of
    every IR is at or after its original position).
  - `model_correction_targets_the_spherical_head`: with `ItdMode::Model` and `head_radius_cm = Some(9.0)`, every
    lateral speaker's `after_us` within 3 µs of `model_us`.
  - `corrections_beyond_the_limit_are_skipped`: a speaker whose target differs by more than 1500 µs is reported
    `skipped` and its data is unchanged.
  - `itd_off_reads_but_does_not_write`: `analyze` followed by `correct(.., Off)` leaves every sample bit-identical.
- `impulcifer-types`: `itd_config_defaults_and_validation` (defaults, both error messages, `from_kwargs`,
  `EXTENSION_FIELD_NAMES` contains both and has length 29).
- `impulcifer-cli`: `cli_itd_options_parse_and_validate` (both flags parse into kwargs, an invalid mode and a
  radius of 20 exit 2, both appear in `--help`).
- `impulcifer-service`
  - `brir_result_carries_the_alignment_report`: a BRIR job on `data/demo` (headphone compensation on, as the demo
    runs by default) returns `alignment` with `mode = "off"`, seven speakers in `SPEAKER_NAMES` order with
    `measured_us` and `after_us` numbers and `target_us = null`, `input_balance_db`, `headphone_balance_db` and
    `predicted_balance_db` numbers, and the log holds `cli_itd_report` and one of `cli_itd_pairing` /
    `cli_itd_pairing_warning`; the README has the title line.
  - `itd_symmetric_on_the_demo_makes_the_pairs_symmetric`: the same job with `itd_mode = "symmetric"` returns
    `after_us` with `FL + FR`, `SL + SR`, `BL + BR` within 3 µs of 0 and FC within 3 µs of 0, logs
    `cli_itd_corrected`, and `hesuvi.wav` differs from the `off` run (byte comparison of the two files).
  - `itd_off_output_is_byte_identical_to_today`: `hesuvi.wav` of an `off` run equals the file of a run of the
    same config made before your change. Do it by comparing against a copy you produce from the current
    `origin/master` CLI if one exists in `E:\Impulcifer\target\release\impulcifer.exe` (read-only use), else skip
    this test and say so in the report (brir-3x in CI is the gate).
  - `pairing_warning_when_the_inputs_were_swapped_between_the_recordings`: copy `data/demo` to a temp folder,
    scale channel 1 (left ear) of every speaker WAV by 10^(−6.5/20) and channel 2 (right ear) of `headphones.wav`
    by 10^(−7/20) (write the WAVs with the crate's own writer), run the job: `predicted_balance_db` within 0.5 dB
    of −13.5 and `cli_itd_pairing_warning` logged.
  - recording checks (`tests/recording_checks.rs`): `take_checks_report_the_input_balance` (a synthetic take whose
    left track is 6 dB lower: `lf_ild_db` within 0.5 dB of −6 on each measurement and one
    `record_check_input_balance` with `db_diff` ≈ −6.0; a balanced take has none);
    `headphone_take_checks_predict_the_final_balance` (headphone mode, FL/FR segments, right ear −7 dB, references
    carrying `lf_ild_db = −6.5`: `record_check_headphone_balance` with `predicted_db` ≈ −13.5; with references
    whose `lf_ild_db` is −7 (same sign as the headphone take) nothing is reported); every existing test in that
    file still passes unchanged.
  - the existing `ipc` tests: update any assertion on the exact key set of the `start_brir` result.
- `impulcifer-policy`: `cargo test -p impulcifer-policy` passes with the new `features.toml` entries.

## Allowed files

`crates/impulcifer-dsp/src/itd.rs` (new), `crates/impulcifer-dsp/src/lib.rs`, `crates/impulcifer-dsp/src/pipeline.rs`,
`crates/impulcifer-dsp/tests/` (new test file(s)), `crates/impulcifer-types/src/config.rs` and its `tests/`,
`crates/impulcifer-cli/src/options.rs`, `crates/impulcifer-cli/src/lib.rs`, `crates/impulcifer-cli/tests/cli.rs`,
`crates/impulcifer-service/src/{lib.rs, brir/run.rs, brir/outputs.rs, brir/validation.rs, brir/mod.rs,
recording/checks.rs, recording/request.rs, recording/run.rs}` (only what the spec needs),
`crates/impulcifer-service/tests/` (new and existing test files), `crates/impulcifer-service/locales/*.json` (only
the twelve keys above), `crates/impulcifer-policy/tests/gates.rs` (only `CANONICAL_CONFIG`), `features.toml`,
`crates/impulcifer-python/src/module.rs` only if it enumerates extension fields and breaks. Do not touch `apps/`,
`.github/`, `docs/`, `Cargo.toml`, `Cargo.lock`, the 2.x Python tree, `data/`, or any golden fixture under
`tests/migration/goldens`. New fields go in the extension lists, never in `FIELD_NAMES` / `OPTIONS` (the 2.x parity
tests compare those with the Python tree).

## Verification (foreground; report the tail of each)

Git Bash on Windows. The session shell sets a proxy that breaks the loopback mock servers, so prefix test runs
with `env -u HTTPS_PROXY -u HTTP_PROXY`.

```bash
cd /e/Impulcifer/.worktrees/itd
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
env -u HTTPS_PROXY -u HTTP_PROXY cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-cli
env -u HTTPS_PROXY -u HTTP_PROXY cargo test -p impulcifer-service
cargo test -p impulcifer-policy
```

The full service suite takes 20–30 minutes on this machine; run it once at the end, and the individual test
binaries (`--test recording_checks`, `--test ui_catalog`, `--test ipc`, your new BRIR test file) while you work.
Run every command in the foreground and end your turn only after reporting: the files changed, the public
signatures (the `itd` module API, the `StageObserver` hook, `PipelineOutputs`/`BrirRun` fields, the result JSON of
a demo run pasted verbatim), the test names, and the command outputs. Do not commit. Do not use a type alias or
any other trick to get past a policy gate; if a gate blocks the design, stop and report it.
