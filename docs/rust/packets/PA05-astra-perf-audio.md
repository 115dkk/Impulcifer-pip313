# PA05 (ASTRA): performance audit of `impulcifer-audio-io` and `impulcifer-sys-win` (measurement session overhead) against 2.x `core.recorder` on the CABLE-A pair

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` exactly (method, allowed files, hard rules, verification, report), with the hardware differences below. Read first: `E:/Impulcifer/crates/impulcifer-audio-io/src/**` (backend trait, WASAPI session, cpal backend, `play_and_capture`), `crates/impulcifer-sys-win/src/**` (the unsafe-free WASAPI wrapper; **do not change this crate's public API and do not add `unsafe`; it is Daybreak's crate, you may only add a bench and report**), `crates/impulcifer-service/src/recording/**` (how a recording job drives the backend), `docs/rust/HARDWARE.md` (the CABLE-A Input/Output pair, exclusive-first policy, measured formats), and the 2.x oracle `E:/Impulcifer/core/recorder.py` (`play_and_record`: `sd.play(blocking=True)` plus the recording thread, PortAudio WASAPI through sounddevice) with `E:/Impulcifer/core/sweep_signal.py` for the sweep it plays.

Run every command in the foreground. Never use background execution. Both languages use the same devices: output `CABLE-A Input (VB-Audio Cable A)`, input `CABLE-A Output (VB-Audio Cable A)`, 48 kHz. Close every other audio app before measuring; run each op 5 times (hardware runs are long) and report median and minimum. Other workers may be editing `crates/impulcifer-service/src/update/**`, `apps/**`, `crates/impulcifer-service/benches/**`; treat those as read-only.

## Operations

| op | Rust | Python oracle | size |
|---|---|---|---|
| `enumerate_devices` | `list_audio_devices` through the backend | `sounddevice.query_devices()` + host API listing as `core.recorder` does | 20 calls |
| `open_close_session` | open the CABLE-A output+input session (exclusive first, shared fallback) and close it, no audio | `sd.Stream(...)` open/close with the same devices, WASAPI host API | 20 runs |
| `play_record_headphones_sweep` | play the bundled 6.15 s sweep (`data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`) and capture 2 channels, exactly what `start_recording` headphones mode does | `core.recorder.play_and_record` with the same file and devices | 5 runs; report **overhead = wall time minus sweep duration**, plus captured frame count |
| `play_record_7_speaker_set` | the 7-segment stereo sweep set as speakers mode (about 43 s) | same through 2.x | 3 runs; overhead and frame count |
| `capture_loop_cpu` | CPU time (user+kernel) of the process during `play_record_headphones_sweep` | same for the Python process (psutil `cpu_times`) | 5 runs |
| `first_sample_latency` | time from session start to the first captured buffer callback | time from `sd.play` to the first recording callback | 10 runs |

Also record for both: sample format negotiated (exclusive or shared, bit depth), buffer sizes, and any xruns/overflows reported by the backend. If exclusive mode fails on this pair, say so and measure shared.

Known suspects to check before measuring: per-buffer `Vec` allocation in the capture callback, format conversion done sample by sample, lock contention between the render and capture threads, polling sleeps in the session loop (compare with PortAudio's callback model), and enumeration re-initialising COM per call.

Report to `docs/rust/perf/impulcifer-audio-io.md` (covering both crates, with a section per crate). Registry test names: `bench_smoke_impulcifer_audio_io` (in `crates/impulcifer-audio-io/tests`, runs with the fake backend so CI needs no device) and `bench_smoke_impulcifer_sys_win` (`crates/impulcifer-sys-win/tests`, enumeration only, `cfg(windows)`, must pass on the CI Windows runner without audio devices by tolerating an empty device list).
