# PA02: impulcifer-dsp primitive performance audit

Date: 2026-09-07. Scope: the 22 landed primitive operations in `../packets/PA02-astra-perf-dsp.md`, following `../packets/PA-astra-perf-audit.md`.

## Result

All 22 operations passed `Python median / Rust median >= 1.0` in the final independent rerun. Full DSP tests, policy, formatting and clippy also passed. This is a single-machine primitive audit, not a pipeline or cross-platform performance claim. Concurrent workers were implementing excluded modules during this audit; those files were neither edited nor benchmarked by PA02. Their tests were included in the full regression runs.

An intermediate independent rerun found magnitude response slower than Python despite the first worker's passing result. That failure was retained, investigated and fixed before the final measurement. See the iteration record below. Timings vary between runs; do not treat these ratios as confidence intervals.

`features.toml` was deliberately not edited, as required by the packet. The available registry hook is `impulcifer-dsp::bench_smoke_impulcifer_dsp`.

## 1. Environment and method

- CPU: Intel Core i5-12600KF, 10 physical cores and 16 logical processors (physical count checked using WMIC).
- OS: Windows 11, build 22621, x86_64.
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`, LLVM 22.1.6; Cargo `1.97.0 (c980f4866 2026-06-30)`; target `x86_64-pc-windows-msvc`.
- Release benchmark profile: ordinary `cargo bench` optimized build, without target-specific flags or new dependencies.
- Python command: `py -3.14`.
- Actual interpreter: `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`.
- Python: `3.14.5 (tags/v3.14.5:5607950, May 10 2026, 10:43:50) [MSC v.1944 64 bit (AMD64)]`.
- NumPy 2.5.3; SciPy 1.18.1.
- NumPy BLAS/LAPACK from `numpy.show_config()`: scipy-openblas 0.3.34.106.0, `USE64BITINT DYNAMIC_ARCH NO_AFFINITY Haswell MAX_THREADS=24`.
- FFT: NumPy and SciPy pocketfft with default worker settings; Rust uses the existing realfft/rustfft dependencies.
- `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `NUMEXPR_NUM_THREADS`: all unset. No thread settings were changed.
- Free-threaded Python was not needed: PA02 measures single-thread primitives, not the per-speaker process/thread pool. No rayon was introduced.
- No sampling profiler was installed. Profiling used code inspection and narrowed benchmark runs. An optional psutil environment probe failed because that interpreter has no psutil; nothing was installed.

Both implementations independently generate the same seed-7 wrapping 32-bit LCG: `state = (1664525 * state + 1013904223) mod 2^32`, followed by `state / 2147483648 - 1`. Noise is exactly reproducible. Windowed-sinc FIRs and the smooth gain mesh use matching scalar formulas, with platform libm rounding possible; no golden inputs are loaded. FIRWIN2 uses 9,600 linearly spaced frequencies from 0 to 24,000 Hz and zero Nyquist gain. Splines receive an already-logarithmic axis in both languages.

Input generation, explicit polyphase taps and the composed resampler's cache warm-up are outside timing. Uncached filter design calls `nnresample_design` / `compute_filt` on every timed invocation. Output calculation and allocation are inside timing. Magnitude response includes the frequency vector in Rust because the Python function returns it. Both correlation operands use the same deterministic noise; each timed call executes eight correlations. The six Savitzky-Golay calls use windows 13, 23, 23, 91, 23, 23 with polynomial order 2.

Every row has 3 warm-ups and 11 measurements; median and minimum are printed in milliseconds. No repetition count was reduced. Final `next_fast_len_1e6` measurements batch 1,000 pairs and divide by 1,000 to overcome Windows clock resolution. The original baseline used one pair, so its before/after timing is coarse and not directly comparable at nanosecond precision. The full benchmark and the smoke test share the actual `benches/perf.rs` module. The smoke test runs all 22 cases, reducing signal sizes; the fixed filter-design API still produces its 32,001 taps.

SciPy `choose_conv_method` returned `fft`, `fft`, `direct` for full convolution, same convolution and 30 ms correlation respectively. Rust already chose FFT for all three sizes. No direct/FFT heuristic change was needed. Spline construction was already banded when this audit began.

## 2. Original baseline tables

These tables are the first worker's pre-optimization output, preserved verbatim.

