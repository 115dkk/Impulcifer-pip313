# P47: catalog and wiring for the virtual room tuning 2 options of 3.15.0 (ADR 0015 bundle 3)

Repository worktree: `E:\Impulcifer\.worktrees\s2r3` (Git Bash: `/e/Impulcifer/.worktrees/s2r3`), branch
`claude/tuning2-s2r-3`. Work only inside it. P46 (backend) is applied on this branch: the settings
`room_tuning2_high_basis`, `room_tuning2_high_transition`, `room_tuning2_input_hp` exist in
`ProcessingConfig`, the CLI and the service, and five `cli_room_tuning2_*` log keys were added to the nine
catalogs with English text. The screen markup is drawn (two selects, one hint and one number field in the
tuning 2 card), and its nine screen keys are already in all nine catalogs in English. You connect the two
and write the words.

Read first:

- `E:\Impulcifer\.worktrees\s2r3\CLAUDE.md`, "3.x Rust 워크스페이스": the overlay catalogs (nine files, same key
  sets and placeholders), `// @ts-check` and `apps/impulcifer-app/ui/ipc.d.ts`,
  `npm run typecheck && npm run test:contracts`.
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\ROOM_CORRECTION.md`, "Virtual room tuning v2": "Options" (the
  three 3.15.0 rows and the paragraph after the 3.14.0 paragraph), "Correction band" ("Upper-limit basis and
  transition"), "Applying to the BRIR" ("Input high-pass"), "Pair phase" (what they do) and "Diagnostics"
  (what the log keys say).
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\packets\P45-astra-tuning2-s2r-2-catalog-and-wiring.md`, the
  bundle-2 packet: the same conventions apply.

## 1. Wiring (`apps/impulcifer-app/ui/app.js`, `apps/impulcifer-app/ui/ipc.d.ts`)

Markup ids (do not change the markup or styles; only the calls and state):

| Control | id | Row id | Request field |
| --- | --- | --- | --- |
| select | `bf-room-tuning2-high-basis` | `bf-room-tuning2-high-basis-row` | `room_tuning2_high_basis` (`"strict"` / `"lenient"`) |
| select | `bf-room-tuning2-high-transition` | `bf-room-tuning2-high-transition-row` | `room_tuning2_high_transition` (number 1, 2 or 4) |
| hint | `bf-room-tuning2-high-volume-hint` | | none |
| number | `bf-room-tuning2-input-hp` | `bf-room-tuning2-input-hp-row` | `room_tuning2_input_hp` (`"off"` or a number) |

- `updateRoomRows`: the basis and transition rows are visible exactly when the room mode is `tuning2` and
  `bf-room-tuning2-high` is empty (the automatic limit; the same condition that shows the room volume and
  Schroeder rows in tuning 2). The hint is visible exactly when those rows are visible, the basis is
  `lenient` or the transition is not 1, and both `bf-room-volume` and `bf-schroeder-freq` are empty. The
  input high-pass row is visible exactly when the room mode is `tuning2`.
- Re-run `updateRoomRows` when `bf-room-tuning2-high`, either new select, `bf-room-volume` or
  `bf-schroeder-freq` changes (some of these already call it; add only what is missing).
- Placeholder of `bf-room-tuning2-input-hp`: `t("placeholder_room_tuning2_off")`, set where the two `auto`
  placeholders are set. Add that key (English `off`).
- Defaults (next to `bf-room-tuning2-high`): basis from `defaults.room_tuning2_high_basis` (`"strict"` when
  missing or unknown), transition `String(defaults.room_tuning2_high_transition ?? 1)` (1 when not 1, 2 or
  4), input high-pass empty when `defaults.room_tuning2_input_hp` is `"off"` or missing, else its number as
  text (the way `autoOrNumberText` treats `auto`; write a small helper or generalize it).
- Request (where `args.room_tuning2_mimo_low` is set): in `tuning2` mode always send the three fields, never
  in other modes. `room_tuning2_high_basis` is the select value, `room_tuning2_high_transition` the select
  value as a number, `room_tuning2_input_hp` the number or `"off"` when empty (`numOrNull(...) ?? "off"`).
- Any reset-to-defaults or field list that enumerates the tuning2 controls gets the three ids.
- `ipc.d.ts`: `ProcessingRequest` gains `room_tuning2_high_basis?: "strict" | "lenient";
  room_tuning2_high_transition?: 1 | 2 | 4; room_tuning2_input_hp?: number | "off";` with a comment in the
  style of the 3.14.0 lines; check the defaults type so `npm run typecheck` passes.

