# DB02 (Daybreak): applied correction for the headphones chart, audio-free Python wheel

Work in `E:/Impulcifer` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `E:/Impulcifer/CLAUDE.md` (section "3.x Rust 워크스페이스") and `E:/Impulcifer/docs/rust/ARCHITECTURE.md` first. Rules: `#![forbid(unsafe_code)]` everywhere, no new runtime dependencies, no shell subprocesses (`impulcifer-policy` gate), run every command in the foreground and never in the background, never terminate a process, never touch `%LOCALAPPDATA%/Impulcifer` or any `Update.exe`. Goldens decide: `cargo test -p impulcifer-dsp` and `cargo test -p impulcifer-service` must stay green and no WAV, README or golden series may change.

**A second worker (ASTRA, packet A02) edits `crates/impulcifer-plots/**` and `docs/rust/PLOTS.md` in the same tree at the same time.** Never open those for writing. Your service change fills `FrCurve.equalization`; drawing it is ASTRA's work. Until ASTRA lands, `plot_headphones` simply ignores the field, so nothing of yours depends on it. Do not touch `features.toml`, `CHANGELOG.md`, the workflows under `.github/`, the Python 2.x tree or `apps/`.

## Part 1. The correction the pipeline applied, handed to the headphones chart

Today `plots/headphones.png` is rendered once, from `crates/impulcifer-service/src/brir/inputs.rs` right after `headphone_compensation`, and shows the measurement and the target only. The equalize stage (`crates/impulcifer-dsp/src/stages/equalize.rs`) computes, per speaker and ear, the frequency response whose `equalization` field is the limited correction actually turned into the FIR, and throws that response away. The owner wants the chart to show the correction that was applied. Design (fixed):

### `impulcifer-dsp`

```rust
// stages/equalize.rs
/// One ear's applied correction: the response after `equalize` (fields `error`,
/// `error_smoothed`, `equalization` on the eq grid) that produced its FIR.
#[derive(Clone, Debug)]
pub struct AppliedEqualization {
    pub speaker: String,
    pub side: Side,
    pub curve: FrequencyResponse,
}
/// Everything `equalization_fir` did up to and including `fr.equalize(..)`.
pub fn equalization_curve(inputs: &EqInputs<'_>, speaker: &str, side: Side) -> Result<FrequencyResponse, DspError>;
/// Unchanged contract: `equalization_curve(..)?.minimum_phase_impulse_response(inputs.fs, 5.0, false)`.
pub fn equalization_fir(inputs: &EqInputs<'_>, speaker: &str, side: Side) -> Result<Vec<f64>, DspError>;
/// Same work as today; additionally returns the curves in task order (one per present ear).
pub fn equalize_hrir(hrir: &mut Hrir, inputs: &EqInputs<'_>) -> Result<Vec<AppliedEqualization>, DspError>;
```

The FIR bytes must not change: `equalization_curve` performs exactly the operations `equalization_fir` performs today, in the same order; `equalization_fir` calls it and appends the min-phase step. `equalize_hrir` keeps the rayon parallel map; collect the curves from the same closure results.

`pipeline.rs`: `StageObserver` gains one provided method

```rust
/// The corrections the equalize stage applied, one per speaker and ear, in
/// task order. Not called when the stage does not run.
fn on_equalized(&mut self, _applied: &[AppliedEqualization]) -> Result<(), DspError> { Ok(()) }
```

called by `run_pipeline` right after `StageKey::Equalize` finishes (after `equalize_hrir` returns, before the next stage). `PipelineOutputs` is unchanged.

### `impulcifer-service`

- `brir/plots.rs`: `pub fn headphones(path, left, right, applied: Option<(&FrequencyResponse, &FrequencyResponse)>)`. When `applied` is `Some((l, r))`, the display `FrCurve` for each ear gets `equalization = l.equalization.clone()` (resp. `r`). The eq grid and the headphone grid are the same `generate_frequencies(10, fs/2, 1.01)` grid (the equalize stage already requires equal `error` lengths); assert `l.frequency == left.frequency` with a `DspError::InvalidArgument("headphone and equalization grids differ")` rather than interpolating. Add a pure helper the tests can call:

```rust
/// The applied curves the headphones chart shows: the front-left speaker's left
/// ear and the front-right speaker's right ear (the measurements the headphone
/// compensation came from). When a front speaker is absent, the first speaker
/// that has that ear. `None` when no curve for that ear exists.
pub fn headphone_applied_curves(applied: &[AppliedEqualization]) -> Option<(FrequencyResponse, FrequencyResponse)>;
```

