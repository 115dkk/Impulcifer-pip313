# PA01: impulcifer-io performance audit

Date: 2026-09-07. Baseline revision: `30ef27e` plus the pre-existing working tree. Only the allowed impulcifer-io files were modified by this audit. Two sequential ASTRA workers implemented and measured the optimizations; the parent reviewed the diff and independently reran all final commands in the foreground.

**Result: all nine requested operations pass `python_median / rust_median >= 1.0` on this machine.** Goldens and additional correctness tests pass unchanged. This is a warmed, local Windows result, not a cross-platform or cold-start guarantee. `features.toml` was deliberately not edited because another worker owns it.

## 1. Environment and method

- CPU: Intel Core i5-12600KF, 10 physical cores, 16 logical processors.
- OS: Windows 11 Education, 10.0.22621, x86-64.
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`, Cargo bench release profile, no added target-specific compiler flags.
- Primary oracle, both baseline and final: CPython 3.13.3, MSC v.1943 AMD64; NumPy 2.4.6; SciPy 1.18.0; soundfile 0.13.1; libsndfile 1.2.2.
- NumPy BLAS/LAPACK from `numpy.show_config()`: scipy-openblas 0.3.31.188.0, `USE64BITINT DYNAMIC_ARCH NO_AFFINITY Haswell MAX_THREADS=24`, msvc 19.44.35226, baseline X86_V2, detected X86_V3.
- FFT: NumPy/SciPy pocketfft. None of these I/O operations invokes FFT.
- `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `BLIS_NUM_THREADS`, `NUMEXPR_NUM_THREADS`, `RAYON_NUM_THREADS`: all unset. No thread pinning or Python thread changes.
- Additional worker cross-check: CPython 3.14.5, NumPy 2.5.3, SciPy 1.18.1, soundfile 0.14.0, libsndfile 1.2.2, OpenBLAS 0.3.34.106.0. All nine ratios passed there too; the acceptance tables below use 3.13 consistently.

Each operation has three untimed warm-up calls and eleven timed calls, reporting median and minimum milliseconds. No call exceeded five seconds, so no repetition reduction was needed. Inputs, fixtures, shape discovery, and correctness checks are outside the timer. Whole calls include file open/close and output allocation. Destruction of returned arrays is outside the timer on both sides. Filename operations collect 10,000 results on both sides.

All sample arrays are track-major f64. LCG state starts at 7 for each shape, proceeds in track-major order, and uses `state = (1664525 * state + 1013904223) mod 2^32`, `sample = state / 2^31 - 1`. Both implementations check the entire generated input and every read/round-trip output against the same little-endian f64 reference, bit for bit, before timing. The optional printed checksum is the sum of f64 bit patterns modulo 2^64; it is not the equality test.

```text
input32             e319203dd6c00000
input30             303c8e22c4000000
input2              1f284819f3400000
pcm32-expected      e319203dd6c00000
sweep-expected      bce785f884800000
demo-expected       4c9f490000000000
float32-expected    00e55e6500000000
roundtrip-expected  303c8e22c4000000
```

The Python harness creates and removes a system temporary directory, writes the float32 WAV with soundfile, and synchronously runs Rust against exactly those files. Standalone `cargo bench` creates its own system temporary directory and invokes the script's `--fixture-only` mode synchronously. No audio fixture or benchmark output is written into the repository. Normal Rust target build artifacts remain under Cargo's configured target directory.

### Actual 2.x oracle details

- `core.audio_io.write_wav` receives contiguous track-major arrays, matching `HRIR.write_wav`'s `np.vstack` output. The wrapper transposes them for soundfile; that conversion belongs to the timed call.
- `core.audio_io.read_wav(expand=True)` returns a transposed view of soundfile's interleaved array. Rust returns independent track vectors. Python is not forced to make an extra copy for benchmark parity.
- The actual `core.sweep_signal._quantize_like_bundled_wav` uses `BytesIO`, `sf.write(..., subtype="PCM_32")`, `sf.read`, and `np.ascontiguousarray(quantized.T)`. It is not a direct `np.int32` cast. The harness calls the actual function.
- A filename regex exists: `core.recording_progress._SEGMENTED_SWEEP_RE.search`. It extracts speakers/layout/duration; the Rust parser also validates rate, bit depth, frequency fields, and speaker names. This comparator is therefore not semantically identical, as disclosed by both benchmark output and this report. It does not establish a full-parser speed comparison.
- The bundled sweep is actually 1 x 295,270 samples; `data/demo/FL,FR.wav` is 2 x 878,540. The requested synthetic two-track write remains 2 x 295,000.

