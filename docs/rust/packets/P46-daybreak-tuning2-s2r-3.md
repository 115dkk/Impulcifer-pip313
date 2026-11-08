# P46: virtual room tuning 2, bundle 3 of ADR 0015 (upper-limit basis and transition, input high-pass, pair phase)

Repository worktree: `E:\Impulcifer\.worktrees\s2r3` (Git Bash: `/e/Impulcifer/.worktrees/s2r3`), branch
`claude/tuning2-s2r-3`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\s2r3\CLAUDE.md`, section "3.x Rust 워크스페이스": `forbid(unsafe_code)`, no new
  dependencies, the `features.toml` gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, brir-3x byte identity,
  "DSP를 병렬화할 때는 map만" (everything here is sequential f64).
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\ROOM_CORRECTION.md`, section "Virtual room tuning v2": **the
  specification**. The parts marked 3.15.0 are what you build: "Options" (three rows and the paragraph after
  the 3.14.0 paragraph), "Position policy" (last paragraph), "Shared numerical primitives" (HP8), "Correction
  band" ("Upper-limit basis and transition"), "MIMO supports" ("Input high-pass"), "Applying to the BRIR"
  ("Input high-pass"), "Pair phase", "Diagnostics" (the 3.15.0 keys), "Reference acceptance". When this
  packet and the spec differ, the spec wins.
- `E:\Impulcifer\.worktrees\s2r3\docs\adr\0015-secs2r-improvements-into-tuning2.md` (Korean; the decision).
  Bundle 3 is item 4.3 of that ADR: D46, D57, D53. Nothing from items 4.4 and 4.5 enters this branch.
- `E:\Impulcifer\.worktrees\s2r3\tests\migration\README-secs2r.md` and `README-secs2.md`: the fixture
  encoding, the four gate classes of the `s2r_*` family (Exact, Curve, Rebuilt, FitDependent) and how they
  depend on `s2r_libm_probe`. `crates/impulcifer-dsp/tests/golden_secs2r.rs` implements them for bundles 1
  and 2; extend it the same way.

Clean room: these options reproduce behaviour of a GPL program. You implement them from the specification and
the fixtures only. Do not search for, open, or ask for that program's source, and do not look for its
repository on this machine. If the spec is ambiguous, say so in the report and implement the reading the
fixtures support.

## Fixtures

The new files sit next to the bundle-1 and bundle-2 ones in
`E:\Impulcifer\.worktrees\s2r3\tests\migration\goldens\s2r_*` (same encoding, same `s2r_bins.json`, exported
under Debian glibc 2.41, `meta.libm` says so). Demo inputs are the same demo omni recordings as before; each
fixture's `meta.derivation` says how the analysed IR is obtained from them. Oracle field names and the spec's
symbols:

| Field | Spec |
| --- | --- |
| `octave_500`, `octave_1000` | the strict estimate's two octave bands |
| `third_<c>` | the lenient band with centre c |
| `i5`, `i25`, `decay_s` | the crossing indices and `3*(i25−i5)/fs` (or `null`: not kept) |
| `sos`, `edges_hz` | the order-4 Butterworth band-pass sections and the edges used |
| `strict` (upper_demo) | f_S |
| `lenient_centres` | per centre c: T_c and g_c (or `null`) |
| `lenient_choice` | `(centre_hz, median_t60_s, g_hz)` = (ℓ, T_ℓ, g_ℓ) or `null` |
| `rc_hi.strict`, `rc_hi.lenient` | hi per multiplier under each basis |
| `rows`, `hz` (upper_rule) | `[f_S, ℓ, m]` (`null` = absent) and hi |
| `coefficients`, `response`, `magnitude_db` (hp8) | a_0 … a_8, HP8 at the bins (and in full for 25 Hz), dB at f_c, f_c/√2, f_c/2, f_c·√2 |
| `f`, `band_f`, `responses` (pair_phase) | the bins and the per-speaker R |
| `bands`, `notes` (pair_phase) | `(E_m, E_(m+1), φ, loss)` per pair, and the report (pairs with their bands at or below −6 dB) |
| `rc_hi`, `rc_limit` (rc upper) | hi used, and f_S, ℓ, T_ℓ |

