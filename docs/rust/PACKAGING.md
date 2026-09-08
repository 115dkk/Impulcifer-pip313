# Impulcifer 3.x packaging (P21)

## Scope and safety

Windows uses Velopack; macOS and Linux use Tauri 2 bundles and the Tauri updater. The single `pywebview_api` command and existing frontend are unchanged. This document specifies commands for the future `.github/workflows/release-3x.yml`; P21 does not create or edit that workflow.

**Never run Setup.exe, Update.exe, or an uninstaller on the development machine for this test.** `%LOCALAPPDATA%/Impulcifer` is the user's live installation and is off limits. A second installation with the same pack ID can change its registration. All local P21 staging is under `target/p21`; SDK fixtures are under `%TEMP%/impulcifer-p21-*` and removed after testing.

The local upgrade test establishes discovery, download, package contents, and a source-verified apply-command plan. It does **not** establish actual 2.x installation, application replacement, restart, or uninstallation. M5 still requires the real-install check on a fresh disposable Windows runner.

## Configuration and runtime data

`apps/impulcifer-app/tauri.conf.json` enables `bundle.active` and `bundle.createUpdaterArtifacts`. `bundle.targets` is deliberately `[]`: Windows must not generate an MSI/NSIS installer in addition to Velopack. Since no platform overlay files are added, macOS/Linux commands below explicitly select their targets; do not use bare `cargo tauri build` expecting installer artifacts.

The resource map is:

```json
{
  "../../data/sweep*.wav": "data/",
  "../../data/harman*.csv": "data/"
}
```

Mapped glob matches are flattened under Tauri's resource directory. `data/` here means `<resource_dir>/data`, not `<resource_dir>/resources/data`. On macOS it is `Impulcifer.app/Contents/Resources/data`; the executable lives in `Contents/MacOS`. On Windows the packaging script independently stages `<packDir>/data` beside the executable. No demo recordings or TrueHD masters are included.

The application passes this directory to `ImpulciferService::with_dependencies`. It prefers an existing `<resource_dir>/data` only when `IMPULCIFER_DATA_DIR` is unset; otherwise it delegates to the existing `impulcifer_service::default_data_dir()`. It does not mutate the environment. The service's existing fallback behavior remains unchanged, including its handling of an override that names a missing directory.

Icons were generated from the repository's Pulse artwork using Tauri CLI 2.11.4. The Windows icon remains byte-identical to `logo/pulse.ico`. To regenerate missing sizes (run from the repository root):

```sh
cargo install tauri-cli --locked
cargo tauri icon logo/pulse-256.png --output apps/impulcifer-app/icons
```

Check that the input exists before regeneration; use the repository's available square Pulse PNG if its size/name changes. The configured desktop icons are `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, and `icon.ico`. Preserve `logo/pulse.ico` as `icons/icon.ico` after generation. The generator also emits mobile/Appx icons under the same allowed directory; those are not runtime data.

## Windows: build and package, never install locally

From a checkout with Rust/MSVC, WebView2 build prerequisites, PowerShell 7, and .NET 10:

```powershell
cargo build -p impulcifer-app --release
pwsh -NoProfile -File E:/Impulcifer/build_scripts/pack_velopack.ps1
```

The script resolves the checkout from its own path, reads `[workspace.package].version`, and creates a unique `target/p21/pack-<guid>/stage` and `releases`. It does not clear or reuse another directory. If vpk is absent it installs the tool with:

```powershell
dotnet tool install -g vpk --add-source https://api.nuget.org/v3/index.json
```

The explicit source handles a machine whose configured NuGet feeds are local-only; it does not modify permanent feed configuration. The tested vpk version is 1.2.0. The packaging invocation is equivalent to:

```powershell
vpk pack --packId Impulcifer --packTitle 'Modern Impulcifer' `
  --packVersion <workspace-version> --packDir <stage> `
  --mainExe impulcifer-app.exe --icon E:/Impulcifer/logo/pulse.ico `
  --shortcuts Desktop,StartMenuRoot --outputDir <output> --channel win
