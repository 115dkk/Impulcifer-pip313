# PA06b (ASTRA): release performance audit, second run with the mandatory regression bisect

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` and `PA06-astra-perf-release.md` (same benches, same oracles, same sizes and repetition rules). The tree is `E:/Impulcifer` at the commit `git rev-parse HEAD` prints when you start (the PA06 first-run report `docs/rust/perf/release.md`, the PA05b thread promotion and the P20b fixes are all in it). Read `docs/rust/perf/release.md` sections 2 and 4 and the seventh-run section of `docs/rust/perf/impulcifer-audio-io.md` first.

Run every command in the foreground. The caller guarantees that no other worker and no build runs while this packet is measured; do not query the process list (the existing oracle environment helper queries processes: call it with that part disabled or note once that the helper did it), do not stop or ask about any process, and never terminate a user process. Start measuring immediately; there is nothing to confirm with the caller.

## What the first run left open, and what this run must settle

1. **Dips and the one miss.** PA06 recorded `find_peaks_96000` at 0.92 on the first measurement (1.27 on a repeat), and more than 20 percent drops for `read_pcm32_32tracks`, `read_bundled_sweep`, `rfft_irfft_96000` and `resolve_recording_paths` (FT), without the bisect the packet asked for. This run does it:
   - Measure each of those five operations three separate times (fresh process each time, Rust and Python, same session) and take the median of the three medians. Record all nine numbers per operation.
   - If the median ratio is still below 1.0 or still more than 20 percent under the crate report's value, create a detached worktree at the commit the crate's report measured (PA01 `39c00c3` for io, PA02 `e98a954` for dsp, PA04 for service: take the hash from the report), `git worktree add C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect <hash>` with `CARGO_TARGET_DIR=C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect-target`, build that bench in release and run it three times too. Compare Rust-at-old versus Rust-at-new on the same machine in the same hour: if Rust moved, find the commit (`git bisect` between the two with the bench as the test, at most six builds) and fix it under the audit rules; if only Python moved, say so with both Python numbers and close the item. Remove the worktree at the end (`git worktree remove --force`).
2. **Audio.** PA05b already measured the seventh run with MMCSS promotion; do not repeat the ten-run integrity trials. Re-run the audio table once (explicit streams and the production recorder, both interpreters) exactly as PA06 did, and put the PA06, PA05b and this run's ratios side by side. The headphones wall time is expected to stay a few tenths of a percent below 1.0; report it, do not try to fix it here.
3. **Pipeline and memory** as in PA06 (both scenarios, both interpreters, plots on both sides, peak RSS of the process tree).
4. **Whole test suite.** `cargo test --workspace` must pass on this CRLF checkout (P20b fixed the smoke test). If a test is flaky (the first run saw `legacy_executor_downloads_verifies_and_opens` fail once), run it ten times in a row and report the count.

## Report
Rewrite `docs/rust/perf/release.md` as the second run: environment; per-crate tables with three columns (crate report, first run, this run) and a verdict column; the bisect section with the nine-number tables and the old-commit comparison for every item; the pipeline table with memory; the audio table with PA06 / PA05b / now; the test-suite result; and a final list of exactly which `perf.release` criteria are met and which are not, with numbers. Keep the first-run text under a "First run (2026-09-08)" heading at the end for history. Do not edit `features.toml` or `CHANGELOG.md`; touch crate sources only for a regression fix found by the bisect.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
git rev-parse HEAD
cargo build --workspace --release
cargo bench -p impulcifer-io --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
cargo bench -p impulcifer-dsp --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_dsp.py
cargo bench -p impulcifer-service --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_pipeline.py
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
cargo test --workspace
cargo test -p impulcifer-policy
git status --porcelain
```
plus the repeated runs and the worktree comparison described above, with their exact commands in the report.
