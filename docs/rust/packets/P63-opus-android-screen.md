# P63: the Android page (ADR 0020)

Repository: `/data/data/com.termux/files/home/bridge/Impulcifer-pip313`, branch `claude/android-port`. Work only
inside it. Read first: `CLAUDE.md` (the 3.x workspace section, especially the bullets on the IPC, the overlay
catalog and vanilla JS; and "안드로이드 이식 (3.x)"), `docs/adr/0020-android-app.md`, `docs/rust/UI.md`, then the
whole of `apps/impulcifer-app/ui/` (`index.html`, `styles.css`, every script, `ipc.d.ts`).

The Android app is the same Tauri shell opening another page, `apps/impulcifer-app/ui/android.html`
(`apps/impulcifer-app/src/lib.rs`, `PAGE`). You make that page. It is built on Studio but is not Studio squeezed
into a narrow window: the markup, the layout, the navigation and the way a running job is shown are made for a
phone held in one hand and touched with a thumb. Recording and the Stable skin do not exist on Android.

## What the page is for

Someone has a folder of measurements on the phone (recorded elsewhere) and wants BRIR files out of it with room
correction, headphone compensation and the other processing options, or wants the missing output files of an
earlier run restored. Processing and output recovery are the two jobs that matter; settings and information
support them. Every processing option Studio offers must be reachable here, and the request the page sends for
the same choices must be the request Studio sends.

## Architecture (fixed)

1. **One behaviour codebase, two pages.** `android.html` loads the same feature scripts as `index.html`
   (`core.js`, `progress.js`, `jobs.js`, `update.js`, `test-signal.js`, `tips.js`, `room.js`, `advanced.js`,
   `eq.js`, `mfile.js`, `settings.js`, `app.js`), not copies. It does not load `recorder.js` and `guide.js`.
   Fields keep Studio's ids (`bf-…` for the shared fields, `ba-…` for the advanced form, `dis-…`, `recovery-…`,
   `sf-…`, `btn-…`), so the existing wiring, rules tables (`ADV_RULES`, `ROOM_FIELDS`, `ROOM_ROWS`), inspections
   and request building work on the new markup unchanged. The Stable-only parts (`bf-` advanced list, the job
   modal) and the recorder are absent from the page.
2. **A part runs only where its markup is.** All scripts are type-checked as one program of global scripts
   (`tsconfig.json`), so a name is declared once. Do not declare a second `FEATURES`, `boot`, `applyStrings` or
   `gatherBrirPayload` for Android. Instead make the shared code independent of which page it is on:
   - `FEATURES` entries gain what they need so that a part is skipped when its root element is not on the page
     (for example a `present` predicate, or an id checked by the loop), and a `boot` hook for what `boot()` does
     for that part today (sweep layouts, guided targets, devices, the first-run language choice and so on), so
     `boot()` becomes a loop like `wireEvents`. Keep the order of the desktop's calls exactly.
   - Code in a shared function that reaches for an element of another part (`updateStartControls` and the
     recording cancel button, `applySkin` and the skin description, `updateJobModal` and the Stable dialog,
     `renderJobState`…) moves to the part that owns the element, or asks whether the element exists. Prefer moving.
   - Android-only parts are new files in `ui/` named after what they do (for example `chooser.js`, `storage.js`,
     `sheet.js`), loaded by `android.html` only, each with `wire`/`render`/`boot` hooks registered in `FEATURES`
     behind the same presence check. `index.html` may load them too if that is simpler, since they do nothing
     without their markup; say which you chose.
   - Where the two pages do one thing differently, dispatch on the markup, not on the platform name: a browse
     button opens the page's own chooser when the page has one (`#chooser`) and the native dialog otherwise.
3. **The desktop page does not change.** `index.html` and what the desktop renders stay as they are (see
   "Desktop identity" under Verification). `styles.css` keeps its rules; `android.html` links `styles.css` for
   the tokens, fonts and components and then `android.css` for everything a phone needs. If you would rather
   split the tokens and fonts out of `styles.css` into a file both pages link, you may, as long as the desktop
   renders the same.
