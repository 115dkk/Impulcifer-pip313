# Impulcifer rewrite research: S1, Rust + Tauri 2

Research cutoff and access date: **2026-09-07**. Repository inspected read-only at `E:/Impulcifer`; no application files changed. Labels apply throughout: **[V] verified** by the cited source or inspected code; **[I] inference/proposal**, not measured or guaranteed; **[U] unknown/unverified**. A verified issue report establishes that someone reported the stated reproduction, not that every current installation has that defect. Mutable documentation and manifests describe the inspected snapshot; release timestamps are stated separately where verified. Numbered references resolve to URLs in §6.

## 1 Verdict

1. **[I] Recommend Rust + Tauri 2 as the target shell/core architecture, conditional on DSP and multichannel-audio acceptance gates. Do not authorize a big-bang rewrite.**
2. **[V] The existing vanilla frontend can remain bundler-less; its bridge and cursor-based job model map directly to Tauri commands.** [1–4, L1–L3]
3. **[I] The important saving is removing Python/Nuitka deployment machinery, not changing browser engines: this app already uses the same three engine families.** [L2]
4. **[V] Tauri is not a Rust substitute for SciPy. CPAL also does not list required Windows MME/DirectSound backends.** [24–27]
5. **[I] Keep a native PortAudio abstraction and prove two-stream operation on actual target hardware before committing to R1 coverage.** [25–26]
6. **[I] Keep Velopack on Windows for the initial Rust release; changing the shell does not require an updater migration.** [35–39]
7. **[I] Use an opaque, normally decorated window, native dialogs, small JSON job messages, and demand-loaded binary plot data.**
8. **[I] Ship Linux AppImage on a published best-effort matrix; community-maintain Flatpak/source packages rather than promise universal compatibility.** [17–23]
9. **[I] Keep PyPI through a GUI-independent PyO3/maturin binding, accepting native wheels per OS/architecture and extra free-threaded wheels.** [41–44]
10. **[U] No defensible Impulcifer-specific Rust build time, final package size, peak RAM, or DSP speedup exists without a representative prototype.**

## 2 Findings (with sources)

### 2.1 What the current code actually requires from a replacement shell

**[V] Read the complete `webview_ui/app.js`, lines 1–1488, and complete `impulcifer_webview.py`, lines 1–340.** Also inspected the recorder, optimizer, job service, release-pipeline entry, and ADR 0001. Local evidence is listed under L1–L7.

| Status | Inspected behavior | Migration consequence |
|---|---|---|
| [V] | `app.js:58` centralizes calls through `api() => window.pywebview.api`. Calls pass positional arguments and await promises. | A named adapter can preserve virtually all request construction and rendering. Tauri `invoke` takes named argument objects, not the same positional arguments. [1] |
| [V] | `app.js:542–603` handles pre-start warnings, confirmation, `{ok,data/error}` envelopes and starting one active job. | Preserve the envelope initially. Rust `Result` normally rejects the JS promise on error; changing to that without an adapter would break branches expecting `response.ok`. [1] |
| [V] | `app.js:617–674` calls `poll_job(jobId,nextSeq)`, consumes progress/log/status events, and polls again after 250 ms. Three terminal states are handled explicitly. | Polling is already adequate for jobs lasting seconds/minutes. Channels are optional, not required for correctness. |
| [V] | Updates use a separate cursor and 250 ms poller at `app.js:787–821`; bootstrap resumes active jobs at `1386–1460`. | An event-only rewrite must not lose reload recovery or terminal results. Retain the job registry and cursor-replay API even if adding push. |
| [V] | Service polling returns events with `seq > after_seq`; event storage is bounded by `_MAX_JOB_EVENTS` (`application/impulcifer_service.py:519–553,965–977`). | Preserve cursor meaning. Add explicit retention-gap metadata rather than pretending channels provide replay. |
| [V] | Two further pywebview dependencies are the readiness condition at `app.js:933` and startup event/guard at `1486–1487`. | Replacing only line 58 is insufficient: replace readiness checks and boot trigger too. |
| [V] | Confirmation/alert calls appear at `app.js:551,584,1009,1014,1028,1035`; theme and strings are already runtime data. | Explicitly use asynchronous native dialog functions. Do not globally replace a synchronous boolean-returning `window.confirm` with a Promise. [12] |
| [V] | `app.js:1216–1244` renders Python/GIL/install-kind information; frontend selection persists CTk/WebView at `1370–1373,1421`. | Update these fields and labels for Rust. A bridge adapter alone cannot make the information page truthful. Frozen CTk is not silently converted to a Rust/Tauri view. |
| [V] | Host maps Windows→EdgeChromium, macOS→Cocoa, Linux→GTK (`impulcifer_webview.py:24–41`). | Tauri keeps WebView2/WKWebView/WebKitGTK; it does not fix upstream WebKit behavior by changing language. [L2,17–23] |
| [V] | Host has native file filters, URL-name allowlisting, and a DWM dark-titlebar workaround with retries (`17–22,76–128,193–213,265–294`). | Use Tauri/native plugin equivalents, retain the URL allowlist, and test live theme changes. Native theme APIs should replace handwritten DWM retries unless a reproduced defect requires otherwise. [11–14] |
| [V] | ADR 0001 rejects forcing CTk through the WebView JSON service. | This proposal concerns the new Rust application; it does not propose retrofitting the frozen CTk application through Tauri IPC. [L7] |

### 2.2 Versions, cadence, WRY/tao and plugin maturity

| Component | Verified snapshot | Interpretation |
|---|---|---|
| **Tauri core** | **2.11.5**, published **2026-07-01 13:56:43 UTC**, returned by GitHub's latest-release endpoint. [5] | [V] Latest verified core release at this access. Core, CLI, API and plugins have separate version sequences. |
| **Tauri CLI** | **2.11.4**, published **2026-06-28**. Core 2.11.4 followed June 30 and 2.11.5 July 1. [6] | [V] Recent patches were days apart, followed by a quieter core-release interval. [I] There is no basis for promising a monthly release SLA. |
| **JS API** | `@tauri-apps/api` **2.11.1** in the core 2.11.5 release tag. [7] | [V] Do not force matching patch numbers across separately versioned packages. |
| **WRY upstream** | **0.56.1**, **2026-08-13**. Includes Windows teardown/focus fixes, IPC hardening and an older-macOS debug startup fix. [8] | [V] Actively maintained; still a 0.x dependency with platform-specific churn. |
| **tao upstream** | **0.37.0**, **2026-08-21**. Raises Rust floor to 1.85 and removes Windows 7 compatibility through updated Windows bindings; includes Windows lifecycle fixes. [9] | [V] Do not repeat old Windows-7 support claims when describing the newest dependency family. Windows 10/11 are this project's actual targets. |
| **Tauri 2.11.5 runtime dependencies** | Declares WRY `0.55.0` and tao `0.35.0` compatibility requirements, not exact pins. [10] | [V] Upstream latest WRY/tao are **not automatically the versions used by released Tauri**; Cargo's 0.x minor compatibility boundaries matter. Check the resolved lockfile. |

**[V] Useful maintenance evidence:** core 2.11.4 restricted `time` after a dependency build break; 2.11.5 removed the restriction after `time 0.3.53`. Its runtime patch fixed a Windows HDC leak and undecorated resizing. Rust does not abolish dependency drift. [5–6]

| Plugin/API | Version observed on 2026-09-07 | Required use and assessment |
|---|---:|---|
| `tauri-plugin-dialog` | 2.7.3 | [V] Official file/folder/save/message/ask/confirm APIs. [I] Required; standard fit for current filters and prompts. [12–13] |
| `tauri-plugin-opener` | 2.5.5 | [V] Official open URL/path and reveal-in-directory APIs. [I] Prefer it for opening over broad shell access. [14] |
| `tauri-plugin-fs` | 2.5.2 | [V] Official filesystem plugin exists. [I] **Not required** if all project/audio reads and writes remain Rust commands; avoid granting frontend-wide file access unnecessarily. [13,15] |
| `tauri-plugin-updater` | **2.11.0**, published **2026-08-31** | [V] Stable 2.x package and current maintenance. [I] Use on Tauri-managed macOS/AppImage distributions, not on top of the same Windows installation managed by Velopack. [33–34] |
| `tauri-plugin-process` | 2.3.1 | [V] App exit/relaunch; not general subprocess spawning. Optional if Rust handles lifecycle. [13,16] |
| `tauri-plugin-os` | 2.3.2 | [V] OS type/version/platform/architecture/hostname/locale, not a full hardware inventory. [I] Rust should additionally report worker count, audio backends, build/runtime versions. [13] |
| `tauri-plugin-shell` | 2.3.6 | [V] Subprocess and sidecar mechanisms. [I] Needed only if using that plugin rather than Rust-owned process launching. [4,13] |
| Window theme API | Core 2.11.5 `set_theme` | [V] Native API exists; macOS/Linux changes affect the whole application. [I] No separate dark-titlebar plugin is necessary. Test dark/light/system changes before and after first paint. [11] |

