# P58: SOFA export through the pipeline, the service and the CLI (ADR 0019)

Repository: `/home/user/Impulcifer-pip313` (branch `ccr-083fe5e3-isoctv`). Work only inside it. Read first:
`CLAUDE.md` (3.x workspace section: `forbid(unsafe_code)`, `features.toml`, `EXTENSION_FIELD_NAMES` /
`EXTENSION_OPTIONS`, "3.x 문자열은 오버레이 카탈로그에 쓴다", the brir-3x rule that default outputs stay
byte-identical), `docs/adr/0019-sofa-export.md`, and `docs/rust/SOFA.md` (the specification: "Files", "Content",
"Headphone split", "Convention").

The SOFA writer already exists in `crates/impulcifer-io/src/sofa.rs` (`encode_simple_free_field_hrir`,
`write_simple_free_field_hrir`, `SofaMeasurement`, `SofaMetadata`, `sofa_date`). Use it; do not change it unless a
test here proves it wrong (then say exactly what and why).

Another worker is fixing `crates/impulcifer-io/src/sofa.rs` and `crates/impulcifer-io/tests/sofa.rs` at the same
time (the public API above stays the same). Do not edit those two files. If a build fails inside them, or
`cargo fmt --all -- --check` flags only them, wait a few minutes and run the command again; report it if it persists.

Build with the shared target directory, one cargo command at a time:
`export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0`. Do not delete anything in
that directory and do not create other target directories (the disk is small).

## 1. Settings and CLI

- `ProcessingConfig` (`crates/impulcifer-types/src/config.rs`): `sofa: bool` and `sofa_split_headphone_eq: bool`,
  both default `false`, listed in `EXTENSION_FIELD_NAMES`. `from_kwargs` and the service's request validation accept
  them like the other boolean extension fields; a non-boolean value is rejected with the usual message shape.
- CLI (`crates/impulcifer-cli/src/options.rs`, `EXTENSION_OPTIONS`): `--sofa` and `--sofa_split_headphone_eq` as
  `CliType::FlagTrue` (like `--jamesdsp`). Help texts:
  - `--sofa`: `Also save the BRIR as a SOFA file (brir.sofa, SimpleFreeFieldHRIR): the speakers of hrir.wav at their nominal directions, 2 m away, without LFE.`
  - `--sofa_split_headphone_eq`: `With --sofa, also save the BRIR without headphone compensation (brir-no-headphone-eq.sofa) and the headphone EQ as a two-track FIR (headphone-eq.wav). Needs headphone compensation.`
- `bootstrap`'s `brir_defaults` then carries both (check the IPC shape tests).

## 2. DSP (`crates/impulcifer-dsp`)

- **Channel balance:** split `correct_channel_balance` (`src/channel_balance.rs`) into
  `channel_balance_corrections(hrir, method) -> Result<Vec<ChannelBalanceCorrection>, DspError>` (per group: the
  speaker names and the two FIRs, computed exactly as now) and `apply_channel_balance(hrir, &corrections)`.
  `correct_channel_balance` becomes the two calls; its output bytes must not change.
- **Headphone EQ FIR:** in `src/stages/equalize.rs`, `headphone_eq_curve(hp, side, fs)` and
  `headphone_eq_fir(hp, side, fs) -> Result<Vec<f64>, DspError>`: the same steps as `equalization_curve` with only the
  headphone term (no room, no user EQ, a 0 dB target on the same grid), the same `smoothen_heavy_light` and the same
  `equalize` parameters, then `minimum_phase_impulse_response(fs, 5.0, false)`. Do not change `equalization_curve`.
