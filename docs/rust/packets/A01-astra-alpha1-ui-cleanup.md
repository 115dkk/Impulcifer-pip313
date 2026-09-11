# A01 (ASTRA): the 3.x app UI leaves alpha.0 (typed JS, Rust-edition settings and info, device access mode, Studio recovery redesign)

Work in `E:/Impulcifer` on the checked-out branch `claude/rust-alpha1-cleanup` (do not switch branches, do not commit, do not stash). Read `E:/Impulcifer/CLAUDE.md` (section "3.x Rust 워크스페이스"), `E:/Impulcifer/docs/adr/0003-3x-frontend-fork-and-overlay.md` and `E:/Impulcifer/docs/rust/ARCHITECTURE.md` first. Run every command in the foreground and never in the background; do not query or terminate processes; never run any `Update.exe`; never touch `%LOCALAPPDATA%/Impulcifer`.

The 3.x frontend now lives in `E:/Impulcifer/apps/impulcifer-app/ui/` (a copy of the 2.x `webview_ui/`; ADR 0003). `webview_ui/` and `i18n/locales/` belong to 2.x: never edit them. 3.x strings go into the overlay catalogue `crates/impulcifer-service/locales/<lang>.json` (nine files, one shared key set; a key there wins over the 2.x catalogue).

**A second worker (Daybreak, packet `docs/rust/packets/DB01-daybreak-share-mode-recovery-plan-sysinfo.md`) implements the backend of this work in the same tree at the same time.** Read that packet: it fixes the JSON shapes you code against (`bootstrap.capabilities.share_modes`, `start_recording.share_mode` and its errors and result, `plan_output_recovery`, the new `get_system_info`). Do not edit any Rust file or `features.toml`. Code against the shapes in DB01; do not wait for the backend and do not run the app end to end in this packet (the caller runs the integration afterwards). If the backend has landed by the time you finish (check for `plan_output_recovery` in `crates/impulcifer-service/src/lib.rs`), say so in the report.

## Design direction (fixed; design within it)

Pulse Studio stays: tokens in `ui/styles.css` (`--bg-*`, `--fg-*`, `--accent*`, `--ok`, `--warn`, `--err`, `--line*`, `--field-*`), the card grammar (`.card`, `.card-head`, `.card-num`, `.card-title`, `.card-meta`, `.card-body`), `.field-row`, `.hint`, `.chip`, `.steps`, `.kv-grid`, the two skins (`[data-skin="studio"]` inline activity, `[data-skin="stable"]` tabbed layout with the job modal), two themes. No new fonts, no framework, no bundler, no TypeScript source files (only `.d.ts` declarations). Every visible string is an i18n key resolved through `t()` and present in all nine overlay files. Element ids that other code uses stay (listed per task).

## Task 1. Type checking to zero errors (`npm run typecheck`)

`apps/impulcifer-app/tsconfig.json` (`checkJs`, `strict`) already covers `ui/*.js`, `ui/*.d.ts`, `src/*.js`, `src/*.d.ts` and `tests/app_smoke/driver.js`; today `tsc` reports about 450 errors. Bring it to zero without changing behaviour:

- Write `ui/ipc.d.ts` declaring the whole IPC surface: `type Envelope<T> = {ok: true, data: T} | {ok: false, error: IpcError}`, `IpcError {code, message, details, retryable}`, the data type of every method of `window.pywebview.api` (all 24 methods, including `plan_output_recovery` and the DB01 shapes for bootstrap, `get_system_info`, recording requests/results/events, recovery plan/result, jobs and events, update check/apply), and the `Window` augmentation for `pywebview`, `__TAURI__` (`core.invoke`), `__impulciferSmoke`, `__impulciferSmokeParams`. Use JSDoc `@typedef`/`@param`/`@returns`/`@type` in the `.js` files; a small typed lookup helper (for example `el(id, HTMLInputElement)`) is fine. Do not loosen `tsconfig.json` and do not add `// @ts-ignore` or `any` casts to silence errors; model the real shapes.
- `src/bridge.js` and `src/smoke-observer.js` are initialization scripts; declare what they touch in `src/globals.d.ts` (or the `ui/ipc.d.ts` `Window` augmentation) rather than weakening them.
- `npm run test:contracts` (the three node contract tests in `apps/impulcifer-app/tests/*.cjs`) must keep passing; adjust the `.cjs` files only if a typing-driven refactor of the scripts requires it.

## Task 2. Settings and Info for the Rust edition

`view-settings` and `view-info` in `ui/index.html`, the matching code in `ui/app.js`.

