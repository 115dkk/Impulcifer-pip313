# P24 (ASTRA): official-order files for NHK 22.2, Auro-3D 13.1, Dolby Atmos 24.1.10 and DTS:X Pro 30.2

You are extending the Impulcifer 3.x Rust workspace at `/home/user/Impulcifer-pip313`. Read first, completely:

- `docs/adr/0007-immersive-layouts.md` (the decision; its rules are the contract),
- `crates/impulcifer-types/src/layouts.rs` (slots, the four formats, `ImmersiveLayout::{track_names, label_track_names, missing}`, `LayoutFiles`, `layout`, `layout_for_file`) and `crates/impulcifer-types/tests/layouts.rs`,
- `crates/impulcifer-types/src/constants.rs` (3.x lists now hold 40 slots; `*_2X` are the 2.x lists),
- `crates/impulcifer-dsp/src/pipeline.rs` (the `WriteBrirs` and `TruehdLayouts` arms are the model: the TrueHD files are the existing "one more file in a fixed order"),
- `crates/impulcifer-service/src/brir/{outputs.rs,run.rs,validation.rs,mod.rs}`, `crates/impulcifer-service/src/lib.rs` (`brir_defaults`), `crates/impulcifer-io/src/brir_layout.rs` (`append_track_names`),
- `crates/impulcifer-types/src/config.rs` and `crates/impulcifer-cli/src/options.rs` (how the 3.x extension field `vbass_mode` is wired: `EXTENSION_FIELD_NAMES`, `EXTENSION_OPTIONS`, `validate_room_options`),
- `crates/impulcifer-service/locales/en.json` and `ko.json` (the 3.x overlay catalog; nine files with one key set) and `crates/impulcifer-service/tests/ui_catalog.rs`,
- `crates/impulcifer-plots/src/charts.rs` around `fn speaker_name`.

Run every command in the foreground. Never use background execution. Do not end your turn before the verification commands have completed and you have written the report.

## What already exists (do not change)
The slot vocabulary and the format tables in `impulcifer-types` are reviewed design. Do not edit `crates/impulcifer-types/src/layouts.rs`, `crates/impulcifer-types/src/constants.rs`, `crates/impulcifer-types/tests/layouts.rs`, `docs/`, `features.toml`, `apps/`, `webview_ui/`, `i18n/`. If you believe a table entry is wrong, say so in the report; do not change it.

## Acceptance tests (already written, currently failing; make them pass without editing them)
- `crates/impulcifer-dsp/tests/immersive_layouts.rs::layout_tracks_read_each_slot_and_ear_from_the_table`
- `crates/impulcifer-service/tests/immersive_layouts.rs` (all five tests)
- `crates/impulcifer-cli/tests/cli.rs::cli_layout_files_option_parses_and_validates`
Every other existing test must keep passing. `crates/impulcifer-service/tests/brir_outputs.rs::output_readonly_file_failure_preserves_existing_bytes` fails in this container because it runs as root (a read-only file stays writable); that one failure is pre-existing and expected.

## Work

