# Rust unsafe quarantine: architecture and enforcement

Observation date: **2026-09-07**. This report builds on the selected Rust/Tauri target; it does not reopen the stack decision.

**Evidence labels:** **[V]** verified in a cited source or identified local file; **[I]** engineering inference, proposal, estimate, or suggested test; **[U]** not established. Source inspection is not a complete soundness audit. Labels apply to the paragraph or table row containing them. Numbered references resolve to §6. Proposed APIs and CI commands were not compiled or executed. No new Rust workspace exists for this study.

## 1 Verdict

1. **[I] Default to zero handwritten unsafe; reserve one optional Windows leaf, `impulcifer-sys-win`, only for demonstrated native-API gaps.**
2. **[V] Tauri itself contains unsafe, as do runtime-wry, wry, and tao.** Copy the safe-interface pattern, not an imaginary transitive ban [1–5].
3. **[I] DSP, audio coordination, jobs, service, analysis/I/O, Tauri app, CLI, Python bridge, and build helpers must forbid unsafe.**
4. **[V] PyO3 owned APIs, rust-numpy owned-output constructors, and RustFFT do not require handwritten unsafe at ordinary call sites [22–31].**
5. **[I] Keep COM interfaces on their creating worker; send endpoint IDs, owned buffers, commands, and results instead.**
6. **[I] Enforce the boundary with compiler lints and a protected workspace-policy check; cargo-geiger is inventory, not certification.**
7. **[V] cargo-vet’s default deployment criterion is not an absolute soundness requirement; define a project-specific review criterion [42–45].**
8. **[I] Use Miri for pure/modelled code, native Windows tests for COM, and separate sanitizer jobs; none replaces review.**
9. **[I] Start on pinned Rust 1.98.1, edition 2024; keep dated nightly tooling separate from shipping builds [64–70].**
10. **[I] Reject speculative SIMD, zero-copy borrowed NumPy storage, copied COM examples, and hand-written DWM work until evidence requires them.**

## 2 Findings (with sources)

### 2.1 Local baseline and scope corrections

**[V, local]** Read `00-synthesis.md`, `01-windows-audio-api.md`, `02-cpal-vs-portaudio.md`, the relevant portions of `report-01-audio-io.md` and `report-03-rust-tauri.md`, and ADR 0001 under `E:/Impulcifer/docs/`. The earlier synthesis recommends PortAudio; the later two audio decisions and this task explicitly supersede it with **wasapi 0.24.0 on Windows and cpal 0.18.2 on macOS/Linux**. This report follows that decision. ADR 0001 remains applicable: no proposal here forces CTk through the WebView service.

**[V, local]** The current `core/recorder.py:514–519` starts capture on a thread; `:539` performs blocking playback; `:568` joins capture on the normal path. `:341–365` already configures per-direction WASAPI settings. Historical reports’ old fallback line numbers are not the current file. **[I]** Preserve the blocking measurement-session contract and independent streams, but add capture-ready acknowledgement and cleanup on every failure path. Do not copy the current incidental ordering into a soundness argument.

**[I]** There are three different claims to track:

| Claim | Meaning | What establishes it |
|---|---|---|
| No handwritten unsafe in an application crate | Maintainer-controlled Rust source uses no unsafe blocks/functions/impls/attributes | Compiler policy plus source/manifest checks |
| No unsafe in the compiled dependency graph | Includes macros, standard library, platform bindings, third-party code | Not a realistic target for this application |
| Sound safe interface | Safe callers cannot cause undefined behavior through the interface | Invariants, implementation review, appropriate tests; no single lint proves it |

### 2.2 Pattern catalogue: what mature projects actually do

| Project / inspected version | Verified pattern | Extracted rule / qualification |
|---|---|---|
| **[V] Tauri 2.11.5** | `tauri/src/lib.rs` has unsafe blocks and `unsafe impl<T> Send for UnsafeSend<T>`; no crate-root unsafe prohibition [1]. | **[I]** Application crates can forbid unsafe while using Tauri. Do not claim Tauri itself is unsafe-free. |
| **[V] tauri-runtime-wry 2.11.4** | Main-thread dispatch wraps native state; `WindowsStore`, `DispatcherMainThreadContext<T>`, and native-handle wrappers have unsafe trait implementations. Source checks thread identity or sends through the event-loop proxy [2]. | **[I]** Thread-affinity enforcement is the substance; the wrapper name and unsafe Send declaration are not proof. Never copy those impls without the dispatch invariant. |
| **[V] wry 0.55.0** | Windows WebView2 implementation makes unsafe COM calls behind normal WebView operations [3]. | **[I]** Own lifecycle and platform contracts below a safe API; callers should not manage controller pointers. |
| **[V] tao 0.35.0** | Safe window operations internally call Win32 functions such as `SetWindowTextW` and `IsIconic` [4]. | **[I]** Prefer its/Tauri’s existing window operations over new application-level FFI. |
| **[V] raw-window-handle 0.6.2** | Safe `HasWindowHandle` returns lifetime-bound `WindowHandle<'_>`; creating one from a raw handle is unsafe. Deprecated `HasRawWindowHandle` is an unsafe trait [7–9]. | **[I]** Borrow native handles for the duration of the operation; a copied integer handle is not ownership. |
| **[V] windows-core 0.62.0 / windows 0.62 family** | `Interface` is an unsafe trait; owning interface values implement reference-counted ownership. Direct IAudioClient methods require unsafe, but `Interface::cast`, `Clone`, and `Drop` hide COM calls behind safe operations [10–12]. | **[I]** Reuse owning interface types; separately prove apartment and buffer contracts. “Every COM call is an unsafe Rust call” is false. |
| **[V] wasapi 0.24.0** | Safe format, initialization, start/stop, capture/render APIs call windows-rs internally. `AudioClient` is `!Send + !Sync`; COM initialization helpers return HRESULT rather than an owning apartment guard [15–16]. | **[I]** A safe call surface still needs lifecycle review. Put apartment/session ownership in the adapter; do not move interfaces through Rayon/Tokio. |
| **[V] cpal 0.18.2** | Host modules implement backend-specific native work behind `Host`, `Device`, and `Stream` APIs and callback slices. Windows platform Device/Stream docs expose Send+Sync; current upgrade notes say streams are Send+Sync everywhere [19–21]. | **[I]** Rely on the selected backend’s reviewed safe boundary, not old claims that all CPAL streams are non-Send. Platform implementation remains trusted code. |
| **[V] PyO3 0.29.2** | Attachment token `Python<'py>`, lifetime-bound `Bound<'py,T>`, detached computation, owning `Py<T>`, safe extraction/conversion APIs; unsafe lives in interpreter/FFI and generated glue [22–26]. | **[I]** Do not use raw C API operations just because an example does. Owned Rust data crosses into workers. |
| **[V] rust-numpy 0.29.0** | Safe owned-array transfer/copy constructors; unsafe borrowed-memory constructors and unchecked array views [27–29]. | **[I]** Owned conversion avoids application unsafe, but an input copy can still race with external mutation. |
| **[V] RustFFT 6.4.1** | Safe planner selects SIMD or scalar implementations; feature-specific kernels contain unsafe intrinsics and target-feature declarations [30–32]. | **[I]** Reuse tested runtime dispatch. There is no initial justification for handwritten SIMD in Impulcifer. |

**[V] Version qualification.** Tauri 2.11.5’s runtime dependency family uses runtime-wry 2.11.4 with requirements `wry ^0.55.0` and `tao ^0.35.0`; upstream latest versions are not necessarily the resolved versions [5]. Inspected package releases include wasapi 0.24.0 (2026-08-12), cpal 0.18.2 (2026-08-16), PyO3 0.29.2 (2026-08-05), rust-numpy 0.29.0 (2026-09-05), and RustFFT 6.4.1 (2026-07-26). **[U]** No actual future Impulcifer Cargo.lock was resolved. Mutable Microsoft/standard-library documentation is identified as such in §6.

#### Tauri is useful precedent, not a blanket assurance

**[V]** runtime-wry justifies some unsafe thread traits with “we ensure this type is only used on the main thread” [2]. That is a claim about every access and teardown path, not just construction. Tauri 2.11.5 also retains deprecated safe `Manager::unmanage`, whose documentation says “This method is unsafe, since it can cause dangling references” and recommends mutable managed state such as `Mutex<Option<T>>` instead [1]. **[I]** Forbid use of `unmanage` in this app. A safe signature can contain an upstream soundness defect; do not treat the signature as an audit.

**[V]** Tauri `Window::set_theme(&self, Option<Theme>) -> Result<()>` is a safe desktop API; on macOS/Linux the theme affects the application [6]. **[I]** Use it before considering a DWM call. Its existence does not prove every desired titlebar appearance on every Windows release; reproduce a visual defect first.

#### Handle borrowing is narrower than owning a window

**[V]** `WindowHandle::borrow_raw` requires the relevant non-null/nonzero pointers to remain valid for the borrow. `WindowHandle` and `RawWindowHandle` are not Send/Sync; RawWindowHandle is Copy/Clone but does not own the native window. X11/web IDs have documented qualifications to the pointer-lifetime guarantee [7–9]. **[I]** Neither casting HWND to an integer nor cloning a COM controller extends the native window’s lifetime. Keep borrowed access on the window thread, prevent concurrent destruction, and do not store raw handles in a job object.

