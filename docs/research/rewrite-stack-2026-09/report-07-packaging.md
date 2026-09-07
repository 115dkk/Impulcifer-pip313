# Impulcifer rewrite: packaging, updater, CI and distribution continuity

Research date: **2026-09-07**. Scope: R6, with runtime/CLI constraints that affect packaging. Repository inspected read-only at the supplied checkout, whose version is 2.13.3. No build, installer, upgrade or timing experiment was performed.

**Evidence labels:** **[V]** verified in the cited primary documentation or inspected repository; **[I]** inference, estimate or proposed design; **[U]** unknown/not experimentally verified. A label at the start of a paragraph, list item or table cell applies to that entire item. References `[n]` resolve to the numbered URLs in §6. Local implementation citations use `E:/Impulcifer/`-relative paths. GitHub pages sometimes expose month/day without a year; those timestamps are not treated as verified complete dates. Live documentation is not necessarily version-pinned.

## 1 Verdict

1. **[I] S1 Rust/Tauri 2 is the best packaging choice when PyPI continuity matters:** use a Rust core, a separate CLI, and Velopack on Windows to preserve existing installations.
2. **[I] C#/Avalonia has the simplest Windows installer/runtime story with self-contained .NET and official Velopack integration; Python interoperability requires extra engineering.**
3. **[I] S2 C++/Electron is viable, not disqualified by size. A C++ sidecar avoids Electron addon ABI coupling; Chromium maintenance remains real work.**
4. **[I] S3 Go/Wails is the weakest stable-release choice for this migration: stable Wails v2 lacks an established first-party updater, and Velopack's Go SDK is still planned.**
5. **[V] Correct the brief: Velopack now has an official Python SDK; Wails v3 has an updater, but the inspected v3 release is beta.17.** [1,5,17,18]
6. **[V] Tauri AppImage bundles WebKitGTK components; it does not simply require users to install system WebKitGTK. Base-system compatibility still matters.** [37,38]
7. **[V] C# PyPI continuity is possible through Native AOT plus a C ABI; “none” is wrong. pythonnet is another, less attractive, option.** [54–56]
8. **[I] Prefer native OS builds and packaged upgrade tests over ambitious cross-compilation. Public-repository standard GitHub runner compute is free.** [32]
9. **[V] Latest PyO3 and cibuildwheel have dropped 3.13t; abi3 does not cover 3.14t. Preserve that support deliberately or retire it explicitly.** [45–50]
10. **[I] The first rewritten release must preserve the old Windows feed and Linux/macOS assets; switching installer technology without a tested migration will strand users.**

## 2 Findings (with sources)

### 2.1 What the current release actually does

| Area | Verified implementation and consequence |
|---|---|
| Release graph | **[V]** `publish.yml:30–104,112–621,626–769`: gate → PyPI publication → three separate Nuitka build jobs → one GitHub Release. Every binary job waits for PyPI. One failed OS build prevents the aggregate release. AUR publication follows release creation (`:776–836`). |
| Version and authentication | **[V]** Gate takes `pyproject.toml` as version authority and can auto-PATCH-bump shippable changes. `publish.yml:11–13,62–104` documents PyPI Trusted Publisher binding to **`publish.yml` + environment `PyPI`**. Keeping PyPI means preserving that registration or deliberately changing it; a new Cargo/npm/.NET version field is not automatically understood by today's gate. |
| Windows identity | **[V]** `publish.yml:197–222`: unpinned `vpk`, `packId=Impulcifer`, `packTitle=Modern Impulcifer`, `mainExe=ImpulciferGUI.exe`. Build directory is the Nuitka `gui_main.dist` tree. No previous package download occurs before packing. |
| Windows update | **[V]** `updater/velopack.py:159–180,208–337`: custom Python reads `releases.win.json`, chooses a `Type=full` package, downloads it, and invokes `Update.exe apply`. It does not consume delta assets. `infra/environment.py:42–53,94–100` selects this implementation only when a standalone install has the expected Windows `Update.exe` layout. |
| Non-Windows update | **[V]** `updater/legacy.py:71–181`: downloads artifact plus `SHA256SUMS.txt`; macOS opens the DMG for user installation, not automatic app-bundle replacement. Linux replaces the running AppImage using a temporary sibling plus `os.replace`, then relaunches; non-AppImage installs follow another path. |
| Existing verification | **[V]** Windows accepts missing SHA256/SHA1 (`velopack.py:182–206`); legacy accepts checksum-manifest HTTP 404 for older releases (`legacy.py:71–120`). These are fail-open compatibility exceptions. The workflow does not sign executables, notarize macOS artifacts, or sign the checksum list. HTTPS and same-origin checksums do not establish an independent publisher identity. |
| AUR | **[V]** README `:64–72` and `packaging/aur/PKGBUILD.in:19–60`: package `impulcifer-py313-bin` consumes `Impulcifer-<version>-linux-x86_64.tar.gz`, expects top-level `Impulcifer-linux/`, and installs an executable, icon and licenses under `/opt`. Merely retaining the tarball suffix is insufficient; archive layout matters. |
| CLI and pip | **[V]** `pyproject.toml:56–60` supplies pip console scripts. `updater/executors.py:74–126` runs pip upgrade for pip installs. Standalone is a separate, Windows console-disabled GUI build (`nuitka_flags.py:188–222`). README `:248–256` documents pip/Velopack/legacy update behavior. |
| Packaging drift | **[V]** `nuitka_flags.py:65–90,166–185` works around pywebview plugin selection and SciPy lazy imports. `publish.yml:128–131` pins Windows Python 3.14.6 because 3.14.7 broke Tk discovery; Linux downloads continuous AppImage tooling. These problems disappear only if the Python standalone dependency graph actually disappears, not merely if a new shell wraps it. |

**[I] Migration contract:** keep the Windows pack ID, launch executable name, installation identity, feed URL, full-package manifest entries and checksum fields until upgrading representative old installs succeeds. Continue publishing DMG, AppImage and `SHA256SUMS.txt`; old clients choose assets by suffix. Multiple architecture-specific assets introduce another problem: the old selector is platform-based, not a proven architecture selector. Do not add same-suffix alternatives to the old client’s feed without testing its selection. Keep AUR layout or change its recipe in the same release. These are compatibility obligations inferred from the inspected callers, not a claim that arbitrary payload replacement has already been proven safe.

**[I] Immediate alternative to rewriting solely for updates:** replacing the handwritten Windows downloader with the now-official Python SDK could remove some updater-specific maintenance without changing the DSP implementation. It does not solve Nuitka's SciPy/GUI bundling issues. [5]

### 2.2 Velopack: current support, APIs and platform status

**[V] Version observed:** upstream `releases/latest` resolves to **1.2.0**, with displayed timestamp **03 Jun 23:39**; the extracted release page did not expose a year. Notes explicitly mention Python typings, Rust semver validation, a Linux locator fix and backward-update fallback on macOS. Do not confuse a newer prerelease with stable 1.2.0. [2]

