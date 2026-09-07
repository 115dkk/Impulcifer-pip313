# S2 deep dive: C++ + Electron

Research date: **2026-09-07**. Scope: architecture research only; no repository changes or builds.

Evidence notation applies throughout: **[V]** verified in cited source; **[I]** inference, recommendation, calculation, or proposed acceptance criterion; **[U]** not established. All web sources were consulted on 2026-09-07. Unversioned `latest` documentation describes what was served that day, not an independently tested release. Local-file citations describe the supplied checkout, which differs from the latest published release. Source numbers resolve in section 6.

## 1 Verdict

1. **[I] S2 is viable; choose Electron + a separately executable C++ core, not a DSP addon in Electron's main process.**
2. **[I] Its strongest benefits are frontend reuse, native audio-library availability, and a CLI/core independent of the GUI runtime.**
3. **[V] Pure Node-API does not inherently require a binary for every Electron ABI; direct V8/Node-C++ addons do.** [1–5]
4. **[I] Sidecar IPC overhead is immaterial if requests contain paths/configuration and the DSP buffers stay native.**
5. **[I] Reject browser-worker Wasm as the primary implementation: it does not supply the required native host-API audio interface.** [8–10]
6. **[V] Current Windows download: Impulcifer 216.18 MB; bare Electron runtime ZIP: 158.20 MB. Comparable scale, not equivalent products.** [11–13]
7. **[U] Equal RAM or startup time is unproven; the maintainer's footprint argument only establishes that a large download may be acceptable.**
8. **[V] Velopack has an official JavaScript/Electron SDK; updater replacement is optional.** [16–18]
9. **[I] The principal rewrite risk is SciPy semantic/numerical parity, followed by native packaging/audio and headless plotting, not DOM work.**
10. **[I] For a solo maintainer, approve S2 only after a bounded-solver/parity spike, a 16-out/2-in hardware test, and a packaged sidecar/update test.**

## 2 Findings (with sources)

### 2.1 What the existing frontend actually requires

**[V] Read all of `E:/Impulcifer/webview_ui/app.js:1–1488`, plus the referenced recorder, optimizer and biquad code, the service polling/cancellation methods, and ADR 0001.** Local inspection supports the following migration inventory. [L1–L7]

| Existing implementation | Consequence for Electron |
|---|---|
| **[V]** `app.js:58` centralizes access as `api() => window.pywebview.api`. | **[I]** Replace one accessor with `window.impulcifer`; retain method names and promise/result envelopes. Do not mechanically expose the whole Electron IPC object. |
| **[V]** Calls 22 distinct methods, including `resolve_recording_paths`, `set_frontend`, and three setting mutators. | **[I]** Preserve the API contract during migration. Remove/replace the CTk selection deliberately, rather than pretending `set_frontend` still does something. |
| **[V]** `begin():542–603` handles `CONFIRMATION_REQUIRED`, retries with `confirm_warnings`, and obtains `data.job`. | **[I]** Preserve structured validation and warning confirmation. Shell migration must not silently change payload defaults or validation. |
| **[V]** `pollJob():628–674` polls at 250 ms with `job_id`, `nextSeq`; consumes `progress`, `log`, `status`; stops for `succeeded/failed/cancelled`. | **[I]** Keep polling initially. A C++ event journal can reproduce it without renderer event subscriptions or a frontend rewrite. |
| **[V]** Service returns events with `seq > after_seq`, plus snapshot and `next_seq`; cancellation sets `cancel_requested` and an event. `impulcifer_service.py:519–553`. | **[I]** Put sequence assignment and snapshots under one synchronization policy. Cancellation acknowledgement is not cancellation completion. |
| **[V]** Update progress has a separate job/cursor/poll loop, `app.js:676–846`. | **[I]** The Electron main process can adapt updater events into the same job shape. Do not move installation privileges into C++. |
| **[V]** `gatherBrirPayload():1102–1173` omits closed option groups and uses bootstrap defaults. | **[I]** Keep this behavior and contract tests; a port that always sends GUI fallback values can change DSP results. |
| **[V]** Boot depends on `pywebviewready` and a bridge-presence check; another check exists at line 933. | **[I]** Replace both readiness mechanisms, not just line 58. Preload establishes the API before renderer startup; boot after the DOM is available. |
| **[V]** System info explicitly renders Python/GIL; preboot strings explicitly mention Python; backend labels mention WebView2/WKWebView/WebKitGTK. | **[I]** Update these and their nine-language keys to C++ build/compiler/core version, Electron/Chromium, backend capabilities and worker count. |
| **[V]** Most UI uses `textContent`/DOM construction; `index.html:728` loads external `app.js`; inspected HTML has no inline script, inline event handler or inline style matches. | **[I]** A restrictive script CSP is plausible without rebuilding the frontend. Chart libraries and runtime style assignments still need explicit testing. |
| **[V]** ADR 0001 rejects forcing CTk through the webview service. | **[I]** Leave the frozen CTk implementation separate. This proposal concerns the replacement web GUI and core, not a retrospective CTk service migration. |

**[I] Effort assessment:** high reuse of HTML/CSS/DOM code; modest transport/UI-integration changes; substantial backend replacement. No defensible exact reuse percentage without a patch. The frontend currently renders status, not the full analysis plots, so chart replacement is additional implementation rather than a transport-only change.

### 2.2 Three C++ attachment options

| Dimension | Node-API addon | Native executable sidecar | Emscripten Wasm in worker |
|---|---|---|---|
| Float64 DSP | **[V]** C++ can retain native double arrays; Node-API exposes typed arrays. [1] | **[I]** Same native core, without V8 buffer ownership constraints. | **[V]** Wasm has genuine `f64`; it is not float32-only. [10] |
| Work lasting minutes | **[V]** `AsyncWorker::Execute` runs off the event loop; a Node worker can host synchronous native work. [6,7] | **[I]** Core scheduler runs independently; IPC reader remains responsive. | **[V/I]** Worker keeps UI responsive, but native synchronization must respect browser worker/event-loop rules. [8] |
| Progress | **[V/I]** Thread-safe function or progress worker; bounded queue and throttling required. Do not call JS from the native compute thread. [1,6] | **[I]** Poll event journal over framed stdio; tiny traffic. | **[I]** Worker messages work between bounded compute steps; blocking native execution needs a separate shared-state/progress strategy. |
| Cancellation | **[V]** `AsyncWorker.Cancel()` only cancels not-yet-started work. A synchronous addon call blocks that worker's message handler. [6,7] | **[I]** Protocol thread sets a native atomic/stop token; compute checks between stages/blocks/solver iterations. | **[V/I]** Shared atomic cancellation for pthread builds; ordinary cancel messages alone cannot interrupt a blocked worker. [7–9] |
| Native crash | **[I]** Fatal native fault in an in-process Node worker can terminate its containing Electron process; a worker is not an OS-process crash barrier. | **[I]** Main survives a core crash and can mark the job failed, retain logs and restart the child. This is crash isolation, not a security sandbox. | **[I]** Wasm memory isolation helps contain memory faults, but introduces a different runtime and its own limits. |
| ABI | **[V]** Pure Node-API reduces runtime ABI matrix; direct V8/NAN/Node-C++ use requires runtime-specific builds. [1–5] | **[I]** Versioned protocol only; no Electron ABI for the core. OS/architecture/compiler/library compatibility still matters. | **[I]** No native Node addon ABI for DSP; Emscripten/browser-feature compatibility replaces it. |
| R1 host APIs | **[I]** Direct PortAudio inside addon can provide them if built correctly; no need for Web Audio. | **[I]** Best fit: PortAudio plus native device enumeration and stream control. | **[V/I]** Browser OpenAL/WebAudio is not a PortAudio host-API interface. A separate native audio component remains necessary. [9,28] |
| Large data | **[V]** V8 memory cage disallows arbitrary external native-buffer exposure; copying or V8-owned allocation is required for affected APIs. [14] | **[I]** Keep raw buffers private; files/binary artifacts handle selected exports. | **[V]** Emscripten default maximum growable memory is 2 GiB; larger settings and memory growth need explicit configuration. [8] |
| Verdict | **[I]** Technically sound second choice; attractive for a reusable npm DSP library, unnecessary coupling here. | **[I]** Recommended. The app already behaves like an asynchronous service client. | **[I]** Wrong primary target for this desktop measurement tool; useful later for browser-only analysis of previously recorded files. |