### 2.3 COM ownership, threading, and WASAPI packets

**[V]** `windows::core::Interface` is an unsafe trait with `Sized + Clone`, not `Send + Sync`, as supertraits [10].

| Operation | Verified ownership semantics [10–12] | Required application policy [I] |
|---|---|---|
| `as_raw(&self)` | Borrowed pointer; no reference increment or ownership transfer | Never feed it to an owning constructor without acquiring a reference |
| `into_raw(self)` | Transfers ownership of the held COM reference; prevents that wrapper’s Release | Avoid; otherwise record exactly which owner releases it |
| `unsafe from_raw(ptr)` | Takes an already-owned reference; does not AddRef | Private leaf only, with provenance and reference-count justification |
| `unsafe from_raw_borrowed(&ptr)` | Borrows a compatible live interface pointer | Keep lifetime local; no storage in queued work |
| `cast::<T>()` | Safe QueryInterface, returning an owned interface on success | Prefer to manual vtable calls |
| `Clone` / `Drop` | AddRef / Release internally | Prefer to handwritten reference counting; obey thread rules |

**[V]** COM apartment rules are separate from reference counts. STA objects normally require calls in their apartment; cross-apartment use requires the appropriate marshalling/proxy or documented agility. Successful CoInitializeEx calls must be balanced, including `S_FALSE`; `RPC_E_CHANGED_MODE` is a failure [13–14]. **[I]** Arc, Mutex, AddRef, and integer pointer casts do not marshal an object. Do not add unsafe Send/Sync to work around a compiler error. MTA initialization alone is not a permission slip to move a `!Send` Rust wrapper.

**[V]** WASAPI adds specific constraints: capture GetBuffer/ReleaseBuffer pairs execute on the same thread, packet storage is usable only until ReleaseBuffer, and the capture interface must be released on the thread that obtained it through GetService [17]. **[I]** These rules justify owning each stream and its interfaces entirely on one worker, even if a general COM explanation would permit more threading flexibility.

**[V]** wasapi 0.24.0 exposes safe `initialize_mta()` and `deinitialize()` without a RAII apartment type [15]. **[I]** Wrap initialization in private thread-confined ownership, count successful initialization correctly, and drop all native interfaces before the last apartment guard. The guard must be retained by every object that needs the apartment; a public temporary guard plus independently escaping session is insufficient.

**[V, source concern; not a confirmed vulnerability]** The pinned wasapi capture readers construct a slice for a nonempty packet before returning flags; they do not first branch on SILENT. Microsoft says to treat SILENT packets as silence and ignore actual data values [15,18]. **[U]** This study did not establish that the OS returns a null pointer in that case, or reproduce UB. **[I]** Review packet validity/initialization and SILENT handling before adoption. If slice construction itself violates the native contract, post-return checks in an outer adapter cannot fix it; patch or bypass the affected implementation.

**[V]** CPAL has a safe WASAPI `Device::immdevice()` accessor, but its running WASAPI Stream does not expose the internal audio clients. A default-device accessor can resolve the current endpoint rather than the endpoint of a previously opened stream [21]. **[I]** Do not reinterpret CPAL internals to reuse its COM state. Windows is already assigned to wasapi; no CPAL-to-WASAPI stream surgery is necessary.

### 2.4 Python and DSP: avoid creating more unsafe exceptions

**[V]** `Python<'py>` denotes interpreter attachment; on a GIL build that includes holding the GIL, but on a free-threaded build it does not confer exclusive access. `Python` and `Bound` are not Send/Sync. `Bound::unbind` produces owning `Py<T>`; accessing Python APIs still requires attachment. Long Rust-only work should detach on both interpreter models [22–25].

**[V]** PyO3 0.29.2 explicitly tests `#![forbid(unsafe_code, unsafe_op_in_unsafe_fn)]` with pyclass/pymodule/pyfunction macros [26]. The macros still generate unsafe code. **[I]** The Python bridge can meet the handwritten-code policy; this is not a claim that its expanded binary contains no unsafe. Compile the actual bridge/macros on the pinned compiler to catch expansion/lint regressions.

**[V]** Stable PyO3 approximates `Ungil` through Send; its documentation demonstrates that `SendWrapper` can defeat the intended attachment boundary using safe syntax. PyO3’s nightly-feature implementation is more precise [25]. **[I]** Ban `send_wrapper` and equivalent escape hatches in first-party dependencies. This is not a reason to move the shipping product to nightly: pass only owned Rust buffers/configuration to detach, never Python borrows.

| rust-numpy operation | Caller-authored unsafe? [V,27–28] | Policy [I] |
|---|---:|---|
| `IntoPyArray::into_pyarray` / `PyArray::from_owned_array` | No; consumes owned storage | Default output path |
| `PyArray::from_array` | No; copies | Acceptable when ownership transfer is inconvenient |
| `PyReadonlyArray::as_array()` followed by ndarray `.to_owned()` | No; checked Rust-side borrow then copy | Only with a credible no-concurrent-writer input contract |
| Direct `PyArrayMethods::as_array` / `as_slice` | Yes | Prohibited in the first-party bridge |
| `PyArray::borrow_from_array` | Yes; lifetime/reallocation/aliasing obligations | Prohibited initially; no evidence warrants it |

**[V]** rust-numpy guards coordinate rust-numpy borrows, not arbitrary Python/NumPy/native accesses. NumPy warns about concurrent read/write and resize hazards [28–29]. **[I]** Copying ends subsequent sharing but does not make a racing copy safe. Do not advertise unrestricted free-threaded ndarray input as memory-safe merely because there is no handwritten unsafe.

**[I] Recommended first Python boundary:** accept files, immutable Python `bytes` containing a validated little-endian f64 payload, or other independently owned serialized data; return newly owned arrays. For convenience ndarray APIs, define a separate caller-level concurrency contract and qualify/free-thread-test that surface before promising support. Reject or defer borrowed ndarray entry points when that contract cannot be enforced. Immutable bytes avoid a mutable NumPy allocation during Rust’s read, at the cost of explicit serialization/copying. Do not add a `sys-python` crate to hide the problem.

**[V]** RustFFT’s safe `FftPlanner<f64>` chooses AVX/SSE/Neon/WASM/scalar implementations as applicable; AVX construction checks AVX+FMA. SSE source checks CPU support before feature-specific construction [30–31]. `#[target_feature]` alone does not perform detection; compiler-enabled features can make `is_x86_feature_detected!` constant true [32]. **[I]** No shipping `target-cpu=native`, global forced AVX/FMA, application SIMD intrinsics, unchecked indexing, or transmute-based casts. Use scalar compatibility code plus the dependency’s safe planner. SIMD changes also require numerical golden tests; memory safety is not numerical parity.

### 2.5 Soundness rules for any approved wrapper

**[V]** Rustonomicon defines a sound unsafe implementation as one where “safe code cannot cause Undefined Behavior through it.” It also says the unsafe obligation depends on “the state established by otherwise ‘safe’ operations” and that privacy limits the relevant scope at the module boundary [33]. **[I]** Review the entire invariant-owning module and its safe mutators, not just the red lines containing `unsafe`.

| Rule [I] | Proof obligation / evidence |
|---|---|
| Write an invariant ledger before implementation | For each resource: origin, allocation/OS owner, extent, alignment, initialization, aliasing, permitted thread, valid states, finalizer. List all methods that can change these facts [33]. |
| Validate at the safe boundary | Channel/rate/layout limits, `checked_mul(frames, channels)`, checked byte counts, representable lengths, supported negotiated format, sufficient slice capacity. Invalid safe input returns Err or a documented panic, never UB. |
| Use private owned types | Prefer windows-rs owning interfaces. Where raw ownership is unavoidable, private NonNull/newtype fields with a correct Drop; no public pointer fields, Deref-to-raw, public unsafe constructors, or unsafe traits [10–11,33]. |
| Distinguish call contract from proof site | An internal `unsafe fn` documents caller obligations under `# Safety`. Each actual unsafe operation has its own explicit unsafe block and a local `// SAFETY:` argument. `unsafe fn` does not discharge its body’s obligations [64]. |
| Comments must prove something | State which checked length, retained owner, state transition, and thread make this particular call valid. “Windows API”, “same as example”, and “pointer is valid” are not sufficient arguments. |
| No public repair obligations | If ordinary safe callers can break the invariant by calling in the wrong order, leaking, closing, cloning, or using invalid input, fix the API. Private state machines/typestates enforce transitions. |
| Do not assume Drop always runs | `mem::forget` is safe. Rustonomicon: “unsafe code cannot rely on destructors to be run in order to be safe” [34]. Forgotten values may leak resources, but must not leave active callbacks pointing into freed memory. |
| Model panic and partial construction | Acquisition guards release exactly once; partially opened streams clean up successfully acquired resources only; no panic unwinds through an incompatible foreign callback boundary. Prefer owned worker state and no custom native callbacks where blocking polling suffices. |
| Prove Send and Sync independently | Both are unsafe traits with different promises [35]. Initial leaf policy permits neither manual impl. A private `PhantomData<Rc<()>>` can conservatively prevent automatic Send/Sync without unstable negative impls. |
| Keep foreign borrows internal | Acquire packet, validate flags/counts, copy/decode or zero-fill, release in the same operation. No returned slice points into a driver packet; no application callback runs while such a borrow exists. |
| Protect teardown order | Stop use, unregister callbacks if any, release stream services/client/endpoint, then release apartment ownership on the same thread. A join timeout cannot authorize freeing memory still used by a worker. |
| Keep raw code small, not artificially one-line | Small blocks aid audit, but the state machine and safe validation remain part of the soundness review. Complexity and public states matter more than keyword count. |

