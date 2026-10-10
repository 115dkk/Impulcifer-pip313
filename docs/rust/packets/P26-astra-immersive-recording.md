# P26 (ASTRA): recorder layouts and slot codes with digits for the immersive formats

You are extending the Impulcifer 3.x Rust workspace at `/home/user/Impulcifer-pip313`. Read first, completely:

- `docs/adr/0007-immersive-layouts.md` (rules 2 and 6),
- `crates/impulcifer-types/src/layouts.rs` and `crates/impulcifer-types/src/constants.rs` (`SWEEP_TRACK_LAYOUTS`, `SEQUENCE_TRACK_ORDERS`, `SPEAKER_NAMES`),
- `crates/impulcifer-dsp/src/estimator.rs` (`sweep_sequence`), `crates/impulcifer-service/src/recording/{sweep.rs,naming.rs,request.rs}`,
- the three ports of the 2.x speaker-list file-name pattern: `crates/impulcifer-service/src/brir/discovery.rs::recording_speakers`, `crates/impulcifer-dsp/src/stages/room.rs::parse_room_measurement_name`, `crates/impulcifer-service/src/recording/request.rs::filename_speakers`,
- `crates/impulcifer-service/src/lib.rs` (`bootstrap`'s `"sweep"` object) and `crates/impulcifer-service/tests/ipc.rs` (the bootstrap shape test).

Run every command in the foreground. Never use background execution. Do not end your turn before the verification commands have completed and you have written the report.

## Work
1. **Recorder layouts.** Append the four format ids to `SWEEP_TRACK_LAYOUTS` (`"mono","stereo","5.1","7.1","7.1.4","7.1.6","13.1","22.2","24.1.10","30.2"` — 3.x extension; `SEQUENCE_TRACK_ORDERS` stays the 2.x table). Add `pub fn sweep_track_order(layout: &str) -> Option<Vec<&'static str>>` to `constants.rs`: the `SEQUENCE_TRACK_ORDERS` entry, or for a format id the slot of every channel in official order (`"LFE"`/`"LFE2"` for LFE channels). `SweepEstimator::sweep_sequence` and `validate_sweep_spec` use it instead of searching `SEQUENCE_TRACK_ORDERS` directly, so a format layout puts each speaker's sweep on the device channel its slot has in the official order (22.2: `FL` on channel 7, `WL` on channel 1) and LFE channels stay silent. A speaker outside the layout keeps the existing error text (`Speaker "TFL" is not available in the "22.2" layout.`).
2. **Bootstrap.** `bootstrap`'s `"sweep"` object gains `"immersive": [{"id","name","file_name","channels":[{"label","slot"}…]}…]` for every `IMMERSIVE_LAYOUTS` entry in order (the UI's track-layout list and channel map read it; see `apps/impulcifer-app/ui/ipc.d.ts` `ImmersiveLayoutInfo`, do not edit that file). Update the expected object in `crates/impulcifer-service/tests/ipc.rs` accordingly (layouts list and the new key).
3. **Slot codes with digits.** `SL1 SR1 SL2 SR2 BL1 BR1 BL2 BR2` are slot codes. Add `pub fn is_speaker_code(name: &str) -> bool` to `constants.rs`: two or three ASCII capitals, or a member of `SPEAKER_NAMES`. Use it in `recording_speakers` and `parse_room_measurement_name` (the lone `X` placeholder rule is unchanged). In `request.rs::filename_speakers` (a port of a regex *search*), let an element be a known digit code (`SL1` …) when the two capitals are followed by a digit that completes a member of `SPEAKER_NAMES`; everything else is unchanged (`FC,XY1.wav` still reads `FC`, `XY`).
4. **Tests.**
   - Copy `/tmp/claude-0/-home-user-Impulcifer-pip313/4fa230dc-ce20-45b3-97f4-e0b396832274/scratchpad/p26/recording_layouts.rs` verbatim to `crates/impulcifer-service/tests/recording_layouts.rs`.
   - Insert the test function in `/tmp/claude-0/-home-user-Impulcifer-pip313/4fa230dc-ce20-45b3-97f4-e0b396832274/scratchpad/p26/request_unit_test.rs` verbatim into the existing `#[cfg(test)]` module of `request.rs` (next to `filename_speakers_follow_the_python_pattern_with_the_skip_placeholder`).
   - Make them pass without editing them; if you are convinced a test is wrong, stop and explain instead.
   - Every existing test keeps passing, including the 2.x goldens (`golden_brir_objects`, `golden_stages`, the CLI goldens) and `discovery.rs`'s `recording_names_accept_the_skip_placeholder`.

## Allowed files
`crates/impulcifer-types/src/constants.rs` (only `SWEEP_TRACK_LAYOUTS`, the two new functions and their doc comments), `crates/impulcifer-dsp/src/estimator.rs`, `crates/impulcifer-dsp/src/stages/room.rs` (the name parser only), `crates/impulcifer-service/src/recording/{sweep.rs,request.rs,naming.rs}`, `crates/impulcifer-service/src/brir/discovery.rs`, `crates/impulcifer-service/src/lib.rs` (the bootstrap `"sweep"` object only), `crates/impulcifer-service/tests/ipc.rs` (the bootstrap expectation only), `crates/impulcifer-service/tests/recording_layouts.rs` (verbatim copy). Nothing else; in particular not `crates/impulcifer-types/src/layouts.rs`, `features.toml`, `docs/`, `apps/`.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe.
- The 2.x layouts (`stereo` … `7.1.6`) produce exactly the same sweep files and errors as before.
- Never use FFmpeg's `22.2` channel order.

## Verification (foreground; paste the tail of each)
```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy --workspace --exclude impulcifer-app --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types
cargo test -p impulcifer-dsp --test immersive_layouts --test golden_brir_objects --test properties_brir_objects
cargo test -p impulcifer-service --lib
cargo test -p impulcifer-service --test recording_layouts --test recording --test ipc --test brir_inputs
cargo test -p impulcifer-io
```

## Report format
(1) files changed with one line each; (2) verification output tails verbatim; (3) the 24 device channels a `22.2` sweep of all 22 speakers fills, as `channel: slot` pairs; (4) anything you could not do or think is wrong.