#### Addon details that matter

**[V] Node-API is an ABI-stable C interface; `node-addon-api` is its header-only C++ wrapper.** The guarantee does not cover direct V8, Node C++, libuv calls or arbitrary external library ABIs. CMake.js explicitly documents Node-API use across Node/Electron, while Electron's generic native-module guide emphasizes rebuilding runtime-specific modules. Read those together, not as contradictory blanket rules. [1–4]

**[I] Build choice:** use CMake.js for an addon if chosen, because the core/CLI/Python bindings already need CMake. `node-gyp` is viable but creates a second native build description unless kept as a thin invocation layer. Pin a supported Node-API version, use no experimental API, and test each Electron upgrade. `prebuildify --napi` packages binaries by OS/architecture; ordinary `prebuild`/runtime-specific builds may instead select Electron ABI targets. On Linux, libc/baseline compatibility is another dimension. [3–5]

**[V] Context awareness and thread safety are separate.** Node-API or a context-aware addon is required for multiple Node environments; global `napi_env`/JS handles are invalid across worker instances. Use per-environment state and explicit cleanup hooks. `NODE_MODULE()` examples with process-global constructors are unsuitable. [1,2]

**[I] Do not launch 15 minutes-long speaker tasks as unrelated libuv `AsyncWorker`s.** A dedicated core pool should own per-speaker scheduling. Use one asynchronous job entry, aggregate events, and keep the JS event loop available for cancellation. If using a synchronous native entry inside `worker_threads`, a posted cancel message will wait; use a correctly implemented shared atomic signal or redesign to asynchronous native jobs. `worker.terminate()` is not a documented arbitrary-native-code interrupt. [6,7]

**[V] Electron's V8 memory-cage restriction is not a blanket 4 GiB cap on the C++ process heap.** The cited announcement describes a 4 GiB V8 heap under pointer compression and separate ArrayBuffer constraints. Avoid `napi_create_external_arraybuffer`/external Buffer ownership assumptions; use runtime-owned storage or copies. Do not cite the historical cage article as proof of a current whole-app memory ceiling. [14]

#### Sidecar transports

| Transport | Assessment for this workload |
|---|---|
| **[V/I] `spawn(executable, args, {shell:false, stdio:'pipe'})`** | Node's asynchronous spawn with pipes is documented. Use this first: no listening port, authentication handshake or firewall prompt; one parent/child lifecycle. Drain stderr and stdout continuously because full pipes block the child. [15] |
| **[I] Named pipe / Unix-domain socket** | Worth adding only for reconnectable independent daemon jobs or multiple clients. Windows ACLs, socket ownership, stale names and reconnection become explicit responsibilities. No workload need was established. |
| **[I] gRPC** | Valid but excessive for roughly 22 local control methods. Adds protobuf/code generation, C++ dependency work and local endpoint security. Binary data streaming is not needed when files are local and DSP buffers never leave the core. |

**[I] Select newline-delimited JSON-RPC 2.0 messages over stdio, with strict escaping, byte limits, incremental parsing and a startup protocol/version handshake.** JSON-RPC defines the envelope, not framing. Keep stdout protocol-only; stderr for diagnostic logs. No raw sample arrays in JSON. A child outside ASAR avoids extraction and spawn restrictions documented for archived executables. [15,44]

#### Wasm: precise limits rather than folklore

- **[V]** Emscripten pthreads require `-pthread`, SharedArrayBuffer and an appropriate cross-origin-isolated browser environment (COOP/COEP). Threaded and fallback non-threaded builds are separate outputs. Worker startup, blocking waits, `PROXY_TO_PTHREAD`, and pool preallocation have specific constraints. [8]
- **[V]** `MAXIMUM_MEMORY` defaults to 2 GiB when growth is enabled; growth replaces JS typed-array views. Wasm64/memory64 options exist. **[U]** A production-compatible >4 GiB memory64/threaded configuration for the chosen Electron release and all dependencies was not tested. Do not assert that Wasm can never exceed 4 GiB, or assume it can do so here without work. [8]
- **[I] Arithmetic example:** 16 channels × 96,000 frames/s × 120 s × 8 bytes = **1,474,560,000 bytes (1.475 GB)** for one double buffer. FFT workspaces, input/output, per-speaker parallelism and JS copies can multiply that. A 2 GiB configuration is an avoidable constraint, not a theoretical footnote.
- **[V]** Wasm `f64` is double precision. **[U]** No measured float64 FFT/optimizer throughput versus native C++ for this application was found. SIMD, compiler flags, allocation and browser runtime change results; no “near-native percentage” is justified. [10]
- **[V/I]** Emscripten's audio interface uses browser Web Audio/OpenAL, permission/event-loop mechanisms and potentially sample-rate conversion. It does not establish MME/DirectSound/WASAPI/ASIO enumeration or exact 16-channel routing. Adding a native audio sidecar defeats the proposed all-Wasm simplification. [9]

### 2.3 Footprint: numbers that actually exist

**[V] Published versions:** Impulcifer **v2.14.0**, published **2026-09-05 17:21:22 UTC**; Electron **v44.2.0**, published **2026-09-04 02:19:23 UTC**, with Chromium **152.0.7977.76**, Node **24.20.0**, V8 **15.2.124.19**. These are remote release facts, not the version of the inspected local checkout. [11–13]

| Artifact | Exact bytes | Decimal MB |
|---|---:|---:|
| **[V]** Impulcifer Windows Setup EXE | 216,175,622 | 216.18 |
| **[V]** Impulcifer Windows portable ZIP | 211,683,971 | 211.68 |
| **[V]** Impulcifer Windows full update nupkg | 211,699,206 | 211.70 |
| **[V]** Impulcifer macOS DMG | 191,023,439 | 191.02 |
| **[V]** Impulcifer Linux x86_64 AppImage | 273,888,448 | 273.89 |
| **[V]** Impulcifer Linux x86_64 tar.gz | 273,541,383 | 273.54 |
| **[V]** Electron Windows x64 runtime ZIP | 158,199,548 | 158.20 |
| **[V]** Electron macOS arm64 runtime ZIP | 130,326,085 | 130.33 |
| **[V]** Electron macOS x64 runtime ZIP | 134,103,753 | 134.10 |
| **[V]** Electron Linux x64 runtime ZIP | 122,996,717 | 123.00 |

All sizes are release API asset values; MB = bytes/1,000,000. [11,12]

**[I] Windows's bare runtime ZIP is about 75% of the existing portable ZIP's download size.** This supports “Electron is not an order-of-magnitude download regression here.” It does **not** prove the new installer will be 158 MB: the C++ executable/libraries, locales/fonts, updater, plots and app resources still need packaging. Different compressor/container settings invalidate treating the numerical difference as savings.

| Question | Evidence-based answer |
|---|---|
| Is the current Python bundle already large? | **[V]** Yes, 191–274 MB for the cited OS installation artifacts. |
| Can deleting Python/NumPy/SciPy compensate for Chromium? | **[I]** Plausible, especially on Windows/Linux, but exact compression and shared assets require an actual build. |
| Is Electron RAM a wash? | **[U]** No same-machine whole-process-tree measurements. Python scientific imports disappear; Chromium/Node and native buffers remain. The current Windows app already has WebView2 subprocesses, so comparing Electron to only the Python PID would be wrong. |
| Is startup a wash or faster? | **[U]** No measurement. Compiled core can initialize lazily, but Chromium startup and helper-process launch remain. |
| Does WebView2 cost only ~2 MB? | **[V]** No: that is a bootstrapper, not the complete runtime. Evergreen shares one installed runtime; fixed-version binaries exceed 250 MB according to Microsoft. [24] |
| What does switching actually buy? | **[I]** A controlled bundled browser and removal of Nuitka's Python-module-discovery failure modes, at the cost of Chromium security updates and a new native dependency pipeline. |

**[I] Required comparison:** same hardware/OS, same locale/fonts, same release optimization, same demo inputs. Record cold and warm click-to-first-window and click-to-ready separately; sum private bytes/PSS where available across the full process tree, not summed RSS that double-counts shared pages. Measure idle after 30 seconds, peak BRIR/plot memory, installed disk size and installer bytes. Run repeated trials and report median/p95. Neither a framework hello-world nor Discord's memory is a substitute. Electron's own performance guidance recommends app-specific measurement. [25]

