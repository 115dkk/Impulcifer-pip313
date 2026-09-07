# Impulcifer rewrite research: unlisted alternatives

Research date: 2026-09-07. Repository examined read-only: `E:/Impulcifer`, version 2.13.3. No implementation, package build, audio-hardware test, size measurement, or license eligibility determination was performed.

## 1 Verdict

1. **[I] Best two unlisted full-rewrite contenders: A, .NET 10 + Avalonia 11; C, C++ + Qt 6.** Neither supplies a complete float64 SciPy replacement.
2. **[I] Best migration plan overall: H, preserve Python DSP behind a packaged sidecar, then port tested stages through PyO3.** This is a migration strategy, not another GUI toolkit.
3. **[I] A wins on cohesive application engineering; C wins on mature native integration and access to C/C++ numerical libraries.** Both lose existing UI implementation if their native GUI is chosen.
4. **[V] JUCE 8 is AGPLv3/commercial, not GPLv3; its FFT is float-only.** Kill the presumed MIT-friendly, all-in-one JUCE DSP replacement. Commercial/AGPL JUCE remains a conditional shell option. [12–15]
5. **[I] egui/eframe is a credible reserve; Flutter and Compose are workable but add more replacement work than this project needs.** Do not choose them just to escape Python packaging.
6. **[V] Dioxus Desktop uses a WebView. [I] It does not qualify as the proposed no-WebView alternative.** [33]
7. **[I] Browser/PWA measurement is disqualified by R1; processing-only WASM is viable as a separate product.** [47–54]
8. **[V] PyPI distribution after a C# rewrite is not impossible. [I] Preserving Python APIs becomes an extra interop project, not a free benefit.** [70–72]
9. **[I] Start with an audio conformance prototype and stage-level numerical oracle, not a whole-application port.** A new language does not make parallelism, channel routing, or numerical equivalence automatic.
10. **[V] Current WAV output is PCM_32 integer, not FLOAT. [I] Format conversion and numerical migration need separate acceptance criteria.** Local evidence below.

## 2 Findings (with sources)

### 2.1 Claim notation, scope, and version discipline

- **[V] Verified** means supported by a cited primary URL, or by explicitly cited local source. It does not mean tested in this application's release binary.
- **[I] Inference** means an engineering recommendation, comparative score, proposed design, or expected implementation cost.
- **[U] Unknown** means not established by the retrieved evidence or not tested.
- Every table of fit, ranking, workload, risk, or LLM documentation depth is **[I]** unless a cell explicitly says otherwise. Documentation depth is an assessment of the public API/reference/example corpus, not a measured benchmark of an LLM's correctness.
- **[V] Version-pinned evidence is used where available.** Current `main`, `master`, `latest`, and product pages are observations retrieved for this report, not proof of what a particular release contained. Some search results exposed newer documentation without reliable release dates. Those results were not treated as the latest stable release as of the research date.
- **[U] Installed and download sizes are unmeasured for every stack.** No hello-world size claim is used to estimate this application, its fonts, numeric libraries, reports, audio backends, or updater.

| Component | Version/date actually established or used | Qualification |
|---|---|---|
| .NET | [V] 10 LTS: released 2025-11-11, support through 2028-11-14; 9 STS support through 2026-11-10 | Choose 10 for a new project. Official policy table last updated 2026-08-11; do not repeat the outdated assumption that 9 had already expired in May. [1] |
| Avalonia | [V] 11 deployment documentation, license pinned at 11.3.0 | Not a claim that 11.3.0 is the latest patch; v11 docs can receive later edits. [2–4] |
| NWaves / Math.NET | [V] NWaves 0.9.6, 2021-10-06; Math.NET stable 5.0.0, 2022-04-03; newer 6.0 beta entries exist | Old stable dates are not proof of abandonment. They are evidence against assuming fast, coordinated scientific-stack evolution. [5–8] |
| NAudio / PortAudio | [V] NAudio 2.2.1 source; PortAudio v19 API | PortAudioSharp2 source was unversioned; NativeAOT compatibility and packaged backend selection remain untested. [9–11] |
| JUCE | [V] 8.0.0 license/headers; JUCE 8 EULA with 2025-07-17 revision entry | Do not import JUCE 7's GPL/$50k terms or current JUCE 9 pages into this decision. [12–17] |
| Qt / QCustomPlot | [V] Qt 6.8 API/license baseline; Qt Charts deprecation since 6.10; QCustomPlot download page 2.1.1, 2022-11-06 | QCustomPlot's listed compatibility through Qt 6.4 is not proof of failure on newer Qt. [18–25] |
| Rust native GUI | [V] egui 0.36.1; egui_plot 0.37.0 depends on egui ^0.36.0; iced 0.14.0; Slint 1.17.1; Dioxus Desktop 0.7.10; GPUI crate 0.2.2 | Different egui/plot version numbers are intentional. Latest Zed source is not equivalent to GPUI 0.2.2. [26–35] |
| Flutter / bridge | [V] Flutter desktop and FRB 2.x docs; package page displayed FRB 2.13.0 and 2.14.0-beta.1, fl_chart 1.2.0 | [U] Exact release dates and latest Flutter SDK patch as of cutoff were not settled; do not use undated “latest” displays as release-history evidence. [36–40] |
| Compose / Java | [V] Compose desktop docs; JTransforms installation coordinate 3.2 | [U] A Compose 1.12.0 release page appeared without a reliably established cutoff date; recommendation is not conditional on that being the cutoff's latest stable. [41–46] |
| Python packaging | [V] PyInstaller 6.22.2, 2026-08-17; PyO3 0.26.0 pinned guide; PyOxidizer maintainer status dated 2024-03-18 | Current maturin docs include newer ABI features; use version-specific 3.13t/3.14t wheels rather than assuming a future free-threaded stable ABI. [55–63] |
| Niche languages | [V] Zig 0.15.2 docs; Nim 2.2.10 announcement 2026-04-24; Swift 6 announcement 2024-09-17; DMD 2.113.0 displayed | These establish relevant capabilities, not the latest compiler for every language. [64–67] |

### 2.2 What the repository changes about the decision

| Local evidence [V] | Implication [I], or limitation [U] |
|---|---|
| `core/recorder.py:139` calls blocking `rec`; `:440–445` starts that recording thread; `:465` calls blocking `play`; the function subsequently joins the recorder. | Preserve the requested **two independent logical stream operations**, not a forced single duplex API. Do not claim the existing code proves concurrent hardware operation. |
| sounddevice 0.5.3 documents that convenience `play` and `rec` first stop existing convenience operations. [96] | **[U] Existing runtime overlap is not established by these two calls.** A replacement must test two explicit streams. This finding does not authorize modifying the recorder in this research task. |
| `core/recorder.py:435–438` truncates playback data to the output device's channel capacity; capture length is the playback sample count at `:442`. | A new measurement contract should report unsupported routing rather than silently equating truncated playback with a successful 16-speaker test. Preserve or intentionally revise behavior with tests, not incidental port behavior. |
| `core/audio_io.py:82–97` maps bit depth 32 to `PCM_32`; `core/hrir.py:426–455` delegates to it; `core/pipeline.py:873–876` writes HRIR/HeSuVi without overriding it. `core/brir_recovery.py:541–546` separately hardcodes `PCM_32`. | R3's 32-bit **float** output is a new output-format requirement. A WAV header/hash change alone must not be diagnosed as a numerical DSP regression. |
| `autoeq/frequency_response.py:350–513` implements ordered peak seeding, filter merging/pruning, and SciPy least squares. `:465–487` optimizes log10(fc), log(Q), gain under bounds; `:494–495` maps `max_time` to `max_nfev`, not wall time. | Reproducing a “biquad optimizer” is insufficient. Preserve parameterization, seed order, pruning, bounds, residual evaluation, and evaluation budget. A real wall-time timeout would change current deterministic semantics. |
| `autoeq/biquad.py:22–131` contains compact RBJ-style coefficient/response code. `core/eqapo.py:191–218`, `:497`, `:750–907` cover numeric parsing, conditions, channel scoping, filters, convolution and includes. | The EqualizerAPO interpreter is a larger semantic port than the biquad formulas. Maintain parser/error/report golden cases, not only WAV comparisons. |
| `application/impulcifer_service.py:201–230`, `:519–553`, `:893–977` define one active job, sequence-cursor polling, bounded events, retained terminal jobs and capability-gated cancellation. Recording/recovery cancellation are currently false. | All native stacks can reproduce this. Do not accidentally promise recording cancellation merely because a toolkit has a cancellation token. |
| `webview_ui/app.js:58` is the single bridge access; `:623–674` polls at 250 ms. The three principal assets total 2,960 lines. | Web shells can preserve most UI code by replacing the transport adapter. Native UI means replacing layout, styling and controller code, not merely recompiling it. |
| Nine supported locales contain 433 keys each; two hyphenated Chinese aliases mirror canonical files. `i18n/localization.py:15–25`, `:49–53`; service `:1329–1341` overlays selected language on English. | Strings, placeholders, translation tests and fonts are reusable across every desktop stack. They are not inherently tied to JavaScript or Python. |
| `LICENSE` is MIT; `pyproject.toml:25` carries the MIT classifier. ADR `docs/adr/0001-native-frontends-stay-native.md:15–28` rejects routing existing CTk through the WebView service. | Preserve CTk as frozen/native during a transition. A new Avalonia/Qt UI may share a typed core, but this is not permission to retrofit CTk to JSON polling. |

### 2.3 Overall ranking against R1–R9

