# P34: HDMI device channel order on macOS, and a check for an input that recorded only silence

Repository: `/home/user/Impulcifer-pip313`. Read `/home/user/Impulcifer-pip313/CLAUDE.md` first (3.x workspace
section: `#![forbid(unsafe_code)]`, no new dependencies, `features.toml`, `EXTENSION_*` lists). The decision
record is `/home/user/Impulcifer-pip313/docs/adr/0012-macos-hdmi-channel-order.md` (being written in parallel; this
packet is self-contained).

Two bugs were reported by a tester on an Apple silicon Mac with an AV receiver on HDMI.

## 1. HDMI channel order on macOS

### Problem

The recorder plays a generated sweep sequence with one row per device channel, in the order of
`impulcifer_types::constants::sweep_track_order(layout)`. For 7.1 that is `FL FR FC LFE BL BR SL SR` (the Windows
WAVE order). On Windows the HDMI driver re-maps this to the HDMI slots. On macOS the HDMI and DisplayPort output
devices expose the HDMI slots directly (CEA-861): channel 3 is LFE, channel 4 is FC, channels 5-6 are the surround
pair, channels 7-8 the surround back pair. The tester heard the FC sweep from the subwoofer. BL/BR and SL/SR are
also swapped on such a device.

### Design (fixed; do not change names or semantics)

**Types** (`crates/impulcifer-types/src/constants.rs`, next to `sweep_track_order`):

```rust
/// The device channel order of an HDMI or DisplayPort output on macOS (CEA-861 slots, ADR 0012), for the
/// layouts whose first eight channels map onto them: LFE on 3, FC on 4, and for the 2.x 7.1 family the side
/// pair on 5-6 and the back pair on 7-8. Channels after the eighth keep their order. `None` for other layouts.
pub fn hdmi_track_order(layout: &str) -> Option<Vec<&'static str>>
```

Rule: start from `sweep_track_order(layout)`. Only for `5.1`, `7.1`, `7.1.4`, `7.1.6`, `9.1`, `9.1.4`, `9.1.6`
return `Some`; every other layout (`mono`, `stereo`, `13.1`, `22.2`, `24.1.10`, `30.2`) returns `None`. Swap
the slots at index 2 and 3 (FC, LFE). If the slots at index 4..8 are exactly `BL BR SL SR`, make them
`SL SR BL BR`. Resulting orders:

| layout | HDMI order |
| --- | --- |
| 5.1 | FL FR LFE FC BL BR |
| 7.1 | FL FR LFE FC SL SR BL BR |
| 7.1.4 | FL FR LFE FC SL SR BL BR TFL TFR TBL TBR |
| 7.1.6 | FL FR LFE FC SL SR BL BR TFL TFR TSL TSR TBL TBR |
| 9.1 / 9.1.4 / 9.1.6 | `sweep_track_order` with index 2 and 3 swapped only (their 5-8 are already SL SR BL BR) |

Assert in a unit test that each HDMI order is a permutation of the standard order and equals the table.