**[I] Maturity judgment:** routine desktop host plumbing is sufficiently documented and released for this project. That is not a guarantee of uniform OS behavior. **[U] Exact publication dates for the auxiliary plugin versions above were not independently established; they are observed versions, not invented release dates.** Official plugin pages still cite Rust 1.77.2 in places; use the actual resolved dependency graph and pinned compiler rather than assume that floor holds for every future update. [8–10,13]

### 2.3 Windows: WebView2 is a managed dependency, not a bundled browser by default

**[V] Microsoft is the authority on runtime presence:** Windows 11 includes Evergreen WebView2; the vast majority of Windows 10 devices have it, but a minority do not. Enterprise/offline configurations matter. Installed Edge Stable is not a substitute for the production WebView2 Runtime. Detect runtime presence and handle absence. Tauri prerequisites use looser Windows-10-preinstalled wording; do not turn it into a universal guarantee. [18–19]

| Distribution choice | Tauri documentation's approximate package increment | Offline and servicing behavior |
|---|---:|---|
| `downloadBootstrapper` | 0 MB | [V] Default bundler mode; download/bootstrap runtime if missing. Needs network when installation is necessary. |
| `embedBootstrapper` | ~1.8 MB; Microsoft calls bootstrapper ~2 MB | [V] Embeds only the downloader, **not** the runtime. Still needs network if runtime absent. |
| `offlineInstaller` | ~127 MB | [V] Embeds Evergreen standalone installer. Works without downloading runtime at install time; Evergreen remains separately serviced. |
| `fixedRuntime` schema variant | Tauri prose estimates ~180 MB; Microsoft currently says fixed binaries **over 250 MB** | [V] Ships private fixed engine; updater/security maintenance becomes yours. These figures are different documentation estimates, not a measured common compression baseline. Budget from the actual chosen archive. |
| `skip` | 0 MB | [V] No runtime installation; app cannot run if no usable runtime exists. Bad general-distribution default. |

Sources [18–19]. **[I] Recommended:** Evergreen with explicit missing-runtime detection, normal online installation, and a documented separate offline-runtime option. Do not choose fixed runtime just to avoid testing Evergreen updates: it increases the package and makes browser security releases your responsibility.

**[V] A Tauri NSIS/MSI bundler's WebView2 installation settings belong to that installer. [I] If Windows remains Velopack-packaged, deliberately implement/retain equivalent detection/bootstrap handling in that distribution; setting `webviewInstallMode` on an unused Tauri installer does not protect Velopack users.** [18,35–39]

**[V] Microsoft also documents fixed-runtime caveats:** no network/UNC deployment; newer fixed runtimes have Windows-10 unpackaged-app ACL requirements. This argues against private fixed engines for a solo maintainer unless offline operation mandates them. [19]

### 2.4 macOS: acceptable form UI, test actual WKWebView rather than Safari alone

| Status | Evidence | Decision for this app |
|---|---|---|
| [V] | Tauri documents native/transparent/custom titlebar configuration and drag regions; drag attributes do not automatically apply to children. [20] | [I] Keep native decoration initially; changing colors is enough. Do not rebuild traffic lights and resize behavior merely to match a mockup. |
| [V, reported] | Tauri #10744 (2024-08-23; macOS 14.3.1, Tauri 2.0.0-rc.6) reports ~28 px native-drop/DOM coordinate offset. Current JS docs warn about inaccurate drop coordinates with attached devtools. [21] | [I] Prefer file dialogs for the existing workflow. If adding drag/drop, test scale, titlebar mode, multiple monitors and actual packaged app. Do not bake a 28 px correction into production. |
| [V, proposed fix] | WRY PR #1723 (2026-05-05) addresses a macOS file-drag crash involving modern `public.file-url` versus legacy pasteboard types; it was open when inspected. [21] | [U] No released fix was verified. Test this scenario on the WRY version actually resolved by Tauri, not upstream HEAD. |
| [V, reported] | Tauri #15471 (2026-06-04; Tauri 2.11.2, WRY 0.55.1, Apple Silicon macOS 26.6 beta) reports ~620 mW GPU consumption with whole-window transparency versus ~75 mW without. [22] | [I] Avoid whole-window transparency. These are one reporter's beta-OS measurements, **not** an 8× general Tauri or canvas penalty. |
| [V, reported] | WebKit #316777 (2026-06-10; macOS 26.5/M2 Pro) reports differences in user-installed font selection between attached and detached/Offscreen canvases in WKWebView, including divergence from Safari. [22] | [I] Bundle licensed fonts, test all nine languages, avoid relying on offscreen font metrics being identical. |
| [U] | No controlled current Tauri/WKWebView benchmark proving unusable form rendering or 2D canvas performance was found. | [I] Do not reject the shell on gaming/120 Hz anecdotes. This app needs responsive forms and occasional analysis plots, not a real-time DAW display. |

**[I] Required macOS packaging work beyond the webview:** microphone permission descriptions and authorization, compatible deployment target, signing of bundled helper executables, file dialogs with Unicode paths, sleep/device-disconnection behavior. These are acceptance tests, not claims that the shell performs them automatically. [40]

### 2.5 Linux: what best-effort support actually means

**[V] Tauri 2 uses WebKitGTK API 4.1.** This is the GTK3/libsoup3 API line, not GTK4 and not engine version 4.1. API 4.0 used libsoup2. The transition dates to Tauri 2.0.0-alpha.3 in 2023; WebKit engine versions such as 2.46/2.48/2.50 are a separate sequence. Installing only `libwebkit2gtk-4.0-dev` does not satisfy Tauri 2. [17]

**[V] Answer to the AppImage question: normal Tauri AppImage packaging bundles WebKitGTK and associated dependencies; it is not simply an executable requiring the target's system WebKitGTK.** Official documentation describes bundling dependencies; maintainer discussion #10026 confirms the recommendation, with an actual mounted AppImage's WebKitGTK library in a failing loader trace. The failure was host GLIBC/GLIBCXX incompatibility. The injected-bundle omission in #12463 and merged fix #12466 demonstrate that WebKit packaging still has failure modes. [23]

**[V] AppImage is not a complete OS container:** build against the oldest supported distribution offering WebKitGTK 4.1; official examples are Ubuntu 22.04 and Debian 12. Newer builds can raise host glibc requirements. **[I] Bundled WebKit needs refreshing through your releases; users' system WebKit updates do not magically replace the embedded library.** GPU/driver and host integration still require real tests. [23]

| Status | Public reproduction | What it does and does not prove |
|---|---|---|
| [V, reported] | WebKit #280210: Arch, WebKitGTK 2.46.0, NVIDIA 560.35.03, Wayland protocol error 71; X11 GBM failure/blank view also reported. Follow-up on Gentoo/2.50.5 in May 2026. | Engine/driver defects can reproduce outside Tauri in MiniBrowser. Neither X11 nor AppImage is a universal cure. [22] |
| [V, reported] | Tauri #13151: Mint 22.1 **X11**, GT 730, Tauri 2.4.1/WRY 0.50.5/WebKitGTK 2.48.0; switching output to Intel reportedly worked. Closed upstream/not planned. | Not a Wayland-only problem; closed issue does not establish a shipped fix. [22] |
| [V, reported] | Tauri #14924: 2.10.2/WRY 0.54.1/WebKitGTK 2.50.4; NVIDIA 590+ and transparent/undecorated window failures. Open/needs triage when inspected. | A real report, but mixed session diagnostics leave exact reproduction details unsettled. [22] |
| [V, reported] | Tauri #15050: Fedora 43/Sway/Intel i915/2.10.3/WRY 0.54.2/WebKitGTK 2.50.5 blank window. | NVIDIA is not the only affected hardware. No verified resolution was recovered. [22] |

