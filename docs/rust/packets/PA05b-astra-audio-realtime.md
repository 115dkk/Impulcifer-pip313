# PA05b (ASTRA): audio wall time below PortAudio with real-time thread promotion and a smaller shared-mode buffer

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` (hard rules, report format) and the sixth-run harness of `E:/Impulcifer/docs/rust/packets/PA05-astra-perf-audio.md`. Read `E:/Impulcifer/docs/rust/perf/impulcifer-audio-io.md` (sixth run) and section 2 "impulcifer-audio-io" plus section 4 of `E:/Impulcifer/docs/rust/perf/release.md` (PA06 first run) first.

Run every command in the foreground. The caller guarantees that no other worker and no build runs while this packet is measured; do not query the process list and do not stop or ask about any process; write in the environment section that other processes were not queried and the caller kept the machine quiet. Never terminate any user process. Measurement output stays under `crates/impulcifer-audio-io/tests/bench_support/` (ignored by git); nothing large goes into tracked files.

## Where we stand

PA06 measured, on CABLE-A with the PA05 harness, Python/Rust ratios (CPython 3.14.5 / 3.14.7t): headphones wall 0.9967 / 0.9965 (Rust 6211.1 ms, Python 6190.9 / 6189.7 ms), headphones overhead 0.66 / 0.64, seven-segment wall 0.99953 / 0.99957, seven-segment overhead 0.72 / 0.74, first-sample delivery 0.87 / 0.89. The rule for M5 is ratio >= 1.0 on every operation; the PA05 exception (4 engine periods = 40 ms shared-mode buffer because 3 and 2 periods underran 10 of 10) does not carry over.

Diagnosis to test first: the render and capture threads run at normal priority, whereas PortAudio's WASAPI host registers its processing thread with MMCSS ("Pro Audio"), which is why PortAudio survives smaller buffers. The crate `audio_thread_priority` (Mozilla, version 0.37.0 on crates.io, safe public API, MMCSS "Pro Audio" task on Windows) provides `promote_current_thread_to_real_time(buffer_frames, sample_rate) -> Result<handle, _>` and `demote_current_thread_from_real_time(handle)`. Verify the exact signatures and the Windows feature set on docs.rs with WebFetch before use.

## Work
1. Add `audio_thread_priority` (default features minus anything Linux-only that pulls dbus on Windows; check its feature flags) to `crates/impulcifer-sys-win/Cargo.toml` and promote the render thread and the capture thread for the lifetime of a session (demote on exit, also on the error paths). No `unsafe` in our code; `unsafe-budget.toml` stays at zero. If the crate's Windows path needs anything our forbid rule cannot accept, stop and report instead of working around it.
2. With promotion in place, measure the shared-mode buffer at 2, 3 and 4 engine periods with the PA05 integrity harness (`sixth_paired.py --period N`, ten runs each): render underruns, capture discontinuities, packet gaps, and the independent Python observer. Choose the smallest period count that is clean 10 of 10 on both counters and observer for the headphones sweep and the seven-segment set; make it the default and keep 4 periods reachable through the existing option. Record the full table (period, underruns, discontinuities, observer verdict) in the report.
3. Re-measure the full PA05 audio table (cached enumeration, duplex open/close, headphones wall and overhead, seven-segment wall and overhead, first-sample delivery, CPU) against both interpreters exactly as PA06 did (explicit streams and the production `core.recorder.play_and_record` runs), plus the sys-win bench. Every audio-io ratio must be >= 1.0 on both interpreters.
4. If promotion plus the smaller buffer does not reach 1.0 somewhere, profile that operation (the PA05 trace counters) and fix within the rules: no unsafe, no f32 sample path change, no new public API, goldens untouched. Candidates you may implement: pre-rolling the first render buffer before `Start` so the first packet is not late, avoiding the extra thread hop between capture packet arrival and the application callback, and trimming the drain at the end of playback to the exact remaining frames. Report anything left below 1.0 with its profile.
5. Append a "seventh run" section to `docs/rust/perf/impulcifer-audio-io.md` with the environment, the period table, the before/after ratios and the exact commands; update `docs/rust/HARDWARE.md` if the buffer default changed.

## Allowed files
`crates/impulcifer-sys-win/{Cargo.toml,src/**}`, `crates/impulcifer-audio-io/{Cargo.toml,src/**}`, both crates' `benches/` and `tests/bench_support/` harness sources (`mod.rs`, `*.py`; never commit captures), `crates/impulcifer-audio-io/tests/session_fake.rs` and `crates/impulcifer-sys-win/tests/bench_smoke.rs` if a test must follow the change, `docs/rust/perf/impulcifer-audio-io.md`, `docs/rust/HARDWARE.md`, `Cargo.lock` (the new dependency only). Not `features.toml`, not `CHANGELOG.md`, not the service crate, not the Python tree.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
cargo fmt --all -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-service recording
cargo test -p impulcifer-service --test recording -- --ignored recording_virtual_cable_end_to_end
cargo test -p impulcifer-policy
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py --period <chosen> --run 200   # and 201..209
git status --porcelain
```

## Report format
(1) environment (state that processes were not queried); (2) the period table with underruns, discontinuities and observer verdicts; (3) the audio ratio table before (PA06) and after, both interpreters; (4) what changed in the code and why, with the `unsafe` count (must be zero); (5) the pasted `test result:` lines and `git status --porcelain`; (6) anything still below 1.0 and its profile. Do not end your turn before the commands complete.
