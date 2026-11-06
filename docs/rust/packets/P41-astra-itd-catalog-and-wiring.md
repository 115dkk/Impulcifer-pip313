# P41: catalog strings and screen wiring for the ITD report and correction (ADR 0014)

Repository worktree: `E:\Impulcifer\.worktrees\itd` (Git Bash: `/e/Impulcifer/.worktrees/itd`), branch
`claude/itd-wizard`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\itd\CLAUDE.md`, bullets "3.x 문자열은 오버레이 카탈로그에 쓴다", "바닐라 JS는 타입 검사를
  받는다" and "IPC 형식은 2.x 그대로".
- `E:\Impulcifer\.worktrees\itd\docs\adr\0014-itd-report-and-correction.md` (Korean, the decision) and
  `E:\Impulcifer\.worktrees\itd\docs\rust\ALIGNMENT.md` (the specification: sign conventions, the result JSON, the
  recording checks).

Daybreak (packet P40) has already built the service side: `itd_mode` / `head_radius` config fields, the
`alignment` object in the `start_brir` job result, `lf_ild_db` in recording measurements and reference
measurements, and twelve `cli_*` / `record_check_*` catalog keys with English text in all nine overlay files.
Claude drew the screen: the option rows (`#bf-itd-mode`, `#bf-head-radius`, `#ba-itd-mode`, `#ba-head-radius`
with per-choice hints `[data-itd-mode]`), the report block `#brir-alignment` in the activity card, the view
function `renderAlignment(report)` and the hint helper `updateItdHint(p)` in `apps/impulcifer-app/ui/app.js`, and
the `AlignmentReport` type in `apps/impulcifer-app/ui/ipc.d.ts`. Do not change that markup, the styles in
`styles.css`, or the body of `renderAlignment`.

## 1. Catalog keys (all nine `crates/impulcifer-service/locales/*.json`)

Add the UI keys below to every file. English as given in `en.json`; the eight other languages get real
translations (de, es, fr, ja, ko, ru, zh_CN, zh_TW). Also translate the twelve keys Daybreak added in English
(`cli_itd_report`, `cli_itd_pairing`, `cli_itd_pairing_warning`, `cli_itd_head_turned`, `cli_itd_corrected`,
`cli_itd_skipped`, `cli_readme_itd_title`, `cli_readme_itd_row`, `cli_readme_itd_summary`,
`cli_readme_itd_pairing`, `record_check_input_balance`, `record_check_headphone_balance`); keep their English.

| key | English |
| --- | --- |
| `label_itd_mode` | Ear timing (ITD): |
| `option_itd_mode_off` | As recorded |
| `option_itd_mode_symmetric` | Symmetric pairs |
| `option_itd_mode_model` | Spherical head model |
| `hint_itd_mode_off` | The time differences between the ears stay as recorded. The report after processing shows them for every speaker. |
| `hint_itd_mode_symmetric` | Mirror pairs (FL/FR, SL/SR…) get the same time difference and the center speakers none, which removes a head turned during the recording and a microphone sitting deeper in one ear. |
| `hint_itd_mode_model` | Every speaker gets the time difference of a spherical head at its nominal direction. Front speakers of real heads usually measure smaller than the model. |
| `label_head_radius` | Head radius (cm): |
| `hint_head_radius` | Empty: estimated from the side pair of the recording. |
| `label_itd_report` | Interaural time differences |
| `th_itd_speaker` | Speaker |
| `th_itd_measured` | Measured (µs) |
| `th_itd_model` | Spherical head (µs) |
| `th_itd_after` | Final (µs) |
| `th_itd_lf_ild` | Bass L−R (dB) |
| `hint_itd_report` | Positive values: the sound reaches the left ear first. Bass L−R below 120 Hz should be the same in both ears; what differs is the recording chain. |
| `label_itd_yaw` | Head turned |
| `value_itd_yaw` | {deg}° to the {side} |
| `value_itd_left` | left |
| `value_itd_right` | right |
| `flag_itd_head_turned` | turned |
| `label_itd_offset` | Center offset |
| `label_itd_input_balance` | Recording chain L−R (below 120 Hz) |
| `flag_itd_input_unbalanced` | inputs differ |
| `label_itd_headphone_balance` | Headphone recording L−R |
| `label_itd_predicted_balance` | Final BRIR L−R |
| `flag_itd_pairing_warning` | check the inputs |
| `flag_itd_pairing_ok` | ok |
| `label_itd_hf_asymmetry` | 4–12 kHz asymmetry (left brighter +) |
| `label_itd_head_radius` | Head radius |
| `value_itd_radius_given` | given |
| `value_itd_radius_estimated` | estimated from the side pair |
| `value_itd_radius_default` | default |

Placeholders: `{deg}` is a number with one decimal, `{side}` is the translated `value_itd_left` /
`value_itd_right`, so `value_itd_yaw` must read with either word (in Korean, write it so no particle depends on
the word, e.g. `{side}으로 {deg}°` → use a form that works for both 왼쪽 and 오른쪽). The `flag_*` texts are short
badges (one or two words). `th_*` are table headings; keep `L−R`, `µs`, `dB`, `kHz` as they are. Use each
language's existing words for "speaker", "left", "right", "center", "head" and "recording" (read
`label_balance`, `option_balance_left`, `option_balance_right`, the speaker descriptions, and the 2.x catalog
`i18n/locales/<lang>.json`; do not edit the 2.x catalog). Plain sentences, no exclamation marks, no em dash.

The three `value_itd_radius_*` keys are looked up with a template string in `renderAlignment`, so the catalog
test does not see them; make sure all nine files have them anyway.

## 2. Wiring in `apps/impulcifer-app/ui/app.js`