| Language | Official support and in-app sequence | Migration judgment |
|---|---|---|
| C# | **[V] Ready.** `VelopackApp.Build().Run()` before application initialization; `CheckForUpdatesAsync` → `DownloadUpdatesAsync` → `ApplyUpdatesAndRestart`. Modern .NET and .NET Framework are documented. [1,3] | **[I] Lowest integration risk** for Avalonia; run startup handling before UI initialization. |
| Rust | **[V] Ready.** Rust crate; `VelopackApp::build().run()`; `UpdateManager` → `check_for_updates` → `download_updates` → `apply_updates_and_restart`. [1,4] | **[I] Good fit** for Tauri's Rust host. Disable the Tauri updater on installations managed by Velopack. |
| C/C++ | **[V] Ready.** Official prebuilt C interface and C++ wrapper; release archive contains library/header artifacts. `CheckForUpdates` → `DownloadUpdates` → `WaitExitThenApplyUpdate`; application must exit itself. UTF-8 strings. [1,6] | **[I] Good native option**, but Electron should normally own the updater through its JS SDK, not through the DSP child process. |
| JavaScript/Electron | **[V] Ready.** Official `velopack` npm package; startup `VelopackApp.build().run()`, then main-process `checkForUpdatesAsync` → `downloadUpdateAsync` → `waitExitThenApplyUpdate` → `app.quit()`. Native Node-module/bundler integration is documented. [1,7] | **[I] Best continuity choice for S2.** Isolate privileged calls in main process; never expose arbitrary update URLs through renderer IPC. |
| Python | **[V] Ready.** `pip install velopack`; `velopack.App().run()`; `check_for_updates` → `download_updates` → `apply_updates_and_restart`. Official guide covers Windows/macOS/Linux. [1,5] | **[I] The current Python shim is historical project code, not proof that official Python bindings do not exist.** |
| Go | **[V] Planned**, not ready, in the official language matrix. C-callable Velopack API exists. [1,6] **[U]** No official Go SDK or endorsed third-party Go binding was verified. | **[I] Go can use a deliberately maintained C bridge/helper, but that is owned integration code, not an official Go solution.** |

- **[V] `vpk` is application-language-agnostic:** its inputs are an already-built application directory, identity, executable and version. Language guides all package build output the same way. The documented tool installation uses the .NET SDK; Rust's guide specifies SDK 8. This is a **build-host dependency**, not proof that a Rust/C++ application's users need .NET. Do not equate language-agnostic packaging with automatic application bootstrapping: startup integration still has to run. Pin the CLI and SDK/library together. [3–7,9]
- **[V] Delta flow:** `vpk download` retrieves the preceding release; `vpk pack` can then generate Zstandard-based per-file binary differences; upload full + delta + feed. SDK selection uses a heuristic comparing full size, aggregate delta size and chain length. Deltas need a cached base package; individual files must be under 2 GB. Windows, macOS and Linux are documented. **[U]** The cited page does not specify every patch-error fallback; test interruption and corrupt-base handling. [8,9]
- **[I] Today's project will not benefit merely by publishing deltas:** the old client filters for `full`. A bridge/new SDK release must be installed before those clients use deltas. Always keep a full fallback for new installations and old clients.
- **[V] macOS is supported**, not a Windows-only promise: Velopack produces `.app`, `.pkg` and portable ZIP; a DMG can be created separately from the ZIP. App Sandbox is unsupported. Updating a protected installation can require AppleScript-mediated administrator approval. [10]
- **[V] Linux is supported through AppImage**, not DEB/RPM package management: staging under `/var/tmp`, replacement of the existing image, and `pkexec` for protected destinations are documented. Desktop integration is optional/user-managed. [11]
- **[V] Packaging cross-host matrix:** Windows/Linux packages can be made on any supported host; macOS Velopack packages require macOS tools (`codesign`, `xcrun`, `productbuild`). `--runtime` alone is not a cross-compilation switch. [12]
- **[V] Windows prerequisites:** `--framework "webview2,vcredist143-x64"` is a documented pattern. Setup downloads prerequisites; updates also check newly introduced prerequisites before applying. **[I]** Use this when wrapping Tauri/Wails build output with Velopack: their normal installers are then not the installers users run. Test the old `Update.exe` version's transition behavior rather than assuming today's SDK documentation retroactively describes it. [13]

### 2.3 Updater alternatives: suitability, not just availability

| Option | Verified scope | Judgment for this project |
|---|---|---|
| Tauri updater plugin, v2 | **[V]** Windows NSIS/MSI, macOS `.app.tar.gz`, Linux AppImage; detached update signatures are mandatory; `check` → download/install → relaunch. Windows installer mode and pre-exit hook are documented. [14] **[U]** No documented delta mechanism found; do not budget delta savings. | **[I] Good macOS/Linux choice for S1**, and good Windows greenfield choice. Replacing the existing Windows Velopack installer directly is not a transparent migration. Keep Velopack on Windows initially. Tauri update signatures and Authenticode/Developer ID signing solve different problems. |
| electron-updater / electron-builder | **[V]** Official current docs list NSIS on Windows, macOS DMG+ZIP (ZIP needed for updater metadata), and several Linux targets including AppImage. macOS app signing is required. NSIS differential download is enabled unless disabled. [15,16] | **[I] Strong S2 alternative for a fresh install base.** Keep Velopack for Windows continuity unless there is a compelling migration benefit. Do not combine both update engines in one installed app. |
| Electron built-in autoUpdater | **[V]** Current documentation distinguishes Squirrel.Windows, Squirrel.Mac and MSIX; Linux has no built-in updater. This is not the same library as `electron-updater`. [19] | **[I] Do not cite Linux AppImage support as a feature of Electron's built-in updater.** |
| Wails v2 / v3 | **[V]** Stable observed v2.14.0; v3.0.0-beta.17 is a prerelease. Official v3 FAQ/tutorial advertises `app.Updater`, GitHub and other providers, checksums and optional Ed25519 signatures. Direct document fetch returned 403; capability was verified through indexed official pages, release pages and related source. [17,18,40] **[U]** No first-party v2 updater contract was verified. | **[I] “Wails: none” needs qualification.** For a stable v2 release, budget a helper/third-party integration. For v3, require signature verification explicitly and treat upgrade behavior as beta software. It does not automatically consume today's Velopack feed. |
| Sparkle 2 | **[V]** macOS updater, Ed25519 update signatures, Developer ID/notarization guidance, signed deltas and appcast generation. Latest observed 2.9.6, displayed Aug 17; full timestamp year not extracted. [20] | **[I] Mature macOS choice for native hosts**, but a separate Objective-C/Swift/FFI integration and feed to maintain. Unnecessary for Tauri if its plugin meets requirements. |
| WinSparkle | **[V]** Windows DLL with C API, GUI-framework-independent update UI, appcast-based installer delivery; no separate dynamically linked CRT required for WinSparkle itself. Signing migration documentation is linked. [21] **[U]** Latest release and delta support not established. | **[I] Sensible Go/C++ bridge candidate**, but does not retain Velopack installation semantics by magic; do not replace a working official Velopack integration with it. |
| MSIX + App Installer | **[V]** Windows on-launch/background updates; background checks every 8 hours. Settings require Windows 10 1709/1803/1903 depending on feature. A trusted signing certificate is required for ordinary sideloading; self-signed certificates require explicit trust deployment. [22,23] | **[I] Good managed/Store distribution option, poor default migration choice.** Different package identity/storage/installer semantics; R6's user-driven in-app check UX still needs an integration. No macOS/Linux solution. |
| Squirrel.Windows | **[V]** Upstream latest observed 2.0.1, Sept 27, 2020; repository requests maintenance contributors. This is not proof that no work occurs. [24] | **[I] Bad new dependency choice compared with Velopack.** Existing Electron compatibility is a reason to maintain legacy users, not a reason to adopt it for this rewrite. |
| Hydraulic Conveyor | **[V]** Native/Electron/JVM packaging; cross-OS packaging/signing/notarization; automatic/delta updates without application update code. Windows MSIX/App Installer, macOS Sparkle, Linux DEB/tar appear in docs. Free for qualifying public OSS with source-repository URL, matching public binaries and attribution; proprietary use paid. Versioned 18.1/20.0 pages were inspected and marked old, so they do not establish latest release version. [25–27] | **[I] Real unlisted alternative if release engineering dominates**, not a compiler and not a drop-in Velopack migration. Adds vendor/license dependency; AppImage-first Linux and existing Windows identity argue against changing to it now. |