## 2. Before and after

### Baseline output (verbatim rows)

| op | size | rust median ms | rust min ms |
| write_pcm32_32tracks | 32x96000 | 54.550200 | 53.662700 |
| write_pcm32_30tracks | 30x96000 | 51.009500 | 50.122900 |
| write_pcm16_2tracks | 2x295000 | 10.248100 | 10.071600 |
| read_pcm32_32tracks | 32x96000 | 17.239900 | 16.853300 |
| read_bundled_sweep | 1x295270 | 1.314000 | 1.224000 |
| read_demo_recording | 2x878540 | 9.615500 | 9.250400 |
| read_float32_8tracks | 8x480000 | 23.483600 | 23.219100 |
| pcm32_round_trip | 30x96000 | 48.996400 | 48.092000 |
| sweep_file_name_parse | 10000 | 3.778300 | 3.735200 |

| op | size | python median ms | python min ms |
| write_pcm32_32tracks | 32x96000 | 23.496500 | 22.654800 |
| write_pcm32_30tracks | 30x96000 | 21.970000 | 21.474700 |
| write_pcm16_2tracks | 2x295000 | 5.657100 | 5.408000 |
| read_pcm32_32tracks | 32x96000 | 7.044600 | 6.981400 |
| read_bundled_sweep | 1x295270 | 0.688100 | 0.611300 |
| read_demo_recording | 2x878540 | 3.819200 | 3.696000 |
| read_float32_8tracks | 8x480000 | 9.338300 | 9.065000 |
| pcm32_round_trip | 30x96000 | 43.918800 | 40.732600 |
| sweep_file_name_parse | 10000 | 4.078500 | 3.921100 |

| op | python/rust median |
| write_pcm32_32tracks | 0.430732 |
| write_pcm32_30tracks | 0.430704 |
| write_pcm16_2tracks | 0.552015 |
| read_pcm32_32tracks | 0.408622 |
| read_bundled_sweep | 0.523668 |
| read_demo_recording | 0.397192 |
| read_float32_8tracks | 0.397652 |
| pcm32_round_trip | 0.896368 |
| sweep_file_name_parse | 1.079454 |

### Final paired output, independently rerun by parent (verbatim rows)

| op | size | rust median ms | rust min ms |
| write_pcm32_32tracks | 32x96000 | 10.904700 | 10.244000 |
| write_pcm32_30tracks | 30x96000 | 9.221500 | 8.282300 |
| write_pcm16_2tracks | 2x295000 | 1.627300 | 1.462600 |
| read_pcm32_32tracks | 32x96000 | 5.536200 | 5.088200 |
| read_bundled_sweep | 1x295270 | 0.427100 | 0.391400 |
| read_demo_recording | 2x878540 | 2.823800 | 2.622600 |
| read_float32_8tracks | 8x480000 | 6.551100 | 6.235600 |
| pcm32_round_trip | 30x96000 | 9.437900 | 7.977900 |
| sweep_file_name_parse | 10000 | 3.226300 | 3.074900 |

| op | size | python median ms | python min ms |
| write_pcm32_32tracks | 32x96000 | 24.643300 | 22.917400 |
| write_pcm32_30tracks | 30x96000 | 22.822600 | 21.881700 |
| write_pcm16_2tracks | 2x295000 | 5.702400 | 5.499700 |
| read_pcm32_32tracks | 32x96000 | 7.530900 | 7.378800 |
| read_bundled_sweep | 1x295270 | 0.665100 | 0.631200 |
| read_demo_recording | 2x878540 | 4.403200 | 4.107400 |
| read_float32_8tracks | 8x480000 | 10.271300 | 9.220000 |
| pcm32_round_trip | 30x96000 | 44.350000 | 42.796600 |
| sweep_file_name_parse | 10000 | 3.857500 | 3.753100 |