### Rust before

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| convolve_full_ir_fir | 96000 x 9600 | 6.437400 | 6.051100 |
| convolve_same_estimate | 391000 x 295000 | 31.982100 | 31.061000 |
| correlate_full_30ms | 8 x 1440 x 1440 | 0.880500 | 0.866200 |
| rfft_irfft_96000 | 96000 | 3.269300 | 2.988400 |
| magnitude_response_96000 | 96000 | 2.426800 | 2.287400 |
| butter8_sosfilt_96000 | 96000 | 0.887900 | 0.881700 |
| firwin2_19200 | 19200 taps / 9600 mesh | 1.906400 | 1.793600 |
| minimum_phase_19200 | 19200 | 2.808000 | 2.595700 |
| savgol_heavy_light_783 | 783 | 0.072500 | 0.072300 |
| find_peaks_96000 | 96000 | 1.007800 | 0.988900 |
| first_peak_index_96000 | 96000 | 1.161100 | 1.134600 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.071500 | 0.071100 |
| spline_k2_783 | 783 | 0.031600 | 0.031200 |
| spline_k3_783 | 783 | 0.040700 | 0.040100 |
| linregress_1000 | 1000 | 0.001800 | 0.001800 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 10.017500 | 9.249900 |
| resample_poly_96000_48k_44k1 | 96000 | 30.316900 | 29.822300 |
| nnresample_96000_48k_96k | 96000 | 4975.661700 | 4940.021200 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 9.906500 | 9.492700 |
| windows_32001 | 32001 / 19200 / 19200 | 1.051900 | 1.050000 |
| expit_783 | 783 | 0.002700 | 0.002600 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000500 | 0.000500 |

### Python before

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| convolve_full_ir_fir | 96000 x 9600 | 3.946500 | 3.774000 |
| convolve_same_estimate | 391000 x 295000 | 26.200800 | 25.303700 |
| correlate_full_30ms | 8 x 1440 x 1440 | 1.192100 | 1.181400 |
| rfft_irfft_96000 | 96000 | 1.491800 | 1.442000 |
| magnitude_response_96000 | 96000 | 1.272900 | 1.069700 |
| butter8_sosfilt_96000 | 96000 | 0.916100 | 0.884600 |
| firwin2_19200 | 19200 taps / 9600 mesh | 2.863600 | 2.516500 |
| minimum_phase_19200 | 19200 | 1.126800 | 1.098300 |
| savgol_heavy_light_783 | 783 | 1.254500 | 1.182300 |
| find_peaks_96000 | 96000 | 1.388800 | 1.279900 |
| first_peak_index_96000 | 96000 | 2.038400 | 1.861700 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.105600 | 0.099400 |
| spline_k2_783 | 783 | 0.082200 | 0.081200 |
| spline_k3_783 | 783 | 0.100900 | 0.100600 |
| linregress_1000 | 1000 | 0.240000 | 0.228000 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 9.862900 | 9.299600 |
| resample_poly_96000_48k_44k1 | 96000 | 8.030600 | 7.872300 |
| nnresample_96000_48k_96k | 96000 | 1514.760300 | 1511.429900 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 12.322500 | 12.039400 |
| windows_32001 | 32001 / 19200 / 19200 | 1.216100 | 1.202100 |
| expit_783 | 783 | 0.003300 | 0.003200 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000300 | 0.000300 |

## 3. Final independent rerun tables

These are the parent session's output after the follow-up optimization and after independently rerunning all required correctness gates. No rows were selected from different final runs.

### Rust after

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| convolve_full_ir_fir | 96000 x 9600 | 2.172600 | 1.988200 |
| convolve_same_estimate | 391000 x 295000 | 14.431500 | 13.607000 |
| correlate_full_30ms | 8 x 1440 x 1440 | 0.122400 | 0.121700 |
| rfft_irfft_96000 | 96000 | 1.027000 | 0.982400 |
| magnitude_response_96000 | 96000 | 0.411900 | 0.393000 |
| butter8_sosfilt_96000 | 96000 | 0.647800 | 0.420300 |
| firwin2_19200 | 19200 taps / 9600 mesh | 1.100000 | 0.986800 |
| minimum_phase_19200 | 19200 | 0.851500 | 0.825000 |
| savgol_heavy_light_783 | 783 | 0.067000 | 0.066700 |
| find_peaks_96000 | 96000 | 0.957900 | 0.919100 |
| first_peak_index_96000 | 96000 | 1.470100 | 1.295600 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.073300 | 0.073000 |
| spline_k2_783 | 783 | 0.031400 | 0.030800 |
| spline_k3_783 | 783 | 0.040500 | 0.040300 |
| linregress_1000 | 1000 | 0.001800 | 0.001800 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 4.869400 | 4.279700 |
| resample_poly_96000_48k_44k1 | 96000 | 4.804300 | 4.644500 |
| nnresample_96000_48k_96k | 96000 | 624.035800 | 621.279100 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 4.212600 | 3.797900 |
| windows_32001 | 32001 / 19200 / 19200 | 0.612500 | 0.611600 |
| expit_783 | 783 | 0.002900 | 0.002800 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000010 | 0.000010 |

