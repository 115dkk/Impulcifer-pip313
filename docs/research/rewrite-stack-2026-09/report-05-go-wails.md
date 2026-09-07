# Go + Wails for an Impulcifer rewrite

Research date: **2026-09-07**. Repository inspection was read-only. No implementation, hardware trial, installer run or benchmark was performed.

Evidence labels throughout: **[V] verified** in the cited primary source or inspected local code; **[I] inference/proposal** based on that evidence; **[U] unknown**, not established by this research. Source numbers resolve to URLs in section 6. Mutable documentation/source was observed on the research date; pinned versions are identified where available. Some official documentation returned HTTP 403, so official repository documentation was fetched instead. Search-only evidence is identified where it matters.

## 1. Verdict

1. **[I] Go + Wails is a credible shell/service choice but a poor default for a complete scientific-DSP rewrite by one maintainer.** The expensive work is numerical compatibility, not the UI bridge.
2. **[V] Wails v3 is beta, not GA:** `v3.0.0-beta.17`, published 2026-09-06; its advertised stable desktop API is a compatibility intention, not stable-release status. [1–3]
3. **[V] v2 remains the stable series; v2.15.0, dated 2026-08-17, is the newest v2 release found in this investigation.** Earlier v2.12/v2.13 snapshots are stale. [4–5]
4. **[I] If choosing Wails for a new prototype, target pinned v3 behind a small adapter; require packaged acceptance tests before committing to it.** Starting v2 merely to migrate shortly afterward adds unnecessary work.
5. **[V] No bundler or runtime `node_modules` is required.** JavaScript bindings plus bundled runtime are supported; the default vanilla starter nevertheless uses Vite. [6–9]
6. **[V] The brief's updater premise is outdated:** v3 has built-in `app.Updater`; community Velopack Go bindings also exist. Neither establishes compatibility with the existing installed product. [19–23]
7. **[I] Preserve the current cursor-based job protocol.** Events should notify the UI that data is available, not replace replayable job state.
8. **[V/I] Gonum supplies useful primitives, not a SciPy replacement.** Bounded TRF fitting, the actual NNResample filter and minimum-phase conventions require substantial verification work. [31–39]
9. **[I] Approve only after two prototypes pass:** real 16-output/2-input recording on the intended Windows hosts, and golden-file DSP comparisons including bounded EQ fitting.
10. **[I] Keeping a Go C ABI/PyPI package is feasible but adds a native-wheel product; it does not preserve today's simple Python packaging automatically.** [46–48]

## 2. Findings (with sources)

### 2.1 Release state, migration and maintenance

| Question | Finding |
|---|---|
| Is v3 stable and released? | **[V] Released as a prerelease, not stable GA.** GitHub API for beta.17 reports `prerelease: true` and `published_at: 2026-09-06T15:11:20Z`. Official docs call it beta with a stable desktop API; v2 remains stable. [1–3] |
| Latest observed v2 | **[V] v2.15.0 exists.** Release page identifies August 17; official changelog search supplies **2026-08-17**, following v2.14.0 on August 10 and v2.13.0 on July 6. **[U]** The latest-release API was inaccessible in the main investigation, so this is the latest version found, not proof that no newer tag exists. [4–5] |
| Maintainer activity | **[V]** Official v3 history has beta.10–17 on August 19, 20, 21, 25, 26, 27, 29 and September 6. Beta.17 fixes fatal/nil request handling in Windows WebView2; beta.15 increased WebView2 embedding timeout to 60 seconds. **[I]** Active maintenance, but recent startup/crash fixes are reasons to test packaged builds, not evidence of finished stabilization. [2] |
| GA target | **[V]** Fetched GitHub milestones schedule GA for September 15, 2026, with RC and beta milestones beforehand. **[U]** These are plans; the milestone listing does not establish that install/rollback, desktop, security and documentation release gates have passed. [10] |
| Migration | **[V]** v2 `wails.Run`/bound objects/context-based runtime calls become v3 `application.New`, separately created windows, registered services and manager APIs. Generated bindings and event imports change. Manual migration is supported; the experimental assistant is not part of the beta. **[I]** This is a real port, not a module-version edit. [3,11] |
| Documentation for coding agents | **[V]** There are guides, examples, a migration guide, source and release notes. However, the methods/models examples fetched contain uppercase field access inconsistent with lowercase Go JSON tags; runtime docs describe both npm and bundled-runtime approaches. **[I]** Pin CLI, Go module, runtime and generated bindings together; give agents version-specific source and contract tests rather than asking them to combine v2, alpha and beta examples. [6–8,11] |

**[I] Recommendation:** use v3 for a time-boxed feasibility branch, not an unconditional production rewrite. v2 is the conservative shipping choice if release must happen immediately, but the shell functions needed here do not justify a v2-to-v3 migration halfway through a long DSP port.

### 2.2 Actual frontend port effort

**[V-local] Inspected all of `E:/Impulcifer/webview_ui/app.js`, lines 1–1488**, including the final blank line. Also inspected the service's bootstrap, job snapshot, poll, cancellation, job-start/completion and event retention code; `index.html` script loading; and ADR 0001. The ADR forbids routing the frozen native CTk frontend through the web service; this proposal does not do that.

