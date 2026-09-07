# P09 (ASTRA): `impulcifer-dsp::fr` — the AutoEQ `FrequencyResponse` subset used by the 2.x core

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first, in this order: `E:/Impulcifer/docs/rust/survey/autoeq-frequency-response.md` (the design input: every method, its algorithm, call-site arguments and the quirks that must be reproduced), `E:/Impulcifer/autoeq/frequency_response.py` and `E:/Impulcifer/autoeq/biquad.py` (the oracle; read the exact lines the survey cites), the finished primitives in `E:/Impulcifer/crates/impulcifer-dsp/src/{interp,smoothing,fir,filters,stats,fft,windows}.rs` (use them; do not reimplement), and the golden conventions in `E:/Impulcifer/tests/migration/README.md` and `E:/Impulcifer/tests/migration/export_goldens.py` (read only; you write a separate exporter, see below).

Run every command in the foreground. Never use background execution.

Another worker (P06) is editing `crates/impulcifer-dsp/src/{resample,spectrogram}.rs`, `tests/migration/export_goldens.py`, `tests/migration/README.md` and `features.toml` at the same time. You must not touch those files. The module `fr` is already declared in `crates/impulcifer-dsp/src/lib.rs` (a stub file `fr.rs` exists); replace the stub's content. The `regex` crate is available as a dependency of `impulcifer-dsp`.

## Scope: `crates/impulcifer-dsp/src/fr.rs` (split into `fr/` submodules if it passes 900 lines)

### Data model
```rust
#[derive(Clone, Debug, PartialEq)]
pub struct FrequencyResponse {
    pub name: String,
    pub frequency: Vec<f64>,
    pub raw: Vec<f64>,
    pub smoothed: Vec<f64>,
    pub error: Vec<f64>,
    pub error_smoothed: Vec<f64>,
    pub equalization: Vec<f64>,
    pub equalized_raw: Vec<f64>,
    pub equalized_smoothed: Vec<f64>,
    pub target: Vec<f64>,
}
```
`parametric_eq` and `fixed_band_eq` are omitted (never set by the app; the `reset` flags for them become no-ops). An empty `Vec` is Python's empty array. Provide:
- `pub fn new(name: &str, frequency: Option<Vec<f64>>, raw: Option<Vec<f64>>) -> Result<Self, DspError>`: `name` must be non-empty after trimming; `frequency` defaults to `generate_frequencies(20.0, 20000.0, 1.01)`; `raw` may be `None`; then `sort()`.
- `pub fn with_fields(name, frequency, FrFields { raw, smoothed, error, error_smoothed, equalization, equalized_raw, equalized_smoothed, target }) -> Result<Self, DspError>` (all `Option<Vec<f64>>`) for the constructions the app performs with `error=`/`target=`; also `pub fn constant(name, frequency, raw_value: f64, error_value: f64)` for the `raw=0, error=0` scalar construction (`np.ones(shape) * value`).
- `fn sort(&mut self) -> Result<(), DspError>`: argsort of `frequency` (stable sort is fine because duplicates are an error), reorder every non-empty array with the same permutation, `DspError::InvalidArgument` naming the first duplicate frequency. Arrays whose length differs from `frequency` are an `InvalidArgument` (Python would raise `IndexError`).
- `pub fn reset(&mut self, ResetFlags)` with a `ResetFlags` struct whose `Default` matches Python (`raw: false`, everything else `true`); document the five internal callers' effective clear sets from the survey.

