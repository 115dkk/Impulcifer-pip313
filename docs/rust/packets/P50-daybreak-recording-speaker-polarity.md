# P50: recording check, relative speaker polarity (ADR 0015 bundle 5)

Repository worktree: `E:\Impulcifer\.worktrees\s2r3` (Git Bash: `/e/Impulcifer/.worktrees/s2r3`), branch
`claude/tuning2-s2r-5`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\s2r3\CLAUDE.md`, section "3.x Rust 워크스페이스": `forbid(unsafe_code)`, no new
  dependencies, the `features.toml` gate, brir-3x byte identity, the IPC rules (a new field in a result or
  request is a shape change: update `ipc.d.ts` consumers through the later wiring packet, but every Rust test
  that asserts a shape is yours).
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\ALIGNMENT.md`, "Recording checks", subsection "Speaker polarity
  (3.17.0)": **the specification**, and the last bullet of "Registration". When this packet and the spec
  differ, the spec wins.
- `E:\Impulcifer\.worktrees\s2r3\docs\adr\0015-secs2r-improvements-into-tuning2.md` (Korean; the decision), item
  2's D55 bullet and its bundle-5 bullet.
- `E:\Impulcifer\.worktrees\s2r3\tests\migration\README-secs2r.md` and `README-secs2.md`: the fixture encoding
  and the four gate classes; `crates/impulcifer-dsp/tests/golden_secs2r.rs` implements them for bundles 1 to 4;
  extend it the same way.

Clean room: this check reproduces behaviour of a GPL program. You implement it from the specification and the
fixtures only. Do not search for, open, or ask for that program's source, and do not look for its repository on
this machine. If the spec is ambiguous, say so in the report and implement the reading the fixtures support.

## Fixtures

The new files are `tests/migration/goldens/s2r_polarity_*` (same encoding, exported under Debian glibc 2.41,
`meta.libm` says so). The oracle compares speakers through "positions"; every fixture that judges speakers uses
position 1 = the left ear (left and centre speakers) and position 2 = the right ear (right and centre speakers),
with equal weights (`meta.positions`, `meta.main_position`, `meta.main_position_absent_weight`), which is exactly
the spec's pairing. `meta.mains` is the speaker order (canonical). Field names and the spec's symbols:

| Field | Spec |
| --- | --- |
| `explicit`, `bin_weights.<fs>` (weight) | the band weight at `inputs.frequencies_hz`, and at every bin `k*fs/4096`, k = 0…2048 |
| `onset`, `peak`, `start`, `length`, `taper`, `window` (window) | `onset`, `peak`, `peak − s(1.5)`, `L`, `t`, the L tapered samples (unpadded) |
| `value`, `lag`, `energy` (pair) | the clamped `r`, its offset, `e`; all `null` when there is no value |
| `inputs.window_a`, `inputs.window_b` (pair) | the two windows to correlate (also derivable from `ir_a`, `ir_b`) |
| `windows[]` (demo, check) | per speaker and ear: as the window family |
| `compared_pairs[] {position, ear, a, b, value, lag, energy}` | one per pair and shared ear |
| `pair_results` (demo, check) | the decided averaged matrix in `meta.mains` order (`null` = undecided or not compared) |
| `notes[]` | `PolarityReversed {role, confidence, compared, opposite}` and `PolaritySplit {groups: [a, b], confidence}` in the order the verdict yields them |

The demo inputs are the first 4096 samples of the seven aligned in-ear demo BRIRs per ear, stored once as the
`s2r_polarity_demo_base` sidecars (`meta.base_prefixes`, `meta.prefix_derivation`); another variant multiplies
the speakers in `inputs.flipped` by −1 at both ears and uses the speakers in `inputs.included`. The synthetic
inputs are stored as sidecars and described in `meta`.