| Existing code | Port assessment |
|---|---|
| `app.js:58` central `api()` returning `window.pywebview.api` | **[I] Low bridge effort:** preserve this call facade and map snake_case methods to generated exported Go methods such as `StartBRIR`, `PollJob`, `CancelJob`. Do not mechanically rewrite every UI handler. |
| 23 distinct API names called in this file | **[V-local]** Includes `resolve_recording_paths` and `set_frontend`, beyond the brief's approximate method list. Preserve/test the exact interface rather than treating “about 22” as a schema. |
| `app.js:542–674`: startup validation, confirmation retry, 250 ms polling, cursor, terminal handling | **[I] Preserve initially.** This is already suitable for Go jobs. Keep `{ok,data,error}` envelopes; otherwise native Go errors become rejected promises and bypass existing `response.ok` handling. |
| `app.js:682–846`: separate updater polling and staged restart | **[I] Preserve UX, adapt backend.** Restart must remain an explicit action after successful staging. A default Wails updater window should not silently replace this existing nine-language UI. |
| `app.js:928–943`, `1386–1487`: explicit `window.pywebview` checks and `pywebviewready` | **[I] Must change:** readiness checks need an adapter/runtime-loaded state. Merely replacing `api()` leaves `refreshResolvedPath` disabled and startup waiting for an event that Wails never emits. |
| `app.js:1102–1173`: payload omission/default rules | **[V-local/I]** Closed groups omit fields; `null`, omitted values and zero have distinct meanings. A Go struct of plain numeric fields would lose distinctions. Use explicit presence-aware DTO parsing and a canonical core config/defaults function. |
| `app.js:1185–1212`: language/theme/skin | **[I] Reuse:** DOM translation and CSS theme/skin changes are frontend-only. Native title-bar synchronization needs a separate platform adapter. |
| `app.js:1216–1244`, `1370–1374`, `1421–1428` | **[V-local/I]** System info explicitly says Python/GIL; settings offer CTk/WebView and backend names are pywebview-specific. Update these fields and catalogue entries. Do not claim the entire file is transport-agnostic. |
| `app.js:1430–1447`: resume active job after frontend reload | **[I] Preserve durable-in-process Go job state and replay.** A fresh event subscription is not enough to restore stages/logs after reload. |
| `index.html:728` classic `<script src="app.js">` | **[I]** Change to an ES module entry, or load a small module adapter and explicitly boot afterward. Generated ESM imports cannot simply be dropped into the current classic script. |
| Plots | **[V-local]** This file has no large plot-array streaming path. It requests plot generation as job options and opens output directories. **[I]** Adding embedded plots is new functionality, not a necessary cost of changing the bridge. |

**[I] Overall UI effort:** a bounded adapter/lifecycle/DTO/type-checking task, not a frontend redesign. The renderer, state model, CSS, layout, most handlers and translation approach can survive. No defensible total person-day estimate follows from source size alone; audio, DSP and packaging dominate the uncertainty.

### 2.3 Plain JavaScript, bindings, development without Vite

| Requirement | Answer |
|---|---|
| JavaScript instead of TypeScript output | **[V]** `wails3 generate bindings` emits JavaScript with JSDoc; `-ts` selects TypeScript. `-d` sets the output directory. Go JSON tags determine frontend property names. [6–7] |
| No npm runtime dependency | **[V]** `wails3 generate bindings -b` bundles the runtime alongside generated code and removes the `@wailsio/runtime` npm requirement. Separately, `wails3 generate runtime` produces `runtime.js` and `runtime.debug.js`, usable as ES modules and exposing a global `wails` API. [6,8] |
| No bundler | **[V/I]** Supported by the prebuilt-runtime approach. Use browser-resolvable relative module URLs and static assets embedded with Go `embed.FS`. Do not leave a bare `@wailsio/runtime` import in unbundled browser code. [6,8,12] |
| Does `vanilla-js` mean no Vite? | **[V] No.** Current starter docs use Vanilla + Vite; `vanilla` defaults to TypeScript and `vanilla-js` selects JavaScript. Template choice and bundler elimination are separate decisions. [9] |
| Type-check only in CI | **[V/I]** `tsc --allowJs --checkJs --noEmit` can validate JS without emitted artifacts; TypeScript is a development/CI tool, not a shipped runtime dependency. Generated models/JSDoc help. The current `$()` returns generic/nullable DOM elements and state has implicit shapes, so adding `checkJs` is not guaranteed to pass without annotations and narrowing. [13] |
| Is `tsc` literally the only build step? | **[I] No.** Go compilation, Wails binding/runtime generation and native packaging still occur. The achievable requirement is no JavaScript bundler and no runtime Node installation. |
| No-Vite development loop | **[V]** `wails3 dev` uses configurable `build/config.yml` watcher/execution entries and Taskfiles; default entries install dependencies and run a frontend server. **[I]** Replace those tasks with bindings generation and Go build/run, serve the static asset tree, and reload/restart on HTML/CSS/JS changes. A full restart is adequate initially. **[U]** This exact no-Vite setup was not executed; stock Vite HMR must not be promised after removing Vite. [12,14] |

**[I] CI proposal:** regenerate bindings with the pinned CLI, check the generated diff, type-check JS without emitting files, run DOM/protocol tests, run Go tests and cross-platform package smoke tests. Bind only the UI service, not every exported DSP method.

### 2.4 Platform WebViews, dialogs and native appearance

**[V]** Wails uses WebView2 on Windows and the system WebKit on macOS; v3 Linux now defaults to **GTK4 + WebKitGTK 6.0**, with a GTK3/WebKit2GTK 4.1 compatibility build through v3.0.x and planned removal in v3.1. Docs require Go 1.25+; macOS needs Xcode Command Line Tools. [15–16]

**[I]** Wails has the same broad system-WebView deployment tradeoff as Tauri: not shipping Chromium reduces app payload but does not eliminate browser-engine, distro, compositor or driver variation. It is inaccurate to describe the Linux dependencies as exactly identical across framework versions. For this product, promise a narrow tested Linux target rather than universal AppImage portability.

**[V] Concrete Linux failures exist.** Official docs describe NVIDIA DMA-BUF blank windows and `WEBKIT_DISABLE_DMABUF_RENDERER=1`, including automatic mitigation when Wails detects the driver. A Wails v2.11/v2.12 report on Ubuntu 26.04/WebKitGTK 2.52.3 reproduced Go's fatal “non-Go code set up signal handler without SA_ONSTACK flag”; the issue is closed and linked to a fix, but release availability was not established here. This is reported native runtime interaction, not proof that every Wails/Linux app crashes. [16–17]

