# P30: room measurements from a REW or Dirac Live file in the BRIR job, the CLI and IPC

Repository: `/home/user/Impulcifer-pip313`. Read first:

- `/home/user/Impulcifer-pip313/CLAUDE.md` (3.x section: features gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, no new dependencies, `#![forbid(unsafe_code)]`)
- `/home/user/Impulcifer-pip313/docs/adr/0010-room-measurements-from-rew-and-dirac.md`
- `/home/user/Impulcifer-pip313/docs/rust/EXTERNAL_MEASUREMENTS.md` (section "Service" is the contract of this packet)
- the readers from P29 in `/home/user/Impulcifer-pip313/crates/impulcifer-io/src/external/` (`read_external`, `parse_map`, `auto_assign`, `format_map`, `slot_for_label`)

Same clean-room rule as P29: work from the specification and this repository only.

Do not edit anything under `apps/impulcifer-app/ui/` (the screen belongs to another worker) and do not
translate catalog strings (below says what to add).

## Changes

1. **Config** (`crates/impulcifer-types/src/config.rs`): fields `room_measurement_file: Option<String>` and
   `room_measurement_map: Option<String>` (default `None`), appended to `EXTENSION_FIELD_NAMES`. No validation
   beyond what deserialization does; the map is checked against the file when the job loads it.
2. **CLI** (`crates/impulcifer-cli/src/options.rs`): `--room_measurement_file PATH` and
   `--room_measurement_map MAP` in `EXTENSION_OPTIONS`, help text in the style of the neighbours:
   - file: `Room measurements from a REW measurement file (.mdat) or a Dirac Live project (.liveproject) instead of the folder's room-*.wav files.`
   - map: `Which measurement of --room_measurement_file feeds which speaker and ear position, e.g. "FL.left=1,FL.right=2" or "FL=1@1,FR=2@1"; empty or "auto" assigns them from the measurement and speaker names.`
3. **Job input** (`crates/impulcifer-service/src/brir/inputs.rs`, room section, and a new module
   `crates/impulcifer-service/src/brir/room_file.rs`): when room correction is on and `room_measurement_file` is
   set, read the file with `read_external`, resolve the assignment with `parse_map(file, map or "")`, and build
   the room `Hrir` from it: for each assignment, `ImpulseResponse { data: samples, fs: file.sample_rate,
   recording: None }` on the slot's left or right side. If the file's rate differs from the job rate (the
   estimator's `fs`), call `Hrir::resample` to the job rate. Do not read the folder's `room-*.wav` then (they are
   still discovered; log `cli_room_measurement_file_replaces_folder` with `{"count": n}` when there were any).
   No recording pairs are added for virtual room tuning. Microphone calibration precedence:
   `room_mic_calibration`, then the folder file, then the Dirac project's calibration (as a
   `FrequencyResponse` with `frequency` and `raw = gain_db`, then the existing `prepare_mic_calibration`); log
   `cli_room_measurement_file_mic_calibration` when the project's calibration is used. After loading, log
   `cli_room_measurement_file_loaded` with `{"file": <file name>, "format": "REW" or "Dirac Live", "count":
   <assigned responses>}`. A missing file, an unreadable file, an invalid map or an assignment that leaves no
   response is a job error with a clear English message (use the existing `BrirError` variants or add one that
   maps to `INVALID_REQUEST`; a missing file is `FILE_NOT_FOUND` like other missing inputs).
   With room correction off the file is not opened.
4. **IPC** `inspect_room_measurements(request)`: `IpcMethod::InspectRoomMeasurements`, wire name
   `inspect_room_measurements`, in `IpcMethod::ALL` and the policy gate's `CANONICAL_IPC`
   (`crates/impulcifer-policy/tests/gates.rs`). Request `{"path": string, "map": string | null}`. Response
   exactly as the specification's JSON (field names, `skipped` values, canonical `mapping` order, `map` from
   `format_map`, `auto`). Errors: `FILE_NOT_FOUND` for a missing file, `INVALID_REQUEST` for a non-object request,
   an unreadable file or an invalid map (message from the reader).
