# Impulcifer rewrite research: precedents and failure lessons

Observation date: **2026-09-07**. Research only; no project files changed.

## 1 Verdict

1. **I — C++/Qt has the strongest directly comparable measurement precedent**, especially Open Sound Meter; the maintainer's EqualizerAPO-XT also supplies relevant build and regression-test experience.
2. **I — Among the proposed shells, provisionally prefer Rust/Tauri 2**, because retaining the existing web UI is valuable and native Rust audio is demonstrated; this survey does not establish a complete SciPy replacement.
3. **I — C++/Electron is defensible, not disqualified by download size.** Its stronger argument is one controlled renderer; its costs include Chromium maintenance and native packaging, not merely megabytes.
4. **I — Go/Wails ranks third for this particular rewrite.** Real native-audio examples exist, but their C/C++ dependencies undermine the supposed Go-only toolchain advantage.
5. **V — Current EasyEffects is Qt/QML, not GTK4; Audacity 4 also rebuilt its interface on Qt.** Historical toolkit labels are insufficient. [26,37]
6. **V — Musicat is a real Tauri/native-Rust audio example; Meadowlark/Yarrow and nih-plug-webview are not current Tauri-app precedents.** [82–87,104–105]
7. **U — No surveyed app establishes the full required 16-output/two-independent-input-stream contract or all required float64 numerical primitives.**
8. **I — The safest architecture decision is a testable headless engine and replaceable desktop adapter, with the current Python implementation retained as a numerical oracle during migration.**
9. **U — Actual maintainer counts and many artifact sizes are not public in the inspected evidence.** Contributor totals, tiny download bootstrappers and source archives are not substitutes.

## 2 Findings (with sources)

### Evidence rules

- **V**: verified in the cited primary source or inspected local source. It verifies what the source says, not successful execution on this research machine.
- **I**: inference/recommendation. It is not a documented fact about a project's performance or authors' intentions.
- **U**: unknown, inaccessible, or not established by the inspected evidence.
- Versions below are **observed versions**, not a promise that all channels agree. A development manifest is explicitly distinguished from a shipped version. Historical issues retain their original dates/versions.
- All projects' **current active-maintainer counts are U**, unless stated otherwise in a row. No inspected source established a defensible count. Named authors and repository owners identify provenance, not bus factor.
- Sizes are compressed download artifacts unless stated otherwise. They are not installed sizes, peak RAM, or equivalent functionality. A displayed rounded MB value remains rounded; no benchmark was run.

### 2.1 What the local code actually requires

| Observation | Verified evidence | Consequence for reading the precedents |
|---|---|---|
| Two independent blocking audio operations | **V** `record_target()` uses `rec(... blocking=True)`; `play_and_record()` starts a recording `Thread`, calls `play(... blocking=True)`, then joins the recorder. Host API names are enumerated. [1, lines 117–139,188–190,440–494] | **I** A DAW proving duplex callback playback is not proof of this contract. Keep capture tests separate from GUI tests. |
| Existing job-oriented web boundary | **V** `start_brir()` executes the pipeline with a cancellation scope; `poll_job(job_id, after_seq)` returns sequenced events; `cancel_job()` records a request rather than instantly killing work. [2,428–553] | **I** Preserve the public request/event semantics while changing the implementation. Do not turn bulk audio into JSON payloads. |
| Actual bounded optimizer, not just filter evaluation | **V** peak-based initialization, log-frequency/log-Q variables, explicit bounds, residual evaluation and `least_squares` appear in the optimizer. `max_time` becomes a deterministic function-evaluation budget. [3,350–516] | **I** A library with RBJ biquads or FFTs has not demonstrated replacement of the fitting algorithm. |
| Frontend is already bridge-oriented | **V** `api()` returns `window.pywebview.api`; polling and start/cancel calls use that adapter. [4,57–58,617–630,1339–1343] | **I** Retaining vanilla JS avoids a separate visual-interface rewrite. A shell precedent using React/Svelte does not make either necessary. |
| Stronger baseline than the brief's single hash example | **V** integrity tests cover default headphone compensation, virtual bass, shaping, resampling/extra outputs, and no-headphone-compensation scenarios; canonical execution is Linux CPython 3.13. [5,39–61,165–175] | **I** Port every scenario and add intermediate golden data; do not reduce coverage to a single convenient demo. |
| Existing native-frontend decision | **V** ADR 0001 rejects forcing CTk through the JSON/webview service. [6] | **I** The proposed new web-shell adapter does not justify retrofitting the frozen CTk frontend. |
| Filter representation details matter | **V** `autoeq/biquad.py` returns normalized feedback coefficients with its particular sign convention; `core/eqapo.py` explicitly models XT semantics and logs commands it cannot represent as magnitude response. [7,22–49;8,1–28] | **I** Compare transfer functions and recurrence conventions, not just arrays named `a` and `b`. |

### 2.2 Closest open-source and established audio precedents

**Maint.** below means current active maintainers, not historical authors.