**[V]** v3 exposes native file dialogs through `app.Dialog.OpenFile()`, filters and selection methods; directory selection uses `CanChooseDirectories(true)`. Windows window options include `Theme` and `CustomTheme`, with dark/light native title-bar palettes. Source shows `SystemDefault` responding to system theme changes. [18]

**[U] Runtime user-selected native theme:** the inspected public `WebviewWindow` source has no `SetTheme` or `SetOptions`; changing background color is not changing the native title bar. Fixed dark title-bar startup and system-following behavior are verified; forcing light/dark independently of the OS at runtime needs a pinned-version API check or a small Windows-specific adapter. Frontend CSS theme/language switching is already implemented locally. Open-path/open-url remain thin OS service operations; preserve validation and test Unicode paths, spaces and forbidden URL schemes.

### 2.5 Updater: correct the premise before choosing an implementation

**[V] Wails v3 DOES have a built-in updater.** The pinned beta.17 tutorial says `app.Updater` is already attached to each application, requires `Init`, supports `CheckAndInstall`, periodic checks, a default/customizable window and GitHub, Keygen and Sparkle AppCast providers. Changelog entries add manifest/endpoint support and fixes for cross-volume Windows updates; beta.2 excludes Windows installer assets by default. **The brief's “Wails has no built-in updater” should not be repeated for v3.** [19–20]

| Option | Verified scope | Suitability and unresolved work |
|---|---|---|
| v3 `app.Updater` | **[V]** Tutorial covers an EXE on Windows, zipped `.app` on macOS and executable Linux binary; staging and helper-mode replacement/relaunch. SHA-256 sidecars and optional Ed25519 are documented. [19] | **[I]** Evaluate as an integrated alternative, not automatically as a Velopack replacement. **[U]** Existing Velopack installs, extra DLLs/resources, AppImage lifecycle, interruption/rollback and mandatory signature enforcement require tests/source review. |
| Official Velopack Go SDK | **[V]** Official language matrix still says **Planned**. Official C API exists; observed Velopack stable release was 1.2.0, June 3, 2026. [21–22] | **[I]** A narrow owned Go wrapper over the official C ABI is viable; it adds cgo/native-library version management. |
| Community `quaadgras/velopack-go` | **[V]** Exists; cgo wrapper exposes run/check/download/apply-and-restart. Repository has platform linker files/native libraries. [23] | **[U]** Official support, maintenance continuity and binary-version matching are unproven. Inspected Windows arm64 linker naming appears inconsistent with available GNU/MSVC binaries; build-test before adopting. “No Go binding exists” is false. |
| Custom Velopack CLI integration | **[V]** Windows `Update.exe apply --package <FILE>` applies a prepared local package; `vpk` is primarily a packaging/distribution tool. Hook handling without SDK is documented. [24] | **[I]** Go may discover/download/verify full packages and hand off apply/restart, but must own feed parsing, validation, installation discovery, locking, lifecycle arguments, partial downloads and shutdown. Not a two-line SDK replacement. |
| `creativeprojects/go-selfupdate` v1.6.0 | **[V]** Release discovery, OS/arch selection, downloading, unpacking, executable replacement; SHA-256/ECDSA/PGP validators. Validator is optional. [25] | **[I]** Plausible for a truly single-executable product, not equivalent to installing/updating a multi-file desktop product. |
| `minio/selfupdate` v0.6.0 | **[V]** Binary/file replacement with optional checksum and minisign verification and rollback attempts. Does not provide the full desktop-release discovery/installer experience. [26] | **[I]** Useful low-level primitive, not the preferred Windows updater here. |

**[V/U] Security caution:** the v3 tutorial promises verification when a release supplies a signature, not a fail-closed policy rejecting every unsigned release. Its default GitHub provider does not fetch a separate signature sidecar; custom provider/Keygen is suggested. The signing example also uses OpenSSH-generated keys with a helper expecting raw Ed25519 bytes without showing conversion. This research did not establish a safe mandatory-signature production configuration. Do not copy that sample blindly. [19]

**[I]** Authenticode/Developer ID signing and update-payload authentication are separate requirements. An unsigned checksum downloaded beside an artifact does not authenticate a compromised feed. Require an authenticated, versioned manifest or signed payload with an embedded trust root; test missing/invalid signatures, wrong channel/architecture, downgrade and interrupted installation. Do not assert Velopack enforces an expected Authenticode publisher on every update without verifying that policy. [19,24–27]

**[I] Recommended first shipping design:** retain Velopack for existing Windows users, with one narrow update service preserving today's stage/confirm/restart behavior. Evaluate v3's updater separately for a new installation channel. Do not have two updaters concurrently own the same installation.

### 2.6 Packaging and binary size

| Platform/concern | Finding |
|---|---|
| Windows v2 | **[V]** `wails build -nsis`; `-webview2 download|embed|browser|error`. `embed` includes an approximately 150 KB bootstrapper, **not the complete WebView2 runtime**. Fixed-version runtime can be distributed separately using `WebviewBrowserPath`. [28] |
| Windows v3 | **[V]** `wails3 package GOOS=windows` builds an NSIS installer and prepares a WebView2 bootstrapper. Bootstrapper installation requires network; offline distribution needs the Evergreen Standalone Installer or a separately managed fixed runtime. Signing tasks support PFX/certificate store and timestamps. [29] |
| Windows product choice | **[I]** NSIS is an available default, not an obligation. If retaining Velopack, package the Wails executable and native dependencies with Velopack, preserving product identity and update feeds. Explicitly verify WebView2 prerequisite handling in that flow rather than assuming NSIS scripts still run. |
| macOS | **[V]** `.app` plus `wails3 task darwin:package:dmg`; DMG creation requires macOS. Signing/notarization tasks exist. **[I]** Include microphone permission metadata and test first-launch permission denial/retry as part of audio packaging. [29] |
| Linux | **[V]** `wails3 task linux:create:appimage` exists. **[I]** AppImage is a distribution format, not proof of compatibility with all WebKit/GTK/driver combinations. [16,29] |
| cgo/cross-compile | **[V]** Plain Wails Windows can be cross-compiled without cgo; macOS/Linux WebViews require native tooling. Once PortAudio or Velopack's C ABI is added, Windows also has cgo/native linking. Official Docker/Zig cross-build tools do not guarantee inclusion of app-specific C libraries. [30] |
| Practical CI | **[I]** Build and test on native Windows/macOS/Linux GitHub runners. Pin Go, Wails, PortAudio, Velopack and plot dependencies. Zig can help, but must not replace validating the actual linked libraries, native signing and platform installation. |
| Binary size | **[V]** Wails v3 FAQ estimates a typical executable around 10 MB. **[U]** No Impulcifer build was measured. **[I]** Native libraries, fonts, demo assets, chart JS, debug symbols and a bundled WebView runtime change the result; webview subprocess memory is separate from EXE size. Neither “10 MB product” nor an exact savings ratio against Nuitka/Electron is justified. [20] |