**[V] Compatibility toggles appear in reports:** `WEBKIT_DISABLE_DMABUF_RENDERER=1`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`, `__NV_DISABLE_EXPLICIT_SYNC=1`; some combinations help and others do not. Official Flatpak docs mention a compositing workaround for black Wayland views. **[I] Expose documented opt-in diagnostics, not unconditional GPU disabling or sandbox disabling.** [22–23]

| Distribution policy | Assessment |
|---|---|
| **AppImage** | **[I] Recommended first-party best effort.** Build on a declared baseline, test packaged launch on that baseline and one newer distro, retain CLI/tar fallback and log engine/session/GPU details. No promise for every proprietary-driver/Wayland combination. |
| **Flatpak** | **[I] Community-maintained alternative.** Runtime consistency helps distro-library variation, but portals, microphone/audio access, external ffmpeg execution, filesystem access and update ownership need packaging work. It does not cure WebKit/GPU bugs. Let Flatpak own installation updates rather than self-replacing its managed app. [23] |
| **Source/AUR** | **[I] Publish documented build requirements and core CLI.** Users maintain distribution-specific dependencies; community packagers own their dependency/update policy. |
| **Nothing** | **[I] Honest only if formally dropping desktop Linux support.** It does not meet the current three-platform delivery intent; do not call it Linux support. |

**[I] Browser media frameworks need not be bundled for native Rust playback/recording.** Tauri's `bundleMediaFramework` is for audio/video playback through the webview and adds GStreamer. Native PortAudio is a separate mechanism. Avoid paying this size/dependency cost unless introducing `<audio>`/`<video>` into the UI. [23,25]

### 2.6 IPC: use JSON for control, binary for arrays, neither as a realtime audio transport

| Mechanism | Verified behavior | Suitability |
|---|---|---|
| `#[tauri::command]` + serde | Deserialize named arguments; serialize ordinary responses; `Result::Err` rejects `invoke` promise. [1] | [I] Excellent for configs, device metadata, jobs, settings and small results. Keep the current envelope for compatibility. |
| `tauri::ipc::Response::new(Vec<u8>)` | Explicit optimized binary response rather than an ordinary serde numeric vector. Raw request bodies can also use ArrayBuffer/Uint8Array. [1] | [I] Use for plot buffers; specify dtype, endianness and shape separately. Returning `Vec<f32>` through normal serde is still a JSON array. |
| `Channel<T>` | Ordered streaming delivery, recommended over events for high-volume traffic. [2] | [I] Optional progress subscription or bounded chunk transfer. Ordered arrival does not force async JS handlers to finish in order. |
| General events | JSON payloads; convenient broadcast; not optimized for large/high-frequency data and lack command-like fine-grained event/data access controls. [2,15] | [I] Suitable for lightweight app notices, not bulk samples or an authoritative job journal. |

**[V] Important implementation qualification, pinned to Tauri 2.11.5:** channel source directly evaluates JSON below **8192 bytes** and raw buffers below **1024 bytes**; the raw small-payload path serializes a byte array and constructs `Uint8Array(...).buffer`. Larger payloads use queued retrieval. The IPC fallback in `protocol.rs` has macOS/iOS and blocked-custom-protocol cases that format responses into evaluated JS. **The public raw-buffer API is not a zero-copy or universal binary-transport guarantee.** [28]

**[V] Published benchmark, not our measurement:** `connectrpc-tauri 0.1.2` (published 2026-08-18, Tauri requirement `^2.11.5`) reports a **64 KiB raw protobuf invoke baseline of 0.340 ms** versus **0.363 ms** for its transport in a release-build M-series Mac webview. It batches measurements because WebKit timers are 1 ms. Exact chip/OS and benchmark date are unspecified. **[U] This does not establish 12 MB throughput or latency.** [29]

**[V, arithmetic]** `30 × 100,000 × 4 = 12,000,000 bytes` (**11.44 MiB**) for one float32 sample payload alone; axes, copies, decoded JS objects, chart state and retained Rust float64 data add memory. The float64 source is 24,000,000 bytes. **[I] Do not ship all tracks on every interaction.** Send visible tracks, decimated data and cached analysis tiles; request full-resolution data only on demand. Test 12 MB worst-case transfers on all three actual webviews before selecting chunk sizes. No base64 unless a measured fallback requires it.

### 2.7 Bundler-less frontend: supported, with a few real code changes

**[V] Tauri supports `build.frontendDist` as a static directory, recursively embedded into the application, with `index.html` as entry. `app.withGlobalTauri: true` exposes `window.__TAURI__`. Omitting `devUrl` allows Tauri CLI's built-in static dev server. `window.__TAURI__.core` supplies `invoke` and `Channel`; dialog has its own documented global namespace.** [3,12]

**[I] Keep `webview_ui` as ordinary local files; use Cargo-installed Tauri CLI.** No frontend framework migration, npm bundling pipeline or runtime Node installation is needed for this app. An optional JS type-check may still use Node in CI, and `tauri-action` itself is a GitHub JavaScript action; those are build-time tooling, not an application runtime requirement. [3,30]

Illustrative adapter contract, **[I] proposal, not compiled code**:

```js
const invoke = window.__TAURI__.core.invoke;
const host = {
  bootstrap: () => invoke('bootstrap'),
  list_audio_devices: hostApi => invoke('list_audio_devices', { hostApi }),
  start_brir: request => invoke('start_brir', { request }),
  poll_job: (jobId, afterSeq = 0) => invoke('poll_job', { jobId, afterSeq }),
  cancel_job: jobId => invoke('cancel_job', { jobId }),
  select_file: kind => invoke('select_file', { kind }),
  select_directory: () => invoke('select_directory'),
};
const api = () => host;
```

**[I] Make every exposed method explicit; do not install an unrestricted dynamic command proxy.** Keep request field names snake_case where already established. Tauri command argument objects default to camelCase (`jobId`, `afterSeq`); serde structs can preserve their existing JSON names. Add a single transport-error wrapper that preserves `{ok:false,error:{code,message,details,retryable}}` instead of letting silent promise rejection stop polling. [1,L1]

**[I] Required non-adapter edits:** boot/readiness conditions; Python-specific preboot text; Python/GIL/system-info rows; obsolete frontend switch; awaited confirmation/message calls; any app-update installation semantics. Keep DOM ids, locale keys where still meaningful, skin/theme logic and BRIR payload-default behavior unchanged. A browser mock `host` still supports gallery tests without Tauri.

### 2.8 Audio and DSP qualification: this is where the rewrite can fail

**[V] The current recorder starts a recording thread, performs blocking playback, then joins the recorder (`core/recorder.py:440–494`); it does not require a single duplex stream.** Host API enumeration and output-channel validation precede this. [L4]

| Topic | Finding |
|---|---|
| **CPAL** | **[V]** Current documented defaults/options include Windows WASAPI with ASIO/JACK options; macOS CoreAudio; Linux ALSA with optional JACK/PipeWire/PulseAudio. **MME and DirectSound are absent.** Callback use is documented; no equivalent blocking read/write contract was verified. [24] **[I] CPAL alone is not an R1-complete choice, regardless of its popularity in Rust audio apps.** |
| **PortAudio native API** | **[V]** Has host/device enumeration, `Pa_IsFormatSupported`, float32 buffers and blocking `Pa_ReadStream`/`Pa_WriteStream`; `Pa_StopStream` drains pending output while abort stops promptly. Supported rates depend on hardware; PortAudio does not resample unsupported rates. Concurrent-stream/backend restrictions exist; ASIO cannot generally open multiple devices at once. [25] |
| **Rust PortAudio wrapper** | **[V]** Crate 0.8.0 (2024-10-13), maintenance-focused; docs.rs build failed for that release and points to 0.7.0 docs. Its build can find an existing C library or download/build one. [26] **[I] This is not a clean, zero-native-dependency win. Pin and build the C library yourself, and own a small audited FFI layer if the wrapper's build behavior is unsuitable.** |
| **FFT baseline** | **[V]** RustFFT 6.4.1 and realfft 3.5.0 document f64 support and planned transforms. Neither normalizes automatically; inverse normalization must be explicit. RustFFT supports non-power-of-two sizes and documents efficient factors. [27] **[I] Do not silently replace SciPy `next_fast_len` with a power-of-two heuristic if changing padding changes the algorithm.** |
| **Biquad fitting** | **[V]** Existing optimizer smooths, finds peaks, merges/prunes initial filters, uses log10 frequency and log Q, box bounds and a function-evaluation budget, then calls SciPy `least_squares` without overriding its default TRF algorithm. [L5,31] **[I] A Rust Levenberg–Marquardt crate is not a drop-in substitute: unconstrained LM does not satisfy these bounds/termination semantics.** |
| **Other R2 operations** | **[U]** No single verified Rust package was established here that reproduces the complete required SciPy surface, especially firwin2/minimum_phase, spline boundary behavior, Savitzky–Golay edge modes, peak plateau rules, polyphase phase/length conventions and TRF. **[I] Budget explicit implementations or tightly scoped native dependencies and tests, rather than declaring the port solved by RustFFT + ndarray.** |
| **Multitrack WAV** | **[V]** Hound 3.5.1 represents channel counts as u16 and writes f32 samples; its writer uses extensible WAV for >2 channels or >16 bits and constructs a speaker mask from low bits, capped at 18. [32] **[I] That mask must not accidentally label 30/32 independent BRIR tracks as ordinary surround speakers. Validate headers/channel order in HeSuVi/Hangloose/EAPO/JamesDSP; use an explicit multitrack writer policy rather than assume library defaults are correct.** |