**[V] Source conflict:** Velopack's signing page claims immediate SmartScreen reputation from Azure/EV signing, while Microsoft's updated signing guidance explicitly says Artifact Signing, OV and EV do **not** guarantee instant trust. Prefer Microsoft's platform guidance; do not promise warning-free first installs. [28,29]

### 2.4 Per-stack packaging / updater / CI matrix

**[I] Risk ratings are comparative engineering judgments**, not vulnerability scores: L = established path with little bespoke integration; M = standard components but several boundaries to test; H = important custom or prerelease integration. PyPI support ratings assume preserving a useful Python core, not merely uploading a launcher wheel.

| Stack | Windows packaging and updater | macOS | Linux minimum | PyPI / CLI continuity | CI + runtime risks | Overall |
|---|---|---|---|---|---|---|
| **S1 Rust + Tauri 2** | **L/M:** official Rust Velopack, preserve current identity; explicitly bootstrap WebView2. | **M:** Tauri DMG + signed `.app.tar.gz` updater; native x64/arm64 builds and microphone permission smoke test. | **M:** Tauri AppImage + tarball; bundled WebKitGTK, old-enough build base, FUSE/fallback tests. | **L/M:** PyO3/maturin; independent CLI/core. 3.13t is a declared compatibility decision. | Cargo/native DSP libraries + webview OS behavior; materially less Python freezer machinery. | **Best when PyPI matters.** |
| **S2 C++ + Electron** | **M:** Electron packaged directory + JS Velopack; C++ DSP sidecar included as unpacked executable; deploy CRT or static-link policy. | **M:** Electron app + DMG; official Velopack JS or electron-updater, exactly one; signed nested helpers. | **M:** AppImage + tarball; Chromium bundled, host glibc/desktop libraries remain. | **M:** nanobind/scikit-build-core wheel, separate native CLI. | CMake/native deps plus npm/Electron security updates; addon ABI avoidable through sidecar. | **Viable. Size alone is not a rejection.** |
| **S3 Go + Wails** | **H on stable v2:** vpk packages output, but custom Go→C/helper update integration and bootstrap are required. v3 updater is beta and a different migration path. | **M/H:** native cgo/UI build, DMG helper or beta updater; Cocoa/signing still exists. | **M/H:** v2 tarball with GTK/WebKitGTK system deps; external AppImage work. v3 has documented AppImage tooling but beta status. | **M/H:** C ABI/ctypes possible; richer Python object API is bespoke. Independent Go CLI is easy in architecture, not proof of cgo-free DSP. | Pure-Go cross-build slogans do not cover cgo/UI/audio native dependencies. | **Weakest stable migration fit.** |
| **C# + Avalonia** | **L:** self-contained .NET publish + official C# Velopack; no WebView2 for ordinary Avalonia UI. | **M:** `.app`/DMG + Velopack; signing/runtime entitlements and native audio need validation. | **M:** Velopack AppImage or external packer + tarball; X11/font/native deps remain. | **M/H:** .NET Native AOT C ABI + ctypes or subprocess wrapper; pythonnet requires runtime and has FT gaps. Separate console project. | JIT self-contained builds are conventional; Native AOT/trim only after proving dependency compatibility. | **Best Windows-only packaging ergonomics; weaker Python-native ecosystem fit.** |

### 2.5 Builds, signing, runner time and actual cost

#### Native versus cross-compilation

| Toolchain | Verified boundary | Recommended use [I] |
|---|---|---|
| Rust `cross` | **[V]** Docker/Podman-based; foreign native libraries require target packages/custom images; Darwin/MSVC prebuilt images are not generally supplied. Latest observed release 0.2.5. [41] | Use for extra Linux CLI/core targets, not as the default Tauri/macOS release factory. |
| `cargo-zigbuild` | **[V]** README covers Linux/macOS targets, glibc selection and native-library/SDK limitations; observed 0.23.4, displayed Sep 2. [42] | Useful for Linux ABI floors and selected core artifacts; not a substitute for Windows MSVC, Apple SDKs, or real installer tests. |
| Tauri cross-build | **[V]** Linux→Windows MSVC via cargo-xwin is documented; NSIS works, MSI requires Windows/WiX. Documentation warns of less testing. [39] | Keep Windows-native production builds; cross-build is an optional optimization after baseline parity. |
| Go + cgo + Zig | **[V]** Cross-cgo requires `CGO_ENABLED=1`, target C compiler, headers/libraries/pkg-config. [43] **[I]** `CC="zig cc ..."` only addresses compiler selection; it does not manufacture GTK/WebKit/Apple frameworks. | Native builds for Wails and native audio; cross-build the genuinely portable CLI/core subset if beneficial. |
| C++ / CMake | **[V]** Toolchains specify compiler/sysroot/search policy; `try_run` cannot run target code without emulator or supplied results. [44] | Native MSVC, Apple Clang and Linux compilers with locked dependency manifests. Cross-target libraries do not remove OS test jobs. |
| C# | **[V]** RID-specific managed publishing and self-contained distribution are supported. Native AOT does not support cross-OS compilation; cross-architecture support is conditional on tooling. [35,36] | Native per-OS publishing is the least surprising policy; add AOT only for a validated benefit. |

**[I] Minimum useful desktop matrix:** Windows x64, macOS arm64, Linux x64. Add a macOS x64 cell if Intel Macs remain supported, yielding four native targets; do not claim that `macos-latest` represents both architectures. Explicit runner/target labels prevent silent architecture changes. Build universal macOS output only if all bundled native libraries support both slices. Windows arm64/Linux arm64 are later additions, not implied by the language.