4. **Page marker and test hooks** (other work depends on these names):
   - `<html data-page="android">` on the new page; the desktop page gets `data-page="desktop"`.
   - After a successful `boot()` both pages set `document.documentElement.dataset.booted = "true"`.
   - `renderJobState` mirrors the job on the root element: `data-job-kind` (`brir`, `output_recovery`, `update`,
     or absent) and `data-job-status` (`running`, `cancel_requested`, `succeeded`, `failed`, `cancelled`, or
     `idle`).
   - Bottom navigation buttons: `.nav-item` with `data-view` and the ids `nav-processing`, `nav-recovery`,
     `nav-settings`, `nav-info`; the views keep `view-processing`, `view-recovery`, `view-settings`, `view-info`.
   - The chooser: `#chooser` (the sheet), `#chooser-path`, `#chooser-roots`, `#chooser-list`, `#chooser-up`,
     `#chooser-select` (choose this folder), `#chooser-close`. The storage notice: `#storage-notice` with
     `#btn-storage-access`. The job sheet: `#job-sheet`, `#job-sheet-close`.
   - `bf-dir-path` and `recovery-dir-path` stay text inputs a person (or a test) can type a path into.
5. **New IPC** (already declared in `ipc.d.ts`; the service side is another packet and may not be finished while
   you work, so develop against a mock):
   - `list_directory({path?, kind?})` → `DirectoryListing`. A folder that cannot be read answers
     `ok: false` with `error.details.kind === "access_denied"`.
   - `get_storage_access()` → `{state: "granted" | "denied" | "not_required"}`;
     `request_storage_access()` → `{opened}` and opens the system screen. Ask the state again whenever the page
     becomes visible (`visibilitychange`), because the person grants it outside the app.
   - `get_system_info()` may carry `device`; `bootstrap().platform` is `"android"`, `install_kind` is `"apk"`,
     `capabilities.recording` is false.
   - Updates on Android: `start_update` opens the release's APK in the browser and the job's result says so
     (`update_apk_*` keys with English defaults in the result, `requires_restart: false`). The update dialog
     must read naturally for that: the action is "download", nothing restarts.
   - The back button: `window.__TAURI__.app.onBackButtonPress(handler)`. While a handler is registered Android's
     own back action (leave the app) does not run, so register it only while the page has something to go back
     from and `unregister()` it otherwise. Expose the page's reaction as a global `handleBack()` that returns
     whether it did anything, so it can be driven without Tauri.

## Design direction (decided; draw within it)

Tokens, fonts and the component vocabulary are Pulse Studio's (`styles.css`): surfaces `--bg-0…4`, hairline
borders, the blue accent, Pretendard for text and JetBrains Mono for paths, numbers and file names, both themes.
Load the `make-interfaces-feel-better:make-interfaces-feel-better` skill before drawing and apply it.

- **Frame.** Portrait phone first (360 to 430 CSS px wide), one column. A top bar with the view's title; the
  scrolling content; on the processing and recovery views an action bar pinned above the navigation holding the
  view's one primary action; a bottom navigation bar with four destinations (processing, output recovery,
  settings, information), each an inline SVG icon (one stroke weight, 24 px) over a label. The top bar and the
  bottom bars use `--bg-0` with a hairline, not a raised surface: the native window paints the strips behind the
  status and navigation bars with that colour (`MainActivity.applyPageTheme`), and they must meet without a seam.
  The page itself never draws under the system bars and needs no safe-area padding. Wider screens (tablets,
  landscape) keep one column with a comfortable maximum width, centred.
- **Touch.** Every control is at least 48 px tall and wide. Text inputs use at least 16 px text. Rows are
  label above control, full width; a switch or checkbox row is label left, control right, the whole row
  tappable. Range sliders get a thumb a thumb can hold. Nothing depends on hover.
- **Processing view**, in this order: the storage notice when access is missing; inputs (the measurement folder
  with a browse button, the BRIR source and test signal controls of Studio's input card, the folder analysis);
  the three processing switches (room correction, headphone compensation, custom EQ) as rows that open in place;
  virtual bass; the advanced options with Studio's four panels behind one horizontally scrollable segmented
  control. Long option groups stay collapsed until asked for; an open group must be easy to close again from
  its head, which stays where the thumb found it.
- **Tables.** Studio's wide tables (the measurement file assignment, EQ slots, the alignment report, the device
  channel tables that remain) become stacked rows on a phone: each row a small block with its label above its
  control. A read-only numeric table may scroll sideways inside its card instead, with the first column fixed.