| Tool | Implementation / shell | Observed version and platforms | Distribution evidence; Maint. | Documented pain and relevance |
|---|---|---|---|---|
| **REW, Room EQ Wizard** | **V** Java, including Java 17 in recent beta packages. **U** exact GUI toolkit from inspected sources; do not assert Swing merely from appearance. [14–16] | **V** stable 5.31.3 (2024-07-25); 5.40 beta 135 (2026-09-06). Windows/macOS/Linux. | **U** size; bundled-runtime and external-runtime variants exist. **U** Maint. | **V** Java 11 font fallback on Chinese macOS locales prompted a Java 17 change; ASIO Int32LSB24 decoding was fixed in 2023; actual/requested sample-rate mismatch warning added in 2021. **I** Managed code is viable for measurement; driver formats and typography remain platform-specific. [15] |
| **Equalizer APO** | **V** C++, Qt configuration GUI, Windows APO engine. [17] | **V** 1.4.2 (2025-03-21); Windows x86/x64/ARM64 packages. [18] | **V** x64 installer displayed as 12.0 MB; exact bytes **U**. **U** Maint. | **V** ASIO/exclusive WASAPI bypass normal system APO processing. 1.4.2 addressed Bluetooth convolution/GraphicEQ problems and non-AVX CPU crashes. **I** It proves native convolution deployment, not independent measurement acquisition. [17–18] |
| **EqualizerAPO-XT** | **V** C++; Qt 6.10.1; MSVC/MSBuild, qmake/nmake, PowerShell provisioning, Velopack, GitHub Actions. Libraries include Highway, FFTW, libsndfile and VST interfaces. **U** exact compiler version from inspected manifest. [9–11,13] | **V** 2.51.0, 2026-09-03; Windows x64 SSE2/AVX/AVX2/AVX-512/AVX10.1 and ARM64 NEON variants. [12–13] | **V** full AVX2 installer **73,045,809 bytes**; CPU-selector bootstrap **222,720 bytes**, not equivalent products. **U** Maint.; repository owner 115dkk is not a count. [12] | **V** provisioning comments document aqt extraction races and a runner-image vcpkg binary/pinned-script mismatch that broke SSE2/AVX builds. Qt extraction is serialized; matching vcpkg is bootstrapped. Unsigned releases and incomplete hardware qualification are disclosed. **I** The maintainer already has relevant C++ delivery experience, but changing language does not end build drift. [9,11,13] |
| **HeSuVi** | **V** Free Pascal; SourceForge labels Win32 UI. **U** precise toolkit. Uses Equalizer APO for convolution. [19–20] | **V** latest-download filename `HeSuVi_2.0.0.1.exe`; date **U**. Windows. | **V** displayed 27.3 MB; bytes **U**. **U** Maint. | **V** documentation covers driver incompatibilities, surround endpoint requirements and Voicemeeter latency/crackling workarounds. **I** Preserve export/channel compatibility, but do not copy playback-routing complexity into an offline renderer. [20] |
| **ASH-Toolset** | **V** Python, DearPyGui, NumPy/SciPy. [21] | **V** 4.2.2 (2026-03-14); tested Windows 10/11 and Ubuntu 24.04.3; macOS **U**. [21–22] | **V** portable `.7z` displayed **136.1 MB**; includes more than a shell. Exact bytes **U**. **U** Maint. | **V** sample-rate matching, convolver setup and virtual routing remain user responsibilities; longer reverberation increases processing. **I** Very relevant offline BRIR precedent, and evidence that Python packaging is still a rational baseline rather than inherently untenable. [21] |
| **Original Impulcifer** | **V** Python CLI plus Tkinter `gui.py`. [23,25] | **V** tag 1.0.0 (2020-07-20); do not confuse tag with packaged release. Windows/macOS/Linux installation instructions. [23–24] | **U** packaged binary size. **U** Maint. | **V** manual Python/Git/dependency setup, careful microphone placement and repeated orientations; limitations/workarounds for IEM compensation. **I** Measurement guidance and validation are product value independent of engine language. [23] |
| **Audacity** | **V** C/C++; legacy generation wxWidgets, but **4.0 rebuilt the interface on Qt**. [26] | **V** 4.0.0 (2026-09-03); 3.7.9 also listed (2026-09-01). Windows/macOS/Linux. [26–27] | **U** artifact size. **U** Maint. | **V** 4.0 initially omits macro management, scripting pipe, time/MIDI tracks, VAMP/LADSPA and other workflows. `.aup3` converts to `.aup4`; no save-back to `.aup3`. Official Windows builds now include ASIO. **I** A UI modernization can cause a large functional migration even when the native engine survives. [26] |
| **Tenacity** | **V** C++17/wxWidgets; build guide requires wxWidgets ≥3.1.3 and commonly uses 3.2. [28–29] | **V** 1.3.5 (2026-07-06); separate 1.4 alpha. Windows/macOS/Linux artifacts. | **V** Windows x64 download displayed **14 MiB**; FFmpeg separate; exact bytes **U**. **U** Maint. | **V** unsigned macOS package/quarantine instructions, untested Windows ARM plugin compatibility and incomplete automatic dark-mode switching. Windows dependency build storage about 10 GB, not installed size. **I** A small binary does not imply a small toolchain. [28–29] |
| **Ardour** | **V** mainly C++, with C/assembly; GTK/gtkmm and project-owned toolkit forks. [30] | **V** homepage 9.8; date **U**. Windows/macOS/Linux. [31] | **U** size. **U** Maint. | **V** developer documentation describes roughly a million lines including bundled libraries/toolkit forks and asynchronous GUI/engine architecture. **I** Strong engine-separation precedent, poor justification for a solo project to own a toolkit fork. [30] |
| **REAPER** | **I** C++/Win32-style application with SWELL portability is consistent with Cockos WDL and product history. **U** direct current product-wide implementation statement; extension SDK alone is insufficient. [32–34] | **V** 7.79 (2026-08-17); Windows/macOS/Linux. | **V** Windows x64 installer displayed **17 MB**; exact bytes **U**. **U** Maint. | **V** recent history includes device-configuration Apply-button and offline-render FX-state corrections. **I** Its compact distribution is not evidence that a new cross-platform GUI abstraction is cheap to write. [34] |
| **Mixxx** | **V** C++20, Qt6 by default since 2.5. [36] | **V** stable 2.5.6; Windows ≥10 build 1809, macOS ≥11, Linux; separate 2.6 beta. [35] | **V** Windows stable x64 **115.1 MiB** displayed; exact bytes **U**. **U** Maint. | **V** Qt6 migration raised OS minimums; download page identifies stable Flathub packaging as unmaintained. **I** Platform support is an explicit maintenance policy, not a framework checkbox. [35–36] |
| **EasyEffects** | **V** C/C++ and **Qt6/QML/Kirigami**, replacing GTK4 in 8.0.0 (2025-11-09). [37–38] | **V** changelog 8.2.9 (2026-08-31). Linux/PipeWire; supported Windows build **U**. | **U** size. **U** Maint. | **V** migration moved presets/assets; 8.0.2 improved migration; 8.0.5 repaired autoload route/default-device selection. **I** These are compatibility costs of migration, not proof GTK is generally bad or Qt inherently better. [37] |
| **Sonic Visualiser** | **V** C++/Qt; Qt6 required since 5.0, Qt5 removed. [39–40] | **V** 5.2.1 (2025-03-21); Windows, macOS Intel/ARM, Linux AppImage/Ubuntu. | **U** size. **U** Maint. | **V** history documents bundled-Qt macOS crashes, 32-bit Vamp compatibility repairs, image/SVG export hangs and an escape hatch for potentially unstable threaded painting. **I** Native plotting has its own export/threading regression surface. [40] |
| **friture** | **V** Python; released v0.54 uses PyQt5 5.15.11; development `master` declares PyQt6. Do not mix the two. [43–44] | **V** release 0.54; release year **U** from observed page. Windows/macOS/Linux. [41–42] | **U** size; fetched download template did not resolve sizes. **U** Maint. | **V** official instructions warn about unsigned Windows installers and unnotarized macOS packages; history includes an x64-only MSI correction. **I** Changing Python's compiler is not the same as solving signing/distribution trust. [41–42] |

### 2.3 Commercial measurement, correction, HRTF and convolver tools

A Windows executable, Visual C++ redistributable, or native-looking dialog **does not establish implementation language**. Proprietary stacks below stay unknown when vendors do not disclose them.

