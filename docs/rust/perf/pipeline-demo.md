# PA03: demo pipeline time and memory audit

Date: 2026-09-08. Registry: `perf.pipeline-demo`. Test: `impulcifer-service::bench_smoke_pipeline_demo`.

## Verdict

The current Rust implementation passes the numerical runtime threshold for both demo scenarios against CPython 3.14.5 and 3.14.7t. No DSP optimization was needed or performed. All whole-run ratios exceed 1.5, including separate in-process comparisons.

**M2 full-output completion is not established.** Python generates `plots/headphones.png` and `plots/results.png`; Rust does not. Python plotting remained enabled. Rust `brir/run.rs` explicitly logs `cli_plots_not_available_yet`. This audit measures the implementations as they exist, not identical output workloads. Adding plotting would be feature work outside this optimization-only packet. The registry remains `planned`, with the smoke test attached for subsequent completion. Do not advertise these ratios as performance for identical full-output workloads.

## Environment

- CPU: Intel Core i5-12600KF, 10 physical cores, 16 logical processors.
- OS: Windows 11 build 22621, x86_64.
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`, LLVM 22.1.6, `x86_64-pc-windows-msvc`.
- Cargo: `1.97.0 (c980f4866 2026-06-30)`.
- CPython 3.14.5: `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`.
- CPython 3.14.7 free-threading build: `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`.
- Both interpreters: NumPy 2.5.3, SciPy 1.18.1, psutil 7.2.2.
- NumPy BLAS/LAPACK: scipy-openblas 0.3.34.106.0, `USE64BITINT DYNAMIC_ARCH NO_AFFINITY Haswell MAX_THREADS=24`.
- FFT: NumPy/SciPy pocketfft; SciPy default FFT workers: 1.
- `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `BLIS_NUM_THREADS`, `NUMEXPR_NUM_THREADS`, `RAYON_NUM_THREADS`: unset. No thread limits were imposed.

Only psutil was installed into the two permitted Python environments. No Rust dependencies were added. Other workers were editing CLI, Python bindings and recovery code in this shared tree. This audit did not modify their files; consequently these measurements are not from an immutable revision or an otherwise guaranteed idle machine.

Observed Python EQ submits 14 tasks with `max_workers=14`: `ProcessPoolExecutor` on regular Python and `ThreadPoolExecutor` on free-threaded Python. A separate seven-task operation explicitly uses threads on both. Five descendants were observed during regular Python runs; configured worker count is not the same as the number the sampler observes.

Rust is not strictly one operating-system process: README timestamp formatting launches PowerShell. The free-threaded venv executable is also a small launcher with the actual interpreter as a child. Both details are included in process-tree memory measurements.

## Method and scope

- Both scenarios use fresh temporary copies of canonical demo inputs, the absolute bundled `sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`, actual defaults, and virtual bass at 250 Hz when selected.
- Rust: one warmup, five measurements; report median and minimum.
- Each Python interpreter and mode: one warmup, three measurements.
- Rust driver uses `ImpulciferService`, `NoopHost`, `JobRegistry`, and the real `start_brir` implementation. It waits for terminal success after output writes and checks expected output files.
- Python CLI time covers the child process. Separate child invocations import the entry module, then time `impulcifer.main(**params)`, retaining lazy work, actual plots and cleanup.
- Rust service time starts at the service call and ends at observed job success. Rust whole-process time additionally includes its internal input copy, setup and temporary-output cleanup. Python's input copy is outside the timer. This asymmetry penalizes Rust; the inner timers are also reported.
- The orchestrator samples process memory with psutil and waits 50 ms between samples. Whole-process elapsed values include sampling and child-reaping overhead. The last sampling pass occurs before stopping the timer. They are observed wall-clock bounds, not precise kernel process-lifetime measurements.
- The final default Rust process median was 1598.8553 ms, versus 1159.9819 ms inside the service. The worker's preceding process median was 1232.1771 ms with 1163.0692 ms inside. Do not interpret this process-only variation as a DSP regression or optimization effect; it includes setup/cleanup, sampling and environmental variation.
- Children are benchmarked sequentially; compilation is excluded. This script is currently Windows-specific for interpreter discovery and the `.exe` release target, even though RSS sampling has a non-Windows fallback.

