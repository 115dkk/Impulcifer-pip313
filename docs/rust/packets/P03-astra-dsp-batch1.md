# P03 (ASTRA): `impulcifer-dsp` batch 1 + Python golden exporter

You are implementing part of the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read these first: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` (sections 6 and 9), `E:/Impulcifer/docs/adr/0002-rust-tauri-rewrite.md`, and `E:/Impulcifer/docs/research/rewrite-stack-2026-09/report-02-dsp.md` sections 2.2 to 2.4 (the exact scipy semantics you must reproduce). The 2.x Python code is the oracle: `E:/Impulcifer/core/audio_io.py`, `E:/Impulcifer/core/virtual_bass.py` (Butterworth/SOS usage), `E:/Impulcifer/autoeq/biquad.py` (RBJ coefficients and the sign convention), `E:/Impulcifer/core/impulse_response_estimator.py` (convolution usage and lengths).

## Scope

Batch 1 primitives in `E:/Impulcifer/crates/impulcifer-dsp`: `fft`, `conv`, `windows`, `filters`, `stats`, and the golden test harness. Batches 2 and 3 (fir/minimum_phase, savgol, peaks, interp, resample, spectrogram, pipeline) are later packets; leave their module files as they are.

All functions are `f64`, operate on `&[f64]` / `Vec<f64>`, and must reproduce the scipy/numpy semantics named below. No `unsafe`. No SIMD intrinsics. Use `rustfft`/`realfft` for transforms.

### `fft.rs`

- `pub fn rfft(x: &[f64]) -> Vec<Complex64>` (numpy.fft.rfft: `n/2 + 1` bins, unnormalized). Handle odd lengths.
- `pub fn irfft(spec: &[Complex64], n: usize) -> Vec<f64>` (numpy.fft.irfft with explicit `n`, normalized by `1/n`).
- `pub fn fft(x: &[Complex64]) -> Vec<Complex64>` and `pub fn ifft(x: &[Complex64]) -> Vec<Complex64>` (normalized by `1/n`).
- `pub fn next_fast_len_legacy(n: usize) -> usize`: scipy.fftpack.next_fast_len (smallest 2·3·5-smooth integer ≥ n).
- `pub fn next_fast_len(n: usize) -> usize`: scipy.fft.next_fast_len for real input (smallest 2·3·5·7·11-smooth integer ≥ n; verify the exact factor set of scipy.fft.next_fast_len(n, real=True) with WebFetch of the scipy docs and note it in a doc comment).
- `pub fn magnitude_response(x: &[f64]) -> Vec<f64>`: port of `core/audio_io.py::magnitude_response` (numpy rfft, keep the first `ceil(N/2)` bins, `20*log10(abs)` without epsilon; `-inf` for zero bins).

Use `num_complex::Complex64` (add `num-complex` to the crate; rustfft re-exports it).

### `conv.rs`

- `pub enum Mode { Full, Same, Valid }`
- `pub fn convolve(a: &[f64], v: &[f64], mode: Mode) -> Vec<f64>`: scipy.signal.convolve semantics, including `Same` centred on the first argument (`a`) as scipy does (not numpy's `max(len)` rule). Use FFT-based convolution for large inputs and direct for small; results must match to ~1e-12 relative.
- `pub fn correlate(a: &[f64], v: &[f64], mode: Mode) -> Vec<f64>`: scipy.signal.correlate.
- `pub fn correlation_lags(in1_len: usize, in2_len: usize, mode: Mode) -> Vec<i64>`: scipy.signal.correlation_lags.

### `windows.rs`

- `pub fn hann(n: usize, sym: bool) -> Vec<f64>`, `pub fn hamming(n: usize, sym: bool) -> Vec<f64>`, `pub fn kaiser(n: usize, beta: f64, sym: bool) -> Vec<f64>` (scipy.signal.windows; implement the modified Bessel I0 yourself with a documented series and test it against scipy values), `pub fn get_window(name: &str, n: usize, fftbins: bool) -> Result<Vec<f64>, DspError>` for "hann", "hamming", "boxcar".

### `filters.rs`

- `pub struct Sos(pub Vec<[f64; 6]>)` second-order sections as scipy lays them out (`b0 b1 b2 a0 a1 a2`, `a0 == 1`).
- `pub enum BType { Lowpass, Highpass }`
- `pub fn butter(order: usize, wn: f64, btype: BType) -> Sos`: scipy.signal.butter(order, wn, btype, output="sos") with `wn` normalized to Nyquist (0 < wn < 1). Implement the analog prototype → bilinear transform → zpk2sos path so the section pairing matches scipy (`pairing="nearest"` default). Verify against goldens for orders 4 and 8 and the cutoffs the pipeline uses (15 Hz and 250 Hz at 48 kHz).
- `pub fn sosfilt(sos: &Sos, x: &[f64]) -> Vec<f64>`: zero initial state, direct form II transposed, section order as given.
- `pub fn tf2sos(b: &[f64], a: &[f64]) -> Sos` for the biquad case used by `core/virtual_bass.py`.
- `pub fn rbj_peaking(fc: f64, q: f64, gain_db: f64, fs: f64) -> Biquad`, `pub fn rbj_low_shelf(...)`, `pub fn rbj_high_shelf(...)` reproducing `autoeq/biquad.py` (note the sign convention of the feedback coefficients in that file and document which convention `Biquad` stores), and `pub fn biquad_response_db(b: &Biquad, freqs: &[f64], fs: f64) -> Vec<f64>`.

### `stats.rs`

- `pub fn linregress(x: &[f64], y: &[f64]) -> LinRegress { slope, intercept, rvalue, pvalue: Option<f64>, stderr }` matching scipy.stats.linregress slope/intercept/rvalue/stderr (pvalue may be `None`).
- `pub fn expit(x: f64) -> f64` numerically stable.
- `pub fn running_mean(x: &[f64], n: usize) -> Vec<f64>`: port of `core/audio_io.py::running_mean` (check its exact definition, including the length of the output).
- `pub fn uniform_filter2d_size3_zero(rows: usize, cols: usize, data: &[f64]) -> Vec<f64>`: scipy.ndimage.uniform_filter(size=3, mode="constant", cval=0) on a row-major 2-D array.

### Errors

`pub enum DspError { InvalidArgument(String) }` with `thiserror` in `lib.rs` (add `pub mod error;` or put it in `lib.rs`).

## Golden test harness

1. Write `E:/Impulcifer/tests/migration/export_goldens.py` (Python, uses the 2.x environment: numpy, scipy, the repo's `core` and `autoeq` packages). It writes `E:/Impulcifer/tests/migration/goldens/<name>.json` files, each a JSON object `{"inputs": {...}, "outputs": {...}, "meta": {"scipy": "...", "numpy": "...", "python": "..."}}` where arrays are plain JSON lists of floats (use `repr`-precision floats: `json.dump(..., allow_nan=True)` and represent `-inf` as the string "-inf" with a documented rule). Keep each file under 200 kB: use short deterministic inputs (seeded `numpy.random.default_rng(1234)` noise of 257, 1024 and 1025 samples, an impulse, a linear chirp of 4096 samples, and the real Butterworth/RBJ parameters used by the pipeline). Cover every function in this packet, including edge cases: odd/even lengths, `Same` mode with unequal lengths, `next_fast_len` around 1000..1100 and 295270, Kaiser beta 5.65326 with N=32001 (store only the first 64 and last 64 taps plus the sum), Butterworth orders 4 and 8, `sosfilt` on the chirp, RBJ filters at fc 105/Q 0.76/gain 4 dB and a high-Q case, `linregress` on a noisy line, `uniform_filter` on a 5x7 matrix.
2. Run it once and commit the generated goldens (they are test fixtures).
3. In `E:/Impulcifer/crates/impulcifer-dsp/tests/golden_batch1.rs`, load each golden with `serde_json` and compare with the tolerances from report-02 section 3.2: complex/FFT `rtol 1e-11, atol 1e-12 * max(1, l1norm(input))`; convolution `atol 1e-11 * max(1, ||x||2 * ||h||2)`; SOS coefficients `rtol 1e-11 / atol 1e-13`, `sosfilt` output `rtol 1e-9 / atol 1e-11 * peak`; window values `atol 1e-12`; `next_fast_len` exact; `linregress` fields `rtol 1e-10`. Write one `#[test]` per function family with a descriptive name (for example `golden_rfft_matches_numpy`, `golden_convolve_modes_match_scipy`, `golden_butter_sos_matches_scipy`, ...). A failing comparison must print the first diverging index, both values and the tolerance.
4. Add unit tests for pure properties too (Parseval for rfft/irfft, linearity of convolve, DC gain of lowpass butter ≈ 1).

