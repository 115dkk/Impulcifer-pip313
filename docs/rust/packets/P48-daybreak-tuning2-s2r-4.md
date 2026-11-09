# P48: virtual room tuning 2, bundle 4 of ADR 0015 (MIMO early window, MIMO ear guard)

Repository worktree: `E:\Impulcifer\.worktrees\s2r3` (Git Bash: `/e/Impulcifer/.worktrees/s2r3`), branch
`claude/tuning2-s2r-4`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\s2r3\CLAUDE.md`, section "3.x Rust 워크스페이스": `forbid(unsafe_code)`, no new
  dependencies, the `features.toml` gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, brir-3x byte identity,
  "DSP를 병렬화할 때는 map만" (everything here is sequential f64).
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\ROOM_CORRECTION.md`, section "Virtual room tuning v2": **the
  specification**. The parts marked 3.16.0 are what you build: "Options" (two rows and the paragraph after
  the 3.15.0 paragraph), "MIMO supports" ("Least squares" with the early window, the guard's own window `W_e`
  in "Guard", and the new "Ear guard" bullet), "Diagnostics" (the 3.16.0 keys), "Reference acceptance". When
  this packet and the spec differ, the spec wins.
- `E:\Impulcifer\.worktrees\s2r3\docs\adr\0015-secs2r-improvements-into-tuning2.md` (Korean; the decision).
  Bundle 4 is item 4.4 of that ADR: D58 and D59. Nothing from item 4.5 enters this branch.
- `E:\Impulcifer\.worktrees\s2r3\tests\migration\README-secs2r.md` and `README-secs2.md`: the fixture
  encoding and the four gate classes of the `s2r_*` family (Exact, Curve, Rebuilt, FitDependent) and how they
  depend on `s2r_libm_probe`. `crates/impulcifer-dsp/tests/golden_secs2r.rs` implements them for bundles 1
  to 3, `crates/impulcifer-dsp/tests/golden_secs2_mimo.rs` gates the SECS 2 MIMO stage boundaries the new
  fixtures build on; extend `golden_secs2r.rs` the same way.

Clean room: these options reproduce behaviour of a GPL program. You implement them from the specification and
the fixtures only. Do not search for, open, or ask for that program's source, and do not look for its
repository on this machine. If the spec is ambiguous, say so in the report and implement the reading the
fixtures support.

## Fixtures

The new files sit next to the earlier ones in `E:\Impulcifer\.worktrees\s2r3\tests\migration\goldens\s2r_mimo_*`
(same encoding, exported under Debian glibc 2.41, `meta.libm` says so). The oracle refuses MIMO below three
positions around its whole run, so these fixtures feed it the two-ear stage boundaries of the SECS 2 MIMO
fixtures `s2_mimo_<setting>_<input>.json` (x per ear, the raw paths g, the low-pass `lp`, the direct energy);
each new fixture's `meta.inputs_from` names that file and the sha256 of every sidecar it read. Load those
inputs the way `golden_secs2_mimo.rs` already does. Positions are `[left, right]` = ear 0, ear 1.

Oracle field names and the spec's symbols:

| Field | Spec |
| --- | --- |
| `rows` (`s2r_mimo_early_samples`) | `[E ms, e]` |
| `rows`, `violation_db` (`s2r_mimo_seat_change`) | `[a, b, c(a, b)]`, `null` = no value; the `+99` |
| `inputs.candidates`, `inputs.lp`, `inputs.early_samples` (normal) | candidate order, the low-pass, e |
| `diagonal`, `block_first_rows`, `Atb`, `frobenius`, `trace` (normal) | AᵀA's diagonal, `AᵀA[j*180][jj*180 + l]` row-major over j, jj, l (the layout `golden_secs2_mimo.rs` builds), Aᵀb |
| `inputs.lp`, `inputs.direct_energy`, `outputs.early_samples` (design) | what to feed `design_paths_with` (equal to the `s2_mimo_*` file's `lp` and `direct`), e |
| `trajectory[] {mid, ratio, accepted}`, `log_lambda`, `ratio` (design) | the 19 solves: log10(λ/σ), the ratio, `ratio ≤ 10^(−1.2)`; the kept log and its ratio |
| `guards[]` (design) | per tried scale: `scale`, `band_db`, `band_hz` `[lo, hi]`, `band_position` (ear or `null`), `tail_db` (the spec's tail change in dB), `t60_change` (`tail_db*0.15`, the oracle's own unit; ignore it), `mode_hz`, `tail_position`, `passes` |
| `guard_scale`, `support_names`, `supports`, `level_db` (design) | the accepted scale (0: none accepted), the supports in one candidate-major sidecar of 210 values each (empty when none), the level |
| `guard_cause {rejected_db, band, tail, band_position, tail_position}` (design) | the ear record of the last rejected scale: `rejected_db` is `20*log10(scale)` (`−12.000000000000002` means the −12 dB step), `band = [lo, hi, dB]`, `tail = [hz, dB]`; `null` when the first scale passed |
| `seat_off`, `seat_on` `{band {db, hz, position}, tail {db, t60_change, hz, position}, band_energies {bands_hz, before[ear][band], after[ear][band]}}` (guard) | `guard_evaluation` with the ear guard off and on; the per-ear band energies the band change is built from |

Two differences between the oracle and SECS 2 that the tests must expect, not paper over:

- The oracle reaches a scale by multiplying by `10^(−3/20)` per step; Impulcifer computes `10^(dB/20)` for
  each step. Compare `scale` at relative 1e-14 and the step index exactly; `rejected_db` maps to the step
  (0, −3, −6, −9, −12) by rounding.
- SECS 2 evaluates no tail guard on a trial whose band guard already failed, so `s2_mimo_*`'s `guard_tail`
  can be shorter than its `guard_band_db` (the `golden_secs2_mimo.rs` filter). The oracle here evaluates both
  on every trial, as Impulcifer does: compare every trial.

Families and their gate class (README-secs2r.md "Gates"; Exact and Curve tighten on the oracle's libm):

| Family | Cases | How the test feeds it | Class |
| --- | --- | --- | --- |
| `s2r_mimo_early_samples` | 21 values of E, inside and outside 5–60 | `early_samples` | Exact |
| `s2r_mimo_seat_change` | 14 `(a, b)` pairs | your `c(a, b)` (make it a public function) | Exact |
| `s2r_mimo_normal_<I>_e<n>` | every input at e = 12 and 18, FL also at 3 and 36 (setting `mixed_fsiso150_auto`) | x, g from the `s2_mimo_*` file, c = g ⊛ `inputs.lp` truncated to 720, `normal_equations_with` | Curve, with the budgets `golden_secs2_mimo.rs` uses for the normal equations |
| `s2r_mimo_design_<setting>_<I>_<variant>` | 2 settings × 7 inputs × `e60`, `e60_seat`, `e20`, `e20_seat`, `e5` | `design_paths_with` on the `s2_mimo_*` inputs with `lp` and `direct_energy` from the fixture, `fsiso` from `meta.settings`, `MimoRule { early: outputs.early_samples, ear_guard: meta.settings.seat_guard }` | decisions, kept index, number of trials, `passes`, ears, `band_hz`, `mode_hz` and the accepted step exact; ratios, supports, level and guard values with the budgets `golden_secs2_mimo.rs` uses (provisional ones stay provisional); `guard_cause` against your `ear_limit` (values abs 1e-9) |
| `s2r_mimo_guard_<case>` | `one_ear_worse`, `both_better`, `new_energy`, `silent_ear`, `tie` | `guard_evaluation` on the stored x and y, `fsiso = 150`, off and on | values Curve (abs 1e-9 as the guard values above), `hz` and `position` exact, `band_energies` relative 1e-12 |

The `e60` cases with the ear guard off are the control: the oracle's results there match the `s2_mimo_*`
files (supports within 1.2e-12 relative L2, every decision and guard value), which `golden_secs2_mimo` already
gates for Impulcifer. On these inputs the ear guard changes the accepted scale of 4 to 5 inputs per setting and
the 20 ms and 5 ms windows change 5 to 6, so the families do exercise both options.

If a fixture holds a field this table does not name, use it as its `meta` describes and say so in the report.

## What to build

### Configuration

`crates/impulcifer-types/src/config.rs`: two fields on `ProcessingConfig`, appended to
`EXTENSION_FIELD_NAMES` in this order (38 → 40):

- `room_tuning2_mimo_early: f64`, default 60.0; valid a finite number from 5 to 60 inclusive (rejected naming
  the field otherwise). JSON takes a number, as `room_tuning2_low` does.
- `room_tuning2_mimo_ear_guard: bool`, default false, as `room_tuning2_dip_limit`.

Both are validated whatever `room_tuning2_mimo` is, and read only when it is true. The two tests that assert
`EXTENSION_FIELD_NAMES.len() == 38` (`crates/impulcifer-types/tests/itd_config.rs`,
`crates/impulcifer-types/tests/output_fill_config.rs`) become 40; extend
`crates/impulcifer-types/tests/tuning2_config.rs` with the defaults, the bounds (5 and 60 accepted, 4.99,
60.01, NaN, infinity rejected) and the boolean. Service validation (`crates/impulcifer-service/src/brir/validation.rs`)
as for the neighbouring tuning2 fields. CLI (`crates/impulcifer-cli/src/options.rs`, `EXTENSION_OPTIONS`):
`--room_tuning2_mimo_early <ms>` (`CliType::Float`, as `--room_tuning2_low`) and
`--room_tuning2_mimo_ear_guard true|false` (as `--room_tuning2_dip_limit`), help text in the style of the
neighbouring MIMO options; extend the tuning2 CLI test (`crates/impulcifer-cli/tests/tuning2.rs`). Policy gate:
`CANONICAL_CONFIG` in `crates/impulcifer-policy/tests/gates.rs` (`[&str; 61]` → 63).

### DSP (`crates/impulcifer-dsp/src/stages/room_tuning2/mimo.rs`)

Signatures (keep the existing public functions working with their old meaning, so every existing caller and
test compiles and passes unchanged):

```rust
pub struct MimoOptions { pub fsiso: f64, pub low: Option<f64>, pub input_hp: Option<f64>,
                         pub early_ms: f64, pub ear_guard: bool }   // two new fields; update every literal
pub const MIMO_EARLY_MS_DEFAULT: f64 = 60.0;
/// max(round(ms * 600 / 1000), 3), f64::round (half away from zero).
pub fn early_samples(early_ms: f64) -> usize;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MimoRule { pub early: usize, pub ear_guard: bool }      // Default: early 36, ear_guard false
pub fn normal_equations(x, c)            // = normal_equations_with(x, c, 36)
pub fn normal_equations_with(x: &[Vec<f64>], c: &[Vec<Vec<f64>>], early: usize)
                                          -> Result<(Mat<f64>, Vec<f64>), DspError>;
pub fn design_paths(...)                  // same parameters as now = design_paths_with(..., MimoRule::default())
pub fn design_paths_with(speaker, candidates, x, g, lp, direct, fsiso, rule: MimoRule)
                                          -> Result<(MimoInputResult, MimoStages), DspError>;
#[derive(Clone, Debug, PartialEq)]
pub struct GuardEvaluation { pub band_db: f64, pub band_hz: Option<(f64, f64)>, pub band_ear: Option<usize>,
                             pub tail_db: f64, pub mode_hz: Option<f64>, pub tail_ear: Option<usize> }
pub fn guard_evaluation(x: &[Vec<f64>], y: &[Vec<f64>], fsiso: f64, ear_guard: bool) -> GuardEvaluation;
```

- **Early window.** `normal_equations_with` is today's `normal_equations` with `36` replaced by `early`
  (`(first(x) + early).min(N)` in the rows and the right side). `design_paths_with` passes `rule.early`.
  `design_supports*` converts `opts.early_ms` with `early_samples` and passes it with `opts.ear_guard`. The
  guard's own windows stay `first(x) + 36` (band) and `first(x) + 180` (tail): name them as constants so the
  two 36s cannot be confused again.
- **Ear guard.** `guard_evaluation` with `ear_guard = false` must give today's band and tail values bit for
  bit: keep today's arithmetic (summed per-bin energies, the same loops). It additionally reports `band_hz`
  (the first band, in the spec's band order, whose change equals the result; `None` when the result is the
  −99 floor) and `mode_hz` (the first mode in ascending order whose value equals the result; `None` likewise);
  `band_ear` and `tail_ear` are `None`. With `ear_guard = true` implement the spec's "Ear guard" bullet
  exactly: per-ear band energies (each ear's own bin sum over the band, bins ascending), `c(a, b)` with the
  floor on both sides, `+99`, skipping, the summed-energy mode selection, the scan orders and the strict
  replacement that decides the recorded ear, band and mode.
- **Results.** `MimoStages` gains `guard_steps: Vec<GuardEvaluation>` (one per tried scale; `guard_band` and
  `guard_tail` stay). `MimoInputResult` gains `ear_limit: Option<MimoEarLimit>`:

  ```rust
  pub struct MimoEarLimit { pub scale_db: f64, pub band: Option<MimoEarBand>, pub tail: Option<MimoEarTail> }
  pub struct MimoEarBand { pub ear: usize, pub lo: f64, pub hi: f64, pub db: f64 }
  pub struct MimoEarTail { pub ear: usize, pub hz: f64, pub db: f64 }
  ```

  `Some` only with `rule.ear_guard` and at least one rejected scale: the **last** rejected scale (`0`, `−3`,
  … `−12`), `band` when that step's band change was above 1 dB, `tail` when its tail change was above 1 dB.
  With the ear guard off it is `None`.
- Every new branch must leave the defaults on the exact 3.15.0 arithmetic. `early_samples(60.0)` is 36, so
  the default reaches `normal_equations_with(.., 36)`, which must be today's code path unchanged.

### Service

- `crates/impulcifer-service/src/brir/inputs.rs`: fill the two `MimoOptions` fields from the config (only where
  MIMO options are built today, that is with `room_tuning2_mimo` on).
- Log (`crates/impulcifer-service/src/brir/run.rs`), next to `cli_room_tuning2_mimo`:
  `cli_room_tuning2_mimo_early` `{ms}` once per run when MIMO is on and the value is not 60 (one decimal);
  per input with `ear_limit`, right after that input's `cli_room_tuning2_mimo` or `cli_room_tuning2_mimo_off`
  line, `cli_room_tuning2_mimo_ear_band` `{speaker, scale, ear, lo, hi, db}` and/or
  `cli_room_tuning2_mimo_ear_tail` `{speaker, scale, ear, freq, db}`; `scale` as an integer (`0`, `-3`, …),
  `ear` as `left`/`right` (the way `cli_room_tuning2_pair_phase` passes it), Hz and dB to one decimal.
- README (`crates/impulcifer-service/src/brir/outputs.rs`): the early window and the ear guard with the MIMO
  settings; under each input the ear records.
- Add the three keys to **all nine** catalogs `crates/impulcifer-service/locales/<lang>.json` with English text
  and identical placeholders (translation is a later packet), next to `cli_room_tuning2_mimo_off`.
  `cargo test -p impulcifer-service --lib settings` and `cargo test -p impulcifer-service --test ui_catalog`
  must pass (run them separately: in one command the `settings` filter also filters out every `ui_catalog`
  test). The catalogs already hold four screen keys of this bundle (`label_room_tuning2_mimo_early` and the
  rest); leave them.

### Tests and gates

- `crates/impulcifer-dsp/tests/golden_secs2r.rs`: one test per new family, through `golden_secs2_helpers.rs`,
  in the class the fixture table gives. Print `MEASURE` lines like the earlier tests. Every earlier `s2r_*`
  test, `golden_secs2_mimo` and `room_tuning2_matches_secs2_mimo` must keep passing unchanged.
- Synthetic unit tests of your own for what the fixtures leave thin: `early_samples` at 5, 7.5, 60;
  `normal_equations_with(x, c, 36)` equal bit for bit to `normal_equations(x, c)`; the dense cross-check in
  `room_tuning2_mimo_fast_normal_equations` repeated with `early = 12`; `design_paths_with(.., MimoRule::default())`
  equal to `design_paths`; a constructed two-ear case where the summed guard passes and the ear guard rejects,
  with the recorded ear; `ear_limit` set only with the ear guard on; the service options inert with MIMO off
  (same output bytes as without them).
- Byte identity: with the two options at their defaults every existing test and gate passes unchanged, and the
  demo outputs are byte-identical to `origin/master` for every tuning2 scenario of
  `.github/scripts/brir_3x_integrity.py` (run its local comparison; report the hashes).
- `tests/migration/README-secs2r.md`: add the new families to the family list and their measured maxima to the
  gates table. Do not edit its "Regeneration" section.
- `features.toml`: `config.room_tuning2_mimo_early`, `config.room_tuning2_mimo_ear_guard` (kind `config`) and
  `room.tuning2_mimo_early`, `room.tuning2_mimo_ear_guard` (kind `stage`), with real test names, as the 3.15.0
  entries do. `cargo test -p impulcifer-policy` must pass.
- brir-3x: two scenarios in `.github/scripts/brir_3x_integrity.py`, after `room_tuning2_mimo`:
  `room_tuning2_mimo_early` (`--room_mode=tuning2 --room_tuning2_mimo=true --room_tuning2_fsiso=120
  --room_tuning2_mimo_early=20`, `"3.16.0"`) and `room_tuning2_mimo_ear_guard` (`--room_mode=tuning2
  --room_tuning2_mimo=true --room_tuning2_fsiso=120 --room_tuning2_mimo_ear_guard=true`, `"3.16.0"`). Report
  for the demo, per input, the guard scale with each option against the default, so we know whether the
  scenarios change the output.
- README: two rows in the CLI option table after `--room_tuning2_input_hp`, Korean, same style.
- Version `3.16.0` in `Cargo.toml` (`[workspace.package].version`), `apps/impulcifer-app/tauri.conf.json` and
  `Cargo.lock` (a build rewrites the lock). CHANGELOG entry at the top:

  ```
  ## 3.16.0 - <today>
  ### 가상 룸 튜닝 2 MIMO의 초기 구간과 귀별 가드

  #### ⭐ 새로운 기능 / 개선
  - **…**: …
  ```

  in Korean, one bullet per option, saying that the defaults keep the output byte-identical and that the
  behaviour comes from SECS 2R through the clean-room method of ADR 0015. Keep the category heading's emoji
  exactly as above.

Do not touch: `apps/impulcifer-app/ui/*`, `webview_ui/`, `i18n/locales/`, `CLAUDE.md`, the ADR,
`docs/rust/ROOM_CORRECTION.md` (if the spec needs a correction, say so in the report instead), the `s2_*`
fixtures and every `s2r_*` fixture file.

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

Then the brir-3x local comparison for every tuning2 scenario between `origin/master` and this branch, and a run
of each new scenario to show it completes (both thread settings).

## Report (in this order, then end your turn)

1. Files changed with a one-line purpose each.
2. The `MEASURE` table: every new family's observed maximum and the gate you set, and whether this machine has
   the oracle's libm.
3. The demo results: per input the guard scale at the default, with 20 ms, and with the ear guard, and every
   ear record.
4. Byte identity: the hashes or the test names that prove the defaults are unchanged.
5. The exact commands run and their final lines.
6. Open questions and anything you could not do, with the reason.