### 2.6 Enforcement tools: verified status and realistic role

| Tool / observed version | Verified capabilities and status | Decision [I] |
|---|---|---|
| **[V] rustc `unsafe_code`** | Rejects unsafe constructs and potentially unsound attributes including no_mangle/export_name/link_section. `forbid` prevents ordinary nested allow, but lint caps/force-warn can alter invocation-level behavior [36–37]. | Required; control the invocation and inspect configuration. Not transitive to dependencies. |
| **[V] Cargo workspace lints** | Since Cargo 1.74; every member must opt in with `[lints] workspace = true` [38]. | Required plus metadata checks; workspace membership alone does not inherit policy. |
| **[V] Clippy** | `undocumented_unsafe_blocks` and `multiple_unsafe_ops_per_block` are restriction lints, off by default. `missing_safety_doc` is style, normally warn. Macro expansions have exclusions; comments are not semantically proved [39–41,76]. | Explicit deny for all three in the leaf. Apply missing-safety documentation to private unsafe helpers by project review too. |
| **[V] cargo-deny 0.20.2**, 2026-07-09 | Advisories/licenses/bans/sources; current package MSRV 1.88.0 [46–49]. | Mandatory dependency-policy gate. Has recent release evidence; not a code soundness scanner. |
| **[V] cargo-audit 0.22.2**, 2026-06-05 | RustSec advisory checking; binary auditing also exists [50–51]. | Weekly and release audit. Overlaps cargo-deny; useful standalone artifact/report, not independent source review. |
| **[V] cargo-vet 0.10.2**, 2026-01-13 | Tracks full/delta audits, criteria, imports, exemptions, and publisher trust [42–45,75]. | Preferred review ledger/gate. Use custom unsafe review criterion for critical dependencies; human approval required for new exemptions. |
| **[V] cargo-crev 0.27.1**, 2026-04-12 | Signed reviews and web-of-trust; actual verify command is `cargo crev crate verify` [57–58]. | Optional evidence source, not a second mandatory ledger. Signed opinion is not a memory-safety proof. |
| **[V] cargo-geiger 0.13.0**, 2025-08-31 | Unsafe usage statistics; changelog fixes Rust 1.89 compilation. Source uses syntactic parsing/build-associated source information, not a whole-program proof [52–56,74]. | Advisory inventory/diff only. **[U]** Successful build and correct counting on 1.98.1 untested; maintenance cadence not established. |
| **[V] Miri, nightly component** | Detects several UB classes/races in executed Rust paths; most FFI/platform APIs unsupported, Windows has less system support than Linux [59]. | Required selected pure/model tests; cannot execute/verify real WASAPI or WebView2 COM integration. |
| **[V] ASan/TSan, nightly `-Zsanitizer`** | Instrument executed code; Linux x86_64 supported. Instrumenting std recommended; TSan synchronization visibility/atomics limitations remain [60]. | Separate periodic jobs on selected pure/worker targets, not the whole desktop process. |
| **[V] Kani** | Bit-precise harness-based verification including unsafe use cases; `cargo kani --harness <name>` and bounded unwinding documented [61–62]. | Optional integer length/routing/state helpers with small bounds. Do not promise entire f64 DSP or COM proofs. |
| **[V] Verus** | Specification-driven verification of a supported Rust subset, with support for some raw-pointer reasoning; active development/incomplete coverage [63]. | No initial gate. Solo-maintainer cost is unjustified absent a small invariant that tests/review cannot handle. |

**[V] Important geiger installation warning:** open issue #564 reports `cargo install --locked` selecting yanked `crossbeam-channel 0.5.14`, affected by RUSTSEC-2025-0024; it links a proposed fix, not a verified release [54]. **[I]** Do not blindly install that locked toolchain into privileged release jobs. Use an independently reviewed fixed tool lock/binary in a no-secrets inventory job, or report geiger unavailable. Do not “fix” it by unpinning everything and ignoring advisories. The report does **not** claim installation necessarily fails.

**[I] Tool distinction:** deny/audit answer known-advisory/policy questions; vet/crev record human trust/review; geiger helps locate unsafe; Clippy checks syntax/documentation patterns; Miri/sanitizers test paths; Kani/Verus prove specified properties within supported models. None audits Windows drivers or WebView2 itself.

### 2.7 Edition, compiler, and nightly policy

| Item | Verified behavior | Proposed policy [I] |
|---|---|---|
| **[V] Edition 2024** | Stable since Rust 1.85.0 [68]. | Use edition 2024; this is an edition floor, not the actual dependency MSRV. |
| **[V] `unsafe_op_in_unsafe_fn`** | Warn by default in edition 2024 [64]. | Deny explicitly, including the leaf; no reliance on edition warning defaults. |
| **[V] Foreign declarations** | `unsafe extern` block required; individual items can be explicitly safe if their caller contract permits it [65]. | No first-party extern blocks outside the leaf. Do not relabel a pointer-taking foreign function safe merely to silence callers. |
| **[V] Unsafe attributes** | `#[unsafe(no_mangle)]`, `#[unsafe(export_name = ...)]`, `#[unsafe(link_section = ...)]` required in 2024 [66]. | Forbid handwritten instances outside leaf; PyO3 macro expansions are separately reviewed dependencies. |
| **[V] Newly unsafe std functions** | `set_var`, `remove_var`, Unix `before_exec` become unsafe in 2024 [67]. | Read environment; keep runtime configuration in Rust objects; set child environment through safe `Command::env`/`env_remove`. Do not open another exception for process-global mutations. |
| **[V] Rust 1.98.1** | Published 2026-09-03, fixes 1.98.0 vtable-generation miscompilation potentially causing UB [69]. | Pin 1.98.1 for the first candidate, not 1.98.0. Re-run all gates on compiler updates. |
| **[V] `-Zbuild-std`** | Needs nightly Cargo/rustc and rust-src; rebuilds std [70]. | No use for normal release builds. Useful deliberately in sanitizer jobs, despite being unnecessary for ordinary compilation. |
| **[V] Miri** | `cargo +nightly miri test`; interpreter options go through MIRIFLAGS [59]. | Pin a dated nightly after a successful pilot. There is no generic stable `cargo -Zmiri` substitute. |

**[I] MSRV policy:** initially declare `rust-version = "1.98"` for first-party packages and require shipping/CI toolchain 1.98.1. This avoids an untested historical support promise while not treating the edition floor as dependency compatibility. If an older source-build MSRV matters, determine the full resolved graph’s requirements, test that exact compiler separately, and only then lower the declared floor. Update MSRV deliberately in release notes; independent audit tools may have their own compiler requirement. Maintain stable shipping and dated-nightly analysis pins separately. **[U]** Actual minimum for the final Tauri/PyO3/wasapi graph remains untested.

## 3 Proposed architecture

All of §3 is **[I] proposed design**, except explicitly cited compiler/tool behavior. No repository implementation or workflow was changed.

### 3.1 Workspace and unsafe budget

A budget is permission for named operations after approval, not a target number to consume. Start with no first-party unsafe at all. Instantiate or relax the one reserved leaf only if the inventory proves a needed operation lacks a sound safe dependency API.

| Crate / target | Responsibility / dependencies | First-party unsafe budget |
|---|---|---|
| `impulcifer-types` | Checked configuration, channel/layout types, errors; serde | **0; forbid** |
| `impulcifer-dsp` | f64 SciPy-compatible primitives, RustFFT/RealFFT, ndarray/nalgebra, Rayon | **0; forbid**; no home-grown SIMD |
| `impulcifer-io` | WAV/CSV/TXT, ffmpeg process management, safe byte parsing | **0; forbid** |
| `impulcifer-analysis` | Analysis arrays, plotters PNG, offline Plotly assets | **0; forbid** |
| `impulcifer-audio-io` | Backend selection, measurement orchestration, owned PCM, readiness/drain/cancel; cpal on macOS/Linux | **0; forbid** |
| `impulcifer-sys-win` | Private Windows endpoint/session implementation using wasapi; optional direct windows-rs operations | **Only permitted leaf**; initial allowance **0**, additions individually approved; hard review tripwire **12 blocks / 120 lines inside unsafe blocks** |
| `impulcifer-jobs` | seq journal, job lifecycle, cooperative cancellation, bounded work queues | **0; forbid** |
| `impulcifer-service` | JSON-safe application operations, settings, validation | **0; forbid** |
| `impulcifer-app` | Tauri commands, dialogs/theme/opener, updater adapters | **0; forbid**, including build.rs and targets |
| `impulcifer-cli` | Headless CLI over the same core/service | **0; forbid** |
| `impulcifer-python` | PyO3/maturin, immutable/owned input boundary, owned NumPy output | **0 handwritten; forbid**, PyO3-generated glue reviewed separately |
| `impulcifer-policy` | Safe Rust CI checker using Cargo metadata + syntax/TOML parsing | **0; forbid** |
| tests/examples/benches/build scripts | Same policy as owning crate; root attributes on each crate target | No separate exception |