Python produced `README.md`, `headphone-responses.wav`, `hesuvi.wav`, `hrir.wav`, `responses.wav`, `room-responses.wav`, and the two PNG files. Rust produces the WAV/README workload without those PNG files. Audio golden parity does not establish image parity.

## Before and after

All times are milliseconds. There were **no runtime optimizations between baseline and final verification**. These are repeat-run measurements, not claimed optimization gains. A zero inner time in the verbatim CLI output means no inner timer exists, not zero execution time.

### Baseline

| scenario | side | mode | process median ms | process min ms | inner median ms |
|---|---|---|---:|---:|---:|
| default | rust | service | 1288.017800 | 1269.855000 | 1218.214900 |
| default | python314 | cli | 13133.382300 | 13081.040700 | n/a |
| default | python314 | inprocess | 11381.614200 | 11062.594600 | 9604.074200 |
| default | python314t | cli | 6299.303900 | 5928.927300 | n/a |
| default | python314t | inprocess | 6578.694900 | 6047.035100 | 4414.278100 |
| vbass | rust | service | 1271.520500 | 1251.847600 | 1206.706200 |
| vbass | python314 | cli | 13883.319500 | 13421.617000 | n/a |
| vbass | python314 | inprocess | 11484.641100 | 11372.898200 | 9623.737300 |
| vbass | python314t | cli | 5758.039700 | 5692.997900 | n/a |
| vbass | python314t | inprocess | 5959.106700 | 5726.936900 | 3901.816900 |

### Final parent verification (verbatim orchestrator table)

| scenario | side | mode | process median ms | process min ms | in-process median ms | root peak MiB | largest workload process peak MiB | sum historical peaks MiB | concurrent tree RSS MiB |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| default | rust | service | 1598.855300 | 1558.641900 | 1159.981900 | 242.371 | 242.371 | 300.766 | 259.652 |
| default | python314 | cli | 13431.047900 | 13060.795400 | 0.000000 | 495.426 | 495.426 | 1142.609 | 947.000 |
| default | python314 | inprocess | 12298.153700 | 11657.257300 | 10354.571400 | 495.172 | 495.172 | 996.531 | 798.227 |
| default | python314t | cli | 5859.093600 | 5833.157700 | 0.000000 | 4.316 | 521.621 | 525.934 | 525.879 |
| default | python314t | inprocess | 5930.965100 | 5861.589800 | 3921.138300 | 4.316 | 521.793 | 526.109 | 526.055 |
| vbass | rust | service | 1227.337900 | 1220.711500 | 1162.485400 | 232.980 | 232.980 | 295.902 | 263.844 |
| vbass | python314 | cli | 12993.005400 | 12977.384000 | 0.000000 | 495.816 | 495.816 | 1142.539 | 913.527 |
| vbass | python314 | inprocess | 11227.771600 | 11138.935100 | 9441.175500 | 495.379 | 495.379 | 995.539 | 762.070 |
| vbass | python314t | cli | 5846.961700 | 5758.188700 | 0.000000 | 4.324 | 521.449 | 525.770 | 525.715 |
| vbass | python314t | inprocess | 5830.532600 | 5777.847700 | 3851.300400 | 4.320 | 522.391 | 526.711 | 526.656 |

### Ratios

Ratio is Python median divided by Rust median. Inner ratio compares Python `main` with Rust service, using separate runs from the CLI comparison.

| scenario | comparator | baseline CLI/process | final CLI/process | baseline main/service | final main/service |
|---|---|---:|---:|---:|---:|
| default | python314 | 10.196584 | 8.400415 | 7.883727 | 8.926494 |
| default | python314t | 4.890696 | 3.664555 | 3.623563 | 3.380344 |
| vbass | python314 | 10.918675 | 10.586331 | 7.975212 | 8.121543 |
| vbass | python314t | 4.528468 | 4.763938 | 3.233444 | 3.312988 |

Standalone `cargo bench` during parent verification, verbatim:

```text
| scenario | rust median ms | rust min ms |
| default | 1162.476000 | 1134.730100 |
| vbass | 1171.182700 | 1152.894600 |
```

## Rust per-stage observations

Median milliseconds from service progress-event timestamps, with the actual 2.x keys. Final columns come from the parent's full orchestrator rerun.

