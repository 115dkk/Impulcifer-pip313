# Impulcifer rewrite research: audio I/O feasibility

Research cutoff and observation date: **2026-09-07**. Scope: R1 and its implications for packaging, cancellation, channel routing and verification. No project files were changed. No audio hardware was exercised; no candidate was built. Shell-based metadata queries were unavailable, so release/source verification used WebSearch/WebFetch and public package/GitHub metadata. The report path uses the Windows temporary directory shown in this session's tool paths; direct environment-variable resolution was unavailable.

**Evidence labels:** **[V]** verified in cited documentation, source, release metadata, or identified local source; **[I]** engineering inference/recommendation, not demonstrated hardware behavior; **[U]** unknown or not verified. In tables explicitly headed “verified inventory,” factual cells inherit [V]; limitations marked [U] remain unknown. GitHub bugs are verified *reports*, not independently reproduced defects. Dates and issue counts are observation snapshots, not promises about later versions. Source numbers link to the numbered URL list in §6.

## 1 Verdict

1. **[I] Keep PortAudio as the audio boundary, regardless of the rewrite language.** It matches the existing host-API model and supplies blocking I/O; changing DSP language does not require replacing the audio backend [4–7].
2. **[I] C++ has the lowest integration risk; Rust, Go and .NET are viable with an owned, small PortAudio adapter.** Existing bindings are not equally complete or safe [20–23, 34, 39–42].
3. **[V] CPAL 0.18.2 is not an exact R1 replacement:** no Windows MME/DirectSound, shared-mode WASAPI, no public speaker-mask control [16–19].
4. **[V] SDL3 3.4.16 and SDL3-CS fail 12/14/16-channel playback; Oto fails capture and more-than-stereo playback** [31–33, 37–38, 48].
5. **[I] ASIO should be an optional supported backend, not dismissed as unnecessary low-latency machinery.** Some large pro interfaces expose only eight channels per WDM device [12].
6. **[V] PortAudio ASIO cannot open two independent ASIO streams, even on one device.** Use one duplex ASIO stream or validate ASIO playback with capture through another host [7].
7. **[V] WASAPI has no universal eight-channel format-description limit, but endpoint and driver support decides what actually works** [8–10].
8. **[I] Do not promise discrete Atmos-speaker measurement through ordinary consumer HDMI merely by requesting 7.1.4/7.1.6 PCM** [8–13].
9. **[V] ASIO SDK 2.3.4 is dated 2025-10-15 and offers GPLv3/proprietary alternatives; this MIT project must choose distribution terms deliberately** [14–15, 59–61].
10. **[I] Gate a rewrite on physical channel-identification and concurrent-recording tests before choosing a GUI shell.** Library marketing and channel-count fields are insufficient evidence.

## 2 Findings (with sources)

### 2.1 Actual behavior to preserve, and behavior not to copy

**[V, local source]** Read `E:/Impulcifer/core/recorder.py` completely, `application/impulcifer_service.py:300–350`, `core/constants.py:123`, `core/impulse_response_estimator.py:18–23`, `LICENSE`, and `docs/adr/0001-native-frontends-stay-native.md`. Public repository counterparts are [1–2]; local observations refer to the checkout actually read, not an assumption that a moving web branch is identical.

| Local source | Verified behavior and consequence |
|---|---|
| `application/impulcifer_service.py:308–347` | Enumerates host APIs and devices; optional exact host-name filter; returns index, name, host API, max input/output channels, default device indices. It does **not** verify each channel/rate/sample-format combination. |
| `core/recorder.py:193–251` | Strips `Windows ` from host names, accepts a host-API suffix in a device-name query, otherwise appends the explicit host argument, otherwise tries **DirectSound → MME → WASAPI**. Delegates actual matching to sounddevice. |
| `core/recorder.py:254–277` | Output minimum channels is the playback buffer width. Input lookup uses default `min_channels=1`, even though recording normally requests **2**. The rewrite should check two input channels rather than reproduce this omission [I]. |
| `core/recorder.py:238–249` | Host-unspecified fallback only tries three Windows API names. With no matching Windows host, the function cannot resolve a device this way. Do not reproduce this Windows-only fallback on macOS/Linux [I]. Explicit host suffixes are a separate branch. |
| `core/recorder.py:280–295` | Stores selected devices as global sounddevice defaults using `name + host API`. Device-list index is not a durable physical-device identifier. |
| `core/recorder.py:117–185, 440–495` | Starts `record_target()` in a thread; it calls `sd.rec(..., blocking=True)`. The calling thread invokes `sd.play(transposed_buffer, samplerate=fs, blocking=True)`, then joins the recording thread on the normal path. Progress uses a separate wall-clock thread. “Main thread” here means the calling playback thread; it need not be the GUI thread. |
| `core/recorder.py:347–358, 387–393` | In-memory/file data is channel-major before transposition. Mono broadcast to both headphone channels is opt-in, not valid for ordinary single-speaker excitation. |
| `core/recorder.py:367–385` | Explicitly rejects TrueHD Atmos-object masters when FFmpeg only decodes their bed. Changing audio libraries will not restore discarded height objects. |
| `core/constants.py:123`; `core/impulse_response_estimator.py:18–23` | Named layouts are mono, stereo, 5.1, 7.1, 7.1.4, 7.1.6. Last two allocate **12 and 14 tracks**, not 16. Keep the brief's separate 16-channel capacity target and test all three counts. |
| `core/impulse_response_estimator.py:21–22` | 7.1.4 order is `FL FR FC LFE BL BR SL SR TFL TFR TBL TBR`; 7.1.6 inserts `TSL TSR` before `TBL TBR`. These orders must not be replaced by a library's generic surround order. |

**Important mismatch between intent and upstream API contract. [V]** sounddevice 0.5.6 documents that both `play()` and `rec()` stop any earlier invocation of its convenience functions [3]. The inspected source documentation uses shared `_last_callback` lifecycle state. Thus the current code *intends* two simultaneous independent streams, but those convenience calls are not independent stream owners. **[I]** Depending on scheduling, playback can terminate capture, or concurrent starts can race. This is a source/API-contract finding, not a hardware reproduction or a claim about every installed sounddevice version. Explicit `InputStream`/`OutputStream` or a deliberate duplex session is the correct rewrite contract. Do not define parity as reproducing the global convenience-function interaction.

**[I] Additional lifecycle corrections in the rewrite:** capture readiness must be acknowledged before playback; capture-thread exceptions must reach the job result; playback failure/cancellation must stop and join capture; do not truncate an unsupported channel count silently. Preserve the blocking *session* contract, not the incidental global-default implementation.

### 2.2 Best and second-best choices by language

These are **[I] recommendations** based on the verified inventory below. Risk rates cover R1 integration, not the much larger DSP rewrite. Even “low” requires hardware acceptance tests. “Owned adapter” means maintain a narrow, audited interface to a pinned native backend; it is **not** a claim that an off-the-shelf binding already meets every requirement.

#### Best choice

| Language | Best library / integration | Risk | One-line reason |
|---|---|---|---|
| **C++** | **PortAudio C API**, explicit stream owners and WASAPI/ASIO extensions | **Low** | Exact host grouping and blocking read/write; native headers expose the controls wrappers often omit. |
| **Rust** | **PortAudio through an owned thin C ABI/FFI adapter**; audit/fix any reused `portaudio` wrapper | **Medium** | Preserves all Windows hosts without accepting the old safe wrapper's missing mask API and unresolved lifetime reports. |
| **Go** | **gordonklaus/portaudio**, pinned commit plus small owned host-specific extension | **Medium** | Good blocking I/O and host model; cgo, native packaging and missing WASAPI mask/ASIO selector fields remain. |
| **C#/.NET** | **PortAudio P/Invoke**, reusing PortAudioSharp2 only after completing required bindings | **Medium** | Backend fits; PortAudioSharp2 alone omits host enumeration calls and blocking read/write, despite comments suggesting otherwise. |
| **Dart/Flutter** | **PortAudio C adapter through `dart:ffi`**, worker isolates or native workers | **High** | Feasible backend but no verified turnkey Dart package for this contract; native audio execution must not depend on asynchronous Dart callbacks. |
| **Kotlin/JVM** | **PortAudio JNI**, using `philburk/portaudio-java` as a reviewed starting point | **High** | Host enumeration and float blocking streams exist, but JNI binaries, mask extensions and aged binding integration become maintainer work. |

#### Second-best fallback

