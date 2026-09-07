# P05 (ASTRA): `impulcifer-dsp` batch 2 — fir, minimum_phase, savgol, find_peaks, splines

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` section 6, `E:/Impulcifer/docs/research/rewrite-stack-2026-09/report-02-dsp.md` section 2.4 (exact algorithms: firwin2, homomorphic minimum phase, Savitzky-Golay `interp` edges, find_peaks plateau rule, FITPACK k=1/2/3 not-a-knot with `ext=0` extrapolation), the finished batch 1 in `E:/Impulcifer/crates/impulcifer-dsp/src/{fft,conv,windows,filters,stats}.rs` and its tests, and the golden exporter `E:/Impulcifer/tests/migration/export_goldens.py` (extend it; do not rewrite it). The 2.x oracle call sites: `E:/Impulcifer/autoeq/frequency_response.py` lines 637 to 705 (firwin2 + minimum_phase with `n_fft=len(ir)`), 859 to 900 and 1033 to 1105 (interpolation k=1/2, savgol), 350 to 420 (find_peaks seeds), `E:/Impulcifer/core/impulse_response.py` lines 32 to 70 and `E:/Impulcifer/core/decay.py` lines 12 to 41 (peak search with height 0.12589 and the earliest-peak rule), `E:/Impulcifer/core/hrir.py` lines 43 to 73 (log-axis k=1 with k=3 retry).

Run every command in the foreground. Never use background execution.

## Scope

### `fir.rs`
- `pub fn firwin2(numtaps: usize, freq: &[f64], gain: &[f64], nfreqs: Option<usize>, fs: f64) -> Result<Vec<f64>, DspError>`: scipy.signal.firwin2 with the default Hamming window (`window="hamming"`, `antisymmetric=False`), `fs` given, `nyq` semantics via `fs/2`. Reproduce the mesh size rule (`nfreqs = 1 + 2**ceil(log2(numtaps))` when None), the epsilon nudging of duplicated interior frequencies, the linear interpolation onto the uniform grid, the `exp(-1j*(numtaps-1)/2 * pi * f/nyq)` phase, the irfft, the truncation and the symmetric Hamming multiplication. Validate: sorted frequencies from 0 to fs/2, even numtaps requires zero gain at Nyquist (Type II).
- `pub fn minimum_phase(h: &[f64], n_fft: usize, half: bool) -> Result<Vec<f64>, DspError>`: scipy.signal.minimum_phase(method="homomorphic") exactly as described in report-02 section 2.4 (`1e-7 * min(nonzero |H|)` floor added to every bin, `0.5*log` when half, lifter with `w[0]=1`, `w[1:stop]=2`, `w[stop]=1` only when `n_fft` is odd, output length `ceil(len/2)` when half else len). Also `pub fn minimum_phase_default_nfft(len: usize) -> usize` (`2**ceil(log2(2*(len-1)/0.01))`).

### `smoothing.rs`
- `pub fn savgol_filter(x: &[f64], window_length: usize, polyorder: usize) -> Result<Vec<f64>, DspError>`: scipy.signal.savgol_filter with `deriv=0`, `delta=1`, `mode="interp"`: interior FIR coefficients from the least-squares design, and the first/last `window_length/2` outputs from a polynomial fit of the first/last `window_length` inputs evaluated at those positions. Odd window required, `polyorder < window_length`.
- `pub fn fractional_octave_window(octaves: f64, ratio: f64) -> usize`: the 2.x window rule `round(log(2**octaves)/log(ratio))`, incremented if even (read `autoeq/frequency_response.py` smoothing helpers for the exact formula and name it after the Python helper).

### `peaks.rs`
- `pub fn find_peaks(x: &[f64], height: Option<f64>) -> Vec<usize>`: scipy.signal.find_peaks with only `height` (inclusive lower bound), strict-neighbour comparison, flat-top runs reported as the floor midpoint, endpoints never peaks, NaN policy documented (scipy treats NaN comparisons as false).
- `pub fn first_peak_index(data: &[f64], start: usize, end: Option<usize>, peak_height: f64) -> usize`: port of `core/impulse_response.py::ImpulseResponse.peak_index` and `core/decay.py::_peak_index` (normalise a copy of the slice by its max-abs, positive and negative peaks with inclusive height, return the earliest, fall back to argmax of abs, silence threshold 1e-20, empty returns start).

### `interp.rs`
- `pub struct Spline { .. }` with `pub fn new(x: &[f64], y: &[f64], k: usize) -> Result<Spline, DspError>` for k in {1, 2, 3} reproducing `scipy.interpolate.InterpolatedUnivariateSpline(x, y, k=k)` (s=0): strictly increasing x, at least k+1 points; for k=3 the FITPACK interpolating knot placement is not-a-knot on the interior points (`t = x[2..n-2]` plus repeated endpoints), for k=2 the interior knots are the midpoints `(x[i]+x[i+1])/2` for i in 1..n-2 plus repeated endpoints; solve the B-spline collocation system (banded, but a dense solve is acceptable for the sizes here, up to about 8,000 points; document complexity); `pub fn eval(&self, x: &[f64]) -> Vec<f64>` with `ext=0` polynomial extrapolation from the end pieces (de Boor evaluation on the boundary polynomial), `pub fn eval_one(&self, x: f64) -> f64`.
- `pub fn interp_log_axis(freq: &[f64], values: &[f64], k: usize, at: &[f64]) -> Result<Vec<f64>, DspError>`: the recurring 2.x pattern `InterpolatedUnivariateSpline(log10(f), v, k)(log10(at))`, with the k=3 → k=1 retry that `core/hrir.py` lines 43 to 73 performs (read it and reproduce the condition).

### Golden exporter and tests
Extend `E:/Impulcifer/tests/migration/export_goldens.py` with a `p05_` family: firwin2 on the actual AutoEQ mesh (numtaps 9600 at fs 48000 with a synthetic smooth target and one with a notch; nfreqs None), minimum_phase with `n_fft=len(h)` (even) and a general odd case, savgol windows 7/11/23 with polyorder 2 on noise and on a quadratic (1 pass and 1000 repeated passes on 695 points at ratio 1.01 like AutoEQ), find_peaks on flat-top and threshold-edge cases plus the real `peak_height=0.12589` impulse cases, splines k=1/2/3 on irregular log grids including extrapolation points on both sides and a 4-point minimal case, and `interp_log_axis` on a real Harman target CSV from `E:/Impulcifer/data/`. Keep files under 200 kB each (store large FIRs as the first/last 256 taps plus SHA-256 of the full LE-f64 bytes, and store the full array in a separate `.f64` binary file when a test needs it; document the format in `tests/migration/README.md`).

Rust tests in `crates/impulcifer-dsp/tests/golden_batch2.rs` and `properties_batch2.rs`: one test per function family (`golden_firwin2_matches_scipy`, `golden_minimum_phase_matches_scipy`, `golden_savgol_matches_scipy`, `golden_savgol_thousand_passes_stay_within_budget`, `golden_find_peaks_matches_scipy`, `golden_first_peak_index_matches_python`, `golden_spline_k1_matches_fitpack`, `golden_spline_k2_matches_fitpack`, `golden_spline_k3_matches_fitpack`, `golden_interp_log_axis_matches_python`) with the tolerances of report-02 section 3.2 (FIR coefficients `rtol 1e-9, atol 1e-11*max(1,peak)`; per-stage spectral difference below 1e-5 dB above a -100 dB floor for minimum phase; savgol single pass `atol 1e-10`, 1000 passes `atol 1e-7`; peak indices exact; spline values in dB `atol 1e-9, rtol 1e-11`; a natural cubic must fail the k=3 fixture, prove it with a negative test). Property tests: firwin2 output symmetry, minimum-phase energy concentration (first half holds > 99% of energy for a smooth target), spline interpolation hits the knots exactly, find_peaks endpoints excluded.

`features.toml`: set `dsp.firwin2`, `dsp.minimum_phase`, `dsp.savgol_filter`, `dsp.find_peaks`, `dsp.spline_k1`, `dsp.spline_k2`, `dsp.spline_k3` to implemented with the exact test names.

## Allowed files
`E:/Impulcifer/crates/impulcifer-dsp/**`, `E:/Impulcifer/tests/migration/export_goldens.py`, `E:/Impulcifer/tests/migration/goldens/p05_*`, `E:/Impulcifer/tests/migration/README.md`, and the seven `dsp.*` entries in `E:/Impulcifer/features.toml`. Nothing else. Do not change batch 1 behaviour; if a batch 1 helper needs a new capability, add a new function.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; f64 only; no SIMD intrinsics.
- Reproduce scipy 1.16+ semantics; verify with WebFetch of the scipy reference pages and cite them in doc comments. Do not "improve" the algorithms.
- Every function has a doc comment naming the scipy/2.x function it mirrors and the fixture that pins it.

## Verification (foreground, paste output)
```
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table function → fixture → tolerance → max observed error; (4) any scipy semantic you could not reproduce and why; (5) anything undone. Do not end your turn before the commands complete.