### 2.7 Go DSP: reference-level fit and major gaps

| Workload | Verified facts and consequences |
|---|---|
| Bounded biquad fitting | **[V-local]** `autoeq/frequency_response.py:465–495` uses bounded log-frequency/log-Q/gain residual fitting via `least_squares`, without overriding method. **[V]** SciPy 1.16 defaults to TRF, finite-difference Jacobian and `ftol=xtol=gtol=1e-8`. Gonum v0.17 documents generic optimizers, not this bounded residual-vector TRF interface; bounds issue #1725 remains open. **[I]** A reparameterized L-BFGS or Nelder–Mead fit is a changed algorithm, not a substitute with known parity. [31–32] |
| Minimum-phase EQ FIR | **[V-local]** At lines 651–680, current code doubles dB gain, designs with `firwin2` and calls `minimum_phase(ir,n_fft=len(ir))`. **[V]** Homomorphic half-length conversion has particular magnitude/length conventions. Gonum's documented DSP packages have no equivalent high-level replacement. **[I]** Port design, log-floor, normalization, cepstral window and truncation together; “we have FFT” does not finish this feature. [33–34] |
| Resampling | **[V-local]** `core/impulse_response.py:123` calls NNResample. **[V]** Its null-on-Nyquist filter design has documented `fc='nn'`, `beta=5`, `N=32001`; ordinary default `resample_poly` is not identical. **[I]** Verify coefficients, rational reduction, boundary treatment, delay trimming and exact length. A pure-Go Kaiser/polyphase library exists, but compatibility is unproven. [35–36] |
| Cubic log-frequency interpolation | **[V]** Gonum has `NotAKnotCubic` and `NaturalCubic`; “Go lacks cubic splines” is false. Gonum v0.17 `PiecewiseCubic` returns endpoints outside the knots, whereas SciPy spline `ext=0` extrapolates. **[V-local]** The requested fitting section specifically uses `k=1` at lines 373 and 415. **[I]** Preserve each caller's interpolation order, log10 coordinates and extrapolation policy. [37–38] |
| FFT and normalization | **[V]** Gonum Fourier transforms are unnormalized as a pair and hold mutable scratch. **[I]** Give each worker its own plan/scratch and scale inverse transforms explicitly. Float64 throughout does not mean NumPy-identical arithmetic. [39] |
| Remaining SciPy surface | **[I]** Treat SOS filtering, Butterworth design, FIR windows, smoothing, peak selection, correlation lags and boundary handling as explicit port tasks. Gonum is not an audited drop-in collection for the entire R2 list. A separate primitive-by-primitive numerical analysis remains necessary. |

**[I] Verdict on Go numerics:** enough foundations to build this, but too many algorithmic contracts to justify choosing Go merely for easy concurrency or deployment. Native solver/FFT/resampler dependencies can reduce implementation work but recreate native packaging and ABI responsibilities.

### 2.8 Audio, parallelism, GC and performance

**[V-local]** `core/recorder.py` starts blocking capture separately, performs blocking playback, then joins capture. Preserve the two-stream contract and recover alignment by sweep detection; a low-latency callback or forced duplex design is unnecessary.

**[V]** `gordonklaus/portaudio`, observed pseudo-version `v0.0.0-20260203164431-765aa7dfa631`, exposes host/device enumeration, maximum channels, independent stream parameters, float32 buffers and blocking reads/writes. MME, DirectSound, WASAPI, ASIO, CoreAudio, ALSA and JACK identifiers exist. **[U]** A representable 16-output/2-input configuration is not proof every device/host/rate can open it. [40]

**[V/I] Concrete Linux host gap:** PortAudio v19.7.0 lacks a native PulseAudio host ID; current master adds ID 16. The inspected Go wrapper has no corresponding constant and its unchecked host-name table ends at ID 14. A wrapper update is necessary before confidently promising native PulseAudio enumeration; an ALSA device named `pulse` is not a native PulseAudio host. [40–41]

**[V]** PortAudio documents same-host requirements within duplex streams, ASIO multi-device restrictions and the portability assumption of one simultaneous stream per device. **[U]** Two separate streams on the same interface, especially ASIO, require hardware validation. The binding cannot override backend restrictions. [41]

**[I] Worker model:** bounded goroutines per speaker with worker-owned FFT plans, coefficients and scratch; deterministic result collection by speaker order. Limit concurrency by CPU and estimated working memory, not speaker count alone. Avoid nested native-library thread pools. Check `context.Context` between stages, chunks and optimizer evaluations; cancellation cannot magically interrupt an arbitrary blocking C call.

**[V/I] GC:** float64 backing arrays have no element pointers, so GC need not pointer-scan every sample, but arrays still consume live heap and cause allocation pressure. A 60-second × 48 kHz × 16-channel float64 buffer is **368,640,000 bytes, approximately 352 MiB** before copies/complex FFT buffers. `GOMEMLIMIT` is soft and excludes C allocations. Reuse buffers and measure total process/native memory. Go 1.26's default Green Tea GC and experimental SIMD do not establish this pipeline's speed. [42–43]