| op | python/rust median |
| write_pcm32_32tracks | 2.259879 |
| write_pcm32_30tracks | 2.474934 |
| write_pcm16_2tracks | 3.504209 |
| read_pcm32_32tracks | 1.360301 |
| read_bundled_sweep | 1.557247 |
| read_demo_recording | 1.559317 |
| read_float32_8tracks | 1.567874 |
| pcm32_round_trip | 4.699139 |
| sweep_file_name_parse | 1.195642 |

The independent standalone `cargo bench` run immediately before the paired run also completed:

```text
| op | size | rust median ms | rust min ms |
| write_pcm32_32tracks | 32x96000 | 10.805300 | 10.447800 |
| write_pcm32_30tracks | 30x96000 | 8.793200 | 8.424100 |
| write_pcm16_2tracks | 2x295000 | 1.478100 | 1.404400 |
| read_pcm32_32tracks | 32x96000 | 5.334900 | 5.030800 |
| read_bundled_sweep | 1x295270 | 0.498800 | 0.400400 |
| read_demo_recording | 2x878540 | 2.854600 | 2.649000 |
| read_float32_8tracks | 8x480000 | 6.328400 | 6.208200 |
| pcm32_round_trip | 30x96000 | 9.954800 | 8.391600 |
| sweep_file_name_parse | 10000 | 3.119200 | 3.053100 |
```

## 3. Profiling, changes, and correctness

Profiling used source-level reasoning followed by release measurements of implementation variants. No profiler was installed. Every baseline operation was below 1.5 and was investigated. No public signatures or dependencies changed; no unsafe, SIMD intrinsics, or reduced-precision arithmetic were added. The existing float32 file-format decode still widens directly to f64: binary32 is an input encoding, not a new arithmetic path.

### Writes and PCM32 round trip

The original writer performed one `write_all` and runtime width selection per sample. A 64 KiB reusable block and const-width PCM16/24/32 encoding dispatch remove those per-sample I/O calls. The default BufWriter now mostly receives whole blocks, so changing its capacity is unnecessary. Contrary to the initial suspect list, the original writer did not build an unreserved whole-file Vec.

The original quantizer called `round_ties_even` for each sample on the baseline target. It now saturates the scaled value to i32 range, then adds/subtracts signed 2^52 to round to integer precision with ties to even. Multiplication, saturation, and final conversion remain f64/i32. Original nonfinite-input validation is retained. Rust timings changed from 54.5502 to 10.9047 ms (32-track write), 51.0095 to 9.2215 ms (30 tracks), 10.2481 to 1.6273 ms (PCM16 stereo), and 48.9964 to 9.4379 ms (round trip). These are combined effects, not isolated attribution to each edit.

Correctness: `golden_write_wav_bytes_match_soundfile` requires exact bytes (zero differing bytes); `golden_pcm32_round_trip_matches_python` passes unchanged. Shared full-size round-trip comparison has max absolute error 0. Added random finite f64 values and ties-to-even boundaries match the original expression exactly in both debug and release. PCM16/24 truncation and saturation remain unchanged.

### Reads

The original reader called `read_exact` and matched the encoding for every sample. Encoding dispatch now occurs once per file, with 256 KiB reads and 256-frame cache tiles, const-width conversion, and capacity-reserved tracks. PCM24 still assembles three bytes and sign-extends, but no longer pays sample-level I/O or encoding-dispatch costs. Known channel counts have constant frame widths, allowing fixed-array indexing for eight or more channels. Generic channel layouts retain the streaming implementation.

After the first block-I/O implementation, 32-track reading was still 8.5420 ms against Python's 7.4974 ms (ratio 0.877710), so that state was not accepted. Python's interleaved allocation/transposed view avoids Rust's independent-track transpose costs.

For 30/32-channel files with at least 32,768 frames, four scoped standard-library threads now decode disjoint output track groups. Each allocates its own output tracks. Encoded input capacity is reused per calling thread, with a 16 MiB retention cap; every call rereads the file. Workers are joined before returning and no detached execution occurs. Final 32-track read is 5.5362 ms against 7.5309 ms (ratio 1.360301). The worker also recorded final-code repeated reads of 5.2530, 5.6425, and 5.6746 ms. This operation remains below the 1.5 investigation threshold but passes the mandatory 1.0 criterion after optimization.