### 2.4 Audio I/O and packaging capabilities

**[V] Current recorder uses independent operations:** `record_target():139` blocks in `sounddevice.rec`; `play_and_record():440–445` starts that on a thread; line 465 blocks in playback; line 494 joins recording. Device selection groups host APIs and checks output channel count. [L2]

**[V] PortAudio supports separate input/output parameters, multichannel float32, blocking `Pa_ReadStream`/`Pa_WriteStream` when opened without a callback, and `Pa_IsFormatSupported`. Device/channel/rate support is hardware-dependent. It does not perform sample-rate conversion itself. A duplex stream requires one host API for its input/output. Its overview documents only one ASIO device open at a time, not a blanket guarantee for independent ASIO streams.** [28]

**[I] Native implementation:** enumerate host/device IDs and capabilities; validate the exact 16-out/2-in format/rate before recording; start capture before playback; write/read bounded blocks on separate threads; drain playback, join capture, then save. Keep the public operation blocking until both finish. Do not force duplex or insert resampling for convenience. Cancellation can be checked between blocks; backend stop/abort behavior and underflow/overflow must be surfaced explicitly.

**[U] Hardware qualification remains mandatory:** combinations of two devices, two host APIs, WASAPI shared/exclusive routing, 96 kHz/16-channel operation, and ASIO's ability to meet the two-independent-stream contract are not certified by the API documentation. If ASIO requires a contract change for a target driver, seek a specific exception; do not silently convert all recording to duplex.

#### naudiodon

| Finding | Implication |
|---|---|
| **[V]** npm latest **2.3.6**, published **2021-11-30**; master manifest **2.4.0**, latest inspected master commit **2022-03-08**. README recommends development/prototype use rather than production. [29] | **[I]** Bad new product dependency for a 2026 rewrite; maintained upstream PortAudio directly is preferable. No declaration of permanent abandonment was found. |
| **[V]** Current master uses raw `node_api.h`/`NAPI_MODULE`, not NAN; installs through `node-gyp rebuild`. [30] | **[I]** Do not justify rejection with an invented NAN ABI problem. Stale delivery/support and opaque bundled backend configuration are sufficient reasons. |
| **[V]** Issue #49 reports a repeated-playback crash; the reporter explicitly was not using Electron. [31] | **[I]** Evidence of a historical addon problem, not evidence of an Electron ABI crash or current reproducibility. |
| **[U]** Bundled Windows DLL's complete WASAPI/ASIO configuration was not established; an ASIO fork is not upstream support. | **[I]** Avoid it; otherwise test the actual shipped binary's host-API list and real streams. |

**[I] Browser audio and an npm audio addon are unnecessary layers when C++ already owns the pipeline.** Keep all device I/O and sample buffers in the core. If callbacks are needed for a particular backend, they must not wait for JS, allocate unpredictably or perform file/IPC work. The existing requirement does not demand low latency, so blocking block-I/O should be the first implementation. [28,32]

### 2.5 C++ libraries and the actual DSP gap

| Library / inspected version | Verified capability | Recommendation or unresolved work |
|---|---|---|
| Eigen **5.0.1** vcpkg | **[V]** C++ linear algebra, all listed triplets. [33] | **[I]** Good vector/matrix building block; not a replacement for SciPy signal or its bounded optimizer. Pin deliberately rather than assuming 3.x examples all describe 5.x. |
| pocketfft vcpkg **2024-11-30** | **[V]** Header-oriented package without dependencies; upstream float/double/long-double FFTs, caller-supplied normalization, thread controls, Bluestein. [34] | **[I]** Preferred FFT. Test normalization, real/complex layout, odd lengths and frequency ordering; related algorithm ancestry does not guarantee NumPy/SciPy bit identity. |
| KissFFT **131.2.0** | **[V]** Available in vcpkg; upstream default scalar is float, `KISSFFT_DATATYPE=double` changes it. [35] | **[I]** Credible smaller alternative, but accidentally accepting default float would violate R2. Choose one FFT library, not both. |
| PortAudio vcpkg **19.7#9** | **[V]** `!uwp`; optional `asio` feature adds `asiosdk`. Recipe pins commit `147dd722548358763a8b649b3e4b41dfffbcfbb6`. [36] | **[V]** Crucial mismatch: non-Windows/non-Apple recipe explicitly sets `PA_USE_ALSA=OFF`, `PA_USE_JACK=ON`. **[I]** Default vcpkg dependency is not sufficient for R1. |
| PortAudio upstream CMake project **19.8** | **[V]** Windows defaults enable WASAPI/MME/DirectSound; ASIO off. Unix ALSA/JACK/PulseAudio depend on discovery; Apple uses CoreAudio. [37] | **[I]** Pin a known commit/release and maintain an overlay recipe with explicit backend features. Project version 19.8 is not by itself proof of a released 19.8 tarball. |
| libsndfile **1.2.2#2** | **[V]** Float WAV/WAVEX/RF64 formats; double-buffer read/write with conversion; channels set through `SF_INFO`; validate with `sf_format_check`. Optional compressed codec dependencies. [38] | **[I]** Use float32 WAV/WAVEX for 32 tracks, RF64 if needed by size/consumer contract. Disable unnecessary codec features; ffmpeg stays external. Test downstream readers and channel masks/order. |
| Ceres Solver, unversioned documentation | **[V]** Bounded nonlinear least squares via trust-region methods; documented strategies are LM and Dogleg, not SciPy TRF. [39] | **[I]** Candidate optimizer, not a drop-in semantic replacement. **[U]** No verified identical SciPy-TRF implementation selected. |

**[V] AutoEQ's local optimization is more than “fit a few biquads.”** It smooths the target, detects positive/negative peaks, inserts low-frequency seeds, merges same-sign filters with a 0.3 dB rule, optimizes log-frequency/log-Q/gain with bounds, limits evaluations with `max(20, int(max_time*120))`, filters gains below 0.1 dB and sorts frequencies. Its `least_squares` call uses default method selection. [L3]

**[I] Port the surrounding initialization/merging/filter-selection logic exactly before changing solvers.** Fit-response tolerances are more meaningful than identical parameter vectors for non-unique filters, but a different solver can cross 0.1 dB thresholds and change filter count. That needs explicit acceptance, not silent tolerance inflation.

| R2 group | Implementation plan and parity hazard, all **[I]** |
|---|---|
| FFT, convolution, correlation, `next_fast_len` | pocketfft double wrapper plus explicitly tested convolution/correlation/cropping/lag conventions. FFT length selection changes rounding and memory; retain the original rule where practicable. |
| Butterworth + SOS, RBJ biquads | Implement or port narrowly scoped designs; test pole ordering, SOS scaling, initial states and recurrence. Local `autoeq/biquad.py` stores denominator signs in a special convention; `digital_coeffs` flips them back. Do not substitute textbook coefficients blindly. [L4,L5] |
| `firwin2`, homomorphic minimum phase | Explicit compatibility implementations with Nyquist endpoint, interpolation grid, window, FFT length and cepstral lifter tests. |
| Savitzky–Golay, peaks, Hann/Kaiser/window factories, uniform filter | Test edge extension, default modes, plateau/tie behavior, axis semantics and even/odd lengths. Small implementations are still specification work. |
| Linear/cubic log-frequency splines | Match log10 input, knot/end conditions and extrapolation. A generic cubic interpolator is not automatically equivalent to `InterpolatedUnivariateSpline(k=3)`. |
| Kaiser polyphase resampling | Pin rational factors, filter length, gain scaling, delay compensation and padding; compare impulse/sweep/output lengths. Do not substitute an arbitrary audio resampler. |
| Regression, expit, spectrogram | Implement stable formulas and document exact regression/window/overlap/detrend/scaling defaults. |
| Bounded biquad fit | Ceres with explicit parameterization/objective/bounds is a spike; retaining SciPy temporarily is a safer transition until response-level acceptance is agreed. |

**[I] There is no verified four-library C++ equivalent of the complete R2 list.** Eigen + FFT + PortAudio + libsndfile solves infrastructure, not the signal-processing port. This is the largest estimate trap in S2.

