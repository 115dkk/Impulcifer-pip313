# Rust unsafe inventory for the Impulcifer rewrite

Observed **2026-09-07**. Target: the later decision, **Rust + Tauri 2; Windows `wasapi`; macOS/Linux CPAL**, not the superseded PortAudio recommendation in the original synthesis.

**[V]** = verified in cited source/docs/upstream test definitions or identified local files. **[I]** = inference, recommendation, estimate. **[U]** = unresolved. A safe function signature is not a soundness proof. “Audit unverified” means no applicable independent audit was established, not that none exists. Versions are those inspected, not necessarily the newest releases. This was source research, not a build, hardware test, exhaustive dependency audit, or execution of UB reproducers.

## 1 Verdict

1. **[I] Yes: every application crate can reasonably retain `#![forbid(unsafe_code)]`, including DSP, audio orchestration, Tauri, CLI, and PyO3. No required capability inherently demands handwritten application `unsafe`.**
2. **[V] The dependency graph and generated code are not unsafe-free.** Native APIs, FFT/SIMD, array storage, queues, Python, webviews, and updater implementations contain unsafe operations. [1–27, 30–48]
3. **[V] `wasapi` 0.24.0 is not entirely safe to use indiscriminately:** `Device::from_raw` is explicitly unsafe; safe `WaveFormat::parse(&WAVEFORMATEX)` permits an out-of-bounds read. [2–4]
4. **[V] `coreaudio-rs` 0.14.2 exposes an unsound safe raw-pointer constructor.** Normal CPAL use reaching that defect was not demonstrated. [13]
5. **[I] Accept the zero-handwritten-unsafe architecture, not an “audited dependencies” claim.** Audio dependencies need focused review, defective-API exclusion or upstream fixes, and hardware gates.
6. **[V] PyO3 0.27.0/0.29.0 have upstream forbid-unsafe compile-pass fixtures including module exports.** Their generated FFI remains unsafe internally. [40–42]
7. **[I] Prefer owned buffers, safe copies, dedicated audio threads, checked lengths, and ordinary file reads; reject premature raw pointers, mmap, custom SIMD, and shared memory.**
8. **[I] If missing native functionality truly requires direct Win32 calls, use one reviewed leaf-crate exception.** That preserves zero unsafe in application crates, but not zero unsafe in *all owned crates*.
9. **[U] The complete selected feature/target/Python-ABI matrix has not been compiled under the lint.** Source feasibility is not an accomplished build gate.

## 2 Findings (with sources)

### 2.1 Scope, local evidence, and what the lint means

**[V, local]** Read `00-synthesis.md`, `01-windows-audio-api.md`, `02-cpal-vs-portaudio.md`, `report-01-audio-io.md`, and `report-03-rust-tauri.md`. Also inspected `core/recorder.py:260–549`, `autoeq/frequency_response.py:350–524`, `autoeq/biquad.py:1–120`, and `core/eqapo.py:1–140`. The current recorder starts a recording thread at lines 514–519 and invokes blocking playback at 539; the new implementation must preserve the blocking session contract with separate stream owners. The optimizer actually calls bounded `least_squares` at line 495. Earlier reports' line numbers and DirectSound-first descriptions predate the current WASAPI change. These local files are context, not independent proof of Rust API safety.

**[V]** `unsafe_code` checks unsafe blocks/functions/implementations and potentially unsound attributes such as `no_mangle`, `export_name`, and `link_section`. Applying it to an application crate does not ban unsafe inside separately compiled dependencies. External macro spans also have diagnostic exemptions; PyO3's tests demonstrate why generated unsafe can coexist with the lint. [1, 40–42]

| Statement | Judgment |
|---|---|
| All handwritten application Rust can forbid unsafe | **[I] Feasible**, subject to the chosen API restrictions and build gates. |
| The resulting executable contains no unsafe implementation | **[V] False.** Dependencies, macro-generated FFI, standard library, native libraries, and OS/browser components remain trusted. |
| Pure Rust means no FFI | **[V] False.** `wasapi` calls Windows COM; CPAL calls Apple and ALSA APIs. [2, 11–13] |
| A `SAFETY:` comment, tests, Miri, or popularity constitutes an independent audit | **[I] Reject this terminology.** They are different evidence. |
| Moving unsafe to our own leaf crate achieves zero unsafe in all our crates | **[V] False by definition.** It meets only the explicitly allowed application/leaf separation. |
| Safe Rust eliminates all security/reliability bugs | **[V] False.** Unsafe dependencies can be unsound; safe code can mishandle permissions, allocation, cancellation, and numerical semantics. [3, 13, 28–29, 39, 44] |

### 2.2 Capability inventory

**Table convention:** API and internal-source cells marked [V] are verified. “No” means no handwritten unsafe is necessary for the prescribed caller path, not that every API in that crate is safe. Unless a row says otherwise, **independent audit coverage for that exact version is [U]**. Release existence is verified; sustained maintenance commitments are addressed separately in §2.11.

