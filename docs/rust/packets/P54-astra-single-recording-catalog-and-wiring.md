# P54: wire the single recording block and write the catalog (ADR 0016, screen tidy-up)

Repository root: the worktree you are started in. Read first:

- `CLAUDE.md` (3.x workspace section: the overlay catalog rules, `// @ts-check`, `ipc.d.ts`, no TypeScript sources).
- `docs/adr/0016-single-recording-file-and-measured-lfe.md` and `docs/rust/SINGLE_RECORDING.md` ("Configuration").
- The screen is already drawn: `apps/impulcifer-app/ui/index.html` (`#bf-brir-recording-block`), `styles.css`,
  and the show/hide logic in `app.js`. You connect it. Do not change markup or styles. If the wiring needs an
  element that is not there, report it instead of adding it.

## 1. Wiring (`apps/impulcifer-app/ui/app.js`, `apps/impulcifer-app/ui/ipc.d.ts`)

The BRIR source select `#bf-brir-source` has three values: `folder`, `file` (REW/Dirac, existing `mfile` code)
and `recording`. For `recording`:

1. **List check.** `parseRecordingSpeakers(text)` mirrors the Rust rules of
   `docs/rust/SINGLE_RECORDING.md` ("Validation"): comma separated, entries trimmed, each entry a slot code
   (`mfileSlotCodes()`), `LFE`, `LFE2` or `X`; case sensitive; no empty entry, no unknown code, no code twice
   (`X` may repeat), `FL` required. Return the entries, the counts (speakers, LFE, skipped) and the first problem.
2. **Status line** `#bf-brir-recording-status`: set `data-state` (`empty` for an empty field, `ok`, `error`) and
   `textContent` on input and on language change. Texts come from the catalog keys in the table below.
3. **Request.** Where the BRIR request is built (next to `if (mfileMode("brir")) addMfileArgs(args, "brir")`):
   `brir_recording_file` = the path field, `brir_recording_speakers` = the entries joined with `,`,
   `brir_recording_sweep` = the sweep path only when `#bf-brir-recording-sweep-source` is `file`. In the other
   two source modes none of the three is sent.
4. **Start gate.** The job cannot start in `recording` mode with no file, an empty or invalid list, or the sweep
   source `file` with no path. Use the mechanism that `mfileBlocked("brir")` feeds today, with the same
   disabled state and reason line.
5. **Types.** `ProcessingRequest` in `ipc.d.ts` gains the three optional fields. A measurement speaker's `slot`
   and a mapping entry's `slot` may now be `LFE` or `LFE2` for purpose `brir`; adjust the types only if they
   enumerate codes.
6. **State.** If the form's values are saved, restored or reset anywhere (look for the existing `bf-brir-file`
   and `bf-brir-source` handling), cover the new fields the same way.
7. **Gallery.** `apps/impulcifer-app/tests/ui_gallery.py` drives this screen with a mock API. Add a check for
   the recording mode: choosing it shows the block; a valid list gives the `ok` state; an invalid one gives
   `error` and keeps the start button disabled; a started job's request carries the three fields (only if the
   gallery already captures requests; do not build a capture mechanism for this).

## 2. Catalog (`crates/impulcifer-service/locales/*.json`, all nine files)

Every key below gets a real sentence in all nine languages (en, ko, ja, de, es, fr, ru, zh_CN, zh_TW), with the
same placeholders in every file. The English column is the meaning to carry, not a text to copy word for word:
write plain interface language, no marketing tone, no exclamation marks. Keep terms consistent with the
neighbouring keys of each file (how that language already says sweep, speaker, measurement, LFE, room
correction). Of the four `cli_*` keys, `cli_lfe_source_measured` and `cli_brir_measurement_file_lfe` already
exist with English placeholder text in all nine files (replace them with translations); the two
`cli_brir_recording_*` keys do not exist yet (add them: the backend worker that uses them does not touch the
catalog).

`label_brir_recording_file` must not read the same as `label_your_recordings` (the folder field two rows above
it) in any language. In Korean the folder field is '녹음 파일:', so the new label needs another wording, for
example '녹음 WAV:'. The screen's English fallback texts in `index.html` may differ slightly from your English
catalog text; the catalog is what the app shows.