The tripwire is an initial scope constraint, **not a soundness metric or a statement that 12 blocks are needed**. In the leaf: **0 public unsafe functions, 0 handwritten unsafe Send/Sync impls, 0 static mut, 0 transmute, 0 SIMD/assembly, 0 exported raw pointers**. Required generated bindings remain the `windows` dependency, not checked-in binding code. Exceeding the tripwire or needing another category requires a maintainer-approved ADR before implementation.

Do not create one universal `unsafe-utils` crate. If future Python/SIMD unsafe is truly indispensable, reject that patch under this policy and explicitly decide a new domain-specific leaf; do not put Python or SIMD into the Windows crate to game the “one leaf” rule.

```text
existing vanilla UI + JSON catalogues
                 |
         impulcifer-app (Tauri)
                 |
        impulcifer-service <---------- impulcifer-cli
                 |
          impulcifer-jobs
           /           \
impulcifer-dsp      impulcifer-audio-io
     ^              /              \
     |      [Windows only]      [macOS/Linux only]
impulcifer-python   impulcifer-sys-win       cpal
     |                 |
 owned bridge         wasapi ---> windows/windows-core
                       |
             approved missing native operations only

io + analysis + types: shared safe crates; no Tauri/Python dependency
app ---> Tauri/runtime-wry/wry/tao (upstream native trust boundary)
app ---> Velopack (Windows), Tauri updater (macOS/Linux)
policy checker: build-time only, no application dependency
```

All arrows are dependency/call direction, not a process isolation boundary. The Windows leaf depends on small common types if needed, never on audio-io/service/app. DSP and Python do not depend on the Windows leaf. The headless CLI need not link Tauri; the DSP wheel need not include native recording backends unless that becomes an explicit Python requirement.

### 3.2 Windows leaf safe API sketch

Illustrative Rust declarations, **not compiled**. Private fields and validated constructors matter more than exact spelling. Error types own their diagnostic data. No HWND, COM interface, native pointer, unsafe trait, or callback taking a raw buffer crosses this boundary.

```rust
// impulcifer-sys-win/src/lib.rs
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::multiple_unsafe_ops_per_block)]
#![deny(clippy::missing_safety_doc)]

pub struct EndpointId { /* owned stable identifier; private */ }
pub struct Endpoint { /* owned metadata; private */ }
pub enum Direction { Input, Output }
pub enum ShareMode { Exclusive, SharedAutoConvert }
pub struct StreamSpec { /* checked rate, channels, routing, format */ }
pub struct PlaybackBuffer { /* checked interleaved Vec<f32> */ }
pub struct CancelToken { /* cloneable Arc<AtomicBool>; private */ }
pub struct PlaybackReport { /* submitted/drained frames, mode, xruns */ }
pub struct CaptureRead { /* frame count, discontinuity, timestamps */ }
pub struct Error { /* stable kind and owned details */ }

// Both are intentionally !Send and !Sync; all native fields are private.
pub struct OutputSession { /* interfaces + retained apartment owner */ }
pub struct CaptureSession { /* interfaces + retained apartment owner */ }

pub fn enumerate(direction: Direction) -> Result<Vec<Endpoint>, Error>;
pub fn open_output(id: &EndpointId, spec: StreamSpec)
    -> Result<OutputSession, Error>;
pub fn open_capture(id: &EndpointId, spec: StreamSpec)
    -> Result<CaptureSession, Error>;

impl OutputSession {
    pub fn play_to_completion(
        &mut self,
        buffer: PlaybackBuffer,
        cancel: &CancelToken,
    ) -> Result<PlaybackReport, Error>;
}
impl CaptureSession {
    // Returns only after capture has actually started, not just queued.
    pub fn start(&mut self) -> Result<(), Error>;
    // Returns initialized interleaved f32 samples in caller-owned storage.
    // Checks capacity/alignment in frames; timeout is a typed result/error.
    pub fn read_into(
        &mut self,
        dst: &mut [f32],
        cancel: &CancelToken,
    ) -> Result<CaptureRead, Error>;
    pub fn stop(&mut self) -> Result<(), Error>;
}
```

**Ownership implementation rules.**

- Open/create on the actual worker, not the UI thread followed by a send. An internal thread-local weak reference to a private `Rc<Apartment>` can share one initialized apartment across sessions on that thread. Each session retains a strong reference. Constructors return owned metadata only from enumeration.
- Track successful COM initialization and balance it even for S_FALSE. Never call deinitialize after an initialization failure. Native interfaces must be dropped before the session releases its apartment ownership. Use explicit `Option::take` cleanup or audited field order, not an undocumented accident.
- Do not export the guard or deinitialize function. Leaking a session must leak its necessary apartment/native ownership, not terminate COM underneath another live object. Miri/model tests cover ownership bookkeeping; native tests cover actual COM behavior.
- Use windows-rs RAII interface types; a custom pointer newtype is justified only where they cannot represent ownership. `Drop` must not panic. A foreign callback, if introduced later, requires registration lifetime and in-flight-call synchronization review.
- Safe numeric conversion is adequate here: use validated `to_le_bytes`/`from_le_bytes` or a reviewed safe cast dependency, not application `from_raw_parts` on `Vec<u8>`. `Vec<u8>` does not promise f32 alignment. Checked lengths and negotiated block alignment must agree.
- A packet guard is private and never returned. For SILENT, fill zero without reading unspecified payload. Check discontinuity/device invalidation and report them; do not treat “all samples submitted” as “drained”. Cancellation stops the session and reports cancellation rather than a successful drain.
- Even a zero-byte slice has Rust pointer validity/alignment rules. Branch on empty/silent cases before constructing any raw slice in an approved native fallback.

**Two-stream orchestration in the safe audio crate.**

1. Capture worker resolves its endpoint and opens/starts capture; only then sends Ready or Error.
2. Playback worker creates its own output session on its own thread, submits the owned buffer, and waits for the selected backend’s drain condition.
3. Coordinator requests capture stop after the specified tail, joins both workers, and returns the measurement result. Errors and cancellation take the same cleanup path.
4. Progress snapshots contain owned metadata/counters. Driver buffers and Python references never appear in event messages.
5. All waits check cancellation and device loss. If a driver is stuck, do not free state still used by its worker or kill a Rust thread. A leaked/quarantined worker or process-level recovery may be necessary; document that operational failure separately from memory safety.

Use wasapi’s safe operations wherever valid. A missing operation involving existing AudioClient state must be implemented where that state is owned, potentially by a reviewed upstream patch/vendor fork. Do not invent a leaf `extend_stream(raw_pointer)` escape that forces audio-io to extract private pointers.

**DWM exception policy.** Initially no DWM API in this leaf: use `Window::set_theme`. If a reproduced requirement later needs one, keep it a synchronous safe operation over a borrow tied to a live window, invoked by the app on its UI thread with destruction excluded. Never expose `set_dark_titlebar(hwnd: usize, ...)` as a supposedly safe public API; arbitrary integers cannot guarantee a live owned window or correct dispatch. If that proof needs a Tauri-specific adapter, revise the architecture rather than making the Windows leaf depend on the app and creating a cycle.

### 3.3 Lint configuration and tamper-resistant checks

**[V]** Workspace lints require member opt-in and apply to that package, not its dependencies [38]. **[I]** Use a workspace default, redundantly put the attribute in every safe crate root, and let the sole leaf explicitly define its own policy.

```toml
# Workspace Cargo.toml, proposed
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
edition = "2024"
rust-version = "1.98"

[workspace.lints.rust]
unsafe_code = "forbid"
unsafe_op_in_unsafe_fn = "deny"

[workspace.lints.clippy]
undocumented_unsafe_blocks = "deny"
multiple_unsafe_ops_per_block = "deny"
missing_safety_doc = "deny"
```

```toml
# Each ordinary member
[lints]
workspace = true
```

```rust
// Every ordinary lib.rs/main.rs/build.rs/test/example crate root
#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]
```

```toml
# Sole approved leaf's Cargo.toml; deliberately does not inherit forbid
[lints.rust]
unsafe_op_in_unsafe_fn = "deny"
[lints.clippy]
undocumented_unsafe_blocks = "deny"
multiple_unsafe_ops_per_block = "deny"
missing_safety_doc = "deny"
```

The leaf’s root still forbids unsafe until an approved exception removes that attribute. `unsafe_code = "deny"` globally with local allow is weaker than the proposed forbid policy; do not use it for ordinary members. Lint-table priorities order groups versus specific lints but cannot authorize weakening inherited forbid [37–38].

**Protected policy checker specification.** `impulcifer-policy` must be implemented before native work, not assumed to exist. Its proposed `check` command does the following:

- Uses `cargo metadata` to enumerate workspace packages/targets; checks every lib/bin/build/test/example/bench root, including explicit nonstandard target paths.
- Requires ordinary members to inherit lints and root attributes; exactly one named Windows leaf may opt out. A new crate is zero-unsafe by default, including internal helper tools and path dependencies.
- Parses first-party Rust/TOML rather than relying on a text grep. Records unsafe blocks/functions/impls/extern blocks/unsafe attributes, source path, reason ID, and budget; rejects public unsafe or unapproved categories in the leaf. Comment-only occurrences do not count.
- Checks platform/feature-gated source too. Compiler checks execute only active configurations; the source policy scan catches forbidden syntax hidden under `cfg(any())` or another target.
- Audits all first-party `.cargo/config*`, build scripts, manifests and CI environment for lint caps, force-warn overrides, compiler wrappers, generated includes, proc-macro additions, arbitrary rustc flags, and bypassed target roots. No trusted build script downloads/generates new native code without approval.
- Resolves direct dependency ownership: only the leaf may directly use windows/windows-core/wasapi; only audio-io may directly use cpal; only python may use PyO3/numpy; only app may use Tauri/Velopack. Upstream transitive use is allowed and audited separately.
- Rejects first-party `send_wrapper`, raw-pointer transport helpers, global target-cpu=native/forced SIMD release flags, and the deprecated Tauri unmanage API. No all-features run that accidentally enables ASIO.
- Tests itself with fixtures that deliberately remove inheritance, add forbidden unsafe to build.rs, hide it behind cfg, change lint flags, add a second exception crate, and exceed the leaf budget. These fixtures are data, not runnable application crates.

**Limits and governance.** rustc documents that `--cap-lints` can lower even forbid and `--force-warn` has special precedence [37]. External macro expansion has lint-hygiene exclusions; a handwritten-unsafe lint is not complete expanded-code accounting [26,39–41]. Use approved pinned macro dependencies and inspect expansion changes when updating them. Do not promise a source parser can prove generated code safe.

The check’s source/config, workflows, toolchain files, manifests, audit ledger, exceptions, and leaf changes require maintainer review. Branch protection must require policy success, disallow unattended direct pushes/bypass, and keep signing/publishing credentials out of PR jobs. An AI agent can edit a checker it is allowed to edit; CI alone cannot protect against a maintainer approving its removal. A solo maintainer supplies the approval; an independent AI review is useful evidence, not a substitute approver.

### 3.4 Exact proposed CI gate list

These are **[I] commands for the proposed workspace**, not executed verification. Install OS libraries and supported Python required by Tauri/cpal/PyO3 first; GUI/native builds run on their own OS. Use approved feature lists per platform, not indiscriminate `--all-features`. Here `desktop` is a proposed service/app feature set, and `scalar` a proposed DSP feature disabling SIMD-dependent test execution; create/test those features before adopting the commands.

**Shared pins and tooling setup.** Initial shipping toolchain is 1.98.1. `$NIGHTLY` below must be an exact dated nightly selected by a successful Miri/sanitizer pilot, not a floating value; no known-working date was established here. All shell snippets below use Bash (also available on GitHub Windows runners). Verify/cache tool binaries by version and checksum; source installation commands are shown for reproducibility, not an instruction to rebuild them in every job.

```sh
rustup toolchain install 1.98.1 --profile minimal --component rustfmt --component clippy
cargo +1.98.1 install --locked cargo-deny --version 0.20.2
cargo +1.98.1 install --locked cargo-audit --version 0.22.2
cargo +1.98.1 install --locked cargo-vet --version 0.10.2
```

| Job / frequency | Commands | Rough wall time [I], not measured |
|---|---|---:|
| **policy-format**, every PR, Linux | `cargo +1.98.1 fmt --all -- --check`; `cargo +1.98.1 run --locked -p impulcifer-policy -- check` | 1–3 min warm; checker first build extra |
| **stable-native**, every PR, Windows/macOS/Linux | `cargo +1.98.1 clippy --locked --workspace --all-targets -- --no-deps -D warnings`; `cargo +1.98.1 test --locked --workspace --all-targets`; `cargo +1.98.1 test --locked --workspace --doc` | 3–10 min warm per OS; 10–30+ cold |
| **feature-contracts**, every PR, each relevant OS | `cargo +1.98.1 check --locked -p impulcifer-dsp --no-default-features`; `cargo +1.98.1 check --locked -p impulcifer-cli --no-default-features`; `cargo +1.98.1 check --locked -p impulcifer-app --features desktop`; `cargo +1.98.1 test --locked -p impulcifer-dsp --features scalar` | 2–8 min warm |
| **dependency-policy**, every PR + daily schedule | `cargo deny --locked check advisories bans licenses sources`; `cargo vet check --locked` | 1–4 min after tool setup |
| **dependency-refresh**, scheduled trusted review job | `cargo vet check`; `cargo vet suggest`; `cargo audit --file Cargo.lock` | 1–4 min, human audit excluded |
| **windows-leaf**, every PR touching leaf/audio/dependency/compiler | `cargo +1.98.1 test --locked -p impulcifer-sys-win --all-targets`; `cargo +1.98.1 test --locked -p impulcifer-audio-io --all-targets` | 1–5 min warm |
| **miri-models**, every PR touching DSP/state/leaf models; weekly otherwise | Dated nightly setup and commands below | 5–20 min for deliberately small fixtures |
| **asan-core**, weekly + native-memory changes | Separate ASan command below | 10–25 min cold; 3–10 warm |
| **tsan-jobs**, weekly + concurrency changes | Separate TSan command below | 10–25 min cold; 3–10 warm |
| **unsafe-inventory**, dependency/leaf PRs + weekly, advisory | Reviewed geiger binary commands below; archive JSON and compare to approved baseline | 2–10+ min; tool/build failure must be reported |
| **python-boundary**, Python/dependency/compiler PRs, selected regular + free-threaded interpreters | `python -m maturin build --locked --manifest-path crates/impulcifer-python/Cargo.toml --out dist`; `python -m pip install --no-deps --force-reinstall dist/*.whl`; `python -m pytest tests/python_boundary -q` | 3–12 min per interpreter/OS warm |
| **numerical-goldens**, every DSP/compiler/dependency PR | `cargo +1.98.1 test --locked -p impulcifer-dsp --test golden_parity --release`; `cargo +1.98.1 test --locked -p impulcifer-io --test wav_contract` | 2–10 min warm, corpus-dependent |
| **hardware-release**, controlled Windows/macOS device runner or signed maintainer acceptance | `cargo +1.98.1 test --locked -p impulcifer-audio-io --test hardware_acceptance -- --ignored --test-threads=1` | 10–30+ min plus setup |
| **packaged-smoke**, release candidate, each OS | Existing release-specific installer/launch/update tests, expanded for Rust shell and leaf; no generic honest one-command substitute | Measure prototype; not included in estimates above |

`desktop`, `scalar`, `golden_parity`, `wav_contract`, `hardware_acceptance`, `tests/python_boundary`, and `impulcifer-policy` are **new specified fixtures/features to implement**, not existing files. Unit tests must not unexpectedly access microphones; real-device tests are ignored by default and invoked only in the controlled hardware job. PyO3 extension linking differs from embedding/test linking: keep extension-module build configuration separate if necessary, and exercise both rather than forcing an invalid all-features matrix. Use one clean `dist` per wheel job so the install command selects one wheel. Pip/maturin/test dependencies require their own pinned tooling environment.

**Miri setup and exact interpreter invocation [I, based on 59].**

```sh
rustup toolchain install "$NIGHTLY" --profile minimal --component miri --component rust-src
cargo +"$NIGHTLY" miri setup
MIRIFLAGS="-Zmiri-many-seeds=0..8" \
  cargo +"$NIGHTLY" miri test --locked -p impulcifer-jobs --lib
cargo +"$NIGHTLY" miri test --locked -p impulcifer-dsp --no-default-features --features scalar --lib
cargo +"$NIGHTLY" miri test --locked -p impulcifer-sys-win --lib --features model-tests
```

The leaf’s proposed `model-tests` feature builds platform-independent state/length/apartment-accounting models with a fake foreign backend on Linux. Native Windows modules remain cfg-gated out. Passing these tests is **not** passing the COM implementation. Keep fixtures tiny and no huge FFTs. Miri checks exercised executions and seeds, not all schedules. Do not disable isolation/checks simply to make desktop/FFI tests pass. Its experimental native-library support on Unix does not turn it into a WASAPI verifier [59].

**Sanitizers [I, based on 60,70].** Use independent target directories; rebuild std intentionally. Explicit target prevents instrumentation flags leaking into host build scripts/proc macros.

```sh
rustup toolchain install "$NIGHTLY" --profile minimal --component rust-src
CARGO_TARGET_DIR=target/asan \
RUSTFLAGS="-Zsanitizer=address" RUSTDOCFLAGS="-Zsanitizer=address" \
  cargo +"$NIGHTLY" test --locked -Zbuild-std \
  --target x86_64-unknown-linux-gnu \
  -p impulcifer-dsp -p impulcifer-io --lib

CARGO_TARGET_DIR=target/tsan \
RUSTFLAGS="-Zsanitizer=thread" RUSTDOCFLAGS="-Zsanitizer=thread" \
  cargo +"$NIGHTLY" test --locked -Zbuild-std \
  --target x86_64-unknown-linux-gnu -p impulcifer-jobs --lib
```

Provide llvm-symbolizer. Do not combine ASan/TSan in one binary. `-Zbuild-std` instruments Rust std, not every native library. TSan requires visibility into synchronization and has documented limitations around atomic fences/assembly [60]. Test the actual Rayon/queue dependency combination before promoting this job to a required gate. Neither Linux command instruments Windows COM. Windows OS/runtime verification and optional native diagnostic tooling are complementary, not replaced by these jobs.