| Language | Second-best library / integration | Risk | Explicit compromise |
|---|---|---|---|
| C++ | **Current miniaudio C device API** | Medium | Retains WinMM/MME, DirectSound and WASAPI with simpler native distribution; no built-in ASIO and callback/drain adapter required. RtAudio is preferable instead if ASIO matters more than retaining MME. |
| Rust | **Current miniaudio C via an owned adapter**, not archived ExPixel `miniaudio` as-is | Medium–high | Preserves legacy Windows host enumeration but requires native bindings and has no ASIO. **CPAL is only a conditional alternative if MME/DirectSound and exclusive/mask control are dropped.** |
| Go | **malgo** | Medium | Good miniaudio/cgo binding and multichannel configuration; no ASIO, custom finite-buffer callback lifecycle. |
| C#/.NET | **MiniAudioExNET AdvancedAPI** | Medium | Current release supports capture and multichannel configuration; no ASIO and some detailed channel-map/native-build exposure remains unverified. |
| Dart/Flutter | **miniaudio C adapter through FFI** | High | Native callbacks/ring buffers and packaging are owned work; no ASIO. No verified pure-Dart substitute. |
| Kotlin/JVM | **Directly maintained minimal PortAudio JNI adapter** instead of the existing Java wrapper | High | Same proven backend, but own the small interop surface and masks. No second turnkey JVM library was verified; Java Sound is **not** a qualifying fallback. |

**[I] Implication for S1/S2/S3:** Rust/Tauri can keep PortAudio; C++/Electron naturally uses it; Go/Wails can call it via cgo. None needs to send sample buffers through a webview or Chromium. Audio I/O alone is not a reason to adopt Electron, nor a reason to reject Rust. Go's audio distribution is not “pure Go, one dependency-free binary” when the best-fit backend is PortAudio.

### 2.3 Verified native/backend capability inventory

**[V]** API capability below is not a guarantee that every device supports 16 channels at all three rates. Backend inclusion is build-dependent. `WinMM` corresponds to the MME family. An ALSA plugin reaching PulseAudio/JACK is not equivalent to separately enumerating those host APIs.

| Library | Windows hosts | macOS / Linux | Devices and channels | Buffer model / independent streams |
|---|---|---|---|---|
| **PortAudio** | MME, DirectSound, WASAPI, optional ASIO, WDM-KS | CoreAudio / ALSA, JACK, current-source PulseAudio, others | Host API count/info; max input/output channels; float32; per-format probing; host extensions [4–7] | Blocking `ReadStream`/`WriteStream` or callbacks; input-only and output-only. **ASIO singleton exception** below. Stop drains playback. |
| **RtAudio** | WASAPI, ASIO, DirectSound; **no MME** | CoreAudio/JACK / ALSA, PulseAudio, JACK, OSS | Compiled-API list, per-API instance, device IDs, float32, channels/rates, `firstChannel` [25–26] | Callback only; one stream per instance. Separate instances can request separate streams; driver restrictions remain. Callback result 1 drains/stops, 2 aborts. |
| **miniaudio** | WASAPI, DirectSound, WinMM; **no ASIO** | CoreAudio / ALSA, PulseAudio, JACK, others | Backend-selected context; capture/playback device lists; f32, channel counts and maps; current C maximum 254 [27–28] | Callback device API; separate capture/output devices or duplex. Application must implement finite-buffer completion. Do not stop/start a device from its audio callback. |
| **JUCE AudioDeviceManager** | WASAPI, ASIO, DirectSound; **no MME** | CoreAudio / ALSA, JACK; **no separate PulseAudio backend** | Device types, input/output names, float channel buffers, `BigInteger` active-channel sets [29–30] | Callbacks; setup supports input/output names with a common sample rate/buffer size. Not a `play(buffer, blocking)` API. Backend-dependent pairing; not arbitrary dual independent clock domains by contract. |
| **SDL3 3.4.16** | WASAPI, DirectSound; no WinMM/ASIO in inspected backend list | CoreAudio / PipeWire, PulseAudio, ALSA, JACK, others | Driver list then current-driver device list; float input/output; **audio conversion accepts 1–8 channels only** [31–33] | Queued/callback audio streams and independent capture/playback, but **fails 12/14/16 channels**. |

**[V] Release versus development-source caveat:** PortAudio's latest formal release is still v19.7.0; the backend list in the current repository is not proof that every backend exists in every v19.7.0 binary. **[I]** Pin and build the selected revision, enumerate its compiled hosts in CI smoke tests, and do not infer PulseAudio/ASIO inclusion from a generic package name [4, 62].

### 2.4 Rust details that change the recommendation

| Candidate | Verified usable functionality | Verified gaps / qualification |
|---|---|---|
| **CPAL 0.18.2** | Host/device abstraction; separate input/output f32 callbacks; `u16` channel counts and requested rates; Windows WASAPI plus optional ASIO/JACK [16–19] | No MME/DirectSound. WASAPI opens shared mode and assigns `KSAUDIO_SPEAKER_DIRECTOUT`; public `StreamConfig` has no mask or exclusive switch. Supported-output configuration uses mix-format channels and rate-conversion capability, not proof of native format. Build a completion/drain protocol yourself. |
| **RustAudio `portaudio` 0.8.0** | Host API/device queries; maxima; f32 input-only/output-only/duplex; blocking or callbacks; format checks [20–21] | `hostApiSpecificStreamInfo` always null in safe parameter conversion: **no direct WASAPI mask or ASIO selector plumbing**. Maintenance-mode README. Lifetime/UAF reports mean “safe Rust” is not enough assurance. Windows native automatic-build fallback is unsupported in inspected `portaudio-sys2`; supply a native library/link configuration. |
| **`mvdnes/portaudio-rs` 0.3.2** | A genuinely separate binding; f32 and blocking/callback stream choices [22] | Also passes null host-specific info. Last crate publication in 2020. Windows CMake/static build exists, but current-toolchain success and its exact bundled backend revision were not verified [U]. Not an improvement over owning a small binding. |
| **ExPixel `miniaudio` 0.10.0** | Actual wrapper exposes Wasapi/DSound/WinMM, separate playback/capture configuration, F32, share mode and 32-element channel-map arrays [23] | Archived in 2023; old bundled native dependency and bindgen/Clang burden. No ASIO; no verified raw `dwChannelMask` control [U]. Do not credit it with fixes/features from C miniaudio 0.11.25. |
| **`wasapi` 0.24.0** | Direct Windows control: shared/exclusive, event/polling, `WaveFormat::new(..., channels, Option<u32> mask)`, format probes, capture/render clients [24] | Windows WASAPI only, no legacy host groups. **[I]** Best Windows-specific escape hatch if mask/exclusive control is needed; own worker loops, draining and other OS backends. |
| **`coreaudio-rs` 0.14.2** | AudioUnit capture/render callbacks, stream-format and rate configuration; current manifest uses `objc2-*`, not old coreaudio-sys/bindgen instructions [24] | Apple-only. Exact 16-channel wrapper setup and two-device lifecycle were not fully verified [U]. CoreAudio's general capability is not a substitute for that check. |
| **`alsa` 0.12.1** | `PCM::new`, `io_f32`, blocking/nonblocking/poll; channel/rate/format tests; channel-map query/get/set [24] | Linux ALSA only; requires native `libasound` development/runtime dependency. Separate PCM objects are an implementation route [I], not hardware proof. |

**[I] Direct-native Rust alternative:** `wasapi` + `coreaudio-rs`/objc2 + `alsa` provides substantial format control, but introduces three device, lifecycle, permission, error and hotplug implementations. It also does not satisfy MME/DirectSound or explicit PulseAudio/JACK enumeration without more adapters. This is disproportionate for a solo maintainer unless PortAudio fails a demonstrated requirement.

### 2.5 Go details

| Candidate | Verified fit | Build / omissions |
|---|---|---|
| **gordonklaus/portaudio** | `HostApis`, `Devices`, float32 buffers, explicit input/output stream parameters, format probing; `[]float32` interleaved and `[][]float32` noninterleaved; registered buffers with blocking `Read`/`Write` [34] | cgo + `pkg-config: portaudio-2.0` + headers/library. Device parameter wrapper lacks WASAPI mask and ASIO channel selectors. Backend availability follows native build. Pin a commit, not an imaginary stable release. |
| **malgo** | Backend-selected contexts; capture/playback device lists and native-format information; `FormatF32`, counts/rates/maps, separate devices; callbacks expose byte buffers [35–36] | cgo compiles bundled miniaudio, without a separately installed PortAudio. No built-in ASIO. `Start`/`Stop` being synchronous does **not** mean blocking sample read/write exists. Callback completion/drain, no-allocation discipline and Go/C buffer lifetime need tests. |
| **Oto 3.5.0** | Convenient `io.Reader`-driven output [37–38] | **Only mono/stereo, no recording, no exposed host/device enumeration matching R1. Reject.** Current README says no cgo on Windows/macOS/Linux/BSD; this improvement does not cure its functional mismatch. |

### 2.6 .NET details, including 2026 changes

