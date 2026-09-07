# P06 (ASTRA): `impulcifer-dsp` batch 3 — nnresample design, polyphase resampling, spectrogram

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` section 6, `E:/Impulcifer/docs/research/rewrite-stack-2026-09/report-02-dsp.md` lines 100 to 110 (the verified nnresample design and the resample_poly padding arithmetic), the finished batches in `E:/Impulcifer/crates/impulcifer-dsp/src/{fft,conv,windows,filters,fir,smoothing,peaks,interp,stats}.rs` and their tests, and the golden exporter `E:/Impulcifer/tests/migration/export_goldens.py` (extend it; do not rewrite it).

The 2.x oracle: the installed nnresample 0.2.4.1 at `C:/Users/32170336/AppData/Local/Programs/Python/Python313/Lib/site-packages/nnresample/nnresample.py` (`compute_filt` lines 14 to 116, `resample` lines 118 to 170) and `utility.py` (`disambiguate_params`, `As_from_beta`), scipy `resample_poly`/`upfirdn` (`scipy/signal/_signaltools.py`, `_upfirdn.py` of the installed scipy; cite the version you read), and the call sites `E:/Impulcifer/core/impulse_response.py` lines 121 to 124 (`nnresample.resample(self.data, fs, self.fs)`, so `up = new_fs`, `down = old_fs`) and `E:/Impulcifer/core/plotting/impulse_response_plotter.py` lines 114 to 222 (nfft/noverlap selection and the `spectrogram(..., window=get_window("hann", nfft), nperseg=nfft, noverlap=noverlap, mode="psd")` call).

Run every command in the foreground. Never use background execution.

## Scope

### `resample.rs`
- `pub struct ResampleDesign { pub up: usize, pub down: usize, pub taps: Vec<f64>, pub cutoff: f64, pub beta: f64 }`.
- `pub fn nnresample_design(up: usize, down: usize) -> Result<ResampleDesign, DspError>`: `nnresample.compute_filt(up, down, fc="nn", beta=<from As=60>, N=32001)` exactly as report-02 lines 100 to 106 and the installed source describe: reduce `up`/`down` by gcd, `q = max(up, down)`, `beta = 0.1102 * (60 - 8.7)` (the value `resample` actually derives through `disambiguate_params`, not the `compute_filt` default of 5.0; write the number in a doc comment), first design `firwin(N, 1/q, window=("kaiser", beta))` (symmetric Kaiser-windowed sinc with unity DC gain; reuse `windows::kaiser` and add a `firwin_lowpass` helper if `fir.rs` lacks one), zero-pad to `F = 2**19`, real FFT, `H = F/2 + 1`, `c = sqrt(1 + (beta/pi)**2)`, `bot = floor(H/q)`, `top = ceil(H * (1/q + 2*c/N))`, argmin of the magnitude over `[bot, top)`, `f_null = (bot + argmin)/H`, final cutoff `2/q - f_null`, redesign with `firwin`. Keep the search deterministic (first minimum on ties, like `np.argmin`). Same-rate input (`up == down` after reduction) must return the design nnresample 0.2.4.1 produces, not a short-circuit copy (0.2.5 master differs; we pin the release).
- `pub fn resample_poly(x: &[f64], up: usize, down: usize, taps: &[f64]) -> Result<Vec<f64>, DspError>`: `scipy.signal.resample_poly(x, up, down, window=taps)` with explicit float64 taps, `padtype="constant"`, `cval=0`: gcd reduction, `n_out = ceil(n_in * up / down)`, `half_len = (len(taps) - 1) // 2`, `h = taps * up`, `pre_pad = down - half_len % down` (a full `down` when divisible), `pre_remove = (half_len + pre_pad) // down`, the minimum post-padding that yields at least `n_out + pre_remove` outputs, polyphase upfirdn with zero signal extension, then keep `[pre_remove, pre_remove + n_out)`. Never materialise the upsampled signal; compute the polyphase equivalent (document the complexity). For 48000 to 44100 the reduced ratio is 147/160, `half_len = 16000`, `pre_pad = 160`, `pre_remove = 101`; pin these numbers in a unit test.
- `pub fn nnresample(x: &[f64], new_fs: u32, old_fs: u32) -> Result<Vec<f64>, DspError>`: the `nnresample.resample(s, up=new_fs, down=old_fs)` composition, output length `ceil(n * new_fs / old_fs)`, using a process-wide design cache keyed by the reduced `(up, down)` (a `std::sync::OnceLock<Mutex<HashMap<..>>>` is fine; document that the cache is deterministic and never affects values).
- A private `gcd(a, b)` helper is fine.