### 2.6 Updater, signing, and installers

| Option | Verified state | Fit |
|---|---|---|
| `electron-updater` npm stable **6.8.9** | **[V]** Integrated with electron-builder; Windows NSIS, macOS DMG plus update ZIP, Linux AppImage and other package targets. Squirrel.Windows is not its supported Windows update path. Current website also contains v27/next and updater-7 material; do not apply those features to 6.8.9 without checking. [16] | **[I]** Straightforward greenfield choice, especially macOS DMG+ZIP and Linux AppImage. Existing Windows Velopack installation cannot be assumed to migrate automatically. |
| `velopack` npm **1.2.110-ge826545** | **[V]** Official JavaScript/Electron guide; contains a native `.node` module. Initialize `VelopackApp.build().run()` early in main; stage with `waitExitThenApplyUpdate(...)`, then quit Electron. JS lacks one combined install/quit/relaunch helper. [17] | **[I]** Keep it for Windows to reduce installer transition risk. This keeps a native SDK dependency, but not a DSP addon. |
| Velopack other OSes | **[V]** macOS packaging documents PKG+ZIP; cross-platform JS support is documented. [18] | **[I]** Do not assume it emits the required DMG automatically. A DMG wrapper/distribution job is additional work, or use electron-builder/updater on macOS/Linux. |

**[I] Recommended initial policy:** retain Velopack on Windows; use electron-builder's DMG+ZIP/update integration on macOS, AppImage on Linux, behind one JS update adapter. This creates two installer backends, but the existing product already has OS-specific updater behavior. Reevaluate a single updater only after migration and signing tests; consistency of dependency names is not worth breaking installed users.

**[V] Signing automation exists, not signing credentials.** electron-builder supports certificate inputs and notarization; Velopack has Windows signing templates/parameters and macOS identities/notary profile support. Velopack's example/default entitlements are not proof of correctness for Electron's helpers. [19–23]

**[I] CI release steps:** compile and stage native binaries; package the app; sign nested frameworks/addons/helpers/sidecar and the outer app; notarize/staple if enabled; create installer/update metadata; smoke-install; publish only matching artifacts. Keep certificates/API keys in protected GitHub environments, never untrusted PR jobs. Windows signing identity must remain compatible with updater publisher validation. Shutdown must wait for or cancel native work before the updater replaces files.

**[U] Existing installation migration is untested:** package ID, executable name, shortcuts, old files, settings directory and restart arguments must be checked from a real currently installed release. Do not promise a seamless Python-to-Electron self-update based solely on Velopack SDK availability.

### 2.7 Frontend reuse, type checking, and security

**[V] No bundler is required.** Existing JS can be checked with TypeScript using `allowJs`, `checkJs`, `noEmit`, with JSDoc and ambient declarations. That checks source; it does not require shipping TypeScript or transpiled output. [26]

**[I] Practical setup:** separate renderer DOM types from Electron-main/preload Node types, define `Window.impulcifer` in a small declaration file, and annotate the result envelopes and job unions. Existing generic DOM lookups need narrowing to `HTMLInputElement`/`HTMLSelectElement`; implicit state fields need declarations. A first `tsc --checkJs` pass will not be clean automatically. Runtime JSON validation remains necessary.

**[I] A no-bundler renderer is not a no-npm desktop build.** Electron/package/signing/type-check tools are build dependencies. Velopack or electron-updater remains runtime JS/native code, whether stored as module directories or ASAR entries. Keep lockfiles and minimal runtime dependencies; do not sell this as literally “only a type check in CI.”

| Security control | Proposed implementation / evidence |
|---|---|
| Renderer privileges | **[V/I]** Explicit `nodeIntegration:false`, `contextIsolation:true`, `sandbox:true`, `webSecurity:true`; no Node-enabled renderer workers. These are documented defaults for key settings but should be fixed in configuration. [27] |
| Preload | **[V/I]** `contextBridge.exposeInMainWorld` with one wrapper per permitted method. Never expose `ipcRenderer`, arbitrary channel names, `fs`, `child_process` or a generic shell command method. [27] |
| IPC sender and payload | **[V/I]** Validate trusted top-level sender frame/origin, method schema, lengths, finite numbers and paths in main. Validate again at the sidecar's public interface; the CLI bypasses Electron. [27] |
| Content and navigation | **[V/I]** App-specific protocol serving only packaged resources; prevent path traversal, arbitrary file serving and remote navigation. Deny unexpected windows/iframes/webviews. Restrictive CSP, no remote executable JS. [27] |
| URLs / updates | **[I]** Allowlist `https:` destinations; do not forward arbitrary URI schemes to `shell.openExternal`. Main owns update source/version selection. Existing `start_update({download_url,...})` must not become a renderer-controlled executable download API. |
| Reports | **[I]** Imported filenames/configs/logs are untrusted strings. Escape HTML and JSON, keep generated analysis HTML separate from the privileged app origin, and never give it the preload API. |
| ffmpeg | **[I]** Download to a controlled application cache; verify pinned source/checksum when available; run explicit executable/argument arrays without a shell; limit outputs and clean up child processes. Do not mistake its own codec sandboxing for an application security boundary. |
| Maintenance | **[V/I]** Electron supports the latest three stable major lines, with roughly eight-week major cadence. Keeping a fixed old Electron forever is not an acceptable packaging simplification. [43] |

**[I] Security cost is bounded but recurring:** approximately 22 method contracts to validate, navigation/protocol tests, updater trust boundaries, native dependency scanning, and Chromium update regression runs. A local-only page reduces exposure; it does not eliminate hostile imported data, shell URI handling or compromised update content.

**[V/I] R5 is straightforward but not completely automatic:** Electron has native asynchronous file/directory dialogs. `nativeTheme` controls dark/light/system appearance; Windows dark title-bar behavior should be verified rather than assumed. A documented `titleBarStyle:'hidden'` plus `titleBarOverlay` can supply dark native controls with an HTML draggable header; it requires layout and accessibility testing. macOS microphone permission requires `NSMicrophoneUsageDescription` and appropriate permission handling. Sidecar ownership of that permission prompt and hardened-runtime entitlements need packaged testing. [45]

### 2.8 CMake, vcpkg/Conan, and build matrix

**[I] Use C++20 + CMake + pinned vcpkg manifest/overlay ports.** MSVC on Windows is the least surprising first shipping toolchain for Windows SDK, PortAudio/ASIO and signing; Apple Clang on macOS; a pinned GCC or Clang toolchain on a conservative Linux baseline. Cross-platform source does not imply binary compatibility between those toolchains.

**[V] vcpkg supports manifest-mode acquisition via `CMAKE_TOOLCHAIN_FILE`, CMake presets, triplets and app-local DLL deployment. Conan 2 offers profiles, CMakeDeps/CMakeToolchain and binary reuse.** [40,41]

**[I] vcpkg wins narrowly for this Windows-first application because the required base ports are present and CMake integration is direct.** Conan 2 is also reasonable; no evidence makes it technically incapable. Both need exact compiler/runtime/architecture settings and native deployment checks. Do not manage the same dependency with both. The PortAudio overlay is necessary either way if the stock recipe omits required backends.

| Target | Sidecar build and test | Packaging | Extra if DSP addon chosen |
|---|---|---|---|
| Windows x64 | **[I]** MSVC, CTest, DLL dependency scan, WASAPI/MME/DS capability smoke; real audio on hardware | Velopack installer and upgrade-from-current test | One Node-API x64 binary plus dependent DLLs; Windows delay-load/load test |
| macOS arm64 | **[I]** Apple Clang, arm64 library closure, CoreAudio test | Signed app, DMG+update ZIP, microphone permission/notarization test | arm64 Node-API binary and dylib install-name/signing closure |
| macOS x64 | **[I]** Separate build if retained, or cross-build plus native test capacity | Separate artifact initially; universal binary only if worth the doubled native dependency merge work | x64 addon; universal packaging must merge all relevant binaries |
| Linux x64 | **[I]** Pinned baseline, explicit ALSA/Pulse/JACK configuration and runtime library audit | tarball and optional AppImage, headless CLI smoke | libc-compatible `.node` plus shared objects; separate musl target only if promised |

