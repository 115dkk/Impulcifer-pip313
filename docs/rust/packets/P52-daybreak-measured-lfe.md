# P52: a measured subwoofer becomes the LFE channel (ADR 0016, items 5–9)

Repository root: the worktree you are started in. Read first:

- `CLAUDE.md` (3.x workspace section: `forbid(unsafe_code)`, no new dependencies, `features.toml`, brir-3x
  integrity, "DSP를 병렬화할 때는 map만").
- `docs/rust/SINGLE_RECORDING.md`, section "Measured LFE". It is the specification; this packet adds where the
  code goes. The Korean decision record is `docs/adr/0016-single-recording-file-and-measured-lfe.md`.
- `docs/adr/0013-missing-speakers-and-lfe-channel.md` and `crates/impulcifer-dsp/src/stages/fill.rs` (how the
  LFE tracks are filled today).

P53 (the single recording file) builds on this packet. Do not start it here.

## Invariant

With no `LFE`/`LFE2` assignment anywhere, every output byte and every log line stays as it is. The brir-3x
workflow compares SHA-256 of all scenario outputs against the base commit.

## What to build

### 1. `impulcifer-io`: assignment (`crates/impulcifer-io/src/external/assign.rs`)

- `LFE` and `LFE2` are valid targets for `Purpose::Brir` only. `parse_map_for(.., Purpose::Brir)` accepts
  `LFE.left=ID`, `LFE.right=ID` (and `LFE2`). A bare `LFE=ID` is rejected the way a bare slot is for BRIR today.
  `Purpose::Room` and `Purpose::Headphones` reject them with a message that names the slot and the purpose
  (follow the style of the existing contract messages; add the exact text to the tests).
- `check_assignments(.., Purpose::Brir)`: `LFE`/`LFE2` need both ears like every slot; they do not satisfy the FL
  requirement and do not count as a speaker.
- `Assignment.slot` stays `&'static str`; use the constants `"LFE"` and `"LFE2"`.
- Automatic assignment (`auto_dirac_brir`, `auto_rew_brir`): as in the specification. Dirac: subwoofer-flagged
  speakers, in `file.speakers` order, take `LFE` then `LFE2` when both ear positions have a usable measurement.
  REW: a title with a subwoofer token (`is_subwoofer_name`) and an ear word goes to `LFE`, or `LFE2` when the
  token ends in `2`; a slot/ear already taken is skipped as for speakers. Room and headphone auto assignment are
  unchanged.
- `canonical` / `format_map_for`: slots in `SLOTS` order, then `LFE`, then `LFE2`.

### 2. `impulcifer-dsp`

- `crates/impulcifer-dsp/src/pipeline.rs`: `PipelineInputs` gains `pub lfe: Vec<SpeakerIrs>` (speakers named
  `LFE`/`LFE2`, both ears present; empty by default) and `PipelineOutputs` gains `pub lfe: Vec<SpeakerIrs>` (as
  written). Every existing constructor of `PipelineInputs` in the workspace (tests, benches, examples) gets
  `lfe: Vec::new()`.
- Stage handling exactly as the table in the specification. Notes:
  - Head crop: `Hrir::crop_heads` looks the speaker delay up in `SPEAKER_NAMES` and fails for other names. Do not
    add `LFE` to `SPEAKER_NAMES`. Crop the LFE pairs with delay 0 through a small helper that shares the code
    (same start rule, same Hann fade-in), so speakers keep their exact arithmetic.
  - Tail crop: `Hrir::crop_tails` returns the tail length. Bring the LFE responses to that length (truncate, or
    zero-pad a shorter one) and apply the same half-Hann fade-out (`half` from the estimator as in `crop_tails`).
  - Equalisation: call `equalize_hrir` on an `Hrir` holding only the LFE pairs with `room_frs: None` and the
    same `hp`, `eq_left`, `eq_right`, `target`, `fs`. Check `equalization_curve` for a speaker-name lookup that
    would fail for `LFE` and handle it without changing the speakers' results. Do not pass the LFE result to
    `observer.on_equalized`.
  - Normalisation: `normalize` returns the gain in dB; multiply every LFE sample by `10^(gain/20)`. After
    resampling the code normalises a second time and discards the gain: use it for the LFE as well.
  - At `WriteBrirs` the LFE responses must have the speakers' length. If a stage left them different, truncate or
    zero-pad to the speakers' length before stacking.
- `crates/impulcifer-dsp/src/stages/fill.rs`: `fill_output` takes the measured LFE pairs (`&[SpeakerIrs]`). For
  each target `LFE`/`LFE2`: the measured pair of that name; else the other measured pair; else the present
  `lfe_mode` synthesis. A measured source is reported as `("LFE", "measured")` in `filled`. The `all` synthesis
  must keep averaging the speakers only.
- Only the layout files receive LFE targets today (`hrir.wav`/`hesuvi.wav` pass `"none"` and filter LFE out of
  their targets); keep that.