**[I] Two required prototypes before full port:** (1) simultaneous independent 16-out/2-in streams at 44.1/48/96 kHz with completion/drain/error propagation on the actual Windows interface and selected host APIs; (2) representative difficult AutoEQ fits plus the demo's deconvolution/compensation output against a pinned Python oracle. An optional ASIO backend should be advertised only for configurations actually proven to work; do not make ASIO a way to evade the independent-stream requirement.

### 2.9 Updater: retain the working Windows installation contract

| Criterion | Tauri updater | Velopack Rust |
|---|---|---|
| Verified release | **[V] 2.11.0, 2026-08-31.** [33–34] | **[V] 1.2.0, 2026-06-03.** Registry timestamp, not docs.rs build timestamp. [35] |
| Readiness | [V] Official Tauri plugin, signed update support across desktop targets. | [V] Official Rust guide marks support ready; real startup/check/download/apply SDK, not a planned .NET-only feature. [36] |
| Signatures | [V] Mandatory updater signatures; private key via `TAURI_SIGNING_PRIVATE_KEY`, optional password; public key contents embedded in app. Metadata contains actual signature contents. Loss of private key blocks updates accepted by existing clients. [33] | [V] Inspected Rust manager checks length/hash against feed, SHA-256 or SHA-1 fallback. [U] No mandatory independent Tauri-style updater public-key configuration verified. Feed consistency is not equivalent to an independently trusted publisher signature. [37] |
| GitHub hosting | [V] Static `releases/latest/download/latest.json` with platform URL/signature entries; action can generate metadata. [30,33] | [V] `GithubSource` consumes repository releases plus channel feeds such as `releases.win.json`. Schemas are different. [38] |
| Windows artifacts | [V] NSIS `.exe` or MSI `.msi` plus `.sig`; install closes the app. [33] | [V] Setup.exe or optional WiX MSI, `Update.exe`, full/delta nupkg packages and channel feed. [36,39] |
| macOS | [V] DMG for initial distribution; `.app.tar.gz` + signature for updater. [33,40] | [V] Documented installer model uses `.pkg`; not automatically a replacement for the present DMG workflow. [39] |
| Linux | [V] AppImage + signature; not generic `.deb`/`.rpm` self-updating. [33] | [V] Self-updating AppImage documented. [39] |
| Extra build tools | [V] Tauri bundler and platform installer/signing toolchains. | [V] `vpk` packaging with documented .NET SDK 8 prerequisite even though app code is Rust. [36] |

**[I] Better for this existing project:** keep **Velopack on Windows** during the rewrite, preserve pack identity, executable/shortcut expectations, feed/channel and user-settings locations. Call the Rust SDK's `VelopackApp::build().run()` before normal Tauri initialization. Package the Tauri executable/resources using Velopack; do not have two updaters competing to replace the same installation. This avoids simultaneous numerical, language, GUI-host and installer transitions. [36,L6]

**[I] For macOS/AppImage**, Tauri's built-in bundling + mandatory-signature updater is a reasonable default, since those existing distributions do not share the Windows Velopack layout. Whether to standardize every OS on Velopack later is a separate decision. A single internal `UpdateProvider` trait can preserve existing service/UI messages while implementations differ by installation kind.

**[U] No verified turnkey Velopack→Tauri-updater migration was found.** Tauri's `v1Compatible` artifacts help **Tauri v1**, not Velopack. Existing Velopack clients cannot consume Tauri `latest.json` + NSIS/MSI as ordinary nupkg updates. A later switch needs an explicit bridge release or reinstall plan, interrupted-migration testing, shortcut/uninstaller handling and settings preservation. [33,38–39]

**[I] Update installation must wait for all recorder/DSP/output-write work to finish or cancel safely.** Downloading can be separately permitted, but do not let an updater terminate a live measurement. Updater signature, Authenticode and Apple signing/notarization are three different controls. [33,37,40]

### 2.10 Build/CI, installer footprint and memory

**[V] Existing latest GitHub release at access time:** **v2.14.0**, published **2026-09-05 17:21:22 UTC**. This remote release is not assumed identical to the locally inspected checkout. Sizes below are exact release API values, compressed distribution assets, **not installed size or RAM**. [45]

| Asset | Bytes | Approx. decimal MB |
|---|---:|---:|
| `Impulcifer-win-Setup.exe` | 216,175,622 | 216.18 |
| `Impulcifer-win-Portable.zip` | 211,683,971 | 211.68 |
| `Impulcifer-2.14.0-full.nupkg` | 211,699,206 | 211.70 |
| `Impulcifer-2.14.0-macOS.dmg` | 191,023,439 | 191.02 |
| `Impulcifer-2.14.0-x86_64.AppImage` | 273,888,448 | 273.89 |
| `Impulcifer-2.14.0-linux-x86_64.tar.gz` | 273,541,383 | 273.54 |

**[I] Expected direction:** removing Python, NumPy/SciPy and Matplotlib/Bokeh deployment should reduce the app payload if replaced by Rust + local JS and modest fonts/assets. **[U] Exact final size is not known.** Retained sweep/demo assets, fonts, native libraries, duplicated CLI executable and Linux's bundled WebKit can dominate. An offline/fixed Windows browser bundle can erase much of the apparent package reduction. Do not claim a 3 MB Impulcifer based on an empty Tauri window.

**[U] No current matched Impulcifer Tauri/Nuitka RAM comparison or cargo build timing was found.** Both existing pywebview and Tauri use multiprocess OS webviews, so counting only the Rust main process is misleading. DSP f64 buffers, scratch allocations, Rayon concurrency and plots may dominate job peak RSS. [L2,27–29]

**[V] Tauri publishes a benchmark-results repository:** startup via hyperfine (3 warmups, 10 runs through DOMContentLoaded), release artifact size and peak memory; separate OS datasets cannot be compared as if runner hardware were equal. The inspected README contains no defensible current Impulcifer-equivalent numbers. [46]

| CI question | Assessment |
|---|---|
| Build on each OS? | **[V]** Windows MSI must build on Windows; NSIS cross-building from Linux/macOS is documented but less tested/discouraged. macOS uses Apple tooling; ARM AppImage cross-compilation is unsupported in current docs. [18,23,40] **[I] Use native Windows/macOS/Linux jobs, even where cross-compiling is technically possible.** |
| Cold vs warm Cargo | **[I]** Cache Cargo registries/git/target or sccache by OS, target, toolchain, feature set and lockfile. Measure cold dependency compile, incremental DSP edit, linking/LTO and packaging separately. **[U] Minutes cannot be predicted from source line count.** |
| Signing | **[V]** Tauri documents macOS certificate/signing/notarization variables and Windows configurable signer/Artifact Signing or Key Vault alternatives. Modern Windows signing is not always a PFX checked into a secret. Unsigned/ad-hoc output remains subject to OS warnings. [40] |
| tauri-action | **[V]** Supports per-OS builds, release upload and updater JSON; Cargo CLI can be selected with `tauriScript`. Its sample Node/Yarn frontend is not mandatory for this static UI. [30] **[I] Compile/verify once, publish tested artifacts; don't rebuild a subtly different app only at upload time.** |
| Existing release design | **[V]** `publish.yml` owns gate→PyPI→platform-build sequencing and the PyPI OIDC workflow/environment identity. [L6] **[I] Keep this identity and gating policy; add platform wheel build/collection before the PyPI publish job, then gate desktop builds on publish success. Do not casually rename the workflow.** |
| Verifiability | **[I]** Run Rust DSP/core tests without Tauri; JS contract/gallery tests without native audio; packaged webview smoke tests per OS; manual/hardware runs for channel mapping/recording. Cargo unit tests do not prove WebView2 provisioning or audio-device behavior. |

### 2.11 PyPI continuity

**[V] PyO3 + maturin can expose a Rust extension in a mixed Python package.** Maturin recommends a Python callable plus `[project.scripts]` for a package that needs both a native library and CLI. It also supports `bindings = "bin"` wheels for standalone Rust binaries, but that is a different distribution shape. [41–42]

**[I] Keep the existing distribution identity `impulcifer-py313` initially** and retain console entry point `impulcifer`; the requested literal `pip install impulcifer` needs separate ownership/name verification, not just a Rust setting. A small Python launcher can call `impulcifer._native.cli_main`, while desktop users get a genuine native `impulcifer-cli` binary. Neither requires bundling Tauri into the Python wheel.

