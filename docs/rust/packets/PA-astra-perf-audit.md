# PA (ASTRA): performance audit of a landed crate against the 2.x Python oracle

This is the template every `PAnn-astra-perf-<crate>.md` packet includes by reference. The rule it implements (docs/rust/ARCHITECTURE.md section 9, gate 5): **a crate is not finished until an ASTRA performance audit has measured it against the Python oracle on the same machine and it is at least as fast for every operation the 2.x pipeline performs.** The rewrite exists to be faster than Python, not only safer; a port that loses to numpy/scipy on its own workload is a failure of the port, not of Rust.

Run every command in the foreground. Never use background execution.

## Method

1. **Benchmark target.** Add `crates/<crate>/benches/perf.rs` and a `[[bench]] name = "perf"` / `harness = false` entry to the crate's `Cargo.toml`. No new dependencies: measure with `std::time::Instant`, warm up 3 runs, then take the **median and minimum of 11 runs** (fewer only when one run exceeds 5 s; say so). Print one Markdown row per operation: `| op | size | rust median ms | rust min ms |`. Use realistic sizes: the ones the 2.x pipeline actually processes on `data/demo` at 48 kHz (they are listed in the crate packet). Build and run with `cargo bench -p <crate> --bench perf` (release profile).
2. **Python oracle bench.** Add `tests/migration/bench_oracle_<crate>.py` timing the *same* operations with the *same* sizes through the 2.x functions (`core.*`, `autoeq.*`, scipy/numpy/nnresample/soundfile), same warm-up and repetition rule, `time.perf_counter`, printing the same table shape. **The opponent is 2.x on the newest CPython, not on whichever `python` is first on PATH:** run the script with the newest CPython installed on this machine (`py -3.14`, currently 3.14.5, with the 2.x requirements installed) and, for any operation that 2.x parallelises with threads (`core/parallel_processing.py`, free-threaded builds), also with the free-threaded interpreter (`uv python find 3.14t`, currently 3.14.7t, requirements installed); report both columns when they differ. Record the exact interpreter path and version, `numpy.__version__`, `scipy.__version__`, the BLAS/FFT backend from `numpy.show_config()`, and the thread environment (`OMP_NUM_THREADS`, `MKL_NUM_THREADS` if set). Do not pin threads to 1 unless 2.x does; the oracle is 2.x as users run it on the latest Python. (The correctness goldens are a separate matter: they come from the recorded oracle environment in `tests/migration/README.md`, and numerics do not depend on the interpreter version.)
3. **Compare.** `ratio = python_median / rust_median`. Every op needs `ratio >= 1.0`. Report the whole table.
4. **Profile before optimising.** For every op with `ratio < 1.5`, explain where the time goes (allocation, re-planning FFTs, direct O(N·M) loops where scipy chooses FFT, single-threaded work that 2.x parallelises per speaker, redundant copies, bounds checks in inner loops) and fix the cause rather than the symptom. Preferred tools in order: reasoning about the code, `cargo bench` with narrowed sizes, `--release` with `debug = 1` and a sampling profiler if available (`perf` on Linux, Windows Performance Recorder or `cargo flamegraph` if installed; do not install anything).
5. **Optimise.** Allowed: caching FFT plans (`rustfft::FftPlanner` / `realfft::RealFftPlanner` kept in a `thread_local!` or `OnceLock` cache keyed by length), choosing FFT vs direct convolution the way scipy's `method='auto'` does, `rayon` across independent tracks/speakers (the workspace already depends on it), removing allocations from inner loops, iterator-based loops that let LLVM auto-vectorise, `Vec::with_capacity`, in-place operations, `chunks_exact`, avoiding `Complex64` temporaries when a real transform suffices. **Not allowed:** `unsafe`, SIMD intrinsics, `f32` anywhere in a numeric path, changing results beyond the golden tolerances, new crates without listing them in the report, disabling bounds checks by casting, and any change to a public signature (add a new function instead and say so).
6. **Prove nothing regressed.** After every optimisation, `cargo test -p <crate>` (all goldens) and `cargo test -p impulcifer-policy` must pass unchanged. Golden tolerances are not to be widened. If an optimisation changes floating-point summation order, show the new max error against each affected golden in the report.
7. **Registry hook.** Add `#[test] fn bench_smoke_<crate>()` in the crate's tests that runs every bench op once at a tiny size, so the bench code keeps compiling under `cargo test`. The registry entry `perf.<crate>` will cite it.
8. **Memory (end-to-end audits only, M2 and M5).** Besides time, measure peak resident memory of the whole demo BRIR run: the Rust pipeline (rayon threads in one process) versus 2.x on `py -3.14` (process pool) and on `3.14t` (free-threaded threads). Use `/usr/bin/time -v`-style peak RSS where available; on Windows read `PROCESS_MEMORY_COUNTERS.PeakWorkingSetSize` through `psutil` for Python and `GetProcessMemoryInfo` via a tiny Rust example, or `Get-Process` sampling at 50 ms as a fallback (state which). This is a measurement the user asked for out of curiosity; report it, do not optimise for it.
9. **Report** to `docs/rust/perf/<crate>.md`: environment (CPU model, core count, OS, Rust version, Python/numpy/scipy versions, backends), the full table before and after with ratios, each optimisation with the measured effect, ops still below 1.0 with the reason, and the exact commands run.

## Allowed files (in addition to the crate-specific list)
`crates/<crate>/benches/perf.rs`, `crates/<crate>/Cargo.toml` (only the `[[bench]]` section and `[dev-dependencies]`), `crates/<crate>/src/**` (optimisations only; no semantic changes), the crate's test files (only to add `bench_smoke_<crate>` and new property tests for the optimised paths), `tests/migration/bench_oracle_<crate>.py`, `docs/rust/perf/<crate>.md`. Nothing else: not `features.toml`, not `Cargo.lock` by hand, not other crates, not the golden exporters or fixtures.

## Hard rules
- `#![forbid(unsafe_code)]` stays. No unsafe, no SIMD intrinsics, no f32.
- Goldens decide. A faster function that fails a golden is not an optimisation.
- Measure on this machine, release profile, same sizes for both languages, both tables pasted verbatim in the report and in your final message.

## Verification (foreground, paste output)
```
cargo bench -p <crate> --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_<crate>.py
cargo fmt -p <crate> -- --check
cargo clippy -p <crate> --all-targets -- --no-deps -D warnings
cargo test -p <crate>
cargo test -p impulcifer-policy
```

## Report format
(1) environment (interpreter path and version for the Python side); (2) the before/after table with ratios; (3) each optimisation: cause, change, effect, golden max error after; (4) ops still slower than Python and why; (5) files changed; (6) the `bench_smoke_<crate>` test name; (7) anything undone. Do not end your turn before the commands complete.
