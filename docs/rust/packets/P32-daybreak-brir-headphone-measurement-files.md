# P32: BRIR and headphone measurements from a REW or Dirac Live file

Repository: `/home/user/Impulcifer-pip313`. Read first:

- `/home/user/Impulcifer-pip313/CLAUDE.md` (3.x section: features gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, no new dependencies, `#![forbid(unsafe_code)]`)
- `/home/user/Impulcifer-pip313/docs/adr/0010-room-measurements-from-rew-and-dirac.md`
- `/home/user/Impulcifer-pip313/docs/adr/0011-brir-and-headphone-measurements-from-rew-and-dirac.md` (Korean; the decision)
- `/home/user/Impulcifer-pip313/docs/rust/EXTERNAL_MEASUREMENTS.md`, sections "Purposes" (all subsections) and
  "Service" / "BRIR and headphone measurement files (ADR 0011)". These sections are the contract of this packet;
  where this packet and the spec disagree, stop and report instead of choosing.
- the existing room implementation this packet mirrors: `crates/impulcifer-io/src/external/assign.rs`,
  `crates/impulcifer-service/src/brir/room_file.rs`, `crates/impulcifer-service/src/brir/inputs.rs`,
  `crates/impulcifer-service/src/brir/estimator.rs`, `crates/impulcifer-types/src/config.rs`,
  `crates/impulcifer-cli/src/options.rs`, and their tests (`crates/impulcifer-io/tests/external_rew.rs`,
  `crates/impulcifer-service/tests/brir_inputs.rs`, `crates/impulcifer-service/tests/room_measurement_file.rs`,
  `crates/impulcifer-service/tests/support/room_measurement.rs`, `crates/impulcifer-cli/tests/room_measurement.rs`).

Clean-room rule (ADR 0010): work from the specification and this repository only. Do not look for REW or Dirac
program code, decompilations or hooking material.

## Boundaries

- Do not edit anything under `apps/impulcifer-app/` (the screen belongs to another worker).
- Do not edit `crates/impulcifer-service/locales/*.json` or `i18n/locales/*.json`. Another worker adds the
  catalogue strings for the seven new log keys in parallel. Until then
  `cargo test -p impulcifer-service --test ui_catalog` fails with exactly those seven keys missing
  (`brir_job_log_keys_resolve_in_every_language`); that failure is expected, report it, do not work around it.
- Do not edit `docs/`, `README.md`, `CHANGELOG.md`, `Cargo.toml` versions, `features.toml` or `.github/`. Report
  the test names for `features.toml` instead (format below).
- No new dependencies. No `unsafe`.
- Room behaviour must not change: `auto_assign`, `parse_map`, `format_map` keep their current results, and the
  room loader keeps ignoring `start_time`.

## Changes

1. **`impulcifer-io` assignment** (`crates/impulcifer-io/src/external/assign.rs`): add
   `pub enum Purpose { Room, Brir, Headphones }` (`Clone, Copy, Debug, Eq, PartialEq, Hash`) with
   `pub fn parse(&str) -> Option<Purpose>` (`"room"`, `"brir"`, `"headphones"`, exact lowercase) and
   `pub fn as_str(self) -> &'static str`; `auto_assign_for`, `parse_map_for`, `format_map_for`,
   `check_assignments` exactly as the spec's "Purposes" section describes (signatures there). The existing
   functions become the `Room` case (`auto_assign(f) == auto_assign_for(f, Purpose::Room)` and so on). Keep the
   error texts of the spec verbatim. Headphone assignments use slot `"FL"` with `EarSide::Left` and slot `"FR"`
   with `EarSide::Right`.
2. **Config** (`crates/impulcifer-types/src/config.rs`): `brir_measurement_file`, `brir_measurement_map`,
   `headphone_measurement_file`, `headphone_measurement_map`, all `Option<String>` defaulting to `None`,
   appended to `EXTENSION_FIELD_NAMES` after the two room fields. Follow whatever the room fields do in
   `from_kwargs`, IPC validation (`crates/impulcifer-service/src/brir/validation.rs`) and anywhere else
   `room_measurement_file` appears (grep the workspace).