| Wheel policy | Status and cost |
|---|---|
| Normal CPython | **[V]** PyO3 `abi3` allows a chosen minimum CPython version and later compatible GIL builds, with limited-API restrictions. Still one artifact per OS/architecture/ABI platform, not one universal wheel. [43] |
| Existing 3.13t / 3.14t users | **[V]** Ordinary `abi3` wheels do not supply the free-threaded ABI. Versioned PyO3 0.26 documentation requires interpreter-specific builds; detach long Rust work even on free-threaded interpreters. [43] **[I] Budget separate cp313t/cp314t target wheels if retaining both.** |
| Future ABI docs | **[V]** Rolling PyO3 docs now describe `abi3t` for 3.15+; that does not retroactively make 3.13t/3.14t consume normal abi3 wheels. **[I] Do not base current shipping requirements on a future/interpreter-specific ABI transition.** [43] |
| Example matrix | **[I]** Windows x64, macOS arm64, macOS x64, manylinux x64 = four regular-ABI build targets; supporting two existing free-threaded versions adds up to eight target builds if all four are supported. Linux arm64/musllinux/Windows ARM add more. This is an example policy, not a required universal matrix. |
| Tooling | **[V]** `maturin-action` supports multiple OS/architectures, explicit manylinux baselines and optional sccache; generates wheel pipelines. [44] **[I] Cache and reuse core crate builds where compatible, but do not pretend a Linux wheel is the AppImage binary.** |

**[I] Binding boundary:** accept config objects and array/buffer inputs, validate dtype/shape/layout, copy or retain safely owned memory for background work, release/detach from Python for long Rust computations, and reacquire only when returning results or explicit callbacks. Do not share a borrow into mutable Python/NumPy memory with Rayon workers after its safe lifetime. Start with owned arrays; optimize copying only after profiling. [43]

### 2.12 Plots and replacement of Bokeh HTML

| Option | Verified abilities | Judgment |
|---|---|---|
| **Plotters 0.3.7** | Headless `BitMapBackend` PNG output; bitmap encoding; 2D and 3D examples; optional font backends including explicitly registered `ab_glyph` fonts. No verified ready-made Bokeh-equivalent interactive HTML export. [47] | **[I] Use for CLI/headless static PNG.** Expect layout/font/axis implementation work. Bundle licensed fonts and test nine languages; CJK shaping/coverage is not guaranteed merely because the renderer accepts Unicode. |
| **uPlot** | Canvas2D line/time-series charts, log axes, live updates, local IIFE build. Project benchmark identifies v1.6.24 and March 2023 Chrome 113/5850U setup; ~25 ms initialization claim for 166,650 points. [48] | **[I] Excellent candidate for FR/IR/ILD/IPD overlays, not a complete spectrogram/waterfall toolkit.** Do not extrapolate its Chrome benchmark to WKWebView or 3 million points. |
| **Plotly.js** | Local standalone script distribution; explicit heatmap and 3D-surface examples; MIT. README examples reference 4.0.0 but current latest release was not separately established. [49] | **[I] Most complete Bokeh replacement for self-contained scientific HTML**, with a larger JS payload and WebGL/GPU testing burden where 3D/WebGL is used. |
| **Handwritten Canvas2D** | **[I]** Small dependency surface and precise decimated/raster rendering are feasible. **[U]** No ready implementation verified. | **[I] Bad default for a solo maintainer if it means rebuilding axes, zoom, crosshair, legends, keyboard access, export and synchronized selections.** Use only for a focused raster/tile layer, not the entire plotting system. |

**[I] Recommended split:** a UI-independent Rust `analysis` model produces numerical series/grids; Plotters consumes it for static PNGs; exported offline HTML embeds a pinned local Plotly.js distribution and serialized plot models. This preserves interaural overlays, ILD, IPD and IACC as explicit layouts rather than hoping another library knows the domain. A decay waterfall can use explicit surface/line geometry; do not confuse Plotly's financial `waterfall` trace with an acoustic decay waterfall.

**[I] Initial desktop UI:** retain the current form-first workflow and open generated interactive reports externally. If embedding plots later, use the same plot model with lazy-loaded Plotly; only introduce uPlot as a second library after profiling demonstrates a need. Avoid shipping two plotting engines from day one solely because one is smaller.

**[I] Offline HTML must really be offline:** embed/vendor JS, data and required fonts; do not leave CDN/D3/MathJax dependencies in examples. Escape user-controlled filenames/labels during HTML generation. Open exported reports outside the privileged app webview, or use an explicitly unprivileged view with no filesystem/shell/command access. Headless PNG must not require launching a webview. [15,47–49]

### 2.13 Real applications and public shell-choice accounts

**[V] Four relevant real applications were verified.** These demonstrate deployment in audio/numerical workflows, not parity with a 16-channel measurement recorder.

| Application | Verified evidence | Limitations |
|---|---|---|
| **Musicat** | Local music player/tagger, waveform display and gapless playback. Inspected source manifest: app 0.17.2, Tauri requirement 2.10.2, Symphonia/CPAL/Rubato/RustFFT. [50] | [V] 0.x evolving-app warning. Manifest requirements are not lockfile resolutions or release dates; no 16-output/2-input evidence. |
| **Handy** | Local speech-to-text with microphone capture, resampling, VAD and local inference. Manifest app 0.9.6, Tauri requirement 2.11.5, transcribe-rs 0.3.8/transcribe-cpp 0.2.0. [51] | [V] Substantial native work outside the webview. [U] This is not a BRIR solver or hardware-host-API coverage test. |
| **Audion** | Local-library player with lyrics/playlists; README identifies Tauri 2.0 and displays 1.2.4 badge. [52] | [U] Release date/resolved dependency versions and advertised behavior were not execution-tested. |
| **Met-ID** | Mass-spectrometry metabolite annotation. Peer-reviewed 2025-04-20 paper describes Tauri/Rust plus packaged Python/RDKit helpers and ~1.3 s for 1,538 searched features on i7-10700K. [53] | [V] Demonstrates scientific desktop use, including sidecars. Paper does not establish Tauri major version; timing is an application result, not shell performance. |

| Account | Verified statement | Use in this decision |
|---|---|---|
| **GitSquid, 2026-04-21** | First-party Electron→Tauri/Rust article reports retaining ~95% of React code and writing ~7,200 Rust lines; settings compatibility and host-service replacement work discussed. Old package >150 MB; no measured final size/RAM/startup comparison supplied. [54] | [I] Supports separating frontend reuse from backend service rewrite effort. It does not prove Impulcifer will retain exactly 95%. |
| **Moosync** | Old Electron repository archived 2025-03-11 and explicitly points to a Rust/Tauri rewrite. [55] | [V] Rewrite announcement, not a detailed postmortem establishing why Tauri was best. |
| **OpenCode** | Official source contains Tauri-store→Electron migration; issue #26143 (2026-05-07) notes stale Tauri docs. [56] | [V] Reverse migrations exist. **[U] No verified first-party rationale here; do not manufacture a claim that WebKit forced the switch.** |

**[I] Documentation/agent suitability:** commands, serde contracts, plugin API docs and versioned crate source provide good testable boundaries for agent-written code. The numerical port has the opposite problem: many easy-to-name crates do not replicate SciPy semantics. Require versioned sources and executable tests in task briefs; reject generic claims that “Rust audio ecosystem” covers R2.

## 3 Proposed architecture

Everything in this section is **[I] proposed architecture**, informed by the verified contracts above; it has not been implemented or benchmarked.

### 3.1 Workspace and dependency boundaries

```text
crates/
  impulcifer-dsp/           # f64 algorithms, typed config, stages, deterministic outputs
  impulcifer-audio-io/      # PortAudio wrapper; enumerate/probe/two independent streams
  impulcifer-analysis/      # plot data, PNG renderer, offline HTML renderer, WAV/export policy
  impulcifer-jobs/          # typed job runner, journal, cancellation, progress sinks
  impulcifer-app/           # Tauri binary, serde commands, OS dialogs/settings/update providers
  impulcifer-cli/           # independent binary, no Tauri/GTK/WKWebView dependency
  impulcifer-python/        # PyO3 cdylib, Python package glue; no Tauri dependency
webview_ui/                # existing static HTML/CSS/JS, host adapter, vendored chart assets
python/impulcifer/          # small stable Python API + console launcher
```

