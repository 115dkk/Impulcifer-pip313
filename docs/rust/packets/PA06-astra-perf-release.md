# PA06 (ASTRA): the release performance audit (`perf.release`, M5): every crate and the whole pipeline once more on the final tree

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` (method, hard rules, report format). This packet re-runs the audits that already exist instead of writing new ones: PA01 (`impulcifer-io`), PA02 (`impulcifer-dsp`), PA03 (`perf.pipeline-demo`, with plots since P19), PA04 (`impulcifer-service`), PA05 (`impulcifer-audio-io` and `impulcifer-sys-win` on CABLE-A). Read their reports under `E:/Impulcifer/docs/rust/perf/` and their bench and oracle scripts (`crates/<crate>/benches/perf.rs`, `tests/migration/bench_oracle_*.py`) first; do not change any of them except to make them run on the current tree.

Run every command in the foreground. Never use background execution. Measure with no other worker running and no builds in progress; list the audio applications that are open in the environment section and never terminate any user process. The Python side runs with `py -3.14` and the free-threaded venv named in the template.

## Method
1. Build everything in release once: `cargo build --workspace --release`, then run each existing bench (`cargo bench -p <crate> --bench perf`) and its oracle script, in this order: io, dsp, service, pipeline-demo (with peak RSS), audio-io + sys-win (CABLE-A). Use the sizes and repetition rules of each original packet.
2. For every operation, put the new ratio next to the ratio recorded in that crate's report. A ratio that fell below 1.0, or by more than 20 percent from the recorded value, is a regression: find the commit that caused it (`git log` on the crate since the report's date, `cargo bench` at the two commits in a detached worktree under `/tmp`) and fix it within the same rules as the original audit, or report it as unresolved with the profile.
3. The whole-pipeline audit is the M5 criterion: the demo BRIR run (default and vbass) on the release binary, process wall time and peak RSS, against CPython 3.14.5 and 3.14.7t as in PA03, with plots included on both sides.
4. Write `docs/rust/perf/release.md`: environment, one table per crate with old ratio, new ratio and verdict, the pipeline table with memory, the regressions found and what was done, and the exact commands. Keep the per-crate reports untouched except for a one-line pointer to the release report at their top.

## Allowed files
`E:/Impulcifer/docs/rust/perf/release.md` (new), one-line pointers at the top of the existing `docs/rust/perf/*.md`, and only when a regression must be fixed: the crate sources under the same restrictions as the original audit packet (optimisations only, goldens unchanged, no unsafe, no f32, no new dependencies). Nothing else. Do not edit `features.toml`.

## Verification (foreground, paste output)
```
cargo build --workspace --release
cargo bench -p impulcifer-io --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
cargo bench -p impulcifer-dsp --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_dsp.py
cargo bench -p impulcifer-service --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
cargo bench -p impulcifer-service --bench perf -- --pipeline
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_pipeline.py
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
cargo test --workspace
cargo test -p impulcifer-policy
```

## Report format
(1) environment; (2) the per-crate tables (old ratio, new ratio, verdict); (3) the pipeline table with memory; (4) regressions and fixes; (5) the registry test name for `perf.release` (propose `impulcifer-service::bench_smoke_pipeline_demo` plus the per-crate smoke tests); (6) anything undone. Do not end your turn before the commands complete.