### Python after

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| convolve_full_ir_fir | 96000 x 9600 | 3.910800 | 3.509700 |
| convolve_same_estimate | 391000 x 295000 | 26.555800 | 25.349800 |
| correlate_full_30ms | 8 x 1440 x 1440 | 1.207700 | 1.176800 |
| rfft_irfft_96000 | 96000 | 2.075400 | 1.917100 |
| magnitude_response_96000 | 96000 | 1.444900 | 1.405900 |
| butter8_sosfilt_96000 | 96000 | 0.916800 | 0.878100 |
| firwin2_19200 | 19200 taps / 9600 mesh | 2.405400 | 2.250100 |
| minimum_phase_19200 | 19200 | 1.080900 | 1.052000 |
| savgol_heavy_light_783 | 783 | 1.196500 | 1.166100 |
| find_peaks_96000 | 96000 | 1.289000 | 1.230500 |
| first_peak_index_96000 | 96000 | 1.702100 | 1.624200 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.101000 | 0.100000 |
| spline_k2_783 | 783 | 0.084300 | 0.083500 |
| spline_k3_783 | 783 | 0.120500 | 0.116100 |
| linregress_1000 | 1000 | 0.243500 | 0.230600 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 9.863000 | 9.636100 |
| resample_poly_96000_48k_44k1 | 96000 | 8.134300 | 7.960900 |
| nnresample_96000_48k_96k | 96000 | 1514.494800 | 1507.552000 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 12.430200 | 11.874300 |
| windows_32001 | 32001 / 19200 / 19200 | 1.217600 | 1.202100 |
| expit_783 | 783 | 0.003300 | 0.003200 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000277 | 0.000276 |

### Ratios

Each column uses its own contemporaneous Python and Rust pair, not a mixture of runs.

| Operation | Before Python/Rust | Final Python/Rust |
|---|---:|---:|
| convolve_full_ir_fir | 0.613 | 1.800 |
| convolve_same_estimate | 0.819 | 1.840 |
| correlate_full_30ms | 1.354 | 9.867 |
| rfft_irfft_96000 | 0.456 | 2.021 |
| magnitude_response_96000 | 0.525 | 3.508 |
| butter8_sosfilt_96000 | 1.032 | 1.415 |
| firwin2_19200 | 1.502 | 2.187 |
| minimum_phase_19200 | 0.401 | 1.269 |
| savgol_heavy_light_783 | 17.303 | 17.858 |
| find_peaks_96000 | 1.378 | 1.346 |
| first_peak_index_96000 | 1.756 | 1.158 |
| spline_k1_783_to_4800 | 1.477 | 1.378 |
| spline_k2_783 | 2.601 | 2.685 |
| spline_k3_783 | 2.479 | 2.975 |
| linregress_1000 | 133.333 | 135.278 |
| nnresample_design_147_160 | 0.985 | 2.026 |
| resample_poly_96000_48k_44k1 | 0.265 | 1.693 |
| nnresample_96000_48k_96k | 0.304 | 2.427 |
| spectrogram_295000_4800 | 1.244 | 2.951 |
| windows_32001 | 1.156 | 1.988 |
| expit_783 | 1.222 | 1.138 |
| next_fast_len_1e6 | 0.600 | 27.700 |

## 4. Optimizations and causes

