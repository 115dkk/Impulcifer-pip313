# Impulcifer rewrite research - shared context (read this first, then your dimension below)

Date: 2026-09-07. You are a research analyst working for the maintainer of an open-source audio tool.
You are NOT allowed to modify any file under E:/Impulcifer. The only file you may write is the report path named at the end of this brief.

## The project

Impulcifer-py313 (repository root: E:/Impulcifer) is a desktop audio-DSP tool. It plays logarithmic sine sweeps through 1 to 16 loudspeakers, records them with binaural in-ear microphones (2-channel input), deconvolves the recordings into HRIR/BRIR impulse responses, then applies room correction, headphone compensation, parametric EQ (a vendored AutoEQ port with a bounded least-squares biquad fit), virtual bass, microphone-deviation correction and decay shaping, and writes multi-track WAV files for HeSuVi, Hangloose, EqualizerAPO and JamesDSP. It also writes analysis PNGs (matplotlib + seaborn + Pillow) and interactive Bokeh HTML reports.

Size: about 23k lines of Python on numpy/scipy. Nine-language i18n (433 keys per JSON catalogue in E:/Impulcifer/i18n/locales). A vanilla HTML/CSS/JS frontend with no bundler and no node_modules (E:/Impulcifer/webview_ui: app.js 1487 lines, index.html 730, styles.css 743) hosted in pywebview (WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux), plus a CustomTkinter fallback GUI that will be frozen, and a CLI. The frontend talks to a JSON application service (E:/Impulcifer/application/impulcifer_service.py) with about 22 methods: bootstrap, list_audio_devices, start_recording, start_brir, start_output_recovery, poll_job(job_id, after_seq), cancel_job, get/set ui settings (language, theme, skin), get_system_info, detect_sweep, generate_sweep_set, open_path, check_for_updates, start_update, apply_pending_update, select_file, select_directory, open_url.

Shipped as (a) a PyPI wheel (`pip install impulcifer-py313`, console scripts impulcifer / impulcifer_gui / impulcifer_webview) and (b) Nuitka standalone builds for Windows (Velopack installer and in-app updater), macOS (dmg) and Linux (AppImage + tarball, plus a community AUR package), all built in GitHub Actions from E:/Impulcifer/.github/workflows/publish.yml. The maintainer has spent a lot of effort fighting Nuitka/pywebview packaging drift (plugin whitelist patches, dependency version breakage, free-threaded Python status) and this pain is one motivation for the rewrite.

The maintainer wants to decide what to REWRITE it in: language, GUI shell and architecture. Windows is the primary platform, macOS second, Linux lowest priority ("Linux can fend for itself"). The maintainer already proposed three candidate stacks and wants unlisted alternatives evaluated too. The maintainer works alone and drives development with AI coding agents, so "verifiability by tests", "quality of documentation an LLM can draw on" and "ecosystem stability" matter more than typing speed.

## Hard requirements the new stack must satisfy

R1 Audio I/O: enumerate devices grouped by host API (Windows: MME / DirectSound / WASAPI, ideally ASIO; macOS CoreAudio; Linux ALSA / PulseAudio / JACK). Open an OUTPUT stream with up to 16 float32 channels at 44.1 / 48 / 96 kHz and play a buffer to completion (blocking), while an INPUT stream records 2 channels. The current code uses TWO independent streams (a playback stream in the main thread, a recording stream started in a separate thread), not one duplex stream; alignment is recovered later by sweep detection. Low latency is NOT needed. See E:/Impulcifer/core/recorder.py.

R2 DSP primitives used today (scipy names): rfft / irfft / fft, next_fast_len; fftconvolve / convolve; correlate + correlation_lags; butter + sosfilt (IIR second-order sections); firwin2 (frequency-sampling FIR design); minimum_phase (homomorphic / cepstral); savgol_filter; find_peaks; windows (hann / kaiser / get_window); ndimage.uniform_filter; InterpolatedUnivariateSpline with k=1 and k=3 on a log10 frequency axis; scipy.optimize.least_squares with bounds (Trust Region Reflective; used by the AutoEQ peaking-biquad fit in E:/Impulcifer/autoeq/frequency_response.py lines 350-520); stats.linregress; special.expit; signal.spectrogram; polyphase rational resampling (the nnresample package: Kaiser-windowed FIR, resample_poly-like); RBJ biquad peaking / shelf coefficients (E:/Impulcifer/autoeq/biquad.py and E:/Impulcifer/core/eqapo.py). All in float64.

