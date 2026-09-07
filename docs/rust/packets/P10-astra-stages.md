# P10 (ASTRA): `impulcifer-dsp::{stages, virtual_bass, mic_deviation, channel_balance}` — the 2.x stage helpers, file-free

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first, in this order: `E:/Impulcifer/docs/rust/survey/pipeline-object-model.md` (sections on `core/room_correction.py`, `core/virtual_bass.py`, `core/microphone_deviation_correction.py`, `core/pipeline_stages.py`, `core/pipeline.py` stage bodies, and `core/hrir.py` channel balance) and `E:/Impulcifer/docs/rust/survey/autoeq-frequency-response.md` (call-site arguments), then the 2.x files they cite, then the Rust modules you build on: `crates/impulcifer-dsp/src/{fr.rs, fr/, ir.rs, hrir.rs, decay.rs, estimator.rs, filters.rs, conv.rs, peaks.rs, fft.rs, windows.rs}` (P08 and P09 are landed; use their types and do not change them; if a needed capability is missing, add a new function in your own modules and say so), and the golden conventions in `tests/migration/README.md`, `README-fr.md`, `README-brir.md` and the exporters `export_goldens_fr.py`, `export_goldens_brir.py` (read only; you write a separate exporter).

Run every command in the foreground. Never use background execution. Another worker (PA02, performance audit) may be editing the existing primitive files under `crates/impulcifer-dsp/src/` and `crates/impulcifer-dsp/Cargo.toml`; do not touch those files. The modules `stages`, `virtual_bass`, `mic_deviation`, `channel_balance` are declared in `lib.rs` as stubs; replace their contents. Do not edit `lib.rs`, `features.toml`, `export_goldens.py`, `README.md`.

## Design rule
No file I/O in `src/`: every helper takes already-loaded objects (`Hrir`, `ImpulseResponse`, `FrequencyResponse`, sample arrays, CSV text) and returns data. File discovery, reading, writing and text rendering with i18n belong to the service packet (P11). Where 2.x writes a debug file in the middle of a helper (`room-responses.wav`, `headphone-responses.wav`), return the tracks to write instead (`Vec<Vec<f64>>` in hexadecagonal order via `Hrir::stack_tracks`).

## Scope