| Capability | Crate/version inspected; safe caller path | Our unsafe? | Internal unsafe / audit evidence | Fallback or restriction [I] |
|---|---|---|---|---|
| Windows endpoint enumeration, names/IDs, format probes | **[V] wasapi 0.24.0**: `DeviceEnumerator::new`, `get_device_collection`, iterator, `get_device`, `Device::get_id`, `get_iaudioclient` [2–5] | **No** | COM/PROPVARIANT/pointer handling. Safe `WaveFormat::parse` defect; explicit unsafe `Device::from_raw`. No independent audit verified. | Do not expose raw device/format constructors. Exchange owned IDs; enumerate on an audio thread. |
| WASAPI shared/exclusive render | **[V] wasapi 0.24.0**: `initialize_client`, render-client getter, `write_to_device(...,&[u8],...)`, start/stop/padding/clock [2, 5] | **No** | Native `GetBuffer`/`ReleaseBuffer`, internal mutable raw slice, narrowing arithmetic. | Checked frame/byte counts; preallocated bytes; bounded writes. Direct Win32 only after demonstrated gap. |
| WASAPI stereo capture | **[V] wasapi 0.24.0**: capture-client getter, `read_from_device(&mut [u8]) -> (u32, BufferInfo)` [2, 5] | **No at call site** | Internal raw slice; SILENT flag is returned but not handled before copying. Conditional soundness concern, §2.3. | Review/fix dependency silence handling; do not claim a post-copy zero-fill prevents internal invalid reads. |
| COM initialization and event waits | **[V] wasapi 0.24.0**: `initialize_mta/sta`, `deinitialize`, `Handle::wait_for_event` [2, 5–7] | **No** | CoInitializeEx/CoUninitialize, CreateEvent/WaitForSingleObject/CloseHandle. Safe API does not encode apartment lifetime. | Private non-Send COM guard and lexical ownership, finite waits, no COM objects crossing threads. |
| Direct native fallback | **[V] windows 0.62.2**, Windows audio COM methods [10] | **Yes if used directly** | Generated unsafe COM calls; safe Clone/cast/value helpers do not make audio calls safe. | Upstream safe wrapper fix first; otherwise separate reviewed leaf exception. |
| macOS capture/render | **[V] cpal 0.18.2 → coreaudio-rs 0.14.2 → objc2-* 0.3 family** [11–13] | **No** | Native callbacks, AudioBufferList pointers, property reads, unsafe Send. CoreAudio wrapper safe-API defect, §2.4. | Use CPAL typed builders only; do not use coreaudio raw helpers directly. |
| Linux ALSA capture/render | **[V] cpal 0.18.2 → alsa 0.11.0 → alsa-sys 0.4.0**, as in CPAL's packaged lock [11–12] | **No** | libasound C ABI, raw audio buffers, libc self-pipe/thread teardown; explicit unsafe Sync in backend. | ALSA defaults first. Additional JACK/PulseAudio/PipeWire features require their own resolved-graph review. |
| Complex/real FFT, convolution/correlation building blocks | **[V] RustFFT 6.4.1 / RealFFT 3.5.0**, safe planners and scratch APIs [15–16] | **No** | SIMD/raw-pointer implementations; RealFFT's inspected `lib.rs` has two explicit unsafe blocks. Miri fix in earlier RealFFT 3.4.0, not independent audit. | Standard planner plus reusable initialized buffers; scalar planner for diagnostic comparison. |
| Multidimensional arrays/views/parallel axes | **[V] ndarray 0.17.2**, slicing, `AxisIterMut`, `par_azip!`, `rayon` feature [17] | **No** | Raw views, pointer offsets, storage invariants internally. | Safe constructors and checked views only; `Vec<f64>` sufficient for ordinary DSP buffers. |
| Matrix algebra / spline/fit linear solves | **[V] nalgebra 0.35.0**, initialized matrices and safe operations [18] | **No** | Unsafe storage traits, uninitialized allocation, pointer conversions. Historical serde issue fixed before this version. [29] | Do not use raw storage, unchecked indexing, or uninitialized output APIs. |
| Per-speaker jobs and parallelism | **[V] rayon 1.11.0 / rayon-core 1.13.0**, parallel iterators and scoped work [19] | **No** | Type-erased jobs, unsafe Send/Sync, raw Box/Arc, UnsafeCell. | Collect in fixed speaker order; serial global reductions; never run Rayon jobs from audio callbacks. |
| SciPy-compatible primitives not supplied by crates | **[I] Owned f64 Rust algorithms**, §2.6; safe slices, numeric methods, FFT, matrices [15–19, 49] | **No inherent need** | Our implementation can be unsafe-free; dependencies are not. No TRF-equivalent solver qualified here. | Port semantics explicitly; keep optional optimizer on Python until qualified if necessary. |
| WAV input and 30/32-track float32 output | **[V] hound 3.5.1**, `WavReader`, `WavWriter::write_sample`, u16 channels [20] | **No** | Internal transmute/uninitialized storage/unchecked optimized writer operations. Maintenance uncertain; last publication 2023-09-25. | Custom **safe** RIFF writer if default channel mask is unacceptable; explicit format tests. |
| File loading instead of mapping | **[V] std file/read/buffer APIs**; **memmap2 0.9.9** file-map APIs are unsafe [21, 49] | **No for reads; yes for file mmap** | mmap validity depends on external file mutation/truncation. Read-only and COW do not remove obligation. | Ordinary bounded reads/streaming for files of a few hundred MB. No mmap needed. |
| Audio bytes / numeric conversion | **[V] std `to_le_bytes/from_le_bytes`; bytemuck 1.25.2 checked slice casts** [49–50] | **No** | bytemuck internal pointer casts; manual `Pod` implementation is unsafe. | std conversion first; checked casts only for built-in primitive types and known endianness/alignment. |
| PNG plots and font handling | **[V] Plotters 0.3.7**, bitmap output, safe font registration [22] | **No** | Default ttf backend includes lifetime transmute and native font-kit services. `ab_glyph` implementation file has no unsafe blocks; transitive graph not certified. | Disable default ttf; explicitly select bitmap encoding + ab_glyph, bundle licensed fonts. |
| Offline HTML / TXT / CSV / settings / i18n | **[V] std writing; serde_json 1.0.151 safe serialization; existing pinned Plotly.js asset policy** [51, local research] | **No** | serde_json internally uses unchecked UTF-8/pointer operations; browser/Plotly is outside Rust lint. | Escape HTML/script boundaries and CSV fields; isolate reports from privileged Tauri IPC. Plotly release version not re-audited here. |
| ffmpeg/ffprobe process lifecycle | **[V] std `Command`, args/env/spawn, Child wait/kill; safe Windows `creation_flags`** [48] | **No** | std encapsulates process FFI. FFmpeg is a separate native executable, not Rust-safe. | Absolute executable path and argument array; no shell, pre_exec, raw handle plumbing, or in-process libav FFI. |
| Helper download/integrity | **[V] reqwest 0.13.4**, bounded chunk reads; **sha2 0.11.0** Digest API [52] | **No** | Network/TLS/OS dependency graph and SHA acceleration contain unsafe/native operations. Exact complete graph/audit unknown. | Pinned hashes/trusted metadata, size bounds, atomic installation; no claim that TLS or SHA alone authenticates an arbitrary feed. |
| Tauri commands, control IPC, themes | **[V] tauri 2.11.5**, command macros and `WebviewWindow::set_theme` [30–32] | **No** | Tauri itself contains unsafe, not just wry/tao; WebView2/native menu/event handling. Historical Tauri v2 external security audit, not current whole-graph proof. [39] | Standard decorated opaque window, scoped commands; avoid custom DWM/window procedure. |
| Native titlebar and native handles | **[V] tao 0.35.0** dark-mode helper; **raw-window-handle 0.6.2** [32–33] | **No for theme; only-if raw integration** | Tao calls DwmSetWindowAttribute internally; `WindowHandle::borrow_raw` is unsafe; safe raw-handle access does not license arbitrary native use. | Use Tauri theme API; test Windows high contrast/system theme; no custom titlebar FFI. |
| Native file/folder dialogs | **[V] tauri-plugin-dialog 2.7.3**, safe file/folder builders [34] | **No** | Plugin `desktop.rs` itself calls unsafe `borrow_raw`; native rfd backend beneath it. | Nonblocking callback API on UI thread; never blocking dialogs on event-loop thread. |
| Open path / URL / reveal | **[V] tauri-plugin-opener 2.5.5**, safe methods [35] | **No** | Inspected lib.rs has no unsafe blocks; delegated platform code/dependencies are not established unsafe-free. | Keep URL scheme/path policy; do not equate safe Rust with authorization. |
| macOS/Linux in-app updater | **[V] tauri-plugin-updater 2.11.0**, safe check/download/install [36] | **No** | Plugin has native unsafe Windows installer code even though this product uses it elsewhere; dependency graph not independently audited here. | Use signature-verifying download flow and idle-only apply. One updater per installation. |
| Windows updater | **[V] velopack 1.2.0**, safe startup/manager APIs [37] | **No** | `process_win.rs` uses unsafe GetCurrentProcess/TerminateProcess. `unsafe_apply_updates` is a **safe fn with an operationally warning name**. | Prefer coordinated exit/apply; startup hook before Tauri. Keep existing package identity. |
| Python extension / NumPy exchange | **[V] PyO3+numpy 0.27.0 or 0.29.0**, borrow guards, copies, from_vec/from_slice, detach [40–44] | **No on prescribed path** | Generated extern exports, CPython/NumPy C API, raw array/borrow internals. External macros bypass portions of lint diagnostics. | Copy inputs under an explicit synchronization contract, detach on owned data, return new arrays; no borrowed Python memory in jobs. |
| Optional SIMD hot loops | **[V] wide 0.7.33, pulp 0.21.5**, safe arithmetic/dispatch surfaces [23–24] | **No; only-if direct intrinsics** | Internal SIMD/pointer casts; wide compile-time selection, pulp runtime dispatch; no exact-version independent audit verified. | Auto-vectorization first; safe SIMD only after profiling. Portable SIMD remains nightly experimental. [25] |
| Real-time SPSC transfer | **[V] ringbuf 0.4.8 / rtrb 0.3.5**, safe push/pop/slice/batch conveniences [26–27] | **No** | Unsafe shared storage/atomic ownership. Manual uninitialized commit/index advance is unsafe. rtrb earlier versions have a 2026 advisory. [28] | Preallocate numeric ring; do not use low-level commit APIs. rtrb >=0.3.5 within 0.3 line. |
| General queues / logs | **[V] crossbeam 0.8.4, queue 0.3.12, channel 0.5.15** [27–28] | **No** | Unsafe storage/type-erased concurrency; earlier channel double-free fixed in 0.5.15. | Bounded channels off callback; do not assume every nonblocking API is wait-free or allocation-free. |
| Globals / cancellation / caches | **[V] std OnceLock/LazyLock, Mutex/Arc, atomics** [53] | **No** | std unsafe internally; no need for `static mut` or handwritten Send/Sync. | Explicit owners; atomic cancellation is a request, not forced thread termination. |
| System information | **[V] sysinfo 0.39.6**, safe System methods [54] | **No** | Windows system implementation calls unsafe GetSystemInfo; no current full audit established. | Use std constants/build metadata where enough; sysinfo only for required richer information. |
| Optional named pipes | **[V] tokio 1.53.1 Windows named-pipe ServerOptions::create** [55] | **No on basic path** | Native pipe/runtime internals; raw security-attributes constructor is unsafe. | Not required by planned Tauri control IPC; CLI stdio is simpler. |
| Shared memory / direct FFI / native extensions | **[I] Not needed by selected architecture** | **Only-if introduced** | File/shared mapping, raw callbacks, OS handles demand lifetime/aliasing/security review. | Keep samples in Rust memory; no shared-memory audio transport through the webview. |

### 2.3 WASAPI 0.24.0: safe caller surface, real defects, and COM discipline

#### Public API versus native memory

**[V]** The ordinary API accepts caller-owned byte slices: render consumes `&[u8]`; capture fills `&mut [u8]`. It does **not** lend application code a native `&mut [u8]` whose lifetime the application must manually end. Internally it obtains native pointers, constructs temporary slices, copies, then releases. Deque variants exist but capture may grow the deque and unwrap a ReleaseBuffer failure. Prefer preallocated slice operations. [2, 5]

**[V]** Safe APIs cover requested stream configuration: `StreamMode::{EventsExclusive,PollingExclusive,EventsShared,PollingShared}`; shared variants have `autoconvert`; `WaveFormat::new` accepts channel count and optional mask. `initialize_client` takes `&mut self`; service getters, start, stop, reset, padding, buffer size and audio clock are safe. Returned COM wrappers and Handle are `!Send`/`!Sync` in the inspected docs. [2, 4–5]

**[V]** `Device::from_raw(IMMDevice, Direction)` is `pub unsafe fn`; the caller must ensure actual data-flow direction matches. `from_immdevice` determines it safely. Thus the blanket assertion “every public method is safe” is false even before considering soundness. [2, 4]

#### Source-level soundness defect: safe WaveFormat parser

**[V, source diagnosis; not executed]** `WaveFormat::parse(&WAVEFORMATEX)` is public, exported, and declared safe. At `waveformat.rs:123–137`, when `wFormatTag` is extensible and `cbSize` is at least the structure-size difference, it casts the supplied reference to `*const WAVEFORMATEXTENSIBLE` and reads the larger structure. A standalone initialized WAVEFORMATEX whose fields claim tag extensible and cbSize=22 is still a valid Rust value. Header fields do not prove that additional storage exists. The larger read goes outside that object's allocation. This is an unsound safe API, not merely a malformed-file error. [3–4]

