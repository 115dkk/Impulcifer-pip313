# P49: catalog and wiring for the virtual room tuning 2 MIMO options of 3.16.0 (ADR 0015 bundle 4)

Repository worktree: `E:\Impulcifer\.worktrees\s2r3` (Git Bash: `/e/Impulcifer/.worktrees/s2r3`), branch
`claude/tuning2-s2r-4`. Work only inside it. P48 (backend) is applied on this branch: the settings
`room_tuning2_mimo_early` and `room_tuning2_mimo_ear_guard` exist in `ProcessingConfig`, the CLI and the
service, and three `cli_room_tuning2_mimo_*` log keys were added to the nine catalogs with English text. The
screen markup is drawn (a number field and a checkbox under the MIMO rows of the tuning 2 card), and its four
screen keys are already in all nine catalogs in English. You connect the two and write the words.

Read first:

- `E:\Impulcifer\.worktrees\s2r3\CLAUDE.md`, "3.x Rust 워크스페이스": the overlay catalogs (nine files, same key
  sets and placeholders), `// @ts-check` and `apps/impulcifer-app/ui/ipc.d.ts`,
  `npm run typecheck && npm run test:contracts`.
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\ROOM_CORRECTION.md`, "Virtual room tuning v2": "Options" (the
  two 3.16.0 rows and the paragraph after the 3.15.0 paragraph), "MIMO supports" ("Least squares", "Guard",
  "Ear guard": what they do) and "Diagnostics" (what the log keys say).
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\packets\P47-astra-tuning2-s2r-3-catalog-and-wiring.md`, the
  bundle-3 packet: the same conventions apply.

## 1. Wiring (`apps/impulcifer-app/ui/app.js`, `apps/impulcifer-app/ui/ipc.d.ts`)

Markup ids (do not change the markup or styles; only the calls and state):

| Control | id | Row id | Request field |
| --- | --- | --- | --- |
| number | `bf-room-tuning2-mimo-early` | `bf-room-tuning2-mimo-early-row` | `room_tuning2_mimo_early` (number, ms) |
| checkbox | `bf-room-tuning2-mimo-ear-guard` | `bf-room-tuning2-mimo-ear-guard-row` | `room_tuning2_mimo_ear_guard` (boolean) |

- `updateRoomRows`: both rows are visible exactly when `bf-room-tuning2-fsiso-row` is (room mode `tuning2`
  and `bf-room-tuning2-mimo` checked). `bf-room-tuning2-mimo` already re-runs it.
- Defaults (next to `bf-room-tuning2-fsiso`): the number from `brirDefault("room_tuning2_mimo_early", 60)`,
  the checkbox `defaults.room_tuning2_mimo_ear_guard === true`.
- Request: inside the existing `if (mimo)` block where `room_tuning2_fsiso` and `room_tuning2_mimo_low` are
  set, `args.room_tuning2_mimo_early = numOr("bf-room-tuning2-mimo-early", brirDefault("room_tuning2_mimo_early", 60))`
  and `args.room_tuning2_mimo_ear_guard = checked("bf-room-tuning2-mimo-ear-guard")`. Never sent with MIMO off
  or in other room modes, like the two neighbours.
- Any reset-to-defaults or field list that enumerates the tuning2 MIMO controls gets the two ids.
- `ipc.d.ts`: the request type gains `room_tuning2_mimo_early?: number; room_tuning2_mimo_ear_guard?: boolean;`
  next to `room_tuning2_fsiso`, and the comment above it says they are read only with `room_tuning2_mimo`
  (5–60 ms); check the defaults type so `npm run typecheck` passes.

## 2. Catalog (`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`)