**[I] Ordering combines fit, migration cost, maintenance burden and the actual asset base, not hypothetical greenfield elegance.** The top two *new full-rewrite stacks* are A and C. H ranks first as a delivery strategy; it can ultimately converge on S1 or C rather than compete as a new language.

Fit key: **C** = credible implementation with explicit work; **G** = major gap or validation gate; **X** = does not satisfy the stated requirement as proposed; **P** = preserves existing implementation, subject to existing behavior and packaging. Nothing in the R1 column means hardware-tested. R6 includes CLI/updater/three-platform distribution; PyPI is noted separately.

| Rank | Stack / decision | R1 | R2 | R3 | R4 | R5 | R6 | R7 | R8 | R9 | Existing assets discarded/reworked | Ecosystem risk | Solo maintainer + AI fit / documentation depth |
|---:|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|---|---|---|
| 1 | **H: Tauri sidecar + incremental PyO3, KEEP** | P/G | P→G | P/C | P | C | C | P | P/C | P→G | Transport/packager first; preserve DSP, UI, locales, CLI, plots; replace stages only after tests | Medium: two runtimes initially, declining only as Python dependencies actually disappear | **Very good / deep** Python/Rust docs; smallest simultaneous change |
| 2 | **A: .NET 10 + Avalonia, KEEP with DSP redesign** | C/G | G | C | C | C | C | C | C | G | All Python implementation and HTML UI; retain tests/specimens, strings, fonts and report concepts | Medium; small scientific/binding libraries are weaker than .NET itself | **Good / very deep** .NET, good Avalonia, uneven numerical ports |
| 3 | **C: C++ + Qt, KEEP** | C/G | G | C | C | C | C | C | C | G | Python core; native variant replaces web UI; WebEngine variant can retain it | Medium: build toolchain, LGPL compliance, optional Chromium payload | **Good with strict gates / very deep** Qt/C++; greater memory/ABI review burden |
| 4 | **D: Rust + egui/eframe, KEEP reserve** | C/G | G | C | C | C/G | C | C | C | G | All Python and web UI; reusable data/specs/locales | Medium-high: GUI API churn and accessibility of custom plots | **Fair-good / good** Rust and egui; smaller scientific corpus |
| 5 | **B: C++ + JUCE 8, KILL as stated; conditional KEEP** | G | G | C/G | C | C | C | C | C | G | Core rewrite; web-relay variant preserves UI; native JUCE replaces it | High until AGPL/commercial decision; float DSP mismatch | **Fair / deep** audio docs, but does not remove scientific-port work |
| 6 | **E: Flutter + Rust, KEEP reserve** | C/G | G | C | C | C | C/G | C | C | G | Python and HTML/CSS/JS UI; add Dart/Rust bridge schema/codegen | Medium-high: two-language UI/core plus native packages | **Fair / deep Flutter, good FRB**, duplicated concepts and more generated interfaces |
| 7 | **F: Compose Desktop + native audio, KEEP reserve** | G | G | C/G | C | C/G | C | C | C | G | Python and web UI; retain strings/data; add JNI/FFM or worker boundary | Medium-high: JVM/native integration, updater/tooling choice | **Fair / deep JVM**, less end-to-end scientific desktop guidance |
| 8 | **G: browser/PWA measurement, KILL** | X | G | C | C | X/G | X | X/G | G | G | Port core; significant storage/device UX redesign; existing static UI partly reusable | High browser/device variance | **Poor for measurement / deep web docs** cannot remove API restrictions |
| Separate | **G: file-processing WASM, KEEP as companion** | N/A | G | C | C | G | G | G | G | G | Port processing, keep much static UI; import/export rather than arbitrary folder writes | Medium-high browser memory/thread/platform constraints | Good only with reduced scope; not an R1–R9 replacement |
| 9 | **I: Zig full app, KILL** | C/G | G | C/G | C | G | G | C | C | G | Essentially all executable implementation | High integration/API/tooling burden | Poor here / good language docs, thin complete-stack examples |
| 10 | **I: Nim full app, KILL** | C/G | G | C/G | C | G | G | C | C/G | G | Same broad rewrite | High small-ecosystem integration burden | Poor here / thinner authoritative task-specific corpus |
| 11 | **I: Swift 6 cross-platform app, KILL** | C/G | G | C/G | C | G | G | C | C | G | All code/UI; Apple GUI assets do not supply Windows/Linux GUI | High for this Windows-first product | Poor here / deep Apple docs, uneven cross-platform GUI story |
| 12 | **I: D full app, KILL** | C/G | G | C/G | C | G | G | C | C/G | G | All implementation | High library/integration burden | Poor here / solid language docs, thin application-specific ecosystem |

**[I] Shared desktop interpretation.** R4, R7 and R8 do not distinguish most native candidates: each can run worker tasks, spawn ffmpeg, communicate progress and use threads. They still require job ownership, cancellation points, output transactions, helper process management and bounded scheduling. “Compiled gets parallelism for free” is false as an architectural claim. R2 and R9 are the largest common implementation costs; R1 is the first hardware gate.

### 2.4 A: .NET 10 + Avalonia 11

**Decision [I]: KEEP as the strongest new native-UI contender; KILL the assumption that NWaves + Math.NET is a drop-in scientific stack.**

| Area | Evidence and consequence |
|---|---|
| Application platform | **[V]** .NET 10 is LTS through 2028-11-14; Avalonia's core is MIT. NativeAOT deployment is documented, with compiled bindings/static resources and reflection/trimming cautions. **[I]** Start with ordinary self-contained .NET publication. Treat AOT as an optimization gate, not the condition on which the whole architecture depends. [1–4] |
| Windows/macOS/Linux | **[V]** Avalonia documents native desktop deployment targets. **[U]** This project's precise dark-title-bar, theme switching, font shaping, platform dialog and packaged accessibility behavior is untested. **[I]** Use native UI controls and per-platform smoke tests rather than assuming pixel/behavior identity. [3–4] |
| Audio | **[V]** NAudio 2.2.1 has Windows Wave/DirectSound/WASAPI/ASIO paths, not a verified three-OS device solution. PortAudio exposes the appropriate portable host/device abstraction. PortAudioSharp2 is Apache-2.0 and wraps native PortAudio. **[V]** Its examined Stream constructor rejects a null callback and does not expose blocking read/write in that file despite its documentation. **[I]** Use a small maintained PortAudio interop layer or callback-plus-completion implementation, not an assumed blocking-wrapper API. [9–11,73] |
| Float64 | **[V]** NWaves commonly uses float signals; double filters/convolvers and Fft64 exist, but Fft64/RealFft64 require power-of-two sizes. Math.NET provides double numerics and a bounded LM API. **[I]** Select individual validated double implementations; never convert to float in the pipeline simply to fit an API. Zero-padding an arbitrary FFT to suit NWaves is not equivalent to the original operation. [5–8] |
| Optimizer | **[V]** Math.NET 5 bounded Levenberg–Marquardt accepts lower/upper bounds; it is not SciPy TRF. **[I]** Either port the specific TRF behavior or explicitly approve a solver change using the EQ response oracle. Do not confuse commercial **Numerics.NET** with **Math.NET Numerics**. [8,68] |
| Plots | **[V]** ScottPlot 5 supplies Avalonia integration and scientific chart/heatmap examples with PNG export. **[I]** It is the default candidate for this application, ahead of decorative chart libraries. LiveCharts2 lists Avalonia and MIT licensing, but its existence proves neither Bokeh-equivalent export nor all required waterfall/phase analyses. OxyPlot is MIT; the inspected Avalonia package has old stable/preview entries and needs an explicit Avalonia 11 compatibility check. [74–77] |
| Interactive HTML | **[V]** ScottPlot's PNG-in-HTML functions produce embedded images, not a complete interactive export. **[I]** Generate standalone HTML from numerical report data using a vendored JS plotting renderer, independent of the native plot controls. [74,78] |
| Web UI reuse | **[V]** Avalonia 11's official WebView is an Accelerate-licensed component; NativeWebView is Windows/macOS, while Linux uses NativeWebDialog. **[I]** Do not choose Avalonia to preserve the current in-window HTML UI on all three platforms. That forfeits its principal simplicity advantage and introduces licensing/platform branching. [79] |
| Updater | **[V]** Velopack has first-class C# integration, but also C++ and Rust APIs. **[I]** C# alignment is convenient, not exclusive. Existing Velopack identity/channel migration still needs an install-upgrade test. [69] |
| PyPI | **[V]** pythonnet loads CLR assemblies; NativeAOT can export a self-contained shared C ABI for non-.NET callers. **[I]** A wrapper wheel is technically possible through pythonnet, NativeAOT exports, or a bundled CLI. None automatically preserves Python classes/NumPy semantics. A NativeAOT library is not an ordinary CLR assembly to load through pythonnet. [70–72] |

**[I] Against S1/S2/S3:** A has a more unified native application story than Rust/Tauri, C++/Electron or Go/Wails if a completely new native UI is genuinely wanted. It is worse than all three at preserving the current static frontend. Its scientific numerical library coverage is not convincingly better than Rust plus selected native libraries. The argument for A is maintainable application code and mature .NET tooling, not reduced DSP effort.

### 2.5 B: C++ + JUCE 8

**Decision [I]: KILL as the default MIT-oriented replacement; KEEP only if the maintainer explicitly accepts AGPLv3 or the applicable JUCE commercial terms.** The license decision comes before implementation.