**[V]** `parse_from_blob_bytes(&[u8])` instead checks actual byte length and uses unaligned reads. `get_device_format` uses that byte parser; `get_mixformat` and format negotiation read COM-allocated structures through their own code. The exposed parser defect does not establish that normal device enumeration automatically triggers it. [2–3]

**[I] Policy:** prohibit `WaveFormat::parse` in our adapter and request an upstream fix that changes the contract (unsafe pointer constructor or genuinely length-bounded input). Use validated `new`, validated explicit structures, or the byte parser. Do not import raw Windows format values from arbitrary application input. An upstream review of the other native-format reads remains necessary. A safe wrapper that never calls the defective method reduces exposure; it does not make the dependency globally sound.

#### Capture silence: verified omission, conditional UB concern

**[V]** Both capture methods decode `BufferInfo` and then unconditionally construct a slice and copy native bytes for a nonempty packet; neither checks SILENT before accessing the pointer (`api.rs:1931–2004`). Microsoft requires treating all packet samples as silence and ignoring actual data values when SILENT is set. [2, 8]

**[I]** If the caller records those bytes without replacing them with zeros, signal content can be wrong. If an actual native response supplies null or otherwise invalid/uninitialized storage for such a packet, the wrapper's slice creation/read is invalid before application code receives the flag. That possible memory-safety consequence needs a native-contract/reproduction investigation.

**[U]** The inspected Microsoft GetBuffer page does **not** say SILENT always returns null. Its explicit null discussion concerns `AUDCLNT_E_BUFFER_ERROR`; the official sample sets its own `pData = NULL` after seeing SILENT to tell its sink to synthesize zeros. Do not cite that assignment as proof Windows returned null. No silent-packet UB reproduction was performed. [8]

**[I]** Correct wrapper behavior should branch on SILENT before dereferencing native data and fill initialized destination bytes with zero. Post-copy application zero-filling handles signal semantics only, not the conditional internal invalid-read problem. Require this review before claiming production-ready capture safety.

#### Lengths, lifetimes, and operational errors

| Verified source observation | Consequence / policy [I] |
|---|---|
| Render zero-frame writes return early; ordinary writes check byte-slice length, but multiply usize lengths and cast frame count to u32 without checked conversion. [2] | Cap frames by queried capacity and u32 range; checked multiplication. Do not market safe signatures as validation of arbitrary huge requests. |
| WaveFormat::new computes block alignment/rate and narrows integer fields without complete validation. [3] | Own config validation: channel/rate/container/valid-bit/mask limits, positive values, checked sizes. |
| Capture too-small destination releases the returned packet and errors. [2] | Allocate for maximum packet, not just an earlier padding snapshot; exclusive capture packet can grow before GetBuffer. [9] |
| Event creation followed by failed SetEventHandle does not establish Handle ownership first. [2] | Source-visible error-path handle-leak concern; seek upstream fix and test failed initialization. |
| Handle wait maps all non-success statuses to EventTimeout. [2] | Do not infer every timeout is merely lack of audio; bound retries and propagate terminal device errors. |
| Safe deinitialize is an unconditional CoUninitialize wrapper with no ownership token. [2] | Private RAII protocol must balance initialization and prevent early teardown. |
| Service wrappers own COM interfaces but are not Rust-lifetime-borrowed from AudioClient. [2] | No automatic UAF conclusion: COM references own native lifetime. Microsoft says stream lives until audio-client and service references are released. Still drop services before client and COM guard. [8] |

**[V]** Issue #62's “unsafe” 24-bit WAVEFORMATEX fallback concerns format interpretation/noise and underruns; it is not the newly identified parser-overread proof. [6]

#### COM and Tauri STA/MTA rules

**[V]** CoInitializeEx applies to the calling thread. Both S_OK and S_FALSE must be balanced by CoUninitialize; RPC_E_CHANGED_MODE is failure, not permission to continue or tear down someone else's COM initialization. OLE initializes STA; incompatible MTA initialization fails. Microsoft retains a specifically Windows 8 warning that first IAudioClient use should be on STA. This is not a documented blanket ban on MTA audio workers on Windows 10/11. [7]

**[I] Required pattern:**

1. Leave Tauri's main/UI apartment alone. Do not call `initialize_mta` or `deinitialize` there to repurpose it for audio.
2. On each dedicated audio OS thread, initialize MTA and check HRESULT. Create enumerator, device, AudioClient, services and event there; use and destroy them there.
3. Send owned IDs, metadata, settings and sample buffers between UI/coordinator/audio threads, never COM wrappers. Even enumeration requested by the UI runs on an audio-management worker.
4. Use a private COM guard which cannot be sent to another thread (for example a `PhantomData<Rc<()>>` marker). Construct it before nested client owners, and scope/destroy all native owners before its Drop calls safe `wasapi::deinitialize`.
5. Do not add `unsafe impl Send/Sync` to “fix” a compiler error. Ordinary async jobs can migrate threads; never hold apartment-bound audio state across executor migration. Dedicated threads avoid this problem.
6. Windows 8 compatibility would need a separate STA-first design/test. It is outside this Windows 10/11 target; do not quietly claim it.

#### Source count, not a geiger certificate

**[V, source-text inventory]** Inspected v0.24.0 files contain the following explicit `unsafe {` sites: `api.rs` **99**, `waveformat.rs` **5**, `events.rs` **15**, of which **5 are tests**. The inspected `lib.rs` and `errors.rs` contain none. This gives **114 production sites and 5 test sites in those files**, plus **one public unsafe function** (`Device::from_raw`); no explicit unsafe impl was found there. [2–3]

`api.rs` subtotal: module helpers 5; DeviceEnumerator 5; device-registration Drop 1; DeviceCollection 3; Device 12; AudioClient 33; session manager/enumerator/control 1/2/8; meter 5; session-registration Drop 1; clock 2; render 6; capture 9; Handle 2; effects 3; AEC 1.

**[U]** These are source-text counts, not executed cargo-geiger/AST counts; exclude macro expansions, dependency source, examples and any uninspected/generated code. A proposed archive-count command did not run because it required approval. Grouping many operations inside one block lowers the count without reducing the proof obligation. All native COM audio calls inspected are inside unsafe blocks, but there is not one block per native call.

### 2.4 CPAL 0.18.2, CoreAudio, ALSA, and soundness history

**[V]** Typed and `*_raw` input/output builders are safe. The raw suffix means runtime sample-format dispatch, not an unsafe pointer requirement. Callbacks require `FnMut + Send + 'static`, not Sync. `Data::bytes/as_slice` and mutable counterparts are safe; `Data::from_parts` is explicitly unsafe and belongs in backend implementation, not application code. Borrowed callback buffers must not escape. [11]

**[V] Version correction:** coreaudio-rs **0.14.2 uses objc2-* 0.3 dependencies, not coreaudio-sys**. CPAL's inspected package resolves ALSA **0.11.0 / alsa-sys 0.4.0**, not latest ALSA 0.12.1. Library package lockfiles are evidence about that package, not the future application's resolution. [11–12]

**[V] Internal implementation:** CoreAudio wraps AudioBufferList pointers, native callbacks/properties and timestamps. ALSA creates worker threads with start/stop coordination, native PCM and libc self-pipe operations, raw-buffer wrapping, and an explicit unsafe Sync implementation relying on ALSA thread safety. Optional realtime features add native scheduling concerns and should not be enabled by habit for a non-low-latency recorder. [12]

**[V, source diagnosis; not executed]** Public safe `coreaudio::audio_unit::render_callback::action_flags::Handle::from_ptr` stores an arbitrary raw pointer; safe `get()` dereferences it internally without a null/lifetime guard. `Handle::from_ptr(std::ptr::null_mut()).get()` is expressible entirely in safe Rust and dereferences null. This is a wrapper API soundness defect. **[U]** Reachability through normal CPAL builders and a RustSec advisory for it were not established. Keep the defect's scope precise. [13]

**[V]** Coreaudio AudioUnit is Send while its direct callback setter bounds omit Send. **[I]** Capturing non-Send state and moving/invoking/destroying the unit across threads merits a separate soundness review. CPAL requires Send itself; this contrast is not proof of a CPAL callback data race. [13]

**[V] CPAL changelog records actual historical fixes:** CoreAudio callbacks after drop (0.16.0); ALSA shutdown race and CoreAudio null/alignment UB (0.17.0); ALSA drop race (yanked 0.17.2); CoreAudio loopback UB and startup/reentrancy fixes (0.18.0). 0.18.2's unsound Send+Sync fix is **WebAudio +atomics**, not a macOS/ALSA finding. [14]

**[V, reported; not reproduced]** Open #215 is an old ALSA/PulseAudio 32-channel crash report, #704 concerns macOS device removal silently choosing a default, and #873 concerns short/distorted Linux recordings. None proves a current 0.18.2 UB reproduction. The closed ARM timestamp report #1134 and merged #1137 are distinct. [14]

**[U]** Searches did not find a matching CPAL/coreaudio-rs/alsa advisory; the entire RustSec database and a resolved application lockfile were not audited. Do not say “RustSec clean” or “no known vulnerabilities.”

### 2.5 Direct `windows` fallback: size and containment

**[V]** In inspected windows **0.62.2**, audio COM methods such as Initialize, Start, Stop, GetBufferSize, GetService, render/capture GetBuffer and ReleaseBuffer are unsafe, even when their signatures hide raw output pointers. Clone, Interface::cast, GUID/value construction, and obtaining a raw pointer are safe helpers. “Every windows method is unsafe” is false; “direct WASAPI transport requires unsafe” is true. [10]

