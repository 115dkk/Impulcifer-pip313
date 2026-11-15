# P61: each part of the screen wires, re-renders and adds its own request fields (architecture candidate 3)

Repository: `/home/user/Impulcifer-pip313`, branch `ccr-083fe5e3-isoctv`. Work only inside it. This packet starts
after P60 (room options table) is committed: `room.js` already has `wireRoom()` and `addRoomArgs(args)`.
Read first: `CLAUDE.md` (the 3.x workspace section, especially the bullet on vanilla JS: fourteen ordered classic
scripts, one global namespace, no ES modules, no bundler, top-level code uses only names from earlier files), then
`apps/impulcifer-app/ui/app.js` (`wireEvents`), `core.js` (`applyStrings`), `advanced.js` (`gatherBrirPayload`),
and the files each wiring line calls into.

This is a refactor of the screen code. The markup, the styles, the strings and every request the screen sends stay
exactly as they are.

## Why

After 3.21.1 every part of the screen has its own file, but three central functions still hold every part's
details: `wireEvents` (about 220 lines of listeners for every view), `applyStrings` (the re-renders after a
language change) and `gatherBrirPayload` (every section's request fields). A new option or part changes its own
file and one or more of these three, and two packets in flight meet on the same lines there.

## Design (fixed)

1. **One wiring function per part, in that part's file.** Move each block of `wireEvents` into the file that owns
   the controls, as `wire<Part>()`, keeping the lines as they are (same handlers, same order inside the part):
   - `recorder.js` `wireRecorder()`: devices, host API, play file, output device, channel order, record folder,
     append, sweep source, speakers, layout and "use all", forced channels and channel count, the record and
     sweep-set buttons, the recording cancel button;
   - `jobs.js` `wireDialogs()`: the ask dialog's buttons and its `document` keydown;
   - `guide.js` `wireGuided()`;
   - `advanced.js` `wireAdvancedForms()`: `for (const p of ADV_PREFIXES) wireAdvanced(p)` and Studio's tabs
     (`ADV_TABS`: select, arrow keys, on switch, reset);
   - `test-signal.js` `wireTestSignal()`: the source select, the three inputs that re-check it, the detect button;
   - `room.js` `wireRoom()` (exists after P60);
   - `mfile.js` `wireMfile()`: the single recording rows and status, and the three measurement file blocks;
   - `eq.js` `wireEq()`: the folder input that re-inspects the EQ, and the preview's `ResizeObserver`;
   - `jobs.js` `wireJobs()`: Generate (with its recording and measurement file checks), the BRIR cancel button, the
     job dialog's cancel and close, open output;
   - `progress.js` `wireRecovery()`: start, open output, the three inputs that re-plan;
   - `settings.js` `wireSettings()`: open data and settings folders, skin, theme, language;
   - `update.js` `wireUpdate()`: the check button and the update dialog's buttons;
   - `app.js` keeps `wireShell()`: navigation, disclosures, browse buttons, `data-open-url`.
   If a line's owner is unclear, it goes with the file that defines the function it calls; say so in the report.

2. **`FEATURES` in `app.js`**: an ordered array of `{ name, wire?, render?, addArgs? }`, one entry per part above.
   - `wireEvents()` keeps its name and becomes `for (const feature of FEATURES) feature.wire?.();`.
   - `render` is what that part re-draws after the language or the skin changes, taken from today's
     `applyStrings` (recorder: channel guidance, resolved path, share modes; guided; room: rows and virtual bass
     rows; measurement files: the three blocks; EQ; advanced: both forms; jobs: steps, job state, the alignment
     report of a finished BRIR; recovery: job and inventory; settings: `applySkin(state.skin)` and system info).
     Give a part a named function (`renderRecorder()` and so on) in its own file when it needs more than one call.
   - `applyStrings()` keeps its name (the gallery calls it), moves to `app.js`, and becomes: language attribute,
     `data-i18n` texts, info button labels, `for (const feature of FEATURES) feature.render?.()`, the runtime status
     line.
   - `addArgs(args)` adds that part's fields to the BRIR request, including its own `isOpen(...)` gate: test signal
     (`test_signal`), measurement files (the BRIR source: measurement file or single recording; and the headphone
     block: measurement file or `headphone_compensation_file`), room (`addRoomArgs`), EQ (`eqRequestFields` when
     its disclosure is open), advanced (Studio's four tabs, or Stable's list when its disclosure is open), virtual
     bass (move the block into `room.js` as `addVbassArgs(args)`).
   - `gatherBrirPayload()` keeps its name (the gallery calls it), moves to `app.js`, and becomes: the base fields
     (`dir_path`, `plot`, the three `do_*` flags), then `for (const feature of FEATURES) feature.addArgs?.(args)`.
   - Order the array as the parts are wired today, so that two listeners on the same element and event keep their
     order (for example the test signal check and the EQ inspection on `bf-dir-path`).

3. **Load order.** `FEATURES` is declared in `app.js` (the last file) and only read at run time, after every file
   has loaded; no top-level code in another file may read it. The `index.html` script list does not change.

4. **Do not** add a registry that files push into at load time, change a handler's behaviour, merge two parts,
   or move code that is not wiring, re-rendering or request building.

Types: describe the entry with a JSDoc typedef in `app.js`. `npm run typecheck` must pass without `@ts-ignore`.

## Allowed files

`apps/impulcifer-app/ui/*.js` (not `ipc.d.ts`, not `index.html`, not the styles). No catalog, markup, style or
test change. Do not commit; the maintainer's session reviews and commits.

## Verification (run all, in the foreground, and report the output)

1. `cd /home/user/Impulcifer-pip313/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts`
2. `cd /home/user/Impulcifer-pip313 && cargo test -q -p impulcifer-service --test ui_catalog && cargo test -q -p impulcifer-app --test packaging`
3. `python3 apps/impulcifer-app/tests/ui_gallery.py --out <a scratch folder>`: must print "Gallery wrote 224 PNGs".
   It takes about four minutes.
4. **Same listeners, same screen, same requests as before.** Write a throwaway script outside the repository (do
   not commit it) that loads the page with the gallery's mock (`ui_gallery.mock_script`) from a `git worktree` of
   `HEAD` (the code before your change) and from your working tree, and compares:
   - **listeners:** an init script that wraps `EventTarget.prototype.addEventListener` and records, in call order,
     the target (element id, or a selector path when it has none, or `document`/`window`), the event type and
     `handler.toString()`; for every target and type, the list of handlers must be the same in the same order;
   - **re-render:** for both skins and a few states (each view, a room mode, a measurement file block open, an
     advanced option set), call `changeLanguage("ko")` and then `changeLanguage("en")`, and after each compare a
     snapshot of every element with an id (`hidden`, `disabled`, `value`, `checked`, `placeholder`, `title`,
     `aria-*`, and `textContent` of leaf elements), excluding the log;
   - **requests:** `JSON.stringify` of `gatherBrirPayload()` with sorted keys in the same states.
   Every pair must be equal. Report the number of listeners, states and comparisons. Remove the worktree afterwards.

## Report

Changed files with a one-line summary each, the `FEATURES` entries in order with their hooks, the new length of
`wireEvents`, `applyStrings` and `gatherBrirPayload`, the output of every verification step, and any line whose
owner was unclear. Finish the report before you end your turn.
