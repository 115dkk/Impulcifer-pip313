# P07 (ASTRA): `impulcifer-io` — WAV read/write, ffmpeg/ffprobe, sweep files

You are implementing one crate of the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` sections 5 and 6, the 2.x oracle `E:/Impulcifer/core/audio_io.py` (read_wav/write_wav orientation and PCM subtypes), `E:/Impulcifer/core/audio_truehd.py` and `E:/Impulcifer/core/ffmpeg_discovery.py` (probe, decode, discovery order), `E:/Impulcifer/core/constants.py` (track orders), `E:/Impulcifer/core/brir_recovery.py` (which files are read back and how), and `E:/Impulcifer/core/sweep_signal.py` lines 130 to 205 (the PCM_32 round trip that makes generated sweeps bit-identical to bundled files).

Run every command in the foreground. Never use background execution.

## Scope

### `wav.rs`
- `pub struct Wav { pub sample_rate: u32, pub tracks: Vec<Vec<f64>> }` tracks-by-samples like 2.x (`data.shape == (tracks, samples)`).
- `pub fn read_wav(path: &Path) -> Result<Wav, IoError>`: RIFF/WAVE and RF64 readers for PCM 16/24/32 and IEEE float 32/64, mono to 32+ tracks, `WAVE_FORMAT_EXTENSIBLE` with any channel mask. Integer samples are scaled exactly like `soundfile` does (divide by 2^(bits-1)); write the rule in a doc comment and pin it with a golden.
- `pub fn write_wav(path: &Path, sample_rate: u32, tracks: &[Vec<f64>], bit_depth: u16) -> Result<(), IoError>`: 2.x semantics: bit_depth 16/24/32 map to PCM_16/PCM_24/PCM_32 (integer, not float); clipping and rounding must match `soundfile` (it uses libsndfile: float to int conversion with rounding to nearest and clipping to the integer range; verify with a golden exported from Python). More than two channels write `WAVE_FORMAT_EXTENSIBLE`; set `dwChannelMask = 0` (`KSAUDIO_SPEAKER_DIRECTOUT`) for anything above 2 channels so 30/32-track BRIR files carry no bogus speaker positions; document why. Use `std::io::BufWriter` and `to_le_bytes`; no `unsafe`, no `bytemuck`, no memory mapping.
- `pub fn write_wav_f32(...)` is NOT part of this packet (2.x writes PCM_32; float output is a separate decision).
- `pub fn pcm32_round_trip(tracks: &[Vec<f64>]) -> Vec<Vec<f64>>`: the in-memory PCM_32 quantise-then-read-back that `core/sweep_signal.py` performs, so generated sweeps stay bit-identical to the bundled WAVs.

### `ffmpeg.rs`
- `pub struct FfmpegPaths { pub ffmpeg: PathBuf, pub ffprobe: PathBuf }`
- `pub fn discover(auto_install: bool) -> Result<Option<FfmpegPaths>, IoError>`: port the discovery order of `core/ffmpeg_discovery.py::get_ffmpeg_paths` (bundled/app-local directory first, then PATH, then the platform install hint). `auto_install = true` may only print the install instruction in this packet; do not run package managers.
- `pub fn probe_codec(paths: &FfmpegPaths, file: &Path) -> Result<String, IoError>` (ffprobe JSON, first audio stream codec name).
- `pub fn is_truehd(paths: &FfmpegPaths, file: &Path) -> Result<bool, IoError>` (codec in {truehd, mlp}).
- `pub fn is_truehd_atmos_object_master(paths: &FfmpegPaths, file: &Path) -> Result<bool, IoError>`: port `core/audio_truehd.py::is_truehd_atmos_object_master` (profile string check).
- `pub fn decode_to_wav(paths: &FfmpegPaths, file: &Path, out: &Path) -> Result<(), IoError>`: same ffmpeg arguments as `convert_truehd_to_wav` (32-bit float PCM output, all channels), spawned with `std::process::Command` (absolute executable, argument array, no shell, `CREATE_NO_WINDOW` on Windows via `std::os::windows::process::CommandExt::creation_flags`), stderr drained, non-zero exit is an error carrying the last stderr lines.
- `pub fn truehd_channel_names(channels: usize) -> Option<Vec<&'static str>>`: the 11/13-channel custom orders from `CHANNEL_LAYOUT_MAP`.

### `sweep_files.rs`
- `pub fn sweep_file_name(speakers: &[&str], layout: &str, duration_s: f64, fs: u32, bits: u16, f_lo: f64, f_hi: f64) -> String` and the parser `pub fn parse_sweep_file_name(name: &str) -> Option<SweepFileInfo>` reproducing the 2.x file naming (`sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`), plus `pub fn infer_sweep_segments(...)` from `core/recording_progress.py` if it is pure (read it; if it depends on the estimator, leave a TODO and report).

### Tests
- Goldens: extend `E:/Impulcifer/tests/migration/export_goldens.py` with a `p07_` family: (a) a 3-track float64 array with values at and beyond the clipping edge written by Python `write_wav` at 16/24/32 bits, stored as the resulting WAV bytes (base64 in JSON, small) so Rust can compare byte-for-byte; (b) `read_wav` of a bundled file `E:/Impulcifer/data/sweep-seg-FL-mono-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav` storing the first/last 64 samples and SHA-256 of the LE-f64 bytes of the full track; (c) `pcm32_round_trip` on a small float array.
- Rust tests: `golden_write_wav_bytes_match_soundfile`, `golden_read_wav_matches_python`, `golden_pcm32_round_trip_matches_python`, `read_write_round_trip_32_tracks_extensible_directout_mask`, `read_rejects_truncated_header`, `sweep_file_name_round_trips`, `ffmpeg_discovery_prefers_bundled_dir` (with a temp dir and a fake executable name; do not require ffmpeg to be installed), `ffmpeg_decode_error_carries_stderr` (fake failing executable script via a temp `.cmd`/`.sh`, guarded by `cfg`).
- `features.toml`: set `output.hrir_wav`, `output.hesuvi_wav`, `output.responses_wav` only if you also implemented the track-order writers (`pub fn write_tracks_in_order(...)` using constants from `impulcifer-types`; add the missing track-order constants to `impulcifer-types::constants` ONLY if they are absent and only as exact copies of `core/constants.py`, with a test comparing them to a JSON dump you add to the p07 goldens). Otherwise leave the `output.*` entries planned and report.

## Allowed files
`E:/Impulcifer/crates/impulcifer-io/**`, `E:/Impulcifer/crates/impulcifer-types/src/constants.rs` (append-only, exact copies of 2.x constants with a golden test), `E:/Impulcifer/tests/migration/export_goldens.py`, `E:/Impulcifer/tests/migration/goldens/p07_*`, `E:/Impulcifer/tests/migration/README.md`, and the `output.*` entries in `E:/Impulcifer/features.toml`. Nothing else.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; no `memmap2`; bounded reads (`std::fs::read` or buffered reads with a size check from the header).
- Never shell out with a string; always argument arrays. Never run an installer.
- Keep the 2.x PCM_32 integer output as the default; do not introduce float output.

## Verification (foreground, paste output)
```
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt -p impulcifer-io -p impulcifer-types -- --check
cargo clippy -p impulcifer-io -p impulcifer-types --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io -p impulcifer-types
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) the exact float→PCM rounding/clipping rule you matched and the evidence; (4) which `output.*` features you marked implemented and why; (5) anything undone. Do not end your turn before the commands complete.