R3 Outputs: multi-track WAV (32-bit float, up to 32 tracks), PNG plots (frequency response, impulse response, spectrogram, waterfall, decay), interactive HTML analysis (Bokeh today: interaural overlay, ILD, IPD, IACC layouts), CSV/TXT.

R4 Long-running jobs (10 seconds to several minutes) with progress events, log lines and cooperative cancellation. The UI polls a job with a sequence cursor (start_brir / poll_job / cancel_job in the service).

R5 Native file and directory dialogs, open-path, open-url, dark title bar, theme / skin / language switch at runtime, a system-info page.

R6 Packaging: installers for Windows (must keep an in-app auto-updater; Velopack today), macOS dmg (notarization optional), Linux AppImage (nice-to-have). CI on GitHub Actions for 3 platforms. A CLI entry point must remain for headless batch use. Keeping a `pip install`-able PyPI distribution of the DSP core is a strong plus but not mandatory.

R7 External helper: ffmpeg / ffprobe are downloaded at runtime to decode TrueHD/MLP (.mlp) sweep files; this stays an external binary.

R8 Parallelism: the pipeline parallelises per-speaker work (today through free-threaded Python 3.13t/3.14t threads or process pools). A compiled language gets this for free, but the design must still allow it.

R9 Numerical parity: today CI checks that the BRIR output is byte-identical to master (SHA-256 of hesuvi.wav). A rewrite in another language cannot be bit-exact against numpy/scipy. The research must say what parity strategy is realistic (tolerance-based comparison, per-stage golden files and so on).

## The maintainer's own candidate stacks (evaluate them, do not assume them)

S1 Rust + Tauri 2 (webview shell; "Linux can fend for itself").
S2 C++ + Electron (the maintainer's argument: the Python/Nuitka build is already heavy, so Electron's weight is a wash).
S3 Go + Wails (the frontend stays vanilla JS like today, no node_modules at runtime, only a type check in CI).

## Rules for your work

- Verify anything you are not certain about with WebSearch and WebFetch. Cite the URL for every non-trivial claim. Prefer primary sources (project docs, GitHub issues / releases / changelogs). Note the version number and the date you saw it.
- Mark each claim as (a) verified with URL, (b) inference, or (c) unknown / could not verify. Never present an inference as a fact.
- Do not be diplomatic. If something is a bad fit, say so and why. If two options are genuinely equal, say that.
- Read files under E:/Impulcifer when the brief points you to them (Read / Grep / Glob). Never edit them.
- Work in English. Be dense: tables over prose where possible. No filler, no restating the brief.
- The report must have these sections in this order: 1 Verdict (10 lines max), 2 Findings (with sources), 3 Proposed architecture (if your dimension asks for one), 4 Risks ranked, 5 Open questions you could not settle, 6 Sources (numbered URL list).
- When done, write the full report to the report path with the Write tool, then print the single line DONE followed by a 10-line executive summary on stdout. Do not finish before the report file exists.

## Decisions already taken (2026-09-07), which your research must build on

The stack study is finished. Its reports live in E:/Impulcifer/docs/research/rewrite-stack-2026-09/ (read 00-synthesis.md, 01-windows-audio-api.md, 02-cpal-vs-portaudio.md and report-01-audio-io.md, report-03-rust-tauri.md at least). The provisional target is:

- Rust core + Tauri 2 shell, keeping the existing vanilla HTML/CSS/JS frontend and the JSON i18n catalogues.
- Audio: Windows uses WASAPI only (no ASIO, no DirectSound/MME) through the pure-Rust `wasapi` crate (HEnquist wasapi-rs 0.24.0); macOS/Linux use `cpal` 0.18.2. PortAudio is dropped. Playback is "play a whole buffer to completion, then drain"; capture is a separate 2-channel input stream started first; low latency is not needed.
- DSP: float64 everywhere; RustFFT/RealFFT; ndarray or nalgebra; rayon for per-speaker parallelism; hand-written SciPy-compatible primitives (firwin2, minimum_phase, FITPACK-style splines, Savitzky-Golay, Kaiser polyphase resampling, SOS filters, find_peaks) and possibly a bounded least-squares solver later.
- I/O: WAV read/write (hound or a custom writer for 32-track files), ffmpeg/ffprobe as external processes, PNG plots (plotters), offline HTML reports (vendored Plotly.js), JSON settings.
- Python: a PyO3 + maturin wheel exposing the DSP core so `pip install impulcifer-py313` survives; Velopack Rust SDK for the Windows updater; Tauri updater on macOS/Linux.
- The maintainer's explicit goal for this research: **write zero `unsafe` in our own crates if at all possible.** If some `unsafe` is unavoidable, it must be quarantined in a small, separately reviewed leaf crate behind a sound safe API, the way Tauri keeps platform code inside wry/tao, so that every application crate can carry `#![forbid(unsafe_code)]`.