| Published benchmark, fastest fetched entries | Go | Rust | C++ | What it establishes |
|---|---:|---:|---:|---|
| Benchmarks Game n-body | 6.39 s | 2.19 s | 2.15 s | **[V]** Particular implementations/toolchains; fast Rust/C++ use explicit vector techniques. |
| Benchmarks Game spectral-norm | 1.43 s | 0.72 s | 0.72 s | **[V]** Particular parallel implementations, not sequential language throughput. |

**[V]** The inspected n-body metadata names Go 1.23.1, rustc 1.84.1 and GCC 14.2, measured in early 2025. **[I]** These benchmarks reject a universal “compiled Go equals Rust/C++ performance” claim; they do not justify saying this audio pipeline will run two or three times slower. **[U]** No published apples-to-apples Impulcifer/FFT/TRF workload comparison was found. Benchmark deconvolution, fitting, resampling and full BRIR generation on the target machine. [44–45]

### 2.9 PyPI continuity and outputs

**[V]** Go supports `-buildmode=c-shared` and exported C functions. Python `ctypes`/CFFI can call them; go-enry provides a real Go shared library bundled in Python wheels through CFFI. [46–48]

**[I] Honest assessment:** feasible, but a second packaging surface. Create an optional dedicated shared-library target without Wails imports; use pointer+length buffers, fixed-width scalar fields/status codes and explicit allocator/free ownership. Do not expose Go slice/string layouts or return unpinned Go-heap pointers as a public ABI. Python-owned contiguous NumPy buffers must remain alive while native code uses them. Release the GIL during synchronous native work; poll progress through handles rather than frequent cross-language callbacks. Build wheels per OS/architecture, account for native libraries and Linux compatibility, and test import/use in clean environments. Existing Python-level classes are not preserved by shipping a C ABI alone.

**[V]** Gonum/plot v0.17 supports PNG/SVG/PDF and other static formats. **[I]** Static FR/IR/decay/heatmap reports are plausible, but spectrogram/waterfall styling and nine-language font coverage need deliberate work. Interactive linked ILD/IPD/IACC views need a browser chart implementation or retained report generator; Gonum/plot is not Bokeh. [49]

**[I]** Preserve CSV/TXT and WAV as core outputs. Keep channel layout/order, float32 WAV sample format and metadata explicit; prove 32-track write/read round trips with an independent reader. Static PNG export must work from the headless CLI without launching Wails. Avoid introducing a headless Chromium dependency solely to render PNGs if eliminating packaging weight is the motivation.

### 2.10 Real applications and reported pain

**[V] These are real repositories, not all equally mature production references.** Their fetched dependencies describe snapshots, not promises of present compatibility.

| App | Relevant complexity | Evidence limits |
|---|---|---|
| Tiny RDM | Redis cluster/Sentinel, SSH/SSL, large-key pagination, multiple data editors, CLI, monitoring and import/export. Fetched Go 1.25/Wails v2.13 dependency. [50] | **[I]** Strong evidence for Go-backed desktop service/UI work; no evidence of this scientific DSP workload. |
| Clustta | Creative-asset versioning, chunking/compression, dependency retrieval, synchronization and permissions. Fetched Wails v3 alpha.56 dependency. [51] | **[V]** Maintainers describe Electron → Tauri → Wails for shared Go client/server language. **[I]** Not a benchmark demonstrating Rust or Tauri failure. |
| Modal File Manager | Dual panes, keyboard modes, JS extensions, watchers, themes and subprocesses. Fetched Wails v2.12 dependency. [52] | **[V]** README explicitly notes beta/unfinished Windows/Linux aspects. Do not present as proof of mature three-platform delivery. |
| 0xPlay | Audio library, waveform, tempo/key analysis and DJ transitions; Wails v2.12 snapshot. [53] | **[V]** Audio/DSP is C++ through cgo, not pure Go. **[U]** Release/test/hardware maturity was not established; relevant architecture example, not a validated numerical-port precedent. |

**[V] Reported pain:** Gonum bounds issue #1725 is directly relevant; Gonum eigenvalue performance issue #511 is an old Go 1.10-era report, not a current performance ratio. PortAudio Go issue #26 records a historical Windows/MSYS2 `-mthreads`/pkg-config build problem; it is closed. PortAudio #680 reports blocking-stream underruns involving multiple opens through ALSA/PulseAudio; it does not demonstrate Go GC caused them. [32,54]

**[I] Bottom line:** Wails is used for substantial desktop applications. This evidence does not establish a mature pure-Go equivalent of Impulcifer's entire numerical and reporting stack.

## 3. Proposed architecture

Everything in this section is **[I] a proposed design**, not a claim that it has been implemented or benchmarked.

### 3.1 Modules and dependency boundaries

```text
cmd/impulcifer/              CLI; no Wails imports or display requirement
cmd/impulcifer-desktop/      Wails v3 lifecycle, windows, asset embedding
cmd/impulcifer-shared/       optional c-shared exports for Python
internal/appservice/         UI DTO validation, response envelopes, bootstrap
internal/jobs/               job ownership, cursor log, snapshots, cancellation
internal/platform/          dialogs, open path/URL, native theme, system info
internal/update/            one install-owner adapter; Velopack first on Windows
internal/audio/             audio interfaces + owned/pinned PortAudio wrapper
internal/report/            static plotting, CSV/TXT, self-contained HTML
internal/ffmpeg/             download/verify/execute ffmpeg and ffprobe
pkg/dsp/                    float64 primitives and numerical contracts
pkg/pipeline/               canonical config/defaults and stage orchestration
pkg/formats/                WAV and consumer-specific channel/layout export
webview_ui/                 existing HTML/CSS/JS + transport adapter
webview_ui/bindings/         pinned generated JS/JSDoc and bundled runtime
assets/locales/             existing nine catalogues, schema/placeholder checks
```

