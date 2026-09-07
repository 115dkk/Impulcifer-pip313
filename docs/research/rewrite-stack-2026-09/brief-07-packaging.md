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

## Your dimension: packaging, updater, CI and distribution continuity (requirement R6)

Read E:/Impulcifer/.github/workflows/publish.yml, E:/Impulcifer/build_scripts/nuitka_flags.py, E:/Impulcifer/updater/velopack.py, E:/Impulcifer/updater/legacy.py, E:/Impulcifer/updater/executors.py, E:/Impulcifer/infra/environment.py and the "Standalone 릴리스", "Arch Linux (AUR)" and "업데이트" sections of E:/Impulcifer/README.md to understand today's pipeline (gate -> PyPI -> Nuitka on 3 platforms -> GitHub Release; Velopack on Windows; SHA256SUMS plus AppImage in-place replacement on Linux and macOS).

Then, for Rust, C++, Go and C#:
1. The Velopack language support matrix in 2026: official libraries for C#, C++, Rust, JavaScript / Electron, and whether Python or Go bindings exist. Verify each with URLs. Confirm that the vpk CLI is language-agnostic. Describe the in-app update flow per language (check, download, apply; delta packages), and Velopack's macOS and Linux support status in 2026.
2. Alternatives per shell: the Tauri updater, electron-updater, Wails (none), Sparkle / WinSparkle, MSIX with App Installer, Squirrel, Conveyor.
3. GitHub Actions matrices: native builds per OS versus cross-compilation (Rust with cross or cargo-zigbuild; Go cgo cross-compiling with zig cc; C++ with CMake toolchains), signing (Azure Trusted Signing, self-signed, none; macOS notarytool), expected runner time and cost.
4. PyPI continuity: maturin + PyO3 (Rust), nanobind + scikit-build-core (C++), Go c-shared + ctypes or cffi (assess honestly), C# (none). Is keeping `pip install impulcifer-py313` with the same CLI realistic for each? cibuildwheel matrix cost; abi3 wheels; free-threaded (3.13t / 3.14t) wheel support status for PyO3 and nanobind.
5. CLI binary: a separate executable versus the GUI binary with a headless flag; how each shell handles a sidecar.
6. Linux minimal story: AppImage feasibility per shell (does a Tauri AppImage require the system webkit2gtk? verify), Flatpak, AUR.
7. Runtime dependencies users must have: WebView2 (which Windows versions ship it), .NET runtime (self-contained versus framework-dependent, size), MSVC redistributable, GTK / WebKitGTK on Linux.

Deliver a per-stack packaging / updater / CI matrix with risk ratings, and the recommended pipeline shape for each of S1, S2, S3 and C#/Avalonia.

Report path: {TMP}/report-07-packaging.md
