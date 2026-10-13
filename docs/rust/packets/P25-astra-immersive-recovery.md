# P25 (ASTRA): output recovery for the immersive channel-order files

You are extending the Impulcifer 3.x Rust workspace at `/home/user/Impulcifer-pip313`. Read first, completely:

- `docs/adr/0007-immersive-layouts.md` (rule 6 is this packet; rules 3–5 define the files),
- `crates/impulcifer-types/src/layouts.rs` (`IMMERSIVE_LAYOUTS`, `ImmersiveLayout::{track_names, label_track_names, missing}`, `layout_for_file`),
- `crates/impulcifer-service/src/recovery.rs` (all of it) and `crates/impulcifer-service/tests/recovery.rs`,
- `crates/impulcifer-io/src/brir_layout.rs` (`read_track_names`, `append_track_names`),
- `crates/impulcifer-service/src/brir/outputs.rs` (how the BRIR job now writes the format files: PCM_32, then `append_track_names(path, label_track_names())`, never compacted).

Run every command in the foreground. Never use background execution. Do not end your turn before the verification commands have completed and you have written the report.

## What recovery must do now (ADR 0007 rule 6)
The speaker vocabulary grew to 40 slots (`SPEAKER_NAMES`), so hrir.wav may now hold 16–82 tracks and hesuvi.wav 14–80, and Hangloose files may be named after any slot, digits included (`SL1.wav`, `BL2.wav`). Besides hrir.wav, hesuvi.wav and Hangloose, recovery now knows the four format files (`nhk_22.2.wav`, `auro_13.1.wav`, `atmos_24.1.10.wav`, `dtsx_30.2.wav`; `layout_for_file`).

1. **Reading a format file.** Found case-insensitively like hrir.wav (same ambiguity rule). It must hold exactly `2 × channels` tracks (otherwise `InvalidChannelCount` with `expected: [2 × channels]`); if it carries an `ICHL` chunk it must equal `label_track_names()` (otherwise `InvalidChannelMap`); its tracks map **by name** through `track_names()` onto the slot tracks (`WL-left` …), never by index; its LFE/LFE2 tracks must be silent (`NonSilentLfe`, `details.tracks` listing the label track names such as `"LFE-left"` or `"LFE1-left"`); same NaN/empty checks as `read_matrix`.
2. **Source precedence** stays hrir+hesuvi → hrir → hesuvi → Hangloose. Only when none of those exists do the format files become the source: `source_kind = "layout"`, `source_path` = the first format file in `IMMERSIVE_LAYOUTS` order; when several exist they are merged (union of slots) and must agree sample for sample where they share a slot (`SourceMismatch`), with the same sample rate and count (`SampleRateMismatch`, `SampleCountMismatch`).
3. **Existing format files are checked against the source** like existing Hangloose files (`verify_subset`): same rate and count, and every non-silent slot track equal; the error `details` are `{"files": [path], "tracks": [slot track names that differ]}` (e.g. `["HBL-left"]`). They are listed in `existing_files`.
4. **Planning.** After hrir.wav / hesuvi.wav (and Hangloose when requested, unchanged), plan every missing format file whose non-LFE channels are all measured in the recovered set (`layout.missing(..).is_empty()`), in `IMMERSIVE_LAYOUTS` order, `kind = "layout"`, `speaker: None`, `channels = 2 × channels`. Write it like the BRIR job: tracks from the set by `track_names()` (zeros for LFE), PCM_32 through the existing staged writer, then `append_track_names(label_track_names())` as the writer's metadata step for that file. Never compacted, even with `remove_silent_channels`.
5. **Speakers** in plans and results stay in `SPEAKER_NAMES` order.
6. The 2.x golden scenarios stay exact except the documented 3.x extension (next section).

## Acceptance tests
- New file `crates/impulcifer-service/tests/recovery_layouts.rs`: copy it verbatim from `/tmp/claude-0/-home-user-Impulcifer-pip313/4fa230dc-ce20-45b3-97f4-e0b396832274/scratchpad/p25/recovery_layouts.rs`. Make all five tests pass **without changing them**. If you are convinced a test is wrong, stop and explain in the report instead of editing it.
- `crates/impulcifer-service/tests/recovery.rs` must keep passing. Two 2.x golden error scenarios are no longer errors in 3.x: `wrong_count_hrir_34` (34 tracks fit the 3.x hrir order of 82) and `wrong_count_hesuvi_32` (32 fit hesuvi's 80); and every other `INVALID_CHANNEL_COUNT` scenario now names the 3.x range (`"hrir.wav must contain 16–82 channels in complete stereo pairs."`, `expected` = 16, 18, …, 82; hesuvi 14–80). Adapt **only the comparison** in `golden_recovery_errors_match_python`: (a) skip those two scenarios there and add a new test `wrong_counts_of_2x_are_valid_3x_extensions` that runs them and asserts success, `speakers` empty, and that the planned/written files exist; (b) before comparing an `INVALID_CHANNEL_COUNT` error of hrir.wav/hesuvi.wav, rewrite the oracle's `message` and `details.expected` to the 3.x range computed from `HEXADECAGONAL_TRACK_ORDER.len()` / `HESUVI_TRACK_ORDER.len()` — keep `path` and `actual` from the oracle. Put a comment naming ADR 0007 above each adaptation. Do not touch the golden JSON.

## Allowed files
`crates/impulcifer-service/src/recovery.rs`, `crates/impulcifer-service/tests/recovery.rs` (only the two adaptations above), `crates/impulcifer-service/tests/recovery_layouts.rs` (verbatim copy). Nothing else. Do not edit `crates/impulcifer-types/**`, `features.toml`, `docs/`, `apps/`.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe. Recovery never applies gain, DSP or resampling.
- Existing error codes, messages and JSON keys stay as they are; new behaviour uses the existing codes.
- Tracks are always matched by name (`track_names()`), never by index arithmetic.

## Verification (foreground; paste the tail of each)
```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service --test recovery --test recovery_layouts --test ipc
```

## Report format
(1) files changed with one line each; (2) verification output tails verbatim; (3) how a format file's tracks are matched to slots (point to the code); (4) anything you could not do or think is wrong.