| Candidate | Verified fit | Reason not to treat as turnkey |
|---|---|---|
| **PortAudioSharp2 1.0.6** | Nullable input/output stream parameters; f32 multichannel configuration; device info includes host index and maxima; desktop native runtime packages [39–42] | Constructor actually **requires a callback**, despite null-callback documentation. Open #24 requests missing `Pa_ReadStream`/`Pa_WriteStream`. Inspected public API lacks host count/info functions. WASAPI mask/ASIO extension structures and compiled DLL backends are unverified [U]. Add P/Invokes or use an owned adapter. |
| **MiniAudioExNET 3.3.6** (`JAJ.Packages.MiniAudioEx`) | Current AdvancedAPI exposes backend arrays/context/device enumeration; WinMM/DSound/WASAPI and desktop backends; max-channel constant 254; f32; actual `AudioRecorder` capture callbacks [43–44] | **Not stereo-output-only.** Use AdvancedAPI, not only high-level source playback. No ASIO. Exact managed channel-map fields/raw WASAPI mask, and backend configuration of every runtime binary, remain unverified [U]. |
| **NAudio 3.0.1** | Windows WASAPI/WinMM/ASIO/DirectSound; **NAudio 3 adds Linux ALSA playback and capture**; Linux output passes channel count with `forceStereo:false`; format fallback exists [45–47] | **Not wholly Windows-only anymore, but no macOS CoreAudio backend.** NAudio.Core portability is not device-I/O portability. Linux ALSA does not supply separate PulseAudio/JACK host group APIs. Target .NET 9+. Not a sole cross-platform R1 solution. |
| **SDL3-CS 3.4.16**, `edwardgushchin/SDL3-CS` | Native desktop packages, float capture/playback and SDL stream APIs [48] | Inherits SDL's **1–8 channel limit** [32]. No amount of C# binding work unlocks supported 16-channel SDL audio. Reject. |

**[I] NAudio is credible for a Windows-only recorder, especially if direct Windows controls become more important than one cross-platform backend.** That is a different product decision from keeping macOS second priority. It needs a separate Mac backend and does not remove format/routing acceptance tests.

### 2.7 Dart/Flutter and JVM

**[V] Dart/Flutter:** `dart:ffi` is the official C interop route; it does not eliminate native build/distribution. `NativeCallable.listener` accepts arbitrary native threads but dispatches asynchronously and returns void [49–50]. **[I]** Do not let a PortAudio/miniaudio output callback return before Dart fills the requested native buffer: the timing and lifetime contract is wrong. Use blocking PortAudio from dedicated non-UI isolates, or keep callbacks and ring buffers in native code. Post progress/completion to Dart asynchronously.

**[U] Pure Dart:** no maintained, pure-Dart solution satisfying all of host selection, stereo capture, 16-channel float output and exact channel routing was verified. `minisound 3.0.1` is miniaudio-based and supports recording, but API-group enumeration and 16-channel/channel-map behavior were not verified; Apple support is marked experimental [51]. `moduslabs/dart-mod-player` is an archived playback example, not a general recorder backend [52].

**[V] JVM:** `philburk/portaudio-java` exposes host API enumeration, devices, format checks, nullable input/output parameters and `BlockingStream.read(float[], frames)` / `write(float[], frames)` [53–55]. It has no callback API; it can naturally put capture/playback on separate JVM workers. It exposes `channelCount`, not a fixed stereo buffer. The wrapper omits host-specific stream info and a PulseAudio host-type constant. **[I]** Extend mask/host handling where needed and ship JNI/native PortAudio artifacts for each architecture.

**[V] `javax.sound.sampled`:** `SourceDataLine.write`, `TargetDataLine.read`, and output `drain()` give useful blocking semantics; `PCM_FLOAT` and arbitrary channel counts are representable in `AudioFormat` [56]. But standard enumeration is mixer/provider-based, not PortAudio host APIs. The inspected OpenJDK Windows DirectSound provider enumerates channels `{1,2}` and sample widths `{8,16}` [57]. **[I] Reject standard Java Sound as the backend for this contract.** A custom provider might change capabilities, but then that provider is the dependency requiring evaluation. Kotlin syntax does not change this limitation.

### 2.8 Maintenance snapshot: versions, issue counts, licenses and build burden

**[V] Verified inventory, 2026-09-07.** “Issues” are open non-PR issues shown by repository UI or filtered issue responses, not `open_issues_count` blindly copied with PRs included. Counts are not a quality score. “Unknown” means this investigation did not establish the number/date. Primary repository/package links below also identify the license and issue tracker. Native backend licensing still applies in addition to wrapper licensing.

| Library | Latest verified release/publication | Open issues | License / build implications | Sources |
|---|---|---:|---|---|
| PortAudio | **19.7.0, 2021-04-06**; default-branch commit 2026-09-04 | 353 | MIT; C compiler/linking, deliberate backend configuration | [4–7,62] |
| RtAudio | **6.0.1, 2023-08-01**; default-branch commit 2026-08-18 | 42 | Permissive MIT-style; C++/OS backend toolchains | [25–26,63] |
| miniaudio C | **0.11.25, 2026-03-03 UTC**; changelog uses Mar 4 | 5 | Public domain or MIT-0; single-header C compilation, native platform dependencies | [27–28,64] |
| JUCE | **9.0.1, 2026-08-10** | 283 | AGPLv3 or commercial JUCE 9 terms; C++17/CMake 3.22+ framework | [29–30,65] |
| SDL C | **3.4.16, 2026-09-02** | 699 | zlib; native SDL packaging; does not meet channel requirement | [31–33,66] |
| CPAL | **0.18.2, 2026-08-16**; asio-sys 0.4.0 same date | **Unknown** | Apache-2.0 wrapper; ASIO adds C++ compiler/LLVM-bindgen/SDK; Linux ALSA native dependency | [16–19] |
| RustAudio `portaudio` | **0.8.0, 2024-10-13** | **Unknown** | MIT; declared maintenance mode; separate Windows PortAudio setup | [20–21,67] |
| `mvdnes/portaudio-rs` | **0.3.2, 2020-05-14** | 0 | MIT; Windows CMake/static native build; current success unverified | [22,67] |
| ExPixel Rust `miniaudio` | **0.10.0, 2020-07-23**; archived **2023-01-06** | 5 | MIT wrapper; old ep-miniaudio-sys native build, Clang/bindgen | [23,67] |
| Rust `wasapi` | **0.24.0, 2026-08-12** | 2 | MIT; Windows APIs through Rust windows crates; no separate audio C library | [24,67] |
| Rust `coreaudio-rs` | **0.14.2, 2026-04-29** | 18 | MIT/Apache-2.0; Apple frameworks/objc2 | [24,67] |
| Rust `alsa` | **0.12.1, 2026-07-31** | 0 | Apache-2.0/MIT; libasound headers/pkg-config/runtime | [24,67] |
| Go PortAudio | **No tags**; commit **2026-02-03**, `765aa7dfa631…` | 7 | MIT; cgo + pkg-config + separately supplied PortAudio | [34,68] |
| malgo | **v0.11.26 tag**, tag commit **2026-08-18**; no latest GitHub release object | 7 | Unlicense wrapper, native miniaudio terms; cgo, bundled C | [35–36,69] |
| Oto | **3.5.0, 2026-09-05** | 19 | Apache-2.0; current desktop implementation avoids cgo; Linux ALSA fallback needs libasound.so.2 | [37–38,70] |
| PortAudioSharp2 | **1.0.6, 2025-10-16** | 6 | Apache-2.0 wrapper + native PortAudio license; runtime packages, missing interop | [39–42] |
| MiniAudioExNET | **3.3.6, 2026-08-04** | 0 | Unlicense or MIT-0; native runtime binaries/backends must be audited | [43–44] |
| NAudio | Stable **3.0.1, 2026-08-18**; 3.1.0-preview.2 Sep 5; 2.x maintenance 2.4.0 Aug 26 | 9 | MIT; .NET 9+ for v3; Windows interop/Linux ALSA, no Mac I/O | [45–47] |
| SDL3-CS | **3.4.16, 2026-09-03** | 0 | zlib; native SDL NuGets; inherits eight-channel ceiling | [48] |
| Dart minisound | **3.0.1**, exact publication date **unknown** | **Unknown** | MIT package, native miniaudio; Apple experimental; R1 unverified | [51] |
| Dart FFI / Flutter | Official SDK APIs; exact current SDK release **not investigated** | N/A to a single audio library | SDK + chosen C backend; own platform assets/build; no claimed complete Dart audio package | [49–50] |
| `philburk/portaudio-java` | Source **0.1.0**; **no GitHub Releases**; latest release date N/A | 4 | MIT-style PortAudio license; native PortAudio + CMake/make JNI + Gradle/JAR packaging | [53–55] |
| Java Sound | Java SE **25 API docs** inspected; latest JDK maintenance build **not investigated** | No single audio issue count verified | JDK-provided API/provider; implementation/distribution license depends on chosen JDK (OpenJDK provider source inspected) | [56–57] |

**[I] Maintenance judgments:** PortAudio and RtAudio's old release dates do not mean abandonment: both have verified 2026 repository activity. ExPixel's Rust miniaudio archive is a materially different situation. An issue count of zero in a tiny binding is not evidence that its device handling is better than a large native library's.

#### Notable open reports to turn into regression tests

All entries here are **[V: report exists/open at observation]**, with **[U: not reproduced here]**. Do not interpret the title as a confirmed defect in every platform/version.

