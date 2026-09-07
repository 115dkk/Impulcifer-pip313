# P17 (ASTRA): `impulcifer-service::recovery` — output recovery by channel reordering (2.x `core/brir_recovery.py`) and the `start_output_recovery` IPC

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/core/brir_recovery.py` (the oracle, 631 lines) and its 2.x tests (`grep -l recovery E:/Impulcifer/tests/*.py`), `E:/Impulcifer/core/brir_layout.py`, `E:/Impulcifer/docs/rust/survey/pipeline-object-model.md` (section `core/brir_recovery.py`), `E:/Impulcifer/application/impulcifer_service.py` (`_validate_output_recovery_request`, `start_output_recovery` lines 496 to 517, the `asdict(result)` response), `E:/Impulcifer/webview_ui/app.js` (how the recovery screen calls `start_output_recovery`, polls the job and renders the result and the error codes), and the Rust side: `crates/impulcifer-service/src/{lib.rs,brir/}` (P11's file helpers and job wiring), `crates/impulcifer-io/src/{wav.rs,brir_layout.rs}`, `crates/impulcifer-types/src/{constants.rs,ipc.rs}`, `crates/impulcifer-jobs/src/registry.rs`.

Run every command in the foreground. Never use background execution. Do not edit `features.toml`, `webview_ui/`, `i18n/`, `crates/impulcifer-dsp/**`.

## Scope: `crates/impulcifer-service/src/recovery.rs` (+ `pub mod recovery;` in `lib.rs`)
- `pub enum RecoveryErrorCode { InvalidDirectory, NoRecoverySource, AmbiguousSource, InvalidWav, InvalidChannelMap, InvalidChannelCount, NonSilentLfe, SampleRateMismatch, SampleCountMismatch, SourceMismatch, AllChannelsSilent, OutputConflict, OutputWriteFailed }` with the exact 2.x wire strings, `pub struct RecoveryError { pub code, pub message: String, pub details: serde_json::Value }`.
- `pub struct RecoveryOptions { pub include_hangloose: bool, pub remove_silent_channels: bool }` mirroring the 2.x `recover_brir_outputs(**params)` keywords (read `_validate_output_recovery_request` for the accepted request keys and defaults).
- `pub fn recover_brir_outputs(dir: &Path, options: &RecoveryOptions) -> Result<RecoveryResult, RecoveryError>` reproducing the module: directory resolution (`_locate_output_and_split_dirs`, the `Hangloose` folder rule), case-insensitive `_find_named_file` with the ambiguity error, `read_wav(expand=True)` semantics through `impulcifer_io::read_wav`, the `ICHL` map via `brir_layout::read_track_names`, positional channel-count validation (`base <= count <= len(order)`, even), `NON_SILENT_LFE`, Hangloose split-file matching (`_match_split_stem` longest-first speaker suffix, one shared prefix, stereo, common rate and length), source precedence (`hrir + hesuvi` cross-verified track by track, then `hrir`, `hesuvi`, `hangloose`), the `ALL_CHANNELS_SILENT` check, and the writer (`_write_all`: temp file in the target directory + rename, `OUTPUT_CONFLICT` checked before and after, cleanup of partial files on failure, `PCM_32`, `compact_tracks` + `append_track_names` when `remove_silent_channels`). `pub struct RecoveryResult` = the 2.x dataclass fields exactly (read the module for names: written files, skipped files, source kind, speakers, sample rate, channel counts, whatever `asdict` returns) serialised with the same JSON keys.
- `lib.rs`: implement `IpcMethod::StartOutputRecovery` (validation rules of `_validate_output_recovery_request`, job kind `output_recovery`, not cancellable, result = `RecoveryResult` as JSON, `_ServiceFailure` mapping = the recovery error code string as the IPC `code`, message and details).

### Goldens and tests
`E:/Impulcifer/tests/migration/export_goldens_recovery.py` (`py -3.14`): build temp directories for every scenario the 2.x tests cover plus: hrir only, hesuvi only, both consistent, both inconsistent (`SOURCE_MISMATCH`), hangloose only (nested and loose), ambiguous names, wrong channel count, odd channel count, `ICHL` map present (with and without `remove_silent_channels`), non-silent LFE, all-silent, output conflict, a read-only output directory (`OUTPUT_WRITE_FAILED`) where the platform allows it; for each store the request, the `asdict(result)` or the error `(code, message, details)`, and the SHA-256 plus first/last 64 samples of every written file. Fixtures `p17_*` (budget 12 MB; use short synthetic tracks, 4,800 samples). `tests/migration/README-recovery.md`.

Rust tests in `crates/impulcifer-service/tests/recovery.rs`: `golden_recovery_scenarios_match_python` (result JSON exact, written files byte-identical since the writer is pure PCM_32 copying), `golden_recovery_errors_match_python` (codes, messages, details exact), `recovery_ipc_start_and_poll_round_trip`, `recovery_never_overwrites_existing_outputs`, `recovery_cleans_up_partial_writes_on_failure`.

Do not edit `features.toml`; report the test names for `output.recovery` and `ipc.start_output_recovery`.

## Allowed files
`E:/Impulcifer/crates/impulcifer-service/src/recovery.rs`, `crates/impulcifer-service/src/lib.rs` (the `mod` line and the `StartOutputRecovery` arm only), `crates/impulcifer-service/tests/recovery.rs`, `E:/Impulcifer/tests/migration/export_goldens_recovery.py`, `E:/Impulcifer/tests/migration/goldens/p17_*`, `E:/Impulcifer/tests/migration/README-recovery.md`. Nothing else.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe. The module never applies gain, DSP or resampling (2.x docstring).
- Error codes, messages and the result JSON are part of the frontend contract; match them exactly.
- Temporary directories only; never touch `data/demo`.

## Verification (foreground, paste output)
```
py -3.14 E:/Impulcifer/tests/migration/export_goldens_recovery.py
cargo fmt -p impulcifer-service -- --check
cargo clippy -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table scenario → result/error match (yes/no) → written files byte-identical (yes/no); (4) 2.x behaviours reproduced and any you could not; (5) test names for the registry; (6) anything undone. Do not end your turn before the commands complete.
