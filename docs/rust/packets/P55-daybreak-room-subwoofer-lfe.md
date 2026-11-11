# P55: the room measurement's subwoofer makes and corrects the LFE track (ADR 0017)

Repository root: the worktree you are started in. Read first:

- `CLAUDE.md` (3.x workspace section: `forbid(unsafe_code)`, no new dependencies, `features.toml`, brir-3x
  integrity, "DSP를 병렬화할 때는 map만").
- `docs/rust/LFE.md`. It is the specification, and every number in it is normative. This packet adds where the
  code goes and what to test. The Korean decision record is `docs/adr/0017-room-subwoofer-lfe.md`.
- What exists from 3.18 (P52/P53): `crates/impulcifer-dsp/src/pipeline.rs` (`PipelineInputs.lfe`, the LFE lines in
  `CropAndAlign`, `Equalize`, `Normalize`, `Resample`, `WriteBrirs`), `crates/impulcifer-dsp/src/stages/fill.rs`
  (`fill_output`, `measured_lfe`), `crates/impulcifer-service/src/brir/measurement_file.rs` (`load_room`,
  `load_brir`, `inspect`), `crates/impulcifer-service/src/brir/inputs.rs` (`load_inputs`, the room correction
  block, `open_measurements`), `crates/impulcifer-service/src/brir/outputs.rs` (`display_lfe_source`),
  `crates/impulcifer-io/src/external/assign.rs`.
- Room EQ v2, which the correction reuses: `crates/impulcifer-dsp/src/stages/room_v2.rs` (`smooth`, `median`,
  `snr_db`, `detect_rolloff`, `correction_gain`) and `docs/rust/ROOM_CORRECTION.md` ("Room correction v2").

## Invariant

With no `LFE`/`LFE2` in any measurement map, every output byte and every log line stays as it is (the brir-3x
workflow compares SHA-256 of all scenario outputs against the base commit). The output of jobs with an in-ear LFE
changes on purpose (timing and band limit); no existing brir-3x scenario has one.

## What to build

### 1. `impulcifer-io` (`crates/impulcifer-io/src/external/assign.rs`)

As "Room measurement assignment" in the spec: `LFE`/`LFE2` valid for `Purpose::Room` (room forms), automatic Dirac
and REW room assignment of subwoofers, `format_map_for` for room, canonical order unchanged. Keep the headphone
rejection and its message. `rew_subwoofer_title` requires an ear word today (BRIR); room needs the variant where a
missing ear word means both sides.

### 2. `impulcifer-dsp`

- New module `crates/impulcifer-dsp/src/stages/lfe.rs` (register it in `stages.rs`), file-free and sequential:

  ```rust
  pub const LFE_BAND_LIMIT_HZ: f64 = 120.0;
  pub fn onset_index(data: &[f64]) -> usize;
  pub struct LevelTransfer { pub left_db: f64, pub right_db: f64, pub speakers: usize, pub spread_db: f64 }
  /// None when fewer than 3 speakers took part.
  pub fn level_transfer(brir: &Hrir, room: &Hrir, mic: Option<&FrequencyResponse>)
      -> Result<Option<LevelTransfer>, DspError>;
  pub struct LfeCorrection { pub fr: FrequencyResponse, pub low: f64, pub high: f64, pub snr_available: bool }
  pub fn room_correction(ir: &ImpulseResponse, target: &FrequencyResponse, mic: Option<&FrequencyResponse>)
      -> Result<LfeCorrection, DspError>;
  pub fn upper_limit(frequency: &[f64], measured_db: &[f64]) -> f64;
  pub fn place_onsets(lfe: &mut Hrir, at: usize, head_ms: f64);
  pub fn band_limit(lfe: &mut Hrir);
  ```

  `spread_db` is the larger of the two ears' spreads. `room_v2::smooth` and `room_v2::median` are `pub(crate)`
  and usable here; do not change their behaviour.
- `crates/impulcifer-dsp/src/pipeline.rs`:
  - `#[derive(Clone, Debug, Default)] pub struct LfeInputs { pub pairs: Vec<SpeakerIrs>, pub labels: Vec<(String,
    String)>, pub correction: Option<RoomFrs> }`; `PipelineInputs.lfe` becomes `LfeInputs`. Update every
    constructor in the workspace (tests, benches, examples: `lfe: LfeInputs::default()` where they had
    `lfe: Vec::new()`). `PipelineOutputs.lfe` stays `Vec<SpeakerIrs>`.
  - Stages as the spec's table: remove `crop_heads_without_speaker_delay` from `CropAndAlign` and from `hrir.rs`;
    place the onsets after the alignment (take `t` from the final FL left), then `crop_tails_to`, then
    `band_limit`; pass `correction` as `room_frs` to the LFE `equalize_hrir`; pass the labels to `fill_output`.
