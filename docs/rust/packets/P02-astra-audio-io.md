# P02 (ASTRA): `impulcifer-audio-io` measurement session + cpal backend

You are implementing one crate of the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/docs/rust/ARCHITECTURE.md` (sections 3.4 and 4), `E:/Impulcifer/crates/impulcifer-types/src/audio.rs` (the traits), `E:/Impulcifer/crates/impulcifer-sys-win/src/lib.rs` (the finished Windows backend; copy its conventions), `E:/Impulcifer/docs/rust/HARDWARE.md` (what the real devices do), and the 2.x oracle `E:/Impulcifer/core/recorder.py::play_and_record` (lines 298 to 500: the session contract you must reproduce).

Run every command in the foreground. Never use background execution; the harness terminates background jobs when you end your turn.

## Scope

1. **Open policy** (`src/policy.rs`): `pub fn open_output_with_policy(backend: &dyn AudioBackend, endpoint: &Endpoint, spec: StreamSpec) -> Result<(Box<dyn OutputSession>, ShareMode), AudioError>` and the input twin. Try `ShareMode::Exclusive` first; on `AudioError::UnsupportedFormat` fall back to `SharedAutoConvert`; any other error is returned as is. Log which mode was used through the returned tuple, not through printing.

2. **Session** (`src/session.rs`): the two-stream measurement session.

```rust
pub struct PlaybackBuffer { pub sample_rate: u32, pub channels: u16, pub interleaved: Vec<f32> }
pub struct SessionRequest {
    pub output: Endpoint,
    pub input: Endpoint,
    pub playback: PlaybackBuffer,
    pub input_channels: u16,          // 2 for binaural microphones
    pub tail_seconds: f64,            // extra capture after playback ends (2.x captures exactly the playback length; keep 0.0 as default but allow more)
}
pub enum SessionEvent { InputReady { mode: ShareMode }, OutputStarted { mode: ShareMode }, Progress { frames_played: u64, frames_total: u64 }, OutputDrained(PlaybackReport), InputStopped { frames: usize } }
pub struct Recording { pub sample_rate: u32, pub channels: u16, pub interleaved: Vec<f32>, pub output_mode: ShareMode, pub input_mode: ShareMode, pub playback: PlaybackReport, pub capture: CaptureRead }
pub fn play_and_record(backend: &(dyn AudioBackend + Sync), request: SessionRequest, cancel: &CancelToken, on_event: &mut dyn FnMut(SessionEvent)) -> Result<Recording, AudioError>;
```

Order of operations, exactly as 2.x: (a) spawn the input thread; it opens the input with the policy, calls `start()`, then sends `InputReady`; (b) only after `InputReady`, the output thread opens the output with the policy and calls `play_to_completion` (which drains); (c) the input thread keeps reading until it has `playback frames + tail` frames, or until cancel; (d) join both, stop input, return `Recording`. Errors on either side must cancel the other side and be returned. Sessions never cross threads: each thread opens its own session. The coordinator communicates through `std::sync::mpsc` channels carrying owned data only. Progress events are emitted at most every 100 ms.

3. **cpal backend** (`src/cpal_backend.rs`): implement `CpalBackend` for `AudioBackend` using cpal 0.18. Make `cpal` an unconditional dependency (remove the `cfg(not(windows))` gating in `Cargo.toml` and `lib.rs`) so it compiles and can be tested on this Windows machine too; `default_backend()` still returns `WasapiBackend` on Windows and `CpalBackend` elsewhere. `ShareMode` is ignored by cpal (document it). `Endpoint.id` is the cpal device name; `host_api` is the cpal host id name. `play_to_completion` must build a blocking implementation over cpal's callback: preallocate the interleaved buffer, feed it from the callback with an atomic cursor, zero-fill after the end, and return only after the cursor reaches the end **and** an extra full buffer period has elapsed (cpal has no drain API; document the heuristic). `read_into` uses a bounded `std::sync::mpsc::sync_channel` from the callback into the caller; the callback must not allocate or block.

4. **Tests**:
   - Fake backend in `tests/session_fake.rs` implementing the traits in memory: the output session records what it was given and simulates time by frames; the input session returns a known signal delayed by N frames. Tests: `session_starts_input_before_output`, `session_reports_events_in_order`, `session_captures_playback_length_plus_tail`, `session_cancel_stops_both_sides`, `session_propagates_input_open_error`, `policy_falls_back_to_shared_on_unsupported_format`, `policy_does_not_retry_on_other_errors`.
   - cpal on this machine: `#[ignore]` test `cpal_play_and_capture_virtual_cable` using output device name containing "CABLE-A Input" and input "CABLE-A Output" at 48 kHz, a 1 kHz -20 dBFS tone of 1 s on channel 1 and a 2 s capture, asserting the right channel RMS is within 1 dB of -26.02 dBFS (see HARDWARE.md for why). Run it locally: `cargo test -p impulcifer-audio-io -- --ignored`.
   - `#[ignore]` test `wasapi_session_virtual_cable` doing the same through `default_backend()` on Windows.

5. **features.toml**: set `audio.play_and_record_session` to `implemented` with the session tests above. Do not touch other entries.

## Allowed files

`E:/Impulcifer/crates/impulcifer-audio-io/**`, and `E:/Impulcifer/features.toml` (only `audio.play_and_record_session`). Nothing else. If `impulcifer-types` lacks something you need, report it; do not edit it.

## Hard rules

- `#![forbid(unsafe_code)]` stays; no `unsafe`; no `static mut`; no `unsafe impl Send`.
- Sessions are `!Send`; do not try to move them. Threads own their sessions.
- No sleeps longer than 5 ms in polling loops; check `cancel` in every loop.
- Verify cpal 0.18 APIs with WebFetch of https://docs.rs/cpal/0.18.2/cpal/ when unsure. Do not guess.

## Verification (foreground, paste output)

```
cargo fmt --all -- --check
cargo clippy -p impulcifer-audio-io --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io
cargo test -p impulcifer-audio-io -- --ignored
cargo test -p impulcifer-policy
```

## Report format

(1) files changed; (2) verification outputs verbatim including the two ignored hardware tests; (3) the drain heuristic you chose for cpal and its measured behaviour; (4) gaps in `impulcifer-types`; (5) anything undone. Do not end your turn before the commands have completed.
