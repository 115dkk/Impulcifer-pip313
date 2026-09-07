# P12 (ASTRA): `impulcifer-dsp::eqapo` — EqualizerAPO / EqualizerAPO-XT configuration parser (2.x `core/eqapo.py`)

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first: `E:/Impulcifer/core/eqapo.py` (the oracle, 940 lines: `looks_like_eqapo_config`, `parse_eqapo_config`, `EqApoEqualization`, `EqApoCommandReport`, the `_handle_*` command handlers, `_parse_biquad`/`_parse_iir`/`_parse_graphic_eq_nodes`, `_try_evaluate_condition`, `_parse_channel_scope`, `_ConditionFrame`, `_ParseState`, the include handling with depth and visited sets), its 2.x tests (`E:/Impulcifer/tests/test_eqapo*.py` if present; grep `eqapo` under `tests/`), the call site `E:/Impulcifer/core/pipeline_stages.py:199-291` (`_read_eq_settings`: how the result's `left_db`/`right_db`/`channel_split`/`applied`/`bypassed`/`skipped`/`preamp_*` are consumed and logged), and the Rust side you build on: `crates/impulcifer-dsp/src/stages/eq_files.rs` (the placeholder error you replace), `crates/impulcifer-dsp/src/filters.rs` (`rbj_*`, `biquad_response_db`, `tf2sos`, `sosfilt`), `crates/impulcifer-dsp/src/fr.rs`, `crates/impulcifer-dsp/src/fft.rs`.

Run every command in the foreground. Never use background execution. Another worker (P11) is editing `crates/impulcifer-service/**` and `crates/impulcifer-io/**`; do not touch those crates. Do not edit `lib.rs`, `features.toml`, `export_goldens.py`, `README.md`.

## Design rule
No file I/O in `impulcifer-dsp`. `Include:` and `Convolution:` need files, so the parser takes a loader the caller provides:

```rust
pub trait EqApoLoader {
    /// Text of an included configuration file (resolved against the including file's directory).
    fn read_text(&mut self, path: &Path) -> Result<String, String>;
    /// Sample rate and tracks of a convolution impulse response file.
    fn read_wav(&mut self, path: &Path) -> Result<(u32, Vec<Vec<f64>>), String>;
}
```
The service implements it with `impulcifer_io::read_wav` and `std::fs::read_to_string` (P11/P13). Tests implement it with an in-memory map. A `NoFilesLoader` that returns errors is provided for callers that cannot resolve files; the Python reports such lines as bypassed with the same messages.

## Scope: `crates/impulcifer-dsp/src/eqapo.rs` (declare `pub mod eqapo;` is already in `lib.rs`? It is not: add the file and ask the parent to add the `mod` line — do NOT edit `lib.rs`; instead put the module under `stages/eqapo.rs` and declare it in `stages.rs`, which you may edit.)

- `pub fn looks_like_eqapo_config(text: &str) -> bool`: `core/eqapo.py:168-189` exactly (the P10 placeholder in `eq_files.rs` already has a version; keep one implementation).
- `pub struct EqApoCommandReport { pub line_number: usize, pub command: String, pub reason: String }` (`eqapo.py:128-136`), `pub struct EqApoEqualization { pub left_db: Vec<f64>, pub right_db: Vec<f64>, pub applied_left: usize, pub applied_right: usize, pub preamp_left: f64, pub preamp_right: f64, pub applied: Vec<String>, pub bypassed: Vec<EqApoCommandReport>, pub skipped: Vec<EqApoCommandReport> }` with `pub fn channel_split(&self) -> bool` (`!= array_equal`, lines 149-166).
- `pub fn parse_eqapo_config(text: &str, srate: u32, frequency: &[f64], base_dir: Option<&Path>, loader: &mut dyn EqApoLoader) -> Result<EqApoEqualization, DspError>`: lines 565 to 940. Reproduce every handler: `Filter` (`ON`/`OFF`, the biquad types the Python accepts with their parameter grammars: PK/PEQ, LS/LSC/LSQ with slope or Q, HS/HSC/HSQ, LP/LPQ, HP/HPQ, BP, NO, AP and any others in `_parse_biquad`; the coefficient formulas of `_biquad_coefficients` lines 218 to 273 and the dB evaluation of `_biquad_gain_db` 275 to 284 must be transcribed, not replaced by `filters::rbj_*` unless you prove equality on the goldens), `Filter: ON IIR` (`_parse_iir`, `_evaluate_iir`), `Preamp`, `GraphicEQ` (node parsing and log-frequency linear interpolation of `_evaluate_graphic_eq`), `Convolution` (file via the loader, `_evaluate_fir` at the config sample rate; a sample-rate mismatch or unreadable file is reported the way Python does), `Include` (depth limit, visited set, relative resolution), `Channel:` scopes (`_parse_channel_scope`: `L`, `R`, `ALL`, numeric channel lists; commands outside the L/R scope are skipped with the Python report), `If:`/`ElseIf:`/`Else:`/`EndIf:` with `_try_evaluate_condition` (only the expression subset the Python evaluates; anything else makes the frame non-evaluable and the lines are reported as bypassed, lines 476 to 513), `Copy:`, `Device:`, `Stage:`, `Eval:`, comments (`#`), BOM handling and line-ending tolerance, the `_report_skipped_lines` bookkeeping and the maximum of bypass reports the caller prints (`_EQAPO_MAX_BYPASS_WARNINGS = 20` lives in the caller; keep the full list here).
- Every human-readable `applied`/`bypassed`/`skipped` string must match the Python text exactly (they are golden-compared), including number formatting (`%g`-style vs `repr`; read `_parse_double`, `_parse_freq` for the accepted numeric forms such as `1k`, `1.5kHz`, comma decimals if supported).
- Wire it into `stages/eq_files.rs`: `read_eq_settings(name, text, fs, frequency, base_dir, loader)` returns `(left, Option<right>)` FrequencyResponses like `_read_eq_settings` (`raw = db`, `error = -db`, right only when `channel_split`), plus the log report struct the service will translate (`applied_left/right`, `preamp_*`, bypass list, skipped count) so P11/P13 can emit `cli_eqapo_*` keys. Keep the plain-CSV path unchanged.

### Golden exporter and tests
`E:/Impulcifer/tests/migration/export_goldens_eqapo.py` (standalone, `py -3.14`), fixtures `tests/migration/goldens/p12_*`, documented in `tests/migration/README-eqapo.md`. Build a corpus of at least 20 configuration texts in the exporter (each stored in the fixture with the expected `left_db`, `right_db` on the `(10, 24000, 1.01)` grid at 48000 Hz, the counters, preamps, and the three report lists verbatim): one per biquad type at representative parameters, IIR, GraphicEQ with unsorted and duplicate nodes, Preamp positive/negative, Channel scopes (L only, R only, ALL, numeric, unknown), nested If/ElseIf/Else with evaluable and non-evaluable expressions, Include (two files in a temp dir, a cycle, a missing file, depth overflow), Convolution (a 64-tap IR WAV written with soundfile at 48000 Hz, one at 44100 Hz for the mismatch report, a missing file), a BOM-prefixed file, CRLF endings, comments, malformed parameter lines, an empty file, and the two real EqualizerAPO exports under `E:/Impulcifer/data/` or `tests/` if any exist (grep for `Filter:` in the repository's text files). Also `looks_like_eqapo_config` truth table on 12 texts.

Rust tests in `crates/impulcifer-dsp/tests/golden_eqapo.rs`: `golden_eqapo_corpus_matches_python` (dB arrays `atol 1e-9, rtol 1e-11`; counters, preamps and report strings exact), `golden_looks_like_eqapo_matches_python`, `golden_eqapo_include_and_convolution_match_python` (with the in-memory loader), and `crates/impulcifer-dsp/tests/properties_eqapo.rs`: `eqapo_rejects_recursive_include`, `eqapo_reports_unreadable_files_like_python`, `eqapo_channel_split_false_when_scopes_symmetric`, `eq_files_routes_eqapo_text_to_parser`.

Do not edit `features.toml`; report the test names for `eq.eqapo_config` (a new id) and `stage.equalization_files` (which should then drop its "EqualizerAPO returns the P12 error" note).

## Allowed files
`E:/Impulcifer/crates/impulcifer-dsp/src/stages/eqapo.rs`, `E:/Impulcifer/crates/impulcifer-dsp/src/stages.rs` (only the `pub mod eqapo;` line), `E:/Impulcifer/crates/impulcifer-dsp/src/stages/eq_files.rs`, `E:/Impulcifer/crates/impulcifer-dsp/tests/golden_eqapo.rs`, `E:/Impulcifer/crates/impulcifer-dsp/tests/properties_eqapo.rs`, `E:/Impulcifer/tests/migration/export_goldens_eqapo.py`, `E:/Impulcifer/tests/migration/goldens/p12_*`, `E:/Impulcifer/tests/migration/README-eqapo.md`. Nothing else. Fixture budget 12 MB.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; f64 only; no file I/O in `src/` (the loader trait is the only door).
- Reproduce the Python parser including its leniencies and its report wording; do not "improve" the grammar. List every deviation.
- Every function has a doc comment naming the Python function, line range and fixture.

## Verification (foreground, paste output)
```
py -3.14 E:/Impulcifer/tests/migration/export_goldens_eqapo.py
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --test golden_eqapo --test properties_eqapo --test golden_stages
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table corpus item → max dB error → report strings exact (yes/no); (4) Python behaviours reproduced and any you could not; (5) test names for the registry; (6) anything undone. Do not end your turn before the commands complete.