```

Keep `packId=Impulcifer` and channel `win`. The 2.x Python updater reads `releases/latest/download/releases.win.json` and downloads the full nupkg into `packages/`. The executable changes from `ImpulciferGUI.exe` to `impulcifer-app.exe`; the package manifest supplies the new main executable.

Each script invocation prints full artifact paths and byte counts plus a final machine-readable `P21_RESULT=<json>` with `version`, `stage`, `output`, and `data_files`. Use that specific result rather than selecting the newest directory from another worker's output.

Expected files:

- `Impulcifer-win-Setup.exe`
- `Impulcifer-<version>-full.nupkg`
- `releases.win.json`
- `RELEASES`
- vpk also emits `assets.win.json` and `Impulcifer-win-Portable.zip`.

The local artifacts are **not code-signed**. vpk warns that no signing parameters were provided. Tauri's updater signature is a separate mechanism from Windows Authenticode and macOS code signing/notarization. Production signing configuration belongs to the release workflow; do not publish these test artifacts as signed releases.

## Isolated SDK upgrade test

```sh
cargo test -p impulcifer-app
cargo test -p impulcifer-app -- --ignored velopack_upgrade_from_2x_windows
```

The ignored test runs the packaging script, creates an XML NuGet-style `current/sq.version` with ID `Impulcifer`, version `2.13.3`, channel `win`, and main executable `ImpulciferGUI.exe`, and supplies all six `VelopackLocatorConfig` fields explicitly. This is a simulated installed version, not a claim about the user's installed version. The historical Python `_get_pack_id` comment describes a different manifest format; the fixture follows the actual Rust SDK and generated nupkg manifest.

The local `TcpListener` serves only the generated feed and full package through `HttpSource`. The test checks:

1. The service install-kind probe identifies the temporary layout as `velopack`.
2. `UpdateManager::check_for_updates` returns the workspace 3.x version for the 2.13.3 locator, with no delta operations.
3. `download_updates` writes into that fixture's `packages/`, reports progress, and matches the expected size.
4. The nupkg contains `lib/app/impulcifer-app.exe`, the exact allowlisted `lib/app/data/` entries with source-identical bytes, and `lib/app/sq.version` with the correct ID/version/main executable.
5. The planned command targets only the temporary updater, package, root, and packages directory; the locator still reports 2.13.3 because nothing was applied.
6. The temporary fixture is removed.

SDK 1.2.0 extracts its bundled updater into the configured temporary `Update.exe` during a full download. It does not run it. Delta patching would run a helper, so the fixture asserts that there are no deltas.

The SDK exposes no public prepare/dry-run apply API. The test therefore constructs the plan from locator getters and the verified `manager.rs` argument sequence:

```text
<TEMP>/Impulcifer/Update.exe apply --package <TEMP>/Impulcifer/packages/<full.nupkg> --waitPid <test-pid> --root <TEMP>/Impulcifer --packageDir <TEMP>/Impulcifer/packages
```

This is **not interception of an actual SDK apply call**. Restart is the default in SDK 1.2.0; `--norestart` disables it. The local test never calls an apply method.

The initial local HTTP server intermittently produced SDK `UnexpectedEof` despite successful full writes. The fixed server uses a length-delimited keep-alive response and waits for SDK client closure before closing its socket. Server errors are surfaced. `local_feed_sdk_download_drains_body_before_close` checks sixteen 16 MiB downloads without retries and verifies complete bytes and terminal progress; `local_feed_propagates_response_file_errors` checks failure reporting.

### Real-install acceptance check: disposable CI runner only

These are requirements for the workflow author, **not commands to execute on this workstation**. Use a fresh GitHub-hosted Windows runner with no existing Impulcifer installation or registration. Fail rather than reusing an existing install.

1. Resolve and record the newest published stable **2.x** tag with an `Impulcifer-win-Setup.exe` asset. Do not use the unrestricted latest release after 3.x is published. Download that tag's Setup with `gh release download <2.x-tag> --pattern Impulcifer-win-Setup.exe --dir <runner-temp>/baseline` and verify the downloaded artifact under the project's release verification policy.
2. Install it silently with `Start-Process -FilePath <baseline-setup> -ArgumentList '--silent' -Wait -PassThru`; fail on nonzero exit. Silent setup may start the application. Read `current/sq.version` as XML and assert ID/version match the chosen 2.x tag. Record root, package directory, and baseline process path.
3. Close only the process launched from that disposable install. Copy the newly built 3.x full nupkg into its `packages/` directory. Confirm the copied package hash equals the packaging output hash.
4. Set up the existing P15 smoke driver/report mechanism or WebView2 CDP observation before restart. Run that runner's `Update.exe apply` and wait for completion with a bounded timeout. Modern Velopack restarts by default. The 2.x Python updater uses `apply --restart`; exercising that exact wrapper/legacy flag is a separate compatibility assertion against the chosen 2.x updater, not something the new SDK simulation proves.
5. Assert `current/sq.version` now names the 3.x version and `impulcifer-app.exe`. Confirm the restarted process executable is inside that install's `current/`, and obtain `bootstrap` through the existing bridge; compare the returned version with the package version. Manifest or PE version alone is not proof that the app started. Check that bundled sweep/Harman data work with `IMPULCIFER_DATA_DIR` unset. Use the smoke observer described in `APP-SMOKE.md`; do not add another Tauri command or assume an unsupported `--version` CLI switch.
6. Record install_kind, old/new versions, apply exit status, restart/bridge output, and all touched paths. In a `finally` cleanup, stop only fixture-owned processes and run the installed updater's documented `--silent uninstall`. Check that the disposable install/registration was removed and save uninstall output even if earlier assertions failed.

Do not count the local simulated locator test as completion of this real-install M5 requirement. The workflow's `upgrade-windows` job runs `Update.exe apply --package <nupkg>` through `tests/app_smoke/smoke.py --launch-command`, so the updater's own restart relaunches the app with the harness environment, the harness attaches to that process by executable path (`launch.details.relaunched_by_updater`) and drives it like any smoke run.

## macOS and Linux builds

Run on the respective native OS; these commands were documented, not executed on Windows. Install platform prerequisites from the Tauri prerequisites guide (including Linux WebKitGTK 4.1, build tools, and AppImage dependencies). From the checkout:

```sh
cargo install tauri-cli --locked
cd apps/impulcifer-app
# The workflow injects TAURI_SIGNING_PRIVATE_KEY and
# TAURI_SIGNING_PRIVATE_KEY_PASSWORD as environment secrets.
# Do not print them or commit them to a file in the checkout.
cargo tauri build --bundles app,dmg     # macOS runner only
```

On Linux, use the same working directory and signing environment:

```sh
cargo tauri build --bundles appimage
```

The private key must match the public key already configured by P20. Do not replace that public key with one generated during a packaging test. An empty password is allowed only when the injected key is unencrypted. These variables must be set in the build process's environment; a `.env` file alone is not sufficient. Apple signing/notarization requires its own production credentials and validation.

For native builds without `--target` or a custom target directory, paths relative to the workspace root are:

| Platform | Artifact | Purpose |
|---|---|---|
| macOS | `target/release/bundle/macos/Impulcifer.app` | App bundle |
| macOS | `target/release/bundle/dmg/*.dmg` | Installer disk image |
| macOS | `target/release/bundle/macos/Impulcifer.app.tar.gz` | Tauri updater payload |
| macOS | `target/release/bundle/macos/Impulcifer.app.tar.gz.sig` | Updater signature |
| Linux | `target/release/bundle/appimage/*.AppImage` | Installer and updater payload |
| Linux | `target/release/bundle/appimage/*.AppImage.sig` | Updater signature |

For explicit `--target <triple>`, insert `<triple>/` before `release/`. Derive the actual AppImage/DMG names from output rather than assuming Tauri's version/architecture naming. Existing 2.x legacy discovery expects `Impulcifer-<version>-macOS.dmg` and `Impulcifer-<version>-x86_64.AppImage`; name the corresponding release assets that way when preserving 2.x discovery. Renaming must not change signed payload bytes. Keep architecture-specific updater asset URLs distinct if publishing both macOS architectures. A DMG is not the Tauri macOS updater payload: use `.app.tar.gz` in `latest.json`.

### Static updater feed

Upload `latest.json` alongside the artifacts to the existing P20 endpoint:

`https://github.com/115dkk/Impulcifer-pip313/releases/latest/download/latest.json`

Example only, using the literal dummy signature `P21-DUMMY-SIGNATURE-NOT-VALID`:

```json
{
  "version": "3.0.0-alpha.0",
  "notes": "P21 example only; replace signatures before publication.",
  "pub_date": "2026-09-08T00:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "url": "https://github.com/115dkk/Impulcifer-pip313/releases/download/v3.0.0-alpha.0/Impulcifer-aarch64.app.tar.gz",
      "signature": "P21-DUMMY-SIGNATURE-NOT-VALID"
    },
    "linux-x86_64": {
      "url": "https://github.com/115dkk/Impulcifer-pip313/releases/download/v3.0.0-alpha.0/Impulcifer-3.0.0-alpha.0-x86_64.AppImage",
      "signature": "P21-DUMMY-SIGNATURE-NOT-VALID"
    }
  }
}
```

Generate the real JSON using a JSON serializer and the **contents** of each corresponding `.sig` file, not its path, URL, or another encoding of that content. `version` must match the selected release; `pub_date` is RFC 3339. Add `darwin-x86_64` only when its payload was built and signed. Do not add Windows here: installed Windows apps use Velopack. Dummy signatures are intentionally invalid and must never enter the production feed.

The `/releases/latest/` endpoint resolves the newest stable release only, never a GitHub prerelease. Channel policy (Codex #197): a **stable** install reads the stable feed only (`releases/latest/download/latest.json` for the Tauri updater, `GithubSource` without prereleases for Velopack). A **prerelease** install (the running version has a PEP 440 pre segment such as `3.0.0-alpha.0`) reads the rolling `updater-3x-pre` release's `latest.json` first and the stable feed second (`apps/impulcifer-app/src/main.rs::updater_endpoints`), and on Windows asks the GitHub API with prereleases included (`crates/impulcifer-service/src/update/velopack.rs::source_kind`). `release-3x.yml` refreshes `updater-3x-pre/latest.json` on every 3.x release, stable ones included, so an alpha install is offered the following stable. The `latest.json` URLs point at the versioned release assets; the rolling release holds the manifest only.

## Python wheel and sdist

The 3.x wheel publishes to the **same PyPI project as 2.x, `impulcifer-py313`** (owner decision 2026-09-09: existing installs, updater feeds and PyPI dependents stay bound to one project, so users are not asked to migrate to a new package; `pip install --upgrade impulcifer-py313` carries them to 3.x once a stable 3.x is released, and `--pre` reaches the alphas). The project's Trusted Publisher list therefore needs a second entry for `release-3x.yml` (environment `PyPI`) next to the 2.x `publish.yml` one. The Linux wheel links ALSA (`libasound.so.2`, through cpal) and manylinux does not guarantee that library, so the workflow tags it `linux_x86_64`, attaches it to the GitHub Release only and never uploads it to PyPI; Linux `pip install impulcifer` builds from the sdist (Rust toolchain and `libasound2-dev` required). A manylinux wheel needs an ALSA-free or dlopen build of the audio backend first. Run these from `crates/impulcifer-python` so maturin uses its pyproject, not the root 2.x Hatch project:

```sh
cd crates/impulcifer-python
python -m maturin build --release --features python
python -m maturin sdist
```

Artifacts normally go to the workspace `target/wheels/`. P14 supplies the wheel verification; P21 did not rerun wheel/sdist builds. Configure a new PyPI Trusted Publisher for project `impulcifer`, repository `115dkk/Impulcifer-pip313`, workflow filename **`release-3x.yml`**, and environment **`PyPI`**. The workflow author must use that exact filename/environment and OIDC permissions. The existing `impulcifer-py313` publisher is bound to `publish.yml` and does not authorize the new project/workflow. Do not rename or modify the existing 2.x workflow.

## Verification and evidence (2026-09-08)

Requested commands:

```sh
cargo build -p impulcifer-app --release
pwsh -NoProfile -File E:/Impulcifer/build_scripts/pack_velopack.ps1
cargo fmt --all -- --check
cargo clippy -p impulcifer-app --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-app
cargo test -p impulcifer-app -- --ignored velopack_upgrade_from_2x_windows
cargo test -p impulcifer-policy
```

Logs are retained in `target/p21/parent-*.log`, `feed-*.log`, and `upgrade-<pid>.log`. The first independent ignored-test run failed with `UnexpectedEof`; keep `parent-upgrade.log` as evidence of that failure. After the local-server fix, `feed-upgrade-1.log` and `feed-upgrade-2.log` record two successful complete runs. Full workspace formatting had failures in concurrently edited audio/sys-win files outside P21's allowed scope; those were not changed here. Final independent Clippy completed (with five unused-import warnings from the sys-win dependency under `--no-deps`), and the final normal app suite passed 13 tests with 3 ignored. A later independent rerun of the ignored test failed before executing P21: concurrently edited `crates/impulcifer-sys-win/src/lib.rs:608,845` called `shared_buffer_duration` with one argument after its signature changed to two (`E0061`). See `parent-upgrade-final.log`. Thus the worker's two successful SDK runs are valid recorded results, but the final shared working tree is not fully verified.

The worker started both long package-test commands in foreground, but the harness automatically detached them at its 600-second limit. It waited for both to complete. This does not meet the strict no-background requirement, despite the commands' eventual success. No installer/updater apply process was run.

Example successful full SDK run (`upgrade-451968.log`):

```text
simulated 2.x locator version: 2.13.3
available 3.x: 3.0.0-alpha.0
install_kind: velopack (simulated)
progress callbacks: [5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100]
apply NOT RUN
installed after apply NOT MEASURED
uninstall NOT NEEDED (nothing installed)
cleanup: TEMP fixture removed; target/p21 artifacts retained
```

That run produced `target/p21/pack-ecb96a70372e4c539bc3ec8d5863eb76/releases/`:

| File | Bytes |
|---|---:|
| `Impulcifer-win-Setup.exe` | 21360382 |
| `Impulcifer-3.0.0-alpha.0-full.nupkg` | 16883966 |
| `releases.win.json` | 270 |
| `RELEASES` | 88 |
| `assets.win.json` | 210 |
| `Impulcifer-win-Portable.zip` | 16868842 |

Package byte sizes differ between runs as other workers update dependencies; these numbers identify one actual run, not a deterministic-size contract.

Registry proposals (do not edit `features.toml` in P21):

| Registry entry | Existing test reference | Coverage |
|---|---|---|
| `package.velopack_pack` | `impulcifer-app::velopack_upgrade_from_2x_windows` | Ignored Windows test builds and inspects package |
| `package.bundle_config` | `impulcifer-app::bundle_config_contract` | Normal test validates config and data allowlist |
| `package.upgrade_from_2x` | `impulcifer-app::velopack_upgrade_from_2x_windows` | Discovery/download/plan only; retain real-apply acceptance as outstanding |

The actual ignored test's Rust module-qualified name is `windows::velopack_upgrade_from_2x_windows`. The registry convention uses the function name. Additional normal tests are `bundled_data_dir_uses_mapped_resources`, `bundled_data_dir_respects_override`, `bundled_data_dir_requires_existing_directory`, `local_feed_sdk_download_drains_body_before_close`, and `local_feed_propagates_response_file_errors`.

Touched locations include the allowed app/script/document files, Cargo's `target/debug` and `target/release` trees (including generated `data/`), `target/p21` logs/stages/packages, temporary `impulcifer-p21-*` fixtures, and installed CLI/tool caches under the user's `.cargo` and `.dotnet/tools`/NuGet directories. Cargo automatically added the two app path-dependency names to the already-dirty `Cargo.lock`; no unrelated lockfile entries were reverted. The generated mobile/Appx icon paths are inventoried in `target/p21/final-inventory.log`. No checkout `data/`, frontend, service implementation, workflow, registry, user application installation, or signing private key was modified.

## Official references

- [Tauri 2 configuration](https://v2.tauri.app/reference/config/)
- [Tauri resources](https://v2.tauri.app/develop/resources/)
- [Tauri updater and signature/feed format](https://v2.tauri.app/plugin/updater/)
- [Tauri CLI](https://v2.tauri.app/reference/cli/)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Velopack 1.2.0 SDK](https://docs.rs/velopack/1.2.0/velopack/)
- [Locator source](https://docs.rs/crate/velopack/1.2.0/source/src/locator.rs)
- [Update manager source](https://docs.rs/crate/velopack/1.2.0/source/src/manager.rs)
- [Windows Setup CLI](https://docs.velopack.io/reference/cli/content/setup-windows)
- [Windows updater CLI](https://docs.velopack.io/reference/cli/content/update-windows)