**[I] Minimum shipping matrix is four OS/architecture targets if both Mac architectures are supported, or three if macOS is arm64-only.** Pure Node-API needs approximately those same binary targets, not four times every Electron major. V8-bound modules multiply by supported Electron ABIs. Optional Windows ARM64/Linux ARM64/Python increase scope; no reason to promise them on day one.

**[I] CI strategy:** core compile/CTest and JSDoc/lint/DOM contract tests on PRs; ASan/UBSan and race testing in suitable non-release configurations; packaged renderer+sidecar launch tests on each OS; real audio matrix on self-hosted hardware. Cache vcpkg artifacts by triplet/compiler/baseline/features. CI cannot certify a 16-channel device without owning one. Keep fast-math disabled and thread budgets controlled during numerical qualification.

### 2.9 PyPI continuity

**[V] Both pybind11 and nanobind work with scikit-build-core/CMake.** pybind11 requires explicit GIL release for long native calls; declaring free-threading support does not make C++ shared state safe. [46]

**[V] Version-sensitive detail:** nanobind **3.0.1** was released in late August 2026 (changelog dated August 28; registry date August 27). Conventional `STABLE_ABI` builds target Python 3.12+; nanobind 3 introduces optional split mode with a Python-3.10-stable-ABI frontend and separate Python-version-dependent backend. Conventional builds remain the default. Python 3.13/3.14 free-threaded wheels must not be assumed interchangeable with normal `abi3`; latest docs also discuss provisional 3.15+ `abi3t`, which is not a reason to promise it now. [47]

**[I] Recommend nanobind conventional bindings first, not a new 3.0 split-mode dependency arrangement during a major rewrite.** Bind the same static core, release the GIL for computation, return explicit arrays/errors, keep optional audio dependencies out of a DSP-only wheel, and make Python plotting an optional extra. Compare pybind11 if its familiarity and existing documentation lower verification effort more than nanobind lowers binding overhead; performance of this thin binding is unlikely to dominate the DSP.

| Policy | Illustrative release-build cost, **[I]**, not measured runner minutes |
|---|---|
| CPython 3.10–3.14 × Windows x64/macOS x64/macOS arm64/manylinux x64 | 5 × 4 = **20 wheel build targets** |
| Add 3.13t and 3.14t on those targets | Up to **8 additional targets** |
| Traditional nanobind `cp312-abi3` for 3.12+, plus 3.10 and 3.11 regular wheels | 3 × 4 = **12 binary builds**, with tests across supported Python versions still needed |
| Adopt 3.0 split frontend | Could reduce frontend wheel targets, but moves version-specific work to a separately delivered backend; not free elimination of compatibility work |

**[V] cibuildwheel automates platform/architecture wheels and repair tooling.** Current docs evolve quickly around free-threaded/default-enabled targets; pin the tool and explicitly select identifiers instead of inheriting every new target. Linux uses auditwheel; macOS uses delocate; Windows DLL repair is available through delvewheel (current v4 documentation describes it as default). [48]

**[I] PyPI continuity is feasible, but keep desktop release gating distinct from the optional wheel matrix.** Users who only want headless DSP should not need Electron, a browser, Node or the updater installed. Existing CLI option spellings/defaults and Python public entry points need compatibility tests; “same C++ core” alone does not preserve the Python API.

### 2.10 Plotting is a separate product requirement

| Approach | Benefits | Cost / verdict |
|---|---|---|
| Renderer canvas/Plotly from reduced analysis arrays | **[V]** Plotly supports downloaded script-tag integration without a bundler and PNG export through `toImage`; Electron can capture a page as a NativeImage. [49,50] | **[I]** Best GUI/interactive HTML path. Keep plotting data, not raw 32-track recordings, in the renderer. Browser output is not a headless-native CLI solution by itself. |
| `Float64Array` over Electron IPC | **[V]** Electron `send/invoke` uses structured clone; documented `postMessage` transfer list is MessagePorts. [51] | **[I]** Do not promise end-to-end zero-copy ArrayBuffer transfer. JS-worker transfer APIs are not the same as Electron main/renderer IPC. Moderate reduced traces can be copied; big matrices should use artifact files and bounded loading. |
| node-canvas stable **3.2.0** | **[V]** Cairo/Pango source-build dependencies; stable prebuild list covers Windows x64, Linux glibc x64, macOS x64/arm64. Current repository main advertises a different v4 prerelease build/prebuild story. [52] | **[I]** Valid server-side canvas, but adds a native addon and text stack to solve rendering that Chromium already supplies. Requires a Node runtime for CLI PNGs. Reject as default. |
| C++ Cairo/Pango | **[V]** Cairo exports PNG from surfaces without a windowing requirement. vcpkg Cairo **1.18.4#1**, Pango **1.58.0**; Pango adds GLib/Harfbuzz/Fontconfig/Freetype/etc. [53] | **[I]** Provides genuinely headless native PNG. It is a drawing/text API, not matplotlib: axes, legends, ticks, layouts, waterfall/heatmaps and plot semantics must be implemented. This is substantial work. |
| Matplot++ / gnuplot | **[V/U]** Matplot++ is a C++ plotting library, but fetched README did not establish its default/backend details. Gnuplot docs describe `pngcairo` and palette-mapped surfaces; direct page fetch hit a certificate mismatch. [54] | **[I]** Candidate for a prototype, not a verified drop-in. Usually entails a plotting runtime/backend and fonts; do not describe it as an already qualified single-header native PNG solution. |

**[I] Recommended endpoint:** one core-generated analysis specification (axes, units, series, heatmap grids, annotations, metadata), two renderers: native Cairo/Pango for CLI PNGs and a locally vendored Plotly renderer for interactive HTML/desktop inspection. Implement stacked 2D waterfall traces before introducing GPU-dependent 3D. Validate that every current IACC/ILD/IPD/decay view has an explicit equivalent. This duplicates drawing, not DSP; document and test the shared plot specification.

**[I] Safer migration:** preserve existing Python/matplotlib/Bokeh reporting as an optional Python extra while the C++ pipeline is qualified. Do not remove it from the full product until native headless PNG and interactive report coverage pass. A hidden Electron window is an interim GUI export facility, not a promise that the standalone CLI works on headless Linux without a browser/display stack.

**[U] No proof of pixel-identical font/layout output across Cairo and Chromium, or across OSes.** Bundle licensed fonts/subsets for nine languages, record dimensions/units/ticks/series in tests, and use per-platform visual baselines with tolerances. PNG hashes must not be used as evidence of numerical DSP correctness.

### 2.11 Real shipped applications, not library demos

| Application | Verified implementation / version evidence | Reported pain and limits |
|---|---|---|
| Discord | **[V]** Official 2018 engineering account: Electron desktop client and shared native C++ WebRTC media engine; 2024 account still distinguishes native voice modules. Exact current addon API/version not public in the inspected sources. [55] | **[V]** 32-bit address-space exhaustion affected reliability; Chromium limits did not limit native voice memory. 64-bit migration exposed a C++↔Rust type-width ABI mismatch. **Do not mislabel that as Electron `NODE_MODULE_VERSION` churn or claim it was specifically in voice.** |
| Signal Desktop | **[V]** v8.26.0 manifest: Electron **43.5.0**, RingRTC **2.71.0**. Actual chain is Electron → Rust/Neon RingRTC addon → C++ WebRTC, not a pure C++ addon. [56] | **[V]** Build tooling includes Rust/Node/Protobuf/CMake and platform native binaries. Historical issue #4596 reported shipping all OS RingRTC binaries inside a Linux app (108 MB download, 104 MB ASAR in that 2020 report). **Not a 2026 memory benchmark or an established ABI failure.** |
| Streamlabs Desktop | **[V]** Official README describes Electron and C++ npm native modules. Source loads `obs_studio_client.node` and points to separate `obs64` backend/IPC: hybrid addon client + native process. Inspected master manifest: app **1.21.9**, Electron **29.3.1**, not independently confirmed latest commercial release. [57] | **[V]** Platform OBS binaries/toolchains and documented macOS packaging/signing issue around an `OSN.app` install prefix. **[U]** No specific verified Electron-upgrade ABI failure or native memory leak established in this research. |
| LivePlay | **[V]** v2.4.3, **2026-07-29**, actual DMG/AppImage/ZIP release assets; manifest Electron **42.3.2**. Electron/Vue client uses a separate C++20 `liveplay-server` for audio mixing/limiting/waveforms via REST/WebSocket. [58] | **[V]** README acknowledges no automated tests and no signing/notarization. **[U]** No verified ABI/rebuild/memory incident. **[I]** Useful shipped sidecar precedent, not evidence of enterprise-grade reliability. |