- **[V] JUCE 8.0.0 is dual AGPLv3/commercial, not GPLv3.** MIT-licensed application source can be combined under a compatible overall distribution regime, but the resulting JUCE-containing distribution is not MIT-only. Merely being an open-source project does not grant a blanket free MIT-compatible JUCE license. [12]
- **[V] JUCE 8 commercial Starter is free up to $20,000 annual revenue/funding; Indie up to $300,000, $40/month or $800 perpetual per user; Pro has no revenue cap, $175/month or $3,500 perpetual per user.** EULA accounting differs for individuals and companies; donations/indirect income can count, and company/affiliate totals need not be limited to JUCE-derived income. **[U]** This maintainer's eligibility and redistribution arrangement are not established. Free Starter is still a commercial contract. [13]
- **[V] AudioDeviceManager/AudioIODevice expose device types, input/output channel selection, supported rates and callback buffers.** JUCE's 8.0.0 device factory header lists Windows WASAPI, DirectSound and ASIO, but no MME factory; it lists ALSA/JACK on Linux. **[I]** Exact R1 host-API inventory is not automatically met. Add PortAudio or explicitly change the MME requirement. Separate input/output names in AudioDeviceSetup do not prove arbitrary two-device/two-stream combinations work on every backend. [16,80]
- **[V] JUCE 8 `dsp::FFT` uses float/Complex<float>, power-of-two size from `order`; its Convolution API is float-based.** Some filter templates support double, but this does not repair the FFT/convolution gap. **[I]** Use a separate float64 scientific core, making JUCE an audio/UI shell rather than the promised complete DSP replacement. [14–15]
- **[V] JUCE 8 WebBrowserComponent offers native functions, events and resources for an HTML/JS UI, using system WebView backends.** WebView2 loader linkage is not Chromium engine bundling. **[I]** This can retain the current frontend with a bridge adapter; native JUCE controls would discard it. [17]
- **[I] Plotting/spectrograms:** JUCE can display custom graphics and audio views, but an audio waveform component is not a scientific report framework. Build f64 analysis arrays and a separate plot/export implementation. **[U]** No complete off-the-shelf JUCE solution was verified for the specified log-FR, phase unwrap, ILD/IPD/IACC, waterfall and offline interactive HTML requirements.

**[I] Against S2:** JUCE can remove Electron's bundled Chromium/Node runtime and give C++-native device/window integration. But it does not eliminate system WebView dependencies when retaining HTML, exact R1 still needs work, licensing is more restrictive, and scientific DSP remains external. If R1's MME requirement forces PortAudio anyway, JUCE's advantage shrinks substantially. Against S1/S3, it retains the C++ dependency/toolchain burden without a clear numerical or licensing win.

### 2.6 C: C++ + Qt 6

**Decision [I]: KEEP as the second full-rewrite contender. Prefer Widgets + dynamically linked LGPL modules + PortAudio + a headless double DSP core. Use WebEngine only if preserving the UI is worth bundling another Chromium-based stack.**

| Question | Finding |
|---|---|
| LGPL/static linking | **[V]** Static linking under LGPL is not categorically forbidden. Relinkable application code/object files, library source/notice obligations, permissions for library modification/debugging and applicable installation information must be supplied. Dynamic linking reduces this practical burden but is not “no obligations.” MIT application licensing can remain; preserve users' LGPL rights. [18] |
| Charts licensing | **[V]** Qt Charts 6.8 is GPLv3/commercial, not LGPL; it is deprecated since 6.10. QCustomPlot is GPL/commercial; exact license text of the chosen archive must be checked. **[I]** Neither belongs in a presumed all-free MIT/LGPL chart stack without an explicit licensing decision. Replacing Charts with Graphs requires a new module/license review, not an automatic assumption. [19–21] |
| Qt audio limits | **[V]** QAudioFormat Float is four-byte PCM; channel count is configurable; QAudioDevice exposes min/max counts, rates, formats and `isFormatSupported`. **[I]** Neither “Qt is stereo-only” nor “Qt supports any 16-channel interface” is supported. Range endpoints do not prove every rate/count/format combination. [22] |
| Exact host APIs | **[U]** QtMultimedia was not established as exposing the requested MME/DirectSound/WASAPI/ASIO grouped inventory. **[I]** Use PortAudio for R1 rather than force measurement requirements through a media-playback abstraction. [11,22] |
| Widgets or QML | **[I]** Widgets is the conservative choice for settings-heavy scientific UI and custom plots. QML is reasonable for a redesigned animated UI, but introduces declarative engine/deployment concepts without preserving existing HTML. Neither solves DSP. |
| PNG/scientific plots | **[V]** QPainter draws on QImage; QImage can save PNG. **[I]** A small reusable log-axis plot widget and offscreen renderer can satisfy FR/IR/decay/heatmaps, but ticks, transforms, selection, accessibility and large-data decimation become project code. Keep numeric analysis separate. [81] |
| Existing web UI | **[V]** WebEngine carries Chromium components and licenses, does not support static builds, and QWebChannel exposes QObject properties/signals/invokables to JavaScript. **[I]** Reuse the existing frontend through a narrow adapter, not broad exposure of backend objects. This is a credible S2 competitor, not a small native-Widgets build. [23–25] |
| Installation size | **[U]** Not measured. **[I]** Widgets without WebEngine avoids the browser payload; WebEngine gives up much of that advantage. No basis was found for a precise MB estimate or automatic equality with the current Nuitka artifact. |
| PyPI and CLI | **[V]** pybind11 supports C++/Python and NumPy/buffer interop. **[I]** A Qt-free C++ core can back CLI and optional wheel while Qt stays out of Python imports and batch processing. [82] |

**[I] Against S2:** Reuse the same C++ core. Widgets removes Electron/Node and Chromium but costs the web UI; WebEngine preserves the UI and C++ integration but retains a heavy browser deployment. There is no strong reason to move from Electron to WebEngine solely for size. **Against S1/S3:** Qt offers deeper native desktop references and direct C++ numerical integration, but has LGPL compliance and greater C++ build/memory-safety review obligations. A Rust core can be used behind Qt, but that is another interop boundary rather than the proposed simple C++ stack.

### 2.7 D: Rust native GUI, without a WebView

| Toolkit | Decision [I] | Verified evidence / unknowns |
|---|---|---|
| egui/eframe + egui_plot | **KEEP reserve; best D option** | **[V]** Native desktop support and plotted data APIs; AccessKit integration supplies accessibility for standard widgets where platform adapters exist. API changes are an acknowledged maintenance concern. **[U]** Plot semantics, screen-reader data exploration and this UI's keyboard behavior require tests. PNG/headless/HTML report generation is separate from embedding egui_plot. [26–28] |
| iced | **KILL for the immediate shortlist** | **[V]** 0.14.0 describes itself as experimental; examined AccessKit PR #3111 was an open draft with limited widget/example coverage. **[I]** Do not volunteer to finish toolkit accessibility during a DSP rewrite. This is not a claim that iced has no accessibility work at all. [29] |
| Slint | **KEEP reserve** | **[V]** Native desktop platform support, accessibility roles/actions and testing guidance; GPLv3, Community and commercial licensing options. **[I]** Evaluate the Community attribution obligations and plot implementation before adoption. **[U]** No egui_plot-equivalent ready scientific plotting solution was established. [30–32] |
| Dioxus Desktop | **KILL in this category** | **[V]** Uses system WebView through wry/tao. Native Rust backend execution does not mean native no-WebView rendering. Dioxus Native/Blitz is a different renderer/product maturity question. **[I]** If a WebView is acceptable, S1 preserves the existing vanilla JS with less conceptual replacement. [33] |
| GPUI | **KILL for this project now** | **[V]** Current source has Windows support; pre-1.0 API churn is explicit. AccessKit work merged into Zed's source is not proof that published GPUI 0.2.2 plus arbitrary application widgets are production-accessible. **[I]** Reject for stabilization burden, not the outdated claim that Windows is unsupported. [34–35] |

**[V]** CPAL's current documented Windows hosts are WASAPI with optional ASIO/JACK; MME and DirectSound are absent from the list. **[I]** Rust native GUI has the same R1 issue as S1 when paired casually with CPAL: use a PortAudio adapter to meet the literal host inventory. Neither egui nor Rust supplies the missing scientific algorithms. [83]

**[I] Against S1:** The same Rust core and packaging logic can be reused. The trade is eliminating OS WebView behavior versus rewriting the whole current UI and rebuilding scientific plotting/export/accessibility glue. With only 2,960 lines of existing hand-written UI, replacement is feasible, but it is still unnecessary work unless a native plotting workflow or no-WebView requirement is independently desired. Against S2 it avoids Node/C++ application concerns; against S3 it offers stronger compile-time memory/ownership checks, not automatically a fuller SciPy-equivalent ecosystem.

### 2.8 E: Flutter + Rust through flutter_rust_bridge 2.x

**Decision [I]: KEEP as a reserve, not one of the top two.**