| Stack | Planning range per native target, cached / cold | What the estimate excludes |
|---|---|---|
| Rust/Tauri | **[I] 5–15 / 15–45 min** | New native DSP library builds can dominate. |
| C++/Electron | **[I] 5–20 / 15–60 min** | Heavy C++ libraries/LTO; notarization and package upload. |
| Go/Wails | **[I] 3–12 / 8–25 min** | Custom cgo deps; updater-helper qualification. |
| .NET/Avalonia, normal self-contained publish | **[I] 3–10 / 5–20 min** | AOT can substantially increase build work. |

**[U] These are capacity-planning estimates, not benchmarks of this rewrite or guarantees of improvement over the current workflow.** Golden-output tests, wheel builds and network-dependent signing/notarization are separate. No measured comparison against current Nuitka runner time was available.

- **[V] Cost:** standard hosted runners are free for this public OSS repository; larger runners remain billable. Current listed paid rates are Linux x64 two-core **$0.006/min**, Windows **$0.010/min**, macOS **$0.062/min**. [32]
- **[I] Paid-equivalent example:** 15 Linux + 15 Windows + 15 macOS minutes = **$1.17**, or **$2.10** with a second 15-minute macOS architecture. These are illustrative arithmetic, not the repository's bill; public standard-runner compute remains $0. Parallelization shortens elapsed time, not summed minutes. Artifact storage, signing subscriptions and human diagnosis are separate costs.
- **[V] Windows signing:** Azure **Artifact Signing**, formerly Trusted Signing, Basic is **$9.99/month**, including 5,000 signatures. Current Microsoft eligibility: individuals US/Canada; organizations US/Canada/EU/UK. **[U]** Maintainer eligibility was not established. If the maintainer is an individual resident elsewhere, do not plan around obtaining this service. [29,30]
- **[I] Signing policy:** use trusted Authenticode with timestamping and a stable publisher identity when affordable/eligible; investigate an OSS signing program such as the one Microsoft lists. Self-signed certificates are suitable for controlled test trust, not general public trust. Unsigned EXEs can be deliberately shipped with warning/support consequences; signed MSIX has a different enforced trust requirement. A signature does not guarantee immediate SmartScreen acceptance. [23,29]
- **[V] macOS:** Apple ended `altool` notarization submissions in November 2023; use `notarytool`. Velopack supports Developer ID signing and notarytool profiles. [28,31] **[I]** Sign nested executable code before the enclosing bundle, notarize the distributable, staple and verify. Although the brief allows optional notarization, do not market unsigned/unnotarized downloads as frictionless Gatekeeper-compatible installation. Exact Apple-tool details should be pinned/tested in the chosen packaging scripts.
- **[I] Keep signing distinct from update authenticity:** Authenticode/Developer ID identifies executable publishers; Tauri/Sparkle detached update signatures protect the update transport; `SHA256SUMS` from the same unsigned release only detects corruption/substitution relative to that list. Never carry today's missing-checksum bypass into a new updater.

### 2.6 PyPI continuity and free-threaded wheels

**[V] Existing distribution name and CLI can survive any implementation language:** Python project metadata names the distribution, and console entry points invoke Python callables. A small wrapper can call a native extension, C ABI library or executable. This preserves command names, not automatically Python API behavior, options, numerical output or supported interpreters. [57]

| Option | Verified current capability | Honest assessment |
|---|---|---|
| Rust + PyO3/maturin | **[V]** Mixed Python/Rust packages and console scripts supported. Observed PyO3 **0.29.2 (2026-08-05)**; maturin **1.15.0 (2026-08-24)**. Regular CPython abi3 can target the current Python 3.9 minimum; PyO3 0.29 **removed 3.13t**. 3.14t needs separate interpreter-specific wheels; future abi3t begins at 3.15, not 3.14. [45–47] | **[I] Best continuity.** Keep Python CLI/frontend adapters and a Rust library crate; do not drag Tauri into the wheel. If 3.13t is mandatory, an older compatible PyO3/tooling lane must be deliberately maintained. |
| C++ + nanobind/scikit-build-core | **[V]** Observed nanobind **3.0.1** (PyPI Aug 27, changelog Aug 28, 2026), scikit-build-core **1.0.3 (2026-07-12)**. Nanobind 3 requires Python ≥3.10. Linked `STABLE_ABI` starts at Python 3.12; `FREE_THREADED` supports 3.13+ with version-specific FT wheels. [48,49] | **[I] Good native Python binding, larger compatibility matrix.** Keeping Python 3.9 requires an older compatible binding line or a different ABI strategy. |
| Nanobind 3 split mode | **[V]** Frontend can use stable ABI from 3.10, but separately distributed `nanobind-backend` still needs interpreter-specific binaries. Split FT stable ABI starts at 3.15. [48] | **[I] Do not advertise one universally stable artifact as if the backend disappeared.** For a first rewrite, prefer the established linked configuration unless split-mode testing pays off. |
| Go `c-shared` + ctypes | **[V]** Go exports a C-callable shared library via `//export`; cgo has pointer/pinning restrictions. ctypes loads C libraries and documents FT concurrency caveats. [43,51] | **[I] Realistic for a coarse buffer-oriented DSP API or CLI wrapper.** OS/CPU-specific wheel, not `py3-none-any`; no CPython ABI dependency if C API is truly independent. Manual memory/error/cancellation ownership and Go runtime packaging remain. Not a peer of PyO3 for a rich Python object model. |
| Go + CFFI | **[V]** ABI mode can dynamically load libraries; CFFI itself includes binary components. Observed **2.1.1 (2026-08-03)** has a 3.14t Windows wheel; 3.13t availability was not established for that release. [52] | **[I] Prefer stdlib ctypes initially if it suffices; CFFI adds its own interpreter/distribution compatibility obligation.** |
| C# + pythonnet | **[V]** pythonnet **3.1.0 (2026-05-23)** supports ordinary Python 3.14; FT support PR/issue remained open. CoreCLR hosting requires a suitable runtime; documented embedding does not support self-contained deployment. [54,55] | **[I] Poor default for `pip install` simplicity**, especially with today's 3.9/3.13t/3.14t expectations. It is possible, not “none.” |
| C# Native AOT + ctypes/CFFI | **[V]** Microsoft supports self-contained native shared libraries with `UnmanagedCallersOnly` exports. No installed .NET runtime is required for that library; unloading is unsupported; AOT restrictions apply. [56] | **[I] Plausible PyPI core**, using the same coarse C ABI design as Go. AOT compatibility, buffer ownership and FT safety need proof. An alternative pip wrapper can bundle/launch a standalone CLI, but that is not an in-process Python DSP API. |

**[V] Builder support is separate from binding support:** observed cibuildwheel **4.2.1 (2026-09-05)**. It removed 3.13t in **4.0.0**; 3.14t is included in current builds. Nanobind's ability to target 3.13t does not restore it to the latest cibuildwheel. [50]

**[I] Artifact-count model:** let `P` be the number of platform/architecture/libc targets, not just operating systems. For ordinary Python 3.9–3.14 plus 3.13t/3.14t:

| Binding strategy | Application wheels | Qualification |
|---|---:|---|
| Per-interpreter native extension | `8P` | Only if selected binding versions support all interpreters. |
| PyO3 abi3 + both FT versions | `3P` | Requires a pre-0.29 compatible PyO3 and older/manual 3.13t builder lane. |
| Current PyO3 abi3 + 3.14t | `2P` | Drops 3.13t. |
| Linked nanobind, current 3.x | `5P` | 3.10, 3.11, 3.12+ abi3, 3.13t, 3.14t; drops 3.9 and needs separate 3.13t tooling. |
| Older linked nanobind preserving 3.9 | `6P` | Adds cp39; confirm exact older-version compatibility before pinning. |
| C ABI + pure Python ctypes wrapper | `P` | Platform-tagged, Python-ABI-independent wheel; wrapper/runtime dependencies still matter. |

**[I] Example:** Windows x64 + Linux manylinux x64 + macOS x64 + macOS arm64 makes `P=4`: current Rust strategy gives 8 wheels, current linked nanobind 20, ABI-independent C wrapper 4. Adding musllinux adds another target. ABI3 cuts build artifacts, **not interpreter smoke tests**. A wheel job may build several wheels; wheel count is not job count. Test installation from wheels in clean environments, including FT concurrency and accidental GIL re-enabling. Do not compile a Linux wheel on a recent desktop distribution and assume its glibc floor is portable. [45,48,50,53]

### 2.7 CLI, sidecars and runtime prerequisites

**[I] Recommend a genuine console executable separate from the GUI on all four stacks.** A headless flag in the GUI is possible, but it risks loading the GUI/webview before flag handling, Windows console-subsystem behavior, DISPLAY dependencies and unnecessary runtime baggage. Publish `impulcifer --help/--version` and batch processing independently of GUI initialization. Preserve Python's `impulcifer` entry point with a wrapper. Keep `impulcifer_gui`/`impulcifer_webview` compatibility explicitly or announce their replacement rather than silently removing them.

| Shell | Sidecar arrangement |
|---|---|
| Tauri | **[V]** `bundle.externalBin` bundles target-triple-suffixed binaries; shell-plugin capabilities control renderer-side spawn/execute and arguments. [58] **[I]** Shared Rust library + console binary is preferable to IPC for every DSP call; a sidecar remains useful for process isolation. Do not expose a generic shell permission. |
| Electron | **[V]** Node `child_process.spawn` launches a native executable with streaming pipes and no shell by default. `utilityProcess.fork` runs a Node script, not an arbitrary C++ executable. [59] **[I]** Package the DSP executable outside ASAR; main process owns it and validates requests. Native addon is optional, not required. |
| Wails | **[V]** Go `os/exec` runs external commands without automatic shell expansion; context cancellation defaults to killing the process. [60] **[I]** Package sidecars as platform resources; prefer cooperative cancellation messages before escalation. A helper dedicated to official Velopack C APIs needs its own pinned version and startup integration. |
| Avalonia | **[V]** .NET `Process` starts executables and supports redirected standard streams. [61] **[I]** Shared class library + separate console project; subprocess only where isolation is useful. Avoid bundling the entire UI dependency graph in a Python-facing core. |

**[I] IPC contract if a sidecar is used:** versioned JSON requests, bounded progress/log events with sequence IDs, raw/binary files or shared buffers for large audio, cancellation request followed by a terminal result, no shell interpolation. Preserve the current start/poll/cancel surface for frontend stability. Drain stderr/stdout to avoid pipe deadlocks; reject update application while recording or an output transaction is active. Package the core/CLI/GUI together and update atomically as one application version.

| User machine | Required runtime story |
|---|---|
| Windows / Tauri or Wails | **[V]** WebView2 Evergreen ships with Windows 11 and is installed on most Windows 10 machines, but Microsoft says to check and install it when absent. Edge browser presence is not the same guarantee. Offline installer and fixed-version deployment exist. [33] **[I]** Evergreen bootstrap is the default; fixed runtime trades reproducibility for bundle size and patch responsibility. |
| Windows / C++ | **[V]** Velopack can bootstrap VC++ redistributables. [13] **[I]** `/MD` and native library dependency choices may require the matching redistributable; `/MT`/static CRT can avoid some external CRT requirements but must be consistent with library contracts. Inspect actual PE dependencies rather than asserting all Rust/Go/C++ outputs need or avoid the redist. |
| Windows / Electron | **[V]** Chromium/Node are bundled; no WebView2 requirement. Native addons have Electron ABI requirements unless restricted to a suitable stable Node-API surface. [34] **[I]** A C++ sidecar avoids addon ABI coupling but retains its own DLL requirements. |
| .NET / Avalonia | **[V]** Framework-dependent requires installed .NET; self-contained includes the target runtime but not every OS-native dependency. Single-file and self-contained are different settings. [35] **[I]** Use self-contained for public installers and patch the included runtime through app releases. Budget tens of MB of runtime overhead, potentially 100 MB+ for complete UI/native assets; **[U]** no measured application size exists, and trimming/AOT savings are not assumed. |
| Linux / Tauri | **[V]** AppImage bundles WebKitGTK processes/injected libraries and GTK deployment components; official build-base guidance names Ubuntu 22.04/Debian 12 and warns about glibc. Optional GStreamer bundling is separate. [37,38] **[I]** No blanket system-WebKit install prerequisite for this AppImage; ordinary unpackaged binaries can still depend on system WebKitGTK. |
| Linux / Wails | **[V]** v2 uses GTK3/WebKitGTK (4.1 selected with `webkit2_41`); v3 defaults to GTK4/WebKitGTK 6.0, with GTK3 fallback. v3 beta.17 AppImage code copies WebKit processes/injected libraries and runs linuxdeploy GTK. [40] **[U]** Clean-machine completeness not tested; do not attribute v3 bundling to v2. |
| Linux / Avalonia | **[V]** Ordinary Linux Avalonia docs list X11/ICE/SM/fontconfig and bundled Skia/HarfBuzz native assets. It does not require GTK/WebKitGTK unless a separate WebView feature adds them. [36] |
| AppImage generally | **[V]** Some runtime variants need FUSE 2; documented extract-and-run fallback avoids FUSE where supported. [62] **[I]** AppImage is a packaging format, not a promise of zero host libraries or compatibility with any glibc. |

**[I] Linux minimum policy:** ship a tarball and one tested x64 AppImage; preserve AUR's name and expected layout initially, updating its dependency list for the chosen shell. Let pacman own AUR installs: disable in-app binary replacement there and show the package-manager update instruction. CLI packaging must remain usable without GTK/WebKit/display. Do not require Flatpak for the first release.

**[V] Flatpak** supplies a runtime and sandbox; filesystem access is restricted by default, file portals can grant selected paths, and audio/network/device access requires permissions. [63] **[I]** It is feasible for each stack with manifest work, but direct ALSA/JACK/device enumeration and external ffmpeg downloads need real tests. Use Flatpak's package updates rather than self-replacing installed application code. It is not the minimal Linux option for an audio measurement application.

## 3 Proposed architecture

All recommendations in this section are **[I]**. These are design proposals, not changes made to the repository.

### 3.1 Shared release design