**Geiger [I, based on 52–56,74].** After qualifying a fixed tool binary independently:

```sh
cargo geiger --locked --all-targets --output-format Json > unsafe-inventory.json
cargo geiger --locked --forbid-only --output-format Json > forbid-inventory.json
```

Here geiger’s `--all-targets` concerns target dependency inclusion, not proof that every target-specific body or macro expansion was compiled. Avoid `--all-features`: it may add unwanted ASIO and change the trust surface. Run resolved/platform-specific inventories as appropriate, record enabled features and tool version, and retain the policy checker as the authoritative first-party gate. An inventory failure must show `unavailable/failed`, not an empty green report.

**Optional Kani proof pilot [I,61–62].**

```sh
cargo kani --harness checked_frame_bytes --default-unwind 32
```

Pin/install Kani separately after selecting a release; no current working Kani version was verified. Bound frames/channels and prove arithmetic/offset/state properties, retain unwinding assertions, and distinguish assumptions from conclusions. A passing proof with a fake COM backend establishes the model property only. No initial Verus gate: proof annotations and solver maintenance are additional implementation work, not free safety.

**Gate policy.** Standard lints/tests/dependency-policy and relevant boundary tests are required. Hardware acceptance is mandatory before publishing a changed audio backend, not on arbitrary untrusted PR hardware access. Miri/sanitizer infrastructure outages are visible and require an explicit temporary waiver with owner/expiry, not silent continue-on-error. Geiger and optional formal proofs are advisory until qualified. Cold compiler/tool installs are not included in warm estimates. These times are planning ranges, not performance claims.

**Release sequencing.** Preserve `publish.yml` and its PyPI OIDC identity from the existing project. Integrate the new required checks before publishing wheels; keep desktop distribution gated on the established release decision. Do not publish a wheel and only then discover an unsafe-policy failure. CI design and approval remain maintainer-owned.

### 3.5 Dependency selection, audit, and update workflow

1. **Start from required semantics.** Prefer a maintained safe API with bounded, reviewed unsafe over a zero-unsafe crate that cannot implement the contract. `forbid` badges and geiger counts are selection signals, not sufficient evidence; neither accounts for all transitive/proc-macro/native behavior [36,52–56].
2. **Record provenance.** Commit Cargo.lock; pin git dependencies by full revision and use registries with checksums. List target/features and native system libraries. Audit release artifacts’ actual graph, build dependencies, and proc macros. All these can affect the product even if application source has zero unsafe.
3. **Prioritize sensitive dependencies.** Review wasapi packet/lifecycle routines, cpal’s selected hosts, windows-core ownership, PyO3/rust-numpy boundary operations, FFT dispatch, Tauri runtime state/thread logic, and updater code. Review current RustSec history and fixes; absence of an advisory is not an audit. The known geiger-tool advisory is a reminder that CI tools also have dependencies [54].
4. **Use cargo-vet as the record.** Import only explicitly trusted audits; inspect new packages and deltas, then certify after review. Example commands: `cargo vet inspect wasapi 0.24.0`, `cargo vet diff wasapi OLD NEW`, `cargo vet certify wasapi 0.24.0`. OLD/NEW are reviewed versions, not literal CLI values [44]. An agent may prepare findings but may not certify or change exemptions without approval.
5. **Add a strict custom criterion for critical unsafe.** Built-in safe-to-deploy says ideally all unsafe is sound but permits auditor judgment; do not misrepresent it as the project goal [42]. A proposed criterion below requires review of all reachable unsafe and relevant safe invariant-maintaining code for the declared platform/features, documented limitations, and no known unresolved soundness hole in that scope. Explicitly list scope in audit notes; reassess when enabling features.
6. **Use exemptions honestly.** Initial vet onboarding/exemptions can make coverage appear complete without source review. Do not exempt the critical audio/Python unsafe surface merely to get a green dashboard. For unavoidable wider-framework exemptions, record unresolved scope, owner, and a review deadline in the project ledger. Do not claim native dependencies have been completely audited.
7. **Do not replace audits with publisher trust.** Publisher trust/wildcards authorize future releases under assumptions; the publisher need not inspect every contribution [45]. Do not apply them to the custom soundness criterion. cargo-crev may supply useful outside reviews; record which identity and scope were trusted rather than blindly importing popularity [58].
8. **Upgrade in small reviewed changes.** Pin the old/new graph, inspect unsafe and safe invariant changes, refresh advisory/audit records, rerun platform/macros/goldens/hardware tests as applicable. Compiler updates also rerun numerical and boundary gates. Never auto-merge native dependency bumps because SemVer says patch.
9. **If a needed crate has unaudited unsafe:** first review the relevant implementation and upstream documentation; prefer an upstream fix. A temporary vendored fork must preserve provenance/license, carry a minimal patch and tests, and have an update/removal plan. Treat vendored code as first-party-reviewed even if its source is outside `crates/`.
10. **If the safe API is actually unsound, a safe façade alone cannot repair UB that occurs inside it.** Patch/bypass the offending operation or reject/defer the dependency. Writing a wrapper ourselves means taking responsibility for the native contract, not sprinkling checks after the dangerous call. A crate boundary offers no memory isolation; if crash containment is required, use a separately designed process boundary.

Proposed audit configuration, based on cargo-vet’s documented syntax [43]; package policy must be completed against the actual resolved graph:

```toml
# supply-chain/audits.toml
[criteria.impulcifer-soundness-reviewed]
description = "Reviewed reachable unsafe and invariant-maintaining safe code for the explicitly recorded targets/features; no known unresolved soundness defect in that scope."
implies = "safe-to-deploy"
```

```toml
# supply-chain/config.toml (illustrative direct-dependency requirements)
[policy.impulcifer-sys-win]
dependency-criteria = { wasapi = "impulcifer-soundness-reviewed", windows = "impulcifer-soundness-reviewed", windows-core = "impulcifer-soundness-reviewed" }

[policy.impulcifer-python]
dependency-criteria = { pyo3 = "impulcifer-soundness-reviewed", numpy = "impulcifer-soundness-reviewed" }
```

Do not stamp this criterion after reading only one method while leaving reachable operations unexamined. Large framework reviews may exceed a solo maintainer’s capacity; distinguish imported audited coverage from a documented risk acceptance instead of lowering the definition. Add explicit policies for CPAL/RustFFT and the selected Tauri graph after deciding tractable review scope.

**[V]** cargo-deny’s bans `wrappers` setting constrains direct dependents of a banned crate; it is not arbitrary workspace-root path analysis [48]. **[I]** Use it where expressible, but do not globally ban windows-rs and accidentally reject Tauri. The project metadata checker enforces first-party ownership of platform dependencies.

### 3.6 One-page unsafe policy for CONTRIBUTING / CLAUDE.md

The following is **[I] proposed paste-ready policy**.

> ## Unsafe Rust policy
>
> **Goal.** All application code is safe Rust. `impulcifer-sys-win` is the only crate eligible for a handwritten unsafe exception; it starts with zero approved operations. This does not claim dependencies, macros, the standard library, or the OS contain no unsafe.
>
> **Default.** Every other library, binary, build script, test, example, and helper uses `#![forbid(unsafe_code)]`, denies `unsafe_op_in_unsafe_fn`, and inherits workspace lints. Do not remove these rules, add lint caps, hide code behind cfg/macros/generated files, or create another exception crate.
>
> **Before adding unsafe.** Identify the unmet requirement; show why the selected safe API cannot satisfy it; link the exact upstream/native contract; propose the smallest safe interface and an invariant ledger. Obtain maintainer approval before writing it. DWM, SIMD, process-environment mutation, zero-copy NumPy borrowing, and copied native examples are not automatic exceptions.
>
> **Leaf contract.** Public APIs expose owned values, checked configurations, borrowed ordinary slices, enums, and Result. No public raw pointers/COM interfaces, unsafe functions/traits, static mut, transmute, or manual Send/Sync. Native objects are created, used, and released on the owning worker; COM initialization outlives every dependent object. Native packet borrows never escape. Invalid safe inputs cannot cause UB.
>
> **Proof and cleanup.** Document caller obligations of private unsafe functions under `# Safety`. Put an operation-specific `// SAFETY:` explanation immediately before each unsafe block/impl. Prove lengths, alignment, initialization, aliasing, provenance, thread/apartment and lifetime. Handle cancellation, panic, partial construction, device loss, and repeated close. Drop must not panic; forgetting a value may leak but must not enable use-after-free.
>
> **Budget.** Each unsafe site has an approved reason ID. More than 12 blocks or 120 lines inside unsafe blocks requires a new architecture review, not a waived counter. Safe code maintaining the same invariants is part of that review. Counters and comments do not prove soundness.
>
> **Python and workers.** Only independently owned Rust buffers/configuration enter detached/Rayon work. Do not send Python attachment tokens, Bound values, borrowed NumPy memory, or COM interfaces. Do not use SendWrapper. A copy does not make concurrent mutation during copying safe; immutable/serialized inputs are the default free-threaded boundary.
>
> **Dependencies.** Keep reviewed locks and feature sets. Review unsafe-related source changes and RustSec history. Agents may prepare audit evidence but may not certify cargo-vet audits, add exemptions, change trusted publishers, or suppress advisories without maintainer approval. Unsafe inside an unsound dependency is not repaired by calling it from a safe crate.
>
> **Required evidence.** Run policy/format, platform Clippy/tests, dependency checks, selected Miri/model tests, numerical goldens, and relevant Python/audio tests. Native audio changes require controlled hardware acceptance. Report failures, skips, and unsupported tools. Miri cannot verify COM; geiger is inventory. Compiler, workflow, audit-ledger, and leaf changes require maintainer review; no agent self-approval or unattended bypass.