1. **Config field.** `ProcessingConfig.layout_files: String`, default `"auto"`, appended to `EXTENSION_FIELD_NAMES` (the 2.x `FIELD_NAMES` stay untouched; `oracle_defaults()` keeps `"auto"`). `validate_room_options()` rejects any value `LayoutFiles::parse` rejects, with `field = "layout_files"` and the parse message as the reason. The service's `brir_defaults` therefore reports `"layout_files": "auto"`; `start_brir` with an invalid value (wrong type or rejected string) answers `INVALID_REQUEST` through the existing validation path.
2. **CLI option.** `--layout_files` in `EXTENSION_OPTIONS` (string, no fixed choices, default suppressed like the others). Help text: `Write the BRIR again in the official channel order of an immersive format: "auto" (default) writes NHK 22.2, Auro-3D 13.1, Dolby Atmos 24.1.10 or DTS:X Pro 30.2 when all of its channels are measured; "none" writes none; a comma-separated list of 22.2, 13.1, 24.1.10, 30.2 also writes those formats when channels are missing (silent).`
3. **DSP.** In `impulcifer_dsp::pipeline`:
   - `pub fn layout_tracks(hrir: &Hrir, layout: &ImmersiveLayout) -> Result<Vec<Vec<f64>>, DspError>`: `hrir.stack_tracks(&layout.track_names(), false)`; tracks of unmeasured slots and of `LFE`/`LFE2` are zeros of the common length. Never compacted.
   - `pub struct LayoutOutput { pub layout: &'static ImmersiveLayout, pub tracks: Vec<Vec<f64>>, pub missing: Vec<&'static FormatChannel> }` and two new `PipelineOutputs` fields: `pub layouts: Vec<LayoutOutput>` (files to write, in `IMMERSIVE_LAYOUTS` order) and `pub layout_notices: Vec<(&'static ImmersiveLayout, Vec<&'static FormatChannel>)>` (near misses).
   - In the `WriteBrirs` arm, after hrir/hesuvi: parse `config.layout_files` (invalid → `DspError::InvalidArgument`), and for every layout compute `missing = layout.missing(|s| hrir.get(s).is_some())`. If `LayoutFiles::writes(id, missing.is_empty())` push a `LayoutOutput`. Otherwise, unless the option is `none`, push a near-miss notice when (a) the measured slots cover at least half of the layout's non-LFE channels and (b) at least one measured slot of the layout lies outside 7.1 (`FL FR FC SL SR BL BR`). No new pipeline stage: progress steps and `stage_table` stay as they are.
