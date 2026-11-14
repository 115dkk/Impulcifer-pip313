# P60: room correction options as one table (architecture candidate 4)

Repository: `/home/user/Impulcifer-pip313`, branch `ccr-083fe5e3-isoctv`. Work only inside it. Read first:
`CLAUDE.md` (the 3.x workspace section, especially the bullet on vanilla JS: fourteen ordered classic scripts, one
global namespace, no ES modules, no bundler), then `apps/impulcifer-app/ui/room.js` in full,
`apps/impulcifer-app/ui/advanced.js` (`gatherBrirPayload`, and `ADV_RULES` as the model of a rules table that already
works this way) and the `wireEvents` lines for room and virtual bass in `apps/impulcifer-app/ui/app.js`.

This is a refactor of the screen code. The markup, the styles, the strings and every request the screen sends stay
exactly as they are.

## Why

Every tuning 2 packet (P43, P45, P47, P49) changed the same three functions in step: `updateRoomRows` (which row
shows), `applyRoomDefaults` (the start value) and `addRoomTuning2Args` (what is sent), and often `wireEvents` (which
inputs re-apply the rows). One room option's facts are spread over four places, and a row that shows while its value
is not sent (or the reverse) is a bug nobody sees. For every room option today, the row shows exactly when its value
is sent. Make that the rule of one table.

## Design (fixed)

In `room.js`:

1. **`roomContext()`** returns the facts every rule reads, computed once:
   `{ mode, range, limit, filter, mimo, roomFacts }` with the meanings `updateRoomRows` gives them today
   (`roomFacts` is the existing expression: tuning at the Schroeder limit, tuning 2 with an empty upper end, Room EQ
   unless legacy).