- **Pipeline (`src/pipeline.rs`):** when `config.sofa && config.sofa_split_headphone_eq` and the Equalize stage has a
  headphone compensation in use (`config.do_headphone_compensation` and `headphone.is_some()`), keep a second HRIR
  `split: Option<Hrir>` from the Equalize stage on:
  - Equalize: `split` = the HRIR as it is just before `equalize_hrir`, equalized with the same `EqInputs` but
    `hp: None`. Also compute the two headphone EQ FIRs at `estimator.fs`.
  - Decay: `adjust_decay` on `split` with the same targets.
  - ChannelBalance: compute the corrections on the final HRIR, apply them to the final HRIR, apply the same
    corrections to `split`.
  - Normalize: `apply_gain_db(split, applied_gain_db)` (the final HRIR's gain).
  - Resample: when the final HRIR is resampled, resample `split` the same way and apply the final HRIR's second
    normalization gain to it; resample each headphone EQ FIR to the output rate with the same resampler as the HRIR
    (no gain change).
  - Every stage that does not run for the final HRIR does not run for `split` either. Nothing of this runs when the
    split is off, so default outputs stay byte-identical.
- **Outputs:** add to `PipelineOutputs`
  ```rust
  pub struct SofaSpeaker { pub slot: &'static str, pub azimuth: f64, pub elevation: f64, pub left: Vec<f64>, pub right: Vec<f64> }
  pub struct SofaSet { pub fs: u32, pub speakers: Vec<SofaSpeaker> }
  pub struct SofaOutputs { pub brir: SofaSet, pub split: Option<(SofaSet, [Vec<f64>; 2])>, pub split_skipped: bool }
  pub sofa: Option<SofaOutputs>, // Some only when config.sofa
  ```
  In WriteBrirs, `brir` comes from the HRIR that `fill_output` returns for the `hrir.wav` targets (the same call the
  `hrir.wav` tracks use), every speaker with both ears in `SLOTS` order, azimuth/elevation from `SLOTS`; `LFE`/`LFE2`
  never. `split` is built the same way from `split` (the same `fill_output` targets and mode) with the headphone EQ
  FIRs. `split_skipped` is true when the split was asked for but headphone compensation was not used.

## 3. Service (`crates/impulcifer-service`)

In `src/brir/outputs.rs::write_outputs_checked`, after `hrir.wav`/`hesuvi.wav`, when `outputs.sofa` is `Some`:
- Save `brir.sofa` with `write_simple_free_field_hrir` (distance 2.0 m), `SofaMetadata { title, comment, version:
  env!("CARGO_PKG_VERSION"), date }` with the titles and comments of `docs/rust/SOFA.md`. `date` = `sofa_date` of
  `SOURCE_DATE_EPOCH` when that environment variable parses as an integer, else of the current Unix time.
- With a split: `brir-no-headphone-eq.sofa` (its own title and comment) and `headphone-eq.wav` (`write_wav`, two
  tracks left/right, 32-bit like the other WAVs, output rate).
- Push every saved path to `files`. Log with these catalog keys. They already exist, translated, in all nine
  overlay catalogs `crates/impulcifer-service/locales/*.json`; use them with exactly these placeholders and do not
  add, change or move catalog entries:
  - `cli_success_sofa` ({count}, {path}): `Saved the BRIR as a SOFA file ({count} speakers): {path}`
  - `cli_success_sofa_split` ({path}, {eq}): `Saved the BRIR without headphone compensation as a SOFA file: {path}, and the headphone EQ: {eq}`
  - `cli_info_sofa_split_skipped` (no placeholder): `This job does not use headphone compensation, so brir-no-headphone-eq.sofa and headphone-eq.wav were not saved.`
- README (`render_readme_with_layouts`): when SOFA files were saved, a section titled by `cli_readme_sofa_title`
  (`SOFA`) with `cli_readme_sofa_note` (`brir.sofa holds the BRIRs of hrir.wav in the SOFA format (SimpleFreeFieldHRIR): one measurement per speaker at its nominal direction, 2 m away. LFE is not included.`) and, with a split, `cli_readme_sofa_split_note` (`brir-no-headphone-eq.sofa holds the same BRIRs without headphone compensation. Apply headphone-eq.wav (track 1 left ear, track 2 right ear) in the player to get brir.sofa.`). These three keys have no placeholders and also exist already.
- Check how the other `cli_success_*` / `cli_readme_*` keys are looked up and formatted, and do the same.

## 4. features.toml

`config.sofa`, `config.sofa_split_headphone_eq` (kind config), `output.sofa`, `output.sofa_split` (kind output), all
`implemented`, each with real tests from below.

## 5. Tests

- DSP: `channel_balance_split_matches_the_single_call` (corrections then apply == `correct_channel_balance`, bit for
  bit, for each method on the demo-like fixture the existing channel balance tests use);
  `headphone_eq_fir_is_minimum_phase_from_the_headphone_term_only`.
- Service (`crates/impulcifer-service/tests/sofa_outputs.rs`, using the demo the other BRIR tests use):
  - `sofa_holds_the_speakers_of_hrir_wav`: `sofa: true`; `brir.sofa` opens in `sofar` 0.4.0 (dev-dependency of the
    service too; `normalized(false)`); M equals the number of speakers with both ears in `hrir.wav`; positions follow
    `SLOTS`; each IR equals the matching `hrir.wav` track within f32 rounding; the job result lists the file.
  - `sofa_split_reassembles_the_final_brir`: `sofa` + split; both SOFA files and `headphone-eq.wav` exist; for every
    speaker and ear, `brir-no-headphone-eq` convolved with the matching headphone EQ track matches `brir.sofa` within
    ±1.0 dB on a 1/3-octave smoothed magnitude between 100 Hz and 12 kHz. If the demo does not meet that, report the
    measured worst deviation and where; do not widen the tolerance.
  - `sofa_split_without_headphone_compensation_saves_only_brir_sofa`: split asked, `do_headphone_compensation: false`;
    only `brir.sofa`; the log has `cli_info_sofa_split_skipped`.
  - `sofa_off_writes_no_sofa_file`: defaults; no `.sofa` and no `headphone-eq.wav`.
  - `sofa_files_are_reproducible_with_source_date_epoch`: two runs with the same `SOURCE_DATE_EPOCH` give identical
    `.sofa` bytes and `DateCreated` is that date (read the attribute through `sofar` if it exposes attributes; otherwise
    search the bytes for the date string).
- CLI: the two flags parse, appear in `--help`, and reach the config.
- Keep every existing test green (config parity tests compare `FIELD_NAMES`/`OPTIONS` with 2.x; the new fields belong
  to the extension lists only).

## Allowed files

`crates/impulcifer-types/src/config.rs` and its tests, `crates/impulcifer-cli/src/options.rs` and `tests/`,
`crates/impulcifer-dsp/src/{channel_balance.rs,pipeline.rs,stages/equalize.rs}` and `crates/impulcifer-dsp/tests/`,
`crates/impulcifer-service/src/brir/outputs.rs` (and the request validation file if booleans are validated there),
`crates/impulcifer-service/tests/`, `crates/impulcifer-service/Cargo.toml` (the `sofar` dev-dependency only),
`features.toml`, `Cargo.lock` (by cargo only). Not the catalogs in `crates/impulcifer-service/locales/`.
Do not touch `apps/`, `.github/`, docs, `crates/impulcifer-io` (unless a test proves the writer wrong), or versions.

## Verification (foreground; report the tail of each)

```bash
cd /home/user/Impulcifer-pip313
export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-cli
cargo test -p impulcifer-service --test sofa_outputs --test ui_catalog --test config_parity --test brir_outputs -- --skip output_readonly_file_failure_preserves_existing_bytes
cargo test -p impulcifer-service --lib
cargo test -p impulcifer-policy
```

(`output_readonly_file_failure_preserves_existing_bytes` fails only because this container runs as root.) Run every
command in the foreground and end your turn only after reporting the files changed, the test names, the measured
split deviation, and the command outputs. Do not commit. Do not start other agents. Do not use an alias or any other
trick to get past a policy gate.