| Tool | Language / shell | Version / platforms | Size; Maint. | Documented problem or lesson |
|---|---|---|---|---|
| **Open Sound Meter** | **V** C++17, Qt 5.15; `QQmlApplicationEngine` establishes Qt Quick/QML. [45–46] | **V** 1.5.2 (2025-09-17), Windows x64/macOS Intel+ARM/Linux x64. | **V** Windows installer **33,633,848 B**, ARM DMG **37,188,361 B**, Intel DMG **37,997,661 B**, Linux AppImage **110,788,296 B**. **U** Maint. [48] | **V** history includes ASIO fixes, ALSA buffer negotiation, OpenGL/Metal and large-project crashes, improved delay finding. **I** The strongest direct measurement/plotting peer here; still requires GPU and audio qualification. [47] |
| **Smaart Suite** | Language **U**, toolkit **U**. | **V** 9.6.4 observed; patch date **U**. Windows 10+ x64/macOS 10.14+; Windows ARM explicitly unsupported. [49–50] | **U** size; **U** Maint. | **V** fixes cover coherence export, phase averaging, startup with saved delay and SPL recording crashes. **I** Validate saved measurement semantics and numerical exports independently of their visual presentation. [49] |
| **ARTA** | Language **U**, toolkit **U**; do not assert C++/MFC without source. | **V** 1.9.8, installer updated 2024-12-16. Windows; Wine/CrossOver are compatibility routes. [51] | **V** `ArtaSetup198.exe` displayed 11.9 MB including ARTA/STEPS/LIMP/help; bytes **U**. **U** Maint. | **V** author stopped development in 2024 and closed the business but promised licensed-user support. **I** Retirement is not zero maintenance, and long-term build/download continuity matters. [52] |
| **HOLMImpulse** | **V historical** developer post (2009-07-03) says C++ and CLR on Windows; C# and exact toolkit **U**. [53] | Current version **U**; 1.0.8 not verified. Historical Windows use **V**; native macOS/Linux **U**. | **U** size; **U** Maint. | **V historical, search-index evidence** developer describes mixed C++/CLR Linux-port difficulty. Direct forum fetch was blocked; original product page had certificate/retrieval failure. **I** Do not use this as evidence of a currently supported .NET cross-platform app. [53–54] |
| **Dirac Live** | Language **U**, toolkit **U**. Runtime prerequisites do not settle either. | **V observed** desktop 3.14.3 (2026-04-15); Windows x64/macOS, with macOS 12+ for 3.14.1 onward; separate mobile apps. [55–56] | **U** size; **U** Maint. | **V** changes cover discovery/runtime compatibility, disconnect/export hangs and about 5 ms relative delay from a 120 Hz LFE low-pass. **I** Delay bookkeeping and hardware recovery need explicit diagnostics. [55–56] |
| **SoundID Reference** | Language **U**, toolkit **U**. | **V** 5.13.8.1308 (2026-08-03); Windows 10/11/macOS 11+. [57–58] | **U** size; **U** Maint. | **V** Core Audio restart failure hid a virtual device; earlier stale channel/output configuration fixes. USB measurement microphones are explicitly excluded by vendor guidance. **I** Device restrictions are product decisions; do not infer support from the GUI framework. [57–59] |
| **Acourate** | Language **U**, toolkit **U**; Delphi claim not verified. | **V** developer announced V4 (2026-08-09), V4.0.2 available by 2026-08-17; Windows. [60–61] | **U** size; **U** Maint. | **V** developer cautions that a good-looking step response does not guarantee absence of pre-ringing and recommends manual PRC optimization. **I** Test correction artifacts, not only smooth frequency-response plots. [60] |
| **DRC-FIR** | **V** C/C++, CLI/ASCII configuration; supporting Shell/Octave scripts, no bundled GUI. [62–63] | **V** 3.2.3; manual dated 2019-07-26. Windows/Linux, BSD also in project metadata. | **V** 31.6 MB displayed for a **source archive**, not an app installer; binary size **U**. **U** Maint. | **V** documentation discusses clock mismatch, resampling artifacts, aggressive correction and separate convolution playback. **I** Acquisition, correction design and convolver playback should remain separate testable responsibilities. [62] |
| **MathAudio Room EQ** | Language **U**, toolkit **U**. **V** hosted VST/VST3/CLAP/AU/foobar2000 forms. [64] | **V** 3.0.7; foobar component date 2026-04-07; other package dates **U**. Windows x64 10+, macOS 10.13+ Intel/ARM. [64,66] | **U** size; **U** Maint. | **V** guide avoids filling deep dips, requires headroom and level-matched comparison, and specifies calibration-file discovery rules. **I** Preserve transparent correction constraints and calibration provenance. [65] |
| **ITA-Toolbox** | **V** MATLAB toolbox with GUI/plots and programmable acquisition; HRTF measurement documented. [67–68] | Numbered current release **U**; nightly download. Historical Windows/macOS/Linux publications; current support matrix **U**. | **U** size; **U** Maint. | **V** requires MATLAB and Signal Processing Toolbox, PortAudio-compatible acquisition with ASIO preferred. **I** Open-source code need not mean redistributable runtime or easy driver setup. [67–68] |
| **Mesh2HRTF** | **V** numerical HRTF simulation from head geometry, not physical microphone/sweep measurement. [69] | **V** 1.3.0 (2026-03-20). Complete platform/GUI matrix **U** in this survey. [70] | **U** binary size; no attached binary assets in observed latest release. **U** Maint. | **I** Useful numerical-validation neighbor, but not evidence that a desktop acquisition workflow has been solved. |
| **CamillaDSP / CamillaGUI** | **V** Rust CLI/YAML/WebSocket engine; separate React browser GUI with Python backend. **Not a Tauri app.** [71,74–75] | **V** engine 4.1.3 (2026-04-09), Windows/macOS/Linux; GUI version separate. [72–73] | **V** Windows amd64 engine ZIP **2,658,787 B**; Linux ARM64 engine tarball **2,632,436 B**. Excludes GUI/runtime. **U** Maint. | **V** independent clocks require buffering/resampling/rate control; 4.1.3 repairs ring-buffer initialization/sizing. **I** Strong Rust engine/web-control precedent, not an all-in-one 2.7 MB measurement desktop app. [71–73] |
| **BruteFIR** | **V** C99 plus POSIX, CLI/configuration and C extension modules. [76] | **V** 1.1.2 (2026-01-20), **currently Linux**. | **U** source/binary download size; **U** Maint. | **V** 1.1.0 removed FreeBSD/SunOS code and OSS, added PipeWire, changed GPL to ISC and processes to threads. Author calls it legacy/low maintenance. Device-clock mismatch can exhaust buffers. **I** Old references to 1.0o and BSD support are stale. [76] |

### 2.4 PipeWire/Rust precedents: distinguish patchbays from DSP engines