- `brir/inputs.rs`: the existing render at load time stays as it is (call with `None`), so a run that fails before equalization still has the chart.
- `brir/run.rs`: `Observer` gains `headphone: Option<(FrequencyResponse, FrequencyResponse)>`, cloned from `inputs.headphone` (`hp.left`, `hp.right`) before `run_pipeline` moves `inputs` (this is two small arrays, not the HRIR). Implement `on_equalized`: when `self.headphone` is `Some` and `headphone_applied_curves` returns `Some`, re-render `plots/headphones.png` through `plots::headphones(.., Some((&l, &r)))`; check cancellation first as `on_plot` does. When `headphone_applied_curves` is `None`, do nothing.

### Tests (write them; they are the registry evidence)

- `impulcifer-dsp` (`tests/`, next to the existing equalize tests): `equalize_curve_and_fir_agree_and_one_curve_per_ear` (the FIR from `equalization_fir` equals `equalization_curve(..)?.minimum_phase_impulse_response(fs, 5.0, false)` bit for bit; `equalize_hrir` returns exactly one entry per present ear in task order; every `curve.equalization` has the grid length and is finite); extend `pipeline_observer_reports_order_steps_and_totals` (or add `pipeline_observer_receives_the_applied_equalization`) so a test observer records `on_equalized` once, after the `Equalize` step and before the next step, with the ear count of the demo HRIR. The default-pipeline golden must stay untouched.
- `impulcifer-service` (`tests/brir_plots.rs`): `headphone_applied_curves_prefer_the_front_pair` (FL/FR chosen when present, first available ear otherwise, `None` when an ear is missing); `headphones_chart_receives_the_applied_correction` (run the demo BRIR with headphone compensation as the existing plot tests do, then assert through the observer path that `plots/headphones.png` was written after the equalize stage: the simplest honest check is to record the file's bytes after `load_inputs` rendered it and assert the bytes differ after the run, or call `plots::headphones` with `Some` on the demo curves and assert the returned/passed `FrCurve.equalization` is non-empty through the helper). `plot_files_match_python_default_run`, `plot_files_match_python_plot_run`, `plots_do_not_change_wavs`, `golden_headphones_series_match_python`, `golden_eq_series_match_python` and `demo_parity` stay green and unchanged.

## Part 2. Python wheels without a native audio backend (manylinux)

The Python module (`crates/impulcifer-python/src/module.rs`: `version`, `run`, `detect_sweep`, `generate_sweep_set`, `recover_brir_outputs`, `cli_main`) never records. It still links the audio backend because `impulcifer-service` depends on `impulcifer-audio-io`, which on Linux links ALSA through cpal; manylinux forbids `libasound.so.2`, so the Linux wheel is tagged `linux_x86_64` and cannot go to PyPI (`docs/rust/PACKAGING.md`, "Python wheel and sdist"). Make the backend a cargo feature and build every wheel without it. Design (fixed):

- `crates/impulcifer-audio-io/Cargo.toml`: `cpal` and the Windows `impulcifer-sys-win` become `optional = true`; `[features] default = ["native"]`, `native = ["dep:cpal", "dep:impulcifer-sys-win"]` (Cargo accepts `dep:` for a target-specific optional dependency).
- `crates/impulcifer-audio-io/src/lib.rs`: `pub mod cpal_backend` only under `#[cfg(all(feature = "native", not(windows)))]`; new always-compiled `pub mod null_backend` with `pub struct NullBackend;` implementing `AudioBackend`: `name()` returns `"none"`, `enumerate` returns an empty list, `selectable_share_modes` the default empty slice, `probe`/`open_output`/`open_input` return `AudioError::Backend("this build has no audio backend".into())`. `default_backend()` returns the WASAPI/cpal backend under `native` and `Box::new(NullBackend)` otherwise (no `CachedBackend` around it). `policy.rs`, `session.rs`, `cached_backend.rs`, the fake-backend tests and the bench are backend-agnostic; keep the bench under `native` if it needs the real backend.
- `crates/impulcifer-service/Cargo.toml`: `impulcifer-audio-io = { path = "../impulcifer-audio-io", default-features = false }`, `[features] default = ["native-audio"]`, `native-audio = ["impulcifer-audio-io/native"]`. No code in the service names cpal or WASAPI; `get_system_info.runtime.audio_backend` already reports `self.backend.name()`, so it becomes `"none"` in an audio-free build, and `bootstrap.capabilities.share_modes` is already empty for a backend without selectable modes. Check that nothing else in the service assumes a device exists.
- `crates/impulcifer-cli/Cargo.toml`: `impulcifer-service = { path = "../impulcifer-service", default-features = false }`, `[features] default = ["native-audio"]`, `native-audio = ["impulcifer-service/native-audio"]`. `console.rs` line 125 falls back to `"WASAPI"`/`"cpal"` when `runtime.audio_backend` is missing; leave it, the key is always present.
- `crates/impulcifer-python/Cargo.toml`: `impulcifer-service` and `impulcifer-cli` with `default-features = false` and a comment saying why (no recording entry point in the Python surface; manylinux forbids libasound). `apps/impulcifer-app/Cargo.toml` is untouched (defaults on).
- `cargo tree -p impulcifer-python --features python -e normal` must list neither `cpal` nor `impulcifer-sys-win`; paste the `grep` count (0) in the report. `cargo test --workspace` still builds the service with the native backend (feature unification across the workspace), which is intended.

### Tests

- `impulcifer-audio-io` (`tests/session_fake.rs` or a new `tests/null_backend.rs`): `null_backend_lists_nothing_and_refuses_to_open` (name `"none"`, empty enumeration, empty share modes, `open_output`/`open_input` errors with the message above). Compiled under both feature states.
- `impulcifer-service` (`tests/ipc.rs`, next to the existing empty-backend `list_audio_devices` test): `audio_free_service_answers_devices_and_recording_honestly` using `NullBackend` through `with_dependencies`: `list_audio_devices` returns the empty shape, `bootstrap.capabilities.share_modes` is `[]`, `get_system_info.runtime.audio_backend` is `"none"`, and `start_recording` with an otherwise valid request fails with the device error code the service uses for an unopenable device (say which in the report).
- `impulcifer-policy` (`tests/gates.rs`): `python_wheel_builds_without_a_native_audio_backend`: parse `crates/impulcifer-python/Cargo.toml` (toml is already a dev-dependency there or add it under `[dev-dependencies]` only) and assert that the `impulcifer-service` and `impulcifer-cli` dependencies carry `default-features = false` and that no feature of the python crate names `native-audio`.
- `crates/impulcifer-python/tests/test_native.py`: `test_extension_module_links_no_audio_library`: on Linux read `/proc/self/maps` after importing `impulcifer` and assert no line contains `libasound`; skip on other platforms with a reason. The wheel must be rebuilt for this (`python -m maturin build --release --features python -m crates/impulcifer-python/Cargo.toml`, then `python -m pytest crates/impulcifer-python/tests -q`); on this Windows machine the test skips, which is fine, the Linux run is CI's.

## Allowed files
`crates/impulcifer-dsp/src/stages/equalize.rs`, `crates/impulcifer-dsp/src/pipeline.rs`, `crates/impulcifer-dsp/tests/**`, `crates/impulcifer-service/src/brir/{plots.rs,inputs.rs,run.rs}`, `crates/impulcifer-service/tests/{brir_plots.rs,ipc.rs}`, `crates/impulcifer-service/Cargo.toml`, `crates/impulcifer-audio-io/{Cargo.toml,src/lib.rs,src/null_backend.rs,src/cpal_backend.rs,benches/perf.rs,tests/**}`, `crates/impulcifer-cli/Cargo.toml`, `crates/impulcifer-python/Cargo.toml`, `crates/impulcifer-python/tests/test_native.py`, `crates/impulcifer-policy/tests/gates.rs`, `crates/impulcifer-policy/Cargo.toml` (dev-dependency only), `Cargo.lock` (only what the feature change moves), `docs/rust/ARCHITECTURE.md` (one sentence each for the `on_equalized` hook and the `native`/`native-audio` features). Nothing else.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo clippy -p impulcifer-python --features python --no-deps -- -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-service
cargo test -p impulcifer-audio-io
cargo test -p impulcifer-audio-io --no-default-features
cargo test -p impulcifer-policy
cargo tree -p impulcifer-python --features python -e normal | Select-String -Pattern "cpal|impulcifer-sys-win" | Measure-Object
python -m maturin build --release --features python -m crates/impulcifer-python/Cargo.toml
python -m pytest crates/impulcifer-python/tests -q
git status --porcelain
```

## Report format
(1) the `AppliedEqualization` and `on_equalized` code as landed; (2) which observer path re-renders the chart and the proof that the FIR bytes did not change (golden test names that passed); (3) the feature table (crate, feature, default) and the `cargo tree` count; (4) the new test names, one line each on what they pin; (5) the pasted `test result:` lines, the maturin wheel file name and `git status --porcelain`; (6) anything undone. Do not end your turn before the commands complete.