## 2. Catalog (`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`)

Screen keys already present in English in every file (the text is the markup's); improve the English if a
sentence is unclear and translate the other eight: `label_room_tuning2_high_basis`,
`option_room_tuning2_high_basis_strict`, `option_room_tuning2_high_basis_lenient`,
`tooltip_room_tuning2_high_basis`, `label_room_tuning2_high_transition`,
`tooltip_room_tuning2_high_transition`, `hint_room_tuning2_high_needs_volume`, `label_room_tuning2_input_hp`,
`tooltip_room_tuning2_input_hp`; and the new `placeholder_room_tuning2_off`. If you change an English text,
change the markup's fallback text to match (that is the only markup edit allowed).

Update `tooltip_room_tuning2_high` in all nine: the automatic limit is now the Schroeder frequency of the
chosen basis times the reach, from 100 Hz up to 500 Hz times the reach, and 500 Hz without a room volume or
Schroeder frequency.

Keys P46 added with English text in every file, to translate in the eight non-English files (find them with
`git diff origin/master -- crates/impulcifer-service/locales/en.json`): `cli_room_tuning2_high`,
`cli_room_tuning2_high_no_volume`, `cli_room_tuning2_input_hp`, `cli_room_tuning2_pair_phase`,
`cli_room_tuning2_pair_phase_hint`. Keep every placeholder exactly as in English, in every language. You may
improve the English of these lines if a sentence is unclear (same placeholders). The pair-phase line is a
note, not a warning: it says that the two speakers partly cancel in those bands at that ear when they play
the same signal. The hint says that the bass arrival alignment brings the speakers' bass phase closer.

After any English change, grep the old English fragments in `crates/impulcifer-service/tests/room_tuning2.rs`
(it asserts README fragments of `cli_room_tuning2_*` lines) and update the asserted fragments so the test
passes.

Korean rules (the maintainer reads these): plain sentences, no translationese, no emoji, no em-dash; numeric
ranges with `~` (`100~500 Hz`), units with a space before them, "dB" and "ms" as is. Say what each option does
to the correction; do not restate the English word order. Other languages: natural, technical register, same
facts.

Gate: `cargo test -p impulcifer-service --lib settings` and `cargo test -p impulcifer-service --test ui_catalog`,
run separately (in one command the `settings` filter also filters out every `ui_catalog` test), and
`cargo test -p impulcifer-service --test room_tuning2`. Do not touch `i18n/locales/` (2.x) or `webview_ui/`.

## 3. Gallery (`apps/impulcifer-app/tests/ui_gallery.py`)

Extend `room_tuning2_checks`: with the room mode `tuning2` and an empty upper limit the basis and transition
rows are visible and the hint hidden; choosing `lenient` with no room volume and no Schroeder frequency shows
the hint; entering a room volume hides it; typing a number into `bf-room-tuning2-high` hides both rows and the
hint; the input high-pass row is visible; the request carries `room_tuning2_high_basis: "strict"`,
`room_tuning2_high_transition: 1`, `room_tuning2_input_hp: "off"` by default and the chosen values after
choosing `lenient`, `×2` and typing 25; with another room mode none of the three keys is sent. Add one capture
pair named in the pattern of the existing tuning2 captures (for example
`processing-tuning2-bundle3-studio-{en,ko}-{dark,light}`) showing the card with `lenient`, `×2`, the hint
visible and 25 Hz in the high-pass field.

## Verification (foreground, before reporting)

```
cd /e/Impulcifer/.worktrees/s2r3/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts
cd /e/Impulcifer/.worktrees/s2r3 && cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-service --test room_tuning2
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out "C:\Users\32170336\AppData\Local\Temp\claude\b7093525e1d0c75f\7bcd40cd-140a-5f35-a38f-646c326a8bcc\scratchpad\ui-gallery-s2r3"
```

(Read the gallery script's usage first; the checks run before the captures, so exit 0 means the checks
passed.)

## Report (then end your turn)

1. Files changed, one line each.
2. The keys you added or translated, per file count, and the final English and Korean text of every new or
   changed key.
3. The commands' final lines; the gallery's PNG count and the names of the new captures.
4. Anything ambiguous in the markup or the spec that you resolved, and how.