| 2.x key | baseline default | final default | baseline vbass | final vbass |
|---|---:|---:|---:|---:|
| `cli_creating_estimator` | 126 | 119 | 126 | 117 |
| `cli_running_room_correction` | 377 | 345 | 355 | 346 |
| `cli_running_headphone_compensation` | 135 | 137 | 138 | 137 |
| `cli_creating_equalization` | 0 | 0 | 0 | 0 |
| `cli_creating_target` | 0 | 0 | 0 | 0 |
| `cli_opening_measurements` | 256 | 225 | 235 | 224 |
| `cli_cropping_responses` | 113 | 108 | 109 | 106 |
| `vbass_status_processing` | n/a | n/a | 12 | 12 |
| `cli_equalizing` | 18 | 19 | 16 | 16 |
| `cli_normalizing_gain` | 27 | 26 | 28 | 27 |
| `cli_plotting_results` | 0 | 0 | 0 | 0 |
| `cli_writing_brirs` | 181 | 170 | 177 | 175 |

These are event intervals at millisecond resolution, **not exclusive function timings**. Response stacking without a progress event contributes to the preceding interval. README analysis follows normalization; actual file writing occurs after the in-memory pipeline and contributes to the last interval. Target computation is deferred into the DSP pipeline. The plotting interval is a placeholder. Independently aggregated medians need not sum to the median total.

## Optimization assessment and numerical integrity

No measured whole-run ratio was below 1.5. Existing EQ already uses rayon for independent FIR construction, and the observed EQ interval was only 16-19 ms in final verification. The largest intervals were room correction, measurement loading, output writing, and headphone compensation. None justified changing already-audited PA02 primitives for this acceptance threshold. No allocation, convolution, summation-order or output-writer changes were made. No memory-driven optimization was attempted.

The worker's Python stage observations put regular-Python EQ at approximately 5.83-5.86 seconds versus 69-86 ms for free-threaded Python, consistent with the process-pool cost being a major difference. Result plotting alone took approximately 223-301 ms; headphone plotting is included in headphone compensation. These timings do not license removing either plot from the oracle.

The unchanged demo goldens passed both before and during parent verification. Existing recorded maximum absolute sample errors are 9.750947356e-7 (default) and 4.656612873e-8 (vbass), as reported by the worker's parity run. The parent independently reran the two passing parity tests, which do not print maximum errors on success. No tolerances, fixtures, numerical code or floating-point summation order changed.

## Memory interpretation

Values in the final table are maxima across measured repetitions, in MiB. Root/workload peaks use Windows `peak_wset` when accessible and RSS otherwise. Historical peak sum is the sum of each observed process's individual high-water value within one run, then the maximum across runs. Concurrent tree RSS is the maximum sampled sum, not a sum of independent historical peaks.

For CLI runs, free-threaded Python's concurrent tree RSS was **525.879 MiB default / 525.715 MiB vbass**, compared with regular Python's **947.000 / 913.527 MiB**. This is approximately 44.5% and 42.5% lower. Historical peak sums were approximately 526 MiB versus 1143 MiB. Thus thread-based execution used less whole-tree memory on this machine, although its actual interpreter process was larger (about 521.5 MiB versus 495.5 MiB). Interpreter builds, allocators, import costs and process startup also differ, so these numbers are not a controlled isolation of thread overhead. Rust's own process peaked at **242.371 / 232.980 MiB**, with tree RSS of **259.652 / 263.844 MiB** including its timestamp child. Missing Rust plots also prevent attributing the Rust/Python memory difference solely to threading.

The 3.14t root values of about 4.3 MiB belong to the venv launcher, not the running Python workload. The script's `workload_process_peak_bytes` is mechanically the largest observed process peak; process identity metadata in the raw rows establishes which process it represents in these runs.

Sampling retains stable `(pid, create_time)` identities and observed peaks after exit. A final sample is attempted after the root exits, but psutil may no longer expose its counters. Fast-exiting or reparented children can be missed. Sampling occurs every 50 ms plus the sampler's own overhead; tree RSS is gathered sequentially, not atomically. Historical peak sums are **not simultaneous peak memory** and neither kind of RSS sum represents unique physical pages across processes.

## Smoke test and changed files

`bench_smoke_pipeline_demo` runs default and vbass using `--fast` semantics: retain `FL,FR.wav`, the first canonical speaker file with the required FL reference, and retain all room/headphone inputs. Other speaker recordings are absent. Samples are not shortened and compensation/DSP stages are not bypassed. It checks terminal success, output presence, 48 kHz, 14 HeSuVi tracks and demo-scale frame length.