Keep the frozen CTk implementation outside this rewrite; do not redirect it through the JSON service. The new CLI calls the core directly. Native GUI calls and application lifetime remain owned by the Wails entrypoint, never imported by the shared library or headless CLI.

### 3.2 Jobs, ownership and cancellation

- Keep `StartRecording`, `StartBRIR`, `StartOutputRecovery`, `PollJob(jobID, afterSeq)` and `CancelJob(jobID)` as short service calls returning the current envelope shape.
- One active recording/processing job initially, matching the service at `application/impulcifer_service.py:878–912`. Job manager owns a mutex-protected snapshot, monotonic sequence, bounded event ring and cancel function. Terminal results remain available with bounded retention.
- Retain `running`, `cancel_requested`, `succeeded`, `failed`, `cancelled`; cancellation request is not terminal success. Record jobs remain noncancellable until safe stream abort/cleanup is proven.
- Preserve complete log/stage keys. Add an explicit retention-gap indicator when a cursor predates the oldest retained event; the present code trims events but does not expose that gap.
- First port uses existing 250 ms polling. Optional Wails event `job:changed` carries `{job_id,next_seq}` as a wake-up hint; coalesce it and fetch the authoritative delta. Register before bootstrap, deduplicate by sequence, retain a slow poll fallback and unsubscribe on teardown.
- Wails source serializes frontend event transport through a mailbox, while Go listeners run asynchronously. Neither establishes durable replay after reload; application sequence state is still needed. [55]
- Never hold the job lock across DSP, C I/O or frontend notification. Catch job-level Go panics for a failed snapshot where possible; C crashes still terminate an in-process application.
- Block update application while recording or writing outputs. Make shutdown wait for safe cleanup or explicitly confirm abort; no unbounded goroutine spawned per sample/channel event.

### 3.3 IPC, arrays and plots

- Preserve snake_case JSON tags and `{ok,data,error}` in a handwritten adapter around generated methods. Normalize bridge promise rejection to the existing error envelope.
- Keep float64 DSP arrays in Go. Send scalar job metadata and decimated plot summaries over method/event JSON, not complete recordings.
- For interactive plot datasets, use an application-scoped asset HTTP handler to return little-endian float32 arrays with schema, shape, units and version metadata; the frontend uses `ArrayBuffer`/`Float32Array`. The handler facility is documented, but binary streaming throughput on each native WebView requires a spike. [12]
- Returning Go `[]byte` through JSON produces base64; `[]float32`/`[]float64` produces JSON number arrays, not zero-copy typed arrays. NaN/Infinity need explicit handling. [56]
- Give plots opaque job/output IDs, not arbitrary filesystem paths. Deny traversal, remote origins and access outside the allowed output root. Limit response sizes and cancel abandoned plot fetches.
- Preserve the existing report-to-disk workflow first. Static PNGs use headless Go rendering; interactive HTML contains local/vendored JS assets and exported data, without a CDN or runtime npm. Chart-library selection remains separate from Wails selection.

### 3.4 Numerical parity and go/no-go gates

1. Freeze the reference Python commit, dependencies, configuration and input fixtures. Include demo default/headphone compensation, virtual bass, 44.1/48/96 kHz conversions and multi-speaker layouts.
2. Export stage goldens: sweep waveform, detected offsets, deconvolved IRs, cropped/gated IRs, room/headphone responses, fitted filter response, minimum-phase FIR, resampled IRs, normalization and final float WAV.
3. Require exact sample counts, channel order, units, filenames/layout mappings and discrete decisions where the reference is unambiguous. Compare numerical arrays using stage-specific absolute/relative tolerances and signal-relative error, not one global epsilon.
4. For bounded fits, compare bounds feasibility, residual norm and resulting frequency/phase response. Different filter parameters can generate equivalent responses; identical parameters are not the acceptance criterion. Include difficult low-Q/high-Q, narrow-band and boundary optima.
5. Suggested initial final-response gate: absolute magnitude difference no more than 0.01 dB in an agreed audible band where reference energy exceeds a defined floor; separately constrain timing, phase/ITD, ILD and tail-energy/decay differences. These are **provisional engineering thresholds**, not a claim that 0.01 dB proves perceptual equivalence. Define phase unwrapping, null handling and delay tolerances before measuring.
6. Do not shift outputs to obtain a favorable comparison when ITD/alignment is the feature being tested. Report both sample-domain and response-domain differences, including silent/null bins under a separately defined rule.
7. During migration, replace the cross-language SHA-256 equality gate with these independent numerical contracts. Keep exact hashes within each pinned same-machine implementation where deterministic arithmetic warrants them; never silently bless a new golden after a failure.
8. Run real Windows R1 trials first: host API grouping, 16 float32 outputs and two independent inputs at all required rates, playback drain, overflow/underflow, failure cleanup, disconnection and same-device/ASIO restrictions. Then test CoreAudio and supported Linux hosts.
9. Packaged acceptance must include a machine without WebView2, offline startup policy, native audio library loading, first-use microphone permissions, Unicode paths, runtime theme switching, unsigned/corrupted updates, interrupted download/apply, restart and uninstall.

**Go/no-go:** if the bounded fit/minimum-phase/resampling spike needs an extensive owned numerical library and the maintainer does not want that maintenance obligation, reject a full-Go DSP rewrite. A Wails shell around an independently packaged engine remains possible, but retaining Python also retains its distribution burden and is not the same proposal as eliminating Nuitka/Python.

## 4. Risks ranked

