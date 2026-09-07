# PA02 (ASTRA): performance audit of `impulcifer-dsp` primitives against scipy / numpy / nnresample

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` exactly (method, allowed files, hard rules, verification, report). This file only names the crate and its operations. Read the crate first (`E:/Impulcifer/crates/impulcifer-dsp/src/*.rs`, all modules that exist when you start) and the 2.x call sites that fix the sizes: `E:/Impulcifer/core/impulse_response.py` (`equalize`: full convolution of an IR with a 9,600-tap FIR; `resample`), `core/impulse_response_estimator.py:149-151` (`estimate`: `convolve(mode='same')` of a recording with the 295,000-sample inverse filter), `core/hrir.py:940-974` (30 ms correlations), `core/parallel_workers.py:69-131` (the per-speaker EQ chain: `smoothen_heavy_light`, `equalize`, `minimum_phase_impulse_response` with `firwin2(19200)` and `minimum_phase(n_fft=19200)`), `core/decay.py` (`linregress`, `running_mean`), `core/virtual_bass.py` (`butter` + `sosfilt` on 96,000 samples), `core/plotting/impulse_response_plotter.py:114-222` (spectrogram sizes).

Run every command in the foreground. Never use background execution. The Python side of the benchmark runs with the newest CPython on this machine (`py -3.14`; and the venv `%LOCALAPPDATA%\impulcifer-bench\py314t\Scripts\python.exe` for thread-parallel operations), never with the default `python`. Check `git status` first: if another worker's uncommitted edits are present under `crates/impulcifer-dsp/`, stop and report instead of starting.

## Operations and sizes (48 kHz, `data/demo`)

| op | Rust | Python oracle | size |
|---|---|---|---|
| `convolve_full_ir_fir` | `conv::convolve(ir, fir, Mode::Full)` | `scipy.signal.convolve(ir, fir, mode="full")` | 96,000 × 9,600 |
| `convolve_same_estimate` | `conv::convolve(rec, inv, Mode::Same)` | `scipy.signal.convolve(rec, inv, mode="same", method="auto")` | 391,000 × 295,000 |
| `correlate_full_30ms` | `conv::correlate(a, b, Mode::Full)` | `scipy.signal.correlate(a, b, mode="full")` | 1,440 × 1,440, repeated 8 times per call like `align_ipsilateral_all` |
| `rfft_irfft_96000` | `fft::rfft` then `fft::irfft(.., n)` | `numpy.fft.rfft` / `irfft` | 96,000 |
| `magnitude_response_96000` | `fft::magnitude_response` | `core.audio_io.magnitude_response` | 96,000 |
| `butter8_sosfilt_96000` | `filters::butter(4, ..)` cascaded twice then `sosfilt` | `scipy.signal.butter(4, .., output="sos")` ×2 stacked, `sosfilt` | 96,000 |
| `firwin2_19200` | `fir::firwin2(19200, f, g, None, 48000.0)` | `scipy.signal.firwin2(19200, f, g, fs=48000)` | 9,600-point linear mesh |
| `minimum_phase_19200` | `fir::minimum_phase(h, 19200, true)` | `scipy.signal.minimum_phase(h, n_fft=19200)` | 19,200 taps |
| `savgol_heavy_light_783` | 4 × `smoothing::savgol_filter` (windows 13, 23, 23, 91) + 2 × window 23 | the same six `scipy.signal.savgol_filter` calls (what `smoothen_heavy_light` performs) | 783 points |
| `find_peaks_96000` | `peaks::find_peaks(x, Some(0.12589))` on `x` and `-x` | `scipy.signal.find_peaks(height=0.12589)` twice | 96,000 |
| `first_peak_index_96000` | `peaks::first_peak_index` | `core.decay._peak_index` | 96,000 |
| `spline_k1_783_to_4800` | `interp::Spline::new(k=1)` + `eval` | `InterpolatedUnivariateSpline(k=1)(x)` | 783 knots, 4,800 queries (log axis) |
| `spline_k2_783` | `Spline::new(k=2)` + `eval` on the same 783 | `k=2` | 783 |
| `spline_k3_783` | `Spline::new(k=3)` + `eval` | `k=3` | 783 |
| `linregress_1000` | `stats::linregress` | `scipy.stats.linregress` | 1,000 |
| `nnresample_design_147_160` | `resample::nnresample_design(147, 160)` (uncached) | `nnresample.compute_filt(147, 160, fc="nn", beta=5.65326, N=32001)` | 32,001 taps, 2^19 FFT |
| `resample_poly_96000_48k_44k1` | `resample::resample_poly(x, 147, 160, taps)` | `scipy.signal.resample_poly(x, 147, 160, window=taps)` | 96,000 |
| `nnresample_96000_48k_96k` | `resample::nnresample(x, 96000, 48000)` (design cached, exclude the first call) | `nnresample.resample(x, 96000, 48000)` (cached too) | 96,000 |
| `spectrogram_295000_4800` | `spectrogram::spectrogram(x, 48000, 4800, noverlap)` with `noverlap` from `spectrogram_params(n, 48000, 10.0, 200)` | `scipy.signal.spectrogram(x, fs, window=get_window("hann", 4800), nperseg=4800, noverlap=.., mode="psd")` | 295,000 |
| `windows_32001` | `windows::kaiser(32001, 5.65326, true)`, `hann(19200, true)`, `hamming(19200, true)` | `scipy.signal.windows.*` | as named |
| `expit_783`, `next_fast_len_1e6` | `stats::expit` over 783 values, `fft::next_fast_len(1_000_000)` / `next_fast_len_legacy` | `scipy.special.expit`, `scipy.fft.next_fast_len` / `scipy.fftpack.next_fast_len` | trivial; report but do not optimise unless below 1.0 by a wide margin |

Inputs: deterministic LCG noise (seed 7) shaped as needed; the FIR for `convolve_full_ir_fir` is a windowed sinc; `firwin2` uses the actual AutoEQ mesh (`linspace(0, 24000, 9600)` and a smooth gain curve, Nyquist gain 0). Python and Rust must use identical inputs (generate them in the bench from the same LCG; do not read the goldens).

Known suspects to check first: per-call `FftPlanner` creation in `fft.rs` (plan once per length, `thread_local!` cache), `convolve` choosing direct O(N·M) where scipy's `method='auto'` would pick FFT (compare against scipy's `choose_conv_method` heuristic; a direct loop at 96,000 × 9,600 is ~1e9 multiply-adds and will lose), `Complex64` temporaries in the real transforms (use `realfft`), `Vec` allocation inside `sosfilt`/`savgol` per call, `Spline` solving a dense system where a banded one suffices, `find_peaks` collecting into intermediate `Vec`s, `resample_poly` inner loop indexing (`t - j * up`) with bounds checks, and `spectrogram` recomputing the window and plan per segment.

The per-speaker EQ chain in 2.x runs in a process pool; this audit measures single-thread primitives only. Parallelism across speakers is the pipeline packet's job; do not add `rayon` inside primitives except where scipy itself is multi-threaded for that op (it is not for these).

Report to `docs/rust/perf/impulcifer-dsp.md`. The registry test name is `bench_smoke_impulcifer_dsp`.
