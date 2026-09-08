# P20b (Daybreak): updater review fixes for PR #194 (behavioral Velopack test, prerelease ordering, installer download timeout)

Work in the git worktree `C:/Users/32170336/AppData/Local/Temp/impulcifer-verify` (branch `claude/rust-p20` is checked out there; do not switch branches, do not commit, do not touch `E:/Impulcifer`). Read `C:/Users/32170336/AppData/Local/Temp/impulcifer-verify/CLAUDE.md` (section "3.x Rust 워크스페이스") and `docs/rust/ARCHITECTURE.md` first. Rules: `#![forbid(unsafe_code)]` everywhere, no new runtime dependencies (dev-dependencies only as allowed in section 1), no shell subprocesses (`impulcifer-policy` gate), the IPC surface (`pywebview_api` methods and their JSON shapes) does not change. Run every command in the foreground and never in the background. Never run any `Update.exe`, never read or write `%LOCALAPPDATA%/Impulcifer` (the user's live 2.x install), never terminate a process.

Codex reviewed PR #194 (commit 7ed262a) and raised three findings. Fix all three.

## 1 (P1). `updater.velopack` needs a behavioral test

`crates/impulcifer-service/src/update/velopack.rs` builds `UpdateManager::new(HttpSource, options, None)`; the third argument `None` makes the SDK locate the install itself, so the executor cannot run outside a real Velopack install and the only registered test is `velopack_kind_selected_on_windows_with_update_exe`, which merely calls `install_kind::detect`.

Design (fixed, do not redesign):
- `UpdateOptions` (`crates/impulcifer-service/src/update/mod.rs`) gets `pub velopack_root: Option<PathBuf>` (default `None`). When it is `Some(root)`, `velopack::manager` passes `Some(VelopackLocatorConfig { RootAppDir: root, UpdateExePath: root/Update.exe, PackagesDir: root/packages, ManifestPath: root/current/sq.version, CurrentBinaryDir: root/current, IsPortable: false })`; when `None` the SDK auto-locates as today. Nothing else in the executor changes.
- Move `TemporaryRoot` and `LocalFeed` (the in-process HTTP feed server) out of `apps/impulcifer-app/tests/packaging.rs` into a new file `crates/impulcifer-service/tests/support/local_feed.rs` (plain functions and structs, `pub`), and include it from both test crates with `#[path = "..."] mod local_feed;` (the app test keeps its behaviour; keep its log path handling as it is). The service test crate must not gain a dependency on the app crate.
- New test in `crates/impulcifer-service/tests/updater.rs`, `#[cfg(windows)]`, named `velopack_executor_downloads_from_local_feed_and_stages_apply`: build a simulated install under a fresh temp root (`Update.exe` is an inert text marker "P20b inert locator marker; never execute", `current/sq.version` manifest with `<id>Impulcifer</id><version>2.13.3</version><channel>win</channel>`, empty `packages/`), serve `releases.win.json` and one synthetic full package from `LocalFeed`, then drive the real service: `UpdateOptions { install_kind: Velopack, platform: "windows", current_version: "2.13.3", releases_url: http://127.0.0.1:port, velopack_root: Some(root), .. }` through the same `start_update` job path the other executor tests use (see `legacy_executor_downloads_verifies_and_opens` for the harness). Assert: the job result equals `restart_result()` (`requires_restart: true`), the progress messages contain `update_downloading`, at least one `Downloading: N%` and `update_installing` in that order, the package file exists under `packages/` with exactly the served bytes, and the stage is present afterwards.
- The synthetic package must satisfy whatever `download_updates` verifies. Read the SDK source under `%USERPROFILE%/.cargo/registry/src/*/velopack-1.2.0/src/` (`manager.rs`, `download.rs`, `bundle/`) before writing the feed: fill `SHA1`/`SHA256`/`Size` of the asset entry from the synthetic bytes if the SDK checks them, and use the file name pattern `Impulcifer-<version>-full.nupkg`. If the SDK opens the package as a zip (nuspec parsing) after download, build a minimal valid zip in the test instead of raw bytes; `zip`, `sha1` and `sha2` are already in `Cargo.lock` (`sha2` is a dependency of the service crate; `sha1` and `zip` are not), so if the test needs `sha1` or `zip`, add them as `[dev-dependencies]` of `crates/impulcifer-service/Cargo.toml` at the version already locked and nothing else; list what you used in the report.
- The apply step: read `apply_updates_and_restart` / `apply_updates_and_exit` in `manager.rs`. If a failed spawn of `Update.exe` returns `Err` before any `std::process::exit`, the test also calls `apply_pending_update` and asserts an `UPDATE_FAILED` error that names the failure (the inert marker is not a program) and that the stage is consumed; if the SDK could exit the process even on spawn failure, do not call apply, assert only that the stage exists (`apply_pending_update` must then not be called) and state that in the report.
- Do not touch `features.toml`; the caller registers the test.

## 2 (P2). Prerelease ordering (`crates/impulcifer-service/src/update/check.rs`)

`is_newer_version("3.0.0-alpha.0", "v3.0.0")` returns false today because both sides are normalised to `3.0.0`. Fixed rule: keep 2.x behaviour whenever the normalised bases differ (the 30 golden pairs in `tests/migration/goldens/p20_updater.json` stay as they are). When the bases are equal, the latest is newer if and only if its pre-release rank is higher: parse the full strings with `pep440_rs` (already a dependency; `3.0.0-alpha.0` and `v3.0.0-rc1` are valid PEP 440 spellings), compare only the pre-release component (`Version::pre()`), a version without a pre-release ranks above any pre-release, and post/dev/local segments are ignored (so `2.3.1 -> 2.3.1.post1` stays false and `v2.3.1 -> v2.3.1-beta` stays false). Strings that do not parse keep today's result.

Exactly one golden pair changes: `3.0.0-alpha.0 -> v3.0.0-rc1` becomes `true`. Keep the golden JSON untouched; in `golden_asset_selection_and_version_compare_match_python` add a small explicit list of deliberate 3.x divergences (that pair only) and assert the divergent value for it. Add unit tests for: `3.0.0-alpha.0 -> v3.0.0` true, `3.0.0-alpha.0 -> v3.0.0-rc1` true, `3.0.0-rc1 -> 3.0.0-alpha.0` false, `3.0.0 -> 3.0.0-rc1` false, `3.0.0-rc1 -> 3.0.0` true, `2.3.1 -> vv2.4.0-20241129123456` true. Document the rule and the divergence in `tests/migration/README-updater.md`.

## 3 (P2). Installer downloads need their own timeout (`update/mod.rs`, `update/legacy.rs`)

`UpdateOptions::agent()` sets `timeout_global(Some(10 s))` and `legacy::execute` reuses it for the installer download, so a body that takes longer than 10 s aborts. Add `UpdateOptions::download_agent()` with `timeout_global(None)`, `timeout_resolve`, `timeout_connect` and `timeout_recv_response` set to `self.timeout`, and `timeout_recv_body(None)`; use it in `legacy.rs` for the installer body only (the GitHub metadata request and `SHA256SUMS.txt` keep `agent()`). Test in `updater.rs`: with `timeout = 1 s`, a local server that streams the installer body in chunks over about 3 s must still complete, checksum-verify and open (extend the mock server used by `legacy_executor_downloads_verifies_and_opens`); keep `check_failures_and_timeout_are_retryable` passing.

## 4. `updater_plugin_registered_with_public_key` fails on CRLF checkouts

`apps/impulcifer-app/tests/smoke.rs` (around line 90) reads a source or config file and searches for a string that contains a line feed; on a Windows checkout with `core.autocrlf=true` the file has CRLF endings and the test fails (it passed in the LF worktree and in CI). Normalise the text read by the test (`replace("
", "
")`) before searching, so the test is independent of the checkout's line endings.

## Allowed files
`crates/impulcifer-service/src/update/{mod.rs,velopack.rs,check.rs,legacy.rs}`, `crates/impulcifer-service/tests/updater.rs`, new `crates/impulcifer-service/tests/support/local_feed.rs`, `apps/impulcifer-app/tests/packaging.rs` (only to include the moved module), `apps/impulcifer-app/tests/smoke.rs` (section 4 only), `tests/migration/README-updater.md`. `crates/impulcifer-service/Cargo.toml` only for the dev-dependencies named above (`Cargo.lock` may change only as a consequence). Nothing else; in particular not `features.toml`, `CHANGELOG.md`, the workspace `Cargo.toml`.

## Verification (foreground, paste the output)
```
cd C:/Users/32170336/AppData/Local/Temp/impulcifer-verify
cargo fmt --all -- --check
cargo clippy -p impulcifer-service -p impulcifer-app --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service --test updater
cargo test -p impulcifer-app --test packaging
cargo test -p impulcifer-app --test smoke
cargo test -p impulcifer-policy
git status --porcelain
```

## Report format
(1) what the SDK verifies on download and how the synthetic package satisfies it; (2) the apply-path decision (called or not, and the SDK evidence); (3) the exact new test names; (4) the pasted command output with the literal `test result:` lines; (5) `git status --porcelain`; (6) anything undone. Do not end your turn before the commands complete.
