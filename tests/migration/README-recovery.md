# P17 output recovery oracle

## Reproduction

Run from the checkout root on a machine with CPython 3.14 and the existing Python dependencies installed.

```sh
py -3.14 E:/Impulcifer/tests/migration/export_goldens_recovery.py
cargo fmt -p impulcifer-service -- --check
cargo clippy -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service
cargo test -p impulcifer-policy
```

To run all service test executables even when an earlier executable fails, use `cargo test -p impulcifer-service --no-fail-fast`. The existing `deferred_methods_keep_not_implemented_envelopes` test in `crates/impulcifer-service/tests/ipc.rs` still includes `start_output_recovery`; P17 intentionally does not modify that file. Its old `INTERNAL_ERROR` expectation must be removed by the caller when registering P17.

The exporter creates its inputs and recovered outputs under `tempfile.TemporaryDirectory`, never under `data/demo`. Only `goldens/p17_recovery.json` is persisted. It contains the Python, soundfile and libsndfile versions. The initial reference environment is Windows, CPython 3.14.5, soundfile 0.14.0, libsndfile 1.2.2.

## Fixture format

Each scenario records its name, deterministic input recipe, explicit recovery request, and either the exact dataclass result or the exact recovery error code/message/details. Paths use `$ROOT`, with forward-slash separators. Tests substitute the temporary root and normalize separators; substantive error fields are not removed.

Each successfully created WAV has a SHA-256 digest over its complete contents, its rate/frame/channel dimensions, and its first and last 64 frames per channel. Inputs normally contain 4800 frames at 48000 Hz. Ordinary row `k`, numbered from one in `SPEAKER_NAMES`/left/right order, is `((i * 17 + k * 101) % 8192 - 4096) / 65536`. Zero, very quiet, nonfinite and rounding-boundary rows have explicit recipes in the exporter and Rust tests. Input rate/count overrides test mismatches. Raw ICHL payload recipes test invalid JSON, versions, duplicate chunks and malformed track arrays. No large reference WAV collection is needed; the fixture limit is 12 MiB.

The checked-in fixture has 97 scenarios and 25 request-validation cases. There are 39 successful recovery scenarios with 76 written outputs, 53 structured-error scenarios, one propagated Python `OSError`, and four host-specific skips. The success tests compare every result key, every output digest and every stored sample. Error tests compare full code/message/details, except for the explicitly marked broken-WAV library diagnostic. Private transaction tests consume the same oracle errors for injected staging/publish conflicts and metadata failure.

## Scenario coverage

- HRIR-only, HeSuVi-only and both combined sources; trackwise agreement, source disagreement, sample-rate and sample-count mismatches.
- Nested, loose and selected Hangloose directories; prefixed filenames; longest speaker suffix; Unicode case folding; case-insensitive named files/directories.
- Matching and mismatching existing Hangloose subsets, shared-prefix ambiguity, and duplicate speaker/file/directory names when the filesystem permits them.
- Combined-source precedence over raw measurements in the same directory, and over split files when Hangloose output is not requested.
- Positional base/extension layouts; every wrong count listed by `test_silent_extensions.py`; mono and non-stereo split rejection; non-silent LFE rejection.
- Compact HRIR and HeSuVi sources with and without compact output, one-ear mono maps, maps at the ambiguous 14/16-channel boundary, duplicate/unknown/count-mismatched names, bad JSON/version/size/duplicate chunks.
- Exact-zero trimming, quiet extensions, float-to-PCM rounding/clipping, silent combined and split sources, and compact all-silent failure. A complete silent pair has no write plan and succeeds even with compaction enabled.
- Invalid/missing directories, lexical parent resolution, empty audio, NaN/infinity and malformed WAV input.
- Output conflicts before staging and before publication, metadata-write failure, publication failure after the first output, and read-only directory behavior where mode bits are enforceable.
- IPC field validation, defaults and error precedence; non-cancellable `output_recovery` job kind; polling, result/error payloads and terminal event replay.

Combined sources list active speakers only; split sources list the speakers represented by files, including silent stereo files. This distinction is intentional. Recovery never performs gain adjustment, resampling or DSP.

## Publication and compatibility limits

Recovery uses a private ordinary RIFF PCM_32 encoder. The general Rust I/O writer emits WAVEX for multichannel data, whereas Python recovery explicitly uses soundfile's WAV format. Recovery's complete output bytes, including its 44-byte header and optional ICHL chunk, match the stored Python hashes.

Outputs are staged in their target directories. Publication first tries `std::fs::hard_link` rather than an overwriting rename: an existing destination makes hard-link creation fail atomically, including when it appears after the final conflict check. On filesystems without hard links (FAT/exFAT, some network shares) publication falls back to the rename Python uses (`os.replace` after the same conflict check), so recovery works everywhere 2.x does and only the check-to-rename window is shared with Python there. Temporary names and outputs created earlier in the transaction are removed on failure, but an output is removed only while it still carries the bytes this run wrote (length plus FNV-1a digest); a file another process replaced in the meantime is kept. Existing source files and the competing writer's file are preserved. Case-only concurrent filenames on a case-sensitive filesystem cannot be reserved atomically as one name through the safe standard-library API; the case-insensitive pre-publication check still runs.

Python's malformed-WAV `details.reason` comes from libsndfile; Rust's comes from the existing strict RIFF reader. Code, message and path match for the broken-WAV scenario, but its reason does not. Rust does not reproduce all libsndfile-supported encodings or its permissive handling of malformed RIFF chunks. Existing PCM16/24/32 and float32/64 decoding is used without changing the I/O crate.

Python wraps sample/metadata write failures but propagates some directory/tempfile/rename failures as ordinary `OSError`; its service then reports `INTERNAL_ERROR`. The fixed Rust API represents these failures with `OUTPUT_WRITE_FAILED` and retains the OS reason. The injected second-publication test verifies rollback and explicitly compares that retained reason rather than pretending the error envelopes match. OS diagnostics vary by platform and language.

The case-insensitive Windows host cannot create the three duplicate-case fixture setups. Windows directory read-only mode bits do not prohibit writes, so the fourth skip does not alter ACLs. These skipped scenarios remain in the exporter for regeneration on suitable hosts. No Windows ACL, macOS or Linux runtime test was performed for the initial fixture.

Home expansion uses Windows USERPROFILE, HOMEDRIVE/HOMEPATH and named-user rules. On Unix, HOME and local `/etc/passwd` entries are supported without unsafe code or extra dependencies; network NSS users and a missing HOME with a misleading USER variable are not fully equivalent to Python's `pwd` lookup. Recovery path strings cannot losslessly represent non-Unicode Unix filenames. Direct API home expansion is separate from IPC validation: as in Python, IPC checks the literal directory before starting a job.

The Python compaction warning uses its process-global logger. The Rust recovery API does not emit that localized console warning or add a new job event. Frontend wiring is already present in `webview_ui/app.js`; P17 changes no frontend or catalogue files.

## Registry test references

The caller can register `output.recovery` with these existing tests after reviewing the change.

- `impulcifer-service::golden_recovery_scenarios_match_python`
- `impulcifer-service::golden_recovery_errors_match_python`
- `impulcifer-service::recovery_never_overwrites_existing_outputs`
- `impulcifer-service::recovery_cleans_up_partial_writes_on_failure`

For `ipc.start_output_recovery`, use `impulcifer-service::recovery_ipc_start_and_poll_round_trip`.

`features.toml` is intentionally unchanged. Performance registration and whole-pipeline performance work are separate from P17 parity tests.
