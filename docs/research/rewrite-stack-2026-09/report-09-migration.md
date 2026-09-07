# Impulcifer rewrite: module costs and migration strategy

Research date: **2026-09-07**. Inspected revision: **0144dcc1f8a4634607b980d15c64549d4aa6c145**, local `master`, `115dkk/Impulcifer-pip313`. No repository files changed; no builds, project imports, tests or hardware experiments ran.

**Claim labels:** [V] verified from inspected source or cited primary documentation; [I] engineering inference, proposed design or estimate; [U] unresolved. In inventories, paths, physical line counts, purposes and current dependencies are [V]; difficulty and disposition are [I]. All external sources were accessed on the research date. Documentation versions are not the project's installed dependency versions.

## 1 Verdict

1. **[I] Use incremental native DSP replacement through Python bindings.** Keep Python as executable specification and test driver until native output passes stage-level and end-to-end acceptance.
2. **[I] C++ plus nanobind offers the lowest expected DSP-port cost; Rust plus PyO3 is the strongest alternative.** Prove both on the same numerical pilot before committing.
3. **[I] Prefer Tauri 2 for the eventual retained-web-UI shell.** A C++ core does not require Electron: Tauri plus a C++ core/CLI is a valid unlisted combination.
4. **[I] Go plus Wails is a poor choice for a pure-Go DSP rewrite.** Go is credible as a shell/process supervisor around a native core.
5. **[I] C# is viable, especially with native numerical kernels.** Rebuilding the UI in Avalonia adds unnecessary work unless dropping web UI reuse is intentional.
6. **[V] Normal BRIR generation does not call AutoEQ's bounded biquad optimizer.** Smoothing, equalization and minimum-phase FIR are the first numerical migration milestone, not the optimizer. [1]
7. **[V] Current 32-bit WAV output is PCM_32, not IEEE float.** R3's float output must be an explicit new format, not silently described as existing parity. [1]
8. **[I] Shell-first removes pywebview packaging but retains Python DSP packaging.** Use it only for urgent shell/installer relief, with a deliberately temporary sidecar.
9. **[I] Budget roughly 4–8 focused maintainer-months for a C++/Rust incremental product migration**, with AI implementation assistance, retained web UI and separately bounded full-AutoEQ scope; this is not a measured forecast.
10. **[I] Keep 2.x and PyPI usable throughout.** Drop CTk only from the new product; do not refactor the frozen CTk line into the JSON service.

## 2 Findings (with sources)

### 2.1 Scope corrections and counting

[V] Physical lines include comments and blanks; counts came from dedicated content search (`^`), not importing or executing project code. Every source file in the requested directories was inspected. Empty package files count as zero. Binary data were inventoried, not decoded.

| Inspected scope | Files | Physical lines |
|---|---:|---:|
| `core/`, including `core/plotting/` | 38 Python | 11,882 |
| `autoeq/` | 4 Python | 1,843 |
| `application/`, `updater/`, `infra/`, `impulcifer.py` | 17 Python | 3,234 |
| `gui/` including skins/theme | 27 Python + 1 JSON | 10,011 |
| `i18n/` | 2 Python + 11 JSON | 5,121 |
| `webview_ui/` | 3 text files | 2,960 |
| **Requested inventory total** | **103 files** | **35,051** |
| Additional shell source inspected: `impulcifer_webview.py` | 1 Python | 340 |

[V] Nine supported languages have **11 catalogue files**, because `zh-cn.json` and `zh-tw.json` are additional filename aliases. Each has 433 item lines and 435 physical lines. This investigation did not execute JSON duplicate-key or placeholder tests; those existing tests must still run. `CLAUDE.md`'s 264-key statement is stale. The repository has **53 `test_*.py` files plus `tests/__init__.py`**, not 56 test files. A static count found 594 `def test_*` declarations, not a collected pytest case count. [2][3][4]

[V] Inventory size is not rewrite size. Roughly 10k lines of CTk implementation are dropped from the new product, about 3k web lines are retained/adapted, and nearly 4.8k catalogue lines are retained. Translating all 35k lines would be wasteful. [1][2][3]

### 2.2 Exhaustive module inventory

**Difficulty [I]:** T = trivial; M = moderate; H = hard; VH = very hard. Ratings cover behavior-preserving native replacement of each module's own responsibility, assuming called dependencies already exist. They do not repeatedly charge AutoEQ's cost to every caller. C++ can use native C/C++ libraries; Rust/Go/C# may use FFI. No score implies a tested library combination.

**Disposition [I]:** P = port responsibility for a Python-free product; A = adapt/replace with target-runtime integration; K = keep source/data; D = drop new-product implementation while retaining it in 2.x/Python compatibility where applicable. Every P/A module can remain Python unchanged during intermediate migration stages. Table paths are relative to `E:/Impulcifer/`; anchors identify inspected implementation. Source roots and immutable links are [1]–[4].

#### Core DSP and processing