## Your dimension: an architecture that quarantines unavoidable `unsafe`, and the tooling that enforces it

Assume the inventory in the sibling brief finds a few unavoidable unsafe spots (most likely: direct COM calls if wasapi-rs lacks something, PyO3 array constructors, a DWM call, SIMD intrinsics). Design how this workspace keeps them contained, and research how mature Rust projects do it. Verify each pattern against real source or docs and cite it.

1. Study and document the quarantine patterns of: Tauri 2 (which crates hold platform unsafe: wry, tao, tauri-runtime-wry; how tauri itself and application code stay safe; use of `raw-window-handle`), windows-rs (unsafe fn on every COM call; how wrapper crates such as wasapi-rs expose a safe layer; `windows::core::Interface` and lifetime/ownership of COM pointers), PyO3 (how `Bound<T>` and `Python<'py>` tokens make the API safe; where `unsafe` remains and how they document it), cpal (its host modules and the `Device`/`Stream` safe boundary), and one DSP-heavy crate such as rustfft (SIMD unsafe under `#[target_feature]` with runtime detection; how it stays sound). Extract the recurring design rules.
2. Write the soundness rules for a safe wrapper: invariants, `// SAFETY:` comments, `unsafe fn` versus `unsafe` block, `#![deny(unsafe_op_in_unsafe_fn)]`, `#![forbid(unsafe_code)]` at crate level, encapsulation of raw pointers in newtypes with Drop, Send/Sync reasoning for COM objects (apartment threading), and the Rustonomicon guidance on which invariants must hold. Quote sources.
3. Propose the concrete workspace layout for Impulcifer with an explicit unsafe budget per crate: which crates are `forbid(unsafe_code)`, which single leaf crate (name it, e.g. `impulcifer-sys-win`) may hold unsafe, its public API sketch (safe types only: owned buffers, `&mut [f32]`, enums, `Result`), its review rules, and how the audio-io crate, the DSP crate, the Tauri app crate, the CLI crate and the PyO3 crate sit relative to it. Show a dependency diagram. State how a contributor or an AI coding agent is prevented from adding unsafe elsewhere (lint config + CI).
4. Enforcement tooling, verified as of 2026-09: `cargo-geiger` (status: maintained? works on current toolchains?), `cargo-deny` (advisories, licenses, bans), `cargo-audit`, `cargo-vet` and `cargo-crev` (supply-chain review of the unsafe in dependencies), clippy lints `clippy::undocumented_unsafe_blocks`, `clippy::multiple_unsafe_ops_per_block`, `clippy::missing_safety_doc`, rustc lint `unsafe_op_in_unsafe_fn` (default level in edition 2024?), `#![forbid(unsafe_code)]`, Miri (what it can and cannot check for FFI/COM code), sanitizers (ASan/TSan on nightly), Kani or Verus (optional formal checks; scope realistic for a solo maintainer), `cargo +nightly -Zbuild-std`? (probably not). Give the exact CI job list with commands and a rough runtime.
5. Dependency policy: how to choose crates by unsafe footprint (cargo-geiger counts, `#![forbid(unsafe_code)]` badges, RUSTSEC history), pinning and vetting workflow, and what to do when a needed crate has unaudited unsafe (vendor + review, or write the safe wrapper ourselves).
6. Edition and toolchain: Rust 2024 edition defaults relevant to unsafe (`unsafe_op_in_unsafe_fn` warn-by-default, `unsafe extern` blocks, `#[unsafe(no_mangle)]`), MSRV policy, and whether `-Zmiri` or sanitizer runs need nightly in CI.
7. Explain briefly what "quarantine like Tauri" buys and costs for a one-maintainer project driven by AI coding agents: review load, build time, and the failure mode where agents copy unsafe from examples.

Deliver: (1) pattern catalogue with sources; (2) the workspace layout and unsafe budget table; (3) the CI gate list with commands; (4) a one-page "unsafe policy" the maintainer can paste into CONTRIBUTING/CLAUDE.md; (5) risks ranked; (6) open questions; (7) sources.

Report path: E:/Impulcifer/docs/research/rewrite-stack-2026-09/report-11-rust-unsafe-quarantine.md
Write the report ONLY to that exact path (create it with the Write tool). Do not write to any other location.