| Tool | Verified stack and maturity | Version / platform / size / Maint. | Relevance and caveats |
|---|---|---|---|
| **Helvum** | **V** Rust/GTK4 PipeWire patchbay; newer package dependencies include libadwaita. [77–78] | **V** published crate 0.4.99 (2023-12-09) is stale; Debian source package 0.6.2+ds-2 observed. Upstream latest **U** because GitLab fetch hit an anti-bot page. Linux documented; size **U**; Maint. **U**. | **V** crates.io distribution is explicitly deprecated: app grew beyond a single executable, so `cargo install` is insufficient. **I** Rust does not guarantee a single-file GUI distribution. It proves graph control, not Impulcifer's numerical pipeline. |
| **pw-viz** | **V** Rust/**egui**, not GTK; PipeWire graph editor. [80] | Version **U**; Linux Arch/Fedora build instructions; other platforms **U**; size **U**; Maint. **U**. | **V** WIP/main-branch instability warning, rough node placement and no zoom. **I** Do not mistake screenshots for a finished cross-platform framework validation. |
| **qpwgraph** | **V** C++20/Qt5 or Qt6/CMake, PipeWire C API; not Rust. [79] | Version **U**; PipeWire/optional ALSA dependencies documented, formal OS matrix **U**; size **U**; Maint. **U**. | **V** Rui Nuno Capela is credited author, not a verified count of current maintainers. **I** Demonstrates that PipeWire usage does not imply a Rust implementation. |
| **zestbay** | **V** Rust + Qt6/QML; native LV2/VST3/CLAP hosts inside PipeWire processing nodes, not only a routing graph. [81] | Version/public binary **U**; Linux; size **U**; Maint. **U**. | **V** LV2 resize-port extension rejects requests, native VST3/CLAP UI embedding uses X11. **I** A useful Rust/native-DSP example, but not Windows/Tauri evidence. |

### 2.5 Candidate shells with actual audio or numerical work

**No row below proves all R1–R9.** The distinction between a shipped app, source implementation and demonstrator is intentional.

| Case | Verified engine and UI relationship | Version/state at observation | Platforms; size; Maint. | Evidentiary weight for Impulcifer |
|---|---|---|---|---|
| **Musicat: Tauri** | **V** Svelte/Tauri UI; Rust decoding → optional Rubato resampling → native peaking-EQ cascade → ring buffer → CPAL callback. RustFFT and Symphonia are declared. It is not merely Web Audio playback. [82–86] | **V** development manifest app 0.17.2/Tauri 2.10.2; actual v0.17.2 release page exists, full date **U**. Pre-1.0 and explicit breakage warning. [87] | **V** macOS/Linux instructions; Windows only in topic metadata, support **U**. Binary size **U**; Maint. **U**. | **I** Real evidence that web UI and native Rust audio coexist. Not a mature 16-channel Windows measurement product. Gapless playback is limited to matching sample rates; macOS signing/notarization is absent. [82] |
| **Dawesome: Tauri** | **V** React/TypeScript invokes Rust; Rodio playback on Windows/macOS, PulseAudio on Linux. [88–89] | **V** manifest 0.1.0/Tauri 1.0.0-rc.5; explicitly very early development. Published binary **U**. | **V** targets Windows/macOS/Linux; actual qualified support **U**. Size **U**; Maint. **U**. | **I** Code-level precedent only. Do not market it as a successful production DAW or current Tauri 2 validation. |
| **Sounder: Electron + native C++** | **V** Electron main loads `sounder_engine.node`; renderer communicates by IPC. CMake builds C++17/N-API 8/node-addon-api/JUCE audio-DSP modules, with offline-rendering sources. [90–92] | **V** manifest 1.0.0-beta.1/Electron ^33.0.0; JUCE 8 documented. **U** shipped binary: beta README and build scripts are not release proof. | **V** README macOS 11+ Intel/ARM; inspected native ONNX download is ARM, so Intel build success **U**. Windows/Linux **U**. Size **U**; Maint. **U**. | **I** An actual implementation of the requested Electron/C++-addon topology, but not proof of mature distribution. AI model download around 2.3 GB is separate from shell size. [90] |
| **Signal Desktop / RingRTC: mature supplemental Electron-native precedent** | **V** native `libringrtc-<arch>.node` Electron module, Rust and WebRTC build integration. This is a mixed native stack, not a simple pure-C++-addon example. [93] | Shipping Electron calling application **V**; exact release as of cutoff **U** in inspected build docs. Do not assign moving-main package versions to a release. | **V** module build targets macOS/Unix/Windows and multiple architectures. App size **U**; Maint. **U**. | **V** initial WebRTC build is explicitly lengthy/resource-intensive; prebuilds and platform-specific setup are documented. **I** Mature native audio is feasible under Electron, but its build complexity is not eliminated by JavaScript. [93] |
| **SoundBoard: Wails** | **V** vanilla JS/WebView2 → Go → malgo/miniaudio WASAPI; WebRTC APM DLL and cgo RNNoise; DSP goroutine and SPSC buffers. [94–95] | **V** `go.mod` declares Go 1.25.5, Wails 2.15.0, malgo 0.11.26. Public release version/date **U**. | **V** Windows 10/11 x64 only; WebView2 and VB-CABLE required. README says **~16 MB**, not a measured versioned artifact. Exact size **U**; Maint. **U**. | **I** Especially relevant vanilla-JS/native-audio precedent. It uses 48 kHz stereo duplex; not evidence for independent 16-channel measurement. Audio needs a C toolchain even if the Wails Windows shell does not. [94–95,113] |
| **OpenUtau: Avalonia** | **V** C#/Avalonia orchestration plus C++ WORLDLINE synthesis/resampling with exported C API and platform-specific native libraries. [96–99] | **V** released product; observed stable 0.1.565 assets dated 2025-09-13. Development csproj declares Avalonia 12.1.0, **not attributed to that stable binary**. [97,100] | **V** Windows/macOS/Linux release/update channels. Exact app size **U**; Maint. **U**. | **I** Stronger deployed managed-shell/native-numerics precedent than a demo. It does not preserve the existing HTML interface; synthesis is not measurement hardware qualification. |
| **Nota: Avalonia** | **V project documentation** Avalonia 12/.NET 10/C++20, C ABI/PInvoke, separate native engine; JUCE limited to plugin hosting. [101] | Specific release/binary **U**. | **V documented targets** Windows/macOS/Linux x64/ARM64; successful shipping **U**. Size **U**; Maint. **U**. | **I** Good architectural example, insufficient evidence of long-term delivery stability. |
| **JUCE WebViewPluginDemo** | **V** JUCE 8.0.3 tagged official source: C++ `dsp::LadderFilter<float>` DSP, parameter relays and spectrum information to a React web UI, bundled web asset ZIP. [102–103] | **V** official demonstrator, not an independent shipped product; demo metadata 1.0.0. | **V** WebKit macOS, WebView2 Windows, GTK WebKit Linux; Linux implementation uses subprocess/XEmbed/X11. Size **U**; Maint. **U**. [116] | **I** Proves that C++/JUCE can retain a web UI without Electron. Float32 filter demo does not supply SciPy's float64 algorithms or measurement/reporting workflows. |
| **Sound-Field: JUCE web UI** | **V README** JUCE C++ mid/side width, waveshaping and RMS/frequency analysis; React/TypeScript/Three.js interface. DSP functions not independently audited here. [106] | JUCE 8.0.3+ documented; app version and binary release **U**. | Xcode/Visual Studio build guidance **V**; supported platform matrix **U**. Size **U**; Maint. **U**. | **I** Implemented source project, not sufficient evidence of a widely deployed JUCE-web ecosystem. |

**Important non-examples**

- **V:** Meadowlark's current GUI work uses **Yarrow**, a Rust retained-mode GUI developed for its DAW. **U:** this survey did not establish a primary-source account of when or why any earlier Tauri approach was abandoned. Do not invent that failure narrative. [104]
- **V:** `nih-plug-webview` is an experimental **Wry-over-baseview NIH-plug editor**, not a Tauri application. The README warns against production use and records an Ableton/macOS Escape-key crash workaround. Sharing a Tauri-maintained webview component does not establish use of Tauri's application shell. [105]
- **V:** CamillaGUI is a separate web application controlling a Rust engine, not a Tauri or Wails desktop application. [71,74–75]
- **I:** The examples prove component arrangements are possible; repository existence and recent commits do not establish commercial viability, maintainability by one person, or Windows audio correctness.

### 2.6 What actually went wrong with the shells/toolchains

| Area | Verified problem/constraint | Appropriate conclusion; what not to claim |
|---|---|---|
| **Tauri/WebKitGTK GPU/packaging** | **V historical** Tauri #9394 (2024-04-06), Tauri 2 beta 14, Ubuntu-built AppImage on EndeavourOS/GNOME/Wayland/NVIDIA: webview resize termination, framebuffer/Wayland errors; environment-variable rendering workarounds. [107] | **I** Linux rendering bugs can survive a Rust rewrite because the system webview remains. **U** whether present Tauri 2 stable plus the user's target distro still reproduces this issue. Do not claim universal current failure. |
| **Electron native ABI** | **V** Electron documents native-module rebuilds, platform/architecture matching and Windows delay-load hooks. Node-API documents ABI stability for its own API/header-only C++ wrapper, excluding direct Node/V8/libuv APIs and external-library ABI. [108–109] | **I** Prefer a restricted Node-API boundary or an independent engine process. **False generalization to avoid:** every Electron update necessarily breaks every N-API-only addon. Compatibility still needs testing. |
| **Electron RAM/performance** | **V** official docs recommend measuring dependencies with CPU/heap profiles and DevTools; process model has main/renderers/optional utilities. No universal RAM figure is supplied. [110–111] | **U** an equivalent Impulcifer idle/peak memory benchmark. **I** The claim 'Nuitka was heavy, so Electron is a wash' is plausible only after measuring like-for-like installers, runtime downloads and RAM. Do not invent a 200 MB/500 MB idle number. |
| **Wails/cgo** | **V** Wails documentation says the Windows shell itself does not require cgo/external DLLs. SoundBoard nevertheless needs cgo/native dependencies for audio. [94–95,113] | **I** `GOOS`/`GOARCH` alone will not cross-build the complete audio application. Native target headers/libraries/toolchain must also work. **U** full current Wails v3 cross-build matrix: direct docs fetch was blocked. |
| **JUCE Linux WebView** | **V** JUCE 8.0.3 uses a GTK child process, XEmbed and forces X11; source comments state Wayland WebKit embedded into an X11 window crashes. [116] | **I** It is supported, but not an uncomplicated Wayland-native path. **False:** JUCE WebView simply does not work on Linux. |
| **JUCE licensing** | **V** JUCE 8.0.3 tagged license offers AGPLv3 or JUCE 8 EULA; third-party licenses remain separate. [114] | **I** Resolve chosen license/distribution terms before adoption. A web interface does not remove JUCE obligations. This report is not legal advice and does not quote an unverified current price. |
| **Qt licensing** | **V** Qt's LGPL guide covers notices, source/offer, modification/relinking and execution rights; some modules are GPL-only; proprietary dynamically linked applications can comply. [115] | **I** Audit actual modules and deployment method. **False:** using Qt necessarily requires releasing the whole app's source, or static linking can be ignored. |
| **Native C++ provisioning** | **V** XT's own scripts document serialized Qt extraction after CI races and matching vcpkg bootstrap after runner drift. [11] | **I** C++ can remove Python-specific failures while introducing or retaining SDK/package/architecture failures. The maintainer's existing expertise reduces learning cost, not the existence of those obligations. |
| **UI rewrite compatibility** | **V** Audacity 4 workflow omissions/project conversion and EasyEffects preset/autoload follow-ups are documented. [26,37] | **I** Functional parity and migration tests must precede declaring the rewrite complete. Do not attribute these costs to one language's inherent quality. |

### 2.7 Size evidence without misleading comparisons

| Artifact | What was measured/reported | Correct use |
|---|---|---|
| XT 2.51.0 full AVX2 setup | **V** 73,045,809 exact bytes. [12] | An actual C++/Qt/Velopack full installer, though not feature-equivalent to Impulcifer. |
| XT CPU-selecting bootstrap | **V** 222,720 exact bytes. [12] | A downloader/selector; **not** a 223 KB audio application. |
| Open Sound Meter 1.5.2 | **V** Windows 33,633,848 B; ARM macOS 37,188,361 B; Linux AppImage 110,788,296 B. [48] | Same product varies substantially by packaging/platform; shell-only comparisons are invalid. |
| CamillaDSP 4.1.3 Windows | **V** 2,658,787 B ZIP, engine only. [73] | Evidence for a compact headless Rust engine, not a complete desktop bundle. |
| ASH-Toolset 4.2.2 | **V** 136.1 MB displayed portable archive. [22] | Python BRIR-tool distribution including more than a renderer. |
| REAPER / Tenacity / Mixxx | **V** displayed Windows downloads 17 MB / 14 MiB / 115.1 MiB. [32,28,35] | Products/assets differ; none isolates Qt/wx/SWELL overhead. |
| Wails SoundBoard | **V** author's '~16 MB' README claim; exact asset unverified. [94] | Qualitative package claim only, excludes separately required WebView2/VB-CABLE considerations. |
| Tauri / Electron-addon / Avalonia examples | **U** comparable exact artifact sizes in this survey. | Do not manufacture a benchmark table or substitute source archive/appcast XML sizes. |

### 2.8 Eight lessons for this decision

1. **I — Choose the engine independently from the shell.** Open Sound Meter supports C++/Qt as a direct peer; CamillaDSP supports Rust with separate web control; OpenUtau supports managed UI/native numerics. None says the GUI's implementation language must own every numerical primitive. [45–46,71,96–99]
2. **I — Preserve the already-written vanilla JS unless there is a specific UX defect that requires replacing it.** Local code already isolates calls through a small bridge. Tauri, Wails, Electron and JUCE webviews can support native-engine arrangements; choosing React/Svelte because a sample uses it adds an unrelated migration. [4,82–103]
3. **I — Prove audio hardware first, with the exact topology.** Music players, duplex voice apps and APO convolvers are not equivalents of two independent streams. Build the 16-output/two-input probe before implementing the numerical engine, especially for Windows host-API selection. PortAudio enumerates host APIs and supports blocking read/write; ASIO permits only one device open at a time, so not every arbitrary device pair is possible. [1,17,94,117]
4. **I — Packaging debt changes rather than disappears.** XT's native provisioning failures, Helvum's non-single-binary packaging, friture signing and RingRTC build prerequisites are concrete counterexamples to simplistic language claims. Use native CI runners, pinned dependencies and packaged-application smoke tests. [11,41,77,93]
5. **I — Limit Linux promises explicitly.** A Windows-first Tauri choice can be reasonable even with Linux WebKitGTK issues. Do not turn Linux failures into engine architecture requirements; keep a headless CLI/build route and publish only the desktop combinations actually tested. Historical Tauri/JUCE evidence justifies qualification, not abandoning Linux without documenting limits. [107,116]
6. **I — Prefer small public contracts and deterministic tests over novel abstraction frameworks.** Unknown maintainer counts mean the survey cannot prove any stack is solo-maintainable. The maintainer can control API size, dependency count and reproducible fixtures. Source prototypes offer patterns, not a maintenance guarantee. [2–5,88–103]
7. **I — Migrate numerical semantics, not function names.** XT already documents reference comparisons at -120 dBFS and SIMD cross-checks. Impulcifer needs its own staged tolerances, including timing/phase and optimizer response, not a borrowed universal threshold. SciPy's 1e-8 optimizer stopping tolerances are not coefficient-error guarantees. [3,5,13,118]
8. **I — Treat persistence, workflow and export compatibility as release gates.** Audacity's missing workflows and EasyEffects' migration fixes show that a successful launch is insufficient. Preserve settings/defaults, nine-language keys, job cancellation, calibration provenance, channel order and file formats before retiring the old implementation. [2,5,26,37,65]

## 3 Proposed architecture

This is a **precedent-informed proposal (I)**, not a completed library feasibility study. No code or CI configuration was changed.

### Recommended boundary

- **Headless numerical engine + CLI** owns float64 DSP, file formats, channel ordering and per-speaker parallelism. No GUI framework imports in this component.
- **Audio acquisition module** exposes explicit device/host-API descriptions and independent playback/capture sessions. Keep long-buffer blocking behavior; recovering alignment afterward is part of the existing contract. PortAudio is a reasonable first portability candidate, not a guarantee that every ASIO pair or 16-channel format is supported. [1,117]
- **Job manager** preserves request validation, sequence-cursor events, progress/logs and cooperative cancellation. The UI gets job summaries and file references, not serialized sample arrays. [2]
- **Desktop adapter** supplies dialogs, shell-open, runtime appearance/language settings, updates and permissions. Reuse HTML/CSS/JS through a replacement for `window.pywebview.api`. [4]
- **Plot/report module** is separate from both capture and shell: deterministic numeric summaries/golden data are tested independently of PNG/HTML rendering. Native measurement tools demonstrate plotting is feasible; none proves a drop-in replacement for every current Bokeh layout.
- **FFmpeg/ffprobe** remain explicitly versioned external helpers under the given requirement; no precedent warrants folding codec acquisition into the GUI.
- **Python remains an oracle during transition**. Optional bindings or a wheel for a native core are a separate packaging decision; no surveyed shell establishes automatic PyPI compatibility.
- **No forced CTk adapter migration.** ADR 0001 remains intact; the new architecture applies to the new engine/web shell, not a retrospective consolidation of existing native GUI code. [6]

### Stack decision informed by these precedents

| Option | Recommendation (I) | Evidence limits |
|---|---|---|
| **S1 Rust + Tauri 2** | Provisional first choice **if UI reuse and Windows/macOS delivery dominate**. Keep Rust engine independently runnable; evaluate a narrow C ABI for missing numerical libraries rather than demanding pure Rust. | Musicat/CamillaDSP establish native Rust audio patterns, not SciPy-equivalent FIR/minimum-phase/spline/TRF algorithms or 16-channel capture. Linux webview qualification remains. |
| **S2 C++ + Electron** | Credible fallback when controlled browser behavior matters more than package size. Put expensive/crash-prone work in an engine process or carefully scoped native worker, not the UI/main event loop. | Sounder proves addon implementation but not mature shipping. Electron's utility process is explicitly intended for CPU-heavy/crash-prone work; actual addon compatibility still needs testing. [111] |
| **S3 Go + Wails** | Do not select because of 'one binary/no node_modules'. Select only if Go orchestration provides a measured maintenance advantage after native DSP/audio dependencies are included. | SoundBoard is Windows-only/stereo and uses native APM/RNNoise/miniaudio. Wails' cgo-free Windows-shell statement does not apply to its audio stack. [94–95,113] |
| **Unlisted: C++ + Qt/Qt Quick** | Strongest conservative choice for an entirely native measurement interface, especially with the maintainer's XT experience. | Requires a substantial frontend rewrite and licensing/module decisions. OSM demonstrates feasibility, not effortless replacement of current JS. [9–13,45–48,115] |
| **Unlisted: C++ + JUCE 8 webview** | Worth a focused Windows prototype if native audio integration and retaining web UI are the priorities. | Licensing and Linux embedding need deliberate acceptance; existing examples do not establish the full offline scientific/reporting stack. [102–103,114,116] |
| **Unlisted: C# + Avalonia + native core** | Legitimate managed-shell alternative with stronger numerical-product deployment evidence than a demo. | Does not naturally preserve vanilla HTML/CSS; requires .NET plus native-library packaging. OpenUtau is synthesis rather than acquisition. [96–100] |
| **Unlisted: Java** | REW makes it impossible to dismiss Java as unsuitable for measurement. | REW alone does not establish available equivalents for all algorithms, contemporary GUI choice, web UI reuse or project-specific deployment effort. [14–16] |
| **Unlisted: retain Python, replace packaging selectively** | Keep as the control option until a native numerical spike passes. Avoid paying for a full rewrite solely because some build failures are Python-specific. | Current project's packaging pain is real, but ASH/friture show ongoing Python audio use; they do not demonstrate a universal fix for Nuitka. [21,41–44] |

### Numerical-parity acceptance strategy (I)

1. Freeze a verified Python revision and environment; save **float64 intermediate arrays plus metadata** before output quantization. Include all five current integrity scenarios and hostile synthetic fixtures. [5]
2. Assert discrete invariants exactly: sample rate, channel mapping, lengths/cropping, sweep segment order, finite values, configuration defaults, cancellation result and output-file schema.
3. For transforms/filtering/resampling, compare absolute and relative sample error with a silence floor; measure delay/group delay, sign/polarity and complex response. Set stage-specific limits from a prototype, not an arbitrary blanket epsilon.
4. For biquad fitting, compare residual objective, achieved EQ response, stability/poles and active bounds. Allow different parameterizations with equivalent response. Log initialization, evaluation budget and stopping reason. Do not interpret `ftol=1e-8` as eight-digit coefficient agreement. [3,118]
5. For BRIRs, compare frequency magnitude **and phase/ITD/ILD**, energy/decay, peak timing and pre-ringing. A magnitude-only pass can miss an audible spatial regression.
6. Preserve final channel-layout/header tests for 32-bit float WAV and all exports. Test report data separately from pixel-perfect screenshots; rendering libraries/platform fonts can legitimately differ.
7. Keep strict same-build/reference hashes where deterministic, but **do not require old NumPy/SciPy SHA-256 equality across a different implementation**. Cross-language numerical agreement is the migration gate; within-new-engine hashes remain useful regression checks.
8. Adopt a documented intentional-change procedure for differences beyond tolerance. XT's -120 dBFS comparisons are a relevant precedent, not an acoustically or mathematically universal limit. [13]

## 4 Risks ranked

| Rank | Risk | Evidence / confidence | Mitigation and decision trigger (I) |
|---:|---|---|---|
| **1** | Replacing scientific semantics with superficially similar DSP APIs | **V** local optimizer has nontrivial initialization/bounds/log parameterization; none of the shell examples validates all required primitives. [3,118] | Prototype least-squares fitting, minimum phase, spline/resampling and one complete BRIR before committing to the language. Reject a stack if parity/debugging effort is unacceptable. |
| **2** | Unsupported Windows device/host-API/channel topology | **V** current independent streams; REW device-format/rate fixes; PortAudio ASIO device limitation. [1,15,117] | Test real 16-output hardware at 44.1/48/96 kHz plus separate stereo capture across required APIs. A player example is not a pass. |
| **3** | Big-bang rewrite drops workflows/defaults or corrupts migration | **V** Audacity and EasyEffects documented compatibility work; local tests already cover five scenarios. [5,26,37] | Keep Python side-by-side until data/settings/export and job-contract comparisons pass. Make deliberate omissions explicit rather than shipping them accidentally. |
| **4** | Solo maintainer acquires more toolchains than expected | **V** XT provisioning drift, RingRTC heavy builds, Wails native DSP dependencies, Avalonia native libraries. [11,93–99] | Budget Rust/C++, Go/C++, or .NET/C++ as real mixed stacks. Pin versions and test complete packaged apps on native CI runners. |
| **5** | Webview behavior differs by OS/GPU | **V historical** Tauri Linux failures; JUCE X11 requirement. [107,116] | Windows packaged-app smoke tests first; macOS second. Limit Linux GUI support to tested distributions/backends; preserve CLI. |
| **6** | Invalid comparison of package size or memory | **V** exact engine-only/full-installer/bootstrap distinctions; **U** equivalent memory benchmark. [12,48,73,110] | Compare the same frontend/data/codec/update payload with installed and running memory measurements. Do not select Go/Tauri solely from marketing binary sizes. |
| **7** | Licensing/signing/update work discovered after implementation | **V** Qt/JUCE obligations and unsigned distribution caveats; XT already uses Velopack. [9,41,82,114–115] | Decide license compatibility and updater ownership before release; test upgrade/rollback, signatures and external-runtime installation. This survey does not certify each shell's complete updater flow. |
| **8** | Treating new repositories as proven sustainable ecosystems | **V** Dawesome early-stage, Sounder release unverified, nih-plug-webview experimental; all active maintainer counts **U**. [88,90,105] | Weight official APIs, stable releases and packaged tests above screenshots/stars. Require a small reproducible vertical slice that future agents can verify. |

## 5 Open questions you could not settle

- **U:** Exact active-maintainer counts for all surveyed projects. Public author names, owners, contributor lists and commercial staff counts do not answer this.
- **U:** Current implementation languages/toolkits for Dirac Live, SoundID Reference, Smaart, ARTA, Acourate and MathAudio. HOLMImpulse's historical developer statement does not prove its final/current implementation.
- **U:** Direct current product-wide language/GUI statement for REAPER; C++/SWELL remains a well-supported inference, not verified source disclosure in this survey.
- **U:** Equivalent Tauri/Electron/Wails/Avalonia installer, installed-footprint and RAM measurements. Several release API fetches were blocked; no benchmark was run. Do not use missing data as evidence of lower overhead.
- **U:** Windows readiness of Musicat; Intel build correctness and packaged release maturity of Sounder; release qualification of Nota/Dawesome/Sound-Field. Development manifests and README targets do not establish successful distribution.
- **U:** A mature released Electron+C++ native-addon **measurement** application matching Impulcifer. Sounder is implementation evidence; Signal is mature calling/native integration, not measurement or a pure-C++-addon topology.
- **U:** A Wails measurement/offline-scientific application meeting all R1–R9. SoundBoard demonstrates native DSP and vanilla JS, but stereo Windows duplex is a narrower contract.
- **U:** Current Helvum upstream version because GitLab was blocked. The 0.4.99 Cargo crate is explicitly obsolete; a 0.6.2 Debian source package is not proof of the newest upstream release.
- **U:** Primary explanation for Meadowlark's historical GUI choices. Current Yarrow does not substantiate an invented story that Tauri was abandoned for performance or reliability.
- **U:** Present-day reproducibility of historical WebKitGTK/NVIDIA reports with current Tauri, target distributions and drivers; the report is a test lead, not a current failure verdict.
- **U:** Full contemporary Wails v3 cross-compilation behavior. Windows shell cgo independence is verified; complete native-audio cross-building was not tested.
- **U:** Whether any candidate's numerical library set reproduces the required SciPy algorithms with acceptable effort. This dimension surveys precedents; optimizer/kernel selection needs separate primary-source and experimental validation.
- **U:** Real hardware confirmation of separate input/output streams and up to 16 float32 output channels at every requested sample rate/API. Source capability must not be confused with endpoint capability.
- **U:** Complete installer/updater/notarization workflows for each proposed stack, or continued PyPI distribution of a rewritten core. These need a packaging proof of concept rather than a precedent analogy.

## 6 Sources

All URLs were consulted on **2026-09-07**, directly or by a read-only research subagent. Local file URLs identify inspected source; moving-branch links are observation-time evidence, not immutable release claims. Restricted or failed retrievals are called out above. Numbered entries may be cited as ranges where a row needs several primary sources.

1. [Local recorder implementation](file:///E:/Impulcifer/core/recorder.py)
2. [Local application service](file:///E:/Impulcifer/application/impulcifer_service.py)
3. [Local bounded biquad optimizer](file:///E:/Impulcifer/autoeq/frequency_response.py)
4. [Local web frontend bridge and polling](file:///E:/Impulcifer/webview_ui/app.js)
5. [Local BRIR integrity scenarios](file:///E:/Impulcifer/tests/test_brir_integrity.py)
6. [ADR 0001: native frontends](file:///E:/Impulcifer/docs/adr/0001-native-frontends-stay-native.md)
7. [Local AutoEQ biquad coefficients](file:///E:/Impulcifer/autoeq/biquad.py)
8. [Local Equalizer APO/XT semantics](file:///E:/Impulcifer/core/eqapo.py)
9. [EqualizerAPO-XT README](https://github.com/115dkk/EqualizerAPO-XT)
10. [XT SIMD/dependency manifest](https://github.com/115dkk/EqualizerAPO-XT/blob/main/.github/simd-variants.psd1)
11. [XT provisioning and documented build failures](https://github.com/115dkk/EqualizerAPO-XT/blob/main/.github/scripts/Provisioning.psm1)
12. [XT latest release API, observed v2.51.0](https://api.github.com/repos/115dkk/EqualizerAPO-XT/releases/latest)
13. [XT build matrix and regression comparisons](https://github.com/115dkk/EqualizerAPO-XT/blob/main/docs/SimdBuildMatrix.md)
14. [REW downloads](https://www.roomeqwizard.com/)
15. [REW beta history](https://www.roomeqwizard.com/beta.html)
16. [REW Java documentation](https://www.roomeqwizard.com/betahelp/help/html/welcome.html)
17. [Equalizer APO project metadata](https://sourceforge.net/projects/equalizerapo/)
18. [Equalizer APO 1.4.2 files/notes](https://sourceforge.net/projects/equalizerapo/files/1.4.2/)
19. [HeSuVi project metadata/download](https://sourceforge.net/projects/hesuvi/)
20. [HeSuVi help](https://sourceforge.net/p/hesuvi/wiki/Help/)
21. [ASH-Toolset repository](https://github.com/ShanonPearce/ASH-Toolset)
22. [ASH-Toolset distribution files](https://sourceforge.net/projects/ash-toolset/files/)
23. [Original Impulcifer repository](https://github.com/jaakkopasanen/Impulcifer)
24. [Original Impulcifer tags](https://github.com/jaakkopasanen/Impulcifer/tags)
25. [Original Tkinter GUI source](https://raw.githubusercontent.com/jaakkopasanen/Impulcifer/master/gui.py)
26. [Audacity 4.0.0 official release notes](https://github.com/audacity/audacity/releases/tag/Audacity-4.0.0)
27. [Audacity official changelog](https://www.audacityteam.org/changelog/)
28. [Tenacity releases](https://codeberg.org/tenacityteam/tenacity/releases)
29. [Tenacity build guide mirror](https://github.com/tenacityteam/tenacity/blob/main/BUILDING.md)
30. [Ardour development architecture](https://ardour.org/development)
31. [Ardour homepage/version](https://ardour.org/)
32. [REAPER downloads](https://www.reaper.fm/download.php)
33. [Cockos WDL/SWELL](https://www.cockos.com/wdl/)
34. [REAPER change history](https://www.reaper.fm/whatsnew.txt)
35. [Mixxx downloads](https://mixxx.org/download/)
36. [Mixxx 2.5 Qt6/C++20 migration](https://mixxx.org/news/2024-12-24-mixxx-2_5-released/)
37. [EasyEffects current changelog](https://raw.githubusercontent.com/wwmm/easyeffects/master/src/contents/docs/community/CHANGELOG.md)
38. [EasyEffects build configuration](https://raw.githubusercontent.com/wwmm/easyeffects/master/CMakeLists.txt)
39. [Sonic Visualiser downloads](https://sonicvisualiser.org/download.html)
40. [Sonic Visualiser release history](https://www.sonicvisualiser.org/news/index.html)
41. [friture download/signing guidance](https://friture.org/download.html)
42. [friture releases](https://github.com/tlecomte/friture/releases)
43. [friture v0.54 dependencies](https://raw.githubusercontent.com/tlecomte/friture/v0.54/setup.py)
44. [friture development dependencies](https://raw.githubusercontent.com/tlecomte/friture/master/pyproject.toml)
45. [Open Sound Meter repository](https://github.com/psmokotnin/osm)
46. [Open Sound Meter QML entry point](https://raw.githubusercontent.com/psmokotnin/osm/master/src/main.cpp)
47. [Open Sound Meter release history](https://opensoundmeter.com/releases)
48. [Open Sound Meter v1.5.2 exact asset metadata](https://api.github.com/repos/psmokotnin/osm/releases/tags/v1.5.2)
49. [Smaart Suite release notes](https://support.rationalacoustics.com/support/solutions/articles/150000069197-smaart-suite-release-notes)
50. [Smaart requirements](https://www.rationalacoustics.com/pages/smaart-v9-minimum-system-requirements)
51. [ARTA downloads](https://www.artalabs.hr/download.htm)
52. [ARTA author news/retirement](https://artalabs.hr/news.htm)
53. [HOLMImpulse developer C++/CLR discussion, historical; search-index evidence](https://www.diyaudio.com/community/threads/holmimpulse-measuring-frequency-impulse-response.144984/page-14)
54. [HOLMImpulse original product URL; retrieval failed](https://www.holmacoustics.com/holmimpulse.php)
55. [Dirac Live 3.14.3 changelog](https://helpdesk.dirac.com/en/dirac-live/Dirac-Live-3143-Software-Changelog)
56. [Dirac software changelog index](https://helpdesk.dirac.com/en/dirac-live/Software-Changelogs-e8c5)
57. [SoundID Reference release notes](https://www.sonarworks.com/legal/soundid-reference/release-notes)
58. [SoundID requirements](https://support.sonarworks.com/hc/en-us/articles/4409355630738-System-and-hardware-requirements-for-SoundID-Reference)
59. [SoundID third-party microphone restrictions](https://support.sonarworks.com/hc/en-us/articles/360020099259-Using-a-third-party-measurement-microphone)
60. [Acourate developer V4 discussion](https://www.aktives-hoeren.de/viewtopic.php?p=254512)
61. [Acourate vendor Windows product flyer](https://pureacouratesound.com/wp-content/uploads/2024/03/Flyer-AudioVero-EN.pdf)
62. [DRC-FIR manual](https://drc-fir.sourceforge.net/doc/drc.html)
63. [DRC-FIR project/download metadata](https://sourceforge.net/projects/drc-fir/)
64. [MathAudio downloads](https://mathaudio.com/download.htm)
65. [MathAudio Room EQ guide](https://mathaudio.com/room-eq.htm)
66. [MathAudio foobar component release history](https://www.foobar2000.org/components/view/foo_room_eq/releases)
67. [ITA-Toolbox download requirements](https://www.ita-toolbox.org/download.php)
68. [ITA-Toolbox HRTF/measurement publication](https://www.ita-toolbox.org/publications/ITA-Toolbox_paper2017.pdf)
69. [Mesh2HRTF simulation workflow](https://github.com/Any2HRTF/Mesh2HRTF)
70. [Mesh2HRTF latest release API, observed 1.3.0](https://api.github.com/repos/Any2HRTF/Mesh2HRTF/releases/latest)
71. [CamillaDSP engine documentation](https://github.com/HEnquist/camilladsp)
72. [CamillaDSP 4.1.3 release](https://github.com/HEnquist/camilladsp/releases/tag/v4.1.3)
73. [CamillaDSP latest asset metadata, observed v4.1.3](https://api.github.com/repos/HEnquist/camilladsp/releases/latest)
74. [CamillaGUI repository](https://github.com/HEnquist/camillagui)
75. [CamillaGUI installation/backend](https://www.camilladsp.com/docs/camillagui/installation/)
76. [BruteFIR current author page](https://torger.se/anders/brutefir.html)
77. [Helvum deprecated crate and build requirements](https://docs.rs/crate/helvum/latest)
78. [Helvum Debian 0.6.2 source dependencies; secondary packaging evidence](https://sources.debian.org/src/helvum/0.6.2%2Bds-2/debian/control)
79. [qpwgraph repository](https://github.com/rncbc/qpwgraph)
80. [pw-viz repository](https://github.com/Ax9D/pw-viz)
81. [zestbay repository](https://github.com/lemonxah/zestbay)
82. [Musicat repository](https://github.com/basharovV/musicat)
83. [Musicat development manifest](https://raw.githubusercontent.com/basharovV/musicat/main/src-tauri/Cargo.toml)
84. [Musicat native audio output](https://raw.githubusercontent.com/basharovV/musicat/main/src-tauri/src/output.rs)
85. [Musicat native EQ](https://raw.githubusercontent.com/basharovV/musicat/main/src-tauri/src/equalizer.rs)
86. [Musicat release listing](https://github.com/basharovV/musicat/releases)
87. [Musicat v0.17.2](https://github.com/basharovV/musicat/releases/tag/v0.17.2)
88. [Dawesome repository](https://github.com/nbennett320/dawesome)
89. [Dawesome manifest](https://raw.githubusercontent.com/nbennett320/dawesome/main/engine/Cargo.toml)
90. [Sounder repository](https://github.com/murry1998/sounder)
91. [Sounder native CMake target](https://raw.githubusercontent.com/murry1998/sounder/main/native/CMakeLists.txt)
92. [Sounder application manifest](https://raw.githubusercontent.com/murry1998/sounder/main/package.json)
93. [Signal RingRTC native Electron build](https://github.com/signalapp/ringrtc/blob/main/BUILDING.md)
94. [Wails SoundBoard repository/native DSP documentation](https://github.com/jodagreyhame/SoundBoard)
95. [SoundBoard Go/native dependencies](https://raw.githubusercontent.com/jodagreyhame/SoundBoard/main/go.mod)
96. [OpenUtau repository](https://github.com/openutau/OpenUtau)
97. [OpenUtau development Avalonia project](https://raw.githubusercontent.com/openutau/OpenUtau/master/OpenUtau/OpenUtau.csproj)
98. [OpenUtau WORLDLINE C++ implementation](https://raw.githubusercontent.com/openutau/OpenUtau/master/cpp/worldline/worldline.cpp)
99. [OpenUtau native resampler adapter](https://raw.githubusercontent.com/openutau/OpenUtau/master/OpenUtau.Core/Classic/WorldlineResampler.cs)
100. [OpenUtau releases](https://github.com/openutau/OpenUtau/releases)
101. [Nota Avalonia/native-engine project](https://github.com/nota-daw/nota)
102. [JUCE WebView UI overview, 2024-04-08](https://juce.com/blog/juce-8-feature-overview-webview-uis/)
103. [JUCE 8.0.3 official WebViewPluginDemo](https://github.com/juce-framework/JUCE/blob/8.0.3/examples/Plugins/WebViewPluginDemo.h)
104. [Meadowlark Yarrow documentation](https://meadowlark.app/yarrow-book/)
105. [Experimental Wry/baseview NIH-plug editor](https://github.com/httnn/nih-plug-webview)
106. [Sound-Field JUCE web UI project](https://github.com/mbarzach/Sound-Field)
107. [Tauri #9394 historical NVIDIA/AppImage/Wayland issues](https://github.com/tauri-apps/tauri/issues/9394)
108. [Electron native module guidance](https://www.electronjs.org/docs/latest/tutorial/using-native-node-modules)
109. [Node-API ABI guarantees and exclusions](https://nodejs.org/api/n-api.html)
110. [Electron performance/measurement guidance](https://www.electronjs.org/docs/latest/tutorial/performance)
111. [Electron process model and utility workers](https://www.electronjs.org/docs/latest/tutorial/process-model)
112. [Wails v3 cross-platform guide; direct fetch blocked](https://v3.wails.io/guides/build/cross-platform/)
113. [Wails official introduction source, Windows cgo statement](https://raw.githubusercontent.com/wailsapp/wails/master/website/docs/introduction.mdx)
114. [JUCE 8.0.3 tagged license](https://raw.githubusercontent.com/juce-framework/JUCE/8.0.3/LICENSE.md)
115. [Qt official LGPL obligations](https://www.qt.io/licensing/open-source-lgpl-obligations)
116. [JUCE 8.0.3 Linux WebBrowser implementation](https://raw.githubusercontent.com/juce-framework/JUCE/8.0.3/modules/juce_gui_extra/native/juce_WebBrowserComponent_linux.cpp)
117. [PortAudio host APIs, blocking streams and ASIO device restriction](https://www.portaudio.com/docs/v19-doxydocs/api_overview.html)
118. [SciPy least_squares algorithm, bounds and stopping criteria](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html)