**[I, estimate, not measured implementation]** Minimal exclusive render+capture, fixed accepted format, event handles, start/stop/join and failure cleanup would require approximately **25–40 distinct unsafe call/pointer-operation sites**, **60–150 executable lines inside unsafe blocks**. Add robust enumeration, property names, format/alignment retries, recovery and scheduling: approximately **40–65 sites, 120–250 unsafe-block lines**. Overall adapter code is larger. Counting one giant unsafe block is meaningless.

**[I]** One module can contain the operations, but a separate `impulcifer-platform-wasapi` leaf crate gives stronger policy separation: no native handles/pointers leave it; public API returns owned metadata/buffers/session commands. Prove same-thread GetBuffer/ReleaseBuffer, bounds/overflow/alignment, SILENT handling, COM reference and event lifetime, failure cleanup, no panic across FFI. This is an **explicit exception to zero unsafe in all owned code**, not a trick for satisfying that stronger claim. Prefer a narrow upstream wasapi fix over duplicating its entire backend.

### 2.6 Every DSP primitive can use safe Rust; algorithm parity remains work

| Required operation | Unsafe-free application implementation plan [I] |
|---|---|
| rfft/irfft/fft and next_fast_len | RustFFT/RealFFT safe planners and scratch APIs; explicit normalization; checked factor-search/length arithmetic matching the intended SciPy variant. |
| fftconvolve/convolve/correlate/correlation_lags | Safe padded vectors, FFT products, checked index/crop arithmetic; direct convolution for small kernels. Fix lag sign, mode and length in tests. |
| butter/SOS and RBJ peaking/shelves | f64/complex arithmetic, fixed coefficient arrays, mutable per-channel state. Validate order, Q, frequencies and finite values; no foreign DSP library is inherently necessary. |
| firwin2/minimum_phase | Safe interpolation/FFT/log/exp/window loops; explicit floor, odd/even rules and output-length semantics. |
| Savitzky–Golay and FITPACK-style splines | Safe matrix factorization and coefficient evaluation. Preserve `interp` edges, log-frequency x values, extrapolation and k=1/2/3 behavior identified by existing research. |
| find_peaks and windows | Slice scans plus explicit plateau/tie/distance rules; hann/kaiser/window equations. |
| uniform_filter | Checked sliding sums and explicit boundary extension/origin rules; separable axis passes. |
| linregress / expit | Safe scalar reductions and stable logistic branches; preserve degeneracy/NaN policy. |
| spectrogram, waterfall, decay analysis | Safe windowed block iteration and FFT; analysis arrays separate from rendering. |
| nnresample-style polyphase rational resampling | Safe initialized FIR/polyphase buffers and checked phase indices; match filter design, padding and trim, not merely sample rate. |
| bounded least_squares/TRF | Implement or qualify a solver through safe vectors/matrices; no qualified SciPy drop-in established here. A solver gap is algorithm work, not evidence unsafe is needed. |

**[V]** The underlying safe slice/FFT/array operations exist. **[I]** Feasibility of implementing the missing algorithms in safe Rust does not establish that those implementations already exist or are numerically correct. [15–19, 49]

**[V]** RustFFT's default planner dispatches supported SIMD; x86 AVX acceleration requires AVX and FMA. Safe explicit SIMD planners check availability; scalar fallback exists. RealFFT does not normalize automatically. Its inspected source has exactly two explicit unsafe blocks in `lib.rs` (424–428, 745–749), reinterpreting real storage as complex storage. Callers do not do these casts. [15–16]

**[I]** Start with `Vec<f64>`, `vec![0.0; n]`, safe slices, `split_at_mut`, `chunks_exact_mut` and worker-local scratch. Neither ndarray nor nalgebra requires our use of `uninit`, `set_len`, raw views or unchecked indexing. Zero-initialization/copy cost must be measured before discussing an exception. The NumPy binding need not impose its ndarray version on the core if the interface uses Vec/slices.

**[I] SIMD policy:** first profile safe scalar code, allocations and cache behavior; let optimized Rust compile ordinary loops. This does not guarantee vectorization or a speedup. Reuse RustFFT's dispatch; if a measured non-FFT hotspot remains, prefer safe wide or pulp operations. Avoid `std::arch` pointer/load operations and unchecked target-feature dispatch in application code. wide commonly selects at compile time; pulp supplies runtime dispatch. Never ship a general binary compiled blindly with `target-cpu=native`. Portable SIMD has safe arithmetic but remains nightly experimental in the inspected documentation. [23–25]

**[V]** Rayon floating reductions have unspecified grouping; floating addition is not associative. **[I]** Parallelize independent speakers, gather in fixed order, and use a deterministic serial/fixed-tree global reduction. No data race does not mean bit-identical output. [19]

**[I] Numerical gates:** retain pinned Python reference inputs and per-stage f64 goldens; exact tests for channel order, shape, integer peak/trim decisions and sample counts; magnitude-aware absolute/relative tolerances for arrays and FR/ILD/IPD/ITD/IACC/decay metrics. Initial `atol=1e-12`, `rtol=1e-10` and 0.01 dB diagnostic thresholds from the existing study are proposals, not approved universal bounds. SIMD/FMA, reduction grouping, FFT plans, libm and optimizer choices can change low bits without introducing UB. Keep SHA reproducibility tests within a pinned implementation/environment; do not require Python-to-Rust SHA equality as the only acceptance gate.

### 2.7 WAV, files, plots, processes, and callbacks

**[V] WAV:** Hound permits >18 channels and f32 samples. For 30 or 32 channels it writes the channel count, but its default extensible mask is `(1 << min(channels,18)) - 1`, hence **0x3FFFF**, not a meaningful mapping of 30/32 independent BRIR tracks. WavSpec has no caller mask field. [20]

**[I]** Use Hound only if consumer tests accept the exact header, or write a small safe RIFF/WAVEFORMATEXTENSIBLE writer with explicit zero/approved mask, channel order, block align, sizes, padding and finalization. Ordinary Write/Seek and `to_le_bytes` require no unsafe. Keep today's PCM_32 output compatibility distinct from the brief's proposed float32 format. Classic RIFF size limits, RF64 and 64-bit-float support are separate requirements, not solved by u16 channels.

**[V] File mapping:** memmap2 file-backed mappings are unsafe because concurrent external file modification/truncation can invalidate assumptions. Anonymous maps have safe constructors but are unnecessary here. **[I]** Use bounded standard reads or buffered streaming; metadata checks alone do not cap a file that grows during reading. No mmap for a few hundred MB is a reasonable default; cap parallel materialization to avoid multiplying memory demand. [21, 49]

**[V] Plotters:** ordinary PNG plotting and font registration are safe caller APIs. Default ttf code uses native font services and an internal lifetime transmute. The ab_glyph backend accepts static font bytes and uses a synchronized font registry. **[I]** Explicitly disable ttf/default features if avoiding native font discovery; enabling ab_glyph while leaving ttf active may still select ttf. Bundle glyph coverage for nine languages and test actual labels. Safe Rust does not provide complex shaping/bidi/font fallback automatically. [22]

**[V] Processes:** `Command::new/arg/env/spawn`, Child wait/kill and Windows `creation_flags(CREATE_NO_WINDOW)` are safe. Unix `pre_exec` is unsafe. In Rust 2024, process-global `env::set_var/remove_var` are unsafe APIs; Windows has a documented soundness exception but the signature remains unsafe. [48, 53]

**[I]** Launch ffmpeg/ffprobe directly with an absolute path, not cmd/bat/sh. Configure child environment with Command::env, not process-global mutation after threads start. Drain stderr/stdout to avoid pipe deadlock, check exit status, kill then wait on cancellation, and validate staged output before promotion. Do not promise Child::kill recursively kills every descendant; process-tree cleanup is a separate requirement. Native helper vulnerabilities and command/option injection are outside Rust's memory-safety lint.

**[V] Queue safety versus realtime:** rtrb 0.3.5 advertises allocation-free post-construction wait-free read/write; ringbuf 0.4.8 advertises lock-free SPSC. Their ordinary push/pop APIs are safe; raw chunk commit/index advancement APIs can be unsafe. crossbeam queues/channels are safe caller APIs but their behaviors differ: SegQueue can allocate, bounded send can block, and nonblocking does not imply wait-free. [26–28]

**[I] Callback policy:** preallocate f32 playback/capture storage or a bounded numeric SPSC; callback only copies/fills slices, advances counters and records minimal error state. No formatting/logging, disk I/O, locks, allocations, Rayon spawn, Python calls, UI calls or waiting. Process errors/logs on an ordinary worker. Check overflow/underflow and mark measurement failure rather than silently dropping samples. `Send + 'static` does not enforce these realtime rules; neither does `forbid(unsafe_code)`. Whole-buffer playback reduces the need for a queue; use one only where ownership/stream duration actually needs it.

### 2.8 Tauri, titlebars, plugins, and the real audit boundary

**[V]** Tauri **2.11.5** command-wrapper generation contains no introduced unsafe block or linker export attribute. Inspected context-generation source also did not introduce an explicit unsafe block, although nested macro expansion was not exhaustively compiled. `set_theme` is safe; on Linux/macOS its effects are application-wide. [30–31]