1. **One version authority and immutable release commit.** Keep the gate concept, generate language manifests/assembly metadata from the selected version, and verify consistency. Do not have Cargo/npm/pyproject/.NET each independently bump. If PyPI continues, keep `publish.yml` and `environment: PyPI` or coordinate a Trusted Publisher change.
2. **Separate validation from irreversible publication.** Build wheels and desktop candidates and test them before publication. If retaining the current PyPI-before-desktop policy during initial migration, document the risk that a desktop failure follows an already-published immutable PyPI version. Preferred final graph: `gate → build/test wheel + native desktop candidates → package/upgrade acceptance → publish PyPI (if retained) → publish complete release/feed → AUR`. This deliberately changes today's ordering and requires maintainer approval before implementation.
3. **Native build matrix, pinned inputs.** Explicit OS/architecture; locked Cargo/npm/Go/NuGet/CMake dependency inputs, SDK/compiler/container versions, updater CLI and package tools. No unpinned continuous packaging downloads. Cache dependency builds by toolchain/target/lockfile; do not treat a cache as release provenance.
4. **Artifact tests before feed publication.** Clean install; GUI boot without Python/SDKs installed; CLI without display; locale/assets; sidecar lookup from paths containing spaces; missing WebView2; recording-device enumeration. Add old-installed-version → candidate upgrade, candidate → next release, interrupted download, tampered package, unavailable server, protected install path, settings preservation, rollback policy and attempted update during recording. Real audio I/O still needs a hardware acceptance lane; headless runners cannot prove it.
5. **Golden DSP compatibility is separate from packaging success.** Cross-language results will generally differ from NumPy/SciPy bytes. Use per-stage float64 golden vectors, agreed absolute/relative tolerances and channel-wise final response limits; retain deterministic/hash checks for a fixed new toolchain where appropriate. Never redefine a packaging smoke test as proof of R9 parity.
6. **Release publication is a transaction at the feed level.** Upload complete immutable-version assets/checksums/signatures first; expose update metadata only when all advertised targets exist. Retain full packages and required preceding versions for deltas. Use one updater owner per installation, with explicit install kind: Velopack, Tauri/plugin, AppImage, AUR, Flatpak, pip or portable.
7. **Linux priority needs a precise policy.** It can be non-blocking only if old Linux clients are not led to an incomplete newest release. Freeze its feed/asset pointer or continue a compatibility artifact while Windows/macOS advance; do not simply skip its job while publishing `latest` metadata that old clients expect to contain a Linux payload.

### 3.2 S1: Rust + Tauri 2

`Rust DSP library + Rust console CLI + Tauri host + optional PyO3 wrapper`

- `gate → Rust unit/golden tests + frontend validation → native Windows/macOS/Linux builds; maturin wheel lane → packaged acceptance → publication`.
- Windows: Tauri release executable/resources → **Velopack 1.2.0-compatible pinned SDK/CLI pair**, existing pack ID/executable name/feed, WebView2/redist prerequisites as needed; previous full package fetched for delta generation. Preserve executable naming even if internal crate names differ.
- macOS: Tauri `.app` and DMG, x64/arm64 as explicitly selected; Tauri updater archive/signature. Apple microphone usage metadata and signed-bundle capture are release smoke-test requirements.
- Linux: Tauri AppImage and compatibility tarball; Tauri updater only for supported AppImage installs. AUR disables in-app mutation.
- PyPI: core-only maturin package plus current Python command wrapper; no Tauri/WebView dependencies. Decide 3.13t support before selecting PyO3 version.
- Avoid dual Windows installation systems: do not ship an NSIS Tauri installer as if it transparently upgrades the old Velopack install.

### 3.3 S2: C++ + Electron

`CMake DSP library + C++ CLI/worker + Electron main/preload + existing web frontend + optional nanobind module`

- `gate → C++ unit/golden + JS checks → native CMake builds → package Electron with unpacked worker/assets → updater packaging → upgrade acceptance → publication`.
- Windows: official JS Velopack, same installation identity and executable name. Include the compiled worker and audited dependent DLLs; renderer gets narrow application IPC, not filesystem/shell/updater authority.
- macOS/Linux: use the same official JS Velopack integration for consistency if package smoke tests pass; create DMG separately on macOS and AppImage on Linux. Alternatively use electron-updater for those install kinds, but not concurrently with Velopack. Document this decision explicitly.
- PyPI: separate scikit-build-core/nanobind target from the same C++ library; no Electron or Node runtime in the wheel. CLI is the real native executable, not `electron --headless`.
- Prefer subprocess JSON/file communication over a Node addon initially; this removes a needless ABI boundary. Do not claim Node-API eliminates dependencies on arbitrary third-party C++ ABIs.

### 3.4 S3: Go + Wails

`Go DSP packages + independent console command + Wails host + versioned updater helper`

- Stable-release route: `gate → Go tests/golden → native cgo/Wails v2.14 builds + CLI → helper integration → package/upgrade acceptance → publication`.
- Windows: wrap the official Velopack C interface in a small dedicated helper or carefully tested cgo adapter; perform both startup lifecycle handling and check/download/apply, not merely a `vpk pack` step. C helper owns no DSP. This is additional maintained code and the principal reason this stack loses the packaging comparison.
- macOS: package app/DMG with a proven helper/Sparkle integration, or retain checksum-verified manual DMG installation initially with clear user expectations. Linux: v2 tarball+AUR first; optional externally assembled AppImage.
- Wails v3 alternative: qualify beta.17's updater/AppImage implementation on clean targets before adopting it; require Ed25519 verification and a specific old-Velopack migration. Do not plan as if GA is already shipped.
- PyPI: platform wheel containing a Go C ABI library with ctypes wrapper, or an explicitly narrower subprocess CLI bridge. Avoid custom rich-object bindings until actual user demand justifies them.

### 3.5 C# + Avalonia

`.NET DSP class library + console project + Avalonia application + optional Native AOT C-ABI export project`

- `gate → managed unit/golden tests → per-RID self-contained publish → Velopack package on native OS → install/upgrade acceptance → publication`.
- Windows/macOS/Linux: one official C# Velopack API and update model. Preserve Windows identity; generate DMG from macOS app output and Linux AppImage plus AUR tarball. Velopack, not unverified Avalonia Parcel AppImage support, supplies the Linux packaging plan.
- Default to normal self-contained JIT distribution first. Native AOT for the Python-facing core is a separate compatibility project; do not force reflection-heavy GUI/audio dependencies through trimming solely to reduce size.
- Python: Native AOT exported C API + ctypes if a real pip core is required. pythonnet is a fallback for users willing to manage .NET and ordinary CPython, not the default promise for FT users.
- Treat bundled .NET updates like bundled Chromium updates: a smaller source tree does not remove runtime security-release responsibility.

## 4 Risks ranked