**[I] What these examples prove:** Electron can host a serious native audio product, including a process-separated engine. They do not prove that a solo maintainer inherits Discord's engineering budget, that all audio addons need rebuilding on every Electron update, or that total RAM is negligible. LosslessCut/FFmpeg was not used to inflate the C++ count: FFmpeg is principally C and a sidecar, not evidence of a C++ Node addon.

## 3 Proposed architecture

Everything in this section is **[I] proposed design**, except source facts explicitly tagged otherwise.

### 3.1 Repository and ownership

```text
CMakeLists.txt                 # independent native core build
CMakePresets.json
vcpkg.json                    # pinned baseline/features
ports/portaudio/               # explicit host-API overlay, tested per OS
cpp/
  include/impulcifer/          # config, job, audio, result, cancellation types
  src/dsp/                    # double-precision, no Node/Electron/Python types
  src/pipeline/               # stage table, deterministic config/defaults
  src/audio/                  # PortAudio + libsndfile, device/backend adapters
  src/analysis/               # IR/FR/ILD/IPD/IACC/spectrogram analysis spec
  src/plots/                  # headless Cairo/Pango renderer
  src/service/                # jobs, cursors, JSON validation, stdio protocol
  src/cli/                    # CLI and --serve-stdio entry point
  tests/                      # primitive/stage/parity/protocol/audio-backend tests
bindings/python/              # thin nanobind + scikit-build-core distribution
webview_ui/                   # reuse existing index.html/app.js/styles.css
  vendor/plotly-<pinned>.js    # offline report renderer, no CDN dependency
  report-viewer.js
  types/bridge.d.ts
  tsconfig.json
 desktop/
  main.cjs                    # trusted shell/bootstrap/child lifecycle
  preload.cjs                 # fixed method allowlist
  ipc-contract.cjs            # request validation + adapters
  updater/                    # Velopack Windows; builder updater other OSes
  package.json
  package-lock.json
 i18n/locales/                # same catalogs, schema/key/placeholder tests
 tests/golden/                # versioned numerical arrays + stage metadata
 tests/frontend/              # existing contract/gallery tests adapted
 tests/package/               # install, upgrade, native child, report export
```

Indentation before `desktop/`, `i18n/`, and `tests/` is visual only; these are repository-root directories. Core library is a static target linked independently by CLI/service and Python extension. Desktop contains no DSP implementation. Pure core tests run without audio hardware or Electron.

### 3.2 Process model

| Process/thread | Responsibility |
|---|---|
| Electron renderer, sandboxed | Existing forms, settings, progress/log display; no filesystem/process access. Report preview only from controlled analysis data. |
| Electron main | Native dialogs, approved open-path/open-url, settings/locales, update checks, permission prompts, child launch/restart, validated IPC. No compute or synchronous file traversal that can block for minutes. |
| `impulcifer-core --serve-stdio` child | Owns DSP/audio jobs, validates config, maintains job journal, schedules bounded speaker parallelism and writes artifacts. |
| Core protocol reader + serialized writer | Always services poll/cancel while compute runs. No compute under journal locks. Writer never prints diagnostic prose into protocol stdout. |
| Core job coordinator + bounded pool | One active recording/BRIR job; per-speaker parallel tasks, single-thread FFTs within tasks initially, stable output ordering. |
| Recording threads | Independent capture and playback; core owns lifetimes, block counts, errors and stream closure. |
| Standalone CLI | Same core directly, no JSON IPC, Electron or Node required. |

Spawn the core from a fixed installed resources path outside ASAR; no PATH search and no shell. Parent EOF means shutdown/cancel unless a future explicit persistent-job mode is introduced. Mark interrupted jobs failed after child death, do not silently rerun recording. Renderer reload may resume polling the still-running child, matching current behavior.

### 3.3 IPC contract

Preserve renderer-facing methods and envelopes through the preload adapter. Representative wire messages:

```json
{"jsonrpc":"2.0","id":1,"method":"hello","params":{"protocol_major":1,"client_version":"3.0.0"}}
{"jsonrpc":"2.0","id":1,"result":{"protocol_major":1,"core_version":"3.0.0","capabilities":{"audio":true,"png":true}}}
{"jsonrpc":"2.0","id":2,"method":"start_brir","params":{"config":{"dir_path":"...","test_signal":"auto"}}}
{"jsonrpc":"2.0","id":2,"result":{"ok":true,"data":{"job":{"job_id":"opaque-id","kind":"brir","status":"queued","cancellable":true}}}}
{"jsonrpc":"2.0","id":3,"method":"poll_job","params":{"job_id":"opaque-id","after_seq":17}}
{"jsonrpc":"2.0","id":4,"method":"cancel_job","params":{"job_id":"opaque-id"}}
```

The example version is illustrative, not a release decision. Framing is one complete UTF-8 JSON object per newline; escaped newlines remain inside strings. Start with a **1 MiB message limit** and a bounded event page; capability-negotiate any extension. Parse incrementally across arbitrary byte chunks. Domain errors retain `{ok:false,error:{code,message,details}}`; malformed protocol requests use JSON-RPC errors. Application errors are not process failures.

Journal events retain `seq/type/payload`; `next_seq` means the last **delivered** event when pagination is active, not the newest undisclosed event. Snapshot and event-page boundaries are read consistently. Retain full in-session journals within a documented byte budget or introduce an explicit `first_available_seq/gap` response. The current renderer does not handle truncation, so never drop history silently while advancing its cursor.

No raw PCM in control JSON. Native artifacts include `{artifact_id, kind, dtype, shape, byte_length, sha256}`. The main process maps trusted artifact IDs to files under the job output/cache directory. Binary preview data is little-endian float64 with explicit shape/sample rate/channel order. Enforce size limits before allocation; encode nonfinite analysis values as masked/null metadata where JSON is used.

### 3.4 Threading, cancellation and output integrity

- All DSP arrays are double until deliberate float32 playback/WAV conversion. Immutable config shared among workers; mutable state belongs to a speaker/job.
- Use C++20 stop tokens or equivalent native atomics. Check before/after each stage, between convolution/resampling blocks and at optimizer iteration boundaries. A library kernel without interruption points defines cancellation latency; report that honestly.
- `cancel_job` changes status to `cancel_requested` immediately, but only coordinator completion changes it to `cancelled`. Stop scheduling new speakers; join running tasks and close audio streams safely.
- Recording API remains blocking from the caller's perspective. If certain backend operations cannot safely cancel, advertise `cancellable:false`, as today's service already allows, rather than claiming cancellation works.
- Output goes to temporary names, with successful files atomically promoted where feasible. Preserve unrelated existing output; do not treat a multi-file directory update as magically atomic. Manifest indicates complete/partial artifacts after failures.
- Hard-killing the sidecar is a last-resort user-confirmed recovery action, not ordinary cancellation. Account for ffmpeg grandchildren and crash leftovers.
- Clamp parallelism by estimated memory, not only CPU count. Start with one outer speaker pool and one FFT thread per task to avoid nested oversubscription.

### 3.5 Plot and report contract

Core calculates every numerical series once. Native PNG renderer and offline HTML renderer consume the same versioned analysis specification. Required chart tests enumerate frequency response, impulse response, spectrogram, waterfall, decay, interaural overlay, ILD, IPD and IACC layouts; absence is a feature regression, not optional polish.

Use bounded display decimation that preserves peaks/extrema; never use decimated data for exported CSV or numerical comparisons. Keep dense spectrogram grids in separate bounded artifacts. Load interactive report scripts locally at a pinned version; no network required. Generated reports run without the app preload, and text is escaped. Reuse fixed fonts/units/axes across renderers, accepting platform-specific rasterization differences.

### 3.6 Numerical parity and adoption gates

**[V]** Existing optimizer and coefficient conventions are verified from the local files; byte-level outputs depend on concrete implementations and arithmetic order. [L3–L5] **[I]** Do not keep “identical to Python SHA-256” as the cross-language release criterion. Retain Python as an oracle during migration, then establish deterministic per-build C++ regression baselines where attainable.