| Crate | Boundary |
|---|---|
| `dsp` | No UI, OS dialogs, Python, Tauri or global locale. `ProcessingConfig` is the sole default definition. `run(config, inputs, context) -> Result<Artifacts, DspError>`. Model arrays explicitly as planar f64 plus shape/sample-rate/channel identities. |
| `audio-io` | Native host API is an explicit enum/string identity; device ids distinguish duplicate names. `probe_output(device,channels,fs)` and separate input/output opens validate exact formats. Exposes a blocking `record_sweep` orchestration method, not a browser/audio stream. PortAudio dependency pinned and audited. |
| `analysis` | Derived curves/grids shared by PNG and HTML. Exact WAV track ordering/header policy. Reuses f64 calculations, with explicit f32 conversion only for visual buffers or required output encoding. |
| `jobs` | No Tauri types. Job snapshots/events/errors plus a sink interface and cooperative token. CLI and Python can run synchronously through the same core without adopting GUI polling. |
| `app` | Very thin commands translate envelopes and call service operations. UI-only capabilities scoped to the main bundled window. Owns dialogs, settings migration, updater strategy and webview lifecycle. |
| `cli` | Independent native executable with batch exit codes, stdout/stderr conventions and optional JSON-lines progress. Same DSP implementation, not a second algorithm. |
| `python` | Stable Python-facing API and launcher; enters the same typed core after validation and detaches for long work. Per-version interpreter compatibility belongs here, not in DSP. |

**Keep ffmpeg/ffprobe external.** A host-owned helper manager downloads platform-appropriate binaries, verifies expected integrity/source policy, stores them under app data, passes arguments as arrays without a command shell, captures errors and supports cancellation. Runtime-downloaded helpers are not necessarily Tauri `externalBin` sidecars; that setting is for binaries bundled at build time. [4]

### 3.2 Jobs, progress and cancellation

- One recorder or BRIR/recovery job at a time initially, matching existing product semantics. Updater application is mutually exclusive with all write/record work; whether download is concurrent is an explicit resource policy.
- `JobId` is opaque. Snapshot includes kind, lifecycle state, cancellable flag, progress, artifacts/error and latest sequence. Preserve `running`, `cancel_requested`, `succeeded`, `failed`, `cancelled`; an optional `queued` state must be added consistently to UI/CLI tests.
- A monotonically increasing per-job journal records `{seq,timestamp_ms,type,payload}`. Bounded memory retention plus persistent log file; polling returns `oldest_seq`/`gap` if a cursor predates retained events. A final snapshot survives frontend reload.
- Use current 250 ms polling first. Batch progress/logs and coalesce elapsed-time updates. Channels can later reduce latency without changing the durable result/journal model.
- For channel attachment, subscribe and obtain the initial snapshot/cursor under one registry synchronization operation. Replay up to that cursor, then ordered live messages; client deduplicates by sequence. Do not create a race between `start_job` and the first listener.
- Cancellation uses an `Arc<AtomicBool>`-backed token or equivalent typed abstraction, checked at stage boundaries, between speaker work and inside long iterative fits/chunked operations. Document the longest noninterruptible primitive. `cancel_requested` is not `cancelled` until workers have stopped and files are consistent.
- FFT/library calls that do not expose cancellation cannot be safely force-killed in-process. If a strict cancellation deadline later becomes mandatory, run the worker in the CLI sidecar and terminate only against staged, disposable output files.
- Write outputs to staging paths and atomically promote completed files; preserve previous good outputs on failure. No detached thread may keep writing after a terminal cancellation is reported.
- Recording cancellation stays disabled until a hardware-tested two-stream stop/drain/abort policy exists. PortAudio/native ASIO limitations cannot be solved by a Rust token alone. Always return recording-thread errors to the job; completion means input saved and playback drained, not only that the output callback ended.

### 3.3 Threading

- Tauri main/event-loop thread is for the shell. Commands validate quickly and enqueue; no FFT, audio blocking call, PNG render or fit runs there.
- A dedicated bounded Rayon pool parallelizes **independent speakers**, not arbitrary global reductions. Reserve capacity for UI and avoid nested library/Rayon pools oversubscribing the machine. Pool size is configurable and limited by memory, not blindly equal to all logical cores.
- Gather per-speaker results in a fixed speaker order and perform cross-speaker/global normalization reductions in a fixed order. Rayon explicitly documents nondeterministic floating-point `sum` reduction grouping; race freedom is not numerical determinism. [57]
- Reuse FFT plans and worker-local scratch buffers; cap simultaneously materialized arrays. Explicitly choose normalization, denormal handling and SIMD policy. Do not enable relaxed/fast-math assumptions merely to advertise speed.
- Recording uses independently owned input/output streams on dedicated native threads, coordinated for startup/completion. Progress reporter reads counters/time; audio code does not wait for JS, take UI locks or allocate formatting strings per buffer.
- Python calls release/detach from the interpreter; CLI does not initialize Tauri at all. A global Python GIL is not needed to protect Rust-owned processing state. [43]

### 3.4 IPC contract sketch

**Compatibility control envelope**

```text
Success<T> = { ok: true, data: T }
Failure    = { ok: false, error: { code, message, details, retryable } }

bootstrap() -> {
  api_version, app_version, platform, webview_backend,
  ui, brir_defaults, sweep, active_job, capabilities
}
start_brir({request: ProcessingRequest}) -> {job: JobSnapshot}
poll_job({jobId, afterSeq}) -> {job, events, next_seq, oldest_seq, gap}
cancel_job({jobId}) -> {job}
subscribe_job({jobId, afterSeq, channel}) -> {snapshot, replay_cursor}
get_plot_manifest({artifactId}) -> {
  arrays: [{arrayId, dtype, byte_order, shape, axes, unit, channel_ids}]
}
get_plot_buffer({arrayId, offset, length}) -> raw Response
```

- Preserve stable stage keys such as `cli_opening_measurements` as machine-readable event fields; human messages remain localized. Do not infer stages by matching translated log text in new code.
- Plot array ids are opaque job/artifact references, not arbitrary file paths. Bound offsets/lengths, cap response sizes and validate job ownership/state. No frontend command reads arbitrary bytes from a caller-specified path solely to make plotting convenient.
- Wire plot buffers as explicitly little-endian f32/f64 with declared shape/order. Validate alignment and byte count before constructing typed arrays; use DataView/conversion where needed. Data intended for scientific computation remains f64 in Rust.
- Native file dialogs return `{path:null}` on cancellation; the existing URL-name allowlist stays authoritative. Tauri capabilities do not automatically make unrestricted custom commands safe. Custom commands need deliberate permissions and input checks. [15]
- `start_update` should consume a backend-owned update descriptor/token, not trust an arbitrary frontend URL as install authority. Backend revalidates provider metadata, version and signatures/hashes before apply.
- No remote content receives privileged IPC. Analysis HTML is unprivileged even if it contains filenames or imported labels originating outside the app.

### 3.5 Separate CLI versus same binary/headless flag

**Recommendation: a separate CLI binary sharing library crates.** Ship it as a desktop resource/sidecar where useful and as a native release artifact. The GUI may call libraries in-process by default; packaging a CLI does not require spawning it for every job.

Tauri sidecars use `bundle.externalBin` and filenames suffixed with a target triple, e.g. `impulcifer-cli-x86_64-pc-windows-msvc.exe`. Rust can spawn through the shell plugin and consume stdout/stderr/termination events; frontend process permissions can remain absent because Rust owns the operation. [4]

A same-executable `--headless` branch is technically plausible if parsed before constructing Tauri, but the executable can still carry/link GUI dependencies, especially Linux GTK/WebKit, and Windows GUI-subsystem console behavior is inconvenient. It is a bad default for a genuinely headless CLI. Separate binaries cost some duplicated code on disk but make deployment and tests honest.

### 3.6 Numerical parity strategy and staged adoption

**Do not promise Python/Rust byte identity.** It is not a universal mathematical impossibility, but it is an inappropriate acceptance criterion across different FFT planners, libm implementations, SIMD paths and nonlinear solvers. Preserve byte-identical tests for repeat runs of the same pinned Rust implementation/environment; replace cross-implementation SHA-only acceptance with explicit numerical and format contracts.