- **[V]** Flutter supports native Windows, macOS and Linux desktop applications; FRB 2.x supports those desktop targets. This is not an experimental “mobile-only” platform premise. Flutter normally renders its own UI rather than using a system WebView. [36–37]
- **[V]** Windows distribution includes the executable, Flutter/native DLLs, data and relevant VC++ runtime handling. **[U]** Full installed size, minimum practical Linux distribution compatibility and all required native plugins were not measured. A Flutter hello-world size would not answer this. [38]
- **[V]** fl_chart offers line/scatter and other charts; Flutter can capture a RenderRepaintBoundary into a PNG. **[I]** Use Rust-produced analysis arrays, not Dart reimplementations of DSP. Waterfall/spectrogram/log-frequency interactions still need application work. Standalone HTML export needs its own document generator. Screenshot export is not interactive HTML export. [39–40]
- **[V]** Flutter documents assistive-technology support and Semantics. **[U]** Custom plot navigation and the exact app's semantics remain untested. [40]
- **[I]** Keep audio in Rust/native PortAudio, not mobile-oriented Flutter recording/playback plugins. Run the DSP outside the UI isolate, return progress events and decimated display arrays, and keep long buffers owned by the core. **[U]** FRB's zero-copy marketing does not establish every f64 transfer's ownership, direction and lifetime behavior; benchmark the chosen bridge representation. [36]
- **[I]** Keep a separate Rust CLI. Wire a verified desktop updater rather than treating Flutter as supplying one. Existing locale strings transfer, HTML/CSS/JS controllers do not.

**[I] Against S1:** Flutter replaces a working UI and adds Dart/codegen for the same Rust DSP burden; Tauri reuses the web assets. Flutter is justified by a deliberate UI/mobile roadmap, not by this desktop application's packaging pain alone. Against S2/S3 it offers coherent custom-drawn UI, but no evidence of a net reduction in total maintenance across audio, numerics and release engineering.

### 2.9 F: Kotlin + Compose Multiplatform Desktop

**Decision [I]: KEEP as a distant reserve with native audio; KILL JTransforms + stock Java Sound as the complete implementation.**

- **[V]** JTransforms supplies double real/complex FFTs, including arbitrary lengths. It does not supply the rest of the requested SciPy stack. **[I]** JNI/FFM to a native scientific core is plausible, but then Kotlin is another GUI shell rather than an independently complete DSP ecosystem. [41]
- **[V]** Java's AudioFormat has PCM_FLOAT, but a format identifier is not device support. REW explicitly documents Windows JavaSound's default stereo/16-bit limitations and separate WASAPI-exclusive/ASIO paths. **[I]** Host-API grouped 16-channel float32 measurement requires a native backend, such as a small PortAudio binding. [42–43]
- **[V]** Compose desktop packaging uses jpackage/jlink for self-contained packages; target-OS builds are required by the ordinary packaging path. DMG/PKG, EXE/MSI and DEB/RPM are documented, not an automatic AppImage. **[I]** A bundled JRE avoids demanding a system JVM, but still contributes native-runtime inventory and update obligations. [44]
- **[V]** Compose desktop accessibility documentation lists macOS, Windows via Java Access Bridge and `jdk.accessibility`, and a Linux limitation. **[I]** Linux's lower priority makes that less decisive, not irrelevant. Windows setup and real assistive-technology tests still matter. [45]
- **[V]** Conveyor is a separate updater/packaging tool with cross-platform packaging/update features and an open-source free policy. **[U]** The exact current pricing/eligibility interpretation, accepted project configuration and release-pipeline performance were not established. Do not infer that jpackage itself supplies an in-app updater. [46]
- **[I]** Plotting can use Compose drawing or embedded chart components, but no complete replacement for this project's PNG + independent Bokeh-style reports was verified. Isolate report data and renderer selection rather than tying DSP to a Compose chart widget.

**REW interpretation [I]:** REW is meaningful evidence that a Java-based audio measurement product can work and can be distributed to these users. It is not evidence that stock JavaSound meets R1, that Kotlin/Compose reduces numerical-port cost, or that REW's historic language choice is optimal now. **[U]** The original author's language-selection motives were not established; calling it solely a historical accident would be unsupported. [43]

**[I] Against S1–S3:** JVM documentation and test tools are deep, but this project has no JVM assets to preserve. Compared with S3, Compose adds a JVM and native audio bridge without solving R2. Compared with S1/S2, it replaces a reusable frontend and adds another plotting/export integration task. Keep only if the maintainer has a strong Kotlin/JVM preference not stated in the brief.

### 2.10 G: Web/WASM first

**Decision [I]: browser-only/PWA MEASUREMENT = KILL; local-file PROCESSING = KEEP as a companion, not a replacement for R1–R9.**

| Requirement | Verified browser capability | Decision / constraint |
|---|---|---|
| Host-API grouped enumeration | **[V]** Media devices and AudioContext sink selection exist, but the specified MME/DirectSound/WASAPI/ASIO selector is not exposed by these APIs. [47–49] | **[I] R1 fails.** A native helper repairs the missing capability only by ceasing to be browser-only. |
| 16-channel output | **[V]** Destination `maxChannelCount` and `channelCount` exist; browser audio is not categorically stereo-only. [47–48] | **[U]** Actual 16-channel device exposure/routing varies and was not tested. A graph with 16 channels does not prove 16 physical outputs. |
| Input channels and rates | **[V]** `getUserMedia` constraints include sampleRate/channelCount; recognized exact constraints can be required; unsupported constraints may be ignored. Check supported constraints and actual settings. [49] | **[I]** No portable guarantee of the required input/output/rate combinations. |
| Signal integrity | **[V]** AGC/echo/noise constraints can be requested off; Web Audio processing rate can differ from device rate, requiring resampling. AudioBuffer uses planar float32. [47,49–50] | **[I]** Browser settings do not prove absence of OS/driver/device processing or bit-transparent hardware transport. |
| Files | **[V]** File picker/drag-and-drop plus Blob downloads work without File System Access. Directory picker support is limited; OPFS is origin-private, quota-limited storage. [51] | **[I]** Processing imported WAVs locally is viable; arbitrary directory workflows across browsers need redesign. No server upload is required. |
| DSP | **[V]** WASM supports f64. Web Audio float32 buffers do not constrain private WASM double arrays. [50,52] | **[I]** Parse WAV bytes directly; avoid AudioBuffer for numerical intermediate storage. All the same FFT/filter/TRF port work remains. |
| Parallelism | **[V]** Emscripten pthreads need SharedArrayBuffer and appropriate COOP/COEP isolation; main-thread blocking is hazardous. Rust's default wasm target does not make std::thread available. [53–54] | **[I]** Use Web Workers; support a separate non-threaded build or worker strategy when isolation is absent. Native threading assumptions do not transfer. |
| ffmpeg and desktop behavior | **[V]** The standard browser APIs examined do not provide arbitrary local binary execution. | **[I] R7 fails as stated.** ffmpeg.wasm or server decoding would be a changed helper design, not keeping the downloaded native binary. Browser download/install behavior also does not preserve native CLI, open-path or desktop updater semantics. |

**[I] Against S1–S3:** An installed shell's native backend bypasses these browser sandbox restrictions while retaining a web frontend. A browser-only product offers frictionless processing distribution, not equivalent measurement. Choose native first; consider WASM once a portable core and portable fixtures already exist.

### 2.11 H: keep-Python sidecars and the incremental Rust variant

**Decision [I]: KEEP, highest-priority migration approach. Reject PyOxidizer as the new default packager.**

| Variant | What it actually removes | What remains / decision |
|---|---|---|
| Tauri + PyInstaller onedir Python worker | **[V]** Tauri explicitly documents PyInstaller-packaged Python sidecars via externalBin and child process stdin/stdout. PyInstaller supports NumPy and platform-specific bundling. [55–57] | **[I]** Removes Nuitka code generation and, with a genuinely headless import graph, pywebview/pythonnet/Tk frontend packaging. Preserves NumPy/SciPy, PortAudio/libsndfile, plotting/font dependencies and per-platform hooks. **KEEP** as first prototype. |
| Go/Wails + Python worker | **[V]** Wails is a system-WebView shell; v2 docs list WebView2 and Linux GTK/WebKit dependencies. [84] | **[I]** Same numerical preservation. Process supervision/protocol is application code; no advantage for R2 over Tauri. Choose if Go shell simplicity demonstrably outweighs Wails release/updater integration. **KEEP**, but rank behind Tauri sidecar for the documented bundling/updater path. |
| python-build-standalone + installed wheels | **[V]** Produces portable CPython distributions; install_only is a ready-to-run interpreter tree, not a complete application bundler. Astral took stewardship in December 2024. Platform/native-runtime constraints remain. [60] | **[I]** Avoids frozen-module surprises and permits a transparent interpreter+site-packages layout. Must build dependency lock, install/relocation, DLL lookup, startup/import settings and license inventory. **KEEP** as packaging fallback, not magic dependency elimination. |
| PyOxidizer worker | **[V]** Maintainer said in March 2024 that meaningful work had stopped after January 2023 and the future was uncertain; later requests to archive do not prove archival. Docs describe extension loading, static-build and third-party shared-library problems. [58–59] | **[I] KILL** for a new numerical desktop distribution. Replacing a maintained but troublesome packager with an uncertain one is not a defensible stabilization strategy. |
| Rust stage/core + PyO3 + existing app | **[V]** PyO3 supports extension builds and free-threading; maturin supports mixed Python/Rust packages and version-specific free-threaded wheels. [61–63] | **[I] KEEP**, best controlled numerical migration. Separate core crate from Python bindings, port one stage at a time, retain the old stage as oracle. Does not remove Nuitka or Python packaging until shell/dependency retirement actually happens. |

**[I] Does H merely move the pain?** It removes identifiable causes, not all causes. Eliminating Nuitka removes its compilation/plugin failure class. Removing Python GUI imports eliminates Python GUI bundling failure classes. Replacing pywebview with Tauri/Wails still leaves WebView2/WebKit runtime differences. Keeping Bokeh/matplotlib/SciPy preserves those wheels/native-library/font/report obligations. A retained Bokeh version that broke Nuitka may no longer break compilation because there is no Nuitka compilation, but that is not proof its API/runtime behavior is compatible.