**[V]** Tauri **itself** uses unsafe in `app.rs`: Windows accelerator pointer handling/TranslateAcceleratorW, native menu theming, and other target-specific operations. Its 2.11.5 runtime manifest requests Wry **0.55.0**, Tao **0.35.0**, raw-window-handle **0.6**; these are compatibility requirements, not exact locks and not upstream latest versions. Tao 0.35.0 dark-mode code calls DwmSetWindowAttribute internally. Raw-window-handle 0.6.2 `borrow_raw` is unsafe; safe handle retrieval does not transfer its proof obligations to arbitrary application FFI. [30–33]

**[I]** A normal decorated window with `set_theme(Some(Dark))` needs no application DWM call. Verify OS builds, high-contrast behavior and live theme changes. If a genuine platform defect remains, report/fix the dependency first rather than adding undocumented DWM retries.

**[V]** Dialog 2.7.3 itself uses unsafe raw-handle borrowing; opener 2.5.5 exposes safe methods and delegates platform work; updater 2.11.0 exposes safe methods and includes a Windows ShellExecuteW unsafe block. Public `Update::install(bytes)` must not be treated as synonymous with the signature-verifying download flow. [34–36]

**[V]** Radically Open Security performed an external Tauri v2 pre-release security audit; Tauri's **2024-08-01** release-candidate announcement says findings were fixed and retested. **[U]** The precise report commit/scope was not extracted here, and that historical audit does not certify every unsafe block or the current 2.11.5/plugin/dependency graph. Do not label all Wry/Tao/browser code audited on that basis. [39]

**[I]** Keep Tauri commands as narrow request/response adapters. No remote web content receives privileged commands. Untrusted filenames/labels in exported HTML need correct escaping and an unprivileged browser context. These are security requirements despite requiring no unsafe. WebView2/WKWebView/WebKitGTK remain large native trusted components; retain normal browser servicing and sandbox behavior.

### 2.9 PyO3 / NumPy: zero handwritten unsafe, not zero generated FFI

| Operation | Verified safe path | Avoid / qualification |
|---|---|---|
| float64 input | `PyReadonlyArray1<f64>`; guard `as_slice()` or `as_array()` [43] | `as_slice` rejects noncontiguous input; use owned copy from a strided view or reject it explicitly. |
| mutable array access | `try_readwrite` then guard `as_slice_mut/as_array_mut` with `&mut self` [43] | Direct PyArrayMethods slice/view methods are unsafe; safe mutable guard is a different API. |
| Rust-owned processing | guarded `as_slice()?.to_vec()` or owned ndarray copy [43] | Copying must not race external writes/resizes. Borrow guards do not synchronize arbitrary Python/C/Fortran access. |
| output | `PyArray::from_vec`, `from_slice` [43] | from_vec transfers Rust allocation ownership with resize restrictions; from_slice copies. Neither needs our unsafe. |
| uninitialized / borrowed raw array | not needed | `PyArray::new`, `borrow_from_array`, raw pointer constructors and manual initialization are unsafe. |
| long job | safe `Python::detach` operating on owned Rust data [44] | 0.27 has deprecated allow_threads; 0.29 uses detach and no longer has it. Ungil/Send alone is not a proof that arbitrary Python-backed memory is safe. |

**[V]** Matching Rust crate versions matter: numpy **0.27.0** requests PyO3 0.27.0 and ndarray `>=0.15,<0.17`; numpy **0.29.0** requests PyO3 0.29.0 and declares ndarray `>=0.15,<=0.17`. These are manifest requirements, not proof of compatibility with every ndarray 0.17 patch. Resolve and test the actual lock; a Vec boundary avoids ndarray type coupling. [43]

**[V]** PyO3 0.27.0's `tests/ui/forbid_unsafe.rs` enables both forbid unsafe lints and includes hygiene tests containing real `#[pymodule]` exports; its pass registration excludes limited-API and experimental-inspect configurations. 0.29.0 has a `//@check-pass` fixture with module generation and its own limited-API exclusion. Upstream fixture existence is verified; this session did not run them. [40]

**[V]** PyO3 macros nevertheless emit `unsafe extern "C"` initialization/export code and `export_name`. Rust 1.90.0 lint source suppresses relevant external-macro-span diagnostics by default; `forbid` does not mean expanded foreign code disappears. PyO3 fixed an external-macro span/forbid regression in 0.22.4. **[I]** Keep a small compile-pass fixture in our actual toolchain/ABI matrix rather than weakening the crate lint preemptively. [41–42]

**[V]** Free-threaded Python's `Python<'py>` token means attached, not exclusive access. Rust-numpy borrow checks coordinate cooperating rust-numpy borrows, not arbitrary Python/native writes. NumPy 2.3 documents crashes from concurrent array resizing. PyO3 0.27 needs explicit free-threading opt-in; 0.29 defaults to declaring thread safety, with `gil_used=true` opt-out. [44]

**[I]** Contract: caller must not mutate or resize input during the synchronous copy; adapter obtains guarded access, validates dtype/shape/layout/limits, copies, releases all Python borrows, and detaches using owned Rust arrays only. Output is a fresh owned NumPy array. For an API that must tolerate arbitrary concurrently mutated inputs, accepting immutable bytes plus shape/rate metadata is safer than pretending a borrow guard freezes NumPy. That alternate input can remain zero-unsafe but is not a drop-in NumPy signature. Do not advertise free-threaded support until aliasing/copy and module-state tests pass.

### 2.10 Updater, global state, and optional IPC

**[V]** Velopack **1.2.0** exposes safe startup/check/download/apply APIs and has internal Windows process FFI. `unsafe_apply_updates` is not an unsafe Rust function; its name warns about update lifecycle. [37]

**[I]** Call Velopack startup handling before Tauri; preserve Windows install/package identity. Use Velopack only for Windows-managed installs and Tauri updater for its macOS/AppImage artifacts. An internal provider trait can be safe. Apply only after audio/DSP/output writes stop. Neither safe signatures nor a file hash proves the feed is authenticated; keep download verification and signing policy separate.

**[V]** OnceLock/LazyLock/Arc/Mutex/atomics provide safe shared state. Basic Tokio named-pipe creation is safe, while raw security-attribute construction is unsafe. [53, 55]

**[I]** No `static mut`, own UnsafeCell abstraction, global process-environment edits, raw DLL loading, native signal handlers, or shared-memory IPC is required. Configuration can be passed explicitly; callbacks send owned messages/counters; CLI communication can use stdio. If a future Windows job-object, custom ACL, device notification or priority feature lacks a sufficient safe abstraction, treat it as new leaf-crate work, not permission to relax all crate lints.

### 2.11 Maintenance, advisories, and what was not audited

| Component | Verified evidence | Honest conclusion |
|---|---|---|
| wasapi 0.24.0 | Published 2026-08-12; versioned source and open format/probing issues [2, 6] | Release activity exists; independent soundness audit not verified. Source defects prevent blanket endorsement. |
| CPAL 0.18.2 | Published 2026-08-16; SECURITY policy supports master/latest stable; documented fixes [11, 14] | Maintained latest line, not a promise to backport to every old release. |
| coreaudio-rs 0.14.2 | Published 2026-04-29; source inspected [13] | Recent release does not cancel identified safe-API defect. |
| Hound 3.5.1 | Published 2023-09-25 [20] | Safe usable APIs exist; current maintenance responsiveness unverified. Do not call it recently maintained. |
| RealFFT 3.5.0 | Changelog documents 3.4.0 Miri UB correction and 3.5.0 feature work [16] | Evidence of remediation, not complete audit. |
| Tauri v2 | External pre-release audit announced 2024-08-01 [39] | Historical security audit; exact current code/whole graph not certified. |
| Other numerical, queue, helper and Python dependencies | Versioned APIs and selected internals inspected | Sustained maintenance, bus factor and independent exact-version audit generally unverified by this task. |

**[V] Relevant advisories:**

- **rtrb RUSTSEC-2026-0274**, published **2026-09-01**: ReadChunk commit with panicking element Drop can double-free/use freed memory in versions `<0.3.5`. Patched ranges `^0.3.5` or `>=0.4.0`. Numeric f32 has no panicking destructor, but use the fixed release anyway. [28]
- **crossbeam-channel RUSTSEC-2025-0024**: Drop race/double-free introduced in 0.5.12, fixed 0.5.15. Audit resolved subcrates, not just crossbeam's umbrella version. [28]
- **nalgebra RUSTSEC-2021-0070**: inconsistent deserialized storage dimensions allowed out-of-bounds access; affected `>=0.11.0,<0.27.1`, fixed `>=0.27.1`. The inspected 0.35.0 is beyond that range. [29]

**[U]** No whole dependency lockfile exists for this proposed rewrite. No cargo-audit/geiger run, Miri run, native sanitizer run, callback stress test, or hardware test occurred. No independent current unsafe-code audit was verified for wasapi, CPAL, coreaudio-rs, the selected FFT/array/queue versions, Python binding stack, or Velopack. The searches are not an exhaustive absence-of-advisories proof.

## 3 Proposed architecture

All prescriptions here are **[I]**, grounded in the safe APIs above.

### 3.1 Crates and ownership

| Crate | Contract | Policy |
|---|---|---|
| `impulcifer-dsp` | f64 primitives, config defaults, stages; Vec/slice interfaces; deterministic gathering | `#![forbid(unsafe_code)]`; no GUI/Python/OS dependency |
| `impulcifer-audio` | safe backend trait, checked config, dedicated thread sessions, owned metadata | Same lint; target-gated wasapi/CPAL; exclude known defective raw/parse APIs |
| `impulcifer-analysis` | numerical analysis model, PNG/HTML and WAV/text exports | Same lint; explicit fonts and output headers |
| `impulcifer-jobs` | sequence journal, polling, bounded progress/logs, cancellation, stage ownership | Same lint; no native callbacks or borrowed Python data |
| `impulcifer-app` | thin Tauri commands/settings/dialogs/theme/updater provider | Same lint; no audio objects in UI thread/state |
| `impulcifer-cli` | batch entry point with no Tauri initialization | Same lint; safe std process/file operations |
| `impulcifer-python` | PyO3 module, guarded-copy-in / owned-compute / new-array-out | Same lint; macro-generated FFI tracked separately |
| Optional `impulcifer-platform-wasapi` | Only if required safe upstream implementation cannot be obtained | Explicit reviewed exception; never describe it as zero owned unsafe |