| File:anchor | Lines | Purpose; direct numerical/library dependencies [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `core/audio_io.py:18` | 118 | WAV/channel orientation, dB conversion, convolution, magnitude, moving mean; NumPy, SciPy signal, SoundFile/libsndfile | H | M | H | H | P; preserve PCM conversion |
| `core/audio_truehd.py:15` | 202 | Probe/decode TrueHD/Atmos profile; NumPy, SoundFile, external ffmpeg/ffprobe, subprocess/tempfiles | M | M | M | M | A; keep external helpers |
| `core/decay.py:44` | 403 | Lundeby knee/noise estimate, Schroeder decay, decay window; NumPy, find_peaks, Hann, linregress | H | H | H | H | P |
| `core/eqapo.py:218` | 940 | Stateful APO/XT config to stereo magnitude curves; NumPy, math/regex, lazy freqz/SoundFile | H | H | H | H | P; parser/report semantics |
| `core/hrir.py:366` | 1,091 | Speaker/ear collection, ingestion, alignment, crop, normalization, layouts; NumPy, signal, fftpack.next_fast_len, spline, AutoEQ | H | H | H | H | P |
| `core/impulse_response.py:15` | 188 | IR model, peak/shift, convolution, resampling, FR; NumPy, SciPy signal, nnresample, AutoEQ/decay | H | H | H | H | P |
| `core/impulse_response_estimator.py:26` | 326 | Exponential sweep/inverse, deconvolution, sequence, reconstruction/CLI; NumPy, SciPy full FFT/convolution/Hann, matplotlib | H | H | H | H | P; separate demo plots |
| `core/microphone_deviation_correction.py:48` | 507 | Direct-sound interaural power matching, bilateral FIR; NumPy, SciPy FFT/signal, AutoEQ, lazy matplotlib | H | H | H | H | P |
| `core/pipeline.py:416` | 978 | Config/default metadata, ordered stages/cancellation/workers/outputs; stdlib at import, runtime DSP/AutoEQ/plotting | M | M | M | M | P; one canonical schema |
| `core/pipeline_stages.py:85` | 687 | Estimator selection, EQ/APO/headphone/target/recording/report helpers; NumPy, AutoEQ, matplotlib, Bokeh, tabulate | H | H | H | H | A; separate render responsibility |
| `core/room_correction.py:78` | 461 | Room measurement/calibration, generic/specific curves, band limiting; NumPy, Hann, AutoEQ, HRIR, matplotlib | H | H | H | H | P |
| `core/sweep_detection.py:53` | 244 | Header-duration grid inference, envelope-onset fallback/confidence; NumPy, SoundFile, lazy estimator | M | M | M | M | P |
| `core/sweep_set_generator.py:75` | 165 | Stereo speaker groups and combined 7.1 sweep files; delegated estimator/WAV, argparse/filesystem | M | M | M | M | P |
| `core/sweep_signal.py:138` | 205 | SweepSpec/playback/progress/sidecar; NumPy, estimator, in-memory PCM_32 SoundFile round-trip | H | M | H | H | P; not raw float generation only |
| `core/virtual_bass.py:82` | 198 | Shared bass IR, crossover matching, directional shelves/delay; NumPy rFFT, Butterworth/SOS/tf2sos | H | H | H | H | P |

#### Core recording, scheduling, recovery and support

| File:anchor | Lines | Purpose; dependencies [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `core/__init__.py:1` | 1 | Package marker; none | T | T | T | T | D |
| `core/brir_recovery.py:84` | 588 | Validate/reconstruct surviving output layouts, staged writes/rollback; NumPy, SoundFile, filesystem | M | M | M | M | P; good early exact-contract module |
| `core/cancellation.py:20` | 34 | Cooperative cancellation scope; ContextVar/event/contextmanager | M | M | M | M | A; explicit cancellation token |
| `core/cli_builder.py:27` | 70 | argparse from dataclass metadata; stdlib | M | M | M | M | A; retain CLI/default semantics |
| `core/constants.py:5` | 150 | Speaker/layout/file/sweep constants; built-ins, lazy resource helper | T | T | T | T | P; data can be generated/shared |
| `core/ffmpeg_discovery.py:199` | 344 | Discovery/version/install/lazy locked state; subprocess/platform, choco/winget/brew | M | M | M | M | A; keep binary external |
| `core/font_setup.py:107` | 211 | Font registration/fallback/matplotlib globals; matplotlib/font_manager | H | M | H | M | A; renderer-specific, not numerical core |
| `core/headphones_recording.py:34` | 65 | Playback validation and mono/stereo reason keys; SoundFile metadata/filesystem | M | M | M | M | P |
| `core/parallel_processing.py:84` | 273 | Thread-first map/worker/GIL compatibility and demos; stdlib, parallel utility | M | M | M | M | A; discard GIL policy |
| `core/parallel_utils.py:33` | 247 | Ordered maps, init/timeout/fallback/memory budget; futures, ctypes, OS memory probes | M | M | M | M | A; preserve ordering/errors, not executors |
| `core/parallel_workers.py:69` | 197 | Array convolution/decay/EQ/render workers; SciPy/AutoEQ, lazy matplotlib/seaborn | H | H | H | H | A; split numerical and render tasks |
| `core/plotting_utils.py:19` | 138 | Axis helpers, PNG quantization, acoustic calculator; NumPy, matplotlib, Pillow | H | M | H | M | A |
| `core/recorder.py:298` | 553 | Host/device selection, blocking play + recording thread, append/progress/CLI; NumPy, sounddevice/PortAudio | H | H | H | H | A; hardware acceptance mandatory |
| `core/recording_naming.py:40` | 109 | Canonical filenames/speaker validation; os/regex/constants | T | T | T | T | P |
| `core/recording_progress.py:63` | 145 | Sweep intervals and immutable progress; stdlib, scalar arithmetic | T | T | T | T | P |
| `core/recording_status.py:46` | 106 | Metadata/peak/activity/duration summaries; NumPy, SoundFile/math | M | M | M | M | P |
| `core/recording_validation.py:24` | 69 | Filename mismatch and input-channel rules; regex/dataclass | T | T | T | T | P |
| `core/utils.py:14` | 54 | Compatibility re-exports; delegates I/O, FFmpeg, fonts/plots | T | T | T | T | D; keep Python imports during transition |

#### Plotting and AutoEQ

| File:anchor | Lines | Purpose; dependencies [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `core/plotting/__init__.py:16` | 19 | Eager mixin exports; plot modules | T | T | T | T | D |
| `core/plotting/analysis.py:31` | 138 | ILD/IPD/EDC/IACC arrays; NumPy, SciPy FFT/correlate/lags | H | M | H | M | P; separate data from presentation |
| `core/plotting/bokeh_registry.py:6` | 37 | Immutable analysis names/title/method registry; dataclasses, no Bokeh import | T | T | T | T | P; retain identifiers |
| `core/plotting/hrir_plotter.py:40` | 1,012 | Two-pass synchronized PNGs and six interactive Bokeh analyses; matplotlib/seaborn/Pillow/Bokeh, NumPy, processes | VH | H | VH | H | A; retain Python renderer initially |
| `core/plotting/impulse_response_plotter.py:33` | 609 | Six-panel IR/decay/FR/spectrogram/waterfall; matplotlib 2D/3D, NumPy, signal/spline/ndimage.uniform_filter | VH | H | VH | H | A; port derived data before drawing |
| `autoeq/__init__.py:2` | 9 | Vendored version/package marker; none | T | T | T | T | D; retain attribution/version metadata |
| `autoeq/constants.py:5` | 36 | Defaults and exact Harman frequency tables; os | T | T | T | T | P/K data |
| `autoeq/biquad.py:22` | 178 | RBJ peaking/shelf coefficients and dB response; NumPy, demo matplotlib | M | M | M | M | P; coefficient-sign convention |
| `autoeq/frequency_response.py:41` | 1,620 | FR mutation/CSV/interpolation/smoothing/EQ/FIR/fit/scores/export; NumPy, SciPy spline/least_squares/savgol/find_peaks/minimum_phase/firwin2/expit/linregress/next_fast_len | VH | VH | VH | VH | P selectively; keep optimizer in Python first |

[I] Plotting's C++/C# scores assume adopting a mature plotting/rendering library and accepting visual equivalence rather than matplotlib/Bokeh object compatibility. They are not evidence that either language has a drop-in Bokeh replacement. If the same shared browser renderer is used for every language, these scores converge; renderer choice matters more than host syntax.

#### Application, updater, infrastructure, CLI

| File:anchor | Lines | Purpose; dependencies [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `application/__init__.py:1` | 5 | Service export; project dependency | T | T | T | T | D/native; K/Python |
| `application/impulcifer_service.py:224` | 1,368 | JSON bootstrap/config/settings/devices/jobs/logs/cancel/recovery/update; threads/locks/dataclasses, sounddevice, delegated DSP | H | H | H | H | A; keep observable protocol |
| `updater/__init__.py` | 0 | Package marker | T | T | T | T | D |
| `updater/environment.py:9` | 15 | Environment re-export | T | T | T | T | D |
| `updater/executors.py:31` | 223 | pip/Velopack/installer execution/results; subprocess/ABC/dataclass | M | M | M | M | A; pip remains in Python product |
| `updater/legacy.py:34` | 181 | Current macOS/Linux update, SHA256/DMG/AppImage replacement; urllib/hashlib/tempfile/shutil/process | M | H | M | M | A; name does not mean obsolete |
| `updater/update_checker.py:34` | 284 | GitHub release query/version/assets; packaging.version, urllib/JSON/regex | M | M | M | M | A; preserve prerelease/version policy |
| `updater/updater_core.py:22` | 51 | Re-export compatibility/diagnostic | T | T | T | T | D/native; K/Python |
| `updater/velopack.py:99` | 337 | Feed/package/cache/hash/Update.exe apply; packaging.version, urllib/hashlib/process | M | H | M | M | A; keep installed-product compatibility |
| `infra/__init__.py` | 0 | Package marker | T | T | T | T | D |
| `infra/_build_info.py:12` | 13 | Generated build type/version; none | T | T | T | T | A; generated metadata |
| `infra/environment.py:25` | 105 | Install/platform/pip/Nuitka probes; stdlib, optional pip | M | M | M | M | A; remove Python probes only from native app |
| `infra/get_version.py:5` | 25 | Build-time TOML version reader; tomllib or toml | T | T | T | T | K as CI tooling |
| `infra/logger.py:78` | 267 | Localized console/progress/GUI callbacks; stdlib, lazy i18n | M | M | M | M | A; typed events, explicit ownership |
| `infra/resource_helper.py:10` | 93 | Resource/font resolution; filesystem/environment | M | M | M | M | A; keep asset layout initially |
| `infra/version.py:17` | 64 | Lightweight marker/TOML/package-metadata fallback; tomllib/tomli/importlib | M | M | M | M | A; native build metadata, Python wrapper stays |
| `impulcifer.py:24` | 203 | Public compatibility/main/CLI/diagnostics; matplotlib.font_manager, lazy pipeline, argparse | M | M | M | M | A; preserve CLI and Python main API separately |
| `impulcifer_webview.py:152` (extra scope) | 340 | pywebview bridge/dialogs/URL allowlist/window/titlebar; pywebview, ctypes/DWM, threading | M | M | M | M | A; necessary shell replacement |

[V] Service complexity is behavioral: canonical defaults derived from ProcessingConfig, one active job, bounded event history, sequence cursors, terminal-job retention, progress/log translation, callback restoration, best-effort summaries, and staged update/restart. Recording is currently non-cancellable. Native dialogs and URL allowlisting live in the shell bridge, not solely in the service. [2], `application/impulcifer_service.py:349,519,878,1194`; `impulcifer_webview.py:259–294`.

#### CTk common code: DROP from new product

[I] These hypothetical port scores are for preserving equivalent screen responsibilities, **not** a proposal to port CTk. Count no CTk widget rewrite in the recommended budget; audit its behaviors and keep the existing web equivalents. No module below implements NumPy/SciPy DSP directly. [3]

| File:anchor | Lines | Purpose; dependencies [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `gui/__init__.py:1` | 1 | Marker | T | T | T | T | D |
| `gui/brir_args.py:111` | 208 | Tk→BRIR arguments, option gating/file copies; dataclass/shutil/core config, scalar conversions | M | M | M | M | D adapter; retain contract cases |
| `gui/constants.py:9` | 49 | Widget sizes/file filters | T | T | T | T | D; reuse file types |
| `gui/dialogs.py:61` | 672 | Modals, jobs/logs/cancel/language/update; CTk/Tk/threads/executors | H | H | H | M | D; web equivalents |
| `gui/event_bus.py:11` | 57 | Publish/subscribe; stdlib | T | T | T | T | D |
| `gui/legacy_gui.py:23` | 1,600 | Old recorder/BRIR UI/help/file detection; Tk/sounddevice/matplotlib/shutil | H | H | H | M | D; audit unique help/detection |
| `gui/modern_gui.py:38` | 440 | CTk startup/tabs/theme/language/update; CTk/Pillow/threads | H | H | H | M | D |
| `gui/recording_actions.py:28` | 440 | Shared recording/sweeps/headphones/sidecar; Tk/threads/delegated recorder | M | M | M | M | D adapter; retain behavioral tests |
| `gui/recording_status.py:25` | 226 | Live timing/completion summaries; Tk timers/monotonic | M | M | M | M | D |
| `gui/recovery_actions.py:20` | 192 | Async recovery/result/open folder; threads/process/core recovery | M | M | M | M | D adapter; retain contracts |
| `gui/sweep_source.py:51` | 200 | Sweep choices/spec/detection preview; Tk/threads/core, numeric parsing | M | M | M | M | D adapter; retain spec grammar |
| `gui/utils.py:28` | 1,002 | Icons/dialogs/state/fonts/scroll workaround; CTk, ctypes/GDI/CoreText/fontconfig/xrandr/fontTools | H | H | H | H | D; browser owns scrolling/fonts |

#### CTk tabs, skins and theme

| File:anchor | Lines | Purpose; dependencies [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `gui/tabs/__init__.py:1` | 1 | Marker | T | T | T | T | D |
| `gui/tabs/recorder_tab.py:43` | 585 | Devices/sweep/recording; CTk/sounddevice/mixins | H | H | H | M | D |
| `gui/tabs/impulcifer_tab.py:55` | 707 | BRIR options/worker/cancel; CTk/main/logger/args | H | H | H | M | D |
| `gui/tabs/recovery_tab.py:17` | 170 | Recovery form/results; CTk/mixin/theme | M | M | M | M | D |
| `gui/tabs/settings_tab.py:25` | 288 | Skin/frontend/language/theme/folders; CTk/localization/events | M | M | M | M | D |
| `gui/tabs/info_tab.py:32` | 264 | Version/runtime/CPU/credits/links; CTk/Pillow/platform | M | M | M | M | D; native runtime info replaces GIL |
| `gui/skins/__init__.py:12` | 14 | Skin identifiers | T | T | T | T | D |
| `gui/skins/studio_shell.py:23` | 247 | Sidebar/lazy tabs/state; CTk/Pillow/theme | M | M | M | M | D |
| `gui/skins/studio_widgets.py:23` | 412 | Cards/disclosure/fields; CTk/theme | M | M | M | M | D |
| `gui/skins/studio_recorder_tab.py:54` | 639 | Card recorder/presets/progress; CTk/sounddevice/mixins | H | H | H | M | D |
| `gui/skins/studio_impulcifer_tab.py:51` | 752 | Card BRIR/EQ/cancel; CTk/main/logger/args | H | H | H | M | D |
| `gui/skins/studio_recovery_tab.py:24` | 171 | Card recovery; CTk/mixins | M | M | M | M | D |
| `gui/skins/studio_settings_tab.py:22` | 271 | Card settings; CTk/localization/events | M | M | M | M | D |
| `gui/skins/studio_info_tab.py:27` | 275 | Card system info/credits; CTk/Pillow/platform | M | M | M | M | D |
| `gui/theme/__init__.py:20` | 180 | Colors/assets/Tk mono fallback; filesystem/tkfont | M | M | M | M | D code; reuse design values |
| `gui/theme/pulse.json:1` | 143 | CTk ThemeManager schema/colors/fonts | T | T | T | T | D; CSS already represents web design |

#### i18n and web source

[V] Catalogues are flat UTF-8 JSON, with named `{placeholder}` interpolation and no numerical dependencies. Nine-language support does not mean every entry is translated: newer keys use English fallback in several locales. [3]

| File:anchor | Lines | Purpose [V] | Rust | C++ | Go | C# | Disposition |
|---|---:|---|:---:|:---:|:---:|:---:|---|
| `i18n/__init__.py` | 0 | Marker | T | T | T | T | D |
| `i18n/localization.py:64` | 336 | Locale normalization/fallback/format/settings/first run; JSON/OS/stdlib | M | M | M | M | A; keep catalogues |
| `i18n/locales/en.json:2` | 435 | 433 English entries | T | T | T | T | K initially |
| `i18n/locales/ko.json:2` | 435 | 433 Korean entries | T | T | T | T | K initially |
| `i18n/locales/de.json:2` | 435 | 433 German/fallback entries | T | T | T | T | K initially |
| `i18n/locales/es.json:2` | 435 | 433 Spanish/fallback entries | T | T | T | T | K initially |
| `i18n/locales/fr.json:2` | 435 | 433 French/fallback entries | T | T | T | T | K initially |
| `i18n/locales/ja.json:2` | 435 | 433 Japanese/fallback entries | T | T | T | T | K initially |
| `i18n/locales/ru.json:2` | 435 | 433 Russian/fallback entries | T | T | T | T | K initially |
| `i18n/locales/zh_CN.json:2` | 435 | 433 simplified Chinese/fallback entries | T | T | T | T | K initially |
| `i18n/locales/zh_TW.json:2` | 435 | 433 traditional Chinese/fallback entries | T | T | T | T | K initially |
| `i18n/locales/zh-cn.json:2` | 435 | Simplified Chinese filename alias | T | T | T | T | K, enforce alias identity |
| `i18n/locales/zh-tw.json:2` | 435 | Traditional Chinese filename alias | T | T | T | T | K, enforce alias identity |
| `webview_ui/index.html:1` | 730 | Five views/forms/modals/IDs; standard HTML | T | T | T | T | K mostly; runtime/frontend wording adapts |
| `webview_ui/styles.css:1` | 743 | Themes/skins/layouts/fonts; standard CSS | T | T | T | T | K; package asset URLs/fonts |
| `webview_ui/app.js:1` | 1,487 | UI/i18n/requests/jobs/update; DOM/Promises/timers, pywebview.api | M | M | M | M | A bridge; keep JS implementation |

[V] The JS entry points are `window.pywebview.api` and `pywebviewready` (`app.js:58,1486`). Keeping the Promise/response contract avoids a UI rewrite. The JS calls 22 bridge methods; `set_frontend` becomes obsolete only in the new CTk-free product. Web Stable/Studio skins remain valid after dropping CTk. [3]

[I] Keep current IDs, option omission rules, success/error envelopes, `job_id`, `events`, `next_seq`, confirmation retry behavior and active-job reconnection. A small transport adapter should replace the host-specific binding. Adapt gallery mocks at that adapter rather than teaching the application about three host APIs. Do not translate app.js into Rust/Go/C#.

### 2.3 The five hardest pieces

| Rank | Piece | Verified difficulty and pitfalls | Migration consequence [I] |
|---|---|---|---|
| 1 | `autoeq/frequency_response.py` | [V] Bounded fit uses log10(fc), ln(Q), gain; Q 0.1–20 and gain −60–60; 1,000 smoothing passes in fit initialization; `max_time` becomes `max_nfev`, not a real deadline. Savitzky–Golay edge handling, logistic blending, quadratic clipping-transition spline, FIR design/minimum phase and mutable field resets all matter. `:350–520,637,859,903,1033,1276`. [1] | Split ordinary FR/smoothing/FIR from full optimizer/API. Generic LM is not equivalent to SciPy TRF; compare both objective and resulting response. |
| 2 | `core/hrir.py` with `decay.py`/IR | [V] Python rounding and recording-column mapping, common-prefix crop preserving ITD, correlation sign/first-maximum, non-circular shifts, coherent ear-wise spectral-sum normalization, strict 80<f<6000 mask, knee-dependent lengths, asymmetric HeSuVi order. `hrir.py:146,426,457,548,614,921,957`. [1] | Exact channel/index/shape contracts first. Floating tolerance must never conceal a changed layout or one-sample alignment regression. |
| 3 | Plotting modules plus Bokeh reports | [V] Two-pass globally synchronized limits; process rendering for memory reclamation; different PNG/Bokeh ear time origins; Bokeh data sources/hover/legend/grid graphs; spectrogram and waterfall contain DSP, not only drawing. `hrir_plotter.py:40,429,488`; `impulse_response_plotter.py:214,459`. [1] | Separate plot arrays from presentation. Retain Python rendering initially; later compare data and visual/interactivity contracts, not PNG bytes or Bokeh object identity. |
| 4 | `core/eqapo.py` | [V] Decimal commas/thousands parsing, frequency quirks, filter defaults, shelf corner/center conversion, condition/include/channel scopes and diagnostic counts. Convolution uses magnitude only; GraphicEQ uses log-axis linear interpolation/flat extrapolation. AutoEQ and APO coefficient-sign conventions differ. `:191,218,286,464,603,827,907`. [1] | Port parser state/diagnostics and formula fixtures together. Do not replace it with a general EqualizerAPO execution engine or blindly reuse AutoEQ coefficients. |
| 5 | `core/microphone_deviation_correction.py` | [V] Interpolate magnitude then square, average powers before dB, frontal/diffuse reference selection, smoothing/band weighting/clamp/strength ordering, opposite per-ear signs, fixed FIR truncation. Compatibility API uses `same`, HRIR uses `full`; existing fallbacks are observable. `:106,195,233,329,438`. [1] | Record intermediate powers/mismatch/FIR/lengths. Test headphone-compensation gating and fallback paths, not only a successful correction. |

[V] **Sweep detection is not one of the five hardest.** At 244 lines it uses duration-grid checks plus envelope thresholding/merging. The risk is exact grid snapping and thresholds, not exotic mathematics: 50 ms smoothing, 1% threshold, 300 ms gap merge and 500 ms minimum segment (`sweep_detection.py:64,128`). Generation rounds a requested minimum upward; detection chooses a nearby discrete grid. [1]

[V] Other easy-to-miss numerical contracts: legacy `fftpack.next_fast_len` and newer `fft.next_fast_len` are both used; `magnitude_response` excludes even-length Nyquist and has no epsilon before log; nnresample must retain its filter/delay/length convention; two cascaded fourth-order Butterworth chains are not an eighth-order redesign; room-correction masking uses a full Hann shape in the transition interval. [1]

### 2.4 Library evidence and stack consequences

| Question | Verified primary-source evidence | Consequence [I] / limitation [U] |
|---|---|---|
| Python/native migration, Rust | [V] PyO3 0.27.1 documents `Python::detach` for concurrent Python-independent work. Current main guide says free-threading support began at 0.23 and default declaration changed at 0.28. Maturin supports mixed Python/Rust packages and wheels. [5][6][7] | Native compute does not require a free-threaded interpreter. Use detached work with owned/immutable arrays. Pin a released PyO3/maturin pair; main docs are not a release lock. |
| Python/native migration, C++ | [V] nanobind ndarray supports typed/contiguous CPU arrays, no-conversion arguments and ownership capsules; free-threading requires deliberate support. Packaging uses CMake/scikit-build-core. Dated changelog: 3.0.1, 2026-08-28, Python >=3.10; 2.10 series retains Python >=3.9. [8][9][10][11] | Current project supports Python 3.9. Keep Python fallback there or intentionally pin an older binding; do not silently raise the package's minimum. Latest docs include unreleased/provisional ABI material: do not base 3.13t/3.14t wheels on Python 3.15 ABI promises. |
| Bounded fitting | [V] SciPy 1.18.0 docs specify default TRF, 2-point Jacobian and default ftol/xtol/gtol 1e-8. Ceres latest docs offer bounded LM/Dogleg with an added line search, not a claim of SciPy TRF equivalence. [12][13] | C++ has a well-documented bounded solver option, but replacing the algorithm remains a behavior change requiring acceptance tests. Source-porting required SciPy routines is another possibility subject to attribution/license review. |
| Rust solver | [V] argmin 0.11.0 TrustRegion API requires cost/gradient/Hessian; documented radius controls are not parameter lower/upper bounds. [14] | [U] No verified drop-in bounded SciPy TRF found in this investigation. Do not claim argmin lacks every constrained solver; the inspected type does not settle this requirement. |
| Go solver | [V] Gonum v0.17.0 optimize documents unconstrained methods; inspected API does not establish bounded nonlinear least squares. [15] | [U] No verified complete pure-Go replacement for this SciPy surface. A C/C++ solver through cgo is possible [I], but undermines the proposed simplification to an all-Go stack. |
| C# solver | [V] Math.NET Numerics API, assembly 5.0.0.0, exposes lower/upper bounds on LevenbergMarquardtMinimizer. It does not claim TRF equivalence. [16] | C# is not disqualified by bounded fitting, but still needs FIR/smoothing/resampling semantic work. |
| FFTs | [V] RustFFT 6.4.1 supports f64 and is unnormalized. FFTW3 FAQ notes unnormalized transforms and plan-dependent rounding under measurement planning; FFTW has GPL/commercial licensing. [17][18][19] | Pin normalization, plan policy and transform lengths. C++ does not guarantee bit equality merely because SciPy uses native code. Check licensing before choosing FFTW; FFTW is not mandatory. |
| SciPy signal defaults | [V] firwin2 has endpoint/grid/type constraints; minimum_phase defaults to half-length output with square-root magnitude and a particular default FFT-size policy. SciPy 1.18.0 reference. [20][21] | Recreate actual call parameters, including explicit n_fft in this repo, rather than applying library defaults from memory. |
| PortAudio | [V] Host-API device mapping, multichannel float32 and blocking read/write are documented; validate requested formats with `Pa_IsFormatSupported`. Multiple streams/devices depend on backend; ASIO has restrictions. API docs at v19 URL display API 2.0. [22] | Prefer preserving PortAudio behavior across languages initially. No language/library promise replaces testing two independent streams on real 16-output hardware at 44.1/48/96 kHz. ASIO remains conditional. |
| Tauri 2 | [V] Static `frontendDist` directories are supported; API global injection is configurable. Sidecars require packaged executables and target-triple naming. Updater signatures are mandatory; Windows MSI/NSIS, macOS app archives, Linux AppImage artifacts are documented. [23][24][25] | Existing static frontend can remain. Tauri does not package Python's scientific dependencies for you. Its updater is not automatically compatible with existing Velopack installations. |
| Electron + C++ | [V] Electron native-module docs describe ABI rebuild obligations; utilityProcess hosts Node scripts. Node child_process.spawn runs an arbitrary executable asynchronously with piped stdio. [26][27][28] | Prefer C++ worker executable over direct Electron ABI-coupled addons for this offline job workload. Electron's size being acceptable does not remove updater/Chromium/security/build upkeep. |
| Go + Wails | [V-search] Official v2 installation search result is labeled v2.15.0 and lists Node/NPM development requirements and Windows WebView2. Direct fetch was blocked. [29] | Do not confuse no node_modules at runtime with no frontend build tooling. [U] Wails v3 stability was not independently settled: search claimed beta but fetched FAQ was blocked. Pin v2 or verify a dated v3 release before choosing it. |
| Retaining Velopack | [V] Official Rust and C/C++ startup/update integrations exist; C API is usable from other C-call-capable languages. C++ docs recommend matching packaging-tool and app-library versions. Version numbers on getting-started examples are placeholders. [30][31] | Keep Windows package identity/feed/update flow for first native release; language change alone does not force updater replacement. [U] Cross-version upgrade from this particular existing Update.exe package needs installed-app testing. |
| Unlisted C#/Qt alternatives | [V] Avalonia docs cover Windows/macOS/Linux desktop on .NET 8+, reference 12.x. ScottPlot 5 docs show headless PNG export and .NET GUI integrations. Qt6 Graphs documents 2D/3D rendering and GPLv3/commercial terms. [32][33][34] | C# + Avalonia or C++ + Qt can produce native UIs/plots, but require screen/report work that preserving web UI avoids. Neither establishes drop-in Bokeh compatibility. |

[I] **Stack ordering for this migration:** (1) C++ core + nanobind, retained Python app, later Tauri shell; (2) Rust core + PyO3, later Tauri; (3) C++ worker + Electron if one bundled Chromium engine is worth its maintenance; (4) C# core or C# host with native kernels; (5) pure Go DSP + Wails. This ordering concerns migration cost/verifiability, not language quality. C++/Rust can be close enough that the pilot should decide. Adding both Rust and C++ to the final product is a real cost; choose pure Rust if the pilot closes the numerical gap with acceptable dependencies.

### 2.5 What can actually be kept

| Item | Keep unchanged? | Verified basis and proposed treatment |
|---|---|---|
| HTML/CSS | Mostly yes | [V] Standard static files reference `font/` and `logo/`; no binary assets inside webview_ui. [I] Keep DOM IDs/layouts/themes, adjust host URLs/runtime wording only. [3] |
| JS | Most logic, not exact whole file | [V] Host binding and readiness event are pywebview-specific; ordinary UI/job logic uses DOM/Promises. [I] Replace one transport adapter and test rejection/disconnection paths. [3] |
| JSON catalogues | Yes initially | [V] 11 files/9 languages/433 entries. [I] Keep keys, fallback semantics and named placeholders; change Python/CTk-specific wording deliberately, not wholesale translation. [3] |
| Fonts/logos/licenses | Yes | [V] Four TTFs: Pretendard, PretendardJP, JetBrains Mono regular/italic; OFL files; PNG sizes 16–256 plus ICO/ICNS/SVG. [I] Keep licenses and package paths. Browser CSS currently does not load the JP variant; test Japanese glyph coverage. [3] |
| Sweep/demo/Harman/MLP data | Yes | [V] Existing WAV/CSV/TXT/MLP assets are language-independent. [I] Use tracked input assets, not arbitrary locally generated demo outputs, as fixtures. Decoder behavior still needs acceptance tests. [1][3] |
| Existing tests/fixtures | Fixtures and many tests yes; all tests unchanged no | [V] Python imports, monkeypatches, Tk/Nuitka and object APIs occur in tests. [I] Keep Python tests for the Python product; parameterize numerical/behavior contracts for native backends. Keep EqAPO TXT/RAW/WAV fixtures. [4] |
| CI shape | Yes, not entire YAML unchanged | [V] publish.yml has release gate → PyPI → three OS builds → release; OIDC names are bound. [I] Retain gate/signing/smoke/checksum concepts, add native wheels/artifacts and adapt build jobs. [4] |
| Docs | Mostly content, not install/build instructions | [V] Measurement/DSP/TrueHD/design/ADR material exists. [I] Keep and audit behavior descriptions; rewrite installation/runtime/build docs; retain historical 2.x material. [4] |
| CTk | No in new product | [V] ADR rejects forcing CTk through the JSON service. [I] Preserve unchanged 2.x, inventory missing help/file detection before removal. Current CLAUDE says freeze rather than remove in v3; this brief's new-product removal is a deliberate changed product decision. [4] |

## 3 Proposed architecture

All designs, thresholds, timelines and sequencing in this section are **[I]**, except explicitly marked current behavior [V].

### 3.1 Boundaries and ownership

- **Portable numerical core:** channel-major owned float64 arrays, explicit sample rate/speaker/ear/layout metadata; operations return arrays and typed diagnostics, with no GUI, global logger, Python object, filesystem side effect or rendering object required by numerical kernels.
- **Application orchestration:** one canonical ProcessingConfig schema supplies CLI and service defaults; ordered stage table; bounded job/event history and monotonically increasing sequences; explicit cancellation token and per-job logging. Do not independently reimplement three sets of defaults.
- **Audio adapter:** PortAudio initially, with independent output and two-channel input streams. Blocking playback completion plus recording completion remains the contract; initialization/error/shutdown are explicit. Recording cancellation is not silently added in the parity release.
- **I/O adapter:** keep external FFmpeg/ffprobe and inspect decoded metadata; controlled temporary paths, timeouts, child reaping. Retain WAV orientation/PCM rounding/clip conventions. Add optional float output separately if R3 requires it.
- **Analysis model and renderer:** native code produces FR/IR/ILD/IPD/IACC/EDC/spectrogram/waterfall arrays and semantic axis metadata. Python plotting remains the first renderer. Later select one reusable renderer for both headless PNG and interactive HTML; do not claim completion while CLI plot outputs still require a missing browser or undocumented Python installation.
- **Adapters:** Python binding for differential tests/PyPI; native CLI for batch use; shell transport for retained JS. Share the same core through these interfaces.

**FFI contract:** accept explicit contiguous float64 shape/stride semantics; initially copy inputs into job-owned storage rather than expose shared mutable NumPy memory. Return owned results with correct lifetime ownership. No callbacks while holding numerical locks; aggregate progress into a thread-safe queue. Detach from Python/release the GIL during long native work. Native cancellation uses an atomic flag checked between bounded chunks and within long iterations; exceptions/panics must not unwind across a C ABI. Reentrancy must be explicit even if the UI initially admits one job.

**Shell transport:** preserve `{ok:true,data}` / `{ok:false,error:{code,message,details,retryable}}`, method names and `poll_job(job_id,after_seq)`. For sidecars, use versioned request IDs over framed stdio, not large float arrays in JSON and not a public localhost server. Protocol stdout must contain only protocol frames; drain stderr continuously. Keep bulk audio in owned native buffers or controlled temporary binary files. Reader/control handling must remain responsive while a worker computes, so `cancel_job` does not queue behind the computation. On child exit, mark active jobs failed and clear UI busy state; on shutdown, request graceful stop, then use a bounded forced-termination fallback. Native file dialogs stay in the shell; open_url remains allowlisted.

### 3.2 Parity harness: add a second contract, do not delete the first

[V] `tests/test_brir_integrity.py:39–61` has five scenarios: defaults/headphone compensation; virtual bass at 250 Hz; decay=100/balance=trend/bass_boost=4; fs=44100 plus TrueHD layouts/JamesDSP/Hangloose; no headphone compensation. It builds a git-worktree reference, but hashes **only hesuvi.wav**, is opt-in and restricted to Linux CPython 3.13. Existing characterization tests allow approximately ±1 dB/±2 samples and are not sufficient rewrite tolerances. Existing repeated-thread tests inspect top-level WAVs, not nested Hangloose outputs. [4]

1. **Freeze a reference artifact.** Pin the verified commit, Python/dependency versions, OS/architecture, input hashes, config including omitted/defaulted keys and thread settings. Generate golden artifacts from a clean reference checkout; never bless local generated demo WAVs without provenance. Golden regeneration is explicit review, not something a failing test does.
2. **Retain Python-to-Python exact tests.** Existing 2.x SHA and exact NumPy rFFT regression checks stay. Add Python-to-native tolerance tests as a separate test class. A passing loose native check must not weaken the maintained Python contract.
3. **Capture each stage.** Sweep/inverse; decoded/quantized playback; deconvolved IR; detected peaks and crops; decay knee/windows; alignment offsets; calibrated room/headphone curves; smoothing/equalization; minimum-phase FIR; microphone correction; virtual bass; normalized/resampled arrays; final layout and serialization. Save primitive-level edge fixtures too.
4. **Golden format.** JSON manifest plus little-endian float64 binary arrays, shape/dtype/units/axes/checksums; explicit real/imag arrays for complex data. NPY is convenient for Python, but do not make a native implementation parse pickle or Python objects. Record expected nonfinite masks for magnitude spectra rather than replacing every −inf with an arbitrary epsilon.
5. **Compare semantics before values.** Exact sample rates, lengths, channels, speaker/ear order, silence slots, filenames, event status/sequence and selected grid multiples. Reject unexpected NaN/inf. Require input files/buffers unchanged. A whole-buffer allclose that broadcasts shapes is unacceptable. NumPy's default allclose is asymmetric, broadcasts and has an inappropriate default atol for near-zero data. [35]
6. **Localize differences.** Report first failing stage, worst channel/index/frequency, max/RMS error, reference scale, timing and output metadata. No post-hoc time/gain alignment in the acceptance comparator: it can hide an actual regression. Diagnostic aligned comparisons may be supplementary only.

**Provisional gate budgets, not empirically validated tolerances:**

| Comparison | Initial proposed gate | Important restriction |
|---|---|---|
| Pure scalar/array primitives, coefficients, FFT round-trip | finite values `abs(a-b) <= 1e-12 + 1e-10*abs(reference)`; exact shape and nonfinite policy | Tighten/adjust by operation and dynamic range after measuring a correct port; no blanket global increase |
| FFT/convolution/FIR stages | normalized RMS error <=1e-8 and peak error <=1e-7 of reference peak; compare transfer curve separately | Silent input uses absolute bound; unit/gain scales documented; long ill-conditioned cases need their own budget |
| Stage transfer magnitudes | <=0.01 dB in declared meaningful passband | Deep-null bins use absolute complex/spectral error with recorded floor, not unbounded dB ratios; inspect phase too |
| Alignment/peak/crop/layout | exact integer decisions and lengths | A changed rounding/tie rule fails; do not permit ±2 samples globally |
| Full BRIR before quantization | normalized RMS <=1e-6, max <=1e-5 of reference peak; <=0.01 dB meaningful response difference | Provisional cumulative ceiling; phase/ITD/decay checks also required, not merely audible similarity |
| Post-quantization WAV | exact subtype/rate/channels/frames/layout; decoded-sample error budget tracked in PCM LSBs | PCM_32 quantization may differ by LSBs despite close doubles; whole-file hash remains for same-backend determinism |
| PEQ optimizer | bounds/finite result/evaluation cap exact; compare fitted curve, weighted objective and convergence diagnostics | Parameter order/individual fc,Q,gain need not match when solutions are equivalent. Start with <=0.05 dB RMS extra fitting error and <=0.2 dB maximum curve difference, then calibrate on corpus; do not declare these hearing thresholds |
| Plots/HTML | exact input analysis arrays/axis units; visual regression with font/platform budget; all expected panels/hover/legend/link behavior | No cross-renderer pixel-identity promise; run offline, check asset inclusion and headless PNG output |

**Fixture expansion:** every five existing BRIR scenario; 44.1/48/96 kHz; 1/2/8/16 output speaker selections; odd/even/tiny buffers; silence/impulse/noisy/low-SNR/clipped recordings; appended measurements; per-ear EQ; microphone correction explicitly enabled with headphone compensation disabled; all balance/decay choices; EqAPO include cycles/conditions/filter defaults/convolution; sweep snap boundaries and false onset candidates; resampling rational ratios; every output layout recursively. Capture the **actual nnresample coefficient construction, gain, padding, phase and trim convention** before implementing a replacement; the package implementation itself was not inspected in this report.

**Concurrency:** repeat native runs at workers=1/2/maximum, compare same-build output hashes and all nested output paths; instrument races/memory where target tooling permits. Keep deterministic reduction order. Native compilation does not make global EQ context or plotting objects thread-safe. Cancellation tests cover pending/running/finalizing states, bounded stop latency, no half-written final outputs and callbacks after job termination.

### 3.3 Migration strategy comparison

| Strategy | What actually changes | Parity harness | i18n transfer | PyPI consequence | Judgment |
|---|---|---|---|---|---|
| **1. Big bang**, new branch/repository; Python frozen 2.x LTS | New core/service/shell/output stack before users switch; retained web assets still possible | Build a language-neutral golden/CLI harness first; run native candidates against pinned Python reference throughout, not only at the end | Copy all 11 catalogues and validation rules; adapt native formatting/settings; retain web t/fmt logic | Keep 2.x package published; native app release is separate. A native Python extension/wrapper is additional work, not automatic | Highest integration/verification risk. New repo also duplicates issues/release policy. Prefer same repo if chosen. Freeze means no new features, not abandoning security/packaging fixes |
| **2. Native DSP through PyO3/nanobind**, shell later | Port tested kernels and then stages behind Python adapters; Python orchestration/rendering/GUI remain until replacements pass | Existing tests run on reference and native backend; capture intermediate arrays; preserve old SHA tests alongside cross-backend tolerance gates | Catalogues, Python formatter and JS unchanged initially; only native diagnostic-key schema is added; later port settings/formatter | Strongest continuity: mixed wheel or optional native backend with Python fallback; existing CLI/imports retained; native wheel platform/Python matrix added | **Recommended.** Fastest discovery of numerical mistakes and smallest reversible units. Temporary dual implementation/build cost is real |
| **3. Shell first**, Tauri/Wails/Electron + Python sidecar | Replace pywebview/desktop host; ship Python scientific runtime as private worker | DSP can remain same-backend SHA exact. Add protocol/process/lifecycle tests and installed shell/audio tests. Later native port still needs the full stage harness | Frontend and all catalogues remain; bootstrap can still supply Python-resolved strings. Change only transport/window/runtime labels first | Existing package stays Python and can run without desktop shell. Native desktop build ships its own private runtime, not user's environment | Good only for urgent pywebview relief. It does not retire NumPy/SciPy/matplotlib packaging; replacing Nuitka with another freezer is not eliminating Python deployment |

**Effort estimates [I], agent-driven:** one focused maintainer-week means ~25–35 hours of specification, review, verification and integration with AI implementation assistance; not 40 hours of typing and not unattended agent wall time. Assumes existing DSP requirements retained, web UI reused, no new measurement algorithm, three-platform builds with Windows-first hardware testing. Estimates are broad planning priors; no project port pilot was performed. Renderer completeness, hardware access and dependency choices can move totals substantially.

| Strategy | Useful intermediate result | Native core + retained-web production app | Full vendored AutoEQ API/optimizer tail |
|---|---|---|---|
| Big bang | Harness and one thin vertical slice in 3–6 weeks | C++ 22–38 weeks; Rust 24–42; C# 24–42; pure Go 32–52 | Add 3–7 C++ weeks, 4–9 Rust, 4–8 C#, 6–12 pure Go if deferred from core scope |
| Native DSP through bindings | First tested kernels in 3–5 weeks; meaningful native BRIR slice in 8–14 | C++ 16–28 weeks; Rust 18–32; C# 22–38; pure Go 28–46 (less direct Python-binding route) | Same tail ranges; reduced if optimizer included early, never count twice |
| Shell first | Tauri 4–7 weeks, Electron 4–7, Wails 5–9 for production-oriented sidecar shell/update work | Then approximately incremental native migration cost; allow 2–5 weeks overlap saving, not a second free rewrite | Same numerical tail remains |

[I] C#/Go binding routes were not validated like PyO3/nanobind. For them, a native CLI/IPC adapter is the initial differential interface. That makes fine-grained Python test reuse less convenient, even though bulk buffer interoperability through a C ABI is possible.

[I] AI changes implementation throughput more than verification throughput. Require small self-contained work packets: exact permitted files, function/array/ownership contracts, frozen fixtures, reference algorithms and foreground gates. Budget roughly half the maintainer effort for specifications, reviewing semantic differences, running gates and installation/hardware checks. Do not parallelize two agents rewriting a shared numerical convention independently. C++ and SciPy have extensive primary algorithm/API documentation; Rust binding/ownership docs are strong, but the combined SciPy-replacement recipe is less complete. These are qualitative evidence-based judgments, not measured LLM benchmark claims.

### 3.4 Milestones and stop/go criteria

| Milestone | Indicative timing for recommended path | Exit criterion |
|---|---|---|
| M0: Contract freeze/provenance | Weeks 1–2 | Five scenarios inventoried; output subtype contradiction resolved; exact vs tolerance policy approved; golden manifest reproducible |
| M1: Numerical/build pilot | Weeks 2–4 | C++/nanobind and Rust/PyO3 pilots run the same FFT→smoothing→minimum-phase fixture, CI builds/imports on three OSes; one hardware recording probe per required host family where hardware exists |
| M2: Primitive library + outputs | Weeks 4–8 | FFT/convolution/windows/spline/SOS/WAV/layout/recovery contracts pass; CLI and Python adapter share core; no numerical globals |
| M3: Production BRIR path | Weeks 8–14 | Estimator, HRIR/decay, room/headphone EQ, virtual bass, microphone correction and resampling pass all stage/end-to-end gates; Python renders reports |
| M4: Native analysis/render/service/audio | Weeks 12–22, partly overlapping | All promised PNG/HTML data and interaction tests; job/cancellation/events; device/recording matrix; shell-independent CLI completes offline |
| M5: Shell and installed update migration | Weeks 18–28+ | Retained web UI works with transport adapter; fonts/9 locales/resources; previous Velopack install upgrades to candidate and restarts; final packaged smoke tests, macOS dmg and Linux artifact checks |
| M6: Retire desktop Python dependency | Only after M3–M5 | No Python scientific runtime needed by native desktop/CLI; 2.x LTS/PyPI policy explicit; deferred AutoEQ API scope either shipped or clearly retained in Python |

[I] The M1 pilot is a real stop/go decision. If Rust needs an unverified optimizer/FIR stack, choose C++ or keep that component Python longer; if C++ wheels/builds become more expensive than its numerical benefit, choose Rust. Do not select a stack by a hello-world installer screenshot.

**First three concrete deliverables**

1. **`migration-contract-v1` specification and golden manifest.** Pin reference SHA/environment; enumerate stage arrays, channel layouts, all five CLI scenarios and all output files; document PCM_32 vs requested FLOAT, nonfinite/tie/default semantics and tolerance budgets. Include proposed `tests/migration/` protocol without changing production behavior.
2. **A dual-backend differential runner plus one comparative native pilot.** Same fixture through Python and C++/Rust for magnitude response, Savitzky–Golay edges, log-frequency interpolation and minimum-phase FIR. Emit first-divergence diagnostics; run wheels/CLI on Windows/macOS/Linux. Choose core language from test results and dependency/build audit.
3. **One releasable opt-in native slice behind the Python API.** Begin with array primitives/output-layout recovery, then wire the tested FIR path; retain Python fallback and unchanged default BRIR behavior until gates pass. Exercise cancellation/ownership and recursive output comparisons; publish only after normal project gates. This is a product slice, not a second UI prototype.

### 3.5 i18n, PyPI and release continuity

**i18n:** keep the 433-key catalogues byte-for-byte first, including CLI/report/APO strings and the two Chinese aliases. Reuse key/placeholder/alias tests; do not convert everything to a target-language translation framework. Preserve named placeholder and fallback behavior. Move settings to a versioned schema and migrate existing language/theme/skin choices. Remove `frontend=ctk` only from the new product. Current gallery covers en/ko rather than all nine languages; extend glyph, long-string and error-state coverage before claiming complete localization. Known locale-prefix and unfilled-placeholder quirks should be separately classified as bug fixes, not deliberately preserved numerical contracts. [V source: i18n/localization.py:40,162,190; app.js:1013,1028; tests/test_i18n_locales.py; build_scripts/webview_gallery.py, in [3][4].]

**PyPI:** [V] current package promises Python >=3.9,<3.15 and has four console scripts including the legacy GUI; wheel construction uses Hatchling/custom hook. [4] Keep existing package/imports/CLI usable during migration. A native wheel adds OS/architecture/Python ABI build obligations, and free-threaded wheels need explicit testing. Prefer an optional native backend initially so old/unsupported interpreters retain Python behavior. Removing CTk from the new standalone app does not require deleting the legacy script from the maintained 2.x package. Decide whether 3.x PyPI is a native core plus Python API, or whether PyPI remains 2.x; do not surprise pip users with a GUI-only installer.

**Release:** keep `publish.yml` and `environment: PyPI` if the existing Trusted Publisher is retained. Maintain release gating and exact release SHA checkout. Add native wheel builds before publish and adapt standalone jobs; do not rewrite CI merely to express it in the new language. If native desktop and Python LTS versions diverge, define separate release products/channels deliberately instead of letting a Python patch auto-bump release a different native product. Test the final installed/package form, not only the pre-installer executable. Keep Velopack first, preserving app identity/feed/layout and testing a 2.x→native upgrade; switching to Tauri updater later is a separate compatibility project. [V existing release behavior in [4]; tool support in [25][30][31].]

## 4 Risks ranked

| Rank | Risk | Why it matters | Control [I] |
|---|---|---|---|
| 1 | Silent numerical drift | [V] spline k=2 exists beyond the brief's k=1/3 list; FFT-length families, FIR/minimum-phase construction, optimizer defaults and resampler trims are specific. [1] | Stage goldens, exact indices/shapes and calibrated numerical budgets; no global tolerance relaxation |
| 2 | Premature packaging victory | [V] Tauri sidecar docs still require Python packaging; Python plots currently run in normal output paths. [1][24] | Define Python-free completion by a dependency audit and offline native CLI run producing every output |
| 3 | Physical recording failure | [V] PortAudio capabilities depend on backend/hardware; two-stream ASIO operation is not universal. [22] | 16-out/2-in device/rate matrix, host API grouping/default selection, disconnect/error/stop tests; do not promise ASIO unconditionally |
| 4 | Breaking installed updates/PyPI | [V] existing Velopack and OIDC/release identities; nanobind 3 requires >=3.10 while project supports 3.9. [4][11][30][31] | Explicit compatibility matrix/fallback, installed upgrade test, preserve package identity and publish binding |
| 5 | Incomplete plot parity | [V] analysis math is mixed into plot code and PNG/Bokeh time origins differ. [1] | Numeric analysis schema first; headless PNG and offline HTML interaction/asset tests |
| 6 | Native races/FFI lifetime mistakes | [V] current EQ initializer context and logger callbacks are shared; native binding docs require ownership/threading discipline. [1][8][9] | Per-job owned state, detached compute, deterministic reductions, sanitizers/race checks, cancellation stress |
| 7 | Optimizer consumes the schedule | [V] complete AutoEQ fitting is complex but absent from normal BRIR path. [1] | Defer full optimizer while preserving Python API; separate compatibility criteria and budget |
| 8 | Porting obsolete code instead of behavior | [V] CTk is ~10k lines; several native-runtime-inapplicable re-export/probe modules exist. [2][3] | Drop implementation-only machinery; retain behavior fixtures and 2.x maintenance |
| 9 | Unreviewed dependency/license change | [V] FFTW and Qt Graphs have GPL/commercial terms; newly chosen stacks have separate transitive packaging obligations. [19][34] | License/SBOM audit before adoption; preserve vendored/data/font notices; do not assume an open-source project has no license constraints |
| 10 | AI-generated plausible substitutions | [I] generic FFT/window/LM/resampler implementations can pass happy-path examples while changing this program | Reference-linked task packets, independent differential tests, small merges, maintainer-owned numerical acceptance |

## 5 Open questions you could not settle

1. **[U] Native numerical parity has not been measured.** All proposed thresholds and effort estimates need M1 calibration; no cross-language executable was built here.
2. **[U] Exact target dependency set is undecided.** In particular, no verified pure Rust/Go drop-in covers the entire requested SciPy surface, including bounded TRF; this is not a claim that no such library exists.
3. **[U] nnresample's installed implementation/version was not inspected.** Its actual Kaiser design, edge padding, scaling and trimming must be captured before replacing it.
4. **[U] R3 conflicts with existing PCM_32 output.** Should v3 default remain PCM_32, add FLOAT as an option, or intentionally change output subtype? Decide separately from numerical tolerance.
5. **[U] Complete AutoEQ API compatibility scope is unclear.** Is the bounded PEQ optimizer required in the first native application, or only in the eventual importable library? Existing call sites support deferring it for BRIR.
6. **[U] ASIO and independent input/output stream behavior on the maintainer's target hardware is untested.** Backend enumeration alone cannot establish successful 16-channel recording sessions.
7. **[U] Renderer choice remains open.** Need a pilot for all six diagnostics, synchronized axes, waterfall/spectrogram, offline interactive reports and headless PNG. ScottPlot/Qt feature listings do not prove Bokeh replacement.
8. **[U] Windows upgrade continuity is not established by SDK existence.** Actual Velopack package identity, startup hook, feed/version and installed-directory transition need a 2.x installation test.
9. **[U] PyPI lifetime and interpreter promises require a maintainer decision.** Latest nanobind cannot preserve Python 3.9 without a fallback/older binding; native product and Python LTS version policies need definition.
10. **[U] Wails v3 status was not fetch-verified.** v2 installation evidence was available through official search snippets; direct pages were blocked. Do not use current-search beta claims as a tested stable release guarantee.
11. **[U] No complete license/SBOM audit was performed.** Data/font notices were inventoried, not legally adjudicated; candidate native dependencies must be checked against the repository license.
12. **[U] Shell-only edge behaviors are not uniformly tested today.** Transport rejection during polling, legacy GUI help/file detection, Japanese font fallback and all-language gallery coverage require explicit acceptance decisions.
13. **[U] `{TMP}` could not be queried through the blocked read-only shell calls.** The report uses `C:/Users/32170336/AppData/Local/Temp`, the temporary-directory root present in this session's tool-generated output paths; the dedicated Glob confirmed no existing report there before writing.

## 6 Sources

**Repository evidence [V].** Inspected local files are authoritative for counts and behavior. Links use the inspected immutable commit; individual table anchors are local line references. No remote repository content was uploaded.

1. [Core and AutoEQ at inspected revision](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/core), [AutoEQ](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/autoeq). All 42 Python files read. Key anchors: [PCM writer](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/core/audio_io.py#L82), [BRIR FIR worker](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/core/parallel_workers.py#L124), [bounded fit](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/autoeq/frequency_response.py#L350).
2. [Application](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/application), [updater](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/updater), [infra](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/infra), [CLI](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/impulcifer.py), [shell](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/impulcifer_webview.py). All modules in inventory read.
3. [GUI](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/gui), [i18n](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/i18n), [web UI](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/webview_ui). All requested source files read; fonts/logos/data inventoried.
4. [Tests](https://github.com/115dkk/Impulcifer-pip313/tree/0144dcc1f8a4634607b980d15c64549d4aa6c145/tests), [BRIR integrity](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/tests/test_brir_integrity.py), [project metadata](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/pyproject.toml), [publish workflow](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/.github/workflows/publish.yml), [ADR](https://github.com/115dkk/Impulcifer-pip313/blob/0144dcc1f8a4634607b980d15c64549d4aa6c145/docs/adr/0001-native-frontends-stay-native.md). Tests inventoried, relevant bodies read, not executed.

**External primary sources, accessed 2026-09-07.** Versions below are what fetched documentation identifies, not guaranteed newest releases.

5. [PyO3 0.27.1 parallelism](https://pyo3.rs/v0.27.1/parallelism.html).
6. [PyO3 main free-threading guide](https://pyo3.rs/main/free-threading). Main-branch documentation; release pin required.
7. [Maturin guide](https://www.maturin.rs/). No exact current release displayed; mixed-package/wheel capabilities verified.
8. [nanobind ndarray](https://nanobind.readthedocs.io/en/latest/ndarray.html). Latest documentation, ownership/conversion rules.
9. [nanobind free-threaded Python](https://nanobind.readthedocs.io/en/latest/free_threaded.html). Support since 2.2.0; latest includes future/provisional ABI caveats.
10. [nanobind packaging](https://nanobind.readthedocs.io/en/latest/packaging.html). CMake/scikit-build-core and ABI matrix.
11. [nanobind changelog](https://nanobind.readthedocs.io/en/latest/changelog.html). 3.0.1 dated 2026-08-28; undated 3.1.0 not treated as released.
12. [SciPy least_squares](https://docs.scipy.org/doc/scipy/reference/generated/scipy.optimize.least_squares.html). Fetched reference identifies 1.18.0.
13. [Ceres nonlinear least squares](https://ceres-solver.readthedocs.io/latest/nnls_solving.html). Latest docs, no exact release displayed.
14. [argmin TrustRegion](https://docs.rs/argmin/latest/argmin/solver/trustregion/struct.TrustRegion.html). 0.11.0.
15. [Gonum optimize](https://pkg.go.dev/gonum.org/v1/gonum@v0.17.0/optimize). v0.17.0.
16. [Math.NET LevenbergMarquardtMinimizer](https://numerics.mathdotnet.com/api/MathNet.Numerics.Optimization/LevenbergMarquardtMinimizer.htm). Assembly 5.0.0.0.
17. [RustFFT](https://docs.rs/rustfft/latest/rustfft/). 6.4.1.
18. [FFTW FAQ, numerical behavior](https://www.fftw.org/faq/section3.html). FFTW3 family; FAQ dated 2014-03-04, not a current release claim.
19. [FFTW FAQ, licensing](https://www.fftw.org/faq/section1.html). GPL/commercial alternatives.
20. [SciPy firwin2](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.firwin2.html). 1.18.0 reference.
21. [SciPy minimum_phase](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.minimum_phase.html). 1.18.0 reference; half option added in 1.14.
22. [PortAudio API overview](https://www.portaudio.com/docs/v19-doxydocs/api_overview.html). v19 URL, displayed API 2.0; exact installed library version not established.
23. [Tauri 2 configuration](https://v2.tauri.app/reference/config/#frontenddist). Static frontend directory and global API configuration.
24. [Tauri 2 sidecars](https://v2.tauri.app/develop/sidecar/). externalBin/target triples/Python executable packaging.
25. [Tauri 2 updater](https://v2.tauri.app/plugin/updater/). Required signatures and artifact formats.
26. [Electron native modules](https://www.electronjs.org/docs/latest/tutorial/using-native-node-modules). Latest documentation; no exact Electron release asserted.
27. [Electron utilityProcess](https://www.electronjs.org/docs/latest/api/utility-process). Node child, not arbitrary executable launcher.
28. [Node child_process](https://nodejs.org/api/child_process.html). Fetched page identifies v26.8.1; spawn/pipes and Windows termination caveats.
29. [Wails v2 installation](https://v2.wails.io/docs/gettingstarted/installation/). Official search evidence identifies v2.15.0; direct fetch blocked. [Wails v3 FAQ](https://v3.wails.io/faq/) also blocked; no verified stability conclusion.
30. [Velopack Rust integration](https://docs.velopack.io/getting-started/rust). Current guide, version examples are placeholders.
31. [Velopack C/C++ integration](https://docs.velopack.io/getting-started/cpp). Current guide, startup/C ABI/update workflow.
32. [Avalonia supported platforms](https://docs.avaloniaui.net/docs/supported-platforms). Current docs reference 12.x and desktop .NET 8+.
33. [ScottPlot](https://www.scottplot.net/). Version 5 documentation, PNG/headless and GUI integration examples.
34. [Qt6 Graphs](https://doc.qt.io/qt-6/qtgraphs-index.html). Qt6 family, 2D/3D and GPLv3/commercial terms.
35. [NumPy allclose](https://numpy.org/doc/stable/reference/generated/numpy.allclose.html). Fetched NumPy 2.5 reference; asymmetric tolerance, broadcasting, nonfinite behavior.