| Rank | Risk | Severity | Why / required mitigation |
|---:|---|---|---|
| 1 | Scientific correctness and algorithm substitutions | **Critical** | **[V/I]** No drop-in bounded TRF; NNResample and minimum-phase behavior are specific. Make numerical spike and per-stage goldens prerequisites, not post-rewrite cleanup. |
| 2 | Two-stream multichannel audio and native build matrix | **High** | **[V/U]** Representable API does not establish driver capability; ASIO/same-device constraints and PulseAudio wrapper gap are concrete. Pin PortAudio and validate actual devices/builds. |
| 3 | Upgrade continuity and authentication | **High** | **[V/U]** Built-in v3 updater exists, but Velopack migration, mandatory signatures and whole-product atomicity are not settled. Retain one install owner; test hostile/interrupted updates. |
| 4 | Prerelease framework / documentation drift | **High for immediate release; medium for prototype** | **[V/I]** beta.17 is active and still fixing crashes; mixed v2/alpha/beta examples can produce incorrect generated code. Pin everything and test installed builds. |
| 5 | Reporting replacement | **Medium–high** | **[V/I]** Gonum/plot covers static output, not Bokeh interaction or automatic matplotlib feature equivalence. Make representative waterfall/spectrogram/i18n report prototypes early. |
| 6 | Native packaging has not disappeared | **Medium–high** | **[V/I]** cgo, SDKs, signing, WebViews and audio libraries remain. Native runners and linked-library inspection are still necessary. |
| 7 | CPU/memory regression | **Medium** | **[V/U]** Large slice/FFT working sets and solver allocations can dominate. Historical microbenchmarks are not product evidence. Measure peak RSS/native memory and per-stage throughput. |
| 8 | Optional PyPI ABI/wheels | **Medium if retained** | **[V/I]** Technically established pattern, but requires owned ABI, wheel matrix and lifetime tests. Keep GUI/native shell out of this target. |
| 9 | Live native title-bar override | **Low–medium** | **[V/U]** Startup/system theme support verified; public independent runtime setter not found in inspected source. Small OS adapter may be necessary. |
| 10 | Vanilla JS bridge migration | **Low** | **[V/I]** Centralized facade and polling make it manageable. Test readiness, null/omission, errors and reload recovery; avoid unnecessary renderer changes. |

## 5. Open questions you could not settle

- **[U]** Exact newest v2 release through an authoritative latest-release API; v2.15.0 is verified as the newest release found, not an exhaustive tag audit.
- **[U]** Whether all GA blockers will be fixed by the planned September 15 milestone. Current beta must be judged as beta.
- **[U]** Native Windows forced light/dark title-bar switching at runtime through a supported v3 public API, independently of OS theme.
- **[U]** Actual no-Vite watcher/reload configuration on all three packaged backends; JS/runtime generation is documented but this integration was not executed.
- **[U]** Binary plot streaming throughput, response-size constraints and complete memory-copy behavior across WebView2/WKWebView/WebKitGTK.
- **[U]** Safe mandatory-signature configuration and full-product replacement behavior of v3 updater, including extra native libraries/AppImage and migration from Velopack installations.
- **[U]** Community Velopack binding library-version provenance and Windows arm64 link correctness; official long-term Go SDK delivery.
- **[U]** Every R1 device/host/sample-rate combination, especially independent streams on ASIO and direct PulseAudio support in a selected PortAudio build.
- **[U]** TRF replacement choice and achieved tolerances for fitted EQ, minimum phase, NNResample and spline boundaries.
- **[U]** Product-level Go/Rust/C++ throughput, binary size, startup time and peak native/Go memory; published microbenchmarks cannot settle these.
- **[U]** Exact static plot feature/font parity and a chosen standalone interactive chart library.
- **[U]** The maintenance cost of platform wheels relative to dropping PyPI, including whether users depend on Python classes beyond the CLI.

## 6. Sources

All sources observed 2026-09-07. Repository-relative line references above are local inspection evidence; external URLs below support third-party claims. Mutable `master` documents may change after observation.