1. **FFT plan and buffer allocation.** Per-call planners were replaced with thread-local real and complex planners. Follow-up work reuses real input, spectrum and scratch buffers, retaining their maximum capacity per thread. Returned output vectors still belong to callers. Immutable inputs still require a copy for destructive real transforms. A consuming internal `rfft_owned` avoids that copy for fresh padding. Convolution multiplies its spectrum in place and avoids copying full-mode output. Final full convolution fell from 6.437400 to 2.172600 ms, same convolution from 31.982100 to 14.431500 ms, and FFT round trip from 3.269300 to 1.027000 ms. Spectrogram benefits from the shared FFT changes without modifying its module (9.906500 to 4.212600 ms). The cache trades retained per-thread memory for less allocation; this primitive audit did not measure peak RSS.
2. **Polyphase dot products.** Previously every multiply recomputed a strided tap index and updated one dependent accumulator. Coefficients are now reversed into contiguous phase vectors once per call. Four independent f64 accumulators and a scalar remainder enable vectorization without unsafe code or intrinsics. Explicit polyphase fell from 30.316900 to 4.804300 ms; cached 48-to-96 kHz nnresample from 4975.661700 to 624.035800 ms. This changes summation order, covered by unchanged goldens and a new scalar-reference test.
3. **SOS section traversal.** Four sections now process each sample as a group with independent state, instead of four whole-buffer passes. Remaining sections retain the sectionwise implementation. Arithmetic inside each section is unchanged. Timing fell from 0.887900 to 0.647800 ms. The new test compares groups with the sectionwise algorithm for 1 through 9 sections, bit for bit.
4. **Minimum-phase transforms.** The real cepstrum and Hermitian synthesis use real transforms instead of full complex transforms. The sensitive full-spectrum log-floor calculation and pocketfft Nyquist correction remain. Timing fell from 2.808000 to 0.851500 ms. Isolated and composed goldens pass their existing budgets; composed errors are listed separately below.
5. **Magnitude calculation.** The initial sqrt(norm_sqr) change did not reliably meet the speed requirement. The follow-up reuses the transform spectrum and uses the exact identity `20 log10(|z|) = 10 log10(|z|^2)` for normal squared magnitudes, avoiding sqrt. Underflow, overflow and nonfinite cases use the original hypot-based expression. Final timing is 0.411900 ms versus the original 2.426800 ms. The new 96,000-sample comparison with the old expression has maximum error 1.42108547152020037e-14 dB.
6. **Kaiser symmetry.** The Bessel expression is evaluated on one half and mirrored, preserving symmetric/periodic indexing. The window bundle fell from 1.051900 to 0.612500 ms. Window symmetry and FFT caching also reduce genuinely uncached nnresample filter design from 10.017500 to 4.869400 ms.
7. **Fast-length early return.** Already-235-smooth inputs return after factoring rather than enumerating candidates. The original row lost to Python; final timing is about 10 ns per pair, but the baseline clock resolution prevents a precise before/after speedup claim. Existing minimality and overflow tests pass.

No changes were needed in spline solving (already banded), Savitzky-Golay, peak finding, expit, or convolution method selection. Their changes in measured timing are not claimed as code optimizations.

### Remaining ratios below 1.5

No final row is below 1.0. Rows below 1.5 were inspected as required:

- SOS (1.415): feedback dependencies remain after grouped traversal; no legal parallel primitive work was introduced.
- Minimum phase (1.269): the full-spectrum log-floor and Nyquist calculation remains to protect numerical parity; output and transform work still cost time.
- Find peaks (1.346): branch-heavy plateau scanning and index-vector allocation dominate. No rewrite was made after inspection because the operation already passes.
- First peak (1.158): normalization, negation and two peak collections remain. It passes, but its final Rust time exceeds its baseline; no implementation change was made to this function, and between-run noise must not be described as an optimization.
- Linear spline (1.378): knot construction is already banded; each query still performs interval search. No spline rewrite was made.
- Expit (1.138): scalar exponential cost dominates; left unchanged under the packet's exception for trivial operations.

## 5. Iteration record and independent failure

The first worker's final magnitude result was Rust 1.048000 / Python 1.152700 ms. The parent then measured Rust 1.041800 / Python 0.707800 ms (minima 0.982600 / 0.697200), ratio 0.679. This was a real audit failure, not discarded as noise. A second worker's pre-fix full run likewise failed at Rust 1.084700 / Python 1.064300 ms.

After buffer reuse and the power-dB identity, two separate focused paired measurements gave:

| Round | Operation | Rust median ms | Rust min ms | Python median ms | Python min ms | Ratio |
|---|---|---:|---:|---:|---:|---:|
| 1 | rfft_irfft_96000 | 1.026600 | 0.960900 | 2.106700 | 1.882800 | 2.052 |
| 1 | magnitude_response_96000 | 0.571900 | 0.546100 | 1.092700 | 1.022500 | 1.911 |
| 2 | rfft_irfft_96000 | 0.978300 | 0.943900 | 1.510500 | 1.453600 | 1.544 |
| 2 | magnitude_response_96000 | 0.615200 | 0.559700 | 1.088100 | 1.059900 | 1.769 |

