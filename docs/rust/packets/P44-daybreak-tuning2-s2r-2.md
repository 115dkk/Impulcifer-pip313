# P44: virtual room tuning 2, bundle 2 of ADR 0015 (arrival alignment, mode-decay cuts, high-frequency phase)

Repository worktree: `E:\Impulcifer\.worktrees\s2r2` (Git Bash: `/e/Impulcifer/.worktrees/s2r2`), branch
`claude/tuning2-s2r-2`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\s2r2\CLAUDE.md`, section "3.x Rust 워크스페이스": `forbid(unsafe_code)`, no new
  dependencies, the `features.toml` gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, brir-3x byte identity,
  "DSP를 병렬화할 때는 map만" (everything here is sequential f64).
- `E:\Impulcifer\.worktrees\s2r2\docs\rust\ROOM_CORRECTION.md`, section "Virtual room tuning v2": **the
  specification**. The parts marked 3.14.0 are what you build: "Options" (three rows and the paragraph after
  the `room_tuning2_bands` paragraph), "Position policy" (last paragraph), "Shared numerical primitives"
  ("Mode decay" at a rate r), "Design curve" (the paragraph after the definition of P, step 5,
  "Mode-decay cuts" and the reported design curve), "Low-end protection in the fits" ("Where"), "IIR-only
  mode" (the cut list first), "FIR modes" (steps 2, 3, 4, 8, "Phase resolution", "Arrival alignment",
  "High-frequency phase"), "Diagnostics" (the 3.14.0 keys), "Reference acceptance". When this packet and the
  spec differ, the spec wins.
- `E:\Impulcifer\.worktrees\s2r2\docs\adr\0015-secs2r-improvements-into-tuning2.md` (Korean; the decision).
  Bundle 2 is item 4.2 of that ADR: D49 and D52. Nothing from the later items enters this branch.
- `E:\Impulcifer\.worktrees\s2r2\tests\migration\README-secs2r.md` and `README-secs2.md`: the fixture
  encoding, the four gate classes of the `s2r_*` family (Exact, Curve, Rebuilt, FitDependent) and how they
  depend on `s2r_libm_probe`. `crates/impulcifer-dsp/tests/golden_secs2r.rs` implements them for bundle 1;
  extend it the same way.

Clean room: these options reproduce behaviour of a GPL program. You implement them from the specification and
the fixtures only. Do not search for, open, or ask for that program's source, and do not look for its
repository on this machine. If the spec is ambiguous, say so in the report and implement the reading the
fixtures support.

## Fixtures

The new files sit next to the bundle-1 ones in `E:\Impulcifer\.worktrees\s2r2\tests\migration\goldens\s2r_*`
(same encoding, same `s2r_bins.json`, exported under Debian glibc 2.41, `meta.libm` says so). Demo cases use
the oracle's own position policy, `meta.weights = [5, 1]` (left ear = listening position, weight 5), so the
tests rebuild the inputs from the demo omni files with the SECS policy as the bundle-1 tests do. Demo settings
(`meta.settings`): `rc_lo = 20`, `rc_hi = 500` manual, max boost 10 dB, no target, both ears, and the mode named
in the case (`mixed` taps 2048 / lift 8 ms / β 6; `linear` 2048 / 28 ms / β 24 and 6; `zero` PEQ only).
`meta.settings` also lists switches of the oracle that are not Impulcifer options; ignore those names.

The fixtures keep the oracle's field names. Their meaning in the spec's symbols:

| Field | Spec |
| --- | --- |
| `beta_raw`, `beta` (lf_prep, realize) | κ_raw, κ (arrival weight; not the phase resolution β) |
| `nb` | the bins with `f < f_r` when low-end protection is on (0/1); all 0 otherwise |
| `lm` | the long-mode mask (0/1) |
| `sm`, `corr` (lf_prep) | S (step 1) and U0 |
| `tau`, `at_lin`, `target` (lf_prep) | τ_Y, τ_lin, τ_T |
| `phase_frac` (lf_excess, realize, rc) | the phase resolution β of the candidate |
| `ut`, `total`, `tau_tot`, `reference_tot`, `total_error` (lf_excess) | V_T, T = S_β(V_T), τ_A, r_A, e |
| `tau`, `reference`, `fade`, `excess` (lf_excess) | τ and r of step 3, γ, the final x |
| `applied_before`, `applied_after` (realize) | adv before and after the high-frequency unwind |
| `region.lo`, `region.hi`, `log_width`, `weight_sum`, `weight` | ν_1, ν_2, ln(ν_2/ν_1), W, w |
| `total`, `rho`, `wrapped`, `rotation`, `correction` (realize) | A, ρ, u, θ in radians, c |
| `long_modes`, `band_t60`, `target_t60_s` (mode_decay) | the candidates (p, T60), the five T60 at fs, T_t |
| `refined`, `merged`, `heights`, `decay_cuts`, `selected`, `cuts`, `dr` | refinements (with rejection reasons), the merged list, η, the cut per merged mode or `null`, (f_m, T_m, depth), D, Dr |
| `power`, `sm`, `design_db` (mode_decay) | P and S through Dr, U0 |
| `design_db` (rc) | the reported curve `U0 + 20*log10(|Dr| + 1e-12)` (U0 without the option) |
| `phase_candidates` (rc) | the two candidates' scores before the final Limit, and the kept β |

Families and their gate class (README-secs2r.md "Gates"; Exact and Curve tighten on the oracle's libm):

| Family | Cases | How the test feeds it | Class |
| --- | --- | --- | --- |
| `s2r_lf_weight` | 400 frequencies × 8 f_lin | direct | Exact |
| `s2r_lf_pull` | 27 `[e, lift, frac, f, b]` rows | direct | Exact |
| `s2r_decay_cut` | 10 `[f0, t60_m, t60_t, height, boost]` rows, `null` = no cut | direct | Exact |
| `s2r_hf_region_<case>` | 5 band sets (`inert_top`, `primary_20000`: no range) | direct on the design grid | Exact |
| `s2r_mode_t60_48k_synthetic` | a decaying 250 Hz sine at 48 kHz, five bands | direct | Curve |
| `s2r_mode_t60_48k_<spk>` | the five band T60 of `irfft_N` of position 0's comb spectrum, T_t | rebuild the comb spectrum | Rebuilt |
| `s2r_refine_resonance` | a stored resonance spectrum, three candidates (one rejected) | direct; indices, dB values, curvature, offset and f_m Exact, T_m Curve | Exact + Curve |
| `s2r_refine_<spk>` | every candidate of position 0 | rebuild | Rebuilt (indices and rejections exact) |
| `s2r_mode_decay_<spk>_mixed` | the whole cut search and its effect on P, S, U0 (`meta.zero_mode`: zero mode gives the same bits, so it has no case of its own) | rebuild; the lists' lengths and the rejections exact, values Rebuilt | Rebuilt |
| `s2r_lf_prep_<spk>_{lf_linear,lf_protect_linear}` | f_lin, κ, τ_T, with and without low-end protection (`meta.mixed_preparation`: the mixed preparation gives the same bits) | rebuild | Rebuilt |
| `s2r_lf_excess_<spk>_lf_linear_{24,6}` | V_T, T, τ_A, e, x per candidate (`meta.mixed_beta6`: mixed's β 6 record gives the same bits) | these values follow the first PEQ fit (V and V_T are built on `comb*P1*M`), so a rebuilt chain differs from the oracle's once its PEQ refinement does: recombine x at the stored bins from the stored τ, r, e, γ and the κ of the matching lf_prep case (Exact), and report the rebuilt values. `fl_lf_linear_24` also holds V_T, τ, γ, τ_T and κ at full resolution (`meta.full_resolution`): from them compute T, τ_A, r_A, e and x and compare (Curve), and compare that x with the full-resolution excess of `s2r_realize_fl_phase_linear` (its `meta` says whether they are the same bits) | Exact + Curve + report |
| `s2r_realize_unity_{hf,off,beta,inert}` and `s2r_realize_fl_phase_linear` | one Realize call with every input: the synthetic cases give each input as a rule (`inputs.*_rule`, the design grid's evaluation order included) plus the sha256 of the oracle's raw array, `band` as runs of true bins; `fl_phase_linear` stores them as sidecars | regenerate the synthetic inputs, compare their sha256 (expect a match on the oracle's libm; elsewhere report it), then: adv, A, ρ, u, c, θ, `limited` Exact; h, rms, max Curve | Exact + Curve |
| `s2r_realize_<spk>_<setting>` (other demo cases) | every Realize call's scalars, the selected call's adv and h | report-only: M and x follow the first PEQ fit, and even the oracle's own glibc and msvcrt exports choose different strengths (FR `hf_mixed`: 0.9 against 0.7) | FitDependent |
| `s2r_rc_<spk>_<setting>` (bundle 2 settings: `lf_mixed`, `decay_mixed`, `hf_mixed`, `off_linear`, `lf_linear`, `hf_linear`, `phase_linear`, `all2_linear`, `decay_zero`, `protect_linear`) | end-to-end | run the whole RC with the SECS policy: `design_db`, f_lin, T_t and the cuts Rebuilt; strength, `phase_frac_used`, arrival, rotation, the `limited` flags, `phase_candidates`, `peq`, `fir`, `response` report-only | Rebuilt + FitDependent |

If a fixture holds a field this table does not name, use it as its `meta` describes and say so in the report.
`s2r_rc_*_protect_linear` exists to check the phase-resolution order (spec "Phase resolution"): print its two
scores and the kept candidate next to yours.

## What to build

### Configuration

`crates/impulcifer-types/src/config.rs`: three `bool` fields on `ProcessingConfig`, default `false`,
appended to `EXTENSION_FIELD_NAMES` in this order (32 → 35): `room_tuning2_lf_align`,
`room_tuning2_mode_decay`, `room_tuning2_hf_phase`. JSON and `from_kwargs` accept what the other tuning2
booleans accept (`room_tuning2_low_protect` is the model). The two tests that assert
`EXTENSION_FIELD_NAMES.len() == 32` (`crates/impulcifer-types/tests/itd_config.rs`,
`crates/impulcifer-types/tests/output_fill_config.rs`) become 35; extend
`crates/impulcifer-types/tests/tuning2_config.rs` with the defaults and the accepted forms.

Service validation (`crates/impulcifer-service/src/brir/validation.rs`) as for `room_tuning2_low_protect`.
CLI (`crates/impulcifer-cli/src/options.rs`, `EXTENSION_OPTIONS`): `--room_tuning2_lf_align true|false`,
`--room_tuning2_mode_decay true|false`, `--room_tuning2_hf_phase true|false`, help text in the style of the
neighbouring tuning2 options; extend `cli_room_tuning2_options_parse_and_validate`
(`crates/impulcifer-cli/tests/cli.rs`). Policy gate: `CANONICAL_CONFIG` in
`crates/impulcifer-policy/tests/gates.rs` (`[&str; 55]` → 58).

### DSP (`crates/impulcifer-dsp/src/stages/room_tuning2/`)

Carry the options through `Tuning2Options` / `RcOptions` (`apply.rs`, `mod.rs`) and implement the spec:

- **Mode decay at a rate** (`primitives.rs`): generalize `mode_decay` to a rate parameter. The 600 Hz path
  must stay bit-identical (every `s2_*` gate that uses it must pass unchanged); keep its arithmetic and only
  replace the constants by the rate where the spec says so.
- **Mode-decay cuts** (`rc.rs`, design curve): the per-position candidate list before the median filter (the
  existing `long_modes` returns only the filtered union, so factor the per-position candidate step out without
  changing what `long_modes` returns), refinement, the pooled target decay, the merge, the cut rule, D and Dr,
  P recomputed through Dr for steps 1 to 3, the cut bins in step 5, the reported design curve
  `U0 + 20*log10(|Dr| + 1e-12)`, Dr in step 2 of the FIR modes and in F_total, the list `D ++ B2` (FIR) and
  `D ++ B_lo ++ B_hi` (zero), and the shifted reduced indices of the final Limit.
- **Arrival alignment** (`rc.rs`): κ, τ_T, the total-phase sum V_T in step 2, the per-candidate combination
  after step 3, the bound in Realize, the `limited` flag and the arrival report.
- **High-frequency phase** (`rc.rs`, `Realizer`): the range, the weights and their sum once per speaker,
  the unwind in every Realize call, the rotation and `limited` of the call at s*.
- **Phase resolution**: score the two candidates on `F*P2*Dr` before the final rolloff Limit, then run that
  Limit on the kept candidate only (spec "Phase resolution"). This changes 3.13.0 output only for low-end
  protection in `linear` and x ms; mixed, long and zero are unaffected.
- Every new branch must leave the off state on the exact 3.13.0 arithmetic: branch on the option, do not rely
  on κ = 0 or Dr = 1 reproducing the old bits.
- **Reports**: `Speaker2Report` (`apply.rs`) gains `lf_align` (f_lin, strength, arrival ms or none, limited),
  `mode_decay` (T_t and the cuts `(f_m, T_m, depth)`), `hf_phase` (rotation in degrees or no range, limited),
  each `None` when its option is off or does not apply. Service log (`crates/impulcifer-service/src/brir/run.rs`)
  and README writer (`crates/impulcifer-service/src/brir/outputs.rs`), next to the 3.13.0 keys:
  `cli_room_tuning2_lf_align` `{speaker, f_lin, strength, arrival, limited}`,
  `cli_room_tuning2_mode_decay` `{speaker, target, count, cuts}`, `cli_room_tuning2_mode_decay_none`
  `{speaker, target}`, `cli_room_tuning2_hf_phase` `{speaker, rotation, limited}`,
  `cli_room_tuning2_hf_phase_none` `{speaker, low, high}`, and `cli_room_tuning2_zero_inert` `{options}` once
  in zero mode. Number formats as the spec's "Diagnostics" says; `limited` and `none` the way the 3.13.0
  keys render them. Add the keys to **all nine** catalogs `crates/impulcifer-service/locales/<lang>.json`
  with English text and identical placeholders (translation is a later packet);
  `cargo test -p impulcifer-service --lib settings --test ui_catalog` must pass.

### Tests and gates

- `crates/impulcifer-dsp/tests/golden_secs2r.rs`: one test per new family, through
  `golden_secs2_helpers.rs`, in the class the fixture table gives. Print `MEASURE` lines like the bundle-1
  tests. Every bundle-1 test must keep passing unchanged.
- Synthetic unit tests of your own for what the fixtures leave thin: a mode within 1.5 Hz of another
  (the merge), the fifth cut never being made, a cut capped by the height rule, κ zero inside the long-mode
  mask and below the rolloff, the arrival bound binding at 1.5 periods, the unwind clip at −5 ms reporting
  `limited`, the no-range case, the zero-mode inert log, and the phase-resolution order (a case where the
  score before the Limit and after it choose differently, if you can build one; otherwise state why not).
- Byte identity: with the three options off every existing test, every `s2_*` and bundle-1 `s2r_*` gate and
  the `room_tuning_secs2_*` references pass unchanged, and the demo outputs are byte-identical to
  `origin/master` for every tuning2 scenario of `.github/scripts/brir_3x_integrity.py` (run its local
  comparison; report the hashes).
- `tests/migration/README-secs2r.md`: add the new families to the family list and their measured maxima to
  the gates table. Do not edit its "Regeneration" section (the maintainer writes it).
- `features.toml`: `config.room_tuning2_lf_align`, `config.room_tuning2_mode_decay`,
  `config.room_tuning2_hf_phase` (kind `config`) and `room.tuning2_lf_align`, `room.tuning2_mode_decay`,
  `room.tuning2_hf_phase` (kind `stage`), with real test names, as the 3.13.0 entries do.
  `cargo test -p impulcifer-policy` must pass.
- brir-3x: three scenarios tagged `"3.14.0"` in `.github/scripts/brir_3x_integrity.py`:
  `room_tuning2_lf_align` (`--room_mode=tuning2 --room_tuning2_mode=linear --room_tuning2_lf_align=true`),
  `room_tuning2_mode_decay` (`--room_mode=tuning2 --room_tuning2_mode_decay=true`),
  `room_tuning2_hf_phase` (`--room_mode=tuning2 --room_tuning2_hf_phase=true`).
- README: three rows in the CLI option table after `--room_tuning2_bands`, Korean, same style.
- Version `3.14.0` in `Cargo.toml` (`[workspace.package].version`), `apps/impulcifer-app/tauri.conf.json` and
  `Cargo.lock` (a build rewrites the lock). CHANGELOG entry at the top:

  ```
  ## 3.14.0 - <today>
  ### 가상 룸 튜닝 2의 저역 도착 맞춤, 모드 감쇠 컷, 고역 위상 되감기

  #### ⭐ 새로운 기능 / 개선
  - **…**: …

  #### 🐛 버그 수정
  - **…**: …
  ```

  in Korean, one bullet per option, saying that the defaults keep the output byte-identical and that the
  behaviour comes from SECS 2R through the clean-room method of ADR 0015, and one bug-fix bullet for the
  phase-resolution order with low-end protection. Keep the category headings' emoji exactly as above.

Do not touch: `apps/impulcifer-app/ui/*`, `webview_ui/`, `i18n/locales/`, `CLAUDE.md`, the ADR,
`docs/rust/ROOM_CORRECTION.md` (if the spec needs a correction, say so in the report instead), the `s2_*`
fixtures and every `s2r_*` fixture file.

## Verification (foreground, before reporting; `env -u HTTPS_PROXY -u HTTP_PROXY` in front of cargo if a
command fails on the network)

```
cd /e/Impulcifer/.worktrees/s2r2
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-cli -p impulcifer-policy
cargo test -p impulcifer-dsp
cargo test -p impulcifer-service
cargo test -p impulcifer-python
```

Then the brir-3x local comparison for every tuning2 scenario between `origin/master` and this branch, and a run
of each new scenario to show it completes.

## Report (in this order, then end your turn)

1. Files changed with a one-line purpose each.
2. The `MEASURE` table: every new family's observed maximum and the gate you set, and whether this machine has
   the oracle's libm.
3. The `s2r_rc_*` report-only maxima for the new settings (`peq` band-wise, `fir`, `response` in dB,
   `phase_strength`, and the lf/mode/hf outcomes side by side with the oracle's).
4. Byte identity: the hashes or the test names that prove the defaults are unchanged.
5. The exact commands run and their final lines.
6. Open questions and anything you could not do, with the reason.