5. **File picker kind**: `select_file` accepts the kind `measurement`
   (`crates/impulcifer-service/src/lib.rs`), and the app shell's `dialog_filters`
   (`apps/impulcifer-app/src/main.rs`, not the UI folder) maps it to `("REW / Dirac Live files", &["mdat",
   "liveproject"])`.
6. **Catalog keys**: the three log keys already exist in all nine overlay catalogs (a translator added them):
   `cli_room_measurement_file_loaded` (`{count}`, `{file}`, `{format}`),
   `cli_room_measurement_file_replaces_folder` (`{count}`) and `cli_room_measurement_file_mic_calibration`.
   Use these keys and placeholders exactly; do not edit the locale files.
7. **Registry** (`/home/user/Impulcifer-pip313/features.toml`): `room.measurement_files` (kind as the
   neighbouring room features, `status = "implemented"`, tests listing the io and service tests below, note
   citing ADR 0010), `ipc.inspect_room_measurements`, `config.room_measurement_file`,
   `config.room_measurement_map`. Every listed test must exist.
8. **BRIR integrity scenario** (`/home/user/Impulcifer-pip313/.github/scripts/brir_3x_integrity.py`): add
   `("room_measurement_file_rew", ("--room_measurement_file=<absolute path of crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat>",), "3.8.0")`;
   build the absolute path from the script's repository root the way the script already locates files, and
   make sure it still runs from any working directory. Run the scenario locally once (see the script's usage)
   and report its hashes and that two runs are identical.

## Tests (must exist and pass)

- `impulcifer-cli`: `cli_room_measurement_options_parse` (both flags reach the config).
- `impulcifer-service` (`tests/brir_inputs.rs`): `room_measurement_file_replaces_the_folder_room_recordings`
  (demo copy plus the REW fixture with map `FL.left=1,FL.right=2`: only FL room responses, the folder ones are
  not read, the three log keys behave as described) and
  `dirac_project_room_measurements_are_resampled_and_bring_their_calibration` (a Dirac project written by a test
  builder at 44 100 Hz with a calibration, map empty: FL and FR responses at the job rate, the project
  calibration is used when the folder has none and not used when it has one). The test builder may live in
  `crates/impulcifer-service/tests/support/` and follow the layout of the specification.
- `impulcifer-service` (new `tests/room_measurement_file.rs`):
  `inspect_room_measurements_reports_the_rew_fixture_with_its_automatic_map` (three measurements, the third
  skipped; mapping FL left 1 and FL right 2; `map` `FL.left=1,FL.right=2`; `auto` true; nulls and empty lists
  for the Dirac-only fields), `inspect_room_measurements_applies_a_map_and_rejects_bad_requests` (`FL=2` gives
  both sides 2 and `auto` false; `XX=1`, `FL=3` (skipped) and a non-object request are `INVALID_REQUEST`; a
  missing path is `FILE_NOT_FOUND`), and `inspect_room_measurements_reports_a_dirac_project` (builder project:
  title, microphone, `mic_calibration` true, positions, speakers with subwoofer flags and automatic slots,
  measurement ids).
- `impulcifer-service`: a test that `select_file` with kind `measurement` reaches the host with that kind (follow
  the existing select_file test).

## Verification (foreground, repository root)

```bash
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io -p impulcifer-types -p impulcifer-cli -p impulcifer-policy
cargo test -p impulcifer-service --test brir_inputs --test room_measurement_file --test ipc --test ui_catalog
cargo test -p impulcifer-service --lib
```

`output_readonly_file_failure_preserves_existing_bytes` fails in this container only because it runs as root;
ignore that one and say so. Disk space is limited: no `cargo clean`. Do not commit. End your turn only after
reporting.

## Report

Changed files, the new public items, test names, the integrity scenario's result, and the final lines of each
command.
