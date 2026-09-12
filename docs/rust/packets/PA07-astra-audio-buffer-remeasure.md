# PA07 (ASTRA): shared-mode buffer re-measurement on the alpha.2 audio stack

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` (hard rules, report format) and the harness of `E:/Impulcifer/docs/rust/packets/PA05b-astra-audio-realtime.md`. Read the seventh run in `E:/Impulcifer/docs/rust/perf/impulcifer-audio-io.md`, section 2.4 of `E:/Impulcifer/docs/rust/perf/release.md`, and `E:/Impulcifer/docs/rust/HARDWARE.md` first.

Run every command in the foreground. The caller guarantees that no other worker and no build runs while this packet is measured; do not query the process list and do not stop or ask about any process; write in the environment section that other processes were not queried and the caller kept the machine quiet. Never terminate any user process. Measurement output stays under `crates/impulcifer-audio-io/tests/bench_support/` (ignored by git); nothing large goes into tracked files.

## Where we stand

PA05b (2026-09-08) added MMCSS promotion (`audio_thread_priority`) and measured the shared-mode buffer at 2, 3 and 4 engine periods on the CABLE-A pair with `sixth_paired.py`: 2 and 3 periods still underran 13 to 16 times per ten runs, so `SHARED_BUFFER_PERIODS` stayed at 4 (1920 frames, 40 ms) and the owner accepted the resulting 0.25 to 0.33 percent headphone-sweep wall-time exception (M5, 2026-09-09; `features.toml` `perf.impulcifer-audio-io`). Since then the recording stack changed: `open_output_with_preference` / `open_input_with_preference` and the `share_mode` request (auto, exclusive, shared; PR #206, 3.0.0-alpha.1), the cached backend forwarding `selectable_share_modes`, and the workspace is at 3.0.0-alpha.2. Nobody has measured the audio path since. This packet is a re-measurement, not an optimisation packet: its first job is to tell whether alpha.2 still holds the PA06/PA05b numbers, and its second job is to test the smaller buffers again under the current code.

The environment override `IMPULCIFER_PA05_SHARED_PERIODS=2|3|4` (`crates/impulcifer-sys-win/src/lib.rs`) still exists; use it for the period trials. The devices are the ones in `docs/rust/HARDWARE.md`: the CABLE-A pair for the paired integrity runs. Line (Realphones System-Wide) is the one endpoint on this machine that accepts exclusive float32 render; there is no exclusive-capable capture endpoint, so no full-duplex exclusive measurement is possible here. Say so in the report instead of inventing one.

## Work
1. Regression table. Re-measure the full PA05 audio table exactly as PA06 did (cached enumeration, duplex open/close, headphones wall and overhead, seven-segment wall and overhead, first-sample delivery, CPU) against both interpreters (`py -3.14` and the 3.14t venv), explicit streams and the production `core.recorder.play_and_record` runs, plus the sys-win bench. Put the PA06 / PA05b numbers next to the new ones. Any ratio that moved by more than 5 percent gets a sentence on the likely cause (the share-preference code path is the first suspect: `crates/impulcifer-audio-io/src/policy.rs`, `session.rs`, `cached_backend.rs`).
2. Period table. `sixth_paired.py --period N --run R` for N in 2, 3, 4 with ten runs each (runs 300 to 309, 310 to 319, 320 to 329), both operations (`play_record_headphones_sweep`, `play_record_7_speaker_set`): render underruns, capture discontinuities, packet gaps, independent observer verdict. Same table layout as PA05b section 3.
3. Verdict on the default. If a smaller period count is clean 10 of 10 on both counters and the observer for both operations, change `SHARED_BUFFER_PERIODS` to it, keep 4 reachable through the override, and re-run item 1 with the new default. If not, `SHARED_BUFFER_PERIODS` stays at 4 and the report says which run failed how. Do not change the default on a partial pass.
4. Report. Append an "Eighth run (PA07)" section at the top of `docs/rust/perf/impulcifer-audio-io.md` with the environment, the regression table, the period table, the verdict and the exact commands; add a dated paragraph to `docs/rust/HARDWARE.md` only if the buffer default changed.

## Allowed files
`crates/impulcifer-sys-win/src/lib.rs` (only the `SHARED_BUFFER_PERIODS` constant and its comment), `crates/impulcifer-audio-io/tests/bench_support/` harness sources (`*.py`; never commit captures), `docs/rust/perf/impulcifer-audio-io.md`, `docs/rust/HARDWARE.md`. Not `features.toml`, not `CHANGELOG.md`, not the service crate, not `Cargo.lock`, not the Python tree.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
cargo fmt --all -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-service --test recording -- --ignored recording_virtual_cable_end_to_end
cargo test -p impulcifer-audio-io --test hardware -- --ignored
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py --period 2 --run 300   # 300..309, then 3 with 310..319, then 4 with 320..329
git status --porcelain
```

## Report format
(1) environment (state that processes were not queried); (2) the regression table PA06 / PA05b versus now, both interpreters, with the moved ratios explained; (3) the period table with underruns, discontinuities, gaps and observer verdicts; (4) the verdict on `SHARED_BUFFER_PERIODS` and the diff if it changed; (5) the pasted `test result:` lines and `git status --porcelain`; (6) anything undone. Do not end your turn before the commands complete.