- `crates/impulcifer-dsp/src/stages/fill.rs`: `fill_output` takes the LFE labels (`&[(String, String)]`) next to
  the pairs and reports the label of the pair it used (`measured` when a slot has no label). Synthetic sources
  (`FC`, `FL+FR`, `all`) are unchanged.

### 3. `impulcifer-service`

- `measurement_file.rs`:
  - `load_room` returns the LFE pairs apart: `(Hrir, Vec<SpeakerIrs>, Option<FrequencyResponse>, ExternalFormat)`.
    The speakers' `Hrir` must not contain them.
  - `inspect`: `slot` of a subwoofer speaker for `purpose == room` as for `brir`.
- `inputs.rs` (`load_inputs`):
  - In the room correction block keep the room LFE pairs, and when there are any, an uncropped clone of the room
    speakers made before any room correction function runs, plus the prepared calibration and the prepared room
    target. Compute `stages::lfe::room_correction` for each room LFE pair and ear there.
  - After the BRIR measurements are opened (both the `opened_hrir` case and the normal one), build `LfeInputs`
    per "Sources and priority": the in-ear pair, else the room pair scaled by `level_transfer` (computed once, only
    when a room-derived track is needed). Labels, correction entries (with `−calibration` for room-derived
    tracks) and the five log lines of the spec.
- `outputs.rs`: `display_lfe_source` maps `measured_corrected` and `room` to `cli_lfe_source_measured_corrected`
  and `cli_lfe_source_room`.
- Catalog: add the five `cli_room_lfe_*` keys and the two `cli_lfe_source_*` keys to all nine
  `crates/impulcifer-service/locales/*.json`, the English text in every file (a translator replaces the other eight
  next), the same placeholders everywhere:
  - `cli_room_lfe_created`: `{slot}: the LFE track is made from the room measurement's subwoofer, level matched on {count} speakers`
  - `cli_room_lfe_level_spread`: `{slot}: the level difference between the in-ear and the room measurements varies by {spread} dB across speakers; they may have been measured at different seats`
  - `cli_room_lfe_no_level`: `{slot}: the room measurement shares only {count} speakers with the BRIR measurements (3 are needed), so no LFE track is made from its subwoofer`
  - `cli_room_lfe_correction`: `{slot}: corrected with the room measurement's subwoofer, {low}–{high} Hz`
  - `cli_room_lfe_uncorrected`: `{slot}: the room measurement has no subwoofer for this LFE track, so it is not corrected`
  - `cli_lfe_source_measured_corrected`: `the measured subwoofer, corrected with the room measurement`
  - `cli_lfe_source_room`: `the room measurement's subwoofer`
  Do not change other keys (the screen worker owns `room_file_subwoofer`).

### 4. brir-3x scenarios (`.github/scripts/brir_3x_integrity.py`)

Two scenarios, first version `3.19.0`, with the REW fixture `crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat`
(check which ids it has with the existing tests; the maps below assume ids 1 and 2):

- `room_lfe_from_room_measurement`: `--room_measurement_file=<fixture>`,
  `--room_measurement_map=FL=1,FR=2,FC=1,LFE=2`, `--layout_files=9.1.4`.
- `room_lfe_corrects_measured_lfe`: `--brir_recording_file=<data/demo/FL,FR.wav>`,
  `--brir_recording_speakers=FL,LFE`, `--room_measurement_file=<fixture>`, `--room_measurement_map=FL=1,LFE=2`,
  `--layout_files=9.1.4`.

Run the script locally with the release CLI as both base and head for these two and `default` (it only checks
that they run and repeat; the CI compares against the base commit).

## Tests (must exist and pass; register them in `features.toml`)

Synthetic inputs only, no new binary fixtures (build Dirac projects with
`crates/impulcifer-service/tests/support/room_measurement.rs`).

- `impulcifer-io`: `lfe_targets_are_valid_for_room_and_brir_only` (replaces or extends
  `lfe_targets_are_brir_only_and_need_both_ears`: room forms parse, headphones reject, room format string),
  `room_automatic_assignment_puts_subwoofers_on_lfe` (Dirac with two subwoofers, REW titles with and without an
  ear word).