| Gate | Proposed checks |
|---|---|
| **Pinned Python oracle** | Record commit, Python/NumPy/SciPy/nnresample versions, config/defaults, input hashes and reference outputs before changing algorithms. Use current implementation as oracle, not an unversioned pip environment. |
| **Primitive goldens** | Odd/even FFT lengths and scaling; convolution/correlation mode and lag sign; SOS order/state; spline knots/extrapolation; FIR symmetry/length; minimum-phase floor/length; Savitzky–Golay edge rules; peak ties/plateaus; resampling phase and exact output length. Compare float64 arrays with magnitude-aware absolute/relative tolerances. |
| **Stage goldens** | Deconvolved impulses, aligned/cropped IR, room/headphone correction, fitted EQ response, decay-shaped output, microphone correction, virtual bass and final normalization. Record peak index/time and channel identity separately from amplitude. |
| **Optimizer parity** | Compare achieved frequency-response error, bound satisfaction, filter count/order policy, pole stability and repeatability, not just optimizer parameter-vector distance. Fix evaluation budgets and initialization; a wall-clock timeout makes goldens machine-dependent. |
| **End-to-end** | Run demo default/headphone path and vbass path, plus real high-channel and pathological recordings. Compare max/RMS sample error, FR dB, ILD, IPD/ITD, IACC, decay metrics, clipping/headroom, length/sample-rate and all output track mappings. Mask near-zero bins for phase/dB comparisons; otherwise numerical noise creates meaningless failures. |
| **Suggested initial investigation thresholds** | Start primitive tests around rtol 1e-10/atol 1e-12 where conditioning warrants; start end-to-end FR/ILD investigation at 0.01 dB over meaningful signal bins, and preserve integer alignment unless a documented change requires otherwise. **These are proposed diagnostics, not established audible-transparency limits or promises every algorithm must meet.** Tighten/adjust per stage using the golden corpus and justified error budgets. |
| **Packaging/GUI parity** | Real installed app launch, WebView2-missing scenario, upgrade from existing Velopack release, input/output file dialogs, nine-language rendering, cancel/reload job recovery, TrueHD helper absence/download/failure, and CLI without display server. |

**Implementation order:** (1) extract reference fixtures and contract tests; (2) prove native audio and difficult optimizer port; (3) port/test DSP stages in independent crates; (4) put the unchanged static frontend behind Tauri with a mock/test backend; (5) connect Rust jobs and exports; (6) verify installers/upgrades and PyPI; (7) retire Python runtime from desktop only when both numerical and operational gates pass. Temporary Python sidecars can help staged validation but retain the packaging pain and are not the final Rust-core solution.

## 4 Risks ranked

Ranks and mitigations are **[I] assessments**, not measured incident probabilities.

| Rank | Severity | Risk | Required mitigation / decision gate |
|---:|---|---|---|
| **1** | **Critical** | SciPy semantic/optimizer drift produces different BRIRs despite clean compilation and plausible plots. | Per-primitive and per-stage oracle fixtures; bounded optimizer prototype; domain metrics; reject guessed drop-in replacements. |
| **2** | **Critical** | Required host APIs and independent 16-out/2-in streams are not proven; CPAL omits MME/DirectSound, ASIO/native drivers impose limits. | Own native audio abstraction; format probing; actual interface/driver matrix and completion/error tests before full rewrite. |
| **3** | **High** | Solo-maintainer scope explosion: port DSP, plots, Python wheels, audio FFI and installer migration at once. | Stage migration; keep frontend and Windows Velopack; no aesthetic rewrite or new bundler. |
| **4** | **High on Linux** | WebKitGTK/GPU/ABI combinations yield blank windows despite an AppImage. | Published best-effort support matrix, native decoration, bundled-engine refresh, packaged smoke runs and CLI/source fallback. |
| **5** | **High** | Existing users strand on incompatible updater feed/layout, or update kills recording/writes. | Preserve Windows pack identity/provider first; upgrade old installation in CI/VM; safe idle-only apply; separately managed signing keys. |
| **6** | **Medium–high** | Three million plotted samples multiply memory and expose platform-specific IPC serialization. | On-demand tracks, decimation/tiles, raw buffer schema, transfer benchmarks on actual webviews, no zero-copy assumption. |
| **7** | **Medium–high** | WAV library invents speaker masks for independent BRIR tracks; channel count correct but consumer mapping wrong. | Explicit export headers/order and validation in all target consumers; do not equate `channels: u16` with output-format parity. |
| **8** | **Medium** | Rust/native packaging still drifts: PortAudio build, WRY/tao 0.x, font libraries, FFmpeg, signing tools. | Pinned compiler/Cargo lock/actions/native sources; controlled updates and installed-artifact smoke tests. |
| **9** | **Medium** | PyPI support multiplies builds, especially free-threaded interpreters; binding ownership errors under Rayon. | GUI-free bindings, explicit ABI support policy, owned buffers first, interpreter detachment and native wheel tests. |
| **10** | **Medium** | macOS drag/drop/font/titlebar behavior differs from browser tests; opaque-window assumptions change later. | Real packaged WKWebView tests with nine languages and scaled displays; avoid whole-window transparency. |
| **11** | **Medium** | Excessive IPC privileges turn imported metadata/report HTML into filesystem/helper access. | Allowlisted host methods, validated paths/tokens, no remote-origin grants, unprivileged external HTML. |

## 5 Open questions you could not settle

1. **[U] Which actual audio interfaces/drivers must pass R1?** Need named hardware, Windows host APIs and separate-device versus same-device combinations. In particular, two-stream ASIO feasibility cannot be inferred from a crate feature.
2. **[U] Which bounded Rust optimizer will meet the current AutoEQ objective and termination semantics?** No TRF-equivalent implementation was qualified in this shell-focused study.
3. **[U] What exact numerical error budget is acceptable?** Proposed 0.01 dB investigation thresholds are not a maintainer-approved replacement for SHA parity; difficult nulls/phase/decay need stage-specific decisions.
4. **[U] Final clean-build, warm-build, launch, package and process-tree RAM measurements.** Must include retained data/fonts and Linux WebKit; do not extrapolate hello-world measurements.
5. **[U] End-to-end raw 12 MB IPC performance and memory on the selected WebView2/WKWebView/WebKitGTK versions.** Published 64 KiB Mac figures are insufficient.
6. **[U] Exact fixes available in Tauri's resolved WRY/tao dependencies for the cited open macOS drop and Linux rendering reports.** Upstream fixes/releases and Tauri's adopted dependency versions are distinct.
7. **[U] Velopack→Tauri payload replacement with this application's existing package identity.** Rust SDK existence is verified; the actual installed-app upgrade is not. A full updater-provider migration is even less established.
8. **[U] Literal PyPI name `impulcifer` ownership and desired interpreter/platform support policy.** Existing known distribution is `impulcifer-py313`; free-threaded and Linux-architecture scope determine cost.
9. **[U] Nine-language Plotters font/shaping equivalence, and how much Bokeh interaction users actually depend on.** Verify font licenses and representative screenshots/exports before selecting final plot composition.
10. **[U] Exact 30/32-track extensible-WAV channel-mask expectations across all target consumers.** A generic writer's defaults cannot settle this.
11. **[U] A complete first-party explanation for OpenCode's reverse migration was not verified.** Do not use it as a causal argument against Tauri.
12. **[U] Published dates for all auxiliary plugin versions and exact release versions of the mutable app examples were not independently established.** They are explicitly inspection snapshots rather than immutable historical claims.

## 6 Sources

All accessed 2026-09-07. URLs grouped under one number concern one evidentiary topic. Local links point to inspected paths; GitHub source URLs provide shareable locations but the local checkout, not mutable master, is the basis of L-series observations.

### Local code evidence

- **[L1]** Frontend, complete read: `E:/Impulcifer/webview_ui/app.js:1–1488`. https://github.com/115dkk/Impulcifer-pip313/blob/master/webview_ui/app.js
- **[L2]** Host, complete read: `E:/Impulcifer/impulcifer_webview.py:1–340`. https://github.com/115dkk/Impulcifer-pip313/blob/master/impulcifer_webview.py
- **[L3]** Job model/bootstrap/poll/cancel/journal: `E:/Impulcifer/application/impulcifer_service.py:201–283,519–553,942–977`. https://github.com/115dkk/Impulcifer-pip313/blob/master/application/impulcifer_service.py
- **[L4]** Recorder, complete read: `E:/Impulcifer/core/recorder.py:1–553`. https://github.com/115dkk/Impulcifer-pip313/blob/master/core/recorder.py
- **[L5]** Bounded biquad fitting: `E:/Impulcifer/autoeq/frequency_response.py:350–524`. https://github.com/115dkk/Impulcifer-pip313/blob/master/autoeq/frequency_response.py
- **[L6]** Release pipeline: `E:/Impulcifer/.github/workflows/publish.yml:1–130`. https://github.com/115dkk/Impulcifer-pip313/blob/master/.github/workflows/publish.yml
- **[L7]** ADR, complete read: `E:/Impulcifer/docs/adr/0001-native-frontends-stay-native.md:1–47`. https://github.com/115dkk/Impulcifer-pip313/blob/master/docs/adr/0001-native-frontends-stay-native.md

### Numbered external sources

