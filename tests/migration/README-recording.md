# P16 recording fixtures

These fixtures pin the existing Python 2.x recording IPC and pure recorder helpers. The implementation is in `crates/impulcifer-service/src/recording/`; the integration tests are in `crates/impulcifer-service/tests/recording.rs`.

## Regeneration

Run from the repository root with CPython 3.14 and the existing project dependencies installed.

```text
py -3.14 E:/Impulcifer/tests/migration/export_goldens_recording.py
```

The exporter imports the real `ImpulciferApplicationService`, injects a fake `sounddevice` module, and creates validation input WAVs in a temporary directory. It does not access audio hardware or write to `data/`. Only `goldens/p16_*` artifacts are generated. `$ROOT` denotes the temporary directory and is replaced by each Rust test. Paths are compared after separator normalization; production paths retain platform-native separators and are not canonicalized.

## Fixture schema

`p16_recording.json` contains these tables.

| Table | Contract |
|---|---|
| `validation` | Ordered recording request validation, complete error envelopes and internal validated payloads |
| `sweep_validation` | Default/custom/file sweep modes, types, boundaries, speaker/layout validation and headphone overrides |
| `paths` | `resolve_recording_paths` responses, including generated sweeps and headphone bypass |
| `naming` | Playback filename to recording filename |
| `speaker_names` | Speaker normalization, duplicate and unknown-name errors |
| `playback` | Requested/normalized spec, track shape, exact segments, display name, recording filename and SHA-256 |
| `progress` | Inferred segments and all ten progress fields at active/gap/boundary timestamps |
| `analysis` | Three synthetic PCM32 WAVs and their analysis summaries |

`p16_signal_*.pcm32` stores only the active quantized sweep, as signed little-endian int32 samples. This avoids storing repeated lead/gap/trailing silence and keeps the fixture budget below 12 MB. The SHA-256 covers the entire sequence, including every silent sample, in track-major little-endian float64 order after the PCM32 round trip. Tests first compare that SHA-256 exactly. If platform libm differences change the hash, the packet permits reconstructing every reference sample from the compact signal and checking a maximum one-PCM32-LSB difference. The reconstructed reference must itself match the exported SHA-256.

Progress timestamps are deserialized as float64 before exact field comparisons because JSON `2` and `2.0` are the same frontend number but distinct `serde_json::Number` representations. Analysis retains the oracle's unrounded duration and peak dB; only the transcendental log10 comparison permits 1e-12 dB absolute error.

## Test commands

All commands run in the foreground. The final command involving CABLE-A deliberately runs an ignored hardware test; it must not be interpreted as a skipped verification.

```text
cargo fmt -p impulcifer-service -- --check
cargo clippy -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service
cargo test -p impulcifer-service -- --ignored recording_virtual_cable_end_to_end
cargo test -p impulcifer-policy
```

The hardware test requires the `CABLE-A Input` playback and `CABLE-A Output` capture endpoints documented in `docs/rust/HARDWARE.md`. It records a generated default sweep into a temporary `FL,FR.wav`, checks sample count and channel count, then calls the existing `detect_sweep` IPC.

## Interface contracts

No frontend or catalogue changes are needed. Existing `start_recording`, `list_audio_devices`, `resolve_recording_paths`, `poll_job`, `cancel_job`, `bootstrap`, and `detect_sweep` names remain unchanged. Recording jobs are not cancellable; `bootstrap.capabilities.recording_cancel` remains false. Progress payloads retain all ten fields and do not emit log events. The result has `mode`, `record_path`, `summary`, `sweep`, and `sidecar_path`; an optional `warnings` array is reserved for playback-channel truncation.

The format policy resides in audio-io's `open_input_with_policy` and `open_output_with_policy`: exclusive first, shared auto-convert only after `UnsupportedFormat`. This replaces the Python native-format probe. The service timer samples session progress every 250 ms after output starts and uses elapsed wall time until session progress arrives. The current audio-io session estimates those frames from elapsed time; it does not expose a hardware cursor.

The fake backend captures the supplied playback delayed by 1000 samples. Other tests cover PCM32 serialization, exact frame count, input-first stream opening, format fallback, append padding/channel stacking, retryable device errors, output-write failures, injected-backend sharing, busy/non-cancellable jobs, mono headphone duplication, speaker mono preservation and sidecar rules.