Korean follows the maintainer's rules: polite 합니다체 for descriptions and 하세요 for what the reader should do;
no em dash; no sentence ending in a colon before a list; no 번역투 (`~의 경우`, `~될 수 있다`, `활용하다`,
`경로`, `탐색하다`, `창` for anything but a window, `약하다`/`절반` for truth values, `X에 가깝다`); single
quotes for terms, double quotes only for real speech; no emoji. Read the neighbouring Korean strings first.

| Key | Where | English meaning |
| --- | --- | --- |
| `section_room_tuning2_filter` | group title | Filter |
| `section_room_tuning2_band` | group title | Correction band |
| `section_room_tuning2_protect` | group title | Protection |
| `section_room_tuning2_timing` | group title | Time and phase |
| `section_room_tuning2_speakers` | group title | Between speakers |
| `hint_room_tuning2_high_needs_facts` | under the room facts | The basis and the reach need the room volume or the Schroeder frequency. Without either the upper limit stays at 500 Hz. |
| `option_brir_measurements_recording` | BRIR source option | One recording file |
| `label_brir_recording_file` | field label | Recording file: |
| `label_brir_recording_speakers` | field label | Speaker order: |
| `tooltip_brir_recording_speakers` | i tooltip | List the sweeps of the recording in the order they were played, separated by commas. Subwoofers are LFE and LFE2; a sweep to skip is X. Dirac Live plays the first speaker once more at the end. |
| `label_brir_recording_sweep` | field label | Sweep of this recording: |
| `tooltip_brir_recording_sweep` | i tooltip | Leave this as it is for a recording made with Impulcifer's sweep. For a recording made with another program's sweep, such as Dirac Live or REW, choose that sweep's WAV. It must be a logarithmic sweep; silence before and after it in the file is cut away. |
| `option_brir_recording_sweep_test_signal` | option | Impulcifer's sweep (the test signal setting below) |
| `option_brir_recording_sweep_file` | option | A sweep file from another program |
| `label_brir_recording_sweep_file` | field label | Sweep file: |
| `brir_recording_status` | status line, ok | {speakers} speakers, {lfe} LFE, {skipped} sweeps skipped |
| `brir_recording_error_empty` | status line | There is an empty entry between two commas. |
| `brir_recording_error_unknown` | status line | {entry} is not a speaker code. |
| `brir_recording_error_duplicate` | status line | {entry} is listed twice. Write X for a repeated sweep. |
| `brir_recording_error_needs_fl` | status line | FL is required. The other speakers are aligned to it. |
| `brir_recording_choose` | start gate reason | Choose the recording file and list its speakers. |
| `brir_recording_choose_sweep` | start gate reason | Choose the sweep file this recording was made with. |
| `brir_file_subwoofer` | BRIR measurement table, subwoofer row | A subwoofer. Assigned to LFE, this measurement becomes the LFE track of the channel-order files. |
| `room_file_subwoofer` (rewrite) | room measurement table, subwoofer row | A subwoofer. Room correction is not applied to the LFE track, so this measurement is used only if you assign it to a speaker. |
| `hint_lfe_measured` | LFE tooltip, second paragraph | A subwoofer measurement assigned to LFE is used whatever this setting is. |
| `cli_lfe_source_measured` | job log, README | the measured subwoofer |
| `cli_brir_measurement_file_lfe` | job log | Measured subwoofer channels: {list} |
| `cli_brir_recording_loaded` | job log | BRIR measurements: {count} channels from {file} ({sweeps} sweeps of {seconds} s, {low}–{high} Hz) |
| `cli_brir_recording_replaces_folder` | job log | A recording file is given, so the folder's {count} speaker recordings are not read |

`hint_room_tuning2_high_needs_volume` is no longer used by the screen. Remove it from the nine files if a test
requires every key to be used, otherwise leave it.

## Allowed files

`apps/impulcifer-app/ui/app.js` (call code and state only), `apps/impulcifer-app/ui/ipc.d.ts`,
`apps/impulcifer-app/tests/ui_gallery.py`, `crates/impulcifer-service/locales/*.json`. Nothing else: no
markup, no styles, no Rust sources, no `i18n/locales/` (2.x), no docs.

## Verification (foreground; report the tail of each)

```bash
cd apps/impulcifer-app && npm run typecheck && npm run test:contracts && cd ../..
cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
```

Run the two cargo commands separately (one combined command filters everything out). Another worker may be
editing Rust sources at the same time; if a cargo command fails to compile in a file you did not touch, wait
and rerun once, then report it as it is. Report: files changed, the functions you added, the key list as written
(key, English, Korean), anything you could not connect, and the command outputs. Do not commit.