A private safe facade can restrict risky dependency APIs, but the facade's lint does not repair unsoundness inside code it calls. Track dependency fixes and exclusions by version. No explicit unsafe leaf is needed merely to use the documented happy-path APIs.

### 3.2 Two independent streams, finite playback, and drain

1. Validate requested rates, channel mappings, capacity, memory budgets and native format before starting. DSP remains f64; audio conversion to f32 is explicit/tested.
2. Dedicated capture thread initializes COM on Windows, creates/opens/starts capture, then acknowledges readiness. Playback thread independently initializes/opens render. Use IDs and owned buffers, not COM pointers transferred from UI enumeration.
3. Capture first; playback begins only after readiness. Track actual frames/discontinuities, not just elapsed wall time.
4. Render precomputed samples in bounded chunks. For WASAPI shared or **exclusive polling**, queried padding describes queued endpoint frames. Exclusive **event** mode processes complete buffers and cannot blindly reuse a padding-based fill/drain algorithm. Safe APIs exist for both; prefer the simpler polling model initially because latency is not a requirement. [5, 9]
5. End-of-source, endpoint queue empty, device-clock progress and actual acoustic completion are different facts. Model drain explicitly; preserve sufficient capture post-roll. Establish final physical tail behavior on hardware, not by assuming `stop_stream()` drains.
6. On cancellation/error, stop/abort both sides, propagate both errors, drop service/client/event owners on the owning thread, uninitialize COM, join workers, and only then publish terminal job state. No worker may keep writing after cancellation completion.
7. CPAL callbacks only copy/fill bounded buffers and signal progress. A coordinator provides the blocking whole-session interface; CPAL itself does not supply a portable whole-buffer physical-drain guarantee.

### 3.3 Exact leak list and prevention

| What would introduce our unsafe | Why | Zero-unsafe alternative |
|---|---|---|
| Calling windows COM/DWM/MMCSS/custom security APIs directly | Native lifetime/thread/pointer contracts not encoded in safe signatures | Existing safe wrapper; omit optional tweak; upstream fix; otherwise explicit leaf |
| Forcing COM/native wrappers Send/Sync | Native apartment/ownership may forbid transfer | Construct/use/drop on dedicated thread; message IDs/results |
| `WaveFormat::parse` or coreaudio Handle raw constructor | **Already safe syntactically but unsound**; lint will not catch use | Exclude them; do not pass arbitrary headers/pointers; upstream repair |
| File-backed memmap2 mapping | External mutation/truncation can invalidate memory | Bounded reads or streaming |
| Raw NumPy/new/uninitialized/borrowed Rust memory | Initialization, aliasing and owner lifetime obligations | Borrow guards, synchronized copy, from_vec/from_slice |
| Handwritten Python/C exports | Unsafe FFI contracts and export attributes | PyO3 external macros, compile fixtures; generated code remains trusted |
| Pointer SIMD, unchecked indexing, assume_init/set_len | Bounds, alignment, CPU features, initialization | Safe slices/Vec, RustFFT planner, safe wide/pulp if justified |
| Raw ring commits/index advance | Must prove initialized slots and publication order | Safe numeric push/pop/slice-copy APIs |
| `static mut`, unsafe Send/Sync/Pod implementations | Global aliasing or incorrect trait invariant | Explicit owners, atomics/locks/OnceLock; built-in POD types |
| Global env mutation or Unix pre_exec | Thread/process runtime safety contracts | Config structs; Command::env and purpose-built safe setters |
| Shared-memory/raw-handle IPC or custom callbacks | Lifetime, peer mutation, FFI unwind/security obligations | Tauri JSON/binary owned responses, stdio, ordinary safe named pipes |

### 3.4 Verification gates before implementation is called safe

- Put the crate-level forbid attribute in **every owned Rust target**, including library, binary, integration test, example and build script as applicable; workspace lint inheritance is useful but not automatic unless each member opts in. Do not pass a global RUSTFLAGS forbid setting to every dependency and mistake expected dependency failures for application failures.
- Compile the exact Windows/macOS/Linux feature matrix and Python ABI combinations. Include PyO3 module-export and representative Tauri macro fixtures under forbid; especially test limited-API/abi3 combinations excluded by upstream fixtures.
- Maintain lockfile/toolchain/target-feature records. Review new dependencies and macro-generated native boundaries; run RustSec tooling against resolved transitive versions. geiger counts are an inventory, not a safety score.
- Keep known-unsound API exclusions testable/reviewable. Upstream or separately review fixes for the audio issues; do not use `catch_unwind` as a UB remedy.
- Use Miri for pure Rust unsafe-dependent components where supported, fuzz parsers/config/shape arithmetic and WAV headers, test queue/drop/panic boundaries, and use platform sanitizers/stress tests where feasible. Miri does not execute arbitrary WASAPI/CoreAudio drivers.
- Hardware gates: 2/8/12/14/16 output configurations where physically available; 44.1/48/96 kHz accepted/rejected tuples; capture SILENT/discontinuity; startup failure, hot-unplug, cancellation during waits/drain, reopen and repeated teardown. Test real output channel identity and tail completion.
- Concurrency gates: overlapping Python calls and aliases, explicit no-mutation contract during copies, free-threaded module state, callback zero-allocation expectations, deterministic reduction/order tests.
- Preserve per-stage numerical goldens and final scientific metrics separately from memory-safety and packaging gates.

## 4 Risks ranked

Rankings and mitigations are **[I]**; factual bases are cited in Findings.

| Rank | Severity | Risk | Required response |
|---:|---|---|---|
| 1 | High | Safe APIs in audio dependencies have source-confirmed soundness defects; silence path has unresolved native-memory assumptions. | Exclude defective entry points, obtain/review upstream fixes, investigate silence before claiming production readiness. [2–4, 8, 13] |
| 2 | High | COM apartment teardown, callback lifetime and native thread contracts are not guaranteed by a safe-looking orchestration API. | Dedicated owners, no unsafe Send, checked init balancing, join-before-terminal, teardown tests. [7–14] |
| 3 | High | NumPy borrowed memory can be mutated/resized outside rust-numpy guards, especially free-threaded Python. | Synchronized copy-in contract; owned detached jobs; no retained views; test module defaults/version differences. [43–44] |
| 4 | High | False assurance from forbid, “pure Rust,” or historical audits masks dependency/native/generated code risk. | Separate policy metrics: handwritten unsafe, generated FFI, dependency unsafe, actual audit coverage and known defects. [1, 39–42] |
| 5 | High | Wrong channel mapping or truncated final audio despite successful stream calls; WASAPI f32 exclusive availability is device-dependent. | Hardware format/routing/drain acceptance, explicit conversion policy, no silent downmix. [5, 8–9] |
| 6 | High | SciPy/FITPACK/resampling/optimizer semantics differ while safe Rust tests only shape or plausible output. | Stage goldens, exact discrete decisions, scientific tolerance gates; no cross-language SHA-only test. |
| 7 | Medium–high | Hound default multitrack mask, RIFF limits, memory amplification and malformed sizes. | Explicit header/consumer tests, checked arithmetic, safe custom writer if needed, bounded allocations. [20–21, 49] |
| 8 | Medium | Callback allocations/blocking, queue overflow and old dependency soundness bugs. | Preallocated SPSC numeric buffers; fixed rtrb/crossbeam versions; worker-side logs; overflow as measurement error. [26–28] |
| 9 | Medium | Updater/helper/privileged-webview vulnerabilities are not prevented by Rust memory safety. | Scoped IPC, escaped unprivileged reports, authenticated verified downloads, idle-only apply. [34–39, 48, 52] |
| 10 | Medium | Macro/lint or ABI/feature drift breaks promised zero-unsafe builds. | Pinned toolchain, own compile fixtures and full platform/Python matrix; no blanket allow. [40–44] |
| 11 | Medium | Feature creep reintroduces raw DWM, mmap, native fonts, custom SIMD or shared memory without measured need. | Maintain explicit no-unsafe API policy; review exceptions separately and keep them leaf-only. |

## 5 Open questions you could not settle