| Project | Relevant open report | Why it matters |
|---|---|---|
| PortAudio | [macOS shutdown deadlock #1174](https://github.com/PortAudio/portaudio/issues/1174); [Antelope ASIO initialization crash #1148](https://github.com/PortAudio/portaudio/issues/1148) | Stop/cancel and vendor-driver acceptance tests. |
| CPAL | [COM/STA enumerator lifetime access violation #1302](https://github.com/RustAudio/cpal/issues/1302); [device rerouting #1339](https://github.com/RustAudio/cpal/issues/1339) | Webview enumeration thread must not leave later audio operations holding invalid COM state. 0.18.2 fixes separate-handle ASIO duplex silence; do not use older behavior as current capability [17]. |
| RustAudio PortAudio | [stream/DeviceInfo lifetimes, including ASAN UAF report #196](https://github.com/RustAudio/rust-portaudio/issues/196); [callback buffer lifetime #186](https://github.com/RustAudio/rust-portaudio/issues/186); [shutdown stall #165](https://github.com/RustAudio/rust-portaudio/issues/165) | Audit wrapper ownership and buffer lifetime before relying on a safe API. [#197](https://github.com/RustAudio/rust-portaudio/issues/197) is a static-analysis write-length report, not independently confirmed here. |
| miniaudio | [WASAPI notification deadlock #1149](https://github.com/mackron/miniaudio/issues/1149); [Realtek exclusive capture discontinuity #1152](https://github.com/mackron/miniaudio/issues/1152) | Device-change callbacks, exclusive stereo input, shutdown. |
| RtAudio | [S24_3LE support #477](https://github.com/thestk/rtaudio/issues/477) | Not itself a float32 rejection; test negotiated native conversion. |
| JUCE | [WASAPI format-comparison UB #1700](https://github.com/juce-framework/JUCE/issues/1700) | Format probing, including unsupported-rate cases. |
| Go PortAudio | [device disconnect #64](https://github.com/gordonklaus/portaudio/issues/64); [Windows executable portability #60](https://github.com/gordonklaus/portaudio/issues/60) | Missing native dependencies and device loss. |
| malgo | [capture-device selection #56](https://github.com/gen2brain/malgo/issues/56); [example nil pointer #60](https://github.com/gen2brain/malgo/issues/60) | Verify selected input is actually used; do not ship an example unreviewed. |
| PortAudioSharp2 | [missing blocking I/O #24](https://github.com/csukuangfj/PortAudioSharp2/issues/24); [Linux lists zero devices #19](https://github.com/csukuangfj/PortAudioSharp2/issues/19) | Binding completeness and runtime backend smoke tests; cause of #19 not established here. |
| NAudio | [WASAPI thread/RCW lifetime #970](https://github.com/naudio/NAudio/issues/970); [Stop/join blocking #1248](https://github.com/naudio/NAudio/issues/1248) | Progress must not report completion before stop/drain succeeds. |
| JNI PortAudio | [Linux loading #2](https://github.com/philburk/portaudio-java/issues/2); [macOS latency #3](https://github.com/philburk/portaudio-java/issues/3); [long-running Windows error #12](https://github.com/philburk/portaudio-java/issues/12) | Native-loader coverage and repeat-session tests. |
| Other Rust native wrappers | [wasapi 24-bit fallback #62](https://github.com/HEnquist/wasapi-rs/issues/62), [exclusive probes #63](https://github.com/HEnquist/wasapi-rs/issues/63); [coreaudio macOS runner #112](https://github.com/RustAudio/coreaudio-rs/issues/112); [archived miniaudio ring buffer #6](https://github.com/ExPixel/miniaudio-rs/issues/6) | Do not assume f32 impact from 24-bit reports; nevertheless test probing and buffer ownership. |
| Oto | [playing false before physical completion #237](https://github.com/ebitengine/oto/issues/237) | Illustrates queued-data versus audible-completion distinction; Oto already fails functional requirements. |

**[U]** No separate SDL audio bug, minisound issue inventory, or current Java Sound bug inventory was verified. SDL's rejection rests on source-level channel validation, not an issue search.

### 2.9 ASIO: requirements, licensing and the independent-stream exception

#### What enabling ASIO actually entails

| Library family | Verified ASIO route / omission |
|---|---|
| PortAudio and all bindings | Compile native PortAudio with ASIO and supply a suitable SDK. Inspected current CMake defaults: `PA_USE_ASIO=OFF`, `PA_USE_WASAPI=ON`, `PA_USE_DS=ON`, `PA_USE_WMME=ON`; SDK archive variable `ASIO_SDK_ZIP_PATH` [58]. A binding enum does not add ASIO to its DLL. |
| CPAL 0.18.2 | `asio = ["dep:asio-sys", "dep:num-traits"]`, asio-sys 0.4.0. Native C++ + LLVM/Clang/bindgen. `CPAL_ASIO_DIR` selects SDK; absent SDK can trigger download from an unversioned Steinberg endpoint [18–19]. No Cargo feature automatically resolves license obligations. |
| RtAudio | Compile ASIO backend with SDK/toolchain; ASIO available, MME absent [25–26]. |
| JUCE | ASIO device backend exists [29–30]. Pin JUCE build options/SDK source and separately satisfy JUCE AGPL/commercial and SDK licensing. Exact JUCE 9 SDK provisioning was not established here [U]. |
| miniaudio / malgo / MiniAudioExNET / archived Rust wrapper | **No built-in ASIO.** Closed miniaudio #133 moved to discussion #263, not to an implemented backend [28]. A custom backend is new work, not an existing feature. |
| NAudio | Windows ASIO support exists [45–47]. Exact SDK-derived code/license requirements for the chosen version/build were not independently audited [U]; do not presume CPAL's build steps apply verbatim. |
| SDL3 / SDL3-CS / Oto / standard Java Sound | No verified qualifying ASIO route for this contract. |

**[V] Licensing changed in 2025.** Steinberg's official page offers **GPLv3 or proprietary licensing**, and its indexed FAQ explicitly associates the GPL route with source-code disclosure [14–15]. Microsoft vcpkg's port identifies the official archive as **`ASIO-SDK_2.3.4_2025-10-15.zip`** [59]. SDK and wrapper licenses are distinct.

**[V] Distribution obligations:** GPLv3 permits source/binary conveyance subject to its notices, combined-work and Corresponding Source requirements; it is not permission to ship the combined ASIO-enabled binary as MIT-only [61]. The existing checkout is MIT (`LICENSE:1–24`). MIT source can be incorporated while retaining notices, but the combined covered distribution must comply with the selected terms. Proprietary licensing remains available [15]. A mirrored 2025 SDK license text says redistribution of SDK material requires advance agreement, and publishing under the proprietary route requires a Steinberg-signed agreement [60]. **[U]** The full current proprietary agreement was not retrievable from the official page; historical 2005/2011 texts are not a reliable substitute. Have the maintainer review the actual pinned SDK agreement before shipping; this report is not legal advice.

**[V]** ASIO name/logo trademark conditions remain separate from GPL licensing [14]. **[I]** Pin SDK version and checksum, record the selected license, preserve required notices, and avoid build-time downloads from moving URLs. An optional DLL or separate process is **not automatically** a GPL exception.

#### Is ASIO necessary?

- **[V]** Low-latency requirements do not decide channel exposure. RME USB.IO documentation specifies WDM multichannel devices of **up to eight channels each**, while the interface itself supports far larger channel totals [12]. “16 multichannel WDM devices” means a count of devices, not one 16-channel endpoint.
- **[I]** If a user's endpoint genuinely accepts all required discrete channels through WASAPI shared/exclusive, ASIO is unnecessary for that configuration. Prefer the backend with proven correct channel routing, not lowest latency.
- **[I]** For professional interfaces whose WDM endpoints omit or split required outputs, ASIO can be necessary to reach all outputs in **one stream**. Make it an optional tested backend rather than mandatory for ordinary stereo/5.1/7.1 users.
- **[I]** Do not recommend ASIO4ALL or combining multiple unrelated output devices as a universal substitute for a native 16-channel interface. That adds unverified driver/clock behavior.

#### Two streams versus duplex

**[V] PortAudio's ASIO implementation has one open-stream guard.** `openAsioDeviceIndex != paNoDevice` rejects a second ASIO stream with `paDeviceUnavailable`; the implementation also has a `theAsioStream` singleton [7]. This is stricter than merely saying “cannot open two different ASIO devices.” One same-device duplex stream is allowed; blocking ASIO read/write uses internal callback/ring-buffer synchronization.

**[I] Session policy:**

1. Default to separate explicit playback and capture streams where the host/driver permits them; start capture first and wait for readiness.
2. If both selected sides are on the same ASIO interface, use **one duplex native stream behind the same logical recording-session interface**. Channel counts may differ (16 output, 2 input).
3. If output is ASIO and input is another host/device (for example WASAPI microphone), try two explicit streams only after capability/open validation and hardware tests. Same-hardware WDM/ASIO multi-client compatibility is vendor-dependent [U].
4. If the brief interprets “two independent streams” as inviolable even for same-interface ASIO, **PortAudio ASIO cannot meet that combination**. Keep ASIO optional or amend that constraint; do not conceal it.

**[V]** Other callback libraries also lack a universal guarantee of arbitrary concurrent streams on a device. CPAL 0.18.2's ASIO duplex fix is not proof of concurrent unrelated ASIO-device support [17]. **[U]** Two independent ASIO streams through RtAudio/JUCE/NAudio/CPAL were not established as a portable guarantee. Do not replace PortAudio solely to assume that limitation disappears.

### 2.10 WASAPI above eight channels: counts, masks and real hardware

| Question | Evidence and conclusion |
|---|---|
| Does Windows have an absolute 8-channel WASAPI limit? | **[V] No such format-description limit:** WAVEFORMATEXTENSIBLE has a channel count and 18 defined speaker positions, including top-front/top-back [8]. **[U]** This says nothing about a particular endpoint accepting 12/14/16 channels. |
| Is `max_output_channels >= 16` sufficient? | **[V] No.** `GetMixFormat`, `IsFormatSupported` and `Initialize` concern full format/mode combinations. Shared mode may accept conversions; exclusive queries hardware format support [9–10]. **[I]** Check each channels/rate/sample-representation/mask/mode tuple, then verify physical routing. |
| Does exclusive mode unlock channels hidden by a driver? | **[V] It requires an explicitly supported hardware format** [9]. **[I]** It is useful when the shared mix configuration is restrictive, but cannot create a 16-channel WDM endpoint that the driver never supplies. |
| Does f32 application I/O imply f32 hardware? | **[V] No.** Shared mixing uses float internally and can end in integer hardware output; libraries may convert formats [9–10,27,47]. **[I]** R1 should require float32 application buffers, while recording actual negotiated native format/conversion. Do not demand IEEE float hardware if an interface only exposes integer PCM. |
| Are 7.1.4 and 7.1.6 ordinary Windows speaker masks? | **[V]** Existing 7.1.4 order maps to the conventional side-7.1 plus four top-front/back positions. **[V]** Classic WAVEFORMATEXTENSIBLE has **no top-side-left/top-side-right bits** corresponding to the project's `TSL/TSR` [8]. **[I]** A 7.1.6 project buffer therefore cannot acquire those semantics merely by assigning a stock 14-channel mask. Use explicit physical output mapping or a separately designed spatial renderer. |
| Can zero mask solve it? | **[V]** `KSAUDIO_SPEAKER_DIRECTOUT=0` denotes unnamed port-to-port channels [8]. **[I]** It is a candidate for a pro raw-output endpoint, not proof the driver accepts it. Historical DirectSound direct-out documentation explicitly targets XP/earlier and warns about processing; do not cite it as a modern WASAPI success guarantee [71]. |
| Consumer HDMI/AVR with height speakers | **[I]** Expect ordinary PCM endpoints often to expose 2/6/8 rather than 12/14/16; query the actual driver instead of promising height outputs. Vendor HDMI examples document 2–8 channels, but the cited NVIDIA document is **Jetson**, not proof of a universal modern Windows-PC/HDMI limit [13]. Receiver marketing such as “7.1.6 Atmos” is not evidence of a 14-channel PCM endpoint. |
| Windows Spatial Audio | **[V]** A distinct object-rendering API exists, with static/dynamic object capability queries [11]. **[I]** It is a separate feature project, not something PortAudio/CPAL gains by increasing `channels`. Spatial rendering may not preserve isolated physical speaker excitation. |
| Pro interface | **[V]** It may expose many ASIO channels but only stereo/eight-channel WDM groups [12]. **[I]** Native ASIO and manual logical-to-physical mapping are the credible fallback for 12/14/16 discrete outputs. |

#### Which library exposes channel semantics adequately?

| Library/binding | Count and channel-control evidence | Assessment |
|---|---|---|
| PortAudio C | Counts/maxima, format probe; `PaWasapiStreamInfo.channelMask` with `paWinWasapiUseChannelMask`; exclusive and explicit-format flags; ASIO selectors in extension API [5–7] | **[I] Best reviewed common backend for an owned adapter.** Must expose the extensions, not just generic parameters. |
| CPAL 0.18.2 | Count field; shared WASAPI; mask fixed to DIRECTOUT; no public mask [18–19] | **[I] Insufficient explicit routing control for claiming every layout.** Physical route tests still possible on known raw endpoints. |
| RustAudio PortAudio / mvdnes wrapper / Go PortAudio / Java JNI wrapper | Counts available; host-specific pointer absent/null in inspected wrapper parameter conversion [20–22,34,55] | **[I] Extend or bypass wrapper.** C capability is not automatically accessible. |
| PortAudioSharp2 | Device maxima/host index; detailed WASAPI extension exposure unverified [39–42] | **[I] Add reviewed P/Invoke structures/functions; do not assume bundled native options.** |
| Direct Rust `wasapi` | Explicit optional u32 channel mask; shared/exclusive format tests [24] | **[I] Best Windows-only direct-control route.** Still no automatic hardware guarantee. |
| miniaudio C / malgo / archived Rust wrapper | Channel maps and counts exposed [23,27,35–36] | **[U] Generic map-to-specific-WASAPI-mask behavior for every 12/14/16 case not established.** Conversion/downmix must be disabled or detected; verify native format and physical outputs. |
| MiniAudioExNET | Advanced config/counts/capture verified; exact channel-map field coverage not fully verified [43–44] | **[U] Requires a focused interop spike before acceptance.** |
| RtAudio | Counts, contiguous `firstChannel` region, no public arbitrary speaker mask [26] | **[I] Suitable for ordered physical ASIO channels; less explicit WASAPI-layout control.** |
| JUCE | Named physical input/output channels and active-channel bitsets [29–30] | **[V] Bitsets are not WAV speaker masks. [I] Test backend routing rather than infer layout from bit positions.** |
| NAudio | Windows-specific audio APIs and WaveFormat infrastructure exist [45–47] | **[U] Exact >8-channel mask configuration for every selected v3 backend was not verified.** Not rejected for a proved Windows channel ceiling; rejected as sole cross-platform backend. |
| SDL / SDL3-CS | Channel remapping exists but converter requires 1–8 [32–33] | **[V] Fails target count, regardless of mapping.** |
| Java Sound / Oto | Format type can describe more than implementation supports / Oto explicitly mono-stereo [37,56–57] | **[V] Do not meet this contract.** |

## 3 Proposed architecture

Everything in this section is **[I] a proposed design**, informed by the verified APIs above. It does not propose modifying the current CTk frontend or routing it through the JSON service; ADR 0001 remains respected.

### 3.1 One recorder contract, owned native streams

Keep the audio adapter independent of the GUI, DSP array library and IPC transport. Reuse it from the headless CLI and chosen new desktop frontend. A narrow interface should cover:

| Operation / type | Required data and behavior |
|---|---|
| `enumerate_hosts()` | Compiled and available host IDs/names, backend revision, capability flags. Never fabricate unavailable host rows. |
| `enumerate_devices(host)` | Stable backend/device identity where available, display name, host name, direction, channel maxima, defaults. Host-local indices are ephemeral. |
| `resolve_device(query, direction, min_channels)` | Accept existing name-plus-host syntax for migration; preserve DirectSound/MME/WASAPI ordering as an explicit compatibility policy, not the universal platform default. Reject ambiguity with candidate choices. |
| `probe(config)` | Requested and accepted sample rate, channels, sample type, mode, native format, speaker mask or ordered raw channels, conversion policy, output-to-physical mapping. Enforce two input channels. |
| `record_session(config, playback_f32, cancel)` | Blocking from the job worker's perspective; explicit input/output owners; readiness acknowledgement, sample counters, completion/drain, errors and cleanup. No global device defaults. |
| Session strategy | Separate input/output streams normally; same-device ASIO duplex exception. The frontend sees one job in either case. |
| Result | Captured data plus requested/actual configuration, first/last timestamps, recorded frames, xrun/discontinuity counters, device identifiers, channel mapping, cancellation/error state. |

For Rust/Go/.NET/Dart/JVM, an owned C ABI can expose **plain structs and opaque session handles** instead of arbitrary PortAudio internals. Keep WASAPI/ASIO structures inside native code where practical; add compile-time size/ABI checks if direct interop structures are exposed. This limits unsafe/FFI review to a small surface. Copy device names into owned memory before returning them; do not leak borrowed native pointers beyond PortAudio lifetime.

### 3.2 Playback/capture lifecycle

1. Allocate buffers before starting; DSP uses float64 separately, audio adapter uses float32 with one explicit tested conversion.
2. Open/validate both sides. Start capture and wait for a positive readiness acknowledgement; thread creation alone is not readiness.
3. Play on the calling **job worker**, preserving a blocking session API without blocking the desktop UI.
4. With PortAudio blocking I/O, write bounded chunks, not one multi-minute call, so cancellation can be checked. A successful final write means data was submitted; call the backend's drain/stop operation before success [5].
5. With callbacks, copy from a precomputed buffer, zero-fill the final partial callback, and atomically signal “source exhausted.” Track backend drain separately; callback completion is not necessarily audible completion. Keep logging, disk writes, locks and allocation off the callback.
6. Capture enough pre/post-roll to tolerate independent-start latency; preserve the sweep detector's role. Document any intentional change from exactly N captured frames, rather than call it byte parity.
7. On cancellation, abort rather than drain when appropriate; stop both sides, join workers and report cancellation. On error, preserve the original failure and finish cleanup. No success event until all native I/O has stopped and recorded-file writing has completed.
8. Use sample counters for playback progress; retain wall-clock information as a separate estimate. Never run a UI callback from the audio callback.

Independent device clocks are not synchronized merely because both requests say 48 kHz. A sweep start offset can be corrected by detection, but a time-varying offset/sample-rate drift is a different problem. Measure end-to-end drift over the longest supported session before deciding whether correction is necessary; prefer a shared interface/clock where practical [4].

### 3.3 Minimal native packaging policy

- Pin the native backend revision, wrapper revision and enabled host list; build on each target OS/architecture. An old formal PortAudio release plus selected fixes may be safer than unpinned main, but choose based on tests, not age alone.
- Produce a backend manifest with version, compiled hosts, compiler/toolchain and ASIO SDK/license choice. No silent SDK downloads from a moving URL.
- Test an installed artifact, not only development imports: enumerate hosts, open a dummy/mock adapter, load the actual native library, exercise teardown. Hardware-free GitHub runners cannot certify physical 16-channel output.
- Keep ffmpeg external as specified; decoding and native audio transport are separate concerns.
- A native adapter still requires signing/packaging/load-path work. It is smaller than shipping Python/scipy solely for audio I/O, but “compiled language” does not remove native dependency drift.

### 3.4 Acceptance tests and realistic numerical parity

| Gate | What passes | What it does not prove |
|---|---|---|
| Mock/deterministic unit tests | Name/suffix matching, host ordering, minimum channels, ambiguous devices, channel permutation, f64→f32 conversion, exact N-frame finite buffers, odd callback sizes, cancellation and cleanup. | Driver behavior. |
| In-memory adapter goldens | Exact channel-major/interleaved conversion and lossless f32 buffer transfer; WAV metadata/sample count; no implicit channel duplication/downmix. | Physical signal equality. |
| Windows device matrix | MME/DirectSound/WASAPI ordinary device; WASAPI shared/exclusive; a pro ASIO interface; separate 2-channel microphone; 44.1/48/96 kHz supported tuples; explicit rejection of unsupported tuples. | All consumer/pro drivers. |
| Physical channel-identification test | Sequential low-level tagged pulse/sweep on **every channel of 8, 12, 14 and 16 output configurations**; verify actual port/speaker, unused/LFE channel handling, no hidden downmix/upmix. | General Atmos-object rendering. |
| Long-session capture | Longest sweep set, input/output xruns, start offset and accumulated drift, cancellation mid-sweep, unplug, reopen, repeated runs, failure during final drain. | Acoustic bit identity between runs. |
| macOS/Linux | Device selection, stereo capture, available multichannel interface, permission denial, hotplug, stop/drain; native artifact load tests on each target. | Full Linux host parity without all backends installed. |
| Rewrite DSP parity boundary | Feed **the identical stored f64/f32 recordings**, not fresh microphone captures, to old/new DSP. Compare per-stage goldens using declared absolute/relative and domain metrics; retain exact tests for pure layout/copy operations. | numpy/scipy byte-identical final DSP results in a new language. |

Do **not** compare live recordings by SHA-256: timing, ADC noise and independent clocks make that the wrong oracle. Retain current SHA checks within the unchanged Python pipeline, and use fixed input recordings for rewrite DSP comparisons. For R1 specifically, bit-exact sample ordering/conversion on mock streams is realistic; physical hardware tests require time alignment, channel identity, amplitude/error bounds, sample-count and discontinuity checks. Numerical thresholds must be chosen from measured baseline variability and task sensitivity, not invented as a universal `1e-6` promise.

## 4 Risks ranked

All rankings and mitigations are **[I]**; evidence references distinguish their factual basis.

| Rank | Risk / severity | Consequence | Required response |
|---:|---|---|---|
| **1** | **Physical 12/14/16-channel access and routing (critical)** | Successfully opened stream still downmixes/remaps; measurements assigned to wrong speakers. Consumer Atmos receiver capability confused with PCM channels. | Real channel-identification matrix; expose native format/mask/mapping; reject unsupported tuples [8–13]. |
| **2** | **ASIO versus mandatory two-stream topology (high)** | PortAudio rejects second ASIO stream; full feature promised but cannot run. | Document duplex exception or explicitly exclude that configuration [7]. |
| **3** | **Wrapper incompleteness / unsafe lifetime (high)** | Missing host/mask control or UAF despite safe-language surface. | Own narrow adapter; validate struct ABI, buffers, native lifetimes; review RustAudio reports and .NET/JNI omissions [20–24,39–42,55]. |
| **4** | **ASIO/JUCE distribution licensing (high)** | MIT-only distribution assumptions conflict with selected combined-work terms. | Choose and document SDK license; inspect actual agreement; do not assume dynamic loading avoids obligations [14–15,29,59–61]. |
| **5** | **Capture/playback startup, stop and drain (high)** | Capture interrupted, final sweep cut off, success before output drains, shutdown deadlock. | Explicit owners/readiness; chunked blocking I/O or native callback state machine; error propagation; regression tests [3,5,open reports]. |
| **6** | **Independent-clock drift (medium–high)** | Sweep alignment at the beginning is correct while later timing/deconvolution degrades. | Long-duration measurements, timestamps/frame counts, shared-clock recommendation; only add compensation if measured necessary. |
| **7** | **Native packaging and moving builds (medium)** | Missing DLL/host API/SDK, wrong architecture, macOS permissions, unsupported Linux backend. | Pin versions/build flags; artifact-level smoke tests; native manifest; hardware tests outside CI [19,21,34,42,58]. |
| **8** | **Choosing a playback-oriented library (medium, avoidable)** | Late discovery of stereo-only/output-only restrictions. | Reject Oto, SDL3/SDL3-CS and Java Sound for strict R1 now [32,37,57]. |
| **9** | **Overbuilding a custom per-OS engine (medium)** | Maintainer replaces packaging work with COM/CoreAudio/ALSA lifecycle work. | Keep PortAudio until a reproducible limitation justifies a native backend. |

## 5 Open questions you could not settle

1. **[U] Actual supported hardware:** Which Windows output interfaces/AVRs must support 12/14/16 physical channels, and what do their installed WDM/ASIO drivers expose at 44.1/48/96 kHz? No hardware test was performed.
2. **[U] Requirement precedence:** Is one same-interface ASIO duplex stream acceptable behind the recording-session API, or is native stream separation mandatory even there? Those demands conflict with PortAudio ASIO.
3. **[U] ASIO distribution selection:** Will the maintainer accept GPLv3 terms for the combined ASIO-enabled distribution, or obtain/use the proprietary agreement? Full current official proprietary agreement text was not retrieved.
4. **[U] Float32 meaning:** Is application f32 buffering sufficient (recommended), or is native IEEE float device format mandatory? Many interfaces can require integer transport while applications use float.
5. **[U] 7.1.6 physical mapping:** What exact devices should map `TSL/TSR`, which classic WAVEFORMATEXTENSIBLE lacks? A physical port map is needed; a conventional speaker mask is insufficient.
6. **[U] Binding/native artifacts:** Exact backend options in PortAudioSharp2 and MiniAudioExNET shipped binaries; exact generic-miniaudio-map→WASAPI-mask behavior for each target configuration; current mvdnes build success; full coreaudio-rs 16-channel setup.
7. **[U] Vendor multi-client behavior:** Whether same-hardware ASIO playback and WDM capture coexist, and whether separately selected devices remain stable for multi-minute sessions.
8. **[U] Maintenance metadata gaps:** Complete current CPAL/RustAudio PortAudio/minisound issue counts; exact minisound publication date; current Dart/Flutter/JDK maintenance release numbers. Marked unknown rather than fabricated. These gaps do not affect proved SDL/Oto/host-API exclusions.
9. **[U] Hardware-independent regressions:** Open issue reports were not reproduced. No assertion that choosing another library eliminates their classes of failure.
10. **[U] Existing convenience-call impact:** The current source conflicts with sounddevice's documented convenience-stream lifecycle. Reproduction with the maintainer's exact installed sounddevice/PortAudio build was outside this read-only research task.

## 6 Sources

All sources observed **2026-09-07**. Public source branches are mutable; version-pinned links are used where material. Search-index evidence is identified where full fetch failed. Inline issue links in §2.8 additionally identify every discussed bug report.

1. Existing recorder and service: [recorder.py](https://github.com/115dkk/Impulcifer/blob/master/core/recorder.py), [application service](https://github.com/115dkk/Impulcifer/blob/master/application/impulcifer_service.py). **Local checkout was the actual evidence; remote counterpart identity was not independently verified.**
2. Existing layout/license counterparts: [constants](https://github.com/115dkk/Impulcifer/blob/master/core/constants.py), [track orders](https://github.com/115dkk/Impulcifer/blob/master/core/impulse_response_estimator.py), [LICENSE](https://github.com/115dkk/Impulcifer/blob/master/LICENSE). Same local-evidence qualification as [1].
3. sounddevice convenience lifecycle and explicit streams: [convenience functions](https://python-sounddevice.readthedocs.io/en/latest/api/convenience-functions.html), [0.5.6 source documentation observed](https://python-sounddevice.readthedocs.io/en/latest/_modules/sounddevice.html), [streams](https://python-sounddevice.readthedocs.io/en/latest/api/streams.html).
4. PortAudio backend/device model: [repository](https://github.com/PortAudio/portaudio), [API overview](https://portaudio.com/docs/v19-doxydocs/api_overview.html).
5. PortAudio blocking, formats and drain: [portaudio.h documentation](https://portaudio.com/docs/v19-doxydocs/portaudio_8h.html).
6. PortAudio WASAPI controls: [extension documentation](https://portaudio.com/docs/v19-doxydocs/pa__win__wasapi_8h.html), [current header](https://raw.githubusercontent.com/PortAudio/portaudio/master/include/pa_win_wasapi.h).
7. PortAudio ASIO singleton and extensions: [implementation](https://raw.githubusercontent.com/PortAudio/portaudio/master/src/hostapi/asio/pa_asio.cpp), [ASIO header](https://github.com/PortAudio/portaudio/blob/master/include/pa_asio.h).
8. Microsoft channel-mask semantics and defined positions: [WAVEFORMATEXTENSIBLE](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ksmedia/ns-ksmedia-waveformatextensible), [Channel Mask](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/channel-mask).
9. Microsoft shared/exclusive and native format: [Device Formats](https://learn.microsoft.com/en-us/windows/win32/coreaudio/device-formats).
10. Microsoft format acceptance: [IAudioClient::IsFormatSupported](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-isformatsupported).
11. Microsoft separate spatial object API: [Render Spatial Sound Using Spatial Audio Objects](https://learn.microsoft.com/en-us/windows/win32/coreaudio/render-spatial-sound-using-spatial-audio-objects).
12. RME USB.IO manual, WDM Devices / chapter 7.2 and interface specifications: [official PDF](https://rme-audio.de/downloads/usb_io_e.pdf). WDM eight-channel and device-count statements verified through indexed official text; PDF pages 12–20 also inspected. Do not generalize this one product's limits to all RME devices.
13. NVIDIA HDMI/DP hardware example, specifically Jetson: [Jetson Linux R36.4.4 audio guide](https://docs.nvidia.com/jetson/archives/r36.4.4/DeveloperGuide/SD/Communications/AudioSetupAndDevelopment.html). Not a universal Windows-PC HDMI limit.
14. Steinberg ASIO open-source license/FAQ/trademark policy: [official page](https://www.steinberg.net/developers/asiosdk-open/). Relevant body obtained from search-index text; direct fetch returned only site navigation.
15. Steinberg proprietary option and dated change: [official SDK page](https://www.steinberg.net/developers/prorietary-sdk/), [2025-10-15 official announcement](https://ocl-steinberg-live.steinberg.net/_storage/asset/808575/storage/master/Press%20Release%20-%202025-10-15%20-%20OBS%20Partnership-%20EN.pdf).
16. CPAL 0.18.2 API: [docs.rs](https://docs.rs/cpal/0.18.2/cpal/), [release metadata](https://api.github.com/repos/RustAudio/cpal/releases?per_page=10).
17. CPAL 0.18.2 and ASIO fixes: [release](https://api.github.com/repos/RustAudio/cpal/releases/tags/v0.18.2), [asio-sys 0.4.0 release](https://api.github.com/repos/RustAudio/cpal/releases/tags/asio-sys-v0.4.0).
18. CPAL version-pinned host/config/features: [hosts](https://raw.githubusercontent.com/RustAudio/cpal/v0.18.2/src/platform/mod.rs), [public config](https://raw.githubusercontent.com/RustAudio/cpal/v0.18.2/src/lib.rs), [manifest](https://raw.githubusercontent.com/RustAudio/cpal/v0.18.2/Cargo.toml).
19. CPAL WASAPI and ASIO native build: [WASAPI device](https://raw.githubusercontent.com/RustAudio/cpal/v0.18.2/src/host/wasapi/device.rs), [asio-sys build](https://raw.githubusercontent.com/RustAudio/cpal/asio-sys-v0.4.0/asio-sys/build.rs).
20. RustAudio PortAudio: [repository/maintenance declaration](https://github.com/RustAudio/rust-portaudio), [0.8.0 stream implementation](https://docs.rs/crate/portaudio/0.8.0/source/src/stream.rs), [public API](https://raw.githubusercontent.com/RustAudio/rust-portaudio/master/src/lib.rs).
21. RustAudio native build/extensions: [portaudio-sys2 build](https://docs.rs/crate/portaudio-sys2/0.1.0/source/build.rs), [extensions](https://raw.githubusercontent.com/RustAudio/rust-portaudio/master/src/ext/mod.rs).
22. Separate mvdnes binding: [repository](https://github.com/mvdnes/portaudio-rs), [stream](https://raw.githubusercontent.com/mvdnes/portaudio-rs/master/src/stream.rs), [native build](https://raw.githubusercontent.com/mvdnes/portaudio-rs/master/portaudio-sys/build.rs).
23. Archived Rust miniaudio: [repository](https://github.com/ExPixel/miniaudio-rs), [Backend](https://docs.rs/miniaudio/0.10.0/miniaudio/enum.Backend.html), [Context](https://docs.rs/miniaudio/0.10.0/miniaudio/struct.Context.html), [playback config](https://docs.rs/miniaudio/0.10.0/miniaudio/struct.DeviceConfigPlayback.html), [capture config](https://docs.rs/miniaudio/0.10.0/miniaudio/struct.DeviceConfigCapture.html), [native manifest](https://docs.rs/crate/ep-miniaudio-sys/2.4.0/source/Cargo.toml).
24. Direct Rust native APIs: [wasapi WaveFormat](https://docs.rs/wasapi/0.24.0/wasapi/struct.WaveFormat.html), [AudioClient](https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioClient.html), [wasapi repo](https://github.com/HEnquist/wasapi-rs), [coreaudio AudioUnit](https://docs.rs/coreaudio-rs/0.14.2/coreaudio/audio_unit/struct.AudioUnit.html), [coreaudio manifest](https://docs.rs/crate/coreaudio-rs/0.14.2/source/Cargo.toml), [ALSA PCM](https://docs.rs/alsa/0.12.1/alsa/pcm/struct.PCM.html), [ALSA HwParams](https://docs.rs/alsa/0.12.1/alsa/pcm/struct.HwParams.html).
25. RtAudio: [repository](https://github.com/thestk/rtaudio), [API](https://caml.music.mcgill.ca/~gary/rtaudio/classRtAudio.html).
26. RtAudio channels/callback semantics: [header](https://raw.githubusercontent.com/thestk/rtaudio/master/RtAudio.h).
27. miniaudio architecture/backends/device API: [manual](https://miniaud.io/docs/manual/index.html), [repository](https://github.com/mackron/miniaudio).
28. miniaudio changes/ASIO non-implementation: [changes](https://github.com/mackron/miniaudio/blob/master/CHANGES.md), [ASIO issue #133](https://github.com/mackron/miniaudio/issues/133), [comments](https://api.github.com/repos/mackron/miniaudio/issues/133/comments?per_page=100), [discussion #263](https://github.com/mackron/miniaudio/discussions/263).
29. JUCE backend and framework terms: [device types](https://docs.juce.com/master/classjuce_1_1AudioIODeviceType.html), [repository](https://github.com/juce-framework/JUCE), [license](https://github.com/juce-framework/JUCE/blob/master/LICENSE.md).
30. JUCE I/O setup: [AudioDeviceManager](https://docs.juce.com/master/classAudioDeviceManager.html), [AudioDeviceSetup](https://docs.juce.com/master/structAudioDeviceManager_1_1AudioDeviceSetup.html).
31. SDL3 backend list: [3.4.16 audio source](https://raw.githubusercontent.com/libsdl-org/SDL/release-3.4.16/src/audio/SDL_audio.c), [repository](https://github.com/libsdl-org/SDL).
32. SDL3 hard channel validation: [3.4.16 SDL_audiocvt.c](https://raw.githubusercontent.com/libsdl-org/SDL/release-3.4.16/src/audio/SDL_audiocvt.c).
33. SDL channel remapping/audio APIs: [output channel map](https://wiki.libsdl.org/SDL3/SDL_SetAudioStreamOutputChannelMap), [audio category](https://wiki.libsdl.org/SDL3/CategoryAudio).
34. Go PortAudio binding: [repository](https://github.com/gordonklaus/portaudio), [portaudio.go](https://raw.githubusercontent.com/gordonklaus/portaudio/master/portaudio.go).
35. malgo backend/device API: [repository](https://github.com/gen2brain/malgo), [enum](https://raw.githubusercontent.com/gen2brain/malgo/master/enumerations.go), [context](https://raw.githubusercontent.com/gen2brain/malgo/master/context.go).
36. malgo configuration/callbacks: [device](https://raw.githubusercontent.com/gen2brain/malgo/master/device.go), [config](https://raw.githubusercontent.com/gen2brain/malgo/master/device_config.go), [native formats](https://raw.githubusercontent.com/gen2brain/malgo/master/device_info.go).
37. Oto current output-only/mono-stereo contract and build requirements: [repository](https://github.com/ebitengine/oto).
38. Oto playback completion report: [#237](https://github.com/ebitengine/oto/issues/237).
39. PortAudioSharp2 package/version/native assets: [repository](https://github.com/csukuangfj/PortAudioSharp2), [NuGet](https://www.nuget.org/packages/PortAudioSharp2).
40. PortAudioSharp2 callback requirement: [Stream.cs](https://raw.githubusercontent.com/csukuangfj/PortAudioSharp2/master/PortAudioSharp/Stream.cs).
41. PortAudioSharp2 exposed API: [PortAudioSharp.cs](https://github.com/csukuangfj/PortAudioSharp2/blob/master/PortAudioSharp/PortAudioSharp.cs), [missing blocking API #24](https://github.com/csukuangfj/PortAudioSharp2/issues/24).
42. PortAudioSharp2 native packaging: [run.sh](https://raw.githubusercontent.com/csukuangfj/PortAudioSharp2/master/scripts/run.sh).
43. MiniAudioExNET package: [repository](https://github.com/japajoe/MiniAudioExNET), [NuGet](https://www.nuget.org/packages/JAJ.Packages.MiniAudioEx), [license](https://raw.githubusercontent.com/japajoe/MiniAudioExNET/master/LICENSE), [runtime assets](https://github.com/japajoe/MiniAudioExNET/tree/master/runtimes).
44. MiniAudioExNET current real capture/config: [MaContext](https://raw.githubusercontent.com/japajoe/MiniAudioExNET/master/src/Core/AdvancedAPI/MaContext.cs), [MaDevice](https://raw.githubusercontent.com/japajoe/MiniAudioExNET/master/src/Core/AdvancedAPI/MaDevice.cs), [enums](https://raw.githubusercontent.com/japajoe/MiniAudioExNET/master/src/Native/MiniAudioNativeEnums.cs), [channel constant](https://raw.githubusercontent.com/japajoe/MiniAudioExNET/master/src/Native/MiniAudioNative.cs), [AudioRecorder](https://raw.githubusercontent.com/japajoe/MiniAudioExNET/master/src/Core/StandardAPI/AudioRecorder.cs).
45. NAudio 3 platform status: [repository](https://github.com/naudio/NAudio), [maintainer's Aug 15 announcement](https://www.markheath.net/post/2026/8/15/naudio-3-release).
46. NAudio versions: [NuGet](https://www.nuget.org/packages/NAudio).
47. NAudio Linux implementation: [ALSA README](https://github.com/naudio/NAudio/tree/main/src/NAudio.Alsa), [AlsaOut](https://raw.githubusercontent.com/naudio/NAudio/main/src/NAudio.Alsa/AlsaOut.cs), [AlsaIn](https://raw.githubusercontent.com/naudio/NAudio/main/src/NAudio.Alsa/AlsaIn.cs), [enumerator](https://raw.githubusercontent.com/naudio/NAudio/main/src/NAudio.Alsa/AlsaDeviceEnumerator.cs).
48. Exact SDL3-CS binding: [repository](https://github.com/edwardgushchin/SDL3-CS), [NuGet](https://www.nuget.org/packages/SDL3-CS).
49. Official Dart C interop: [dart:ffi guide](https://dart.dev/interop/c-interop).
50. Official asynchronous native callback contract: [NativeCallable.listener](https://api.dart.dev/dart-ffi/NativeCallable/NativeCallable.listener.html).
51. Dart minisound: [pub.dev package](https://pub.dev/packages/minisound).
52. Archived Dart playback example: [moduslabs/dart-mod-player](https://github.com/moduslabs/dart-mod-player).
53. PortAudio JNI project/build/license: [philburk/portaudio-java](https://github.com/philburk/portaudio-java), [license](https://raw.githubusercontent.com/philburk/portaudio-java/main/LICENSE.txt), [releases](https://github.com/philburk/portaudio-java/releases).
54. JNI exposed host API: [PortAudio.java](https://raw.githubusercontent.com/philburk/portaudio-java/main/src/main/java/com/portaudio/PortAudio.java).
55. JNI stream/control boundary: [BlockingStream.java](https://raw.githubusercontent.com/philburk/portaudio-java/main/src/main/java/com/portaudio/BlockingStream.java), [StreamParameters.java](https://raw.githubusercontent.com/philburk/portaudio-java/main/src/main/java/com/portaudio/StreamParameters.java).
56. Java SE 25 contracts: [SourceDataLine](https://docs.oracle.com/en/java/javase/25/docs/api/java.desktop/javax/sound/sampled/SourceDataLine.html), [TargetDataLine](https://docs.oracle.com/en/java/javase/25/docs/api/java.desktop/javax/sound/sampled/TargetDataLine.html), [PCM_FLOAT encoding](https://docs.oracle.com/en/java/javase/25/docs/api/java.desktop/javax/sound/sampled/AudioFormat.Encoding.html).
57. OpenJDK Windows DirectSound implementation: [PLATFORM_API_WinOS_DirectSound.cpp](https://raw.githubusercontent.com/openjdk/jdk/master/src/java.desktop/windows/native/libjsound/PLATFORM_API_WinOS_DirectSound.cpp).
58. PortAudio native CMake options and ASIO provisioning: [CMakeLists.txt](https://raw.githubusercontent.com/PortAudio/portaudio/master/CMakeLists.txt).
59. SDK version/archive provenance: [vcpkg asiosdk package](https://vcpkg.io/en/package/asiosdk.html), [Microsoft vcpkg portfile](https://raw.githubusercontent.com/microsoft/vcpkg/master/ports/asiosdk/portfile.cmake). The portfile identifies the official Steinberg archive; the archive itself was not downloaded/extracted.
60. Mirrored 2025 ASIO license text: [audiosdk/asio LICENSE.txt](https://github.com/audiosdk/asio/blob/main/LICENSE.txt). **Secondary mirror, not independently authenticated as Steinberg's own repository.** Use actual pinned SDK terms for final legal decision.
61. GPLv3 combined-work and object-code conveyance: [GNU GPL v3](https://www.gnu.org/licenses/gpl-3.0.en.html).
62. PortAudio maintenance metadata: [latest release](https://api.github.com/repos/PortAudio/portaudio/releases/latest), [latest commit](https://api.github.com/repos/PortAudio/portaudio/commits?per_page=1).
63. RtAudio metadata: [release](https://api.github.com/repos/thestk/rtaudio/releases/latest), [commit](https://api.github.com/repos/thestk/rtaudio/commits?per_page=1).
64. miniaudio release metadata: [latest release](https://api.github.com/repos/mackron/miniaudio/releases/latest).
65. JUCE release metadata: [latest release](https://api.github.com/repos/juce-framework/JUCE/releases/latest).
66. SDL release metadata: [latest release](https://api.github.com/repos/libsdl-org/SDL/releases/latest).
67. Rust crate publication metadata: [portaudio](https://crates.io/api/v1/crates/portaudio), [portaudio-rs](https://crates.io/api/v1/crates/portaudio-rs), [miniaudio](https://crates.io/api/v1/crates/miniaudio), [wasapi](https://crates.io/api/v1/crates/wasapi), [coreaudio-rs](https://crates.io/api/v1/crates/coreaudio-rs), [alsa](https://crates.io/api/v1/crates/alsa).
68. Go PortAudio metadata: [commits](https://api.github.com/repos/gordonklaus/portaudio/commits?per_page=1), [tags](https://api.github.com/repos/gordonklaus/portaudio/tags?per_page=5).
69. malgo tag metadata: [tags](https://api.github.com/repos/gen2brain/malgo/tags?per_page=5), [tag commit](https://api.github.com/repos/gen2brain/malgo/commits/4de8979ec9496b6fbb45d10625a087d8cc011a62).
70. Oto release metadata: [latest release](https://api.github.com/repos/ebitengine/oto/releases/latest).
71. Microsoft historical direct-out caveat: [DSSPEAKER_DIRECTOUT Speaker Configuration](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/dsspeaker-directout-speaker-configuration).