3. **CLI** (`crates/impulcifer-cli/src/options.rs`): four options in `EXTENSION_OPTIONS`, after the room ones:
   - `--brir_measurement_file`: `Speaker measurements at both ears from a REW measurement file (.mdat) or a Dirac Live project (.liveproject) instead of the folder's speaker recordings.`
   - `--brir_measurement_map`: `Which measurement of --brir_measurement_file feeds which speaker and ear, e.g. "FL.left=1,FL.right=2" or "FL.left=1@1,FL.right=1@2"; empty or "auto" assigns them from the measurement and speaker names.`
   - `--headphone_measurement_file`: `Headphone measurement from a REW measurement file (.mdat) or a Dirac Live project (.liveproject) instead of headphones.wav.`
   - `--headphone_measurement_map`: `Which measurement of --headphone_measurement_file is the left earcup at the left ear and which the right earcup at the right ear, e.g. "left=1,right=2"; empty or "auto" picks them from the measurement names.`
4. **Service loaders**: rename `crates/impulcifer-service/src/brir/room_file.rs` to `measurement_file.rs`
   (`git mv`; update `mod` and every caller, including the IPC dispatch in `crates/impulcifer-service/src/lib.rs`).
   It holds `load_room` (the current `load`, unchanged behaviour), `load_brir`, `load_headphones` and `inspect`.
   Share the file reading and the missing-file mapping (`FILE_NOT_FOUND`) the room loader already has.
   - `load_brir(path, map, fs, events) -> Result<(Hrir, ExternalFormat), BrirError>`: `parse_map_for(Brir)`,
     `check_assignments(Brir)`, then "Building the BRIR responses" of the spec (time axis, 10 s and overlap
     errors, slot order, resample), then the time-difference log lines (`cli_brir_measurement_file_itd`, `_itd_large`,
     `_itd_zero`) with the exact argument shapes of the spec. Slot azimuth is `impulcifer_types::layouts::SLOTS`
     (`azimuth`). Use `ImpulseResponse::peak_index(0, None, 0.12589)`; check its signature first. Errors from
     the map or the checks map to `INVALID_REQUEST` like the room loader's.
   - `load_headphones(path, map, fs) -> Result<(Hrir, ExternalFormat, String, String), BrirError>` (the last two
     are the left and right measurement names): `parse_map_for(Headphones)`, `check_assignments(Headphones)`,
     `Hrir` with `FL` left and `FR` right only, both cut to the shorter length, resampled to `fs`.
   - No microphone calibration from the file for either (ADR 0011, point 6).
5. **Job input** (`crates/impulcifer-service/src/brir/inputs.rs`):
   - One function opens the HRIR for both places that call `ingest_measurements` today (the early opening for
     virtual bass and the final opening). With `config.brir_measurement_file` set it calls `load_brir` (once
     per job: when the early opening ran, reuse its result as today) and logs
     `cli_brir_measurement_file_replaces_folder` `{"count": dir.recordings.len()}` when the folder has speaker
     recordings and `cli_brir_measurement_file_loaded` `{"file", "format": "REW" | "Dirac Live", "count": <slots>}`.
     Folder speaker recordings are not opened then. Without the field, behaviour is unchanged.
   - Headphone section: with `config.headphone_measurement_file` set (and compensation on), skip the folder's
     headphone messages (`cli_info_hp_param_provided`, `cli_info_hp_default`, `cli_info_using_hp_file`,
     `cli_error_hp_file_missing`, …), log `cli_headphone_measurement_file_replaces_folder` `{"file": <name of
     dir.headphones>}` when `dir.headphones` is `Some`, call `load_headphones`, log
     `cli_headphone_measurement_file_loaded` `{"file", "format", "left", "right"}`, then continue exactly as the
     recording path does (`headphone-responses.wav`, `headphone_compensation`, plot). With compensation off the
     file is not opened.
   - Every new `events.log` call must use a string literal key in the form `events.log("info", "cli_…", …)`
     (the catalogue test extracts keys with a regex over `src/brir/*.rs`).