- Remove the "Default interface" row (`#sf-frontend`) and its `set_frontend` wiring: there is no CustomTkinter in 3.x. Keep `#sf-skin` (with its description line), `#sf-theme`, `#sf-language` and their ids (the smoke driver uses `#sf-skin` and `#sf-language`).
- Settings gets two cards: `UI Appearance` (skin, theme, language) and `Folders` (the existing `#btn-open-data`, plus the data folder path and the settings file path shown in mono from `get_system_info().paths`, each with an open button; the settings file button opens its parent folder through `open_path`).
- Info: the version pill becomes `VERSION 3.x · RUST rustc 1.xx · <install kind>`; the System card lists Runtime (toolchain), Shell (`runtime.shell`), Web engine (`runtime.webview`), Audio backend, OS, CPU cores, Update channel (localized `prerelease`/`stable`), Data folder, Settings file. Rows whose value is absent are not rendered. The GIL and "optimal workers" rows are gone with their keys. The contributors and links keep their ids; reword the 2.x-only texts through overlay keys (`contributor_role_py313` becomes a sentence about this fork: 2.x Python port, 3.x Rust rewrite; `button_fork_repo` becomes "This fork").
- The sidebar footer and the pre-boot strings no longer mention Python: override `webview_bridge_connecting`, `webview_bridge_connected`, `webview_bridge_failed` in the overlay (service, not bridge) and update `PREBOOT_STRINGS` in `app.js` to the same English and Korean texts.

## Task 3. Device access mode on the Recorder

- In the `Audio Devices` card add a `field-row` `Device access:` with `<select id="rf-share-mode">` offering `auto` (`Auto: exclusive, shared if refused`), `exclusive` (`Exclusive`), `shared` (`Shared`). Populate the options from `bootstrap.capabilities.share_modes`; when only `auto` is offered, render the select disabled with a hint that a fixed mode is available with WASAPI on Windows only.
- The `Host API` row (`#rf-host-api`) stays in the markup and the request but is hidden (`hidden` attribute on the row) whenever `list_audio_devices` returns one host API or none.
- `gatherRecordingPayload` sends `share_mode: <value>` for both speaker and headphone recordings. The confirmation texts (`message_recording_setup_info`, `message_record_headphones_confirm`) are overridden in the overlay so the line `Host API: {host_api}` becomes `Device access: {share_mode}` (the localized option label) and the headphone text also gets its placeholders filled by `fmt` (today `message_record_headphones_confirm` is shown unfilled).
- Errors: when a start or job error has `details.kind === "share_mode_unavailable"` show `error_share_mode_unavailable` (`This audio backend cannot fix the device access mode. Choose Auto.`); when `details.kind === "share_mode_refused"` show `error_share_mode_refused` (`The device did not accept {mode} mode: {reason}. Choose Auto to let Impulcifer fall back, or try the other mode.`). Render them in the recorder status line (`setRecorderStatus`) and in the log; in the Stable skin also through the existing `window.alert` path.
- On success, the recorder detail line adds `Output: {output} · Input: {input}` from `job.result.share` with localized mode names (`Exclusive`, `Shared (auto-convert)`), and the `recording_share_mode_opened` log event is rendered through `t()` when its key is known.

## Task 4. The Studio recovery screen, redesigned (`view-recovery`)

Stable keeps the plain three-card form (source, outputs, result) and runs the job in the modal as today. Studio gets a flow that shows its work:

1. **Source card** (`01 Source`): the folder field `#recovery-dir-path` with its browse button, and below it the **inventory** panel `#recovery-inventory` driven by `plan_output_recovery` (call it 300 ms after the last input/change of the path or either option; ignore stale responses; show a small "looking" state while waiting). States, set as `data-state` on the panel: `empty` (no path: the idle hint), `planning`, `ready`, `nothing` (a valid source but no file to create: the ledger is all present), `error` (the error message and code inline, the panel outlined with `--err`).
   - In `ready`/`nothing`: a source badge (`Source: hesuvi.wav` etc.), `48000 Hz · 500 ms` (sample count converted), the speakers as `.chip`s, and the **ledger** `#recovery-ledger`: one row per file (`hrir.wav`, `hesuvi.wav`, and `Hangloose/<SPK>.wav` rows when Hangloose is requested or present) with the file name in mono and a status pill: `present` (kept as is), `will create`, and after a run `created`. Rows come from `existing_files` and `planned_files`; nothing is invented.
