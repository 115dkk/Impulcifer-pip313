# P45: catalog and wiring for the virtual room tuning 2 options of 3.14.0 (ADR 0015 bundle 2)

Repository worktree: `E:\Impulcifer\.worktrees\s2r2` (Git Bash: `/e/Impulcifer/.worktrees/s2r2`), branch
`claude/tuning2-s2r-2`. Work only inside it. P44 (backend) is applied on this branch: the settings
`room_tuning2_lf_align`, `room_tuning2_mode_decay`, `room_tuning2_hf_phase` exist in `ProcessingConfig`, the
CLI and the service, and six `cli_room_tuning2_*` log keys were added to the nine catalogs with English text.
The screen markup is drawn (three check rows and one hint in the tuning 2 card). You connect the two and write
the words.

Read first:

- `E:\Impulcifer\.worktrees\s2r2\CLAUDE.md`, "3.x Rust 워크스페이스": the overlay catalogs (nine files, same key
  sets and placeholders, `cargo test -p impulcifer-service --lib settings --test ui_catalog`), `// @ts-check`
  and `apps/impulcifer-app/ui/ipc.d.ts`, `npm run typecheck && npm run test:contracts`.
- `E:\Impulcifer\.worktrees\s2r2\docs\rust\ROOM_CORRECTION.md`, "Virtual room tuning v2": "Options" (the
  three 3.14.0 rows and the paragraph after the `room_tuning2_bands` paragraph), "Arrival alignment",
  "High-frequency phase", "Mode-decay cuts" (what the options do) and "Diagnostics" (what the log keys say).
- `E:\Impulcifer\.worktrees\s2r2\docs\rust\packets\P43-astra-tuning2-s2r-1-catalog-and-wiring.md`, the
  bundle-1 packet you followed: the same conventions apply.

## 1. Wiring (`apps/impulcifer-app/ui/app.js`, `apps/impulcifer-app/ui/ipc.d.ts`)

Markup ids (do not change the markup or styles; only the calls and state):

| Control | id | Row id | Request field |
| --- | --- | --- | --- |
| checkbox | `bf-room-tuning2-mode-decay` | `bf-room-tuning2-mode-decay-row` | `room_tuning2_mode_decay` (boolean) |
| checkbox | `bf-room-tuning2-lf-align` | `bf-room-tuning2-lf-align-row` | `room_tuning2_lf_align` (boolean) |
| checkbox | `bf-room-tuning2-hf-phase` | `bf-room-tuning2-hf-phase-row` | `room_tuning2_hf_phase` (boolean) |
| hint | `bf-room-tuning2-fir-hint` | | none |

- `updateRoomRows`: the three rows are visible exactly when the room mode is `tuning2`. The hint is visible
  exactly when the room mode is `tuning2` and `tuning2Filter()` is `"zero"`.
- In `zero` the two FIR-only checkboxes (`lf-align`, `hf-phase`) are `disabled` (their checked state is kept,
  so switching back to another filter restores it); in every other filter they are enabled. Re-evaluate this
  wherever the filter select (`bf-room-tuning2-mode`) changes and on defaults, the same way the latency row
  follows the filter.
- Defaults (where `bf-room-tuning2-dip-limit` takes `defaults.room_tuning2_dip_limit === true`): the three
  checkboxes from `defaults.room_tuning2_mode_decay === true`, `defaults.room_tuning2_lf_align === true`,
  `defaults.room_tuning2_hf_phase === true`.
- Request (where `args.room_tuning2_dip_limit` is set): always send the three fields in `tuning2` mode, never
  in other modes. `room_tuning2_mode_decay` is the checkbox state. `room_tuning2_lf_align` and
  `room_tuning2_hf_phase` are the checkbox state, but `false` while the filter is `zero` (they do nothing there,
  and the service would log that they were ignored).
- Any reset-to-defaults or field list that enumerates the tuning2 controls gets the three ids.
- `ipc.d.ts`: `ProcessingRequest` gains `room_tuning2_lf_align?: boolean; room_tuning2_mode_decay?: boolean;
  room_tuning2_hf_phase?: boolean;` with a comment in the style of the 3.13.0 lines; check the defaults type
  so `npm run typecheck` passes.