### 3. `impulcifer-service`

- `crates/impulcifer-service/src/brir/measurement_file.rs` `load_brir`: return the LFE pairs separately (they
  are on the same time axis and get the same padding and common length as the speakers). The slot loop iterates
  `SLOTS`; add the two LFE names after it into the separate list. `log_itd` and the `count` of
  `cli_brir_measurement_file_loaded` cover speakers only; when LFE pairs were read, log info
  `cli_brir_measurement_file_lfe` with `{"list": "LFE, LFE2"}`.
- `inspect` (same file): for `purpose == brir`, a subwoofer speaker's `slot` is the automatic `LFE`/`LFE2`
  (it is `null` today); for the other purposes it stays `null`. Nothing else in the response changes shape.
- `crates/impulcifer-service/src/brir/inputs.rs`: `open_measurements` returns the LFE pairs with the HRIR and
  `load_inputs` puts them into `PipelineInputs.lfe`. The virtual-bass plan and the "No HRIR recordings" check use
  the speakers only.
- `crates/impulcifer-service/src/brir/outputs.rs`: the existing LFE lines (`cli_info_lfe_filled`,
  `cli_readme_lfe_filled`) render the source `measured` with the catalog text `cli_lfe_source_measured`, the way
  `all` uses `cli_lfe_source_all`.
- New catalog keys in all nine `crates/impulcifer-service/locales/*.json`, English text in every file (a
  translator replaces the other eight later), same placeholders everywhere:
  - `cli_lfe_source_measured`: `the measured subwoofer`
  - `cli_brir_measurement_file_lfe`: `Measured subwoofer channels: {list}`

## Tests (must exist and pass; register them in `features.toml`)

- `impulcifer-io`: `lfe_targets_are_brir_only_and_need_both_ears` (parse, reject for room/headphones, one ear
  missing, canonical order), `brir_automatic_assignment_puts_subwoofers_on_lfe` (Dirac with two subwoofers and a
  REW title pair).
- `impulcifer-dsp`: `measured_lfe_takes_the_output_stages_only` (a synthetic HRIR plus an LFE pair through
  `run_pipeline`: the speakers' tracks are sample-identical to the run without the LFE; the LFE in the layout
  file has the speakers' length, the speakers' gain, and the headphone term when one is given),
  `measured_lfe_precedes_lfe_mode_and_fills_the_other_track` (`fill_output` with one and with two measured
  pairs, against `center` and `all`), and extend `defaults_leave_every_output_unchanged` to an empty `lfe`.
- `impulcifer-service`: `brir_measurement_file_reads_subwoofers_as_lfe` (a job from a measurement fixture whose
  map assigns `LFE`: the layout file's LFE tracks are not silent, hesuvi.wav is byte-identical to the same job
  without the LFE assignment, the log has `cli_brir_measurement_file_lfe` and `cli_info_lfe_filled` with the
  measured source text, the README names it). Build the fixture the way the existing measurement-file tests do
  (see `crates/impulcifer-service/tests/support/room_measurement.rs`); do not add binary fixtures.
- `features.toml`: new entry `input.measured_lfe` (`status = "implemented"`, the test paths above) next to
  `output.lfe_mode`; follow the neighbouring entries for the fields.

## Allowed files

`crates/impulcifer-io/src/external/assign.rs` (+ its tests), `crates/impulcifer-dsp/src/pipeline.rs`,
`crates/impulcifer-dsp/src/hrir.rs` (the crop helpers only), `crates/impulcifer-dsp/src/stages/fill.rs`,
`crates/impulcifer-dsp/src/stages/equalize.rs` (only if the name lookup needs it),
`crates/impulcifer-service/src/brir/{measurement_file,inputs,outputs,run}.rs`,
`crates/impulcifer-service/locales/*.json` (the two keys only), tests, benches and examples of those crates (to
add `lfe: Vec::new()` or the new tests), `features.toml`. Do not touch `apps/`, `.github/`, `docs/`, `Cargo.toml`,
`Cargo.lock`, the 2.x tree (`core/`, `gui/`, `i18n/`, `webview_ui/`), `crates/impulcifer-types/src/layouts.rs`,
`constants.rs`, or any golden fixture.

## Verification (foreground; report the tail of each)

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io -p impulcifer-dsp
cargo test -p impulcifer-service --lib
cargo test -p impulcifer-service --test brir_inputs --test brir_outputs --test brir_ipc --test immersive_layouts --test room_measurement_file
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-policy
```

The shell sets `HTTPS_PROXY`; if a loopback test fails with CONNECT 400, rerun it with `env -u HTTPS_PROXY`. Run
every command in the foreground and end your turn only after reporting: the files changed, the public signatures
you added or changed, the test names, and the command outputs. Do not commit. Do not use a type alias or any
other trick to get past a policy gate; if a gate or a parity test blocks the design, stop and report it. Check an
API you are not sure of in the source before using it.