- **Running a job.** Starting a job opens a full-height sheet (`#job-sheet`) with the stage list, the progress
  bar, the log (collapsed by default) and cancel. It can be put away to look at the form; a slim bar above the
  action bar then shows that the job is running and reopens the sheet. When the job ends the sheet shows the
  outcome: the output folder as a path, the alignment report, the failure sentence. "Open output folder" stays
  (Android may or may not have a file manager that answers; a failure shows the path instead of an error box).
- **Output recovery view**: the folder, the two options, the inventory with its five states and the ledger, the
  result. The inventory behaves as `docs/rust/UI.md` says for Studio.
- **Settings view**: language, theme (no skin choice: Android has one), storage access (state in words, the
  button to the system screen), and the two folders as read-only paths. The data and settings folders are
  inside the app's private storage, so there are no "open" buttons for them on Android.
- **Information view**: the about block with the version and the update check, system information (version,
  install kind, device, OS, CPU count, runtime, shell, web view, update channel; a missing value hides its row),
  contributors, project links.
- **The chooser** (`#chooser`), a full-height sheet used for every browse button: the storage volumes as chips
  when there is more than one, the current path (it can be long: let it scroll sideways, end visible), "up",
  the list (folders first; in folder mode files are shown dimmed only to tell folders apart, in file mode the
  matching files are the choices and a tap chooses one), "choose this folder" in folder mode, and close. States:
  loading, empty folder, folder with more entries than listed, unreadable folder, and no storage access (the
  same words and button as the storage notice, inside the sheet). It opens at the folder already in the field
  when that exists, otherwise at the first volume.
- **Dialogs and tips.** The ask dialog, the update dialog and the first-run language choice are sheets or
  centred dialogs sized for a phone, with the confirming action where the thumb is. An info button opens its
  text as a sheet from the bottom with the field's label as the title, not as a bubble beside the button.
- **Back.** `handleBack()` closes the topmost thing in this order: an open tip, the ask dialog (as its cancel),
  the chooser, the update dialog, the job sheet (puts it away), then returns to the processing view from another
  view. With nothing left it does nothing and the system leaves the app.
