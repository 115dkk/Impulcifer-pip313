# PA03 (ASTRA): end-to-end performance and memory audit of the demo BRIR run (`perf.pipeline-demo`, M2 close)

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` (method, hard rules, report format) with the additions below. Read first: `crates/impulcifer-service/src/brir/{run.rs,inputs.rs,outputs.rs}`, `crates/impulcifer-dsp/src/pipeline.rs`, `crates/impulcifer-dsp/src/stages/{equalize.rs,room.rs,headphone.rs}`, `crates/impulcifer-dsp/src/hrir.rs`, the audit reports `docs/rust/perf/impulcifer-dsp.md` and `docs/rust/perf/impulcifer-io.md`, `tests/migration/README-service.md`, and the 2.x entry point `E:/Impulcifer/impulcifer.py` + `core/pipeline.py` (what a default run does, including the process pool in `_stage_equalize` and the always-on `_stage_plot_results`).

Run every command in the foreground. Never use background execution. Other workers may be editing `crates/impulcifer-service/src/recording/**` and `crates/impulcifer-service/src/lib.rs`; treat those as read-only.

## What to measure
The whole default demo run and the `--vbass --vbass_freq=250` run (the two CI parity scenarios), on a temporary copy of `data/demo`, from "start" to "all files written":

| side | how |
|---|---|
| Rust | `crates/impulcifer-service/examples/demo_brir.rs`: `demo_brir <dir> [--vbass]` runs `run_brir` through a `JobRegistry` with `NoopHost` and prints the wall-clock and the per-stage durations from the observer; benchmark with `cargo run --release -p impulcifer-service --example demo_brir -- <dir>` (median and minimum of 5 runs, warm-up 1) |
| Python 3.14.5 | `py -3.14 E:/Impulcifer/impulcifer.py --dir_path=<copy> --test_signal=E:/Impulcifer/data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav` (and `--vbass --vbass_freq=250`), timed around the whole process, median of 3; also in-process `impulcifer.main(**params)` timing to separate interpreter start-up |
| Python 3.14.7t | the same with `%LOCALAPPDATA%\impulcifer-bench\py314t\Scripts\python.exe` (2.x uses free-threaded parallelism in `core/parallel_processing.py` when available) |

Peak resident memory (the user asked for this out of curiosity; report, do not optimise for it): the bench script `tests/migration/bench_oracle_pipeline.py` spawns each side as a child process and samples `psutil.Process(pid)` every 50 ms (`memory_info().peak_wset` on Windows at exit, `rss` maximum during the run elsewhere); install `psutil` into `py -3.14` and the 3.14t venv if missing (`pip install psutil` in those interpreters only). Rust is one process with rayon threads; Python 3.14 uses a process pool for the EQ stage (sum the children's peaks and report both the parent and the total); 3.14t uses threads.

## Optimisation scope
Allowed files: `crates/impulcifer-service/src/brir/**` (except what the recording worker touches), `crates/impulcifer-service/examples/demo_brir.rs`, `crates/impulcifer-service/benches/perf.rs` (+ `[[bench]]` with `test = false` in `Cargo.toml`), `crates/impulcifer-service/tests/perf_smoke.rs` (`bench_smoke_pipeline_demo`: the example logic at demo scale but with `--fast` = only the first speaker file), `crates/impulcifer-dsp/src/{pipeline.rs,hrir.rs,ir.rs,stages/**,channel_balance.rs,virtual_bass.rs,mic_deviation.rs,fr.rs,fr/**}` (optimisations only, no semantic change), `tests/migration/bench_oracle_pipeline.py`, `docs/rust/perf/pipeline-demo.md`. The primitives audited by PA02 are out of scope unless the per-stage breakdown proves one of them dominates; then say so and change it with the same rules.

Known suspects: per-speaker EQ chain not parallel across speakers (check the rayon use in `stages::equalize`), `ImpulseResponse::equalize` reallocating per convolution, `Hrir::stack_tracks` copying every track, the outputs writer serialising 30/32-track files one after another (the io audit made writes fast; parallel writes of hrir/hesuvi/responses are acceptable), `FrequencyResponse` clones in the EQ chain, `magnitude_to_frequency_response` FFTs of the full IR length, the readme decay analysis running twice, debug-level logging in hot loops.

Acceptance: `python_median / rust_median >= 1.0` for both scenarios against **both** interpreters (3.14.5 and 3.14.7t), with the demo parity tests still passing (`cargo test -p impulcifer-service --test demo_parity`). The per-stage table must be in the report.

## Verification (foreground, paste output)
```
cargo build --release -p impulcifer-service --example demo_brir
cargo bench -p impulcifer-service --bench perf -- --pipeline
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_pipeline.py
cargo fmt -p impulcifer-service -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-service -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service -p impulcifer-dsp
cargo test -p impulcifer-policy
```

## Report
Per the template, plus: the per-stage duration table (Rust) with the 2.x stage keys, the memory table (Rust peak RSS; Python 3.14 parent and parent+children; 3.14t), and a one-paragraph answer to "does thread-based parallelism save memory versus the 2.x process pool", with numbers. Registry: `perf.pipeline-demo` cites `impulcifer-service::bench_smoke_pipeline_demo`.