## 2. Catalog (`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`)

New screen keys (the English text is in the markup; copy it into `en.json` and translate the other eight):

- `checkbox_room_tuning2_mode_decay`, `tooltip_room_tuning2_mode_decay`
- `checkbox_room_tuning2_lf_align`, `tooltip_room_tuning2_lf_align`
- `checkbox_room_tuning2_hf_phase`, `tooltip_room_tuning2_hf_phase`
- `hint_room_tuning2_fir_only`

Keys P44 added with English text in every file, to translate in the eight non-English files (find them with
`git diff origin/master -- crates/impulcifer-service/locales/en.json`): `cli_room_tuning2_lf_align`,
`cli_room_tuning2_mode_decay`, `cli_room_tuning2_mode_decay_none`, `cli_room_tuning2_hf_phase`,
`cli_room_tuning2_hf_phase_none`, `cli_room_tuning2_zero_inert`. Keep every placeholder (`{speaker}`,
`{f_lin}`, `{strength}`, `{arrival}`, `{limited}`, `{target}`, `{count}`, `{cuts}`, `{rotation}`, `{low}`,
`{high}`, `{options}`) exactly as in English, in every language. `{limited}` renders as `true`/`false`; word
the sentence so that it reads as "the pull/unwind was limited: true", the opposite sense of the 3.13.0
low-protection key, where `{limited}` means the limit was satisfied. You may improve the English of these six
log lines if a sentence is unclear (same placeholders); `cli_room_tuning2_zero_inert` should name the mode as
the screen does ("PEQ only (zero)") rather than "Tuning2 zero mode".

Korean rules (the maintainer reads these): plain sentences, no translationese, no emoji, no em-dash; numeric
ranges with `~` (`200~500 Hz`), units with a space before them, "dB" and "ms" as is. Say what each option does
to the correction; do not restate the English word order. Other languages: natural, technical register, same
facts.

Gate: `cargo test -p impulcifer-service --lib settings --test ui_catalog`. Do not touch `i18n/locales/` (2.x)
or `webview_ui/`.

## 3. Gallery (`apps/impulcifer-app/tests/ui_gallery.py`)

Extend `room_tuning2_checks`: with the room mode `tuning2` the three rows are visible and the hint is hidden;
the request carries `room_tuning2_lf_align: false`, `room_tuning2_mode_decay: false`,
`room_tuning2_hf_phase: false` by default and `true` for each after ticking it; switching the filter to `zero`
disables `lf-align` and `hf-phase`, shows the hint, keeps `mode-decay` enabled, and the request then carries
`false` for the two FIR-only fields and `true` for `room_tuning2_mode_decay`; switching back to `mixed`
re-enables them with their ticks kept and the request carries `true` again; with another room mode none of the
three keys is sent. Add one capture pair named in the pattern of the existing tuning2 captures (for example
`processing-tuning2-bundle2-studio-{en,ko}-{dark,light}`) showing the card with the three controls visible in
`mixed`, and one pair in `zero` showing the disabled boxes and the hint.

## Verification (foreground, before reporting)

```
cd /e/Impulcifer/.worktrees/s2r2/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts
cd /e/Impulcifer/.worktrees/s2r2 && cargo test -p impulcifer-service --lib settings --test ui_catalog
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out "C:\Users\32170336\AppData\Local\Temp\claude\b7093525e1d0c75f\7bcd40cd-140a-5f35-a38f-646c326a8bcc\scratchpad\ui-gallery-s2r2"
```

(Read the gallery script's usage first; the checks run before the captures, so exit 0 means the checks
passed.)

## Report (then end your turn)

1. Files changed, one line each.
2. The keys you added or translated, per file count, and the final English and Korean text of every new key.
3. The three commands' final lines; the gallery's PNG count and the names of the new captures.
4. Anything ambiguous in the markup or the spec that you resolved, and how.