**[V]** PyInstaller onefile extracts native support files into a temporary directory at startup, adds startup work and can leave residue after crashes. **[I]** Prefer onedir inside the outer installer: users still get one installable application, while the interpreter and DLL layout remain inspectable. Use locked wheels and offline build inputs rather than runtime pip installation. [57]

**[I] Recommended order:** (1) capture oracle and build sidecar without DSP change; (2) replace only web-shell packaging; (3) port chosen f64 stages through PyO3 while keeping PyPI/CLI; (4) remove Python runtime only when all remaining Python-only tasks, including reports and parsers, are replaced. Starting with PyO3 before the shell is also valid if numerical performance is the first problem, but it does not promptly address the stated Nuitka pain.

### 2.12 I: quick elimination of Zig, Nim, Swift 6 and D

**Zig [I]: KILL the full application; conditional KEEP for a small C-interop component.** **[V]** Zig 0.15.2 documents IEEE f64, C interop and cross-compilation; PortAudio and C numerical libraries are reachable. **[I]** That offers no unique solution to this app's missing SciPy algorithms, desktop UI, plots or updater. A Windows-first solo maintainer would inherit compiler/library integration work while discarding every functioning implementation. **[U]** No tested complete three-OS toolchain for this application's requirements was established. [64]

**Nim [I]: KILL the full application.** **[V]** Nim documents native C/C++ backends, Windows/macOS/Linux, cdouble/float64 and ORC memory management; its site showed 2.2.10 dated 2026-04-24. **[I]** Syntax resemblance to Python does not reuse NumPy/SciPy semantics or the current UI. C bindings still require dependency distribution and numerical tests. No unique benefit offsets the smaller application-specific ecosystem. **[U]** Suitable complete audio/plot/updater integration was not verified. [65]

**Swift 6 [I]: KILL for this Windows-first three-OS rewrite; revisit only for an Apple-only product.** **[V]** Swift 6 explicitly supports Linux/Windows and C/C++ interop; Swift is not confined to Apple CPUs or operating systems. **[I]** Language support does not provide SwiftUI/native desktop integration on all three targets. PortAudio and double numerics remain possible, but the UI/deployment burden defeats the stated motivation. **[U]** A single mature Swift desktop UI/updater combination meeting the brief was not established. [66]

**D [I]: KILL the full application.** **[V]** D has extern(C), double, Windows/macOS/Linux compilers and @nogc checks; its GC documentation describes thread stopping during collection. **[I]** Offline DSP is possible and low latency is not required, so GC is not itself disqualifying. The reason to reject D is the unearned cost of adopting another small desktop/scientific ecosystem with no reusable project assets, not a claim that D cannot do audio. **[U]** Complete required plotting, hardware and release integration was not verified. [67]

### 2.13 Specific comparison with the maintainer's S1–S3

**[I] These are comparisons, not full independent audits of S1–S3.**

| Baseline | Verified caveat | What an unlisted stack offers / recommendation [I] |
|---|---|---|
| S1 Rust + Tauri 2 | **[V]** System WebView and external sidecars; signed updater supports Windows installers, macOS app archives and Linux AppImage. CPAL does not list all required Windows host APIs. [55,83,85] | H is the safest route into S1. D removes WebViews but throws away UI. A is a real native-UI alternative; C gives mature C++ scientific integration. S1's frontend reuse remains a substantial advantage. |
| S2 C++ + Electron | **[V]** Electron uses Chromium/Node process architecture. Native Node addons may need Electron-target ABI rebuilds; an ordinary external C++ process is not subject to that Node-addon ABI rule. [86] | C/Widgets genuinely removes browser/Node dependencies, with UI rewrite cost. C/WebEngine keeps Chromium and is not an obvious size win. B offers an audio/web relay with licensing and float-DSP gaps. Electron's weight may be acceptable, but “already heavy” is not a measured equality of size, cold start or security-update work. |
| S3 Go + Wails | **[V]** System WebView runtime dependencies remain; Go's PortAudio wrapper needs native PortAudio. Gonum optimize 0.17 docs do not establish SciPy-compatible bounded TRF. [84,87] | H preserves scientific correctness while Go acts only as the shell. A and C have stronger cohesive native-GUI/scientific-library stories, but both cost more than retaining UI/Python. Go's lack of a Node runtime is not unique among these alternatives and does not remove C audio dependencies. |

### 2.14 R2 coverage: the library names do not equal SciPy parity

**[I] The implementation plan must have a per-operation inventory.** The following is a required validation inventory, not a claim that every listed alternative lacks every individual routine.

| Operation family | Verified useful building blocks | What must be implemented or matched [I] |
|---|---|---|
| rfft/irfft/fft, next_fast_len, fftconvolve | **[V]** NWaves Fft64 power-of-two limitation; JUCE float FFT; JTransforms double arbitrary-length; pocketfft C++ supports double and arbitrary lengths under BSD-3-Clause. [6,14,41,88] | Transform scaling, real spectrum layout, even/odd lengths, exact requested length, convolution crop/padding and fast-length selection. Pick one f64 FFT contract and test it; don't choose a toolkit FFT by name. |
| convolve, correlate, correlation_lags | **[V-local]** These are in the current SciPy-dependent pipeline identified in the brief and numerical inventory. | Mode/crop/lag sign, complex conjugation where applicable, tie behavior and sample indexing. |
| butter/sosfilt, firwin2, minimum_phase, windows | **[V]** SciPy specifies firwin2/minimum_phase behavior; homomorphic method, half/full mode and FFT-length choices matter. [68] | SOS order/state, cutoff normalization, filter order, window conventions, FIR mesh and endpoint handling, cepstral epsilon/lifter/length, minimum-phase amplitude target. |
| savgol_filter, find_peaks, uniform_filter | **[V-local]** AutoEQ relies on peak-based initialization and smoothing, including the thresholds cited above. | Edge modes, derivative/scaling convention, tie/plateau order, window alignment and boundary extension. These can change optimizer seeds before the solver runs. |
| k=1/k=3 log-frequency splines | **[V-local]** AutoEQ explicitly constructs log10-frequency interpolation. | Spline endpoint conditions, exact interpolating behavior, extrapolation, duplicate input rejection and spacing. Natural cubic, not-a-knot and smoothing splines are not interchangeable. |
| bounded least_squares | **[V]** SciPy TRF differs from Math.NET bounded LM and Ceres bounded trust-region LM/Dogleg. [8,68,89] | Parameter transform, finite-difference Jacobian, scaling, bounds, stopping tests, evaluation budget, seed order and pruning. Either port the reference behavior or label an intentional algorithm change. |
| linregress, expit, RBJ formulas | **[V-local]** Existing coefficient source and decay-analysis operations supply a compact reference. | f64 stability, overflow-safe expit, degenerate regression inputs, coefficient normalization/sign and EQ parser semantics. |
| spectrogram / rational resampling | **[V-local]** Existing spectrogram and nnresample requirements are explicit in the brief. | Window, overlap, scale/power convention, padding, rational reduction, Kaiser design, polyphase alignment, delay and output length. A generic “resampler” is not sufficient. |

**[I] C++ numerical option:** pocketfft is a concrete permissive double FFT candidate. Ceres can solve bounded nonlinear least squares, but its algorithms are not SciPy TRF. If the bounded fit must remain algorithmically equivalent, a carefully attributed port of the relevant SciPy BSD code is more defensible than pretending Ceres is identical. It still requires adapting array/linear-algebra operations and checking third-party source obligations. [88–90]

**[I] Plot/render strategy across native stacks:** represent plots as versioned numerical data plus labels/units/axes, with a native or offscreen PNG renderer and a separate self-contained JS HTML renderer. This keeps Bokeh-style output possible without carrying Python just to instantiate Bokeh models. **[U]** No complete replacement renderer was selected or benchmarked here. Plotly's documented self-contained HTML behavior establishes a viable pattern, not permission to assume all scientific traces are already implemented. [78]

## 3 Proposed architecture

### 3.1 Top contender A: .NET 10 + native Avalonia 11

**[I] Sketch; not an implementation claim.**

```text
Avalonia native desktop                         impulcifer CLI
  ViewModels, dialogs, theme, i18n                    |
              \                                     /
               typed Application / JobCoordinator
               (no GUI dependencies in computation)
                         |
              Impulcifer.Core: double arrays
       DSP stages, EQ parser, sweep detection, reports
                         |
        +----------------+----------------+
        |                |                |
  PortAudio C ABI    WAV/export I/O    ffmpeg supervisor
  native library     + report data     downloaded helper
        |
  separate input/output operations; explicit completion

ScottPlot native/offscreen plots + standalone HTML renderer
Velopack package/update integration at desktop entry point
Optional Python adapter built separately, only if justified
```

