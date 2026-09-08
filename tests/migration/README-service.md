# P11 service migration fixtures and acceptance tests

## P18 native platform queries (2026-09-08)

README timestamps now use `chrono::Local::now()` with Python's `%Y-%m-%d %H:%M:%S` format; per-job `Catalog.readme_date` injection and README parity tests are unchanged. Platform locale lookup uses `sys_locale::get_locale()`, preserving `LC_ALL`, `LC_CTYPE`, `LANG`, `LANGUAGE` precedence and `language_selected` semantics. First-run detection retains Python's case-sensitive, ordered prefix matching (all `zh` variants select `zh_CN`). Explicit saved-language normalization accepts `ko-KR`, `zh-Hans-CN`, `zh-Hant-TW`, and `zh-TW`; traditional Chinese remains `zh_TW`, while unsupported `pt-BR` selects `en`. This intentionally goes beyond Python's normalizer, which only changes separators and two-part casing.

System information is `<platform> <version> <arch>`: on Windows `os_info::get()` (registry/RtlGetVersion, no subprocess), on Linux the kernel release from `/proc/sys/kernel/osrelease` (what Python's `platform.release()` reports), on macOS the product version from `/System/Library/CoreServices/SystemVersion.plist` (Python reports the Darwin kernel release there, so that string differs), and `std::env::consts::ARCH` for the architecture. IPC response keys are unchanged. Existing FFmpeg absolute-path/batch-file restrictions and Windows `CREATE_NO_WINDOW` flag remain untouched.

This removes every process spawn from the service: `os_info` is a Windows-only dependency (it spawns `uname`, `lsb_release` and `sw_vers` on Unix, and 3.15 compiles an Objective-C helper on macOS), so Unix reads the files above directly. The new APIs require no C build on Windows/Linux/macOS. Chrono's optional Haiku target dependency does compile C++ (Haiku is not a supported application target).

New tests are `impulcifer-service::local_date_has_python_format_and_current_clock`, `impulcifer-service::platform_locale_maps_to_a_supported_language`, `impulcifer-service::os_description_names_the_platform`, and `impulcifer-policy::no_shell_subprocesses`. The policy scans raw and comment/string-stripped first-party source, with only the exact FFmpeg module exempt. A temporary service comment proved rejection and was removed. `Command::new(` counts only in files that reach `std::process`, so clap's `Command::new("impulcifer")` in the CLI is not a violation.

P11 is not complete: the strict English and Korean README byte gates fail. WAV checks pass on the measured Windows host. Do not mark the feature registry implemented on the strength of the WAV checks alone.

## Regenerating the oracle

From the repository root, run `py -3.14 E:/Impulcifer/tests/migration/export_goldens_service.py` in the foreground. Dependencies are the existing 2.x dependencies (including NumPy, SciPy, soundfile, tabulate and plotting dependencies). The exporter launches a separate CPython child per scenario, copies demo measurements to a `TemporaryDirectory`, and calls the actual `impulcifer.main` with the bundled sweep and either default options or `vbass=True, vbass_freq=250`. Real DSP and plotting execute; no Rust output is substituted into the oracle.

Observation wrappers capture the actual pipeline's crop/align, virtual-bass and EQ output for FR-left, EQ source curves, and pre-write README statistics. They do not replace DSP. Timing includes these small observation costs as well as real Python plots. The stored Python wall time is therefore diagnostic, not a controlled performance audit.

The exporter writes `goldens/p11_*` only. It captures `en` and `ko` README text, per-track SHA-256 over little-endian float64 PCM-decoded samples, first/last 256 samples, max absolute value, RMS and absolute-maximum peak index. Full FL-left, FL-right and FC-left tracks are stored for both output layouts. Diagnostic FR-left fixtures preserve unquantized float64 data. The complete fixture set must remain below 12 MiB.

The timestamp is fixed at `2026-09-07 12:00:00` through a child-local Python clock wrapper and a per-job Rust `Catalog.readme_date`. Goldens use LF. Tests convert expected text to the host-native newline before comparing bytes (CRLF on Windows, LF elsewhere). No numerical strings are normalized. The oracle host has tabulate 0.10.0 without wcwidth (`WIDE_CHARS_MODE=False`); installing wcwidth changes Korean column widths and is not the frozen formatting environment.

## Numerical contracts

For default and virtual-bass scenarios, sample rate, track count, frame count and absolute-maximum peak indices are exact. Both scenarios currently produce 33599 frames at 48000 Hz, 14 HeSuVi tracks and 16 hexadecagonal tracks. Full stored tracks and every track's first/last 256 samples use `atol = 1e-3 * oracle max_abs`; max-absolute and RMS ratios use `rtol = 1e-4`. Zero tracks must stay zero. SHA-256 is diagnostic rather than a Rust equality assertion. Canonical position fixes the untagged track names; compact outputs separately test ICHL names against canonical order.

README byte equality is a separate, stricter gate. Neither its numbers nor its tolerance have been changed to conceal DSP differences.

### Locating the current README blocker

Run `cargo test -p impulcifer-service --test readme_trace -- --nocapture` to reproduce the trace. It checks auto and explicit bundled sweep samples and inverse filters for exact equality. Both use 295270 sweep samples at 48000 Hz. Room and headphone inputs are present; EQ sidecar inputs are absent. The demo discovery order is `BL,SL.wav`, `FC.wav`, `FL,FR.wav`, `SR,BR.wav`.

Observed FR-left maximum absolute sample differences against actual Python stage output are:

| Stage | Default | Virtual bass |
| --- | ---: | ---: |
| crop/align (24000 samples) | 3.469446951954e-18 | 3.469446951954e-18 |
| virtual bass (24000 samples) | not enabled | 1.537398680584e-16 |
| EQ (33599 samples) | 2.974321141758e-7 | 2.646035247005e-7 |
| normalized, unquantized | 1.960917079130e-7 | 8.880954840181e-9 |

EQ source curve differences are at most 5.755396159657e-13 dB for room correction and 4.277467269276e-12 dB for headphone compensation; the target is exact. This excludes a different estimator or missing service inputs as the cause of the observed discrepancy. The substantial numerical divergence appears in EQ, not the service's README formatter.

For default FR-left, Python has noise floor -132.558376228099 dB, knee 32189 and PNR 85.440125509738 dB; the Rust pipeline has -132.587001403996 dB, knee 32187 and PNR 85.468515687618 dB. These print as 85.4 and 85.5. The same unquantized Python samples passed directly to Rust `decay_params` reproduce Python's peak, knee, window and noise floor (noise floor within 1e-9 dB, asserted). This diagnostic does not feed Python data into the production pipeline or replace the failing README gate.

For virtual-bass FR-left, Python has noise -150.190784731626 dB, knee 32971, PNR 76.687914965681 dB; the Rust pipeline has -153.317528434256 dB, knee 32536, PNR 79.814428802238 dB. Again, identical Python samples through Rust decay analysis reproduce Python's values. Across virtual-bass rows the largest observed knee difference is 634 samples (SL-left, 13.2083 ms), and the largest PNR difference is 3.126514 dB (FR-left). SR-left late reflection prints -13.89 dB in Rust versus -13.90 dB in Python; other printed reflection values agree. The exact low-level cause of that rounding boundary was not isolated independently.

Actionable DSP locations, which P11 must not modify:

- `E:/Impulcifer/crates/impulcifer-dsp/src/stages/equalize.rs:52-77` combines the curves, smooths/equalizes them and builds the minimum-phase FIR; lines 95-102 apply the FIR.
- `E:/Impulcifer/crates/impulcifer-dsp/src/fr/processing.rs:240-287` builds the FIR, including the zero-Nyquist magnitude and `minimum_phase` call.
- `E:/Impulcifer/crates/impulcifer-dsp/src/fir.rs:161-176` performs homomorphic conversion. Compare these intermediates with actual SciPy output before changing anything. This trace identifies the EQ stage, not a proven individual erroneous operation.
- `E:/Impulcifer/crates/impulcifer-dsp/src/stages/readme.rs:51-81` analyzes the post-EQ data. Its noise-floor sensitivity exposes tiny waveform differences; the identical-input test does not identify a decay implementation defect.
- `E:/Impulcifer/crates/impulcifer-dsp/src/pipeline.rs:249-254` creates immutable `ReadmeData`. Service rendering cannot legitimately fix those numbers without changing the DSP result or the contract.

## Discovery and actual 2.x differences

Discovery sorts filenames for cross-platform determinism. Python uses `os.listdir`. Unique speaker recordings produce the same associations, but calling the difference unconditionally harmless would be incorrect: duplicate speaker recordings use last-writer-wins merge semantics, so sorting can change the winner. Room correction also centers relative to its first discovered measurement; reordered room input can change that reference. The tested Windows demo has the same relevant order. Do not record both the combined surround sweep and overlapping stereo groups into one measurement directory.

The packet explicitly requires `test.wav` from `generate_sweep_set`. It is retained. There are five playback files in the returned `files` list and six WAV files on disk; `play_path` remains the first stereo file. Actual `core/sweep_set_generator.py:98-117` writes only the five playback files. This is an explicit conflict with the packet's general ban on extra files, not an accidental omission of the sidecar from the returned playback list.

The actual 2.x logger emits uppercase INFO/WARNING/ERROR/SUCCESS and a PROGRESS log immediately before each progress event. `logger.step` truncates to integer percent before the service divides by 100. P11 preserves this, rather than the packet's lowercase-level and exact `step/total` wording. Progress has `progress`, `message`, `key`; logs have `level`, `message`, `key`. Arguments are formatted but not forwarded as an `args` payload, matching the Python service. When an explicit output sample rate equals the input rate, Python includes the stage in the total but skips its progress callback; P11 does the same.

IPC decay values are seconds (positive numbers or per-channel objects). Actual `_validate_brir_request` rejects CLI strings such as `FL:300`. P11 preserves the existing webview IPC rather than implementing the packet's contradictory CLI-millisecond parsing instruction. File-writing exceptions are INTERNAL_ERROR, not OUTPUT_MISSING. The latter is only the post-success absence check. Missing directory/recordings map to FILE_NOT_FOUND per the packet; Python's no-recordings ValueError instead becomes INTERNAL_ERROR. EqualizerAPO input is INVALID_REQUEST with its filename until P12.

## Logs and write timing

Available catalogue logs include virtual-bass INFO/status completion and frequency warnings, per-channel decay count, TrueHD insufficiency warnings and post-write successes, JamesDSP success, per-speaker Hangloose INFO and final success. No success is emitted before the corresponding file write succeeds. The virtual-bass completion event is emitted at the next successful DSP stage boundary.

A default run has eleven progress keys in this order:

```
cli_creating_estimator
cli_running_room_correction
cli_running_headphone_compensation
cli_creating_equalization
cli_creating_target
cli_opening_measurements
cli_cropping_responses
cli_equalizing
cli_normalizing_gain
cli_plotting_results
cli_writing_brirs
```

Each is preceded by its PROGRESS log. Other default logs are `cli_starting_brir_generation`, headphone default/using-file messages, parallel-executor and parallel-EQ messages, and the plot placeholder warning.

Remaining log limitations are intentional and must not be claimed as parity: Python emits raw README content and several uncatalogued normalization/microphone messages. P11 does not invent catalogue keys for those. `cli_info_parallel_executor` uses truthful Rust runtime metadata rather than pretending to run a Python executor. `cli_plots_not_available_yet` is requested by the packet but absent from the catalogue; the parent must add it in all locales. Plot progress exists but plot files do not. The immutable DSP observer supplies stage entry only, with no output snapshots: final outputs are written after the complete in-memory run. Physical file order is preserved, but response/README write times and optional-success interleaving with progress cannot match Python's stage-local I/O with the settled interface. Do not redesign the DSP interface inside P11.

## Tests and registry coverage

Tests are in `E:/Impulcifer/crates/impulcifer-service/tests/` unless noted. These are candidate coverage mappings, not authorization to mark a failing feature implemented.

| Registry IDs | Tests and limits |
| --- | --- |
| `ipc.start_brir` | `start_brir_rejects_missing_dir`, `start_brir_emits_progress_with_cli_keys_and_total_eleven`, `start_brir_cancel_mid_run_reports_cancelled`, `start_brir_output_missing_when_write_fails`; demo README blockers remain |
| `ipc.detect_sweep` | `detect_sweep_reports_demo_parameters`, `snap_sweep_samples_matches_python` |
| `ipc.generate_sweep_set` | `generate_sweep_set_writes_named_files_and_sidecar` checks all playback samples and six disk WAVs |
| `stage.estimator`, `stage.room_correction`, `stage.headphone_compensation`, `stage.equalization_files`, `stage.target`, `stage.open_measurements` | `room_and_eq_inputs_use_real_measurements_and_write_responses`, `estimator_alias_generate_sidecar_and_auto_match`, demo parity tests |
| `stage.crop_and_align`, `stage.equalize`, `stage.normalize`, `stage.write_responses`, `stage.write_readme`, `stage.write_brirs` | both demo parity tests and `readme_numerical_trace_on_identical_oracle_samples`; README gates fail |
| `stage.virtual_bass` | `demo_vbass_matches_python_within_budget` (WAV checks pass, README fails) |
| `stage.mic_deviation_skipped` | `mic_deviation_is_skipped_with_headphone_compensation` |
| `stage.mic_deviation`, `stage.decay`, `stage.channel_balance`, `stage.resample` | `optional_dsp_stages_and_plot_placeholders_complete`; execution and resampling covered, individual mic/decay/balance numerical effects are not isolated |
| `stage.plot_pre`, `stage.plot_post`, `stage.plot_results`, `stage.plot_additional`, `stage.interactive_plots` | optional-stage test verifies progress and five placeholders only, not plot implementation |
| `stage.truehd_layouts`, `output.truehd_layout_wavs` | `truehd_layouts_succeed_end_to_end_from_measurements` creates height recordings from actual FC measurement, runs actual DSP through start_brir and compares every TrueHD output track against canonical hrir.wav; both successful layouts covered |
| `stage.jamesdsp`, `stage.hangloose`, `output.jamesdsp_wav`, `output.hangloose_wavs` | optional-stage test checks independently normalized JamesDSP and exact Hangloose FL samples; `output_variants_preserve_pcm32_tracks_names_and_write_order` checks all split tracks |
| `output.hrir_wav`, `output.hesuvi_wav`, `output.responses_wav` | demo/optional/output-variant tests; compact ICHL names and PCM32 conversion checked |
| `output.readme_txt` | `demo_readme_korean_bytes_match_python` and both demo parity tests; currently blocked |
| `output.recovery` | untouched, no P11 implementation or coverage claim |

`E:/Impulcifer/crates/impulcifer-io/tests/brir_layout.rs` covers four-track round trip, odd JSON padding and RIFF size, Python fixture reading, and invalid metadata.

## Isolation and permissions

Rust tests use atomically numbered, process-specific temporary directories with exclusive creation, separate settings paths and JobRegistry instances. Catalogue/date state is per job. No test mutates process environment, cwd, global clock or real user settings. The exporter sets environment only for subprocesses; Python locale/logger/clock wrappers live in isolated child processes. Concurrent exporter invocations must not target the same golden directory, and generation must finish before Rust tests read fixtures.

`output_readonly_file_failure_preserves_existing_bytes` verifies the Windows read-only file behavior and restores permissions. `start_brir_output_missing_when_write_fails` uses a directory occupying hesuvi.wav for deterministic cross-platform write failure. The added Unix-only `start_brir_readonly_directory_reports_internal_error` removes directory write bits, restores them via RAII, and explicitly reports a skip if the account bypasses them (for example root). It was not executed on Windows. A Windows directory's read-only attribute does not deny file creation; actual directory denial requires ACL changes and identity-specific security tooling. No ACL manipulation was added under the no-new-dependencies/no-unsafe constraint. Windows read-only-directory behavior therefore remains unverified; the file and collision tests are not described as substitutes for that permission test.

## Windows verification results (2026-09-07)

All six packet commands completed in the foreground. Export, format check, clippy and policy passed. `git status --short data/demo` produced no output. The aggregate service/IO gate failed only in the Korean README test before Cargo stopped running subsequent binaries. Both English demo tests were then run separately and failed only the README-byte check. No WAV comparison exceeded its budget. `ipc` (23), `sweep_grid` (1) and the numerical trace (1) passed when run separately. Optional-stage tests (3), including successful TrueHD layouts, passed. Unix directory permissions were not exercised on this Windows host. Service/IO doctests were not reached by the aggregate command; a separate `cargo test -p impulcifer-service -p impulcifer-io --doc` passed (zero doctests in either crate).

Exact final timing and fixture-size output:

```
P11 default: Python 8.445405s; [('hesuvi.wav', 33599, 14), ('hrir.wav', 33599, 16)]
P11 vbass: Python 8.316909s; [('hesuvi.wav', 33599, 14), ('hrir.wav', 33599, 16)]
P11 fixtures: 5357886 bytes (budget 12582912)
P11 default: Rust 20.164271s Python 8.445405s ratio 0.418830
P11 vbass: Rust 20.287081s Python 8.316909s ratio 0.409961
```

The largest checked sample errors are 9.750947356e-7 (default) and 4.656612873e-8 (virtual bass). Across nonzero tracks the default max-absolute ratio range is 0.999968562–1.000001504 and RMS range 0.999985096–0.999991181; virtual bass ranges are 0.999970225–1.000002789 and 0.999993615–0.999997760. Peak indices, counts, rates and lengths all match. These checks cover full stored tracks and the specified first/last samples, not every sample of every other track.

Complete local stdout/stderr, including per-track numbers and numerical traces, is retained at `E:/Impulcifer/crates/impulcifer-service/p11-oracle.log`, `p11-tests.log`, `p11-default.log`, `p11-vbass.log` and `p11-trace.log`. These are local command transcripts, not portable golden inputs. The reproducible tests and fixtures are the persistent verification contract. No commit, push or CI run was performed.

## Verification commands

Run all six packet commands in the foreground. The aggregate service/IO command stops at the failing Korean README binary, so run both demo cases separately with `--nocapture`, and run `ipc`, `sweep_grid` and `readme_trace` separately to cover later binaries. Debug Rust timings are not a release-mode performance result. No CI or cross-platform execution is implied by local Windows success.

```
py -3.14 E:/Impulcifer/tests/migration/export_goldens_service.py
git -C E:/Impulcifer status --short data/demo
cargo fmt -p impulcifer-service -p impulcifer-io -- --check
cargo clippy -p impulcifer-service -p impulcifer-io --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service -p impulcifer-io
cargo test -p impulcifer-policy
cargo test -p impulcifer-service --test demo_parity demo_brir_matches_python_within_budget -- --nocapture
cargo test -p impulcifer-service --test demo_parity demo_vbass_matches_python_within_budget -- --nocapture
cargo test -p impulcifer-service --test ipc --test sweep_grid --test readme_trace -- --nocapture
```

## README budgets (parent, 2026-09-08)

The English and Korean README checks compare text and table layout byte for byte and only the numeric cells with a tolerance (`tests/brir_support/mod.rs::readme_differences`). Two regimes were measured: with real room tails (default demo) the PNR moves by about 0.03 dB and the tail length by up to 2 samples through the minimum-phase EQ FIR noise (budget 0.1 dB / 2 ms, ITD exact); with virtual bass the tails are 8th-order filter roll-off at the 1e-8 level, so the Lundeby noise floor is numerical noise and PNR/length differ by up to 3 dB / 11 ms while the written WAVs still agree to 5e-8 (budget 5 dB / 20 ms). `readme_trace.rs` shows the Rust decay analysis reproduces the Python numbers exactly when fed the Python samples, so the README arithmetic itself is verified; the end-to-end numbers are limited by the estimator's sensitivity, not by the port.

### Measured oracle sensitivity of the README numbers (2026-09-08)

`tests/migration/oracle_noise_readme.py` runs the 2.x `core.decay.decay_params` on the Python-side final `FR-left` track of each scenario (`p11_<scenario>_readme_FR-left.f64`) and on copies perturbed by white noise of the stated amplitude (six draws each):

| scenario | tail RMS (second half) | 1e-9 noise: max dPNR / dlen | 5e-8 noise | 1e-6 noise |
|---|---|---|---|---|
| default | 1.19e-6 (-118.5 dBFS) | 0.70 dB / 0.04 ms | 26.0 dB / 21.3 ms | 52.2 dB / 284.8 ms |
| vbass | 2.35e-8 (-152.6 dBFS) | 37.2 dB / 104.1 ms | 71.1 dB / 387.2 ms | 97.2 dB / 484.9 ms |

The Lundeby estimate is a threshold search on windowed energies, so once the tail is at the numerical noise floor (the virtual-bass tails are 8th-order filter roll-off at 2e-8) a perturbation far below one PCM_32 LSB (4.7e-10) moves the reported PNR by tens of dB in Python itself. Byte-exact README parity is therefore not a property the 2.x oracle has, and the parity gate compares the README numbers with the budgets above while the WAVs (the actual outputs) agree to 5e-8.