1. Tauri commands, serde, raw responses and raw requests. https://v2.tauri.app/develop/calling-rust/
2. Tauri channels versus frontend events. https://v2.tauri.app/develop/calling-frontend/
3. Static assets/global API configuration and core JS namespace. https://v2.tauri.app/reference/config/ ; https://v2.tauri.app/reference/config/#withglobaltauri ; https://v2.tauri.app/reference/javascript/api/namespacecore/
4. Tauri sidecar mechanics. https://v2.tauri.app/develop/sidecar/
5. Latest Tauri core version/date API and release. https://api.github.com/repos/tauri-apps/tauri/releases/latest ; https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.11.5
6. Tauri release cadence/CLI/runtime notes. https://api.github.com/repos/tauri-apps/tauri/releases?per_page=15 ; https://github.com/tauri-apps/tauri/releases
7. JS API version in core release. https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/packages/api/package.json
8. WRY 0.56.1 release metadata. https://api.github.com/repos/tauri-apps/wry/releases/latest
9. tao 0.37.0 release metadata. https://api.github.com/repos/tauri-apps/tao/releases/latest
10. WRY/tao requirements used by Tauri 2.11.5. https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri-runtime-wry/Cargo.toml
11. Native theme API and platform notes. https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindow.html
12. Dialog APIs/global namespace/permissions. https://v2.tauri.app/plugin/dialog/ ; https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/dialog/guest-js/index.ts
13. Official plugins and observed versioned crate documentation. https://v2.tauri.app/plugin/ ; https://docs.rs/tauri-plugin-dialog/latest/tauri_plugin_dialog/ ; https://docs.rs/tauri-plugin-fs/latest/tauri_plugin_fs/ ; https://docs.rs/tauri-plugin-process/latest/tauri_plugin_process/ ; https://docs.rs/tauri-plugin-os/latest/tauri_plugin_os/ ; https://docs.rs/tauri-plugin-shell/latest/tauri_plugin_shell/
14. Opener APIs/version. https://docs.rs/tauri-plugin-opener/latest/tauri_plugin_opener/
15. Tauri capabilities/custom-command defaults/remote-content restrictions. https://v2.tauri.app/security/capabilities/
16. Process exit and relaunch. https://v2.tauri.app/plugin/process/
17. Linux API 4.1 migration and dependencies. https://v2.tauri.app/blog/tauri-2-0-0-alpha-3/ ; https://webkitgtk.org/reference/webkit2gtk/stable/index.html ; https://v2.tauri.app/start/prerequisites/ ; https://v2.tauri.app/reference/webview-versions/
18. Windows installers, WebView2 options and cross-build limitations. https://v2.tauri.app/distribute/windows-installer/
19. Microsoft runtime distribution/presence/size/security requirements. https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
20. Window customization. https://v2.tauri.app/learn/window-customization/
21. macOS drag/drop reports and API caveat. https://github.com/tauri-apps/tauri/issues/10744 ; https://github.com/tauri-apps/wry/pull/1723 ; https://v2.tauri.app/reference/javascript/api/namespacewebview/
22. Platform rendering/font issue evidence. https://github.com/tauri-apps/tauri/issues/15471 ; https://bugs.webkit.org/show_bug.cgi?id=316777 ; https://bugs.webkit.org/show_bug.cgi?id=280210 ; https://github.com/tauri-apps/tauri/issues/13151 ; https://github.com/tauri-apps/tauri/issues/14924 ; https://github.com/tauri-apps/tauri/issues/15050
23. AppImage/Flatpak packaging and WebKit bundle evidence. https://v2.tauri.app/distribute/appimage/ ; https://github.com/orgs/tauri-apps/discussions/10026 ; https://github.com/tauri-apps/tauri/issues/12463 ; https://github.com/tauri-apps/tauri/pull/12466 ; https://v2.tauri.app/distribute/flatpak/
24. CPAL supported backend table. https://github.com/RustAudio/cpal
25. PortAudio native API capabilities and stream restrictions. https://www.portaudio.com/docs/v19-doxydocs/api_overview.html
26. Rust PortAudio maintenance/build/documentation status. https://docs.rs/portaudio/latest/portaudio/
27. FFT f64/normalization/planning. https://docs.rs/rustfft/latest/rustfft/ ; https://docs.rs/realfft/latest/realfft/
28. Pinned raw IPC/channel implementation. https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri/src/ipc/channel.rs ; https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri/src/ipc/protocol.rs
29. Published Mac IPC microbenchmark and limitations. https://docs.rs/crate/connectrpc-tauri/0.1.2
30. Official Tauri GitHub Action. https://github.com/tauri-apps/tauri-action
31. SciPy least_squares defaults and method/bounds semantics. https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html
32. Hound 3.5.1 WAV layout/float support and writer-mask implementation. https://docs.rs/hound/latest/hound/ ; https://docs.rs/hound/3.5.1/hound/struct.WavSpec.html ; https://docs.rs/hound/3.5.1/src/hound/write.rs.html ; https://docs.rs/hound/3.5.1/src/hound/lib.rs.html
33. Tauri updater configuration, artifacts, signatures and endpoints. https://v2.tauri.app/plugin/updater/
34. Updater exact release metadata/API. https://crates.io/api/v1/crates/tauri-plugin-updater ; https://docs.rs/tauri-plugin-updater/2.11.0/tauri_plugin_updater/
35. Velopack Rust publication timestamp and API. https://crates.io/api/v1/crates/velopack ; https://docs.rs/velopack/1.2.0/velopack/
36. Velopack Rust integration, packaging prerequisite and samples. https://docs.velopack.io/getting-started/rust ; https://github.com/velopack/velopack/tree/develop/samples
37. Velopack signing documentation and inspected hash validation. https://docs.velopack.io/packaging/signing ; https://docs.rs/velopack/1.2.0/src/velopack/manager.rs.html
38. Velopack GitHub feed API/source. https://docs.rs/velopack/1.2.0/velopack/sources/struct.GithubSource.html ; https://docs.rs/velopack/1.2.0/src/velopack/sources/github.rs.html ; https://docs.rs/velopack/1.2.0/src/velopack/sources/mod.rs.html
39. Velopack installation/update layout. https://docs.velopack.io/packaging/installer ; https://docs.velopack.io/packaging/overview ; https://docs.velopack.io/distributing/overview ; https://docs.velopack.io/integrating/overview
40. Signing, notarization and macOS distribution. https://v2.tauri.app/distribute/sign/windows/ ; https://v2.tauri.app/distribute/sign/macos/ ; https://v2.tauri.app/distribute/dmg/
41. Maturin mixed-project layout. https://www.maturin.rs/project_layout.html
42. Maturin binary wheels versus library-plus-console-script. https://www.maturin.rs/bindings.html
43. PyO3 ABI/distribution/free-threading behavior; distinguish versioned 0.26 from rolling future-facing guide. https://pyo3.rs/v0.26.0/building-and-distribution.html ; https://pyo3.rs/v0.26.0/free-threading.html ; https://pyo3.rs/main/building-and-distribution.html
44. Maturin GitHub Action and build matrix. https://github.com/PyO3/maturin-action
45. Actual Impulcifer release assets and sizes. https://api.github.com/repos/115dkk/Impulcifer-pip313/releases/latest ; https://github.com/115dkk/Impulcifer-pip313/releases/tag/v2.14.0
46. Tauri benchmark methodology/data availability. https://github.com/tauri-apps/benchmark_results
47. Plotters capabilities and font backends. https://docs.rs/plotters/latest/plotters/
48. uPlot capabilities, local IIFE and benchmark methodology. https://github.com/leeoniya/uPlot
49. Plotly.js distribution/license and scientific plot examples. https://github.com/plotly/plotly.js ; https://plotly.com/javascript/heatmaps/ ; https://plotly.com/javascript/3d-surface-plots/
50. Musicat and inspected manifest. https://github.com/basharovV/musicat ; https://raw.githubusercontent.com/basharovV/musicat/main/src-tauri/Cargo.toml
51. Handy and inspected manifest. https://github.com/cjpais/Handy ; https://raw.githubusercontent.com/cjpais/Handy/main/src-tauri/Cargo.toml
52. Audion. https://github.com/dupitydumb/Audion
53. Met-ID paper and repository. https://pmc.ncbi.nlm.nih.gov/articles/PMC12044586/ ; https://github.com/pbjarterot/Met-ID
54. GitSquid first-party migration account. https://gitsquid.dev/blog/why-we-rewrote-electron-in-tauri-rust/
55. Moosync original repository/rewrite notice. https://github.com/Moosync/Moosync-electron
56. OpenCode reverse-migration evidence, without verified causal rationale. https://github.com/anomalyco/opencode/issues/26143 ; https://github.com/anomalyco/opencode/blob/dev/packages/desktop/src/main/migrate.ts
57. Rayon parallelism and non-deterministic floating-point reduction ordering. https://docs.rs/rayon/latest/rayon/ ; https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html