6. **Estimator** (`crates/impulcifer-service/src/brir/estimator.rs`, `run.rs`): when `brir_measurement_file` is
   set and the folder has no speaker recording, the auto path falls back to the default sweep without logging
   `cli_warning_sweep_detect_fallback`. Pass what you need through `open_with_events`; keep `open_estimator`'s
   public signature.
7. **IPC** `inspect_room_measurements`: optional `purpose` (missing or `null` → room; a string from
   `Purpose::parse`; anything else `INVALID_REQUEST` with `purpose must be room, brir or headphones.`). The
   response adds `"purpose"`; `mapping`, `map`, `auto` follow the purpose. No `check_assignments` call. No new
   IPC method.

## Tests (all must exist and pass; names are yours, report them)

- `impulcifer-io` unit tests in `assign.rs` (or a test file) covering at least: REW `brir` auto (titles with and
  without an ear; a measurement without an ear is not assigned; first wins), Dirac `brir` auto (two positions →
  lowest left, next right; one position → both ears the same id), REW `headphones` auto (ear words; FL/FR names;
  the two-measurement fallback; no fallback with three), Dirac `headphones` auto; `parse_map_for` rejections
  (bare `SLOT=ID` for brir, `FL.left=` for headphones, `left` twice, unknown/skipped ids), canonical output of
  `format_map_for` for both, each `check_assignments` error message, and that the `Room` versions equal the old
  functions on the REW fixture and a Dirac project.
- Extend `crates/impulcifer-service/tests/support/room_measurement.rs` so a test can build a Dirac project whose
  elements have different peak positions (for example a per-element delay argument), keeping `dirac_project(fs)`
  as it is for the existing tests.
- Service tests: BRIR from the REW fixture `crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat` (its
  automatic `brir` map is `FL.left=1,FL.right=2`; check the leading zeros of the later ear equal
  `round((start_right - start_left) * 48000)` computed from the fixture's start times, both ears the same
  length, poisoned folder speaker recordings are not opened, the two log lines); BRIR from a Dirac project with
  a right-ear position delayed by more than 1 ms (the `_itd_large` warning for each speaker) and one without
  delay (the `_itd_zero` warning); the incomplete-map, missing-FL and missing-file errors; headphones from the
  REW fixture with `left=1,right=2` (compensation computed, `headphone-responses.wav` written, poisoned
  `headphones.wav` not opened, log lines) and with compensation off (file not opened); one full BRIR job run
  through the same entry point other job tests use, with a temporary folder holding no speaker recordings and
  `brir_measurement_file` set, that writes `hesuvi.wav` and does not log `cli_warning_sweep_detect_fallback`.
- IPC tests for `purpose` (each value, the default, an invalid value, the brir/headphones `map` strings).
- CLI test that the four options reach the config.

## Verification (foreground, all before you report)

```bash
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io
cargo test -p impulcifer-types
cargo test -p impulcifer-cli
cargo test -p impulcifer-service
cargo test -p impulcifer-policy
```

Run every command in the foreground and finish your turn only after reporting.

## Report

1. Files changed, one line each.
2. Output tails of every verification command (the only acceptable failure is the `ui_catalog` one named above;
   paste its message).
3. The new log keys you emit and where (file:line).
4. For `features.toml`, the test paths in the form `crate-name::test_fn` (integration tests as
   `crate-name::<test file stem>::test_fn` if that is how the existing entries name them; look at
   `room.measurement_files` in `/home/user/Impulcifer-pip313/features.toml` and copy its style).
5. Anything in the spec you found ambiguous and how you resolved it.