## Allowed files

- `E:/Impulcifer/crates/impulcifer-dsp/**` (Cargo.toml dependencies may be added; keep `[lints] workspace = true`)
- `E:/Impulcifer/tests/migration/export_goldens.py`, `E:/Impulcifer/tests/migration/goldens/**`, `E:/Impulcifer/tests/migration/README.md`
- `E:/Impulcifer/features.toml`: only the `dsp.*` entries for the functions you implemented, setting `status = "implemented"` and `tests = ["impulcifer-dsp::<test_fn>", ...]` with exact test function names.

Do not modify any other crate, `webview_ui/`, or existing Python modules. Do not delete or rename existing module files in `impulcifer-dsp`.

## Hard rules

- `#![forbid(unsafe_code)]` stays in every crate root; no `unsafe` anywhere.
- Do not "improve" scipy behaviour. If scipy does something odd (for example `Same` centring), reproduce it and document it in a doc comment with the scipy version you verified against.
- Verify scipy semantics with WebFetch of the scipy 1.16 reference pages when unsure; cite the URL in the doc comment.
- Keep functions allocation-simple and readable; no premature optimisation.

## Verification (run in the foreground and paste the output)

```
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
```

`cargo test -p impulcifer-policy` must pass after your `features.toml` edits (it checks that every test you named exists).

## Report format

Reply with, in this order: (1) files changed; (2) verification outputs verbatim; (3) a table function → golden file → tolerance used → max observed error; (4) any scipy semantic you could not reproduce exactly and why; (5) anything left undone. Do not end your turn before the verification commands have run to completion.