| Rank | Risk | Severity / most affected | Required mitigation [I] |
|---:|---|---|---|
| 1 | **[V/I] Installed-base migration depends on feed, pack identity, startup hooks, executable names and suffix-based asset selection. Arbitrary replacement was not tested.** | Critical / all stacks | A bridge release and actual old-installer upgrade tests; retain full package + checksum/feed compatibility. No simultaneous updater/identity rewrite without evidence. |
| 2 | **[V/I] Current checksums fail open in two compatibility cases; origin compromise defeats unsigned same-origin hashes. Signing keys can also strand updates if lost.** | High / all | Fail closed in new clients; independent signature validation where supported; protected release/signing credentials and tested rotation/recovery policy. |
| 3 | **[V] Toolchain drift conflicts with Python 3.9/3.13t support.** | High / Rust and C++ PyPI; pythonnet | Explicit supported-version policy; dedicated older compatibility lane or announced retirement; interpreter-by-interpreter installed-wheel tests. |
| 4 | **[V/I] Stable Go/Wails requires extra updater integration; v3 functionality is prerelease.** | High / S3 | Adopt only after prototype upgrade qualification; otherwise accept helper ownership or choose another stack. |
| 5 | **[V/I] Native DSP/audio libraries still require ABI/OS/architecture management, regardless of shell.** | High / all | Lock dependencies; inspect binary dependencies; clean-machine smoke tests and real multichannel hardware checks. |
| 6 | **[V/I] Linux AppImage bundles may still fail due to glibc/FUSE/graphics/audio assumptions.** | Medium-high / all; webviews especially | Old-enough build base, no-system-WebKit test, extract-and-run test, documented baseline and tarball fallback. |
| 7 | **[V/I] Signing eligibility/cost and macOS notarization differ from current unsigned distribution.** | Medium-high / all | Verify maintainer eligibility rather than assume Azure service availability; choose explicit unsigned/trusted-signed policy and account for user friction. |
| 8 | **[I] Monolithic all-platform release gating can make low-priority Linux block Windows; dropping it carelessly breaks latest-release clients.** | Medium / all | Decide feed partitioning before making Linux non-blocking; retain complete old-client assets. |
| 9 | **[V/I] Bundled Chromium/.NET/WebKit/native libs need security patch releases; deltas are not a patch policy.** | Medium / all | Pin known-good inputs and schedule monitored updates; measure payloads rather than arguing from shell labels. |
| 10 | **[I] Cross-build optimization can add more untested SDK/toolchain combinations than it removes runners.** | Medium / all | Native release baseline first; optimize only measured bottlenecks. |

## 5 Open questions you could not settle

- **[U] No actual rewritten package exists:** sizes, cold/warm timings, delta compression ratios, clean-machine dependencies, installation paths and end-to-end old→new updates remain unmeasured.
- **[U] Existing Velopack updater compatibility:** documentation does not prove that every shipped `Update.exe` can consume the chosen modern SDK/CLI package and newly introduced WebView2/runtime prerequisites. Preserve full manifests and test installed historical releases.
- **[U] Minimum OS/CPU support policy:** Intel macOS, Windows arm64, Linux arm64, Windows 10 retirement and Python 3.9/3.13t retention were not specified. All materially affect matrices and installer selection.
- **[U] Go bindings:** no official ready Go Velopack SDK was verified; existence/quality of every community binding was not surveyed. Wails v3 beta updater's full failure/rollback semantics and old-install migration are untested.
- **[U] Tauri delta updates:** official updater docs describe complete artifacts; no supported delta mechanism was verified. This is not proof that no external project can add one.
- **[U] AppImage completeness:** source confirms Tauri/Wails WebKit bundling intent and implementation, not zero dependency requirements on every distribution. Wails v2 automatic AppImage bundling was not verified.
- **[U] Avalonia's latest framework version and Parcel's general AppImage support were not established.** Plan uses documented self-contained publishing and Velopack packaging instead. XPF-specific dependencies are not treated as ordinary Avalonia requirements.
- **[U] Signing:** maintainer jurisdiction/identity eligibility, Apple credentials, willingness to pay and OSS signing-program acceptance are unknown. Notarization is allowed to be optional by the brief; a final product policy is still needed.
- **[U] Rich Python API continuity:** a pip-installed CLI wrapper is easy to describe; preserving existing importable classes/functions with identical semantics is a different project. Whether users require that API was not established.
- **[U] New DSP AOT/binding/library suitability and numerical tolerances:** outside this packaging dimension. No claim of byte parity or FT safety is made from build support alone.
- **[U] Full-date precision/latest versions:** several GitHub release extractions omitted years; Wails docs returned 403 and were partially verified using official indexed pages plus tagged source. electron-builder stable 26.16.0 was observed, but current unversioned updater docs may include newer/prerelease behavior. Pin exact production versions and rerun compatibility tests at implementation time.

## 6 Sources

All accessed **2026-09-07**. Numbered entries may contain several closely related primary URLs. Version observations belong to §2; a documentation URL containing a version is not evidence that it is the latest release.