| Gate | Proposed test |
|---|---|
| Exact structural parity | File sample rate, frame count, channel labels/order, output naming, shape/dtype, CLI defaults, omitted-vs-explicit configuration, peak/lag indices when the fixture has an unambiguous peak. |
| Primitive goldens | Small analytic cases and representative real arrays for FFT normalization, convolution crop, correlation lag, SOS sign/state, spline endpoints, FIR/minimum phase, resampling delay/length and window edges. Compare absolute + relative error with units. |
| Stage goldens | Save inputs/outputs and metadata after sweep estimation, deconvolution, cropping/alignment, room/headphone correction, EQ, virtual bass, decay, normalization and WAV conversion. Include silence, low-SNR, asymmetric ears, odd lengths and all sample rates. |
| Solver parity | Compare final EQ response/RMSE, bounds, poles/stability, filter-selection behavior and convergence failures. Do not require identical center-frequency/Q vectors for equivalent fits; do not accept worse fit silently because coefficients differ. |
| Acoustic output | Magnitude response, unwrapped phase where meaningful, ITD, ILD, IACC, decay measures, clipping/headroom and time-domain residuals. Mask phase/deep-null relative-error tests below documented signal floors. |
| Initial thresholds, not proven promises | Start primitive well-scaled double tests around `atol=1e-12, rtol=1e-10`; use an initial **0.01 dB** above-floor FR alert. Calibrate each stage against measured numerical behavior and task sensitivity; these are investigation thresholds, not a blanket waiver. |
| Reproducibility | Fixed compiler/lib versions, no fast-math, controlled reduction order/thread count; repeat C++ runs on each platform. Record SHA-256 as a per-environment regression signal and metadata checksum, not universal cross-compiler equality. |
| Migration gate | Qualify defaults/headphone compensation and virtual-bass paths separately; add all remaining feature flags/formats. Compare against a pinned verified Python commit and the same source inputs, not whichever master happens to be latest. |

Recommended sequence: first core FFT/deconvolution + solver spike and golden corpus; second CLI end-to-end parity; third independent 16-output/2-input hardware test; fourth frontend bridge/packaging and upgrade test; fifth full plot coverage; only then remove the Python runtime from desktop distribution. No full rewrite should be approved on a hello-world Electron installer alone.

## 4 Risks ranked

All rankings are **[I]** judgments; evidence refers to section 2.

| Rank | Risk | Severity / likelihood | Mitigation / go-no-go |
|---:|---|---|---|
| 1 | SciPy semantics and nonlinear optimizer divergence | Critical / high | Golden primitives/stages and bounded-fit spike before UI work. Stop if required output fidelity cannot be expressed and verified. |
| 2 | Native audio/backend build differs from required hardware behavior | Critical / high | PortAudio overlay, actual host-API enumeration, 16-out/2-in at 44.1/48/96 kHz on target devices. Stock Linux vcpkg port already fails the intended backend configuration. |
| 3 | PNG/Bokeh replacement underestimated | High / high | Explicit plot inventory and headless export tests. Retain Python reporting temporarily; do not hide browser dependence in the CLI. |
| 4 | Native packaging/signing/update migration | High / medium-high | Real installed upgrade tests, DLL/dylib audit, microphone permissions and signed nested binaries. Preserve Windows Velopack identity. |
| 5 | C++ memory lifetime/race bugs under parallel cancellation | High / medium-high | RAII, per-job ownership, sanitizers, stress cancellation/shutdown, memory budget and fault-injection tests. Sidecar limits crash consequences, not memory corruption within the core. |
| 6 | Electron privileged bridge/update/URL attack surface | High / medium | Minimal allowlist, sender/schema validation, isolated reports, CSP, no shell, security release cadence. |
| 7 | Maintenance expansion from optional Python wheels/addons | Medium-high / high if all enabled | Sidecar for desktop; optional thin bindings; explicit ABI/platform support policy. Avoid shipping DSP addon + sidecar + Wasm simultaneously. |
| 8 | Unproven footprint/performance benefit | Medium / certain measurement gap | Same-machine packaged benchmark and memory stress test. Large current download is not a RAM result. |
| 9 | Dependency/version/documentation drift | Medium / high over time | Pin CMake/vcpkg/npm/Python build tools. Distinguish stable tags from latest docs (nanobind 3, node-canvas v4 prerelease, updater 7/next). |
| 10 | License and redistribution omissions | Medium / medium | Audit libsndfile, text stack/fonts, ASIO SDK, Electron notices and external ffmpeg builds; honor license obligations for chosen linkage. Do not infer a stock vcpkg feature grants redistribution rights. |

## 5 Open questions you could not settle

1. **[U] Measured footprint:** final C++/Electron installer/installed bytes, idle/peak process-tree memory, cold/warm startup and plotting costs. No executable was built or downloaded in this research.
2. **[U] Audio devices:** whether the maintainer's interfaces support two independent streams with required channel counts/rates, especially ASIO. CoreAudio permissions under a signed Electron parent plus native child need real packaging tests.
3. **[U] Optimizer selection:** no tested C++ replacement matching SciPy TRF's decisions/termination closely enough for the existing biquad corpus. Ceres provides bounds, not identical algorithm semantics.
4. **[U] Native numerical coverage estimate:** no audited implementation was found that covers the full R2 list with SciPy-compatible edge behavior. Engineering time cannot be responsibly estimated from Eigen/pocketfft availability alone.
5. **[U] Plot renderer qualification:** no tested native implementation of every current spectrogram/waterfall/decay/IACC layout, no settled cross-OS font behavior, and no measured Cairo/Pango packaging delta.
6. **[U] Update migration:** Python Velopack installation to Electron package, executable/shortcut/settings preservation, downgrade/rollback and current signing identity were not exercised. Velopack JS availability does not establish this compatibility.
7. **[U] Scope:** Mac x64 support, Windows ARM64, Linux distribution baseline and Python version/ABI promises remain maintainer decisions; they materially change CI cost.
8. **[U] Performance:** no native-vs-Wasm FFT/optimizer benchmark for this workload; memory64/pthreads compatibility of an actual Emscripten build is untested.
9. **[U] Real-app incident attribution:** verified memory/packaging issues were found, but not a representative measured rate of Electron-ABI upgrade breakage for all four example products. Do not invent such a rate.
10. **[U] Tool access:** shell-based temp-variable and `gh` API checks were blocked. Release facts came from public web/API reads, not a local `gh` result. The report destination uses the Windows temporary directory shown in the tool-generated task paths. No repository file was written.

## 6 Sources (numbered URL list)

Numbered groups contain the exact primary pages supporting their adjacent claims. All consulted 2026-09-07; stable versions are stated above where confirmed. GitHub default-branch and `latest` documentation are mutable.