**Service** (new module `crates/impulcifer-service/src/recording/channel_order.rs`, declared in
`recording/mod.rs`):

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChannelOrder { #[default] Auto, Standard, Hdmi }
impl ChannelOrder {
    pub fn parse(value: &str) -> Option<Self>;      // "auto" | "standard" | "hdmi", exact
    pub fn as_str(self) -> &'static str;
}

/// Names of the HDMI and DisplayPort audio devices in the JSON of `system_profiler -json SPAudioDataType`.
/// Pure and platform independent. Walk the whole JSON value recursively; every object that has a string
/// `_name` and a string `coreaudio_device_transport` whose lowercase value contains "hdmi" or "displayport"
/// contributes its `_name`. Invalid JSON gives an empty list.
pub fn hdmi_output_names(system_profiler_json: &str) -> Vec<String>;

/// On macOS run `system_profiler -json -timeout 10 SPAudioDataType` (std::process::Command, stdout only,
/// stderr to null) and return `hdmi_output_names` of its output; any failure gives an empty list. On other
/// platforms always an empty list without running anything.
pub fn hdmi_outputs() -> Vec<String>;

/// The layout of the playback when it is one that has an HDMI order: the sweep spec's `tracks` for a
/// generated sweep; for a playback file, the layout token of an Impulcifer sweep file name
/// (`sweep-seg-<speakers>-<layout>-...`, reuse the parser in `impulcifer_io::sweep_files` if it exposes the
/// layout, otherwise parse the token after the speaker list) but only when the file's channel count equals
/// that layout's channel count. Otherwise None.
pub fn playback_layout(sweep: Option<&SweepSpec>, play_path: Option<&str>, channels: usize) -> Option<String>;

/// Rows reordered from `sweep_track_order(layout)` to `hdmi_track_order(layout)`: the row of slot S moves to
/// S's index in the HDMI order. Returns the input unchanged when the layout has no HDMI order or the row count
/// differs from the layout's channel count.
pub fn to_hdmi_order(tracks: Vec<Vec<f64>>, layout: &str) -> Vec<Vec<f64>>;
```

**Request** (`recording/request.rs`): new optional field `channel_order` (string `auto` | `standard` |
`hdmi`, missing or null = `auto`). Anything else is an `INVALID_REQUEST` validation error in the same style as
`share_mode` (`"channel_order must be auto, standard or hdmi."`, details `{"channel_order": <raw>}`).
`ValidatedRecording` gains `pub channel_order: ChannelOrder`. Keep the 2.x golden validation test passing (see
how `share_mode` is listed as a 3.x-only key around line 240 and do the same).

**Run** (`recording/run.rs`, right after `resolve_devices` and before the truncation warning): when
`playback_layout(...)` is `Some(layout)`:

- effective order: `Hdmi` if requested `Hdmi`; `Standard` if requested `Standard`; for `Auto`, `Hdmi` when
  `hdmi_outputs()` contains `output.name` exactly, else `Standard`. Call `hdmi_outputs()` only for `Auto`.
- if `Hdmi`, `tracks = to_hdmi_order(tracks, &layout)`.
- log one line (always, for both orders, only when a layout was found):
  `ctx.log(json!({"level":"info","key": <"recording_channel_order_hdmi" | "recording_channel_order_standard">,
  "message": <English text below>, "channels": <string>, "auto": <bool>}))`. `channels` is
  `"1 FL, 2 FR, 3 LFE, 4 FC, 5 SL, 6 SR, 7 BL, 8 BR"` (1-based device channel, slot; LFE slots included). English
  messages: `"HDMI channel order: {channels}"` and `"Device channel order: {channels}"` with `{channels}`
  substituted.
- the job result gains `"channel_order": "hdmi" | "standard"` (absent when no layout was found).

**Device list** (`recording/devices.rs::list_audio_devices`): each device object gains `"hdmi": bool`, true when
its name is in `hdmi_outputs()` (call it once per listing, not per device). On Windows and Linux it is always
false.

**Bootstrap** (`crates/impulcifer-service/src/lib.rs`, the `"sweep"` object): add
`"hdmi_orders": { "<layout>": [slots...] }` for every layout in `SWEEP_TRACK_LAYOUTS` whose
`hdmi_track_order` is `Some`, and `"track_orders": { "<layout>": [slots...] }` with `sweep_track_order` for
every layout that has one. The screen uses them to show the device channel map.

## 2. An input that recorded only silence

### Problem

macOS hands an app without microphone permission a stream of exact zeros. The recording then finishes
"successfully" and processing fails later or produces nothing useful.

### Design

In `run.rs`, after `tracks` are de-interleaved: when every sample of every captured channel is exactly `0.0`
(and there is at least one sample), the result's `checks` is exactly one check (replacing whatever
`check_recording` returned, and present in headphone mode too):

```json
{"key": "<record_check_input_silent_macos on macOS, record_check_input_silent elsewhere>",
 "speaker": "", "severity": "error", "values": {}}
```

Use `cfg!(target_os = "macos")` for the key choice. Build it with the existing `RecordingCheck` type
(`recording/checks.rs`) so it serializes the same way. Also log it:
`ctx.log(json!({"level":"error","key":<same key>,"message":"The input recorded only digital silence."}))`.
The file is still saved (keep behaviour otherwise unchanged). `result["measurements"]` stays as
`check_recording` returned it in speaker mode.

## Tests (all must exist and pass)

- types: `hdmi_track_order_matches_cea_861_slots` (table above, permutation property, `None` cases).
- service unit tests in `channel_order.rs`:
  - `hdmi_output_names_reads_system_profiler_json`: a fixture string in the test with the real shape
    `{"SPAudioDataType":[{"_items":[{"_name":"LG TV","coreaudio_device_transport":"coreaudio_device_type_hdmi",...},
    {"_name":"DELL U2720Q","coreaudio_device_transport":"coreaudio_device_type_displayport"},
    {"_name":"MacBook Pro Speakers","coreaudio_device_transport":"coreaudio_device_type_builtin"},
    {"_name":"Scarlett 18i20","coreaudio_device_transport":"coreaudio_device_type_usb"}],"_name":"coreaudio_device"}]}`
    gives exactly `["LG TV", "DELL U2720Q"]`; invalid JSON gives `[]`.
  - `to_hdmi_order_moves_each_row_to_its_hdmi_slot` (7.1 rows tagged by index; also 5.1; wrong row count and
    stereo unchanged).
  - `playback_layout_from_spec_and_file_name`.
  - `#[cfg(target_os = "macos")] system_profiler_runs_on_this_mac`: `hdmi_outputs()` returns without panicking
    and every returned name is non-empty (the macOS CI runner has no HDMI device; this proves the command and
    the JSON shape).
- service integration in `crates/impulcifer-service/tests/recording.rs` with the existing `FakeBackend`
  (extend it to remember the interleaved playback and its channel count if it does not already):
  - `hdmi_channel_order_moves_fc_to_device_channel_four`: generated 7.1 sweep of `FC` alone with
    `channel_order: "hdmi"` puts the sweep energy on device channel index 3 and silence on index 2; with
    `"standard"` the reverse; the result carries `channel_order` and the log has the matching key and
    `"3 LFE, 4 FC"` in `channels` for hdmi. With `"auto"` on a non-macOS host the order is standard.
  - `channel_order_request_validation` (invalid value rejected with the message above; missing = auto).
  - `silent_input_is_reported_as_one_error_check`: the fake input returns zeros; speaker and headphone modes both
    give the single check with the platform key; a fake input with any non-zero sample does not.
  - `list_audio_devices` test: every device has `"hdmi": false` on non-macOS hosts.
- `features.toml`: add, with `status = "implemented"` and the real test paths in the existing format,
  `recording.hdmi_channel_order` and `recording.input_silence_check`. Do not add other entries.

## Allowed files

`crates/impulcifer-types/src/constants.rs`, `crates/impulcifer-service/src/recording/*.rs` (new
`channel_order.rs`), `crates/impulcifer-service/src/lib.rs` (bootstrap only),
`crates/impulcifer-service/tests/recording.rs`, `crates/impulcifer-service/tests/ipc.rs` (only if a bootstrap or
device-list shape assertion needs the new keys), `features.toml`. Do not touch `apps/`, `crates/*/locales/`,
`.github/`, docs, or `Cargo.toml`/`Cargo.lock`. No `unsafe`, no new dependencies.

## Verification (foreground; report the tail of each)

```bash
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types
cargo test -p impulcifer-service --lib recording
cargo test -p impulcifer-service --test recording --test ipc --test recording_layouts --test recording_checks
cargo test -p impulcifer-policy
```

Run every command in the foreground and end your turn only after reporting. Report: the files changed, the
public signatures as written, the test names, and the command outputs (last lines). Do not commit.