4. **Service writer** (`write_outputs_checked`, after `hesuvi.wav`, before the TrueHD files):
   - For every `LayoutOutput`: write `dir/<file_name>` with `write_wav(..., 32)` (PCM_32 like every other output), then always `append_track_names(path, &layout.label_track_names())` (the format's own labels, e.g. `TpFL-left`). Push the path to `files`.
   - Log `success` / `cli_success_layout_file` with params `{"layout": <name>, "path": <path>}`. When `missing` is not empty (a listed, incomplete format) also log `warning` / `cli_warning_layout_partial` with `{"layout": <name>, "file": <file_name>, "missing": <list>}`.
   - For every near-miss notice log `info` / `cli_info_layout_missing` with `{"layout": <name>, "count": <n>, "missing": <list>}`.
   - `<list>` is `"<label> (<slot>), <label> (<slot>), ..."` in official order, e.g. `"BC (BC), TpBC (HBC), BtFC (DFC), BtFL (DFL), BtFR (DFR)"`.
   - Use the existing event levels; the tests read `payload["key"]`, `payload["level"]` (`SUCCESS`, `WARNING`) and `payload["message"]` (the translated text, which must contain the layout name and the list).
5. **README.md** (`render_readme`): when files were written, add after the existing sections a section titled by `cli_readme_layouts_title` with, per file, a line `### <file_name> (<layout name>)`, the translated note `cli_readme_layouts_note` (the file's tracks are two per channel, left ear first, in the format's channel order; LFE is silent), for `24.1.10` also `cli_readme_layouts_atmos_order` (Dolby publishes no channel order above 9.1.6; tracks 1–32 follow the 9.1.6 interchange order of Apple and AWS MediaConvert, the rest the order of the Dolby Atmos Home Theater Installation Guidelines; no decoder outputs this order), for `30.2` also `cli_readme_layouts_dts_order` (the order of the 32-bit channel mask in ETSI TS 103 491 Table B-5; DTS publishes no PCM order for a 30.2 decoder), when channels are missing `cli_readme_layouts_partial` with `{missing}`, and a plain Markdown table **without padding**:
   ```
   | <cli_readme_layouts_header_tracks> | <cli_readme_layouts_header_channel> | <cli_readme_layouts_header_speaker> |
   | --- | --- | --- |
   | 1–2 | L | FL |
   ...
   | 7–8 | LFE | — |
   ```
   Track ranges are 1-based with an en dash (`25–26`); the speaker column is the slot code, or `—` for LFE channels. With no layout file the README is byte-identical to today (the demo README parity tests must keep passing).
   `render_readme` currently has only `ReadmeData`; pass what it needs (e.g. add a `layouts` field to `ReadmeData` filled by the service, or an extra parameter) — your choice, but keep `write_outputs`'s public signature.
6. **Catalog.** Add every new key (`cli_success_layout_file`, `cli_warning_layout_partial`, `cli_info_layout_missing`, `cli_readme_layouts_title`, `cli_readme_layouts_note`, `cli_readme_layouts_atmos_order`, `cli_readme_layouts_dts_order`, `cli_readme_layouts_partial`, `cli_readme_layouts_header_tracks`, `cli_readme_layouts_header_channel`, `cli_readme_layouts_header_speaker`) to all nine overlay files `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json` with the same placeholders in every language. Write real translations in every language, plain and literal; no marketing tone. Format names (`NHK 22.2`, `Auro-3D 13.1`, `Dolby Atmos 24.1.10`, `DTS:X Pro 30.2`), file names, labels and slot codes are never translated. Korean uses the 해요체 like the existing ko entries. English suggestions: success `Wrote {layout} in its channel order: {path}`; partial `{layout}: {file} has silent tracks for channels that were not measured: {missing}`; missing `{layout} needs {count} more speakers for its channel-order file: {missing}`; title `Channel-order files`.
7. **Chart titles.** `impulcifer-plots` `speaker_name` must name every slot: add `impulcifer-types` as a path dependency of `impulcifer-plots` (it has no dependencies of its own) and take the description from `impulcifer_types::layouts::SLOTS` (keep `LFE` → `Subwoofer`, `X` → `Reference microphone`); make the existing `sheet_titles_expand_speakers_without_changing_identifiers` test iterate `SPEAKER_NAMES` instead of its hard-coded list. Update `Cargo.lock` with cargo, not by hand.

## Allowed files
`crates/impulcifer-types/src/config.rs`, `crates/impulcifer-cli/src/options.rs`, `crates/impulcifer-cli/src/lib.rs` (only if needed), `crates/impulcifer-dsp/src/pipeline.rs`, `crates/impulcifer-service/src/brir/{outputs.rs,run.rs,validation.rs,mod.rs}`, `crates/impulcifer-service/src/lib.rs` (only if `brir_defaults` needs it), `crates/impulcifer-dsp/src/stages/readme.rs` (only if you add the README field there), `crates/impulcifer-service/locales/*.json`, `crates/impulcifer-plots/{Cargo.toml,src/charts.rs}`, `Cargo.lock`, and existing tests only where a new `PipelineOutputs`/`ReadmeData` field must be filled in a struct literal. Nothing else.

## Hard rules
- `#![forbid(unsafe_code)]` stays; no `unsafe`.
- hrir.wav, hesuvi.wav, responses.wav and every other existing output must stay byte-identical for every existing scenario; the format files are additional files only.
- Do not use FFmpeg's channel names or its `22.2` order anywhere.
- Tracks are found by name (`stack_tracks` with `track_names()`), never by index arithmetic on hrir.wav.

## Verification (foreground; paste the tail of each)
```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy --workspace --exclude impulcifer-app --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types
cargo test -p impulcifer-dsp --test immersive_layouts
cargo test -p impulcifer-cli
cargo test -p impulcifer-plots
cargo test -p impulcifer-service --test immersive_layouts --test brir_outputs --test brir_optional --test readme_trace --test ui_catalog --test demo_parity --test ipc
cargo test -p impulcifer-policy
```
`impulcifer-policy` will fail on `config.layout_files` missing from `features.toml`; that one failure is expected (Claude registers features). Report it, do not edit `features.toml`.

## Report format
(1) files changed with one line each; (2) the verification output tails verbatim; (3) for the README: paste the section your code writes for the Auro 13.1 file of the community test; (4) anything you could not do or think is wrong in the design or tables.
