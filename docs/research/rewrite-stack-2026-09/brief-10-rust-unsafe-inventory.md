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

## Your dimension: where does `unsafe` unavoidably appear in this Rust stack, and can OUR crates stay at zero

Produce an exhaustive inventory. For every capability the product needs, say (a) whether a safe API exists in a maintained crate, (b) whether that crate is itself unsafe-free or contains audited unsafe, (c) whether OUR code would need any `unsafe` to use it, and (d) the fallback if the safe API is missing. Verify each claim against the crate's current source or docs (WebFetch raw GitHub files or docs.rs), and record the crate version you looked at.

Capabilities to cover, at minimum:
1. WASAPI capture and render through `wasapi` (wasapi-rs 0.24.0): is its public API entirely safe? Look at AudioClient::initialize_client, get_audiorenderclient / get_audiocaptureclient, read/write of the buffer (does it hand out `&[u8]`/`&mut [u8]` safely, or raw pointers?), event handles (`Handle`, WaitForSingleObject), device enumeration (DeviceCollection, IMMDevice), COM initialisation (`initialize_mta`/`initialize_sta`) and the STA/MTA threading rules when the Tauri main thread (STA) and a worker thread both touch audio. Does `wasapi` use `unsafe` internally on every COM call (windows-rs makes COM calls `unsafe fn`)? Count them with the source or cargo-geiger output if available.
2. cpal 0.18.2 for macOS CoreAudio and Linux ALSA: is the public API safe? Where is its internal unsafe (coreaudio-rs, alsa-sys FFI)? Any known soundness issues (RUSTSEC advisories, open issues mentioning UB, data races in callbacks)?
3. If a needed WASAPI feature were missing from wasapi-rs, using `windows` 0.62 directly: every COM method is `unsafe`; how much unsafe would a minimal exclusive-mode render+capture path need (estimate lines), and can it be confined to one module?
4. RustFFT 6.4.1 / RealFFT 3.5.0: safe public API; internal SIMD unsafe (AVX/NEON) and its runtime detection; `#![forbid(unsafe_code)]` compatibility for the caller.
5. ndarray 0.17 / nalgebra 0.35: safe API for our use (slicing, views, axis iteration, `par_azip`/rayon integration); do we ever need `uninit` or raw pointer constructs for performance? State whether `Vec<f64>` plus safe slicing suffices.
6. rayon 1.x: safe by construction; note determinism concerns for reductions (not unsafe, but relevant).
7. WAV I/O: hound 3.5.1 safety; writing 30/32-track float WAV; memory-mapped reads (memmap2 requires `unsafe` for `Mmap::map`) versus plain `std::fs::read` for files of at most a few hundred MB. Recommend the safe option.
8. Process spawning of ffmpeg/ffprobe: std::process::Command is safe; note Windows-specific concerns (CREATE_NO_WINDOW needs `std::os::windows::process::CommandExt::creation_flags`, which is safe).
9. Tauri 2 host code: are `#[tauri::command]`, window theming (`WebviewWindow::set_theme`), dialogs (tauri-plugin-dialog), opener, updater (tauri-plugin-updater) all safe APIs? Does Tauri's own crate (not wry/tao) contain unsafe? Where does raw window-handle unsafe live (raw-window-handle, wry, tao)? Is a dark title bar on Windows achievable without our own DWM call (DwmSetWindowAttribute via windows-rs would be unsafe)?
10. PyO3 0.27/0.29 + numpy crate: the safe surface for accepting NumPy float64 arrays (`PyReadonlyArray1<f64>::as_slice()` is safe; `as_slice_mut`/`as_array_mut` rules; `Bound<PyArray>` uninit constructors like `PyArray::new` are `unsafe`). Which constructors/copies keep us unsafe-free (`PyArray::from_vec`, `from_slice`)? Also `Python::detach`/`allow_threads` safety.
11. Velopack Rust SDK 1.2.0: safe API? Any FFI inside?
12. SIMD for DSP hot loops: does autovectorisation with safe iterators suffice, or would `std::arch` intrinsics (unsafe) or `wide`/`pulp`/`portable-simd` (nightly) be needed? Recommend a policy.
13. Real-time audio callback constraints (no allocation, no locks): which safe lock-free crates (ringbuf 0.4, rtrb, crossbeam) are unsafe internally but audited; do we need any unsafe to use them.
14. Anything else you find: `static mut`, global state, FFI for font enumeration in plotters, Windows named pipes, shared memory for IPC (not planned, but say whether avoiding it keeps us safe).

Deliver: (1) a table capability -> crate/version -> our-code unsafe needed? (yes/no/only-if) -> crate's internal unsafe and audit status -> fallback; (2) a clear verdict: can every application crate be `#![forbid(unsafe_code)]` with the chosen dependencies; (3) the short list of places where unsafe would leak into our code and exactly why; (4) risks ranked; (5) open questions; (6) sources.

Report path: E:/Impulcifer/docs/research/rewrite-stack-2026-09/report-10-rust-unsafe-inventory.md
Write the report ONLY to that exact path (create it with the Write tool). Do not write to any other location.