Families and their gate class (README-secs2r.md "Gates"; Exact and Curve tighten on the oracle's libm):

| Family | Cases | How the test feeds it | Class |
| --- | --- | --- | --- |
| `s2r_polarity_weight` | 16 frequencies, bins at 44.1, 48, 96 kHz | `band_weight` | Exact |
| `s2r_polarity_window_<case>` | `pulse`, `tie`, `early`, `short`, `silent`, `fs44100`, `fs96000` | `direct_window` on `inputs.ir` | indices exact; samples Exact |
| `s2r_polarity_pair_<case>` | `same`, `opposite`, `delay24`, `delay72`, `scaled`, `tweeter_reversed`, `silent` | `window_correlation` on the stored windows, and `direct_window` + `window_correlation` on the IRs (both must agree) | `lag` and presence exact; `value`, `energy` Curve |
| `s2r_polarity_demo_<variant>` | `base`, `flip_SL`, `flip_FR`, `flip_FC`, `flip_FL_FR`, `only_FL_FR`, `trio_flip_FC` | rebuild the windows, `pair_matrix` with `PolarityInput`s in `meta.mains` order (side from the speaker: FL, SL, BL left; FR, SR, BR right; FC, BC centre), `verdicts` | windows as above; per-ear values Curve, lags exact; the decided pattern exact, values Curve; notes: kinds, roles, groups, `compared`, `opposite` and order exact, `confidence` Curve |
| `s2r_polarity_check_<case>` | `minority`, `split_pair`, `two_reversed`, `components`, `undecided`, `centre_ears` | as the demo family on the stored synthetic IRs | as the demo family |

The tie rules matter and the fixtures exercise them deliberately: `window_tie` has two equal |x| in the search
range (the window takes the last, 300, while the onset comes from the first global maximum, 256), and
`window_silent` is all zeros (the search returns its last index; the correlation then has no value). On the demo
every same-side pair is decided (|r| from 0.58 to 0.90), and flipping one speaker reports exactly that speaker.

If a fixture holds a field this table does not name, use it as its `meta` describes and say so in the report.

## What to build

### DSP primitives (`crates/impulcifer-dsp/src/polarity.rs`, new, `pub mod polarity` in `lib.rs`)

```rust
pub const FFT_LEN: usize = 4096;
pub const DECISION: f64 = 0.5;
/// The window and the indices that located it, for the fixture gate.
pub struct DirectWindow { pub onset: usize, pub peak: usize, pub start: isize, pub taper: usize,
                          pub samples: Vec<f64> }            // samples.len() == L (unpadded)
pub fn direct_window(h: &[f64], fs: u32) -> Option<DirectWindow>;
pub fn band_weight(f: f64) -> f64;
pub struct Correlation { pub value: f64, pub lag: isize, pub energy: f64 }
/// Two unpadded windows of the same fs; pads to FFT_LEN itself.
pub fn window_correlation(a: &[f64], b: &[f64], fs: u32) -> Option<Correlation>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Side { Left, Right, Centre }
pub struct PolarityInput<'a> { pub fs: u32, pub side: Side,
                                pub left: Option<&'a [f64]>, pub right: Option<&'a [f64]> } // windows
/// Decided averaged pair values (None when undecided or not compared), inputs in caller order.
pub fn pair_matrix(inputs: &[PolarityInput<'_>]) -> Vec<Vec<Option<f64>>>;
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict { Reversed { index: usize, confidence: f64, compared: usize, opposite: usize },
                   Split { a: Vec<usize>, b: Vec<usize>, confidence: f64 } }
pub fn verdicts(pairs: &[Vec<Option<f64>>]) -> Vec<Verdict>;
```

Use the crate's own `fft::rfft` / `fft::irfft` (the inverse is normalized by `1/n`, as the spec requires). The
caller passes the inputs already in the canonical slot order; `verdicts` scans in index order. Every rule of the
spec (the first-index global peak, the last-index windowed peak and lag on ties, the taper, the weight, the
multiplicities, the `f64::EPSILON` guard, the clamp, the plain two-ear mean, the 0.5 decision, the breadth-first
signs with neighbours in index order, contradiction, the smaller group, equal groups, the medians) is part of
the contract the fixtures check.

### Service (`crates/impulcifer-service/src/recording/checks.rs`, `request.rs`)

- `Measurement` gains `direct: Option<DirectSound>` with `DirectSound { fs: u32, left: Vec<f64>, right: Vec<f64> }`
  (serialized as `{"fs", "left", "right"}`; omit the field when `None`, so earlier result consumers keep their
  shape for takes without it). Fill it, from the deconvolved `left_ir`/`right_ir` the take already computes,
  only for takes that qualify under the spec's "Takes that take part" (speaker recording, two tracks and an
  estimator, `snr ≥ 6`, no silent ear, not wrong side, no `record_check_polarity`, no
  `record_check_distortion`) and when both windows exist.
- `ReferenceMeasurement` gains the same optional `direct`. `request.rs` accepts it (and `null`) and rejects,
  naming the field: `fs` not an integer from 8000 to 768000, `left`/`right` not arrays of exactly
  `round(6*fs/1000)` finite numbers, a length above 4096. Keep the 64-entry limit and the unknown-field
  rejection for everything else.
- The check: the current run's qualifying takes plus the references with `direct` whose speaker is not in this
  run, in canonical `SLOTS` order, each with its side from the existing `direction(speaker, azimuth)` helper
  (`side > 0` left, `< 0` right, `0` centre); `pair_matrix`, `verdicts`; then, after every existing check,
  `record_check_speaker_polarity` (severity `warning`, speaker = the reversed one, values `compared` and
  `opposite` as JSON integers and `confidence` as a **string** with two decimals) per reversed speaker and
  `record_check_speaker_polarity_split` (severity `warning`, speaker = group a's first, values `group_a` and
  `group_b` as strings of codes joined by `, `, `confidence` as a two-decimal string). The screen formats
  every numeric value not ending in `_db` as an integer, which is why the confidence is a string. Headphone
  recordings run no part of this.