- **[I] DSP boundary:** explicit f64 buffer/layout types and sample rate; avoid passing toolkit objects or float plot buffers back into DSP. Math.NET for validated primitives, selected NWaves double pieces only, reference-derived algorithms for gaps. A C ABI numerical kernel is an acceptable fallback, but acknowledge that this makes the stack hybrid.
- **[I] Audio boundary:** expose host APIs/devices/capabilities, open separate capture/playback operations, return exact channel mapping and actual configuration; block completion until output has drained and capture has finished. Callback adapters must preallocate and communicate through bounded buffers, even though low latency is not required.
- **[I] Job boundary:** typed native events internally; preserve the existing JSON cursor/envelope schema only for compatibility adapters, IPC or a future web view. Cancellation token checks between stages and within long owned loops; no pretend cancellation of uninterruptible third-party work. Keep one active job until output commits.
- **[I] GUI:** build native controls, preserve locale keys and format placeholders, read-only system information and runtime theme/skin/language changes. Implement platform title-bar behavior explicitly. Do not pay for Avalonia WebView just to reproduce S1 badly.
- **[I] Packaging:** self-contained ordinary .NET first; separately qualify AOT with PortAudio callbacks, plot/font rendering and updater maintenance startup. Windows Velopack, macOS app/DMG wrapping, Linux package/AppImage feasibility spike. Bundle CLI without initializing Avalonia.
- **[I] Reports:** f64 analysis data to ScottPlot PNG and offline HTML, with deterministic serialization. Do not mistake image HTML for interactive HTML.
- **[I] Why it competes:** deepest cohesive application tooling of the new native alternatives, straightforward domain tests and no browser shell. **Main cost:** replacing both Python DSP and the UI together unless a temporary worker is retained.

### 3.2 Top contender C: C++ + Qt 6 Widgets

**[I] Sketch; prefer this over QML unless a redesigned visual language is a separate goal.**

```text
Qt Widgets desktop                              impulcifer CLI
  native dialogs, locale catalog adapter             |
              \                                     /
                 Qt-free application/core API
       Stage pipeline + cancellation + structured events
                         |
            double DSP and format/parser modules
     pocketfft + selected kernels + validated bounded fit
                         |
        +----------------+----------------+
        |                |                |
   PortAudio adapter   WAV writer     ffmpeg supervisor

Qt plot/offscreen PNG renderer + standalone HTML report renderer
Optional pybind11/maturin-independent Python packaging adapter
Velopack C/C++ updater; dynamically linked Qt LGPL deployment
```

- **[I] Keep Qt out of the core.** No QObject ownership, GUI event loop or Qt containers required for batch DSP. Expose C++ value types and a narrow C ABI only where Python or another shell needs it.
- **[I] Numerical implementation:** use double pocketfft; port short deterministic filters/interpolation/resampling with tests. First retain the Python fitter in a worker or implement the relevant reference TRF. Evaluate Ceres as an intentional optimizer change, not equivalent machinery.
- **[I] UI/plots:** Widgets forms and a reusable plot view consume report data. Native plots need accessible textual summaries and CSV alternatives. Render PNG with the same scales/style model; emit an independent HTML document with vendored JS.
- **[I] Licensing:** dynamically link only the selected LGPL Qt modules; maintain notices, library source availability and replacement/relink rights. Exclude Qt Charts/QCustomPlot unless separately licensed or intentionally adopting GPL distribution. Include pocketfft/other numeric notices and audit the exact native dependency closure.
- **[I] Packaging:** CMake/pinned dependency manifest, per-OS builds, staged Qt deployment then installer/updater packaging. No WebEngine in the default native design. A WebEngine + QWebChannel variant is a separate decision that preserves UI but forfeits the small-native-distribution rationale.
- **[I] Why it competes:** mature desktop primitives, direct native audio/numeric integration, optional genuine Python extension and a clean headless core. **Main cost:** C++ safety/build/compliance obligations plus native UI replacement.

### 3.3 Recommended transition architecture H

**[I] The lowest-risk first release is not either full port.**

```text
Existing static webview_ui + thin transport adapter
                         |
               Tauri native shell / dialogs
                         |
      supervised Python worker, onedir or interpreter tree
               framed JSON messages, private stdio
                         |
       existing Python application/DSP/report modules
                         |
             optional Rust stages via PyO3
```

- **[I] Use a headless worker entry point**, not the current pywebview bridge module, so packaging does not import Python GUI stacks inadvertently. Reuse the service's JSON semantics and keep native dialog/open-path operations in the shell. Do not modify frozen CTk's architecture.
- **[I] Preserve start/poll/cancel behavior and error envelopes** through a transport shim. Version the protocol; bind worker lifetime to parent; capture stderr separately from framed stdout; cap event queues; support worker crash/error recovery. Do not serialize multichannel WAV arrays as JSON: use controlled files or explicit binary transfer.
- **[I] No localhost HTTP server is needed.** Private stdio avoids unauthenticated local service exposure and port-management work. If future IPC changes, permissions/authentication must be designed explicitly.
- **[I] Package worker, shell and native dependencies atomically.** Keep updater package identity, verify all three platforms, and perform upgrade tests from the currently installed Velopack version. Choose one updater owner; never let Python and the shell race to install updates.
- **[I] Preserve ordinary PyPI separately.** The desktop worker can carry a fixed interpreter while pip users continue using supported Python versions. PyO3 mixed packaging can maintain public wrappers; separate standard/free-threaded wheels where required. [61–63]
- **[I] Retire dependencies only after their last use disappears.** Porting one convolution does not remove SciPy if spline/optimization still imports it; replacing plots without the numerical algorithms does not remove Python.

### 3.4 Acceptance gates for R1–R9 and numerical parity

**[I] Suggested gates; thresholds are proposed starting points, not validated tolerances.**

1. **Hardware gate before UI rewrite.** Test Windows MME, DirectSound and WASAPI, optional ASIO; macOS CoreAudio; a documented best-effort Linux backend matrix. For each supported device combination, enumerate/group, query exact format/rate/count support, play distinct markers on all 16 outputs, capture two input channels, run 44.1/48/96 kHz, verify playback drain and retained capture duration. Include mixed devices and long captures. Log overrun/underrun, actual rate/count and clock drift. A host label alone is not proof of routing. PortAudio's ASIO one-device and shared-device/multiple-stream restrictions need explicit unsupported-case errors. [11]
2. **Keep exact integer/discrete contracts.** Require exact channel order, sample count, sample rate, lag conventions where unambiguous, mask/layout, filenames, output recovery metadata, structured errors, event sequence and parser decisions. Fix these before floating-point tolerances obscure them.
3. **Golden intermediate arrays.** Persist input/outputs for FFT/deconvolution, alignment, room correction, headphone EQ, minimum-phase FIR, resampling, virtual bass, microphone correction, decay and normalization. Record dtype, shape, units, axes, options, dependency versions and reference commit. Capture before quantization/export.
4. **Primitive tolerances.** For normalized benign f64 primitives, begin with `atol=1e-12, rtol=1e-10`, then adjust only from documented conditioning/error analysis. High-Q IIR, tiny magnitudes and ill-conditioned spline/optimization cases need separate gates. These values are not a universal acceptable-error budget.
5. **Frequency/time validation.** Compare complex transfer where phase matters; magnitude in dB only above a documented floor, plus impulse time-domain absolute/relative residual, channel alignment, decay and interaural metrics. A proposed 0.01 dB FR bound can be an engineering starting point, not a proven perceptual threshold. Do not hide polarity/ITD changes behind a magnitude-only pass.
6. **Optimizer validation.** Reuse existing seeds, bounds and evaluation budgets. Verify fit response, residual cost, stability and pruning; parameter equality is not always meaningful for interchangeable/degenerate biquads. But different filter count/order that changes emitted EQ files remains a semantic decision. If switching from TRF to bounded LM/Ceres, document the change explicitly and test difficult/flat/near-boundary curves, not only the demo.
7. **Separate container parity from DSP parity.** First export legacy PCM_32 and compare against same-machine reference; then test optional R3 FLOAT/WAVEX independently for 1–32 tracks and downstream player compatibility. Reinterpreting a PCM_32 integer file as float data is not conversion. Ensure recovery output matches the chosen format policy.
8. **Maintain two reproducibility contracts.** Cross-language migration uses tolerances plus exact discrete properties. Same-build deterministic regressions may still use SHA-256. A different language is not a mathematical prohibition on byte equality, especially with identical kernels, but it is unrealistic to require arbitrary rewritten libraries to match NumPy/SciPy bytes universally. Freeze a new baseline only after approval of measured differences.
9. **End-to-end packaging gate.** Test CLI without GUI/display, cold launch without a development environment, dialogs, nine locales, theme/titlebar, long jobs/cancel, ffmpeg download/decode failure, 32-track output, PNG/HTML offline use, old-to-new updater transition, and damaged/missing sidecar behavior. NativeAOT, compressed installer size and startup benchmarks come from these binaries, not framework marketing.

## 4 Risks ranked