- **Words.** Reuse existing catalog keys wherever the meaning is the same. For what is new (navigation labels
  if the sidebar's do not fit, the chooser, storage access, the Android update wording, the job sheet) add keys
  to all nine overlay files `crates/impulcifer-service/locales/<language>.json` with the same key set and
  placeholders: write the English text in `en.json`, a Korean draft in `ko.json`, and the English text in the
  other seven. A later packet rewrites the Korean and translates the rest, so keep the key list in your report
  with one line on where each appears. Product words follow the existing catalog (해요체 is not used; look at
  `ko.json`). Do not touch `i18n/locales/` (2.x) or `webview_ui/`.

## Allowed files

`apps/impulcifer-app/ui/**` (new `android.html`, `android.css`, new scripts; the shared scripts and, only for
the `data-page` attribute and script list, `index.html`), `apps/impulcifer-app/tsconfig.json` if a new file
needs it, `crates/impulcifer-service/locales/*.json` (new keys only), and the tests that pin the script list or
the catalog if they need the new files named (`crates/impulcifer-policy/tests/gates.rs` only where it lists UI
scripts, `apps/impulcifer-app/tests/packaging.rs` likewise). Not in this packet: `ui_gallery.py` (the gallery's
Android scenarios are the next packet; write your own throwaway harness outside the repository), Rust sources,
`src/bridge.js`, workflows, documents. Do not commit, stash, reset or check out anything: another worker is
editing `crates/` in the same working tree right now.

## Environment (an Android phone, not a PC)

- Node runs in Termux: `cd apps/impulcifer-app && node node_modules/typescript/bin/tsc -p tsconfig.json` and
  `node tests/bridge-contract.cjs src/bridge.js && node tests/observer-contract.cjs src/smoke-observer.js && node tests/driver-contract.cjs ../../tests/app_smoke/driver.js`.
  `npm run` and `npx` do not find executables here.
- Python with Playwright and Chromium runs inside the proot container, from the repository root:
  `~/.local/bin/ubuntu-run /opt/pw-venv/bin/python <script>`. Use it to look at your page: import
  `apps/impulcifer-app/tests/ui_gallery.py` for its mock (`mock_script`, `catalog`), add mocks for the three new
  methods and an Android bootstrap, open `android.html` over `file://` at 412×915 with `device_scale_factor=2`
  (and once at 360×740 and at 800×1280), screenshot every view and state in both themes, English and Korean, and
  read the PNGs yourself. Keep the harness and its output outside the repository (for example under
  `/data/data/com.termux/files/usr/tmp/claude-10518/-data-data-com-termux-files-home-bridge/a788c326-b089-5f70-9d5f-6a4d1556e4b4/scratchpad/imp/p63/`).
- Cargo runs through `~/.local/bin/ubuntu-run cargo …` from the repository root, in the foreground, one command
  at a time. Builds take minutes. The other worker's unfinished edits may break the build of
  `impulcifer-service`; if a cargo step fails in a file you did not touch, report it and do not fix it.

## Verification (run all, in the foreground, and report the output)

1. Type check and contract tests (the two Node commands above). No `@ts-ignore`, no `any`, no loosened options.
2. **Desktop identity.** The gallery of the code before this packet was rendered on this machine into
   `/data/data/com.termux/files/usr/tmp/claude-10518/-data-data-com-termux-files-home-bridge/a788c326-b089-5f70-9d5f-6a4d1556e4b4/scratchpad/imp/gallery-base/`
   (224 PNGs). After your change run
   `~/.local/bin/ubuntu-run /opt/pw-venv/bin/python apps/impulcifer-app/tests/ui_gallery.py --out <a scratch folder>`
   (it also runs the gallery's behaviour checks and takes a long time here; run it once near the end and again
   only after changes to shared files). It must print "Gallery wrote 224 PNGs", and every PNG must be
   byte-identical to the one of the same name in `gallery-base` (compare SHA-256). If the base folder holds fewer
   than 224 files when you get there, its run failed: say so and compare what is there. A difference is a change
   to the desktop page: find and remove its cause, do not accept it.
3. **Same request.** With your harness, set the same choices on `index.html` (Studio skin) and `android.html`
   for at least these states and compare `JSON.stringify(gatherBrirPayload())` with sorted keys: defaults; room
   correction open in each of the three room modes with a few options changed; a room measurement file assigned;
   headphone compensation from a measurement file; custom EQ with one slot off and one file; virtual bass manual;
   each of the four advanced panels on with every field changed; BRIR from a measurement file; BRIR from one
   recording file. Every pair must be equal. Also list every element id Studio's request building reads and
   confirm each exists on `android.html` (a script, not by eye).
4. **Android behaviour**, with the harness and mocks: the storage notice appears for `denied`, disappears after a
   `visibilitychange` once the mock says `granted`; the chooser lists, enters a folder, goes up, stops at a root,
   chooses a folder into the field that opened it and fires the field's `input` event, chooses a file in file
   mode, and shows each of its states; `handleBack()` closes things in the stated order and returns false with
   nothing open; the back handler is registered only while there is something to go back from (mock
   `window.__TAURI__.app.onBackButtonPress` and count); a BRIR job opens the sheet, shows stages, can be put
   away and reopened, ends in the outcome; a failed job shows the sentence; recovery walks its five states and a
   run; language and theme changes re-render everything, in all nine languages without a raw key on screen;
   the update dialog for `install_kind: "apk"`; no page, console or resource error in any of it; no horizontal
   page scroll at 360 px; no control smaller than 48 px (measure the boxes of every button, input, select, switch
   and nav item that is visible in each view and list any that is smaller).
5. Catalog and policy: `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --lib settings`,
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --test ui_catalog`,
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-policy`,
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-app --test packaging`.

## Report

Changed and new files with a one-line summary each; the script list of each page in order and the `FEATURES`
table with its hooks; every change to a shared script and why the desktop is unaffected; the new catalog keys
with where each appears; the output of every verification step (the identity comparison as a count of equal and
different files, the request comparison as a list of states, the list of undersized controls or "none"); where
the screenshots are, with the ten that show the page best named first; and anything in this packet you decided
against, with the reason. Finish the report before you end your turn.
