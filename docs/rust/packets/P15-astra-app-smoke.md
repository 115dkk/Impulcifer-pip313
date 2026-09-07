# P15 (ASTRA): the Tauri app end to end with the 2.x frontend (M3 smoke: BRIR, recovery, settings, recording)

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `apps/impulcifer-app/src/main.rs` (Tauri 2 shell, one command `pywebview_api`, `TauriHost`), `apps/impulcifer-app/src/bridge.js` (the `window.pywebview.api` polyfill and the `pywebviewready` event), `apps/impulcifer-app/tauri.conf.json` (`frontendDist: ../../webview_ui`, bundle inactive), `apps/impulcifer-app/Cargo.toml`, the frozen frontend `E:/Impulcifer/webview_ui/index.html` and `app.js` (element ids: `#nav`, `#view-processing`, `#bf-dir-path`, `#btn-generate-brir`, `#brir-steps`, `#view-recovery`, `#recovery-dir-path`, `#btn-start-recovery`, `#recovery-status-title`, `#view-settings`, `#sf-language`, `#view-recorder`, `#rf-record-dir`, `#btn-record-headphones`, `#btn-start-recording`; `boot()` runs on `pywebviewready` and calls `bootstrap()`; `check_for_updates`/`start_update`/`apply_pending_update` are called from the settings/info views and currently receive `INTERNAL_ERROR ... not implemented` envelopes), the service contract tests `crates/impulcifer-service/tests/{ipc.rs,brir_ipc.rs,recording_ipc.rs,recovery.rs}`, `docs/rust/HARDWARE.md` (the CABLE-A virtual cable pair used for hardware tests), `docs/rust/ARCHITECTURE.md` section 7 and milestone M3 ("recording, BRIR and recovery work in the UI").

Run every command in the foreground. Never use background execution. Other workers are editing `crates/impulcifer-service/**`, `crates/impulcifer-plots/**`, `crates/impulcifer-io/**`, `crates/impulcifer-policy/**`; treat those as read-only. The frontend (`webview_ui/**`) and the i18n catalogues are frozen: if the app cannot work without changing them, stop and report exactly what and why.

## Scope
1. **Build the app**: `cargo build -p impulcifer-app --release` on this Windows machine (WebView2 runtime is installed). Record the executable path (`E:/Impulcifer/target/release/<bin>.exe`).
2. **Smoke harness** `E:/Impulcifer/tests/app_smoke/smoke.py` (Python 3.14; install `playwright` into `py -3.14` with `py -3.14 -m pip install playwright`; no browser download is needed for CDP attach). It launches the executable with the environment variable `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<free port>` (WebView2 honours it; verify with WebSearch if unsure) and `USERPROFILE`/`HOME` pointed at a temp directory (fresh settings), attaches with `playwright.sync_api.chromium.connect_over_cdp("http://127.0.0.1:<port>")`, finds the app page, and drives the frozen UI:
   - waits for `bootstrap` to finish (the processing view renders and `#btn-generate-brir` is enabled), captures every console message and page error;
   - **BRIR**: fills `#bf-dir-path` with a temp copy of `E:/Impulcifer/data/demo` (inputs only, like `crates/impulcifer-service/examples/demo_brir.rs::DemoCopy`), leaves the test signal on `auto`, clicks `#btn-generate-brir`, polls `#brir-steps` / the status text until the job finishes (time limit 120 s), asserts `hesuvi.wav` and `README.md` exist in the copy;
   - **Recovery**: copies that `hesuvi.wav` alone into a second temp directory, opens the recovery view, fills `#recovery-dir-path`, clicks `#btn-start-recovery`, waits for `#recovery-status-title` to report success, asserts `hrir.wav` was created;
   - **Settings**: switches `#sf-language` to `ko`, asserts a known label changed to the Korean catalogue text and that `<temp>/.impulcifer/settings.json` now holds `"language": "ko"`, switches back to `en`;
   - **Updates**: opens the info/settings view where `check_for_updates` is called and asserts the UI shows the error envelope without breaking (no uncaught page error);
   - **Recording** (`--hardware` flag only): selects the CABLE-A output and input of `docs/rust/HARDWARE.md` in the recorder view, records the headphones sweep (`#btn-record-headphones`) into a temp directory, asserts `headphones.wav` exists with 2 tracks at 48 kHz;
   - writes `target/app-smoke/summary.json` (each step: ok/fail, duration, console errors) and a screenshot per view to `target/app-smoke/*.png`; exits non-zero on any failure or on any uncaught page error other than the known not-implemented update envelope.
3. **Rust test hooks** in `apps/impulcifer-app/tests/smoke.rs`: `bridge_script_contract` (reads `src/bridge.js`: defines `window.pywebview.api`, invokes `pywebview_api` with `{ method, args }`, dispatches `pywebviewready` after DOM ready) and `#[ignore] app_smoke_cdp_windows` (runs `py -3.14 tests/app_smoke/smoke.py --exe <release exe>` and asserts exit 0; documented as the local M3 check like `recording.hardware`).
4. **Fixes**: if the smoke finds a defect in the app shell or the service envelopes, fix it in `apps/impulcifer-app/**` and report; service-side defects go into the report as a list (other workers own the service now).

Do not edit `features.toml`; report the test names for `app.pywebview_polyfill` and propose `app.smoke_brir`, `app.smoke_recovery`, `app.smoke_settings`, `app.smoke_recording`.

## Allowed files
`E:/Impulcifer/apps/impulcifer-app/**` (except `tauri.conf.json` bundle settings), `E:/Impulcifer/tests/app_smoke/**`, `E:/Impulcifer/docs/rust/APP-SMOKE.md` (how to run, what it checks, the evidence of this run with the summary table and the screenshot list). Nothing else.

## Hard rules
- `#![forbid(unsafe_code)]`; the app crate keeps a single Tauri command.
- Temp directories only; never process `data/demo` in place; never write into the checkout's `.impulcifer` settings.
- The frontend stays byte-identical.

## Verification (foreground, paste output)
```
cargo build -p impulcifer-app --release
cargo fmt --all -- --check
cargo clippy -p impulcifer-app --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-app
py -3.14 E:/Impulcifer/tests/app_smoke/smoke.py --exe <release exe> --hardware
cargo test -p impulcifer-app -- --ignored app_smoke_cdp_windows
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim including the smoke summary; (3) table: UI flow, result, duration, console errors seen; (4) defects found and what you fixed or deferred; (5) test names for the registry; (6) anything undone. Do not end your turn before the commands complete.