2. **`ROOM_FIELDS`**: one entry per room setting that is sent. Each entry has
   - `id`: the input's id; its row is `${id}-row` (true for every current row; check each);
   - `arg`: the `ProcessingRequest` field, which is also the `brir_defaults` key;
   - `modes`: the room modes that show and send it;
   - `when` (optional): a further condition on the context. Shown and sent only when the mode matches and `when` holds;
   - a kind that decides how the value is read and how the default is applied:
     - number (`numOr(id, brirDefault(arg, fallback))`, default `String(brirDefault(arg, fallback))`),
     - number or a word (`numOrNull(id) ?? "auto"` or `?? "off"`, default `numberSpecText(defaults[arg])`),
     - optional number (`numOrNull(id)`, no default: the room volume and the Schroeder frequency start empty),
     - check (`checked(id)`, default the boolean from `brir_defaults` when it is a boolean, else the entry's own
       default: `true` for level match, `false` for the others; this is what `=== true` and `!== false` do now),
     - text (`val(id)`, default the string from `brir_defaults` or `""`),
     - choice (a fixed list, a fallback; read and default both fall back when the value is not in the list;
       `room_tuning2_high_transition` sends a number),
     - raw select (`val(id)`, no default: `fr_combination_method` keeps the markup's selection, as today),
     - custom (`read(context)` and `reset(defaults)` functions, for the settings with two controls or a spec:
       `room_tuning_phase_limit` (slider and Stable's checkbox), `room_tuning_delay` (number and its auto box),
       `room_tuning2_mode` (filter select and latency));
   - `disabled` (optional): when the input is disabled (`room_tuning2_high_basis` and `_high_transition` without
     room facts in tuning 2, `room_tuning2_lf_align` and `_hf_phase` for the zero filter, `room_tuning_delay` while
     its auto box is ticked);
   - `refresh` (optional): its input re-applies the rows when it changes (the inputs `wireEvents` wires to
     `updateRoomRows` today).
   `room_tuning2_lf_align` and `_hf_phase` keep sending `false` for the zero filter (`filter !== "zero" && checked`).
   `room_volume` and `schroeder_freq` are one entry each for all three modes with `when: c => c.roomFacts`.
   `room_tuning_level_match` is one entry for tuning and tuning 2.

3. **`ROOM_ROWS`**: the rows that are not a sent setting, with `modes` and `when`: tuning 2's group headings and the
   wrappers that pair or indent rows, Stable's Schroeder checkbox row, the latency row, the MIMO detail wrapper, the
   upper-end volume hint, the FIR hint. Together `ROOM_FIELDS` and `ROOM_ROWS` cover every id `updateRoomRows` shows
   or hides today, no more.

4. **`updateRoomRows()`** keeps its name (the gallery and `applyStrings` call it) and becomes: context, then hide or
   show every table row, set `disabled` from the table, then the parts that are not rows, unchanged
   (`placeRoomFacts`, the named phase limit line, the readout, the tips, the placeholders).

5. **`applyRoomDefaults()`** keeps its name (the gallery calls it) and becomes a loop over `ROOM_FIELDS`, then
   `setRoomMode(...)` last, as now.

6. **`addRoomArgs(args)`** replaces `addRoomEqArgs`, `addRoomTuningArgs` and `addRoomTuning2Args` and holds the
   whole `if (isOpen("dis-room"))` block of `gatherBrirPayload` (target, microphone calibration, the room measurement
   file, `room_mode`, then every table field that is shown). `gatherBrirPayload` calls it.

7. **`wireRoom()`** holds the room and virtual bass listeners that are in `wireEvents` today (range, mode buttons
   with their arrow keys, phase slider and checkbox, delay auto box, the tuning 2 inputs, MIMO, virtual bass mode),
   with the `refresh` inputs wired from the table. `wireEvents` calls `wireRoom()` where those lines were. Use
   `change` for selects and checkboxes and `input` for text and number inputs, as now.

Types: describe the entries with JSDoc typedefs in `room.js`. Writing `args[field.arg]` needs a cast to
`Record<string, unknown>`; keep it in one place. `npm run typecheck` must pass without `@ts-ignore`.

## Allowed files

- `apps/impulcifer-app/ui/room.js`, `apps/impulcifer-app/ui/advanced.js` (only `gatherBrirPayload`'s room block),
  `apps/impulcifer-app/ui/app.js` (only the room and virtual bass lines of `wireEvents`).
- `apps/impulcifer-app/tests/ui_gallery.py`: add one check function, `room_table_checks(browser)`, called from
  `render_gallery` next to `room_tuning2_checks`. Do not change existing checks. It checks that
  - every `ROOM_FIELDS` input and row and every `ROOM_ROWS` id exists, and every `[id$="-row"]` inside the room
    correction card is in one of the two tables or in a short explicit list of rows that always show (target,
    microphone calibration, measurement file);
  - for each mode and the states that change rows (range legacy and not; limit off, Schroeder, a Hz stop; filter
    zero, mixed, custom; MIMO on and off; upper end empty and typed), every table field's arg is in
    `gatherBrirPayload()` exactly when its row is shown.
- No other file. No markup, style or catalog change. Do not commit; the maintainer's session reviews and commits.

## Verification (run all, in the foreground, and report the output)

1. `cd /home/user/Impulcifer-pip313/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts`
2. `cd /home/user/Impulcifer-pip313 && cargo test -q -p impulcifer-service --test ui_catalog && cargo test -q -p impulcifer-app --test packaging`
3. `python3 apps/impulcifer-app/tests/ui_gallery.py --out <a scratch folder>`: must print "Gallery wrote 224 PNGs".
   It takes about four minutes.
4. **Same requests and same screen as before.** Write a throwaway script outside the repository (do not commit it)
   that opens the page from a `git worktree` of `HEAD` (the code before your change) and from your working tree with
   the gallery's mock (`ui_gallery.mock_script`), drives both through the same matrix of room states (every mode,
   the states listed above, each `brir_defaults` value the gallery's `room_*_checks` set, and both skins), and
   compares
   - `JSON.stringify` of `gatherBrirPayload()` with sorted keys, and
   - a DOM snapshot of the room correction card: for every element with an id, `hidden`, `disabled`, `value`,
     `checked`, `placeholder` and `textContent` of tips.
   Every pair must be equal. Report how many states were compared. Remove the worktree afterwards.

## Report

Changed files with a one-line summary each, the table sizes (`ROOM_FIELDS`, `ROOM_ROWS`), how the lines of
`room.js` changed, the output of every verification step, and anything in the code that did not fit the table
(say what and why, rather than forcing it). Finish the report before you end your turn.
