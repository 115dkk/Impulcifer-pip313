# P18 (ASTRA): remove shell subprocesses from the service (local time, locale, OS description) and quiet the ffmpeg spawn

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `crates/impulcifer-service/src/brir/outputs.rs` (`local_date`: spawns `powershell.exe Get-Date` on Windows and `date` elsewhere for the README timestamp), `crates/impulcifer-service/src/settings.rs` (`platform_locale`: spawns `powershell.exe` / `locale` for first-run language detection; `normalize_language`, `LANGUAGES`), `crates/impulcifer-service/src/lib.rs` (`os_description`: spawns `cmd.exe ver` / `uname`; `get_system_info`), `crates/impulcifer-io/src/ffmpeg.rs` (the legitimate external-tool spawn), `crates/impulcifer-policy/tests/gates.rs` (how the policy gates scan sources), the 2.x behaviour they mirror: `E:/Impulcifer/application/impulcifer_service.py` (`get_system_info`, `platform.platform()`), `E:/Impulcifer/i18n/localization.py` (first-run language detection from `locale.getdefaultlocale`/`getlocale`), `E:/Impulcifer/core/pipeline_stages.py` (README date `datetime.now().strftime('%Y-%m-%d %H:%M:%S')`).

Why: the PA03 audit found that every BRIR run and every service start spawns a shell. In the Tauri app (a windowed process) those spawns can flash console windows and cost 100 ms or more each, and they fail in sandboxes without a shell. Replace them with in-process, cross-platform crates. No `unsafe` in our code (dependencies are outside the unsafe budget).

Run every command in the foreground. Never use background execution. Other workers are editing `crates/impulcifer-plots/**`, `crates/impulcifer-service/src/brir/run.rs`, `apps/**` and `tests/app_smoke/**`; treat those as read-only. Put new dependencies in the crate `Cargo.toml` files, not in the root `Cargo.toml` (another worker edits the workspace members list).

## Scope
- `local_date` (outputs.rs): `chrono::Local::now().format("%Y-%m-%d %H:%M:%S")`. Same text as Python. The `Catalog::readme_date` injection for tests stays.
- `platform_locale` (settings.rs): `sys_locale::get_locale()` (BCP 47 like `ko-KR`), then the existing normalization (`ko-KR` to `ko`, `zh-Hans-CN` to `zh_CN`, `zh-Hant-TW`/`zh-TW` to `zh_TW`, unknown to `en`). Keep the first-run rule documented in the module (Python's ordered `startswith` mapping).
- `os_description` (lib.rs): `os_info::get()` rendered like Python's `platform.platform()` as closely as the crate allows (`Windows-11-10.0.22621`, `Linux-6.8.0-...`, `macOS-14.5-arm64`; exact equality with Python is not required, but the OS name, version and architecture must appear). Verify the `os_info` feature flags and platform support with WebSearch/WebFetch before adding it; if it needs a feature you are unsure of, say so in the report.
- ffmpeg (impulcifer-io): on Windows add `CREATE_NO_WINDOW` (`0x0800_0000`) through `std::os::windows::process::CommandExt::creation_flags` so the GUI never flashes a console; keep everything else.
- Policy gate: add `no_shell_subprocesses` to `crates/impulcifer-policy/tests/gates.rs`: scan `crates/*/src/**/*.rs` and `apps/*/src/**/*.rs` (comments and strings stripped the way the existing scanner does, then also the raw text) for `Command::new(` and fail unless the file is `crates/impulcifer-io/src/ffmpeg.rs`. Register nothing yourself; report the test name.

### Tests (add to the service crate; unit tests next to the code are fine)
- `local_date_has_python_format_and_current_clock`: regex `^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$` and within 5 seconds of `chrono::Local::now()`.
- `platform_locale_maps_to_a_supported_language`: whatever `sys_locale` returns normalizes to one of `LANGUAGES`; plus table cases for `ko-KR`, `zh-Hans-CN`, `zh-Hant-TW`, `pt-BR` (to `en`), `None` (to `en`).
- `os_description_names_the_platform`: contains `Windows`, `Linux`, `macOS` or `Darwin` matching `cfg!` of the test host, and a digit.
- The existing README tests (`crates/impulcifer-service/tests/brir_outputs.rs`, `demo_parity.rs`) must still pass unchanged.
- `impulcifer-policy::no_shell_subprocesses` passes on the final tree and fails if you temporarily re-add a `Command::new("cmd.exe")` (show this once in the report, then remove it).

## Allowed files
`E:/Impulcifer/crates/impulcifer-service/src/brir/outputs.rs`, `E:/Impulcifer/crates/impulcifer-service/src/settings.rs`, `E:/Impulcifer/crates/impulcifer-service/src/lib.rs` (only `os_description` and its helpers), `E:/Impulcifer/crates/impulcifer-service/Cargo.toml`, `E:/Impulcifer/crates/impulcifer-io/src/ffmpeg.rs`, `E:/Impulcifer/crates/impulcifer-policy/tests/gates.rs`, `E:/Impulcifer/Cargo.lock`, `E:/Impulcifer/tests/migration/README-service.md` (a short section on the replacements). Nothing else. Do not edit `features.toml`.

## Hard rules
- `#![forbid(unsafe_code)]` stays; the ffmpeg `creation_flags` call is a safe API.
- Dependency licenses must be MIT or Apache-2.0 (check with WebFetch on crates.io); no crate that pulls in a C build.
- Do not change the README text, the settings file format, or the `get_system_info` response keys.

## Verification (foreground, paste output)
```
cargo fmt --all -- --check
cargo clippy -p impulcifer-service -p impulcifer-io -p impulcifer-policy --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service
cargo test -p impulcifer-io
cargo test -p impulcifer-policy
```

## Report format
(1) files changed and the dependencies added with versions and licenses; (2) verification outputs verbatim; (3) a before/after table of the three functions (what they spawn now versus what they call); (4) deviations from Python's strings; (5) test names for the registry (`policy.no_shell_subprocesses`); (6) anything undone. Do not end your turn before the commands complete.