1. **[U] Audio upstream resolution:** when will wasapi's safe parser and coreaudio's safe raw Handle constructor be corrected, and what versions will include fixes? No issue or patch was posted by this task.
2. **[U] Silent capture memory contract/reproduction:** does target hardware ever supply invalid/null/uninitialized storage with nonempty SILENT packets? The source omission is verified; native UB reproduction is not.
3. **[U] Complete audio-wrapper soundness:** COM deinitialization misuse, event lifetime/error cleanup, service ownership, malformed formats and callback trait contracts need a focused audit. This inventory is not that audit.
4. **[U] CPAL reachability:** normal CPAL 0.18.2 use triggering the identified coreaudio Handle defect or non-Send callback issue was not established.
5. **[U] Actual all-crate forbid build:** no Rust workspace/prototype was built. Exact Tauri/PyO3 macro, Rust edition, limited-API, free-threaded Python and target combinations remain untested.
6. **[U] Dependency resolution:** no application Cargo.lock or complete feature graph exists. CPAL's packaged lock and manifest requirements do not select the future graph, including numpy/ndarray compatibility.
7. **[U] Audit scope:** exact current-version independent unsafe audits were not established for most components. Tauri's historical audit scope/commit was not extracted; it is not a complete dependency certificate.
8. **[U] Hardware behavior:** independent 16-out/2-in streams, exclusive native f32 support, shared conversion, drain completion, drift, hotplug and Apple microphone permissions still need actual devices.
9. **[U] WAV consumer contract:** explicit mask/header expectations for 30/32-track HeSuVi/Hangloose/EAPO/JamesDSP output, PCM_32 versus float32 default, and any RF64 need are not settled.
10. **[U] Performance:** no evidence that safe initialization/copies are a bottleneck; no justification yet for mmap/raw SIMD/uninitialized buffers. Peak job memory and callback slack remain unmeasured.
11. **[U] Numerical budget and optimizer:** tolerances need maintainer approval against the golden corpus; no qualified bounded TRF replacement was selected.
12. **[U] Cross-platform fonts and updates:** packaged nine-language glyph/layout behavior, upgrade continuity from the current Velopack app, macOS/AppImage updater artifacts and helper lifecycle were not executed.
13. **[U] Maintenance beyond release metadata:** Hound responsiveness and sustained maintenance/audit commitments of every selected numerical/helper dependency were not established.

## 6 Sources

All accessed **2026-09-07**. Version-pinned links are preferred. Grouped URLs under one number support the same topic. Rolling std/Microsoft docs and issue pages are explicitly snapshots. Local research paths are identified in §2.1. A raw PDF fetch could not be decoded into usable audit text; only the official audit announcement supports the historical-audit claim. No cargo/compile/hardware verification is implied.