The follow-up worker's full run passed all 22 rows; magnitude was 0.393400 / 1.154000 ms. The final parent run in section 3 then independently passed all 22 rows. Full-run and focused numbers differ; all are reported without combining their medians into one ratio.

## 6. Numerical regression results

Golden tolerances and fixtures were not edited. The following maxima were printed by the existing diagnostics during the worker's golden runs. FFT-buffer reuse was additionally tested against fresh plans and is bit-identical. The arithmetic-changing optimizations were checked against all primitive and FR golden suites; selected release-profile suites also passed.

| Affected output family | Maximum absolute error |
|---|---:|
| RFFT components | 4.26325641456060112e-14 |
| IRFFT samples | 2.96984659087229375e-15 |
| Convolution samples | 5.50670620214077644e-14 |
| Correlation samples | 5.95079541199083906e-14 |
| P03 magnitude response (dB) | 1.42108547152020037e-13 |
| SOS with frozen coefficients | 0 |
| Butter + SOS chain | 2.86370926971812878e-12 |
| Kaiser taps | 1.22124532708767219e-15 |
| Kaiser sums | 1.23691279441118240e-10 |
| FIRWIN2 taps | 1.81299419921288063e-13 |
| Isolated minimum-phase taps | 4.09394740330526474e-16 |
| Isolated minimum-phase spectrum (dB) | 1.05883155827544476e-12 |
| Isolated minimum-phase deep-bin absolute error | 2.22044604925031308e-16 |
| Resample-design taps | 2.44249065417534439e-15 |
| Resample-design cutoff/beta/null index | 0 |
| Explicit polyphase impulse/sine | 1.21014309684142063e-14 |
| Explicit polyphase demo | 2.01661604082303825e-17 |
| Composed nnresample impulse/sine | 1.49880108324396133e-14 |
| Composed nnresample demo | 2.49366499671666020e-17 |
| Polyphase edge cases | 0 |
| nnresample DC | 1.53210777398271603e-14 |
| Spectrogram power | 3.46944695195361419e-17 |
| Spectrogram axes | 0 |
| Composed P09 magnitude | 9.94759830064140260e-13 |
| P08 magnitude (read-only regression test) | 1.03455022326670587e-11 |
| Reused FFT buffers versus fresh plans | 0 (bit-identical) |
| New power-dB identity versus old hypot expression, 96000 samples | 1.42108547152020037e-14 dB |

Composed P09 minimum-phase fixtures also pass, but their errors must not be confused with isolated primitive accuracy: maximum tap error 6.36810040253732446e-5, maximum spectral error 7.40302994188743568e-1 dB, and worst reported sub-20 kHz band error 3.080e-4 dB. These tests retain their pre-existing budgets. No claim of bit-identical Python output is made.

The first worker's detailed local diagnostics are at `C:/Users/32170336/AppData/Local/Temp/pa02-golden-errors.log` and `C:/Users/32170336/AppData/Local/Temp/pa02-release-goldens.log`; these temporary paths are supplemental, not repository deliverables.

## 7. Files changed and smoke hook

PA02-owned changes:

- `crates/impulcifer-dsp/Cargo.toml`: added `[[bench]] name = "perf", harness = false`. Preserved existing dependency edits by other work.
- `crates/impulcifer-dsp/benches/perf.rs`: shared 22-operation benchmark and tiny-size runner.
- `crates/impulcifer-dsp/tests/perf_smoke.rs`: `bench_smoke_impulcifer_dsp` imports and runs the benchmark module.
- `crates/impulcifer-dsp/tests/properties_perf.rs`: six properties covering cache reuse, threads, inverse padding, grouped SOS, scalar polyphase parity and dB range behavior.
- `crates/impulcifer-dsp/src/{fft,conv,filters,fir,resample,windows}.rs`: optimizations described above.
- `tests/migration/bench_oracle_impulcifer_dsp.py`: matching CPython 3.14 oracle benchmark and environment output.
- `docs/rust/perf/impulcifer-dsp.md`: this report.