### 3.7 Cost and benefit for one maintainer using coding agents

| Effect | Assessment [I] |
|---|---|
| Review workload | Most algorithm/service work can be reviewed as safe Rust plus semantic tests. Invariant-changing native patches remain exceptional and receive concentrated review. This reduces the changing first-party unsafe surface, not the total transitive trusted code. |
| Agent failure mode | Agents often copy raw-pointer/COM/SIMD examples or add unsafe Send to satisfy async bounds. Forbid fails the patch early; a task should explicitly demand owned-data/same-thread redesign rather than a lint waiver. |
| Architectural overhead | One adapter, checked data types, private apartment/session ownership, a policy checker, and audit ledger are real work. They are cheaper than debugging undefined behavior spread across UI/DSP/jobs. This is an estimate, not measured effort. |
| Runtime cost | Buffer ownership/copies, serialization, and thread messages may cost memory/time. Low-latency is not required here; measure before optimizing. Trait/crate boundaries do not inherently require copying or dynamic dispatch. |
| Build cost | An extra small Rust leaf is unlikely to dominate Tauri/PyO3 compilation. Miri, rebuilt-std sanitizers, multiple OS/interpreter jobs, and cold audit-tool builds are the significant added CI costs. Cache pins; keep expensive tests selective. |
| Residual failure | An upstream wrapper, driver, or macro may still be unsound; the process can still crash. “Quarantine like Tauri” is a review/ownership boundary, not a sandbox. |
| Maintainer bottleneck | Independent automated review helps identify issues, but approval and any incomplete-audit risk acceptance remain explicit human decisions. Do not manufacture an audit record that overstates what one maintainer reviewed. |

## 4 Risks ranked

| Rank | Risk / status | Consequence | Mitigation / stop condition [I] |
|---:|---|---|---|
| 1 | **[V/I] Soundness mistaken for zero unsafe keywords**; dependencies/macros remain trusted [1,25–29,33] | Safe callers can still trigger upstream UB; green counts create false assurance | Review invariant owners and actual safe interfaces; do not claim full binary certification |
| 2 | **[V/I] COM apartment, Release, packet, or shutdown error** [10–18] | Crash, dangling packet access, use after apartment teardown, hung capture | Thread-confined ownership; paired acquire/release; failure/leak/panic tests; hardware stress before release |
| 3 | **[V/I] Python/NumPy concurrent mutation during copy or detached borrow** [22–29] | Data race/interpreter crash despite a safe Rust signature | Immutable/owned input default; qualified ndarray API; no SendWrapper; FT tests |
| 4 | **[V/I] Unsound or incompletely reviewed upstream native API**; Tauri deprecated unmanage is concrete, wasapi packet concern remains unconfirmed [1,15,18] | Outer safe wrapper cannot repair internal UB | Ban known bad operation; investigate concerns; upstream patch/vendor minimal fix or stop adoption |
| 5 | **[V/I] Policy circumvention through manifests/build scripts/macros/CI** [26,37–41] | Unsafe appears outside the leaf or checks silently weaken | Protected policy files, metadata/syntax fixtures, explicit macro inventory, required checks |
| 6 | **[V/I] Miri/sanitizer coverage mistaken for COM proof** [59–60] | FFI-specific errors escape and get certified | Label native/model coverage separately; real Windows test matrix |
| 7 | **[V/I] “Audited” dependency graph built from exemptions/trust** [42–45] | Review claims exceed evidence; native changes auto-approved | Strict custom criterion, scoped notes, no unattended exemptions/publisher wildcard soundness claims |
| 8 | **[V/I] Toolchain/tool drift**; geiger locked dependency warning; Rust 1.98.0 compiler UB fix [54,69] | CI breakage or insecure tool; invalid release assumptions | Pin qualified stable/nightly/tools; periodic update review; tool failures visible |
| 9 | **[I] Quarantine grows into generic unsafe utility library** | Python/SIMD/UI invariants mix; review becomes impractical | Windows-only purpose, budget tripwire, new ADR for a new domain |
| 10 | **[I] Numerical semantics regress independently of memory safety** | Different BRIR despite no UB | Per-stage f64 golden arrays, exact discrete decisions, justified tolerances; keep Python SHA checks for Python regressions |

**[I] Numerical parity reminder.** Rust/scipy cross-language SHA identity is not the acceptance criterion. Keep the existing research’s per-stage oracle comparison, exact crop/peak/layout decisions, waveform/FR/ILD/ITD tolerance checks, and optimizer response comparison. Treat provisional tolerances as measurements to qualify, not universal constants. Miri/ASan cannot detect wrong spline boundary conditions or a wrong normalization factor.

## 5 Open questions you could not settle

1. **[U] Is any handwritten unsafe actually necessary after the complete inventory?** wasapi already supplies the principal native operations, Tauri supplies theme changes, PyO3/numpy owned conversions exist, and RustFFT hides SIMD. The leaf reservation is conditional, not evidence that 12 operations are needed.
2. **[U] wasapi 0.24.0 packet validity/SILENT handling:** source and native flags warrant focused review; a null/uninitialized-packet UB reproduction was not established. Do not label it a confirmed vulnerability from this report.
3. **[U] Final COM guard design against all safe wasapi calls:** initial proposals need compile tests, leak/Drop-order review, and actual failure injection. Wrapper helper initialization can interact with existing thread apartment state.
4. **[U] rust-numpy’s complete free-threaded input contract:** borrow docs and NumPy warnings do not establish exclusion against arbitrary native writers. Immutable payload input is the conservative initial choice; legacy ndarray API compatibility remains a product decision.
5. **[U] Final resolved versions/MSRV/features:** no Cargo workspace was built. Versioned evidence is not a lockfile or a proof that every listed version interoperates.
6. **[U] cargo-geiger 0.13.0 on Rust 1.98.1:** latest verified release is old relative to the compiler; documented 1.89 fix and an open install advisory warning do not establish current success or abandonment. Qualify separately or omit its non-gating report.
7. **[U] Exact working dated nightly for Miri/ASan/TSan and Kani/Verus version:** select by pilot; no date/version was invented. The proposed command shapes are documented, not locally run.
8. **[U] Dependency-audit availability:** no claim that current imported vet/crev audits cover all selected versions, features, generated code, or native hosts. Whole-framework strict review may exceed the maintainer’s capacity; record incomplete coverage honestly.
9. **[U] Hardware and driver behavior:** 16-out/2-in on physical devices, shared conversion/exclusive fallback, 44.1/48/96 kHz, unplug/sleep, tails/drain, and repeated cancellation remain native acceptance work. Virtual-endpoint success in prior research is not universal hardware validation.
10. **[U] DWM fallback need and API:** Tauri theme API exists, but exact titlebar behavior is not tested. Do not create speculative raw-handle code or promise a safe generic integer-HWND API.
11. **[U] CI wall times and proof value:** all runtime estimates are planning ranges; no whole-DSP f64 formal proof is promised. Reduce fixtures if Miri/sanitizer jobs overwhelm review throughput, not their declared guarantees.
12. **[V/U] Execution limitation:** web documentation and local sources were read; a grouped GitHub CLI metadata request required approval and did not execute. Release metadata was instead verified from primary web/package sources. No compilation, native tests, repository code changes, or CI changes were performed.

## 6 Sources

All sources were consulted for this report on **2026-09-07**, directly or by read-only research agents. Versioned URLs are preferred; `main`, `master`, and documentation portals are mutable snapshots. Local project files are named in §2.1; the supplied later audio decisions take precedence over older stack reports. Quotes are short extracts, not independent demonstrations of soundness.