1. Rust unsafe-code lint and external macro diagnostic implementation: https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#unsafe-code ; https://raw.githubusercontent.com/rust-lang/rust/1.90.0/compiler/rustc_lint/src/builtin.rs ; https://raw.githubusercontent.com/rust-lang/rust/1.90.0/compiler/rustc_middle/src/lint.rs ; https://raw.githubusercontent.com/rust-lang/rust/1.90.0/compiler/rustc_lint_defs/src/lib.rs
2. wasapi 0.24.0 source, exports, manifest and release metadata: https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/api.rs ; https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/lib.rs ; https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/errors.rs ; https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/events.rs ; https://docs.rs/wasapi/0.24.0/src/wasapi/events.rs.html ; https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/Cargo.toml ; https://crates.io/api/v1/crates/wasapi/0.24.0
3. wasapi format parsing source: https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/waveformat.rs ; https://docs.rs/wasapi/0.24.0/src/wasapi/waveformat.rs.html
4. Public WaveFormat/Device API: https://docs.rs/wasapi/0.24.0/wasapi/struct.WaveFormat.html ; https://docs.rs/wasapi/0.24.0/wasapi/struct.Device.html
5. wasapi safe stream/enumeration/event surfaces: https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioClient.html ; https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioRenderClient.html ; https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioCaptureClient.html ; https://docs.rs/wasapi/0.24.0/wasapi/struct.DeviceEnumerator.html ; https://docs.rs/wasapi/0.24.0/wasapi/struct.DeviceCollection.html ; https://docs.rs/wasapi/0.24.0/wasapi/struct.Handle.html ; https://docs.rs/wasapi/0.24.0/wasapi/enum.StreamMode.html
6. wasapi issue inventory and distinct 24-bit fallback issue: https://github.com/HEnquist/wasapi-rs/issues ; https://github.com/HEnquist/wasapi-rs/issues/62 ; https://github.com/HEnquist/wasapi-rs/issues/63
7. COM/apartment rules and Windows 8 caveat: https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex ; https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iaudioclient
8. Capture buffer, silence and service-reference lifetime contracts: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer ; https://learn.microsoft.com/en-us/windows/win32/api/audioclient/ne-audioclient-_audclnt_bufferflags ; https://learn.microsoft.com/en-us/windows/win32/coreaudio/capturing-a-stream
9. Padding and exclusive-mode behavior: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient-getcurrentpadding ; https://learn.microsoft.com/en-us/windows/win32/coreaudio/exclusive-mode-streams
10. windows 0.62.2 metadata/core plus generated Windows audio docs (generated site snapshot): https://docs.rs/crate/windows/0.62.2 ; https://docs.rs/windows-core/0.62.2/windows_core/struct.GUID.html ; https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IAudioClient.html ; https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IAudioRenderClient.html ; https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IAudioCaptureClient.html ; https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IMMDevice.html ; https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/Com/fn.CoCreateInstance.html
11. CPAL 0.18.2 API, version, dependency/maintenance records: https://docs.rs/crate/cpal/0.18.2 ; https://docs.rs/cpal/0.18.2/cpal/traits/trait.DeviceTrait.html ; https://docs.rs/cpal/0.18.2/cpal/struct.Data.html ; https://docs.rs/crate/cpal/0.18.2/source/Cargo.toml ; https://docs.rs/crate/cpal/0.18.2/source/Cargo.lock ; https://docs.rs/crate/cpal/0.18.2/source/SECURITY.md
12. CPAL/ALSA internals: https://docs.rs/crate/cpal/0.18.2/source/src/host/alsa/mod.rs ; https://docs.rs/crate/cpal/0.18.2/source/src/host/coreaudio/mod.rs ; https://docs.rs/crate/cpal/0.18.2/source/src/host/coreaudio/macos/device.rs ; https://docs.rs/crate/alsa/0.11.0/source/src/pcm.rs ; https://docs.rs/crate/alsa/0.11.0/source/Cargo.toml ; https://docs.rs/crate/alsa-sys/0.4.0/source/build.rs
13. coreaudio-rs 0.14.2 public unsafe-behind-safe boundary: https://docs.rs/coreaudio-rs/0.14.2/coreaudio/ ; https://docs.rs/coreaudio-rs/0.14.2/coreaudio/audio_unit/render_callback/action_flags/struct.Handle.html ; https://docs.rs/crate/coreaudio-rs/0.14.2/source/src/audio_unit/render_callback.rs ; https://docs.rs/crate/coreaudio-rs/0.14.2/source/src/audio_unit/mod.rs ; https://docs.rs/coreaudio-rs/0.14.2/coreaudio/audio_unit/struct.AudioUnit.html
14. CPAL fix history and issue reports: https://docs.rs/crate/cpal/0.18.2/source/CHANGELOG.md ; https://github.com/RustAudio/cpal/issues/215 ; https://github.com/RustAudio/cpal/issues/704 ; https://github.com/RustAudio/cpal/issues/873 ; https://github.com/RustAudio/cpal/issues/1134 ; https://github.com/RustAudio/cpal/pull/1137
15. RustFFT 6.4.1 safe planning and unsafe SIMD: https://docs.rs/rustfft/6.4.1/rustfft/ ; https://docs.rs/rustfft/6.4.1/rustfft/struct.FftPlanner.html ; https://docs.rs/rustfft/6.4.1/src/rustfft/plan.rs.html ; https://docs.rs/rustfft/6.4.1/src/rustfft/avx/avx64_butterflies.rs.html
16. RealFFT 3.5.0 APIs, internal casts and change history: https://docs.rs/realfft/3.5.0/realfft/trait.RealToComplex.html ; https://docs.rs/realfft/3.5.0/src/realfft/lib.rs.html ; https://docs.rs/crate/realfft/3.5.0/source/README.md
17. ndarray 0.17.2 safe array/parallel APIs and unsafe internals: https://docs.rs/ndarray/0.17.2/ndarray/ ; https://docs.rs/ndarray/0.17.2/ndarray/parallel/index.html ; https://docs.rs/ndarray/0.17.2/src/ndarray/impl_views/conversions.rs.html
18. nalgebra 0.35.0 and storage internals: https://docs.rs/nalgebra/0.35.0/nalgebra/ ; https://docs.rs/nalgebra/0.35.0/src/nalgebra/base/vec_storage.rs.html
19. Rayon 1.11.0 / core 1.13.0, safe disjoint work and nondeterministic reductions: https://docs.rs/rayon/1.11.0/src/rayon/slice/mod.rs.html ; https://docs.rs/rayon/1.11.0/rayon/iter/trait.ParallelIterator.html#method.sum ; https://docs.rs/rayon-core/1.13.0/src/rayon_core/job.rs.html
20. Hound 3.5.1 version/WAV fields/writer/mask: https://docs.rs/crate/hound/3.5.1 ; https://docs.rs/hound/3.5.1/hound/struct.WavSpec.html ; https://docs.rs/hound/3.5.1/src/hound/write.rs.html ; https://github.com/ruuda/hound
21. memmap2 0.9.9 mapping safety: https://docs.rs/memmap2/0.9.9/memmap2/struct.MmapOptions.html ; https://docs.rs/memmap2/0.9.9/src/memmap2/lib.rs.html
22. Plotters 0.3.7 features/fonts/internals: https://docs.rs/plotters/0.3.7/plotters/ ; https://docs.rs/plotters/0.3.7/src/plotters/style/font/ttf.rs.html ; https://docs.rs/plotters/0.3.7/src/plotters/style/font/ab_glyph.rs.html ; https://docs.rs/plotters/0.3.7/src/plotters/style/font/mod.rs.html ; https://docs.rs/plotters/0.3.7/plotters/style/fn.register_font.html
23. wide 0.7.33 internals and usage policy: https://docs.rs/wide/0.7.33/src/wide/f32x4_.rs.html ; https://github.com/Lokathor/wide
24. pulp 0.21.5 safe dispatch and unsafe internals: https://docs.rs/pulp/0.21.5/pulp/ ; https://docs.rs/pulp/0.21.5/src/pulp/lib.rs.html
25. Portable SIMD status, inspected versioned std docs: https://doc.rust-lang.org/1.98.1/std/simd/index.html
26. ringbuf 0.4.8 safe and raw producer operations: https://docs.rs/ringbuf/0.4.8/ringbuf/ ; https://docs.rs/ringbuf/0.4.8/ringbuf/traits/producer/trait.Producer.html
27. rtrb/crossbeam versions and internals: https://docs.rs/rtrb/0.3.5/rtrb/ ; https://docs.rs/rtrb/0.3.5/rtrb/chunks/struct.WriteChunkUninit.html ; https://docs.rs/crossbeam/0.8.4/crossbeam/ ; https://docs.rs/crossbeam-channel/0.5.15/crossbeam_channel/ ; https://docs.rs/crossbeam-queue/0.3.12/src/crossbeam_queue/array_queue.rs.html
28. Queue/channel soundness advisories: https://rustsec.org/advisories/RUSTSEC-2026-0274.html ; https://rustsec.org/advisories/RUSTSEC-2025-0024.html
29. Historical nalgebra storage advisory: https://rustsec.org/advisories/RUSTSEC-2021-0070.html
30. Tauri 2.11.5 safe commands and internal unsafe: https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri-macros/src/command/wrapper.rs ; https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri-codegen/src/context.rs ; https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri/src/app.rs
31. Tauri theme/IPC APIs: https://docs.rs/tauri/2.11.5/tauri/webview/struct.WebviewWindow.html ; https://v2.tauri.app/develop/calling-rust/ ; https://v2.tauri.app/security/capabilities/
32. Actual Tauri runtime requirements and Tao 0.35.0 native titlebar implementation: https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.11.5/crates/tauri-runtime-wry/Cargo.toml ; https://raw.githubusercontent.com/tauri-apps/tao/tao-v0.35.0/src/platform_impl/windows/dark_mode.rs
33. raw-window-handle 0.6.2 API; additional pinned runtime/native source inspected: https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/struct.WindowHandle.html ; https://raw.githubusercontent.com/tauri-apps/tauri/tauri-v2.10.2/crates/tauri-runtime-wry/src/lib.rs ; https://raw.githubusercontent.com/tauri-apps/wry/wry-v0.54.0/src/webview2/mod.rs
34. Dialog 2.7.3 APIs and plugin's own unsafe: https://docs.rs/tauri-plugin-dialog/2.7.3/tauri_plugin_dialog/struct.FileDialogBuilder.html ; https://docs.rs/tauri-plugin-dialog/2.7.3/src/tauri_plugin_dialog/desktop.rs.html
35. Opener 2.5.5: https://docs.rs/tauri-plugin-opener/2.5.5/tauri_plugin_opener/struct.Opener.html ; https://docs.rs/tauri-plugin-opener/2.5.5/src/tauri_plugin_opener/lib.rs.html
36. Updater 2.11.0 safe API/native implementation and signature policy: https://docs.rs/tauri-plugin-updater/2.11.0/tauri_plugin_updater/struct.Update.html ; https://docs.rs/tauri-plugin-updater/2.11.0/src/tauri_plugin_updater/updater.rs.html ; https://v2.tauri.app/plugin/updater/
37. Velopack 1.2.0 safe startup/manager and internal process FFI: https://docs.rs/velopack/1.2.0/velopack/struct.VelopackApp.html ; https://docs.rs/velopack/1.2.0/src/velopack/manager.rs.html ; https://docs.rs/crate/velopack/1.2.0/source/src/process_win.rs ; https://docs.velopack.io/getting-started/rust
38. Velopack update API reference: https://docs.rs/velopack/1.2.0/velopack/
39. Tauri historical external audit announcement (usable evidence), report location (scope not extracted): https://v2.tauri.app/blog/tauri-2-0-0-release-candidate/ ; https://github.com/tauri-apps/tauri/blob/dev/audits/Radically_Open_Security-v2-report.pdf
40. PyO3 0.27/0.29 forbid compile-pass fixtures and registration conditions: https://raw.githubusercontent.com/PyO3/pyo3/v0.27.0/tests/ui/forbid_unsafe.rs ; https://raw.githubusercontent.com/PyO3/pyo3/v0.27.0/tests/test_compile_error.rs ; https://raw.githubusercontent.com/PyO3/pyo3/v0.27.0/src/tests/hygiene/pymodule.rs ; https://raw.githubusercontent.com/PyO3/pyo3/v0.29.0/tests/ui/forbid_unsafe.rs ; https://raw.githubusercontent.com/PyO3/pyo3/v0.29.0/tests/test_compile_error.rs
41. PyO3 generated unsafe exports: https://raw.githubusercontent.com/PyO3/pyo3/v0.27.0/pyo3-macros-backend/src/module.rs ; https://raw.githubusercontent.com/PyO3/pyo3/v0.29.0/src/impl_/pymodule.rs
42. PyO3 forbid/macro regression correction: https://github.com/PyO3/pyo3/pull/4574
43. Rust-numpy paired requirements, guards, constructors and unsafe methods: https://raw.githubusercontent.com/PyO3/rust-numpy/v0.27.0/Cargo.toml ; https://raw.githubusercontent.com/PyO3/rust-numpy/v0.29.0/Cargo.toml ; https://docs.rs/numpy/0.27.0/numpy/array/trait.PyArrayMethods.html ; https://docs.rs/numpy/0.29.0/numpy/array/trait.PyArrayMethods.html ; https://docs.rs/numpy/0.29.0/numpy/array/struct.PyArray.html ; https://docs.rs/numpy/0.29.0/numpy/borrow/struct.PyReadonlyArray.html ; https://docs.rs/numpy/0.29.0/numpy/borrow/struct.PyReadwriteArray.html
44. Python detachment and concurrent NumPy ownership: https://docs.rs/pyo3/0.27.0/pyo3/marker/struct.Python.html ; https://docs.rs/pyo3/0.29.0/pyo3/marker/struct.Python.html ; https://raw.githubusercontent.com/PyO3/pyo3/v0.29.0/src/marker.rs ; https://docs.rs/numpy/0.27.0/numpy/borrow/index.html ; https://docs.rs/numpy/0.29.0/numpy/borrow/index.html ; https://pyo3.rs/v0.27.0/free-threading.html ; https://pyo3.rs/v0.29.0/free-threading.html ; https://numpy.org/doc/2.3/reference/thread_safety.html
45. PyO3 API-removal change record: https://raw.githubusercontent.com/PyO3/pyo3/v0.29.0/CHANGELOG.md
46. CPAL platform Stream trait documentation: https://docs.rs/cpal/0.18.2/cpal/platform/struct.Stream.html
47. RustSec index (search only, not full lockfile audit): https://rustsec.org/advisories/
48. Safe process spawning, flags and unsafe pre-exec distinction: https://doc.rust-lang.org/std/process/struct.Command.html ; https://doc.rust-lang.org/std/os/windows/process/trait.CommandExt.html ; https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html
49. Safe slices, file reads and endian conversions: https://doc.rust-lang.org/std/primitive.slice.html ; https://doc.rust-lang.org/std/fs/fn.read.html ; https://doc.rust-lang.org/std/primitive.f32.html#method.to_le_bytes
50. bytemuck 1.25.2 checked caller API/internal unsafe: https://docs.rs/bytemuck/1.25.2/bytemuck/ ; https://docs.rs/bytemuck/1.25.2/src/bytemuck/internal.rs.html
51. serde_json 1.0.151 APIs/internal unsafe: https://docs.rs/serde_json/1.0.151/serde_json/ ; https://raw.githubusercontent.com/serde-rs/json/v1.0.151/src/read.rs
52. Helper network/hash APIs and acceleration: https://docs.rs/reqwest/0.13.4/reqwest/ ; https://docs.rs/reqwest/0.13.4/reqwest/struct.Response.html ; https://docs.rs/sha2/0.11.0/sha2/ ; https://docs.rs/sha2/0.11.0/src/sha2/sha256.rs.html
53. Safe global initialization and unsafe environment mutation: https://doc.rust-lang.org/std/sync/struct.OnceLock.html ; https://doc.rust-lang.org/std/env/fn.set_var.html
54. sysinfo 0.39.6 safe system-info API/native implementation: https://docs.rs/sysinfo/0.39.6/sysinfo/ ; https://docs.rs/crate/sysinfo/0.39.6/source/src/windows/system.rs
55. Tokio 1.53.1 optional named-pipe API: https://docs.rs/tokio/1.53.1/tokio/net/windows/named_pipe/struct.ServerOptions.html
