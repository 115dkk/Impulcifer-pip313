# P43: catalog and wiring for the virtual room tuning 2 options of 3.13.0 (ADR 0015 bundle 1)

Repository worktree: `E:\Impulcifer\.worktrees\s2r1` (Git Bash: `/e/Impulcifer/.worktrees/s2r1`), branch
`claude/tuning2-s2r-1`. Work only inside it. P42 (backend) is already applied on this branch: the three
settings exist in `ProcessingConfig`, the CLI and the service, and the `cli_room_tuning2_*` log keys and
README keys were added to the nine catalogs with English text. The screen markup is drawn (commit
"feat(app): 가상 룸 튜닝 2 카드에 …"). You connect the two and write the words.

Read first:

- `E:\Impulcifer\.worktrees\s2r1\CLAUDE.md`, "3.x Rust 워크스페이스": the overlay catalogs (nine files, same key
  sets and placeholders, `cargo test -p impulcifer-service --lib settings --test ui_catalog`), `// @ts-check`
  and `apps/impulcifer-app/ui/ipc.d.ts`, `npm run typecheck && npm run test:contracts`.
- `E:\Impulcifer\.worktrees\s2r1\docs\rust\ROOM_CORRECTION.md`, "Virtual room tuning v2": "Options" (the
  three 3.13.0 rows and the `room_tuning2_bands` paragraph) and "Diagnostics" (what the three log keys say).
- `E:\Impulcifer\.worktrees\s2r1\docs\rust\packets\P41-astra-itd-catalog-and-wiring.md` for the conventions
  you followed last time (key naming, how `applyStrings` re-renders, how the gallery checks are written).

## 1. Wiring (`apps/impulcifer-app/ui/app.js`, `apps/impulcifer-app/ui/ipc.d.ts`)

Markup ids (do not change the markup or styles; only the calls and state):

| Control | id | Row id | Request field |
| --- | --- | --- | --- |
| text input | `bf-room-tuning2-bands` | `bf-room-tuning2-bands-row` | `room_tuning2_bands` (string; send the trimmed text, `""` when empty) |
| checkbox | `bf-room-tuning2-dip-limit` | `bf-room-tuning2-dip-limit-row` | `room_tuning2_dip_limit` (boolean) |
| checkbox | `bf-room-tuning2-low-protect` | `bf-room-tuning2-low-protect-row` | `room_tuning2_low_protect` (boolean) |

- `updateRoomRows` (the visibility map that shows `bf-room-tuning2-high-row` and `bf-room-tuning2-mimo-row`
  for `tuning2`): the three rows are visible exactly when the room mode is `tuning2`.
- Defaults (`applyRoomDefaults`, where `bf-room-tuning2-high` and `bf-room-tuning2-mimo` take the bootstrap
  defaults): checkboxes from `defaults.room_tuning2_dip_limit === true` / `room_tuning2_low_protect === true`,
  the text input from `defaults.room_tuning2_bands` (string) or `""`.
- Request (`addRoomArgs` or wherever `args.room_tuning2_mimo` is set): always send the three fields in
  `tuning2` mode, as the other `room_tuning2_*` fields are sent; never in other modes. Do not validate the
  bands text in the browser beyond trimming; the service rejects bad input naming the field and the existing
  error path shows it.
- Any reset-to-defaults or "advanced fields" list that enumerates the tuning2 controls gets the three ids.
- `ipc.d.ts`: `ProcessingRequest` gains `room_tuning2_dip_limit?: boolean; room_tuning2_low_protect?: boolean;
  room_tuning2_bands?: string;` with a comment line in the style of the surrounding tuning2 comment; check
  the `BrirDefaults` `Omit<…>` list and the defaults type so `npm run typecheck` passes.
- If `applyStrings` re-renders hints that mention these controls, nothing new is needed; the labels and
  tooltips are plain `data-i18n` text.

## 2. Catalog (`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`)

New screen keys (English text is in the markup; copy it into `en.json` and translate the other eight):

- `label_room_tuning2_bands`, `tooltip_room_tuning2_bands`
- `checkbox_room_tuning2_dip_limit`, `tooltip_room_tuning2_dip_limit`
- `checkbox_room_tuning2_low_protect`, `tooltip_room_tuning2_low_protect`

Keys P42 added with English text in every file, to translate in the eight non-English files (find them
with `git diff origin/master -- crates/impulcifer-service/locales/en.json`): the `cli_room_tuning2_low_protect`,
`cli_room_tuning2_dip_limit`, `cli_room_tuning2_bands` log lines and the README line keys next to them. Keep
every placeholder (`{speaker}`, `{rolloff}`, `{reduced}`, `{cuts}`, `{limited}`, `{count}`, `{hz}`, `{depth}`,
`{fill}`, `{bands}`) exactly as in English, in every language.

Korean rules (the maintainer reads these): plain sentences, no translationese, no emoji, no em-dash; numeric
ranges with `~` (`1000~3000 Hz`), units with a space before them, "dB" as is. The ko text for the tooltips
should say what the option does to the correction, not restate the English word order. Other languages:
natural, technical register, same facts.

Gate: `cargo test -p impulcifer-service --lib settings --test ui_catalog` (same key sets, same placeholders,
every `data-i18n` key resolved in every language). Do not touch `i18n/locales/` (2.x) or `webview_ui/`.

## 3. Gallery (`apps/impulcifer-app/tests/ui_gallery.py`)

Extend `room_tuning2_checks`: with the room mode `tuning2` the three rows are visible, the request carries
`room_tuning2_dip_limit: false`, `room_tuning2_low_protect: false`, `room_tuning2_bands: ""` by default,
and after ticking both boxes and typing `1000-3000, 6000-9000` it carries `true`, `true` and the trimmed
string; with another room mode none of the three keys is sent. Add one capture pair
(`processing-tuning2-bundle1-studio-{en,ko}-{dark,light}`, or whatever name pattern the existing tuning2
captures use) showing the card with the three controls visible, following the existing capture helpers.

## Verification (foreground, before reporting)

```
cd /e/Impulcifer/.worktrees/s2r1/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts
cd /e/Impulcifer/.worktrees/s2r1 && cargo test -p impulcifer-service --lib settings --test ui_catalog
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out "C:\Users\32170336\AppData\Local\Temp\claude\b7093525e1d0c75f\7bcd40cd-140a-5f35-a38f-646c326a8bcc\scratchpad\ui-gallery-s2r1"
```

(Read the gallery script's usage first; the checks run before the captures, so exit 0 means the checks
passed.)

## Report (then end your turn)

1. Files changed, one line each.
2. The key list you added or translated, per file count.
3. The three commands' final lines; the gallery's PNG count and the names of the new captures.
4. Anything ambiguous in the markup or the spec that you resolved, and how.