| Rank | Risk | Severity / why | Concrete response [I] |
|---:|---|---|---|
| 1 | False scientific equivalence | Critical: generic FFT/LM/minimum-phase names conceal different lengths, boundaries, scaling and optimizer branches | Build per-stage oracle; preserve discrete semantics; forbid float32 intermediate shortcuts; require evidence for changed fit algorithms |
| 2 | Rewriting DSP and UI simultaneously | Critical schedule/correctness risk for one maintainer | Sidecar first or keep a reference worker; one independently testable stage per port |
| 3 | Assumed multichannel hardware support | Critical to measurement; no candidate was tested on actual 16-output hardware | Explicit host/rate/channel device matrix and two-stream tests before committing to a stack |
| 4 | Licensing surprise | High: JUCE 8 AGPL/commercial; Qt Charts/QCustomPlot GPL/commercial; Avalonia WebView commercial; LGPL obligations remain | Pin exact source/licenses; choose distribution policy before dependency adoption; confirm commercial eligibility where relevant |
| 5 | Treating a packager change as dependency elimination | High: numerical DLLs, fonts, report engines, audio drivers and WebViews still ship | Document removed versus retained failure classes; use immutable dependency locks and clean-machine packaged tests |
| 6 | Output-format misunderstanding | High: requested FLOAT differs from current PCM_32, including recovery path | Keep format-policy test separate from numerical tests and hash baselines |
| 7 | Updater migration and subprocess lifecycle | High: two installed runtimes, worker crashes, mismatched versions, partial replacements | One updater authority; atomic package version; protocol handshake; rollback/upgrade tests from existing installations |
| 8 | GUI maturity/accessibility and report regression | Medium-high for pre-1.0 Rust GUI/custom charts | Prefer stable control sets; screen-reader/keyboard tests; preserve data tables/CSV and standalone reports |
| 9 | NativeAOT or “single binary” optimism | Medium-high: reflection, callbacks, fonts, native libraries and platform runtime assumptions | Make AOT an optional release configuration until proven; prefer inspectable multi-file installer payloads |
| 10 | Oversubscribed parallelism | Medium: per-speaker pools multiplied by FFT/BLAS threads increase memory and variance | One global worker budget, deterministic reductions, cancellation-aware task boundaries and measured memory limits |
| 11 | Moving-source/version contamination | Medium: main/latest pages differ from released crates, old tutorials misstate licenses/API support | Pin release tags and lockfiles; keep referenced documentation with implementation tasks; never use “latest” as a version |
| 12 | AI-generated plausible but wrong API composition | Medium-high: invented TRF support, assumed blocking bindings, wrong GUI version pairings | Give agents exact versions/signatures and minimal compile tests; reject unverified API names; human owns architecture and gates |

## 5 Open questions you could not settle

1. **[U] Real audio combinations:** which actual interfaces/drivers must expose 16 outputs, at which rates, through which host APIs? Is MME a permanent requirement or compatibility convenience? Is an ASIO-only single-device configuration acceptable when separate streams cannot coexist?
2. **[U] Current sounddevice overlap:** source intent and documented convenience-call behavior conflict. The research did not reproduce recording on hardware; do not use current two-call code as the conformance oracle for stream coexistence. [96]
3. **[U] Distribution licensing preference:** must all combined executables remain redistributable under permissive terms, or would AGPL/commercial JUCE be acceptable? The repository being MIT does not answer willingness to distribute a combined work differently.
4. **[U] JUCE commercial eligibility:** no revenue/funding, entity or contributor licensing facts were supplied. Free Starter eligibility was not established. [13]
5. **[U] Numerical tolerance approval:** acceptable FR/phase/ITD/decay and optimizer-fit differences require measured reference cases and maintainer approval. The suggested thresholds are not established project policy.
6. **[U] Output format:** should R3 replace integer PCM_32 or add FLOAT as an option? Do all target consumers accept the chosen float/WAVEX layout with up to 32 tracks?
7. **[U] AOT dependency closure:** no complete Avalonia + PortAudio interop + ScottPlot/font + Velopack NativeAOT executable was compiled or tested. Library-level AOT documentation is insufficient. [3–4,10]
8. **[U] Artifact size, cold start and RAM:** no apples-to-apples application binaries exist here. Electron's weight being “a wash” remains a hypothesis, not a finding.
9. **[U] PortAudio wrapper maintenance choice:** PortAudioSharp2's callback behavior was checked, but no exact pinned package/native binary set passed the required matrix. Rust/Go wrappers also require shipped-backend and binary validation.
10. **[U] Latest-version cutoff:** some Flutter/Compose/GPUI/current documentation pages were not chronologically reliable. Their capabilities were used with qualifications; a release selection requires tagged-source/dated-release checks.
11. **[U] Native chart completeness:** the full ILD/IPD/IACC/spectrogram/waterfall suite, screen-reader semantics, font subsets and nine-locale exports were not implemented in any proposed chart library.
12. **[U] Linux support floor and updater continuity:** oldest Linux/glibc/WebKit baseline, actual macOS signing requirements and Velopack application-identity migration need explicit product decisions and clean-machine tests.
13. **[U] REW's original language-selection rationale:** the evidence establishes a working Java precedent and native-driver caveats, not the author's historical motives.
14. **[U] Retained Python surface:** whether PyPI must preserve imports/classes/NumPy-array API or only CLI installation materially changes the value/cost of PyO3, pybind11 and C# wrappers.

## 6 Sources

All URLs below were checked directly or through a delegated primary-source research pass for this report on 2026-09-07. Versioned links identify the evidence baseline; unversioned links do not assert a release date. Local paths in §2.2 are repository evidence, not public URLs. A few direct pages failed to fetch; where relevant, an official source repository or separate primary page supplied the evidence instead.