Families and their gate class (README-secs2r.md "Gates"; Exact and Curve tighten on the oracle's libm):

| Family | Cases | How the test feeds it | Class |
| --- | --- | --- | --- |
| `s2r_band_sos` | the ten band-pass SOS sets at 48 kHz | direct | Exact |
| `s2r_band_t60_synthetic` | a stored 48000-sample IR, ten bands | direct; indices exact, decay Exact | Exact |
| `s2r_band_t60_demo` | 14 demo omni IRs (`<spk>_<position>`) × ten bands | rebuild each IR as `meta.derivation` says, then the band decay; indices exact | Rebuilt (expect Exact) |
| `s2r_upper_demo_v{20,40,80,200}` | f_S, every T_c and g_c, ℓ, hi for m = 1, 2, 4 under both bases | rebuild with the oracle's position policy: f_S from position 0 of every speaker, the lenient medians over both positions of every speaker (`inputs.main_position`, `inputs.positions`) | Rebuilt (expect Exact) |
| `s2r_upper_rule` | 21 `[f_S, ℓ, m]` rows | direct | Exact |
| `s2r_hp8` | f_c 10, 25, 27, 50 | direct on the design grid; coefficients and response | Exact |
| `s2r_pair_phase_synthetic_<case>` | 11 cases (`same`, `deg90`, `deg120`, `deg180`, `top_edge`, `below_range`, `nan`, `huge`, `denominator_overflow`, `ranking`, `threshold`) | direct into your band and report functions | Exact |
| `s2r_pair_phase_demo_{mixed,linear}` | the oracle's own corrected responses of seven speakers at its listening position (sidecars) | feed the stored responses to your band and report functions (Exact); your own demo report from the BRIRs is a different measurement (per ear, in-ear) and is report-only | Exact + report |
| `s2r_rc_<spk>_upper_mixed` | one-speaker runs, V = 20, lenient, m = 2, mixed | run your correction band with the oracle's policy on that speaker alone: `rc_hi` and `rc_limit` Rebuilt (expect Exact), `design_db` Rebuilt; `peq`, `fir`, `response`, `phase_strength` report-only | Rebuilt + FitDependent |

On the demo the lenient centre never rises above the oracle's f_S for these four volumes (its g does), so
the two bases give the same hi there. Under Impulcifer's policy (both ears) the lenient value rises above
f_S for V in 34–36.5, 53–57, 87–102 and 136–138 m³; the brir-3x scenarios use **VOLUME = 95** (f_S ≈ 119.5 Hz,
ℓ = 125 Hz). Check these numbers with your implementation and report them.

If a fixture holds a field this table does not name, use it as its `meta` describes and say so in the report.

## What to build

### Configuration

`crates/impulcifer-types/src/config.rs`: three fields on `ProcessingConfig`, appended to
`EXTENSION_FIELD_NAMES` in this order (35 → 38):

- `room_tuning2_high_basis: String`, default `"strict"`; valid `strict`, `lenient` (anything else, including
  `agreement`, is rejected naming the field).
- `room_tuning2_high_transition: u32`, default 1; valid 1, 2, 4. JSON accepts the integer; `from_kwargs`
  accepts what it accepts for the other integer-valued options.
- `room_tuning2_input_hp: String` with `string_or_number`, default `"off"`; valid `off` or a finite number
  10–50 inclusive, stored as a decimal string like `room_tuning2_mimo_low`.

The two tests that assert `EXTENSION_FIELD_NAMES.len() == 35` (`crates/impulcifer-types/tests/itd_config.rs`,
`crates/impulcifer-types/tests/output_fill_config.rs`) become 38; extend
`crates/impulcifer-types/tests/tuning2_config.rs` with the defaults, the accepted forms and the rejections.
Service validation (`crates/impulcifer-service/src/brir/validation.rs`) as for the neighbouring tuning2
fields. CLI (`crates/impulcifer-cli/src/options.rs`, `EXTENSION_OPTIONS`): `--room_tuning2_high_basis
strict|lenient`, `--room_tuning2_high_transition 1|2|4`, `--room_tuning2_input_hp off|<Hz>`, help text in the
style of the neighbouring tuning2 options; extend `cli_room_tuning2_options_parse_and_validate`
(`crates/impulcifer-cli/tests/cli.rs`). Policy gate: `CANONICAL_CONFIG` in
`crates/impulcifer-policy/tests/gates.rs` (`[&str; 58]` → 61).

### DSP (`crates/impulcifer-dsp/src/stages/room_tuning2/`)

Carry the options through `Tuning2Options` / `RcOptions` / `MimoOptions` and implement the spec:

- **Upper limit** (`mod.rs`, `correction_band`): the lenient value ℓ (eight third-octave bands, every position
  and every speaker under both policies, the band decay exactly as the strict estimate computes it, the last
  candidate's centre), the combination `clamp(v*m, 100, 500*m)`, the `schroeder_freq` and no-volume rules.
  With basis `strict` and m = 1 the code must take the existing branch (do not rely on the general formula
  reproducing the old bits, even though it should). `CorrectionBand` gains what the diagnostics need: basis,
  m, f_S, ℓ and T_ℓ.
- **HP8** (`primitives.rs`): the response exactly as the spec's arithmetic order says.
- **MIMO with the cut** (`mimo.rs`): multiply every long spectrum by HP8 before the support bands, x, the
  paths and the guard; the strength ratio's denominator stays without it.
- **The final cut** (`apply.rs`): after every diagnostic and the pair-phase report, convolve every speaker's
  response at both ears with `irfft_N(HP8)`, keep `len(r)` samples, fade the last 5 ms as the existing apply
  does. Unmeasured speakers too, every mode. Without ear-position measurements (the service's
  `cli_room_tuning_no_data` path) the cut still applies to every BRIR: find where the service skips tuning2 in
  that case and route the cut through it without running the RC.
- **Pair phase** (`apply.rs` or a new `pair_phase.rs` in this directory): the long spectra of the aligned
  in-ear BRIRs (MIMO's definition, without the cut; compute them even when MIMO is off), `R_e,i` over
  40–200 Hz, the six bands, the per-band phase and loss with the spec's arithmetic, the per-ear report, the
  ordering and the six-pair limit. It must not change any output sample.
- Every new branch must leave the off state on the exact 3.14.0 arithmetic: branch on the option.
- **Reports**: `Tuning2Report` gains the band's basis evidence (through `CorrectionBand`), the input cut
  (f_c or none) and `pair_phase` (the reported pairs with their per-ear bands). Service log
  (`crates/impulcifer-service/src/brir/run.rs`) and README writer (`crates/impulcifer-service/src/brir/outputs.rs`),
  next to the 3.14.0 keys: `cli_room_tuning2_high` `{basis, factor, schroeder, band, band_t60, high}`,
  `cli_room_tuning2_high_no_volume` (warning) `{}`, `cli_room_tuning2_input_hp` `{freq}`,
  `cli_room_tuning2_pair_phase` `{a, b, ear, spans}`, `cli_room_tuning2_pair_phase_hint` `{}`. Number
  formats as the spec's "Diagnostics" says; `none` the way the 3.14.0 keys render it. Add the keys to **all
  nine** catalogs `crates/impulcifer-service/locales/<lang>.json` with English text and identical
  placeholders (translation is a later packet); `cargo test -p impulcifer-service --lib settings` and
  `cargo test -p impulcifer-service --test ui_catalog` must pass (run them separately: in one command the
  `settings` filter also filters out every `ui_catalog` test). The catalogs already hold nine screen keys of this bundle (`label_room_tuning2_high_basis`
  and the rest); leave them.

### Tests and gates

- `crates/impulcifer-dsp/tests/golden_secs2r.rs`: one test per new family, through
  `golden_secs2_helpers.rs`, in the class the fixture table gives. Print `MEASURE` lines like the earlier
  tests. Every bundle-1 and bundle-2 test must keep passing unchanged.
- Synthetic unit tests of your own for what the fixtures leave thin: the clamp at 100 and at 500·m, no v
  giving 500 without m, `schroeder_freq` with lenient (ℓ not computed) and with m = 2, a numeric
  `room_tuning2_high` making both options inert, the no-volume log; the MIMO cut being exactly "every long
  spectrum times HP8" (build the MIMO inputs once with the cut and once with pre-multiplied spectra and the
  cut off, and compare bit for bit); the final cut on an unmeasured speaker and on the no-data path; the
  diagnostics reading the same with the cut on and off; pair phase on two BRIRs where one speaker's bass is
  inverted (reported) and on two identical ones (not reported), the six-pair limit and the ordering.
- Byte identity: with the three options at their defaults every existing test, every `s2_*` and bundle-1/2
  `s2r_*` gate and the `room_tuning_secs2_*` references pass unchanged, and the demo outputs are
  byte-identical to `origin/master` for every tuning2 scenario of `.github/scripts/brir_3x_integrity.py` (run
  its local comparison; report the hashes). The pair-phase report must not change those hashes.
- `tests/migration/README-secs2r.md`: add the new families to the family list and their measured maxima to
  the gates table. Do not edit its "Regeneration" section.
- `features.toml`: `config.room_tuning2_high_basis`, `config.room_tuning2_high_transition`,
  `config.room_tuning2_input_hp` (kind `config`) and `room.tuning2_high_basis`, `room.tuning2_input_hp`,
  `room.tuning2_pair_phase` (kind `stage`), with real test names, as the 3.14.0 entries do.
  `cargo test -p impulcifer-policy` must pass.
- brir-3x: four scenarios in `.github/scripts/brir_3x_integrity.py`:
  `room_tuning2_volume` (`--room_mode=tuning2 --room_volume=VOLUME`, tagged `"3.5.0"`: the strict ×1
  default with a volume, compared against the base CLI),
  `room_tuning2_high_lenient` (`--room_mode=tuning2 --room_volume=VOLUME --room_tuning2_high_basis=lenient
  --room_tuning2_high_transition=2`, `"3.15.0"`),
  `room_tuning2_input_hp` (`--room_mode=tuning2 --room_tuning2_input_hp=25`, `"3.15.0"`),
  `room_tuning2_input_hp_mimo` (`--room_mode=tuning2 --room_tuning2_mimo=true --room_tuning2_fsiso=120
  --room_tuning2_input_hp=25`, `"3.15.0"`).
  VOLUME = 95 (fixture section).
- README: three rows in the CLI option table after `--room_tuning2_hf_phase`, Korean, same style.
- Version `3.15.0` in `Cargo.toml` (`[workspace.package].version`), `apps/impulcifer-app/tauri.conf.json` and
  `Cargo.lock` (a build rewrites the lock). CHANGELOG entry at the top:

  ```
  ## 3.15.0 - <today>
  ### 가상 룸 튜닝 2의 상한 기준과 전이 배수, 입력 하이패스, 짝 위상 보고

  #### ⭐ 새로운 기능 / 개선
  - **…**: …
  ```

  in Korean, one bullet per option and one for the report, saying that the defaults keep the output
  byte-identical and that the behaviour comes from SECS 2R through the clean-room method of ADR 0015. Keep the
  category heading's emoji exactly as above.

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
3. The demo results: f_S, ℓ, T_ℓ and hi for VOLUME under both bases and m = 1, 2, 4; the demo pair-phase
   report (Impulcifer's, per ear) next to the oracle's notes.
4. Byte identity: the hashes or the test names that prove the defaults are unchanged.
5. The exact commands run and their final lines.
6. Open questions and anything you could not do, with the reason.