No PA02 changes to public signatures, regular dependencies, other crates, `features.toml`, golden exporters/fixtures, or excluded modules/tests. No unsafe, intrinsics, f32 numeric path or primitive parallelism was introduced. Existing unrelated edits in Cargo.lock, DSP Cargo.toml, IO, types and in-flight DSP modules were preserved.

## 8. Commands and verification output

All commands ran in the foreground. Commands below were run from `E:/Impulcifer`; workers also used the equivalent explicit `--manifest-path E:/Impulcifer/Cargo.toml` form.

```text
cargo bench -p impulcifer-dsp --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_dsp.py
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
```

The parent independently ran the exact six commands after each worker returned; the final complete output tables are in section 3. Final formatting and clippy exited 0. Test summary output, with the suite labels added for readability:

```text
lib:                     test result: ok. 5 passed; 0 failed
golden_batch1:           test result: ok. 19 passed; 0 failed
golden_batch2:           test result: ok. 12 passed; 0 failed
golden_batch3:           test result: ok. 7 passed; 0 failed
golden_brir_objects:     test result: ok. 15 passed; 0 failed
golden_fr:               test result: ok. 12 passed; 0 failed
perf_smoke:              test result: ok. 1 passed; 0 failed
properties_batch1:       test result: ok. 7 passed; 0 failed
properties_batch2:       test result: ok. 5 passed; 0 failed
properties_batch3:       test result: ok. 7 passed; 0 failed
properties_brir_objects: test result: ok. 11 passed; 0 failed
properties_fr:           test result: ok. 6 passed; 0 failed
properties_perf:         test result: ok. 6 passed; 0 failed
DSP doc-tests:           test result: ok. 0 passed; 0 failed
policy gates:            test result: ok. 4 passed; 0 failed
```

Total final DSP tests: 113 passed, 0 failed. Policy: 4 passed, 0 failed. Counts grew during the audit because other workers added read-only BRIR tests.

Additional foreground commands used during the audit:

```text
cargo test -p impulcifer-dsp --test golden_batch1 --test golden_batch2 --test golden_batch3 --test golden_fr -- --nocapture
cargo test -p impulcifer-dsp --release --test golden_batch1 --test golden_batch2 --test golden_batch3 --test golden_fr --test properties_perf -- --nocapture
cargo test -p impulcifer-dsp --test golden_batch1 -- --nocapture
cargo test -p impulcifer-dsp -- --nocapture
py -3.14 -m py_compile E:/Impulcifer/tests/migration/bench_oracle_impulcifer_dsp.py
```

All passed. The parent additionally reran the four release golden suites plus `properties_perf` after the follow-up: 56 tests passed. Its first final `git diff --check` found an extra blank line at the end of the PA02 bench section in Cargo.toml; the parent removed that line and reran the check successfully before the release tests. Narrowed benchmarks used the same bench commands with `PA02_FILTER` set to operation-name substrings (`96000`, `rfft_irfft`, `magnitude_response`), without changing inputs or repetition rules. Owned-file `git diff --check` passed.

During an intermediate polyphase iteration, `golden_decay_adjustment_matches_python` in the excluded `tests/golden_brir_objects.rs` failed at line 41 with `Option::unwrap()` on `None`. The worker did not edit it. At that point the packet's permitted fallback was used:

```text
cargo test -p impulcifer-dsp --test golden_batch1 --test golden_batch2 --test golden_batch3 --test golden_fr --test properties_batch1 --test properties_batch2 --test properties_batch3 --test properties_fr
cargo test -p impulcifer-dsp --lib
cargo test -p impulcifer-dsp --test perf_smoke
cargo test -p impulcifer-policy
```

Those passed. Later full runs, including the final independent run, passed without fallback. The workers ran DSP and policy regression checks after optimization iterations; no failed golden was accepted by increasing its tolerance.

## 9. Exclusions and remaining work

- No measured operation remains slower than Python in the final run.
- Registry status remains unchanged by design; the parent/integration owner can cite the provided smoke test when updating `perf.impulcifer-dsp` outside this packet.
- Pipeline-wide parallelism, M2/M5 peak memory, release checks and other platforms are outside PA02.
- No commits, pushes, CHANGELOG, README or version edits were made, because those files are outside this packet's allowed list.
- The Python environment-reporting code uses Windows `winreg`; this benchmark script was verified on the requested Windows machine, not as a cross-platform benchmark runner.

API reference checked during implementation: https://docs.rs/realfft/latest/realfft/trait.RealToComplex.html