1. Wails beta.17 release API, exact prerelease/date: https://api.github.com/repos/wailsapp/wails/releases/tags/v3.0.0-beta.17 ; release notes: https://github.com/wailsapp/wails/releases/tag/v3.0.0-beta.17
2. Official v3 changelog: https://v3.wails.io/changelog/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/changelog.mdx
3. Beta announcement and status: https://v3.wails.io/blog/wails-v3-beta/ ; https://v3.wails.io/status/ ; fetched status: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/status.mdx
4. v2.15.0 release: https://github.com/wailsapp/wails/releases/tag/v2.15.0
5. Official v2 changelog (date/entry corroborated through WebSearch): https://v2.wails.io/changelog/
6. Advanced binding/runtime bundling: https://v3.wails.io/features/bindings/advanced/ ; pinned source: https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/docs/src/content/docs/features/bindings/advanced.mdx
7. Method/model bindings: https://v3.wails.io/features/bindings/methods/ ; https://v3.wails.io/features/bindings/models/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/features/bindings/methods.mdx
8. Frontend runtime/no-bundler usage: https://v3.wails.io/reference/frontend-runtime/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/reference/frontend-runtime.mdx
9. Vanilla/Vite templates: https://v3.wails.io/quick-start/first-app/
10. Release milestones: https://github.com/wailsapp/wails/milestones
11. v2-to-v3 migration: https://v3.wails.io/migration/v2-to-v3/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/migration/v2-to-v3.mdx
12. Build system and asset handler: https://v3.wails.io/concepts/build-system/ ; https://v3.wails.io/contributing/asset-server/ ; fetched asset source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/contributing/asset-server.mdx
13. TypeScript checking/no emit: https://www.typescriptlang.org/tsconfig/checkJs.html ; https://www.typescriptlang.org/tsconfig/noEmit.html
14. Custom build/dev loop: https://v3.wails.io/guides/build/customization/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/guides/build/customization.mdx
15. Installation/version requirements: https://v3.wails.io/quick-start/installation/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/quick-start/installation.mdx
16. Linux packaging/GTK/NVIDIA caveats: https://v3.wails.io/guides/build/linux/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/guides/build/linux.mdx
17. Wails Linux signal-handler report: https://github.com/wailsapp/wails/issues/5506
18. Dialog and native theme references: https://v3.wails.io/tutorials/03-notes-vanilla/ ; https://v3.wails.io/features/windows/options/ ; pinned Windows source: https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/v3/pkg/application/webview_window_windows.go ; public window source: https://raw.githubusercontent.com/wailsapp/wails/master/v3/pkg/application/webview_window.go
19. Built-in updater tutorial, pinned beta.17 source: https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/docs/src/content/docs/tutorials/04-self-update-a-wails-app.mdx ; public page: https://v3.wails.io/tutorials/04-self-update-a-wails-app/
20. Wails FAQ, built-in updater and size estimate: https://v3.wails.io/faq/ ; fetched source: https://raw.githubusercontent.com/wailsapp/wails/master/docs/src/content/docs/faq.mdx
21. Velopack language matrix: https://docs.velopack.io/ ; release API: https://api.github.com/repos/velopack/velopack/releases/latest
22. Official Velopack C/C++ API: https://docs.velopack.io/getting-started/cpp
23. Community Go binding: https://github.com/quaadgras/velopack-go ; Windows linker file: https://raw.githubusercontent.com/quaadgras/velopack-go/release/velopack/ldflags_windows.go ; native libraries: https://github.com/quaadgras/velopack-go/tree/release/binaries
24. Velopack apply CLI and integration/hooks: https://docs.velopack.io/reference/cli/content/update-windows ; https://docs.velopack.io/integrating/overview ; https://docs.velopack.io/integrating/hooks
25. creativeprojects/go-selfupdate: https://pkg.go.dev/github.com/creativeprojects/go-selfupdate ; pinned validators: https://raw.githubusercontent.com/creativeprojects/go-selfupdate/v1.6.0/validate.go
26. minio/selfupdate: https://pkg.go.dev/github.com/minio/selfupdate ; pinned apply: https://raw.githubusercontent.com/minio/selfupdate/v0.6.0/apply.go ; minisign: https://raw.githubusercontent.com/minio/selfupdate/v0.6.0/minisign.go
27. Velopack code signing: https://docs.velopack.io/packaging/signing
28. Wails v2 Windows runtime options, pinned v2.13: https://raw.githubusercontent.com/wailsapp/wails/v2.13.0/website/docs/guides/windows.mdx ; CLI: https://v2.wails.io/docs/reference/cli/
29. Wails v3 packaging: https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/docs/src/content/docs/guides/build/windows.mdx ; https://v3.wails.io/guides/build/macos/ ; https://v3.wails.io/guides/build/linux/
30. Wails pinned cross-compilation guide: https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/docs/src/content/docs/guides/build/cross-platform.mdx
31. SciPy 1.16 least_squares: https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.optimize.least_squares.html
32. Gonum v0.17 optimize and bounds issue: https://pkg.go.dev/gonum.org/v1/gonum@v0.17.0/optimize ; https://github.com/gonum/gonum/issues/1725
33. SciPy 1.16 minimum_phase: https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.signal.minimum_phase.html
34. Gonum DSP package index: https://pkg.go.dev/gonum.org/v1/gonum/dsp
35. NNResample actual filter: https://github.com/jthiem/nnresample ; SciPy baseline: https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.signal.resample_poly.html
36. Existing pure-Go resampler, unverified compatibility: https://github.com/tphakala/go-audio-resampler
37. Gonum cubic behavior, pinned source: https://raw.githubusercontent.com/gonum/gonum/v0.17.0/interp/cubic.go
38. SciPy interpolation/extrapolation: https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.interpolate.InterpolatedUnivariateSpline.html
39. Gonum FFT normalization/scratch, pinned source: https://raw.githubusercontent.com/gonum/gonum/v0.17.0/dsp/fourier/fourier.go
40. Go PortAudio binding: https://pkg.go.dev/github.com/gordonklaus/portaudio ; inspected snapshot: https://raw.githubusercontent.com/gordonklaus/portaudio/765aa7dfa631/portaudio.go
41. PortAudio API/host IDs: https://portaudio.com/docs/v19-doxydocs/api_overview.html ; https://raw.githubusercontent.com/PortAudio/portaudio/v19.7.0/include/portaudio.h ; https://raw.githubusercontent.com/PortAudio/portaudio/master/include/portaudio.h
42. Go GC guide: https://go.dev/doc/gc-guide
43. Go runtime releases: https://go.dev/doc/go1.26 ; https://go.dev/doc/go1.25
44. Benchmarks Game n-body results and implementation/compiler metadata: https://benchmarksgame-team.pages.debian.net/benchmarksgame/performance/nbody.html ; https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-go-3.html ; https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-rust-9.html ; https://benchmarksgame-team.pages.debian.net/benchmarksgame/program/nbody-gpp-0.html
45. Benchmarks Game spectral-norm: https://benchmarksgame-team.pages.debian.net/benchmarksgame/performance/spectralnorm.html
46. Go build modes/cgo pointers: https://raw.githubusercontent.com/golang/go/go1.25.0/src/cmd/go/internal/help/helpdoc.go ; https://pkg.go.dev/cmd/cgo
47. Python ctypes/GIL and ABI: https://docs.python.org/3/library/ctypes.html
48. Real Go+CFFI Python wheel example: https://raw.githubusercontent.com/go-enry/go-enry/master/python/README.md
49. Gonum/plot static formats: https://pkg.go.dev/gonum.org/v1/plot
50. Tiny RDM: https://github.com/tiny-craft/tiny-rdm
51. Clustta: https://github.com/eaxum/clustta-client
52. Modal File Manager: https://github.com/raguay/ModalFileManager
53. 0xPlay: https://github.com/T58574/0xPlay
54. Reported numerical/audio build problems: https://github.com/gonum/gonum/issues/511 ; https://github.com/gordonklaus/portaudio/issues/26 ; https://github.com/PortAudio/portaudio/issues/680
55. Wails event semantics/source: https://v3.wails.io/features/events/system/ ; https://raw.githubusercontent.com/wailsapp/wails/v3.0.0-beta.17/v3/pkg/application/events.go
56. Go JSON slice/NaN behavior: https://pkg.go.dev/encoding/json