2. **Outputs card** (`02 Outputs`): the two checkboxes keep their ids (`#recovery-include-hangloose`, `#recovery-remove-silent-channels`); toggling re-plans.
3. **Result card** (`03 Result`, `.recovery-result` with `data-status`, `#recovery-status-meta`, `#recovery-status-title`, `#recovery-status-detail`, `#recovery-created-row`, `#recovery-existing-row`, `#btn-open-recovery-output`: all kept, the smoke driver reads them): after a run the ledger pills flip to `created`, the summary line stays, and the open-folder button appears.
- The header button `#btn-start-recovery` is disabled in Studio until the inventory is `ready` (in Stable it stays always enabled and the service validates). Failure of the run renders the error in the result card as today.
- In Stable the inventory panel is hidden (`[data-skin="stable"] #recovery-inventory { display: none }`) and the plan is not requested.
- Add the new pieces to `ui/styles.css` with the existing tokens (pills: `present` in `--fg-2` on `--bg-3`, `will create` in `--accent` on `--accent-soft`, `created` in `--ok`).

## Task 5. Overlay catalogue (`crates/impulcifer-service/locales/*.json`)

Every key you add or override goes into all nine files with real translations (you translate; no English fallback text in the non-English files) and identical `{placeholder}` sets; `cargo test -p impulcifer-service --lib settings` enforces parity and `cargo test -p impulcifer-service --test ui_catalog` enforces that every key the page uses resolves. Keep the 2.x prefix conventions (`label_`, `button_`, `option_`, `message_`, `error_`, `tooltip_`, `section_`, `recovery_`, `info_`).

## Task 6. Smoke harness and gallery

- `tests/app_smoke/driver.js`, `tests/app_smoke/smoke.py`, `tests/app_smoke/in_app.py`: the recovery step waits for `#btn-start-recovery` to become enabled (the plan) before clicking, then asserts the ledger shows `hrir.wav` as `created` after success; the bootstrap step asserts `#rf-share-mode` exists and lists `auto`; the settings step no longer references `#sf-frontend`; `frozen_hashes` covers `apps/impulcifer-app/ui`, `i18n` and `crates/impulcifer-service/locales`; the English strings the harness compares against (`self.en`, `params.labels`) are the 2.x catalogue merged with the overlay for that language. Keep `npm run test:contracts` green (the driver contract test loads `driver.js`).
- New `apps/impulcifer-app/tests/ui_gallery.py`: a Playwright script in the pattern of `build_scripts/webview_gallery.py` (mocked `window.pywebview.api`, one init script per scenario) that renders the 3.x page from `apps/impulcifer-app/ui/index.html` with `py -3.14` and the already installed Chromium into `E:/Impulcifer/target/ui-gallery/`: both skins × both themes × en/ko for settings, info, recorder; the Studio recovery screen in every inventory state (`empty`, `planning`, `ready` with hangloose rows, `nothing`, `error`) and after a successful run; the recorder with the access select enabled and disabled and with a `share_mode_refused` error shown. Self-check the shot count and exit non-zero on a mismatch. **Run it and list every PNG path in the report; the owner reviews the pictures.** Do not add it to CI.

## Docs

New `docs/rust/UI.md`: where the 3.x UI lives, the overlay catalogue rule, the typed IPC surface (`ui/ipc.d.ts`) and how to add a method, the Studio/Stable split, the recovery inventory states, how to run the gallery. Update `docs/rust/APP-SMOKE.md` only where the harness steps you changed are described.

## Allowed files

`apps/impulcifer-app/ui/**`, `apps/impulcifer-app/src/*.js`, `apps/impulcifer-app/src/*.d.ts` (new), `apps/impulcifer-app/tests/*.cjs`, `apps/impulcifer-app/tests/ui_gallery.py` (new), `tests/app_smoke/**`, `crates/impulcifer-service/locales/*.json`, `docs/rust/UI.md` (new), `docs/rust/APP-SMOKE.md`. Nothing else: no Rust, no `features.toml`, no `CHANGELOG.md`, no workflows, no `tsconfig.json`/`package.json`, no `webview_ui/`, no `i18n/`.

## Verification (foreground, paste the output)

```
cd E:/Impulcifer/apps/impulcifer-app
npm run typecheck
npm run test:contracts
cd E:/Impulcifer
cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-policy -- app_scripts_opt_into_type_checking
cargo test -p impulcifer-app --test smoke
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out target/ui-gallery
git status --porcelain
```

## Report format

(1) the typing decisions (the helper you introduced, how the envelope union is narrowed, any place the real shape forced a code change); (2) the settings/info layout and the recovery inventory as built, with the id list you kept; (3) the overlay keys added and overridden (count and the list); (4) the full list of PNG paths under `target/ui-gallery/`; (5) the pasted command output with the literal result lines; (6) `git status --porcelain`; (7) anything undone, and whether DB01 had landed when you finished. Do not end your turn before the commands complete.