1. Velopack language support matrix: https://docs.velopack.io/
2. Velopack stable release 1.2.0: https://github.com/velopack/velopack/releases/tag/1.2.0 ; latest resolver: https://github.com/velopack/velopack/releases/latest
3. Velopack C#: https://docs.velopack.io/getting-started/csharp
4. Velopack Rust: https://docs.velopack.io/getting-started/rust
5. Velopack Python: https://docs.velopack.io/getting-started/python
6. Velopack C/C++: https://docs.velopack.io/getting-started/cpp
7. Velopack JavaScript/Electron: https://docs.velopack.io/getting-started/javascript
8. Velopack deltas: https://docs.velopack.io/packaging/deltas
9. Velopack deployment CLI: https://docs.velopack.io/distributing/deploy-cli
10. Velopack macOS: https://docs.velopack.io/packaging/operating-systems/macos
11. Velopack Linux: https://docs.velopack.io/packaging/operating-systems/linux
12. Velopack cross-host packaging: https://docs.velopack.io/packaging/cross-compiling ; runtime identifiers: https://docs.velopack.io/packaging/runtime
13. Velopack prerequisite bootstrapping: https://docs.velopack.io/packaging/bootstrapping
14. Tauri 2 updater: https://v2.tauri.app/plugin/updater/
15. electron-updater supported packages/signing: https://www.electron.build/docs/features/auto-update/ ; observed builder release: https://github.com/electron-userland/electron-builder/releases/latest
16. NSIS differential downloads/API: https://www.electron.build/docs/api/electron-updater.class.nsisupdater/
17. Wails release status: https://github.com/wailsapp/wails/releases/tag/v2.14.0 ; https://github.com/wailsapp/wails/releases/tag/v3.0.0-beta.17
18. Wails v3 updater and beta FAQ (official indexed content; direct fetch returned 403): https://v3.wails.io/tutorials/04-self-update-a-wails-app/ ; https://v3.wails.io/faq/
19. Electron built-in autoUpdater: https://www.electronjs.org/docs/latest/api/auto-updater
20. Sparkle documentation and observed release: https://sparkle-project.org/documentation/ ; https://github.com/sparkle-project/Sparkle/releases/latest
21. WinSparkle official site/C API: https://winsparkle.org/
22. MSIX App Installer update settings: https://learn.microsoft.com/en-us/windows/msix/app-installer/update-settings
23. MSIX trust requirements: https://learn.microsoft.com/en-us/windows/msix/app-installer/installing-windows10-apps-web ; https://learn.microsoft.com/en-us/windows/msix/package/signing-package-overview
24. Squirrel.Windows releases/maintenance notice: https://github.com/Squirrel/Squirrel.Windows/releases ; https://github.com/Squirrel/Squirrel.Windows
25. Conveyor current product page: https://conveyor.hydraulic.dev/
26. Conveyor versioned FAQ (marked old): https://conveyor.hydraulic.dev/18.1/faq/
27. Conveyor OSS conditions (versioned documentation, marked old): https://conveyor.hydraulic.dev/20.0/tutorial/tortoise/8-site/
28. Velopack signing/notarization integration (SmartScreen claim conflicts with Microsoft): https://docs.velopack.io/packaging/signing
29. Microsoft code-signing options, updated 2026-08-29: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
30. Artifact Signing prices/quotas: https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-change-sku
31. Apple's dated notarytool migration announcement: https://developer.apple.com/news/?id=y5mjxqmn
32. GitHub Actions billing and runner pricing: https://docs.github.com/en/billing/concepts/product-billing/github-actions ; https://docs.github.com/en/billing/reference/actions-runner-pricing
33. Microsoft WebView2 distribution: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
34. Electron bundled runtime and native modules: https://www.electronjs.org/docs/latest/ ; https://www.electronjs.org/docs/latest/tutorial/using-native-node-modules/ ; Node-API ABI: https://nodejs.org/api/n-api.html
35. .NET deployment models: https://learn.microsoft.com/en-us/dotnet/core/deploying/ ; single-file distinction: https://learn.microsoft.com/en-us/dotnet/core/deploying/single-file/overview
36. Avalonia Linux: https://docs.avaloniaui.net/docs/deployment/linux ; Parcel: https://docs.avaloniaui.net/tools/parcel/packaging-for-linux ; XPF-only AppImage guidance: https://docs.avaloniaui.net/xpf/deployment/linux ; Native AOT cross-compilation: https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/cross-compile
37. Tauri AppImage baseline/GStreamer: https://v2.tauri.app/distribute/appimage/
38. Tauri WebKit bundling proof: https://v2.tauri.app/release/tauri-bundler/v2.2.3/ ; https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/linux/appimage/linuxdeploy.rs ; https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/linux/appimage/linuxdeploy-plugin-gtk.sh
39. Tauri Windows installers/cross-build: https://v2.tauri.app/distribute/windows-installer/
40. Wails Linux source/docs: https://raw.githubusercontent.com/wailsapp/wails/master/website/docs/gettingstarted/installation.mdx ; https://raw.githubusercontent.com/wailsapp/wails/master/website/docs/gettingstarted/building.mdx ; https://v3.wails.io/getting-started/installation/ ; https://v3.wails.io/guides/build/linux/ ; https://github.com/wailsapp/wails/blob/v3.0.0-beta.17/v3/internal/commands/appimage.go ; https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/v3/internal/commands/linuxdeploy-plugin-gtk.sh
41. Rust cross: https://github.com/cross-rs/cross ; https://github.com/cross-rs/cross/releases/latest
42. cargo-zigbuild: https://github.com/rust-cross/cargo-zigbuild ; https://github.com/rust-cross/cargo-zigbuild/releases/latest
43. Go cgo and build modes: https://pkg.go.dev/cmd/cgo ; https://pkg.go.dev/cmd/go
44. CMake cross toolchains/try_run: https://cmake.org/cmake/help/latest/manual/cmake-toolchains.7.html ; https://cmake.org/cmake/help/latest/command/try_run.html
45. PyO3 0.29.2 release history/distribution: https://pyo3.rs/v0.29.2/changelog ; https://pyo3.rs/v0.29.2/building-and-distribution
46. PyO3 FT safety and 3.13t removal: https://pyo3.rs/v0.29.2/free-threading ; https://github.com/PyO3/pyo3/releases/tag/v0.29.0
47. maturin release/layout/bindings: https://pypi.org/project/maturin/ ; https://www.maturin.rs/project_layout.html ; https://www.maturin.rs/bindings.html
48. nanobind current release and ABI modes: https://pypi.org/project/nanobind/ ; https://nanobind.readthedocs.io/en/latest/changelog.html ; https://nanobind.readthedocs.io/en/latest/api_cmake.html ; https://nanobind.readthedocs.io/en/latest/packaging.html ; https://nanobind.readthedocs.io/en/latest/free_threaded.html
49. scikit-build-core: https://pypi.org/project/scikit-build-core/ ; https://scikit-build-core.readthedocs.io/en/latest/configuration/index.html
50. cibuildwheel current release/3.13t removal: https://pypi.org/project/cibuildwheel/ ; https://cibuildwheel.pypa.io/en/latest/changelog/ ; https://cibuildwheel.pypa.io/en/v4.2.1/options/
51. ctypes concurrency/shared-library behavior: https://docs.python.org/3.14/library/ctypes.html
52. CFFI current release/ABI mode/FT: https://pypi.org/project/cffi/ ; https://cffi.readthedocs.io/en/latest/whatsnew.html ; https://cffi.readthedocs.io/en/latest/using.html
53. Python wheel compatibility tags: https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/
54. pythonnet runtime hosting: https://pythonnet.github.io/pythonnet/python.html ; current release: https://pypi.org/project/pythonnet/ ; https://github.com/pythonnet/pythonnet/releases
55. pythonnet unreleased FT support: https://github.com/pythonnet/pythonnet/issues/2720 ; https://github.com/pythonnet/pythonnet/pull/2721
56. .NET Native AOT shared libraries and limitations: https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/libraries ; https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/
57. Python distribution metadata and console scripts: https://packaging.python.org/en/latest/specifications/pyproject-toml/ ; https://packaging.python.org/en/latest/specifications/entry-points/
58. Tauri sidecars/capabilities: https://v2.tauri.app/develop/sidecar/
59. Node process spawning: https://nodejs.org/api/child_process.html ; Electron utility process: https://www.electronjs.org/docs/latest/api/utility-process
60. Go subprocess support: https://pkg.go.dev/os/exec
61. .NET Process API: https://learn.microsoft.com/en-us/dotnet/api/system.diagnostics.process
62. AppImage FUSE and fallback: https://docs.appimage.org/user-guide/troubleshooting/fuse.html
63. Flatpak sandbox/portals/audio permissions: https://docs.flatpak.org/en/latest/sandbox-permissions.html

**Local primary evidence, inspected without modifications:** `E:/Impulcifer/.github/workflows/publish.yml`; `build_scripts/nuitka_flags.py`; `updater/velopack.py`; `updater/legacy.py`; `updater/executors.py`; `infra/environment.py`; README standalone/AUR/update sections. Related contracts were traced through `updater/update_checker.py`, `packaging/aur/PKGBUILD.in`, `.github/scripts/release_gate.py`, `pyproject.toml` and `build_scripts/build_nuitka.py`. Local citations describe this checkout; upstream tool URLs do not independently verify local behavior.