### `spectrogram.rs`
- `pub struct SpectrogramParams { pub nfft: usize, pub noverlap: usize }` and `pub fn spectrogram_params(n: usize, fs: u32, f_res: f64, n_segments: usize) -> Option<SpectrogramParams>`: the exact selection logic of `plot_spectrogram` lines 131 to 211 (`target_f_res_nfft = round(fs / f_res)`, `min_time_segments = 3`, `max_nfft_for_segments = (2*n) // (min_time_segments + 1)`, clamps to `n`, `None` when nfft is 0, `noverlap` from `step_size = (n - nfft) / n_segments` when valid else `nfft // 2`, the `noverlap >= nfft` and `< 0` clamps). Read the Python to get every branch and the defaults of `f_res` and `n_segments` from the method signature (cite the lines).
- `pub struct Spectrogram { pub freqs: Vec<f64>, pub times: Vec<f64>, pub power: Vec<Vec<f64>> }` (`power[freq_bin][segment]`, the same orientation as the scipy `Sxx[f, t]` array).
- `pub fn spectrogram(x: &[f64], fs: u32, nperseg: usize, noverlap: usize) -> Result<Spectrogram, DspError>`: `scipy.signal.spectrogram(x, fs, window=get_window("hann", nperseg), nperseg, noverlap, mode="psd")` with the scipy defaults for everything else (`nfft = nperseg`, `detrend="constant"` meaning per-segment mean removal, `scaling="density"` so `Sxx` is divided by `fs * sum(win**2)`, one-sided with the interior bins doubled, `boundary=None`, `padded=False`, `axis=-1`; `get_window("hann", n)` is the periodic Hann (`fftbins=True`), which `windows::hann(n, sym=false)` already provides). Time stamps are `(nperseg/2 + i*step) / fs` for `step = nperseg - noverlap`. Read `scipy/signal/_spectral_py.py::_spectral_helper` of the installed version and cite the version; do not "improve" the estimator.

### Golden exporter and tests
Extend `E:/Impulcifer/tests/migration/export_goldens.py` with a `p06_` family:
- nnresample designs for (48000, 44100), (44100, 48000), (96000, 48000), (48000, 96000) and the same-rate case (48000, 48000): store `up`, `down`, `beta`, the final cutoff, the argmin index, first/last 256 taps and SHA-256 of the full LE-f64 taps in JSON, plus the full taps in `p06_taps_<up>_<down>.f64` (the `.f64` binary format is documented in `tests/migration/README.md`).
- `resample_poly` and `nnresample.resample` on (a) a unit impulse of 4800 samples, (b) a 0.5 s windowed 1 kHz sine at 48000 Hz, and (c) the real demo impulse response, first track of `E:/Impulcifer/data/demo/FL.wav` cropped to 8192 samples (read it with `soundfile` in the exporter), for 48000 to 44100 and 48000 to 96000; store first/last 256 outputs, length and SHA-256, and the full outputs as `.f64` when the Rust test needs them.
- `spectrogram_params` on the lengths 0, 1, 100, 4800, 48000, 295000 with fs 48000, f_res 10, n_segments 200 (the 2.x default), and one case with n_segments 0.
- `spectrogram` on the 1 kHz sine and on seeded white noise at nfft 4800 / noverlap 2400: store `freqs`, `times`, and the full `Sxx` as `.f64` (row-major, shape in JSON).

Rust tests in `crates/impulcifer-dsp/tests/golden_batch3.rs` and `properties_batch3.rs`: `golden_nnresample_design_matches_python` (taps `rtol 1e-9, atol 1e-11 * max(1, peak)`; cutoff, beta and argmin index exact within 1e-12), `golden_resample_poly_matches_scipy` (`atol 1e-10` on the impulse and sine, `atol 1e-9 * max|x|` on the demo IR), `golden_nnresample_matches_python` (same tolerance, length exact), `resample_poly_padding_arithmetic_48k_to_44k1` (the pinned 147/160/16000/160/101 numbers), `golden_spectrogram_params_match_python` (exact), `golden_spectrogram_matches_scipy` (`Sxx` `rtol 1e-9, atol 1e-14`, times and freqs `atol 1e-12`), and properties: a same-rate `nnresample` returns the input length and reproduces a delayed copy within 1e-6 after group-delay alignment (state the alignment you use), `resample_poly` on a DC signal of length 4800 has a flat interior (relative deviation below 1e-6 away from the edges), `spectrogram` of a pure sine puts its maximum in the 1 kHz bin, and the spectrogram of a zero signal is all zeros with the expected shape.

`features.toml`: set `dsp.nnresample_design`, `dsp.resample_poly`, `dsp.spectrogram` to implemented with the exact test names.

## Allowed files
`E:/Impulcifer/crates/impulcifer-dsp/**`, `E:/Impulcifer/tests/migration/export_goldens.py`, `E:/Impulcifer/tests/migration/goldens/p06_*`, `E:/Impulcifer/tests/migration/README.md`, and the three `dsp.*` entries named above in `E:/Impulcifer/features.toml`. Nothing else. Do not change batch 1 or batch 2 behaviour; if a helper needs a new capability, add a new function.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; f64 only; no SIMD intrinsics; no new crates beyond the workspace dependencies (rustfft/realfft are already there).
- Reproduce the installed nnresample 0.2.4.1 and scipy semantics; verify with the installed sources and WebFetch of the scipy reference pages, and cite them in doc comments. Do not "fix" the df-to-N branch of `disambiguate_params` (not exercised) and do not add the 0.2.5 same-rate short-circuit.
- Every function has a doc comment naming the Python function it mirrors and the fixture that pins it.
- The 32001-tap design plus the 2^19 FFT search runs once per (up, down) and must complete in under 200 ms in release mode on this machine; report the measured time.

## Verification (foreground, paste output)
```
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table function → fixture → tolerance → max observed error; (4) the measured design time; (5) any scipy/nnresample semantic you could not reproduce and why; (6) anything undone. Do not end your turn before the commands complete.