Only call code, state and event handling; the markup and styles are done.

1. **Request.** In `addTimeArgs(args, p)` add `args.itd_mode = /** @type {ItdMode} */ (val(`${p}-itd-mode`) || "off")`
   and `args.head_radius = numOrNull(`${p}-head-radius`)` (null when empty). Declare both in
   `ProcessingRequest` and `BrirDefaults` in `ipc.d.ts` (`itd_mode?: ItdMode; head_radius?: number | null`),
   following `missing_speakers` / `lfe_mode`. Studio mode: the Timing tab owns `ba-itd-mode` and
   `ba-head-radius`; add them to the tab's field list (search for `head-ms` in the per-tab field ownership
   table around `addStudioAdvancedArgs` / the "Which fields each tab owns" comment) so the Studio reset and the
   on/off switch treat them like `ba-head-ms`. When the tab is off, the request must not carry the two fields
   (same rule as the other Timing fields).
2. **Defaults.** Where `brir_defaults` are applied to the form (search `brirDefault("head_ms"`), set the two
   selects from `brirDefault("itd_mode", "off")` and the radius inputs from `brirDefault("head_radius", null)`
   (empty string when null), for both prefixes.
3. **Hints.** Call `updateItdHint("bf")` and `updateItdHint("ba")` after the strings are applied and on every
   `change` of `#bf-itd-mode` / `#ba-itd-mode` (next to the `bf-lfe-mode` listener; see `updateLfeHint`).
4. **Report.** In `pollJob`, when a `brir` job ends: on `succeeded` call `renderAlignment(job.result?.alignment ?? null)`
   right after `completeSteps()`; on failure or cancel, and when a new BRIR job starts (where `resetSteps(true)`
   runs), call `renderAlignment(null)` so a stale report never stays beside a new run.
5. **Recording references.** `rememberMeasurements` and the payload at `payload.reference_measurements` must
   carry `lf_ild_db` through (`lf_ild_db: item.lf_ild_db ?? null`); add it to the `ReferenceMeasurement` /
   measurement types in `ipc.d.ts` (`lf_ild_db?: number | null`). Headphone recordings now send
   `reference_measurements` as well (the service accepts them); check that the payload builder does not skip
   them for `mode === "headphones"` and that the recording result's `measurements` of a headphone take do not
   get remembered as speaker references (`rememberMeasurements` already returns for non-speaker modes; keep it).
6. **Logs.** The new `cli_itd_*` events arrive translated by the service; nothing to add to `LOG_FIELDS`.

## 3. Gallery and tests

- `apps/impulcifer-app/tests/ui_gallery.py`: the mocked service's `poll_job` result for a finished `brir` job
  must include an `alignment` object in the spec's shape with seven speakers (use values like the spec example:
  FL +288, FR −309, FC −25, SL +882, SR −874, BL +329, BR −327; `mode: "off"`, `yaw_deg: 4.4`,
  `common_offset_us: -20`, `input_balance_db: -6.5`, `headphone_balance_db: 7.0`, `predicted_balance_db: -13.5`,
  `hf_asymmetry_db: 2.1`, `head_radius_cm: 11.2`, `head_radius_source: "estimated"`) so the activity card's
  report renders with the warning badges in the captured screens. Add request assertions beside the
  `missing_speakers` ones: the default request carries `itd_mode == "off"` and `head_radius is None` (or absent
  when the Studio Timing tab is off, whichever the existing rule for `head_ms` is), and after choosing
  "Spherical head model" with radius 9 the request carries `itd_mode == "model"` and `head_radius == 9`.
- `apps/impulcifer-app/tests/contracts` (whatever `npm run test:contracts` runs): extend only if it enumerates
  request fields.

## Allowed files

`crates/impulcifer-service/locales/*.json` (the keys above and the translations of Daybreak's twelve),
`apps/impulcifer-app/ui/app.js` (wiring only: `addTimeArgs`, defaults, listeners, `pollJob`, the Studio tab
field table, `rememberMeasurements` / payload), `apps/impulcifer-app/ui/ipc.d.ts` (request, defaults,
measurement types), `apps/impulcifer-app/tests/ui_gallery.py`, `apps/impulcifer-app/tests/**` contract tests.
Do not touch `index.html`, `styles.css`, the body of `renderAlignment`, any Rust file, docs, or the 2.x tree.

## Verification (foreground; report the tail of each)

```bash
cd /e/Impulcifer/.worktrees/itd/apps/impulcifer-app
npm run typecheck
npm run test:contracts
cd /e/Impulcifer/.worktrees/itd
py -3.14 - <<'PY'
import json, glob, re
files = sorted(glob.glob('crates/impulcifer-service/locales/*.json'))
data = {f: json.load(open(f, encoding='utf-8')) for f in files}
first = set(data[files[0]])
print('same key sets' if all(set(d) == first for d in data.values()) else {f: sorted(first ^ set(d)) for f, d in data.items()})
ph = lambda s: sorted(re.findall(r'\{[a-z_]+\}', s))
bad = [(f, k) for f, d in data.items() for k in d if ph(d[k]) != ph(data[files[0]][k])]
print('placeholders ok' if not bad else bad)
PY
env -u HTTPS_PROXY -u HTTP_PROXY cargo test -p impulcifer-service --lib settings --test ui_catalog
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out "C:/Users/32170336/AppData/Local/Temp/claude/b7093525e1d0c75f/7bcd40cd-140a-5f35-a38f-646c326a8bcc/scratchpad/ui-gallery-itd"
```

The gallery writes about 196 PNGs and its assertions must pass; name the PNG that shows the activity card with
the report. Do not commit, do not start other agents, and end your turn only after reporting the keys in all
nine languages (a table), the app.js functions you changed, and the command outputs.