1. [.NET support policy](https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core).
2. [Avalonia 11.3.0 license](https://github.com/AvaloniaUI/Avalonia/blob/11.3.0/licence.md).
3. [Avalonia v11 NativeAOT deployment](https://v11.docs.avaloniaui.net/docs/deployment/native-aot/).
4. [Microsoft NativeAOT deployment and limitations](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/).
5. [NWaves package/version](https://www.nuget.org/packages/NWaves); [precision guidance](https://github.com/ar1st0crat/NWaves/wiki/Notes-for-non~experts-in-DSP).
6. [NWaves transforms and Fft64 constraints](https://github.com/ar1st0crat/NWaves/wiki/Transforms).
7. [Math.NET Numerics package versions](https://www.nuget.org/packages/MathNet.Numerics).
8. [Math.NET 5 LevenbergMarquardtMinimizer](https://numerics.mathdotnet.com/api/MathNet.Numerics.Optimization/LevenbergMarquardtMinimizer.htm); [trust-region API namespace](https://numerics.mathdotnet.com/api/MathNet.Numerics.Optimization.TrustRegion/index.htm).
9. [NAudio 2.2.1 source](https://github.com/naudio/NAudio/tree/v2.2.1).
10. [PortAudioSharp2](https://github.com/csukuangfj/PortAudioSharp2); [Stream source](https://github.com/csukuangfj/PortAudioSharp2/blob/master/PortAudioSharp/Stream.cs).
11. [PortAudio v19 API overview](https://portaudio.com/docs/v19-doxydocs/api_overview.html); [API header reference, stream/format/drain rules](https://portaudio.com/docs/v19-doxydocs/portaudio_8h.html); [device enumeration](https://portaudio.com/docs/v19-doxydocs/querying_devices.html).
12. [JUCE 8.0.0 AGPL/commercial license](https://github.com/juce-framework/JUCE/blob/8.0.0/LICENSE.md).
13. [JUCE 8 commercial EULA](https://juce.com/legal/juce-8-licence/).
14. [JUCE 8.0.0 FFT header](https://raw.githubusercontent.com/juce-framework/JUCE/8.0.0/modules/juce_dsp/frequency/juce_FFT.h).
15. [JUCE Convolution API, unversioned](https://docs.juce.com/master/classjuce_1_1dsp_1_1Convolution.html).
16. [JUCE 8.0.0 AudioIODevice header](https://raw.githubusercontent.com/juce-framework/JUCE/8.0.0/modules/juce_audio_devices/audio_io/juce_AudioIODevice.h).
17. [JUCE 8 WebView feature overview](https://juce.com/blog/juce-8-feature-overview-webview-uis/).
18. [Qt 6.8 LGPL text](https://doc.qt.io/qt-6.8/lgpl.html).
19. [Qt Charts 6.8 licensing](https://doc.qt.io/qt-6.8/qtcharts-index.html); [Qt Charts deprecated since 6.10](https://doc.qt.io/qt-6.10/qtcharts-index.html).
20. [QCustomPlot licensing](https://www.qcustomplot.com/index.php/license).
21. [QCustomPlot downloads/version/compatibility](https://www.qcustomplot.com/index.php/download).
22. [Qt 6.8 QAudioDevice](https://doc.qt.io/qt-6.8/qaudiodevice.html); [Qt 6.8.0 audio format source](https://raw.githubusercontent.com/qt/qtmultimedia/v6.8.0/src/multimedia/audio/qaudioformat.h).
23. [Qt 6.8 WebEngine platform/build notes](https://doc.qt.io/qt-6.8/qtwebengine-platform-notes.html).
24. [Qt 6.8 WebEngine licensing](https://doc.qt.io/qt-6.8/qtwebengine-licensing.html).
25. [Qt 6.8 QWebChannel](https://doc.qt.io/qt-6.8/qwebchannel.html).
26. [egui 0.36.1](https://docs.rs/crate/egui/0.36.1).
27. [egui_plot 0.37.0](https://docs.rs/crate/egui_plot/0.37.0).
28. [egui accessibility guidance](https://github.com/emilk/egui/blob/main/docs/accessibility.md).
29. [iced 0.14.0](https://docs.rs/crate/iced/0.14.0); [AccessKit draft PR 3111](https://github.com/iced-rs/iced/pull/3111).
30. [Slint 1.17.1](https://docs.rs/crate/slint/1.17.1).
31. [Slint development/accessibility practices](https://docs.slint.dev/latest/docs/slint/guide/development/best-practices/).
32. [Slint licensing options](https://slint.dev/get-started).
33. [Dioxus Desktop 0.7.10](https://docs.rs/crate/dioxus-desktop/0.7.10); [desktop renderer architecture](https://dioxuslabs.com/learn/0.7/guides/platforms/desktop/).
34. [GPUI 0.2.2](https://docs.rs/crate/gpui/0.2.2); [current GPUI README](https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md).
35. [GPUI AccessKit PR 56065](https://github.com/zed-industries/zed/pull/56065).
36. [flutter_rust_bridge package](https://pub.dev/packages/flutter_rust_bridge).
37. [Flutter desktop support](https://docs.flutter.dev/platform-integration/desktop); [Flutter architecture](https://docs.flutter.dev/resources/architectural-overview).
38. [Flutter Windows distribution](https://docs.flutter.dev/platform-integration/windows/building).
39. [fl_chart package](https://pub.dev/packages/fl_chart).
40. [Flutter PNG capture API](https://api.flutter.dev/flutter/rendering/RenderRepaintBoundary/toImage.html); [assistive technologies](https://docs.flutter.dev/ui/accessibility/assistive-technologies).
41. [JTransforms](https://github.com/wendykierp/JTransforms); [DoubleFFT_1D source](https://github.com/wendykierp/JTransforms/blob/master/src/main/java/org/jtransforms/fft/DoubleFFT_1D.java).
42. [Java AudioFormat.Encoding / PCM_FLOAT](https://docs.oracle.com/en/java/javase/25/docs/api/java.desktop/javax/sound/sampled/AudioFormat.Encoding.html).
43. [REW audio driver guidance](https://www.roomeqwizard.com/help/help_en-GB/html/calsoundcard.html); [REW product/distributions](https://www.roomeqwizard.com/).
44. [Compose desktop native distributions](https://kotlinlang.org/docs/multiplatform/compose-native-distribution.html).
45. [Compose desktop accessibility](https://kotlinlang.org/docs/multiplatform/compose-desktop-accessibility.html).
46. [Conveyor documentation](https://conveyor.hydraulic.dev/latest/).
47. [Web Audio specification](https://webaudio.github.io/web-audio-api/).
48. [AudioDestinationNode.maxChannelCount](https://developer.mozilla.org/en-US/docs/Web/API/AudioDestinationNode/maxChannelCount); [AudioContext.setSinkId](https://developer.mozilla.org/en-US/docs/Web/API/AudioContext/setSinkId).
49. [MediaTrackConstraints](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackConstraints); [supported constraints](https://developer.mozilla.org/en-US/docs/Web/API/MediaTrackSupportedConstraints).
50. [AudioBuffer float32 storage](https://developer.mozilla.org/en-US/docs/Web/API/AudioBuffer); [AudioContext sample rate](https://developer.mozilla.org/en-US/docs/Web/API/AudioContext/AudioContext).
51. [Directory picker](https://developer.mozilla.org/en-US/docs/Web/API/Window/showDirectoryPicker); [OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system); [File API](https://developer.mozilla.org/en-US/docs/Web/API/File_API); [download attribute](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/a#download).
52. [WebAssembly numeric types](https://webassembly.github.io/spec/core/syntax/types.html).
53. [Emscripten pthreads](https://emscripten.org/docs/porting/pthreads.html).
54. [Rust wasm32-unknown-unknown limitations](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html); [wasm-bindgen-rayon](https://github.com/RReverser/wasm-bindgen-rayon).
55. [Tauri 2 external binaries/sidecars](https://v2.tauri.app/develop/sidecar/).
56. [PyInstaller 6.22.2 manual](https://pyinstaller.org/en/stable/).
57. [PyInstaller onefile/onedir operation](https://pyinstaller.org/en/stable/operating-mode.html).
58. [PyOxidizer maintainer status discussion](https://github.com/indygreg/PyOxidizer/discussions/740).
59. [PyOxidizer native extension packaging restrictions](https://pyoxidizer.readthedocs.io/en/stable/pyoxidizer_packaging_extension_modules.html).
60. [Astral stewardship of python-build-standalone](https://astral.sh/blog/python-build-standalone); [distribution/install_only documentation](https://gregoryszorc.com/docs/python-build-standalone/main/distributions.html).
61. [PyO3 0.26.0 free-threading guide](https://pyo3.rs/v0.26.0/free-threading.html).
62. [maturin mixed project layout](https://www.maturin.rs/project_layout.html).
63. [maturin bindings/wheel ABI guidance](https://www.maturin.rs/bindings.html).
64. [Zig 0.15.2 language reference](https://ziglang.org/documentation/0.15.2/); [cross-compilation overview](https://ziglang.org/learn/overview/).
65. [Nim releases](https://nim-lang.org/); [C types](https://nim-lang.org/docs/ctypes.html); [memory management](https://nim-lang.org/docs/mm.html).
66. [Swift 6 announcement](https://www.swift.org/blog/announcing-swift-6/); [platform support](https://www.swift.org/platform-support/); [C/C++ interop](https://www.swift.org/documentation/cxx-interop/).
67. [D compiler downloads](https://dlang.org/download.html); [C interop](https://dlang.org/spec/interfaceToC.html); [GC and @nogc](https://dlang.org/spec/garbage.html).
68. [SciPy 1.16.2 least_squares](https://docs.scipy.org/doc/scipy-1.16.2/reference/generated/scipy.optimize.least_squares.html); [minimum_phase](https://docs.scipy.org/doc/scipy-1.16.2/reference/generated/scipy.signal.minimum_phase.html); [SciPy 1.15.3 firwin2](https://docs.scipy.org/doc/scipy-1.15.3/reference/generated/scipy.signal.firwin2.html).
69. [Velopack C#](https://docs.velopack.io/getting-started/csharp); [C/C++](https://docs.velopack.io/getting-started/cpp); [Rust](https://docs.velopack.io/getting-started/rust); [packaging overview](https://docs.velopack.io/packaging/overview); [cross-packaging restrictions](https://docs.velopack.io/packaging/cross-compiling).
70. [Python.NET CLR embedding](https://pythonnet.github.io/pythonnet/python.html).
71. [NativeAOT shared native libraries](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/libraries).
72. [Python wheel format specification](https://packaging.python.org/en/latest/specifications/binary-distribution-format/).
73. [PortAudio blocking I/O](https://portaudio.com/docs/v19-doxydocs-dev/blocking_read_write.html).
74. [ScottPlot 5 heatmap and PNG examples](https://scottplot.net/cookbook/5/Heatmap/); [ScottPlot API/HTML image helpers](https://www.scottplot.net/quickstart/api/); [license](https://github.com/ScottPlot/ScottPlot/blob/main/LICENSE).
75. [LiveCharts2 project/license/platforms](https://github.com/Live-Charts/LiveCharts2).
76. [OxyPlot project/license](https://github.com/oxyplot/oxyplot).
77. [OxyPlot.Avalonia package history](https://www.nuget.org/packages/OxyPlot.Avalonia).
78. [Standalone interactive HTML export pattern](https://plotly.com/python/interactive-html-export/).
79. [Avalonia 11 WebView platform support](https://v11.docs.avaloniaui.net/accelerate/components/webview/quickstart/); [Accelerate licensing/setup](https://v11.docs.avaloniaui.net/accelerate/installation/).
80. [JUCE 8.0.0 device-type factory header](https://raw.githubusercontent.com/juce-framework/JUCE/8.0.0/modules/juce_audio_devices/audio_io/juce_AudioIODeviceType.h).
81. [Qt 6.8 QPainter](https://doc.qt.io/qt-6.8/qpainter.html); [QImage/PNG](https://doc.qt.io/qt-6.8/qimage.html).
82. [pybind11 C++/Python/NumPy integration](https://pybind11.readthedocs.io/en/stable/).
83. [CPAL current backend matrix](https://github.com/RustAudio/cpal); [CPAL 0.18.2 crate](https://docs.rs/crate/cpal/0.18.2).
84. [Wails v2 prerequisites](https://v2.wails.io/docs/gettingstarted/installation/); [Windows WebView2 deployment](https://v2.wails.io/docs/guides/windows/).
85. [Tauri 2 signed updater/platform artifacts](https://v2.tauri.app/plugin/updater/).
86. [Electron process model](https://www.electronjs.org/docs/latest/tutorial/process-model); [native module ABI/rebuild guidance](https://www.electronjs.org/docs/latest/tutorial/using-native-node-modules).
87. [Go PortAudio wrapper](https://github.com/gordonklaus/portaudio); [blocking API](https://pkg.go.dev/github.com/gordonklaus/portaudio); [Gonum optimize v0.17.0](https://pkg.go.dev/gonum.org/v1/gonum@v0.17.0/optimize).
88. [pocketfft C++ double/arbitrary-length FFT and BSD license](https://github.com/mreineck/pocketfft).
89. [Ceres bounded nonlinear least-squares algorithms, official source](https://github.com/ceres-solver/ceres-solver/blob/master/docs/source/nnls_solving.rst); [Ceres license](https://github.com/ceres-solver/ceres-solver/blob/master/LICENSE).
90. [SciPy BSD license](https://github.com/scipy/scipy/blob/main/LICENSE.txt).
91. [libsndfile supported formats, including float WAV](https://libsndfile.github.io/libsndfile/formats.html). This page alone does not establish all 32-channel/WAVEX interoperability.
92. [Rust Plotters bitmap/PNG/SVG renderer](https://docs.rs/plotters/latest/plotters/). A potential separate report backend, not a verified complete Bokeh replacement.
93. [Flutter SDK archive](https://docs.flutter.dev/install/archive). Latest cutoff patch not established from the fetched archive.
94. [Compose 1.12.0 release page](https://github.com/JetBrains/compose-multiplatform/releases/tag/v1.12.0). Not used as proof of latest stable at the cutoff.
95. [PyOxidizer repository](https://github.com/indygreg/pyoxidizer). Inactivity discussion, not a confirmed archival claim, governs the recommendation.
96. [sounddevice 0.5.3 convenience-function lifecycle](https://python-sounddevice.readthedocs.io/en/0.5.3/api/convenience-functions.html).
