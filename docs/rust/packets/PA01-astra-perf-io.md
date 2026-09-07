# PA01 (ASTRA): performance audit of `impulcifer-io` against soundfile / 2.x `core.audio_io`

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` exactly (method, allowed files, hard rules, verification, report). This file only names the crate and its operations. Read the crate first: `E:/Impulcifer/crates/impulcifer-io/src/{wav,ffmpeg,sweep_files}.rs` and the 2.x oracle `E:/Impulcifer/core/audio_io.py` (`read_wav`, `write_wav`), `E:/Impulcifer/core/sweep_signal.py` lines 130 to 205 (the PCM_32 round trip), `E:/Impulcifer/core/hrir.py` lines 427 to 474 (how `write_wav` is fed: 32 tracks, `bit_depth=32`).

Run every command in the foreground. Never use background execution. The Python side of the benchmark runs with the newest CPython on this machine (`py -3.14`; and `uv python find 3.14t` for thread-parallel operations), never with the default `python`. Other workers are editing `crates/impulcifer-dsp/**`, `tests/migration/export_goldens.py`, `tests/migration/README.md` and `features.toml`; do not touch those.

## Operations and sizes (the 2.x pipeline on `data/demo` at 48 kHz)

| op | Rust | Python oracle | size |
|---|---|---|---|
| `write_pcm32_32tracks` | `write_wav(path, 48000, &tracks, 32)` | `core.audio_io.write_wav(path, 48000, data, bit_depth=32)` | 32 tracks × 96,000 samples (like `hrir.wav`/`responses.wav`) |
| `write_pcm32_30tracks` | same | same | 30 × 96,000 (`hesuvi.wav`) |
| `write_pcm16_2tracks` | `write_wav(.., 16)` | `bit_depth=16` | 2 × 295,000 (a sweep) |
| `read_pcm32_32tracks` | `read_wav(path)` | `core.audio_io.read_wav(path, expand=True)` | the 32 × 96,000 file written above |
| `read_bundled_sweep` | `read_wav` | `read_wav` | `E:/Impulcifer/data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav` |
| `read_demo_recording` | `read_wav` | `read_wav(expand=True)` | `E:/Impulcifer/data/demo/FL,FR.wav` |
| `read_float32_8tracks` | `read_wav` | `soundfile.read` | an 8 × 480,000 IEEE float32 file written by soundfile in the Python bench (write it into a temp dir and read it from both languages) |
| `pcm32_round_trip` | `pcm32_round_trip(&tracks)` | `core.sweep_signal` PCM_32 round trip (`np.int32` quantise then `/ 2**31`, exactly the code at lines 130 to 205) | 30 × 96,000 |
| `sweep_file_name_parse` | `parse_sweep_file_name` × 10,000 | the 2.x regex parse if one exists, else `ImpulseResponseEstimator.file_name` formatting × 10,000 | strings |

Use a temp directory under `std::env::temp_dir()` / `tempfile.mkdtemp` for written files, delete them at the end, and never write inside the repository. Fill the tracks with deterministic pseudo-random data (LCG, seed 7) so both languages see the same values. Time the whole call including file open/close; the OS page cache is part of both measurements, so run the warm-up first.

Known suspects to check before measuring: per-sample `to_le_bytes` pushes into an unreserved `Vec`, `BufWriter` default capacity for a 12 MB file, `read` copying the whole file then converting sample by sample with bounds checks, `f64` conversion of PCM_24 via 3-byte assembly in a scalar loop, and `Vec<Vec<f64>>` de-interleaving with per-sample indexing (prefer `chunks_exact(block_align)` and pre-sized output vectors).

Report to `docs/rust/perf/impulcifer-io.md`. The registry test name is `bench_smoke_impulcifer_io`.
