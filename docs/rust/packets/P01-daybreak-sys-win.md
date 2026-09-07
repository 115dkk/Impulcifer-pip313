# P01 (Daybreak Blue): `impulcifer-sys-win` WASAPI backend + hardware probe

You are implementing one crate of the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read these first, in this order: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` (the contract), `E:/Impulcifer/docs/adr/0002-rust-tauri-rewrite.md`, `E:/Impulcifer/crates/impulcifer-types/src/audio.rs` (the trait you implement), `E:/Impulcifer/docs/research/rewrite-stack-2026-09/01-windows-audio-api.md` (policy and measurements), and the wasapi-rs sections of `E:/Impulcifer/docs/research/rewrite-stack-2026-09/report-10-rust-unsafe-inventory.md` (section 2.3: known defects you must avoid). Do not modify any file outside the allowed list.

## Scope

Implement `E:/Impulcifer/crates/impulcifer-sys-win` so that `WasapiBackend` fully implements `impulcifer_types::audio::AudioBackend` on Windows using the `wasapi` crate 0.24.x (HEnquist/wasapi-rs), plus an example binary `hardware_probe` that measures real devices on this machine.

## Allowed files

- `E:/Impulcifer/crates/impulcifer-sys-win/Cargo.toml` (you may add dependencies; keep `[lints] workspace = true`)
- `E:/Impulcifer/crates/impulcifer-sys-win/src/**` (new modules welcome)
- `E:/Impulcifer/crates/impulcifer-sys-win/examples/hardware_probe.rs`
- `E:/Impulcifer/crates/impulcifer-sys-win/tests/**`
- `E:/Impulcifer/features.toml`: only the `audio.*` entries, and only to set `status = "implemented"` with the exact names of tests you wrote.

Everything else is read-only. In particular do not touch `crates/impulcifer-types`, `crates/impulcifer-audio-io`, `webview_ui/`, or any Python file. If the trait in `impulcifer-types` is insufficient, stop and say exactly what is missing in your report instead of editing it.

## Hard rules

1. `#![forbid(unsafe_code)]` stays at the top of every Rust file root in this crate. Handwritten `unsafe` is not allowed (budget 0 in `E:/Impulcifer/unsafe-budget.toml`). If you believe a required operation is impossible without unsafe, do not write it; describe the gap in the report.
2. Never call `wasapi::WaveFormat::parse` (unsound: reads past a `&WAVEFORMATEX`) or `wasapi::Device::from_raw`. Build formats with `WaveFormat::new(...)`, and read device formats through `get_mixformat()` / `is_supported()` / `is_supported_exclusive_with_quirks()` / `parse_from_blob_bytes` only.
3. Capture: when a packet carries `AUDCLNT_BUFFERFLAGS_SILENT` (see `BufferInfo`), write zeros into the destination for that packet instead of the bytes wasapi copied, and report `silent = true` in `CaptureRead`. Report `discontinuity = true` when the data-discontinuity flag is set.
4. No ASIO, no DirectSound, no MME. WASAPI only.
5. Every COM object is created, used and dropped on the thread that created it. Call `wasapi::initialize_mta()` once per backend-owned thread before touching devices; keep track so `deinitialize()` is balanced (or document why you leave the apartment alive for the process lifetime). Sessions must be `!Send` (they hold wasapi objects, which already are `!Send`), so the trait objects returned from `open_output`/`open_input` are only used on the thread that opened them. Document this at the top of `lib.rs`.
6. Exclusive mode uses the requested `StreamSpec` verbatim and must fail with `AudioError::UnsupportedFormat` if the device rejects it. Shared mode uses `StreamMode::{EventsShared|PollingShared}` with `autoconvert = true`; open with the requested sample rate/channels and let the engine convert; if the engine rejects, return `UnsupportedFormat`. Never silently downmix: if `spec.channels` is larger than the endpoint's mix-format channel count in shared mode, return `UnsupportedFormat` describing both numbers.
7. Sample format: always 32-bit float in the application (`WaveFormat::new(32, 32, &SampleType::Float, rate, channels, None)` or the exact constructor of the pinned wasapi version). If exclusive mode rejects float on a device, report that in `ProbeResult.detail`; do not fall back to integer transport in this packet.
8. `play_to_completion` returns only after every frame has been handed to the device and the endpoint has drained (padding reaches zero or the audio clock passes the last frame), then stops the stream. Check `cancel.is_cancelled()` between buffer fills; on cancel, stop the stream and return `Ok(PlaybackReport { cancelled: true, .. })`.
9. `read_into` fills `dst` completely (interleaved f32, `spec.channels` per frame) unless cancelled; it must not return borrowed driver memory. Use owned `Vec<u8>` scratch and `f32::from_le_bytes` conversions; no `bytemuck`, no transmute. `dst.len()` not being a multiple of `channels` is an `AudioError::UnsupportedFormat`.
10. `Endpoint.id` is the WASAPI endpoint id string (`Device::get_id()`), `Endpoint.name` the friendly name, `host_api = "Windows WASAPI"`. `default_samplerate` and the channel maxima come from the mix format. `is_default_*` from the default device of each direction. Enumerate active endpoints only.
11. `probe` must not open a stream longer than needed: use `is_supported` (shared, with the autoconvert caveat: in shared mode a rate mismatch is still supported through autoconvert, report `native_sample_rate` from the mix format) and `is_supported_exclusive_with_quirks` (exclusive).
12. Verify any wasapi API you are unsure about against the pinned source or docs with WebFetch: https://docs.rs/wasapi/0.24.0/wasapi/ and https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/api.rs . Do not guess method names.

## `hardware_probe` example

`cargo run -p impulcifer-sys-win --example hardware_probe -- [--output <name substring>] [--input <name substring>] [--play] [--channel N] [--rate 48000] [--seconds 1.0]`

- With no arguments: print every endpoint (direction, name, id, channels, mix rate, default flags), then for every output endpoint and every input endpoint probe the matrix rates {44100, 48000, 96000} x channels {2, 8, 16 clipped to the endpoint maximum} x modes {Exclusive, SharedAutoConvert} and print a table of supported/unsupported with the detail string. No audio is played.
- With `--play`: open the chosen output in Exclusive, falling back to SharedAutoConvert, and play a 1 kHz sine at -20 dBFS on channel `--channel` (0-based, default 0) for `--seconds`; simultaneously open the chosen input (2 channels) on a second thread started first, capture `--seconds + 1.0` seconds, then print: mode used, frames submitted/drained, underruns, captured frames, discontinuity count, silent-packet count, per-channel RMS in dBFS of the capture. Use `std::thread` and channels; sessions stay on their threads.
- Exit code 0 when the requested operations succeed, 2 on device errors, with a readable message.

## Tests you must write

Hardware-free unit tests (run by CI on all OSes; gate the wasapi-dependent parts with `#[cfg(windows)]`):

- `endpoint_from_mix_format_fields`: a fake mix format (rate, channels) maps to `Endpoint` fields and `ProbeResult.native_*`.
- `bytes_to_f32_round_trip`: interleaved f32 -> little-endian bytes -> f32 is exact for a known vector; odd trailing bytes are rejected.
- `silent_packet_is_zero_filled`: feed a fake packet source (a small trait you define for testability) with a SILENT-flagged packet containing garbage; the destination receives zeros and `CaptureRead.silent == true`.
- `discontinuity_flag_is_reported`.
- `shared_mode_rejects_more_channels_than_mix_format` and `dst_length_must_be_frame_aligned`.
- `play_reports_cancelled_when_token_set`: with the fake sink, cancel mid-way and check the report.

Hardware tests, `#[ignore]` by default (run locally with `cargo test -p impulcifer-sys-win -- --ignored`): `enumerate_lists_default_endpoints`, `probe_matrix_runs_without_panic`, `open_and_play_silence_exclusive_or_shared` (plays 200 ms of digital silence on the default output).

Register every implemented `audio.*` feature in `E:/Impulcifer/features.toml` with `status = "implemented"` and `tests = ["impulcifer-sys-win::<fn>", ...]` using the exact test function names. `audio.play_and_record_session` belongs to `impulcifer-audio-io` (another packet): leave it `planned`.

## Verification (run in the foreground and paste the output)

```
cargo fmt --all -- --check
cargo clippy -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-sys-win
cargo test -p impulcifer-policy
cargo run -p impulcifer-sys-win --example hardware_probe
cargo run -p impulcifer-sys-win --example hardware_probe -- --output "CABLE In 16ch" --input "CABLE Output" --play --channel 3 --rate 48000 --seconds 1.0
```

The last command uses the VB-Audio virtual cable present on this machine (16-channel WASAPI endpoint "CABLE In 16ch(VB-Audio Virtual Cable)" and 16-channel input "CABLE Output(VB-Audio Virtual Cable)"). It must run to completion. Also run it once with `--rate 44100` and once with `--rate 96000` and record the mode each rate ended up using.

## Report format

Reply with, in this order: (1) the list of files you changed; (2) the verification outputs verbatim; (3) the hardware probe matrix as printed; (4) any trait or type gaps in `impulcifer-types` you needed and did not have; (5) anything you could not do and why. Do not end your turn before the verification commands have run to completion.