1. Tauri 2.11.5 source, unsafe use and deprecated unmanage: https://docs.rs/tauri/2.11.5/src/tauri/lib.rs.html
2. tauri-runtime-wry 2.11.4 source, thread dispatch and unsafe trait implementations: https://docs.rs/tauri-runtime-wry/2.11.4/src/tauri_runtime_wry/lib.rs.html
3. WRY 0.55.0 WebView2 native implementation: https://docs.rs/crate/wry/0.55.0/source/src/webview2/mod.rs
4. tao 0.35.0 Windows window implementation: https://docs.rs/crate/tao/0.35.0/source/src/platform_impl/windows/window.rs
5. tauri-runtime-wry 2.11.4 package/dependencies: https://docs.rs/crate/tauri-runtime-wry/2.11.4
6. Tauri 2.11.5 Window API, set_theme: https://docs.rs/tauri/2.11.5/tauri/window/struct.Window.html#method.set_theme
7. raw-window-handle 0.6.2 HasWindowHandle: https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/trait.HasWindowHandle.html
8. raw-window-handle 0.6.2 WindowHandle / borrow_raw: https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/struct.WindowHandle.html
9. raw-window-handle 0.6.2 legacy trait and raw enum: https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/trait.HasRawWindowHandle.html ; https://docs.rs/raw-window-handle/0.6.2/raw_window_handle/enum.RawWindowHandle.html
10. windows-core 0.62.0 Interface source: https://docs.rs/crate/windows-core/0.62.0/source/src/interface.rs
11. windows-core 0.62.0 IUnknown source, Clone/Drop: https://docs.rs/crate/windows-core/0.62.0/source/src/unknown.rs
12. Microsoft windows-rs IAudioClient API (mutable generated docs): https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Media/Audio/struct.IAudioClient.html
13. Microsoft COM processes, threads, and apartments: https://learn.microsoft.com/en-us/windows/win32/com/processes--threads--and-apartments
14. Microsoft CoInitializeEx return/cleanup contract: https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex
15. wasapi 0.24.0 pinned source, revision 63ad5a26c3f6bda360816e4b25a613fb94ca3f72: https://raw.githubusercontent.com/HEnquist/wasapi-rs/63ad5a26c3f6bda360816e4b25a613fb94ca3f72/src/api.rs
16. wasapi 0.24.0 AudioClient and package metadata: https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioClient.html ; https://docs.rs/crate/wasapi/0.24.0
17. Microsoft IAudioCaptureClient GetBuffer and Release threading: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudiocaptureclient-getbuffer ; https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iaudiocaptureclient
18. Microsoft AUDCLNT_BUFFERFLAGS, SILENT semantics: https://learn.microsoft.com/en-us/windows/win32/api/audioclient/ne-audioclient-_audclnt_bufferflags
19. CPAL 0.18.2 API and Windows Stream traits: https://docs.rs/cpal/0.18.2/cpal/ ; https://docs.rs/cpal/0.18.2/x86_64-pc-windows-msvc/cpal/platform/struct.Stream.html
20. CPAL 0.18.2 changelog/upgrade notes: https://docs.rs/crate/cpal/0.18.2/source/CHANGELOG.md ; https://docs.rs/crate/cpal/0.18.2/source/UPGRADING.md
21. CPAL pinned WASAPI device and 0.18.2 stream implementation: https://raw.githubusercontent.com/RustAudio/cpal/e1612d5d98152f8dc2a62e1b51ef7cbf4f7f26b7/src/host/wasapi/device.rs ; https://docs.rs/crate/cpal/0.18.2/source/src/host/wasapi/stream.rs
22. PyO3 0.29.2 free-threading guide: https://pyo3.rs/v0.29.2/free-threading.html
23. PyO3 Python attachment token (mutable latest documentation): https://docs.rs/pyo3/latest/pyo3/marker/struct.Python.html
24. PyO3 Bound (mutable latest documentation): https://docs.rs/pyo3/latest/pyo3/struct.Bound.html
25. PyO3 0.29.2 Ungil and stable SendWrapper limitation: https://docs.rs/pyo3/0.29.2/pyo3/marker/trait.Ungil.html
26. PyO3 0.29.2 forbid compile-pass test; macro regression fix: https://raw.githubusercontent.com/PyO3/pyo3/v0.29.2/tests/ui/forbid_unsafe.rs ; https://github.com/PyO3/pyo3/pull/4574
27. rust-numpy 0.29.0 PyArray / IntoPyArray constructors: https://docs.rs/numpy/0.29.0/numpy/array/struct.PyArray.html ; https://docs.rs/numpy/0.29.0/numpy/convert/trait.IntoPyArray.html
28. rust-numpy 0.29.0 borrowing and implementation: https://docs.rs/numpy/0.29.0/numpy/borrow/index.html ; https://docs.rs/numpy/0.29.0/src/numpy/array.rs.html
29. NumPy thread-safety guidance: https://numpy.org/doc/stable/reference/thread_safety.html
30. RustFFT 6.4.1 planner and AVX planner: https://docs.rs/rustfft/6.4.1/src/rustfft/plan.rs.html ; https://docs.rs/rustfft/6.4.1/rustfft/struct.FftPlannerAvx.html
31. RustFFT 6.4.1 SSE checked dispatch/kernels: https://docs.rs/rustfft/6.4.1/src/rustfft/sse/sse_radix4.rs.html ; https://docs.rs/rustfft/6.4.1/src/rustfft/sse/sse_common.rs.html
32. Rust Reference target_feature and detection macro: https://doc.rust-lang.org/reference/attributes/codegen.html ; https://doc.rust-lang.org/std/macro.is_x86_feature_detected.html
33. Rustonomicon, Working with Unsafe: https://doc.rust-lang.org/nomicon/working-with-unsafe.html
34. Rustonomicon, Leaking / mem::forget: https://doc.rust-lang.org/nomicon/leaking.html
35. Rustonomicon, Send and Sync: https://doc.rust-lang.org/nomicon/send-and-sync.html
36. rustc unsafe_code lint: https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#unsafe-code
37. rustc lint levels, forbid/caps/force-warn: https://doc.rust-lang.org/rustc/lints/levels.html
38. Cargo workspace/package lint configuration: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-lints-table ; https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section
39. Clippy undocumented unsafe blocks implementation: https://raw.githubusercontent.com/rust-lang/rust-clippy/master/clippy_lints/src/undocumented_unsafe_blocks.rs
40. Clippy multiple unsafe operations per block implementation: https://raw.githubusercontent.com/rust-lang/rust-clippy/master/clippy_lints/src/multiple_unsafe_ops_per_block.rs
41. Clippy missing_safety_doc implementation: https://raw.githubusercontent.com/rust-lang/rust-clippy/master/clippy_lints/src/doc/mod.rs
42. cargo-vet built-in criteria: https://mozilla.github.io/cargo-vet/built-in-criteria.html
43. cargo-vet custom criteria and policy configuration: https://mozilla.github.io/cargo-vet/config.html
44. cargo-vet commands: https://mozilla.github.io/cargo-vet/commands.html
45. cargo-vet publisher trust: https://mozilla.github.io/cargo-vet/trusting-publishers.html
46. cargo-deny checks overview: https://embarkstudios.github.io/cargo-deny/
47. cargo-deny CLI options: https://embarkstudios.github.io/cargo-deny/cli/common.html
48. cargo-deny bans/wrappers semantics: https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html
49. cargo-deny 0.20.2 package and changelog: https://docs.rs/crate/cargo-deny/0.20.2 ; https://github.com/EmbarkStudios/cargo-deny/blob/main/CHANGELOG.md
50. cargo-audit 0.22.2 release metadata: https://docs.rs/crate/cargo-audit/0.22.2
51. cargo-audit project documentation: https://github.com/rustsec/rustsec/tree/main/cargo-audit
52. cargo-geiger 0.13.0 package metadata: https://docs.rs/crate/cargo-geiger/0.13.0
53. cargo-geiger changelog, Rust 1.89 compatibility fix: https://github.com/geiger-rs/cargo-geiger/blob/master/CHANGELOG.md
54. cargo-geiger issue #564, vulnerable locked installation dependency warning: https://github.com/geiger-rs/cargo-geiger/issues/564
55. cargo-geiger argument definitions: https://raw.githubusercontent.com/geiger-rs/cargo-geiger/master/cargo-geiger/src/args.rs
56. geiger syntax inspection / scan implementation: https://raw.githubusercontent.com/geiger-rs/cargo-geiger/master/geiger/src/lib.rs ; https://raw.githubusercontent.com/geiger-rs/cargo-geiger/master/cargo-geiger/src/scan.rs
57. cargo-crev 0.27.1 package metadata: https://docs.rs/crate/cargo-crev/0.27.1
58. cargo-crev getting-started commands and trust: https://github.com/crev-dev/cargo-crev/blob/main/cargo-crev/src/doc/getting_started.md
59. Miri README, support, setup and limitations: https://github.com/rust-lang/miri
60. Rust sanitizer documentation: https://doc.rust-lang.org/unstable-book/compiler-flags/sanitizer.html
61. Kani project and harness scope: https://github.com/model-checking/kani
62. Kani invocation and unwinding: https://model-checking.github.io/kani/usage.html
63. Verus project and supported-scope caveats: https://github.com/verus-lang/verus
64. Rust 2024 unsafe_op_in_unsafe_fn default: https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-op-in-unsafe-fn.html
65. Rust 2024 unsafe extern blocks: https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-extern.html
66. Rust 2024 unsafe attributes: https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-attributes.html
67. Rust 2024 newly unsafe environment/process functions: https://doc.rust-lang.org/edition-guide/rust-2024/newly-unsafe-functions.html
68. Rust 1.85.0 / edition 2024 announcement: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/
69. Rust 1.98.1 compiler soundness fix, 2026-09-03: https://blog.rust-lang.org/2026/09/03/Rust-1.98.1/
70. Cargo -Zbuild-std requirements: https://doc.rust-lang.org/cargo/reference/unstable.html#build-std
71. Clippy CLI --no-deps and warning control: https://doc.rust-lang.org/clippy/usage.html
72. wasapi 0.24.0 render-client safe buffer API: https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioRenderClient.html
73. RustFFT 6.4.1 Fft trait / buffer API: https://docs.rs/rustfft/6.4.1/rustfft/trait.Fft.html
74. cargo-geiger purpose and limitations: https://github.com/geiger-rs/cargo-geiger
75. cargo-vet 0.10.2 package metadata: https://docs.rs/crate/cargo-vet/0.10.2
76. Clippy lint category/default table: https://rust-lang.github.io/rust-clippy/master/index.html