### `stages/room.rs` (`core/room_correction.py`)
- `pub struct RoomMeasurementName { pub speakers: Vec<String>, pub side: Option<Side> }` and `pub fn parse_room_measurement_name(file_name: &str) -> Option<RoomMeasurementName>` (regex `^room-([A-Z]{2,3}(,[A-Z]{2,3})*)(-(left|right))?\.wav$`, lines 36 to 75; the discovery over a directory listing is the service's job, but give it `pub fn discover_room_measurements(file_names: &[&str]) -> Vec<(String, RoomMeasurementName)>` on a listing).
- `pub struct RoomCorrectionOptions { pub fr_combination_method: FrCombination /* Average | Conservative */, pub specific_limit: f64, pub generic_limit: f64 }`.
- `pub fn calculate_specific_room_corrections(rir: &Hrir, target: &FrequencyResponse, mic_calibration: Option<&FrequencyResponse>, limit: f64) -> Result<RoomFrs, DspError>` (185 to 210, first-FR `reference_gain` rule, `IR_ROOM_SPL` zeros, `compensate(min_mean_error=false)`, `apply_correction_limit`), where `pub struct RoomFrs(pub Vec<(String, Side, FrequencyResponse)>)` in insertion order.
- `pub fn calculate_generic_room_correction(irs: &[ImpulseResponse], target: &FrequencyResponse, mic_calibration: Option<&FrequencyResponse>, method: FrCombination, limit: f64) -> Result<FrequencyResponse, DspError>` (231 to 292 with the conservative sign-agreement rule and the `smoothen` calls), `pub fn correction_limit_mask(frequency: &[f64], limit: f64) -> Vec<f64>` (295 to 302, the full Hann over the octave, reproduce literally), `pub fn apply_correction_limit(fr: &mut FrequencyResponse, limit: f64)`.
- `pub fn split_generic_room_recording(estimator: &SweepEstimator, fs: u32, tracks: &[Vec<f64>]) -> Result<Vec<ImpulseResponse>, DspError>` (350 to 386: `n_cols = round((len/fs - 2)/(duration + 2))`, the column arithmetic, `estimate`, `crop_head(1)`).
- `pub fn prepare_room_target(csv: Option<&FrequencyResponse>, fs: u32) -> Result<FrequencyResponse, DspError>` (431 to 442: `interpolate(1.01, 10, fs/2)` then `center(1000)` for a CSV, or a flat zero target without `center`), `pub fn prepare_mic_calibration(csv: &FrequencyResponse, fs: u32) -> Result<FrequencyResponse, DspError>` (450 to 461).
- `pub fn room_correction(rir: &mut Hrir, generic_irs: &[ImpulseResponse], target: &FrequencyResponse, mic_calibration: Option<&FrequencyResponse>, estimator: &SweepEstimator, options: &RoomCorrectionOptions) -> Result<Option<RoomCorrection>, DspError>` composing lines 78 to 182: `crop_head(1.0)` on every specific IR, `rir.crop_tails(estimator)`, the responses tracks (`stack_tracks(HEXADECAGONAL, false)`), specific corrections, generic fallback copies for every `SPEAKER_NAMES` entry that has no specific measurement; `pub struct RoomCorrection { pub frs: RoomFrs, pub responses_tracks: Vec<Vec<f64>> }`. `None` when there is neither a specific nor a generic measurement.

### `stages/headphone.rs` (`core/pipeline_stages.py:353-482`, DSP part only)
- `pub fn headphone_compensation(hp: &Hrir) -> Result<HeadphoneCompensation, DspError>`: `left = magnitude_to_frequency_response(FL left)`, `right = ... (FR right)`, `gain = left.center(Band(100, 10000))`, `right.raw += gain`, `compensate(zero, min_mean_error=false)` on both; returns `pub struct HeadphoneCompensation { pub left: FrequencyResponse, pub right: FrequencyResponse, pub responses_tracks: Vec<Vec<f64>> }`. The file resolution rules (`HEADPHONES_FALLBACK_FILENAMES`, directory probing) are a pure function too: `pub fn resolve_headphone_file(requested: Option<&str>, dir_listing: &[&str]) -> Option<String>` (369 to 418; document each branch).

### `stages/eq_files.rs` (`core/pipeline_stages.py:183-350`, plain CSV only)
- `pub fn eq_settings_file_names(base: &str) -> [String; 2]` (`.csv` then `.txt`).
- `pub fn looks_like_eqapo_config(text: &str) -> bool` (port `core/eqapo.py:168-189` exactly).
- `pub fn read_eq_settings_csv(name: &str, text: &str) -> Result<FrequencyResponse, DspError>`: `FrequencyResponse::parse_csv`, then the `error = -raw` fixup when `error` is empty and `raw` is not. EqualizerAPO configs return `DspError::InvalidArgument("EqualizerAPO configs are not supported yet (P12)")`; the service will surface that.
- `pub fn finalize_eq(left: Option<FrequencyResponse>, right: Option<FrequencyResponse>, fs: u32) -> Result<(Option<FrequencyResponse>, Option<FrequencyResponse>), DspError>`: the `interpolate(1.01, 10, fs/2, k=1)` calls (right only when distinct, lines 328 to 335) and the precedence rules of `equalization` (`eq`, `eq-left`, `eq-right`, 294 to 334) expressed as `pub fn select_eq_pair(eq: Option<FR>, eq_right: Option<FR>, left: Option<FR>, right: Option<FR>) -> (Option<FR>, Option<FR>)`.

### `stages/target.rs`
`pub fn create_target(fs: u32, bass_boost_gain: f64, bass_boost_fc: f64, bass_boost_q: f64, tilt: f64) -> FrequencyResponse` (`pipeline_stages.py:485-501`).

### `stages/equalize.rs` (`core/parallel_workers.py:69-131`, `core/pipeline.py:655-700`)
- `pub struct EqInputs<'a> { pub room_frs: Option<&'a RoomFrs>, pub hp: Option<&'a HeadphoneCompensation>, pub eq_left: Option<&'a FrequencyResponse>, pub eq_right: Option<&'a FrequencyResponse>, pub target: &'a FrequencyResponse, pub fs: u32 }`.
- `pub fn equalization_fir(inputs: &EqInputs, speaker: &str, side: Side) -> Result<Vec<f64>, DspError>`: the worker body verbatim (`FrequencyResponse::constant(name, common_freq, 0, 0)`, add the errors that exist, subtract `target.raw`, `smoothen_heavy_light`, `equalize(max_gain 40, treble 10000..fs/2)`, `minimum_phase_impulse_response(fs, 5.0, false)`).
- `pub fn equalize_hrir(hrir: &mut Hrir, inputs: &EqInputs) -> Result<(), DspError>`: compute every FIR (use `rayon` across `(speaker, side)`, the way 2.x uses a process pool), then `ir.equalize(&fir)`.

### `stages/decay.rs` (`core/pipeline.py:702-724`, `core/parallel_workers.py:24-39`)
`pub fn adjust_decay(hrir: &mut Hrir, targets: &[(String, f64)]) -> Result<(), DspError>` (seconds per speaker; only speakers present in the list; `rayon` optional).

### `channel_balance.rs` (`core/hrir.py:39-74, 674-818`)
- `pub enum ChannelBalance { Trend, Left, Right, Avg, Min, Mids, GainDb(f64) }` with `pub fn parse(s: &str) -> Result<Self, DspError>` (the numeric fallback and its error).
- `pub fn channel_balance_firs(left_fr: &mut FrequencyResponse, right_fr: &mut FrequencyResponse, method: ChannelBalance, fs: u32) -> Result<[Vec<f64>; 2], DspError>` (674 to 783, every branch with its exact parameters, `unit_impulse(int(round(fs*0.1)))`).
- `pub fn correct_channel_balance(hrir: &mut Hrir, method: ChannelBalance) -> Result<(), DspError>` (785 to 818: groups from `IPSILATERAL_PAIRS`, fully present groups only, mean of the group's data per ear, `magnitude_to_frequency_response`, then `equalize` per speaker).

### `virtual_bass.rs` (`core/virtual_bass.py`)
`pub struct VirtualBassOptions { pub crossover_freq: u32, pub head_ms: f64, pub hp_freq: f64, pub invert_polarity: Option<bool> }` and `pub fn apply_virtual_bass(hrir: &mut Hrir, options: &VirtualBassOptions) -> Result<(), DspError>` (82 to 198, the guards, the padding to the max length, the 4th-order sub high-pass, the duplicated 4th-order crossover pairs, the three RBJ shelves `(150, -1.5, 0.760)`, `(400, -3.0, 0.660)`, `(800, -3.5, 0.610)`, the global gain from `_mag_at` at the crossover, the per-speaker ITD/head delay and polarity; read lines 120 to 175 for the exact per-speaker loop). Keep `_delay_signal`, `_mag_at`, `_duplicate_sos`, `_rbj_high_shelf` as private helpers pinned by `tests/test_virtual_bass.py`-equivalent cases.

### `mic_deviation.rs` (`core/microphone_deviation_correction.py`)
`pub struct MicDeviationOptions { pub correction_strength: f64, pub max_correction_db: f64, pub smoothing_octave: f64, pub f_min: f64, pub f_max: f64, pub window_ms: f64, pub pre_ms: f64, pub anchor: Anchor /* Auto | Frontal | Diffuse */ }` with `Default` = the Python defaults; `pub struct MicMatching { .. }` mirroring `MicrophoneMatchingCorrector` (`new`, `windowed_power`, `collect_speaker`, `band_weight`, `estimate_interaural_mismatch`, `design_correction_filters`, `fir_from_curve`, `analysis_summary`), and `pub fn apply_mic_deviation_correction(hrir: &mut Hrir, options: &MicDeviationOptions) -> Result<MicDeviationSummary, DspError>` (387 to 454 including the `< 0.05 dB` skip).

### `stages/readme.rs` (`core/pipeline_stages.py:525-687`, numbers only)
`pub struct ReadmeRow { pub speaker: String, pub side: Side, pub pnr_db: f64, pub itd_us: f64, pub length_ms: Option<f64>, pub reverb: Option<(ReverbKind, f64)> }`, `pub struct ReadmeData { pub rows: Vec<ReadmeRow>, pub reverb_header: ReverbKind /* mode across rows or RTxx */, pub reflections: Vec<(String, Side, ReflectionLevels)>, pub applied_gain_db: f64, pub fs: u32 }`, `pub fn readme_data(hrir: &Hrir, fs: u32, applied_gain: f64) -> ReadmeData` (sorting by `SPEAKER_NAMES` index, the contralateral-ear ITD rule, `pnr = 20*log10(|data[peak]| + 1e-9) - noise_floor_db`, `length = (tail - peak)/fs*1000` when `tail > peak`, first available of RT60/RT30/RT20/EDT, the `Counter.most_common` header). Rendering (markdown table, i18n labels) is P11.

### `pipeline.rs` (in-memory driver, `core/pipeline.py:424-511, 593-609, 733-743, 859-895`)
- `pub struct PipelineInputs { pub estimator: SweepEstimator, pub hrir: Hrir, pub room: Option<RoomCorrection>, pub headphone: Option<HeadphoneCompensation>, pub eq_left: Option<FrequencyResponse>, pub eq_right: Option<FrequencyResponse> }` (the service assembles these from files with the helpers above).
- `pub struct StageProgress { pub key: StageKey, pub step: usize, pub total: usize }` and `pub trait StageObserver { fn on_stage(&mut self, progress: StageProgress); fn check_cancelled(&self) -> Result<(), DspError>; }`.
- `pub fn stage_table(config: &ProcessingConfig) -> Vec<(StageKey, bool, usize)>` reproducing `_stage_table` (the 26 rows, gates and step counts from the survey; `mic_deviation_active`/`_skipped` rules) and `pub fn total_steps(config) -> usize` (11 for the default config).
- `pub fn run_pipeline(config: &ProcessingConfig, inputs: PipelineInputs, observer: &mut dyn StageObserver) -> Result<PipelineOutputs, DspError>`: executes the DSP stages in table order on the in-memory objects (`target`, `crop_and_align`, `virtual_bass`, `mic_deviation`, `equalize`, `decay`, `channel_balance`, `normalize`, `resample` with the second normalize whose gain is discarded), skipping the stages that are file/plot work but still reporting their progress keys so the total matches 2.x; `pub struct PipelineOutputs { pub hrir: Hrir, pub applied_gain_db: f64, pub readme: ReadmeData, pub hrir_tracks: Vec<Vec<f64>>, pub hesuvi_tracks: Vec<Vec<f64>>, pub responses_tracks: Vec<Vec<f64>>, pub truehd: Vec<(String, Vec<String>, Vec<Vec<f64>>)>, pub jamesdsp: Option<Vec<Vec<f64>>>, pub hangloose: Vec<(String, Vec<Vec<f64>>)> }` (the track matrices exactly as `_stage_write_brirs`, `_stage_truehd_layouts`, `_stage_jamesdsp` (subset re-normalised), `_stage_hangloose` would write them). Cancellation is checked between stages through the observer.

### Golden exporter and tests
Write `E:/Impulcifer/tests/migration/export_goldens_stages.py` (standalone; may import the other exporters' helpers; must not modify them). Fixtures `tests/migration/goldens/p10_*` (+ `.f64`), documented in `tests/migration/README-stages.md`. Work on a temporary copy of `data/demo` for every run; `git status --short data/demo` must stay empty. The demo directory contains `FL,FR.wav`, `SR,BR.wav`, `BL,SL.wav`, `FC.wav`, `headphones.wav`, `room-*-left/right.wav`, `room-mic-calibration.txt`, so room correction, headphone compensation and the default EQ chain all run there.

Scenarios (each exports the intermediate objects it names; `.f64` for arrays above 4,096 values):
1. **default**: `ProcessingConfig` defaults on the demo copy. Export: `room_frs` (every speaker/side `error` and `target`), `room responses` tracks (first/last 256 + SHA per track), `hp` left/right `error`, `target.raw`, the 14 EQ FIRs (full `.f64`), and after each in-memory stage (`crop_and_align`, `equalize`, `normalize`) the per-track summaries used by P08 (`length, peak_index, argmax, max_abs, rms, first/last 256, SHA`) plus the full `FL` pair after `equalize`; the final `hrir_tracks`/`hesuvi_tracks` names and per-track summaries; `applied_gain`; `readme_data` numbers; `stage_table` rows and `total_steps`.
2. **vbass**: `vbass=True, vbass_freq=250` (the CI parity path). Export the tracks after `virtual_bass` (summaries + full `FL` and `FC` pairs) and the final tracks.
3. **channel balance**: for each of `trend, left, right, avg, min, mids, "3"` on the default-run HRIR after `equalize`: the two FIRs (full `.f64`) and the tracks after `correct_channel_balance` (summaries).
4. **mic deviation**: `microphone_deviation_correction=True, do_headphone_compensation=False`: the two FIRs, `mismatch_db`, `analysis_summary`, tracks after (summaries). Also `mic_deviation_skipped` with headphone compensation on (stage table only).
5. **decay**: `decay={"FL": 0.3, "FR": 0.3}` (the CLI form `--decay=FL:300,FR:300`): the adjustment params per side and the tracks after (summaries + full `FL`).
6. **resample**: `fs=44100`: final tracks (summaries + full `FL`), the second normalize gain (discarded in 2.x; record it anyway), `stage_table`.
7. **options**: `fr_combination_method="conservative"`, `specific_limit=0`, `generic_limit=0`, `target_level=-12`, `tilt=-1.0`, `bass_boost_gain=4` each as a separate small scenario exporting only the objects they change (`room_frs`, `target.raw`, `applied_gain`, final `FL` summaries).
8. **generic room**: build a synthetic `room.wav` (two tracks, `n_cols=1`, the demo `FL` sweep recording shifted) in the temp copy and export `calculate_generic_room_correction` for `average` and `conservative`, and `split_generic_room_recording` column boundaries.
9. **headphone file resolution**: `resolve_headphone_file` cases (explicit file, explicit directory with fallbacks, relative path, missing) as a JSON truth table computed by calling the Python resolution code on temp directories.
10. **eq files**: `eq.csv` plain gain curve with and without an `error` column, `eq-left`/`eq-right` precedence, an EqualizerAPO text (expect the P12 error in Rust; record only `looks_like_eqapo_config`).

Rust tests in `crates/impulcifer-dsp/tests/golden_stages.rs`: one test per scenario family (`golden_default_pipeline_matches_python`, `golden_vbass_matches_python`, `golden_channel_balance_matches_python`, `golden_mic_deviation_matches_python`, `golden_decay_matches_python`, `golden_resample_matches_python`, `golden_option_variants_match_python`, `golden_generic_room_matches_python`, `golden_headphone_resolution_matches_python`, `golden_eq_files_match_python`, `golden_stage_table_matches_python`) plus `properties_stages.rs` (`stage_table_total_is_eleven_by_default`, `correction_limit_mask_is_full_hann_over_one_octave`, `virtual_bass_guards_reject_crossover_at_nyquist`, `mic_deviation_skips_below_threshold`, `channel_balance_parse_accepts_numbers_and_rejects_words`, `readme_rows_sorted_by_speaker_names`). Tolerances: FR dB arrays `atol 1e-9, rtol 1e-11`; sosfilt-based virtual bass `atol 1e-9 * max_abs`; anything that passes through `minimum_phase_impulse_response` (EQ FIRs, channel-balance FIRs, mic-deviation FIRs, and every track after `equalize`) uses the oracle-noise budget of `tests/migration/README-fr.md` (taps `1e-3 * peak`; band-wise spectrum 2e-2 / 5e-2 / 0.3 / 20 dB) and, for tracks, `atol 1e-3 * max_abs` per sample plus exact `length`/`peak_index`; integer decisions exact; `applied_gain` `atol 1e-6` (it depends on the EQ FIRs); readme numbers `atol 1e-6` dB/ms, ITD exact; stage tables exact. Read the recordings with `impulcifer-io::read_wav` (already a dev-dependency after P08).

Do not edit `features.toml`; report the test names for `stage.*` entries (the 26 stage keys are pre-registered as planned: mark which ones your work covers) and any new ids you propose.

## Allowed files
`E:/Impulcifer/crates/impulcifer-dsp/src/stages.rs`, `src/stages/**`, `src/virtual_bass.rs`, `src/mic_deviation.rs`, `src/channel_balance.rs`, `src/pipeline.rs`, `E:/Impulcifer/crates/impulcifer-dsp/tests/golden_stages.rs`, `tests/properties_stages.rs`, `E:/Impulcifer/tests/migration/export_goldens_stages.py`, `E:/Impulcifer/tests/migration/goldens/p10_*`, `E:/Impulcifer/tests/migration/README-stages.md`. Nothing else.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; f64 only; no file I/O in `src/`.
- Reproduce 2.x behaviour and quirks (44100 Hz shelf, the full Hann limit mask, the `i // 2` ingestion rule already in P08, the second normalize's discarded gain, `mids` using the caller grid, the `frontal` anchor falling back to diffuse). Do not "improve"; list every quirk in the report.
- Every function has a doc comment naming the Python function, line range and fixture.
- New fixture bytes under 25 MB total; `.f64` for anything above 200 kB.

## Verification (foreground, paste output)
```
py -3.14 E:/Impulcifer/tests/migration/export_goldens_stages.py
git -C E:/Impulcifer status --short data/demo
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --test golden_stages --test properties_stages
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table scenario → fixture → tolerance → max observed error; (4) quirks reproduced and anything you could not reproduce; (5) test names per stage key and proposed ids; (6) anything undone. Do not end your turn before the commands complete.