1. Node-API, ABI scope, environments and async APIs: https://nodejs.org/api/n-api.html
2. Node addons, context awareness and worker support: https://nodejs.org/api/addons.html
3. CMake.js, Node-API and Electron targets: https://github.com/cmake-js/cmake-js
4. Electron native modules, rebuilds and Windows loading: https://www.electronjs.org/docs/latest/tutorial/using-native-node-modules
5. prebuildify, Node-API/runtime target modes: https://github.com/prebuild/prebuildify
6. node-addon-api AsyncWorker, execution and cancellation: https://github.com/nodejs/node-addon-api/blob/main/doc/async_worker.md
7. Node worker_threads: https://nodejs.org/api/worker_threads.html
8. Emscripten pthreads and memory settings: https://emscripten.org/docs/porting/pthreads.html ; https://emscripten.org/docs/tools_reference/settings_reference.html
9. Emscripten audio integration: https://emscripten.org/docs/porting/Audio.html
10. WebAssembly f64/numeric instructions: https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/Numeric
11. Impulcifer latest-release asset metadata and release: https://api.github.com/repos/115dkk/Impulcifer-pip313/releases/latest ; https://github.com/115dkk/Impulcifer-pip313/releases/tag/v2.14.0
12. Electron latest-release asset metadata: https://api.github.com/repos/electron/electron/releases/latest ; https://github.com/electron/electron/releases/tag/v44.2.0
13. Electron 44.2.0 component versions: https://releases.electronjs.org/release/v44.2.0
14. Electron V8 memory cage announcement: https://www.electronjs.org/blog/v8-memory-cage
15. Node child-process spawn, pipes and lifecycle: https://nodejs.org/api/child_process.html
16. electron-updater stable metadata and update documentation: https://registry.npmjs.org/electron-updater/latest ; https://www.electron.build/docs/features/auto-update/
17. Velopack SDK metadata and official Electron guide: https://registry.npmjs.org/velopack/latest ; https://docs.velopack.io/getting-started/javascript
18. Velopack macOS packaging: https://docs.velopack.io/packaging/operating-systems/macos
19. electron-builder code signing: https://www.electron.build/docs/features/code-signing/
20. electron-builder notarization: https://www.electron.build/docs/notarization/
21. electron-builder update security: https://www.electron.build/docs/features/security/
22. Velopack signing/notarization: https://docs.velopack.io/packaging/signing
23. Velopack GitHub Actions examples: https://docs.velopack.io/distributing/github-actions
24. Microsoft WebView2 runtime distribution: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution
25. Electron process model and performance: https://www.electronjs.org/docs/latest/tutorial/process-model ; https://www.electronjs.org/docs/latest/tutorial/performance
26. TypeScript checking JavaScript: https://www.typescriptlang.org/docs/handbook/type-checking-javascript-files.html
27. Electron security, context bridge and defaults: https://www.electronjs.org/docs/latest/tutorial/security ; https://www.electronjs.org/docs/latest/api/context-bridge ; https://www.electronjs.org/docs/latest/api/structures/web-preferences
28. PortAudio API overview: https://www.portaudio.com/docs/v19-doxydocs/api_overview.html
29. naudiodon release/maintenance evidence: https://registry.npmjs.org/naudiodon ; https://github.com/Streampunk/naudiodon ; https://github.com/Streampunk/naudiodon/commits/master/ ; https://raw.githubusercontent.com/Streampunk/naudiodon/master/package.json
30. naudiodon raw Node-API and build: https://raw.githubusercontent.com/Streampunk/naudiodon/master/src/naudiodon.cc ; https://raw.githubusercontent.com/Streampunk/naudiodon/master/src/AudioIO.cc ; https://raw.githubusercontent.com/Streampunk/naudiodon/master/binding.gyp
31. naudiodon historical non-Electron crash: https://github.com/Streampunk/naudiodon/issues/49
32. PortAudio callback restrictions: https://www.portaudio.com/docs/v19-doxydocs/writing_a_callback.html
33. Eigen vcpkg: https://vcpkg.io/en/package/eigen3.html
34. pocketfft package and upstream: https://vcpkg.io/en/package/pocketfft.html ; https://github.com/mreineck/pocketfft
35. KissFFT package and scalar configuration: https://vcpkg.io/en/package/kissfft.html ; https://github.com/mborgerding/kissfft/blob/master/CMakeLists.txt
36. PortAudio vcpkg manifest/recipe: https://github.com/microsoft/vcpkg/blob/master/ports/portaudio/vcpkg.json ; https://github.com/microsoft/vcpkg/blob/master/ports/portaudio/portfile.cmake
37. PortAudio upstream backend switches: https://github.com/PortAudio/portaudio/blob/master/CMakeLists.txt
38. libsndfile package/formats: https://vcpkg.io/en/package/libsndfile.html ; https://libsndfile.github.io/libsndfile/api.html
39. Ceres bounded nonlinear least squares: https://ceres-solver.readthedocs.io/latest/nnls_solving.html
40. vcpkg CMake integration: https://learn.microsoft.com/en-us/vcpkg/users/buildsystems/cmake-integration
41. Conan 2 CMake/profiles: https://docs.conan.io/2/tutorial/consuming_packages/build_simple_cmake_project.html
42. scikit-build-core CMake/ABI configuration: https://scikit-build-core.readthedocs.io/en/stable/guide/cmakelists.html ; https://scikit-build-core.readthedocs.io/en/latest/guide/getting_started.html
43. Electron release cadence/support: https://www.electronjs.org/docs/latest/tutorial/electron-timelines
44. Electron ASAR native execution restrictions: https://www.electronjs.org/docs/latest/tutorial/asar-archives
45. Electron dialogs/theme/titlebar/media permissions: https://www.electronjs.org/docs/latest/api/dialog ; https://www.electronjs.org/docs/latest/api/native-theme ; https://www.electronjs.org/docs/latest/tutorial/custom-title-bar ; https://www.electronjs.org/docs/latest/api/system-preferences
46. pybind11 build integration and GIL/free-threading: https://pybind11.readthedocs.io/en/stable/compiling.html ; https://pybind11.readthedocs.io/en/stable/advanced/misc.html
47. nanobind CMake/ABI and version history: https://nanobind.readthedocs.io/en/latest/api_cmake.html ; https://nanobind.readthedocs.io/en/latest/changelog.html ; https://pypi.org/project/nanobind/
48. cibuildwheel target/repair options: https://cibuildwheel.pypa.io/en/stable/options/
49. Plotly local script loading and PNG export: https://plotly.com/javascript/getting-started/ ; https://plotly.com/javascript/static-image-export/
50. Electron page capture: https://www.electronjs.org/docs/latest/api/web-contents
51. Electron IPC serialization/transfer restrictions: https://www.electronjs.org/docs/latest/api/ipc-renderer
52. node-canvas stable versus prerelease documentation: https://github.com/Automattic/node-canvas/blob/v3.2.0/Readme.md ; https://github.com/Automattic/node-canvas
53. Cairo/Pango native rendering and packages: https://www.cairographics.org/manual/cairo-PNG-Support.html ; https://docs.gtk.org/PangoCairo/ ; https://vcpkg.io/en/package/cairo.html ; https://vcpkg.io/en/package/pango.html
54. Plotting alternatives, incomplete qualification: https://github.com/alandefreitas/matplotplusplus ; https://www.gnuplot.info/docs_6.1/Gnuplot_6.pdf ; https://gnuplot.info/docs_6.0/loc15267.html
55. Discord native C++ media and 64-bit issues: https://discord.com/blog/how-discord-handles-two-and-half-million-concurrent-voice-users-using-webrtc ; https://discord.com/blog/how-discord-seamlessly-upgraded-millions-of-users-to-64-bit-architecture
56. Signal native stack/version/build/packaging: https://raw.githubusercontent.com/signalapp/Signal-Desktop/v8.26.0/package.json ; https://raw.githubusercontent.com/signalapp/ringrtc/main/src/rust/src/electron.rs ; https://github.com/signalapp/webrtc ; https://github.com/signalapp/ringrtc/blob/main/BUILDING.md ; https://github.com/signalapp/Signal-Desktop/issues/4596
57. Streamlabs native modules/processes: https://github.com/streamlabs/desktop/blob/master/README.md ; https://raw.githubusercontent.com/streamlabs/desktop/master/package.json ; https://raw.githubusercontent.com/streamlabs/obs-studio-node/staging/js/module.ts ; https://github.com/streamlabs/obs-studio-node
58. LivePlay shipping sidecar architecture/version: https://github.com/tdoukinitsas/liveplay ; https://github.com/tdoukinitsas/liveplay/releases ; https://raw.githubusercontent.com/tdoukinitsas/liveplay/v2.4.3/client/package.json

Local verified sources (URLs point to the inspected files, not potentially different remote master):

- **L1.** Frontend, complete file: file:///E:/Impulcifer/webview_ui/app.js ; HTML script loading: file:///E:/Impulcifer/webview_ui/index.html
- **L2.** Audio operations and device selection: file:///E:/Impulcifer/core/recorder.py
- **L3.** AutoEQ bounded fit, lines 350–520: file:///E:/Impulcifer/autoeq/frequency_response.py
- **L4.** RBJ coefficient conventions: file:///E:/Impulcifer/autoeq/biquad.py
- **L5.** EQAPO parsing/design conventions: file:///E:/Impulcifer/core/eqapo.py
- **L6.** Job polling and cancellation: file:///E:/Impulcifer/application/impulcifer_service.py
- **L7.** Native frontend decision: file:///E:/Impulcifer/docs/adr/0001-native-frontends-stay-native.md