- Add the two keys to **all nine** catalogs `crates/impulcifer-service/locales/<lang>.json` with English text and
  identical placeholders (translation is a later packet), next to `record_check_polarity`:
  - `record_check_speaker_polarity`: `{speaker}: its polarity is opposite to most of the other speakers ({opposite} of {compared} comparisons, correlation {confidence}). Check its + and − wiring, then record it again.`
  - `record_check_speaker_polarity_split`: `Speakers {group_a} and {group_b} have opposite polarity, and no other speaker shows which side is reversed (correlation {confidence}). Check their + and − wiring.`
  `cargo test -p impulcifer-service --lib settings` and `cargo test -p impulcifer-service --test ui_catalog` must
  pass (run them separately). If `ui_catalog` requires every `record_check_*` key the service can emit to be
  listed somewhere, list the two new keys there.

### Tests and gates

- `crates/impulcifer-dsp/tests/golden_secs2r.rs`: one test per new family, through `golden_secs2_helpers.rs`, in the
  class the fixture table gives. Print `MEASURE` lines like the earlier tests. Every earlier test passes unchanged.
- Unit tests of your own for what the fixtures leave thin: a contradicting triangle (build the pair matrix by
  hand) gives no verdict; components are judged separately; a mismatched `fs` gives no value.
- Service tests (`crates/impulcifer-service/tests/recording_checks.rs`, alongside the existing take checks): a
  run with a speaker wired in reverse among correctly wired ones reports it; a reversed speaker found only
  through references (recorded earlier) is reported; a take with an inverted microphone or strong distortion
  carries no `direct` and takes no part; a re-recorded speaker replaces its reference; headphone runs emit
  nothing; `reference_measurements_validate_shape_limit_and_speakers` (or a sibling test) covers the new field's
  accepted and rejected forms. Build the takes the way the existing tests synthesize them.
- `tests/migration/README-secs2r.md`: add the new families to the family list and their measured maxima to the
  gates table. Do not edit its "Regeneration" section.
- `features.toml`: `recording.speaker_polarity_check` (kind `recording`, as `recording.input_balance_check`), with
  real test names. `cargo test -p impulcifer-policy` must pass.
- No brir-3x scenario: no output file changes. Confirm that every brir-3x scenario's WAVs are byte-identical to
  `origin/master` by reasoning (the BRIR pipeline does not call the recording check) and say so; do not run it.
- Version `3.17.0` in `Cargo.toml` (`[workspace.package].version`), `apps/impulcifer-app/tauri.conf.json` and
  `Cargo.lock` (a build rewrites the lock). CHANGELOG entry at the top:

  ```
  ## 3.17.0 - <today>
  ### 녹음 점검에 스피커 상대 극성 검사

  #### ⭐ 새로운 기능 / 개선
  - **…**: …
  ```

  in Korean: what the check compares, which takes take part, the two warnings, that output files do not change,
  and that the behaviour comes from SECS 2R through the clean-room method of ADR 0015. Keep the category
  heading's emoji exactly as above. Check whether `README.md` lists the recording checks; add the new one only if
  it does.

Do not touch: `apps/impulcifer-app/ui/*`, `apps/impulcifer-app/tests/*`, `webview_ui/`, `i18n/locales/`,
`CLAUDE.md`, the ADR, `docs/rust/ALIGNMENT.md` and `docs/rust/ROOM_CORRECTION.md` (if the spec needs a correction,
say so in the report instead), the `s2_*` fixtures and every `s2r_*` fixture file.

## Verification (foreground, before reporting; `env -u HTTPS_PROXY -u HTTP_PROXY` in front of cargo if a
command fails on the network)

```
cd /e/Impulcifer/.worktrees/s2r3
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-cli -p impulcifer-policy
cargo test -p impulcifer-dsp
cargo test -p impulcifer-service
cargo test -p impulcifer-python
```

## Report (in this order, then end your turn)

1. Files changed with a one-line purpose each.
2. The `MEASURE` table: every new family's observed maximum and the gate you set, and whether this machine has
   the oracle's libm.
3. The demo: Impulcifer's pair values and verdicts on the fixtures' demo variants next to the oracle's.
4. The IPC shape: the exact JSON of one `Measurement` with `direct` and of an accepted reference, and every Rust
   test you changed because of the shape.
5. The exact commands run and their final lines.
6. Open questions and anything you could not do, with the reason.
7. A wiring list for the next packet: the result field, the reference field, the two catalog keys and their
   placeholders.