- `impulcifer-dsp` (`stages::lfe` and pipeline):
  - `lfe_onset_finds_the_start_of_the_first_lobe`: a delayed 4th-order low-passed impulse of either polarity in
    −60 dB noise; the onset lies within the first 10% rise, before the first peak.
  - `lfe_level_transfer_recovers_the_gain_between_microphones`: BRIR speakers equal to room speakers times a
    per-ear gain with small per-speaker deviations; `K` within 0.1 dB; 2 speakers give `None`; a spread above 3 dB
    is reported.
  - `lfe_room_correction_cuts_modes_within_the_subwoofer_band`: a subwoofer with a +10 dB, Q 5 peak at 50 Hz and a
    4th-order low-pass at 80 Hz: cut at 50 Hz within 1 dB of 10 dB, `hi` within 1/6 octave of 80 Hz, zero gain
    above `hi`, no gain above 6 dB, median of the gain over `[lo, hi/√2]` within 0.5 dB of 0; a full-band
    subwoofer gives `hi = 120`; a rolloff below 30 Hz gives no boost below it.
  - `lfe_band_limit_is_the_lr4_low_pass`: magnitude at 120 Hz within 0.2 dB of −6 dB, at 60 Hz within 0.2 dB of
    −0.5 dB, at 1 kHz below −70 dB.
  - `measured_lfe_takes_the_output_stages_only` (update): onset at FL left's first peak, band limit applied, the
    speakers sample-identical to the run without the LFE.
  - `room_lfe_correction_reaches_the_lfe_track`: a pipeline run where the correction entry has a −10 dB notch: the
    LFE track's magnitude shows it, the speakers are unchanged.
  - `measured_lfe_precedes_lfe_mode_and_fills_the_other_track` (update for labels): the label of the pair used is
    reported.
- `impulcifer-service`:
  - `room_measurement_subwoofer_makes_the_lfe_track`: a job with a synthetic Dirac room project (3 speakers and a
    subwoofer) and the demo BRIR, room correction on, `layout_files` with an LFE channel: the LFE tracks are not
    silent, `cli_room_lfe_created` and `cli_room_lfe_correction` are logged, the README shows the `room` source
    text, and hesuvi.wav is byte-identical to the same job without the subwoofer in the map.
  - `room_measurement_subwoofer_corrects_the_measured_lfe`: an in-ear LFE (single recording) plus a room LFE: label
    `measured_corrected`; without the room LFE: `cli_room_lfe_uncorrected`.
  - `room_lfe_needs_three_shared_speakers`: two shared speakers: `cli_room_lfe_no_level`, no LFE from the room.
  - Update `inspect_room_measurements_reports_a_dirac_project` (room purpose now reports the subwoofer's `LFE`).
- `features.toml`: new entry `input.room_lfe` (`kind = "input"`, `status = "implemented"`, the tests above);
  update `input.measured_lfe` if a test you renamed is listed there.

## Allowed files

`crates/impulcifer-io/src/external/assign.rs`, `crates/impulcifer-dsp/src/{stages.rs,stages/lfe.rs,stages/fill.rs,pipeline.rs,hrir.rs}`,
`crates/impulcifer-service/src/brir/{measurement_file,inputs,outputs,run}.rs`,
`crates/impulcifer-service/locales/*.json` (the seven keys only), tests, benches and examples of those crates,
`.github/scripts/brir_3x_integrity.py` (the two scenarios), `features.toml`. Do not touch `apps/`, `docs/`,
`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `README.md`, `CLAUDE.md`, the 2.x tree, or any golden fixture. No new
dependency.

## Verification (foreground; report the tail of each)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io -p impulcifer-dsp
cargo test -p impulcifer-service --lib
cargo test -p impulcifer-service --test brir_inputs --test brir_outputs --test brir_ipc --test immersive_layouts --test room_measurement_file --test room_v2 --test room_tuning2
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-policy
cargo build --release -p impulcifer-cli
```

Then a manual run on the maintainer's measurement. Inputs are read-only; write only into the output folder named
in the task message. Report the log lines of the LFE and the files written.

The shell sets `HTTPS_PROXY`; if a loopback test fails with CONNECT 400, rerun it with `env -u HTTPS_PROXY`. Run
every command in the foreground and end your turn only after reporting: the files changed, the public signatures
added or changed, the test names, the command outputs, the manual run, and anything you did differently from this
packet and why. Do not commit. Do not use a type alias or any other trick to get past a policy gate; if a gate or
a parity test blocks the design, stop and report it. Check an API you are not sure of in the source before using
it.