Audit-owned files:

- `crates/impulcifer-service/examples/demo_brir.rs`
- `crates/impulcifer-service/benches/perf.rs`
- `crates/impulcifer-service/tests/perf_smoke.rs`
- `crates/impulcifer-service/Cargo.toml` (only `[[bench]]` registration)
- `tests/migration/bench_oracle_pipeline.py`
- `docs/rust/perf/pipeline-demo.md`
- `features.toml` (parent integration of the explicitly requested test citation; status remains `planned`)

The general audit template prohibits worker edits to `features.toml`. The parent added only the citation requested by PA03. No files belonging to recording, recovery, CLI or Python-binding workers were modified. README user instructions do not change. No version bump, changelog edit, commit, push or PR was performed; this packet has a restricted file list and completion remains blocked by missing plots.

## Verification commands and output

All parent verification commands ran in the foreground with 600-second deadlines. The requested orchestrator command was run without arguments and redirected to `target/pa03-parent-verification.log`; the full service/DSP test output is in `target/pa03-parent-tests.log`.

```text
cargo build --release -p impulcifer-service --example demo_brir
    Finished `release` profile [optimized] target(s) in 0.88s

cargo bench -p impulcifer-service --bench perf
    Finished `bench` profile [optimized] target(s) in 0.22s
| scenario | rust median ms | rust min ms |
| default | 1162.476000 | 1134.730100 |
| vbass | 1171.182700 | 1152.894600 |

py -3.14 E:/Impulcifer/tests/migration/bench_oracle_pipeline.py
    Exit code 0; full verbatim summary table above.

cargo fmt -p impulcifer-service -p impulcifer-dsp -- --check
    Exit code 0; no output.

cargo clippy -p impulcifer-service -p impulcifer-dsp --all-targets -- --no-deps -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s

cargo test -p impulcifer-service -p impulcifer-dsp
    Aggregate of per-target results: 218 passed; 0 failed; 1 ignored.
    Ignored: recording_virtual_cable_end_to_end requires Windows CABLE-A hardware.

cargo test -p impulcifer-service --test demo_parity
running 2 tests
 test demo_vbass_matches_python_within_budget ... ok
 test demo_brir_matches_python_within_budget ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.63s

cargo test -p impulcifer-policy
running 4 tests
 test every_crate_root_forbids_unsafe ... ok
 test canonical_features_registered ... ok
 test no_unsafe_outside_budget ... ok
 test implemented_features_have_existing_tests ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.82s
```

After the registry citation was added, the parent reran policy: all four tests passed again (0.87s). Python compilation passed. `py -3.14 -m ruff` failed because that interpreter has no Ruff module; the already-installed `ruff check E:/Impulcifer/tests/migration/bench_oracle_pipeline.py` then passed with `All checks passed!`. No extra package was installed. `git diff --check` passed for the tracked audit changes. The worker also passed Python compilation and ruff for the new script. Its latest policy run had failed because another worker's Python-binding test lacked `#![forbid(unsafe_code)]`; this was resolved by the owning work before the parent's successful rerun. It is not a current blocker.

Foreground-discipline exception: the worker reported that a pip command was automatically backgrounded by its tool after a 120-second timeout despite not requesting background execution. It waited for completion before continuing, then used 600-second deadlines. This violated the requested discipline and is recorded rather than omitted. The parent used no background execution.

## Raw evidence and remaining work

Local evidence directories contain `environment.json`, `rows.json`, and per-child logs:

- Initial baseline: `C:/Users/32170336/AppData/Local/Temp/impulcifer-pa03-baseline-4x0av_ma/`
- Worker final run: `C:/Users/32170336/AppData/Local/Temp/impulcifer-pa03-baseline-cbg2py9z/`
- Parent final verification: `C:/Users/32170336/AppData/Local/Temp/impulcifer-pa03-baseline-k0z57f16/`

The no-argument script labels directories `baseline`, including final runs. These are local temporary evidence, not repository fixtures or durable CI artifacts. Parent logs are `target/pa03-parent-verification.log` and `target/pa03-parent-tests.log`.

Remaining completion requirement: implement and verify Rust's two default plot outputs under a feature packet, then rerun PA03 before changing `perf.pipeline-demo` to `implemented` or claiming M2 full-output completion. No measured timing operation currently fails the numerical speed threshold.