### Static helpers
- `pub fn generate_frequencies(f_min: f64, f_max: f64, f_step: f64) -> Vec<f64>`: the repeated-multiplication loop, exactly (`let mut f = f_min; while f <= f_max { out.push(f); f *= f_step; }`). Pin the point counts 783 / 695 / 713 for the three app grids at fs 48000 and the last values against the golden.
- `pub fn window_size(frequency: &[f64], octaves: f64) -> usize`: `_window_size` with the naive left-to-right `sum(steps)/len(steps)` and Python's half-to-even `round` (implement `round_half_even(x: f64) -> i64` and test it on 23.5, 24.5, -0.5, 2.5). Compare with `smoothing::fractional_octave_window` in a doc comment (the P05 helper takes the ratio; this one derives it from the grid) and pin equality on the 1.01 grid for 1/12, 1/6, 1/5, 1/3, 1.3, 2 octaves (7, 13, 15, 23, 91, 139).
- `pub fn sigmoid(frequency: &[f64], f_lower: f64, f_upper: f64, a_normal: f64, a_treble: f64) -> Vec<f64>`: `_sigmoid` using `stats::expit`.
- `pub fn tilt(frequency: &[f64], tilt: f64) -> Vec<f64>`: `_tilt` with the hard-coded `c = 20 * sqrt(1000)`.
- `pub fn create_target(frequency: &[f64], bass_boost_gain: f64, bass_boost_fc: f64, bass_boost_q: f64, tilt: Option<f64>) -> Vec<f64>`: `biquad.digital_coeffs(frequency, 44100, *biquad.low_shelf(fc, q, gain, 44100))` plus the tilt term. Use `filters::rbj_low_shelf` and `filters::biquad_response_db` if they reproduce `autoeq.biquad` bit-for-bit (P03's `golden_rbj_matches_autoeq` says they do; verify against your own golden at gain 0, -6, +4 dB with Q 0.76 and 0.71); otherwise write a private `autoeq_low_shelf_db` that transcribes `biquad.py:52-79` and `112-131` and say so in the report. The design rate is always 44100 regardless of the project rate; document why (2.x quirk, do not fix).

### Methods (each mirrors the Python method of the same name; cite the line range in the doc comment)
- `pub fn interpolate(&mut self, f: Option<&[f64]>, f_step: f64, pol_order: usize, f_min: f64, f_max: f64) -> Result<(), DspError>`: NaN pruning of `raw`/`frequency` only; k = `pol_order` splines on `log10(frequency)` for the seven keys (`raw, error, error_smoothed, equalization, equalized_raw, equalized_smoothed, target`; `smoothed` is dropped); new grid; the `0 → 0.001` fix and restore; `ext=0` extrapolation via `interp::Spline`; then `reset` of `smoothed`. Provide `pub fn interpolate_default(&mut self)` = `interpolate(None, 1.01, 1, 20.0, 20000.0)`.
- `pub enum CenterAt { Frequency(f64), Band(f64, f64) }` and `pub fn center(&mut self, at: CenterAt) -> Result<f64, DspError>`: build the `equal_energy` copy, `interpolate_default()` on it, band mean or k=1 spline value, apply the signed shifts to `raw`/`smoothed`/`error`/`error_smoothed`, `reset(raw=false, smoothed=false, error=false, error_smoothed=false, target=false)`, return `-diff`. Also `pub fn center_value(&self, band: (f64, f64)) -> f64` = `core/hrir.py::get_center_value` for a band (mean of `raw` on the object's own grid, returns `-diff`, no re-grid) and `pub fn center_value_at(&self, frequency: f64) -> Result<f64, DspError>` (k=1 spline).
- `pub struct CompensateOptions { pub bass_boost_gain: f64, pub bass_boost_fc: f64, pub bass_boost_q: f64, pub tilt: Option<f64>, pub min_mean_error: bool }` with `Default` = `(0.0, 105.0, 0.71, None, false)` and `pub fn compensate(&mut self, compensation: &FrequencyResponse, options: &CompensateOptions) -> Result<(), DspError>`: copy + `center(CenterAt::Frequency(1000.0))` of the compensation curve, `target = compensation.raw + create_target(self.frequency, ...)`, `error = raw - target`, the 100 Hz to 10 kHz mean recentring when `min_mean_error`, then the `compensate` reset set. `InvalidArgument` when the grids differ in length (Python would broadcast-fail). No `sound_signature`.
- `pub struct SmoothingParams { pub window_size: f64, pub iterations: usize, pub treble_window_size: f64, pub treble_iterations: usize, pub treble_f_lower: f64, pub treble_f_upper: f64 }` and `fn smoothen_fractional_octave_data(&self, data: &[f64], p: &SmoothingParams) -> Result<Vec<f64>, DspError>` (the private `_smoothen_fractional_octave`: NaN gate, normal pass, treble pass, sigmoid blend, using `smoothing::savgol_filter(window, 2)`), `pub fn smoothen_fractional_octave(&mut self, p: &SmoothingParams) -> Result<(), DspError>` (`treble_f_upper <= treble_f_lower` is an error; `smoothed` from `raw`; `error_smoothed` from `error` when present; reset), `pub fn smoothen(&mut self, window_size: f64, treble_window_size: f64, treble_f_lower: f64, treble_f_upper: f64) -> Result<(), DspError>` (iterations 1), and `pub fn smoothen_heavy_light(&mut self) -> Result<(), DspError>` with the four hard-coded calls and the signed element-wise max.
- `pub struct EqualizeParams { pub max_gain: f64, pub smoothen: bool, pub treble_f_lower: f64, pub treble_f_upper: f64, pub treble_max_gain: f64, pub treble_gain_k: f64 }` (`Default` = `6.0, true, 6000.0, 8000.0, 6.0, 1.0`) and `pub fn equalize(&mut self, p: &EqualizeParams) -> Result<(), DspError>`: error source selection, NaN gate, the sigmoid `max_gain`/`gain_k` arrays, `gain = -error * gain_k`, `clipped`, `kink_inds` (edges of `clipped`, index 0 dropped), `np.where`, the doomed-index window of `window_size(1/12)` with the two rescued tail samples, the k=2 re-spline over the kept points, then `equalized_raw`/`equalized_smoothed`. Reproduce the "no re-clamp after the re-spline" behaviour and the "equalized_smoothed is not cleared at entry" quirk (document both).
- `pub fn minimum_phase_impulse_response(&self, fs: u32, f_res: f64, normalize: bool) -> Result<Vec<f64>, DspError>`: the exact sequence of `frequency_response.py:637-681` (`f_res /= 2`, `f_min = max(frequency[0], f_res)`, k=1 spline value at `f_min`, `n = round(fs // 2 / f_res)` then `fft::next_fast_len_legacy` (2/3/5, the `scipy.fftpack` variant), `linspace(0, fs // 2, n)`, `interpolate(Some(f), 1.01, 1, ...)` on a copy whose `raw` is `self.equalization`, the flat extension below `f_min`, optional normalisation, `*= 2`, dB to linear, `raw[n-1] = 0`, `fir::firwin2(2n, f, gain, None, fs)`, `fir::minimum_phase(ir, len(ir), true)`). Integer floor division for `fs // 2`. Pin the output lengths 9600 (`f_res=5`) and 4800 (`f_res=10`) at fs 48000.
- `pub fn read_from_csv(path: &Path) -> Result<Self, DspError>` and `pub fn parse_csv(name: &str, text: &str) -> Result<Self, DspError>`: both branches with the regex quirks (`float_pattern = -?\d+\.?\d+` requiring two digit characters; the strict AutoEq header at position 0 with only known columns and `\n` line anchors; the guess branch taking the first two floats of every matching line). Use the `regex` crate with the same patterns. Name derivation from the file name (everything before the last dot). Missing columns become empty arrays; the `error = -raw` fixup is the caller's job (do not do it here).
- `pub fn magnitude_to_frequency_response(name, fs: u32, data: &[f64]) -> Result<Self, DspError>`: `core/impulse_response.py:157-188` (`fft::magnitude_response`, the guards that return zeros on `generate_frequencies(10, fs/2, 1.01)`, the decimation `step = max(1, round(len(f) / ((fs/2)/4.0)))` starting at index 1, then `interpolate(None, 1.01, 1, 10.0, fs/2)`).

### Golden exporter and tests
Write `E:/Impulcifer/tests/migration/export_goldens_fr.py` (a standalone script; it may `import export_goldens` for the shared helpers and the output directory but must not modify that file). Fixtures `tests/migration/goldens/p09_*.json` (and `.f64` for arrays above 4,096 values, same binary format as P05; document in `tests/migration/README-fr.md`, a new file). Freeze the seed and record the environment like P05 does. Fixtures:
- `generate_frequencies` for the three app grids and `(20, 20000, 1.01)`.
- `window_size` and `sigmoid` on the (10, 24000, 1.01) grid for the app parameter sets.
- `create_target` at (gain, fc, q, tilt) = (0, 105, 0.76, 0.0), (-6, 105, 0.76, None), (4, 80, 0.71, -1.0), (0, 105, 0.71, None), on the (10, 24000, 1.01) grid.
- `interpolate` of a synthetic response (`3*cos(log10(f/500))` with a notch, on an irregular 300-point log grid between 15 Hz and 22 kHz) onto the (10, 24000, 1.01) grid with k=1, and onto the linear `linspace(0, 24000, 4800)` grid (the 0 Hz fix); also with NaN holes in `raw`.
- `center` scalar 1000 Hz and band `[100, 10000]` on the same synthetic response, plus `get_center_value` for `[100, 3000]` and `[100, 10000]`.
- `compensate` with a zero curve and `min_mean_error` false, and with the Harman target `E:/Impulcifer/data/harman-room-target.csv` (or whichever room target CSV exists under `data/`; list what you used) and `min_mean_error` true.
- Real headphone measurement: run the 2.x `core.pipeline_stages.headphone_compensation(estimator, "E:/Impulcifer/data/demo")` with `estimator = ImpulseResponseEstimator.from_wav("E:/Impulcifer/data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")` and store `left.frequency`, `left.raw`, `left.error`, `right.error`; then export, from `left.copy()`: `smoothen_heavy_light` (`smoothed`, `error_smoothed`), `equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=24000)` (`equalization`, `equalized_raw`, `equalized_smoothed`), `minimum_phase_impulse_response(fs=48000, normalize=False, f_res=5)` (9600 taps as `.f64`), and the two channel-balance parameter sets of `equalize` (`max_gain=15`, `treble_f_lower=20000|2000`, `treble_f_upper=24000`) plus `minimum_phase_impulse_response(fs=48000, normalize=False)` (4800 taps). Delete any file `headphone_compensation` writes into `data/demo` (`headphone-responses.wav`, `plots/headphones.png`) after exporting, or run it on a temporary copy of the directory; `data/demo` must be left unchanged (`git status` clean for that path).
- `smoothen` with `window_size=1/3, 1/6` and `smoothen_fractional_octave(window_size=2, treble_f_lower=20000, treble_f_upper=24000)` on the real error.
- `read_from_csv` on every `*.csv` under `E:/Impulcifer/data/` that the 2.x function accepts, plus three synthetic texts: a strict AutoEq file with `frequency,raw,target`, a guess-branch file with `;` separators and a comment line, and a file containing a single-digit number (which the float pattern rejects).
- `magnitude_to_frequency_response` on the demo `FL.wav` first track (first 8192 samples) and on a 1-sample and a 0-sample input (guards).

Rust tests in `crates/impulcifer-dsp/tests/golden_fr.rs`: `golden_generate_frequencies_match_python`, `golden_window_size_and_sigmoid_match_python`, `golden_create_target_matches_autoeq`, `golden_interpolate_matches_python`, `golden_center_matches_python`, `golden_compensate_matches_python`, `golden_smoothen_heavy_light_matches_python`, `golden_smoothen_matches_python`, `golden_equalize_matches_python`, `golden_minimum_phase_impulse_response_matches_python`, `golden_read_from_csv_matches_python`, `golden_magnitude_to_frequency_response_matches_python`. Tolerances: frequency grids exact (`==`); dB arrays `atol 1e-9, rtol 1e-11`; FIR taps `atol 1e-9 * max(1, peak)` plus the P05 spectral criterion (below 1e-5 dB above a -100 dB floor); CSV parses exact. `crates/impulcifer-dsp/tests/properties_fr.rs`: `round_half_even_matches_python`, `interpolate_is_identity_on_its_own_grid`, `center_band_then_center_value_is_zero`, `equalize_never_exceeds_max_gain_away_from_kinks`, `compensate_error_is_raw_minus_target`, `read_from_csv_rejects_single_digit_numbers`.

Do not edit `features.toml`; report the test names and I will register `fr.*` entries.

## Allowed files
`E:/Impulcifer/crates/impulcifer-dsp/src/fr.rs` (or `fr/` submodules), `E:/Impulcifer/crates/impulcifer-dsp/tests/golden_fr.rs`, `E:/Impulcifer/crates/impulcifer-dsp/tests/properties_fr.rs`, `E:/Impulcifer/tests/migration/export_goldens_fr.py`, `E:/Impulcifer/tests/migration/goldens/p09_*`, `E:/Impulcifer/tests/migration/README-fr.md`. Nothing else: not `lib.rs`, not `Cargo.toml`, not `features.toml`, not `export_goldens.py`, not `README.md`, not any other `src/*.rs`.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; f64 only.
- Reproduce the vendored AutoEQ 1.2.5 behaviour including the quirks listed in the survey. Do not "improve" anything; if a Python behaviour looks like a bug, reproduce it and list it in the report.
- Every method has a doc comment naming the Python method, its line range, and the fixture that pins it.

## Verification (foreground, paste output)
```
python E:/Impulcifer/tests/migration/export_goldens_fr.py
git -C E:/Impulcifer status --short data/demo
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --test golden_fr --test properties_fr
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table method → fixture → tolerance → max observed error; (4) every Python quirk you reproduced and any you could not; (5) the exact test function names for the `fr.*` registry entries; (6) anything undone. Do not end your turn before the commands complete.