Other read timings: bundled mono 1.3140 to 0.4271 ms; demo stereo 9.6155 to 2.8238 ms; float32 eight-track 23.4836 to 6.5511 ms. Full pre-zeroed output, intermediate interleaved decoded arrays, stack transpose groups, 16 KiB/1 MiB read variants, and two/eight worker variants were measured and rejected when slower. No claim of isolated causal speedup is based on noisy individual runs.

Correctness: `golden_read_wav_matches_python`, `golden_read_riff_rf64_extensible_pcm_and_float`, and byte-exact full-size PCM32/sweep/demo/float comparisons pass. Max absolute error for the finite compared sample outputs is 0. Existing RIFF/RF64 chunk validation is unchanged. New tests compare scalar PCM conversion at 32,767/32,768/32,769 frames, cover all three PCM depths, float32/64, signed zeros/infinities, trailing nonaudio chunks, and truncated files. No floating-point sums were reordered; no golden tolerance changed.

### Filename parser

Replaced `split('-').collect::<Vec<_>>()` with a checked iterator, preserving rejection of missing or excess fields. Rust timing changed from 3.7783 to 3.2263 ms. The final ratio is 1.195642; the extra temporary allocation was removed, while required owned strings and validation remain. Filename/segment tests pass unchanged; numerical max error is not applicable to the parser.

## 4. Remaining limitations

- No requested operation remains below 1.0 in the final paired run.
- Warm-up intentionally amortizes the reusable input-buffer allocation. Cold first-call performance was not measured separately.
- Large 30/32-channel reads temporarily hold the entire encoded data chunk, plus decoded output. After return, up to 16 MiB of encoded-buffer capacity may remain per calling thread. The cap limits retained capacity, not peak memory during a large RF64 decode. Peak RSS was not measured.
- The four per-call threads can oversubscribe a caller already parallelizing multiple files. Full pipeline performance and multi-file concurrency remain for the required pipeline audit.
- Other workers were active on this machine. Repeat runs are reported to avoid treating a single favorable sample as definitive. No claim is made for Linux/macOS, different CPUs, or free-threaded Python.
- The parser comparison uses the actual partial Python regex, not an equivalent full validator.
- The benchmark prints ratios but does not fail its process solely for a ratio below 1.0. Timing acceptance requires inspecting its table; correctness failures do fail the run.

## 5. Files changed

- `crates/impulcifer-io/Cargo.toml`: only `[[bench]] name = "perf", harness = false`.
- `crates/impulcifer-io/benches/perf.rs`: std-only timings, temporary fixtures, equality checks, reusable tiny smoke runner.
- `crates/impulcifer-io/src/wav.rs`: block conversion, ties-to-even implementation, parallel large-layout decode and bounded retained buffer.
- `crates/impulcifer-io/src/sweep_files.rs`: remove parser temporary Vec.
- `crates/impulcifer-io/tests/perf_smoke.rs`: smoke and five conversion/dispatch property tests.
- `tests/migration/bench_oracle_impulcifer_io.py`: actual Python oracles, deterministic input, shared float fixture, environment and paired reporting.
- `docs/rust/perf/impulcifer-io.md`: this report.

No edits to `features.toml`, other crates, golden exporters/fixtures, migration README, CHANGELOG, or dependency lists. No commit or push.

## 6. Registry hook and verification

Registry test reference: `impulcifer-io::bench_smoke_impulcifer_io` for `perf.impulcifer-io`.

The smoke test includes the actual bench source and runs all nine operations at tiny sizes, without requiring Python. Synthetic lengths are 17/19 frames, the float fixture is 8 x 23, and parsing runs three times. Real sweep/demo reads use the tiny PCM fixture in smoke mode; full benchmarking uses the real repository input files read-only.

Exact final commands, independently run from `E:/Impulcifer`, all foreground:

```text
cargo bench -p impulcifer-io --bench perf
python E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
cargo fmt -p impulcifer-io -- --check
cargo clippy -p impulcifer-io --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io
cargo test -p impulcifer-policy
cargo test -p impulcifer-io --release
git diff --stat -- crates/impulcifer-io
git diff --check -- crates/impulcifer-io
git diff -- crates/impulcifer-io/Cargo.toml crates/impulcifer-io/src/sweep_files.rs
```

Workers used the same test commands with `--manifest-path E:/Impulcifer/Cargo.toml` after each optimization and repeated `python E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py`. Additional worker verification used `py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py`. Rust formatting edits used `cargo fmt -p impulcifer-io`. An initial clippy `new_without_default` finding in the benchmark helper was resolved by making its constructor crate-visible; the final run has no warnings.

Final test output excerpts (verbatim):

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s

running 6 tests
test bench_smoke_impulcifer_io ... ok
test quantization_matches_round_ties_even_at_boundaries ... ok
test float_blocks_preserve_ieee_widening_and_special_values ... ok
test block_io_matches_scalar_conversion_all_depths_and_layouts ... ok
test parallel_float_decode_preserves_samples_and_ignores_trailing_chunks ... ok
test parallel_pcm_decode_matches_scalar_at_dispatch_boundary ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.71s

running 4 tests
test every_crate_root_forbids_unsafe ... ok
test canonical_features_registered ... ok
test no_unsafe_outside_budget ... ok
test implemented_features_have_existing_tests ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.99s

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
```

`cargo fmt --check` emitted no output and exited 0. All commands exited 0. `git diff --check` reported no whitespace errors, only Git's existing LF-to-CRLF notices. No tests were skipped. All doc-test runs contained zero tests.

## 7. Anything undone

The local audit, implementation, smoke hook, report, and requested verification are complete. Registry registration is intentionally left to its owning worker, and no CI/PR/commit operations were requested or run. Cold-start, peak-memory, multi-file, and cross-platform performance are not covered by this packet's measurements.

## 8. Rerun against the newest CPython (parent, 2026-09-07)

The audit above ran its Python side with the interpreter first on PATH (3.13.3). The rule was tightened while it ran: the opponent is Impulcifer 2.x on the newest CPython. The parent reran `tests/migration/bench_oracle_impulcifer_io.py` unchanged with `py -3.14` (CPython 3.14.5, NumPy 2.5.3, SciPy 1.18.1, soundfile 0.14.0, libsndfile 1.2.2) and with the free-threaded venv (CPython 3.14.7 free-threading build, same packages). The script also reruns the Rust bench in the same process, so each table is a paired run.

| op | rust median ms (paired with 3.14) | python 3.14.5 median ms | ratio | rust median ms (paired with 3.14t) | python 3.14.7t median ms | ratio |
|---|---|---|---|---|---|---|
| write_pcm32_32tracks | 11.6177 | 26.3034 | 2.26 | 11.0866 | 23.4126 | 2.11 |
| write_pcm32_30tracks | 9.3021 | 24.4579 | 2.63 | 9.0356 | 22.3444 | 2.47 |
| write_pcm16_2tracks | 1.5909 | 6.4447 | 4.05 | 1.6557 | 5.5071 | 3.33 |
| read_pcm32_32tracks | 5.7793 | 9.8067 | 1.70 | 5.9545 | 7.7050 | 1.29 |
| read_bundled_sweep | 0.4993 | 0.7325 | 1.47 | 0.5539 | 0.6518 | 1.18 |
| read_demo_recording | 3.8319 | 4.8388 | 1.26 | 3.1590 | 4.7121 | 1.49 |
| read_float32_8tracks | 7.7574 | 10.1582 | 1.31 | 6.6967 | 9.3329 | 1.39 |
| pcm32_round_trip | 9.6460 | 46.2008 | 4.79 | 9.5105 | 42.6555 | 4.49 |
| sweep_file_name_parse | 3.2734 | 4.2159 | 1.29 | 3.1363 | 4.0283 | 1.28 |

Every operation stays above 1.0 against both interpreters; the smallest margins are the single-track and stereo reads (1.18 to 1.26), where the work is dominated by the page cache and libsndfile is already a C loop. Peak memory was not measured here (that belongs to the M2 pipeline audit); note that the large-file reader keeps up to 16 MiB of encoded-buffer capacity per calling thread after a call.