Screen keys already present in English in every file (the text is the markup's); improve the English if a
sentence is unclear and translate the other eight: `label_room_tuning2_mimo_early`,
`tooltip_room_tuning2_mimo_early`, `checkbox_room_tuning2_mimo_ear_guard`,
`tooltip_room_tuning2_mimo_ear_guard`. If you change an English text, change the markup's fallback text to
match (that is the only markup edit allowed).

Update `tooltip_room_tuning2_mimo` in the eight non-English files: the English now says the 60 ms can be
changed below (`(adjustable below)`); the rest of its meaning is unchanged.

Keys P48 added with English text in every file, to translate in the eight non-English files (find them with
`git diff origin/master -- crates/impulcifer-service/locales/en.json`): `cli_room_tuning2_mimo_early`,
`cli_room_tuning2_mimo_ear_band`, `cli_room_tuning2_mimo_ear_tail`. Keep every placeholder exactly as in
English, in every language. You may improve the English of these lines if a sentence is unclear (same
placeholders). The two ear lines explain why an input's MIMO copies were turned down or off: at that ear, at
that scale, the copies raised the late energy of that band (or lengthened that mode's tail) by that many dB.
`{ear}` arrives as the word `left` or `right`, the way `cli_room_tuning2_pair_phase` receives it; write the
sentence so it reads with that word, as the pair-phase line does in each language.

After any English change, grep the old English fragments in `crates/impulcifer-service/tests/room_tuning2.rs`
(it asserts README fragments of `cli_room_tuning2_*` lines) and update the asserted fragments so the test
passes.

Korean rules (the maintainer reads these): plain sentences, no translationese, no emoji, no em-dash; numeric
ranges with `~` (`5~60 ms`), units with a space before them, "dB" and "ms" as is. Say what each option does to
the correction; do not restate the English word order. The existing Korean MIMO strings call the copies
'서포트' or describe them as other speakers cancelling the late bass; stay consistent with
`checkbox_room_tuning2_mimo`, `tooltip_room_tuning2_mimo` and `cli_room_tuning2_mimo` as they read in
`ko.json`. Other languages: natural, technical register, same facts.

Gate: `cargo test -p impulcifer-service --lib settings` and `cargo test -p impulcifer-service --test ui_catalog`,
run separately (in one command the `settings` filter also filters out every `ui_catalog` test), and
`cargo test -p impulcifer-service --test room_tuning2`. Do not touch `i18n/locales/` (2.x) or `webview_ui/`.

## 3. Gallery (`apps/impulcifer-app/tests/ui_gallery.py`)

Extend `room_tuning2_checks`: with the room mode `tuning2` and MIMO off both new rows are hidden; checking MIMO
shows both, with 60 in the number field and the checkbox clear; the request carries
`room_tuning2_mimo_early: 60` and `room_tuning2_mimo_ear_guard: false` by default and the chosen values after
typing 20 and checking the box; with MIMO off neither key is sent. Keep the existing asserts about the MIMO
request (`room_tuning2_fsiso`, `room_tuning2_mimo_low`) passing; the key sets they compare may need the two new
keys. Add one capture pair named in the pattern of the existing tuning2 captures (for example
`processing-tuning2-bundle4-studio-{en,ko}-{dark,light}`) showing the card with MIMO on, 20 ms and the ear
guard checked.

## Verification (foreground, before reporting)

```
cd /e/Impulcifer/.worktrees/s2r3/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts
cd /e/Impulcifer/.worktrees/s2r3 && cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-service --test room_tuning2
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out "C:\Users\32170336\AppData\Local\Temp\claude\b7093525e1d0c75f\7bcd40cd-140a-5f35-a38f-646c326a8bcc\scratchpad\ui-gallery-s2r4"
```

(Read the gallery script's usage first; the checks run before the captures, so exit 0 means the checks
passed.)

## Report (then end your turn)

1. Files changed, one line each.
2. The keys you added or translated, per file count, and the final English and Korean text of every new or
   changed key.
3. The commands' final lines; the gallery's PNG count and the names of the new captures.
4. Anything ambiguous in the markup or the spec that you resolved, and how.
