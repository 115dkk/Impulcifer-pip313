## A08 status: implemented in the worktree, but acceptance is incomplete

All foreground commands have finished. **No commits, branch changes, stash operations, delegation, or edits to the protected A07d files were made.**

The required “only the root-permission test fails” condition is **not met**: three DSP tests fail, plus the expected service permission test.

### 1. API implemented

Added in `crates/impulcifer-dsp/src/stages/room_tuning2/`:

- `MimoOptions`, `MimoInput`, `MimoSupport`, `MimoInputResult`, `MimoDesign`
- `design_mimo`, `support_response`
- `Tuning2Options`, `Tuning2Plan`, `PlannedSpeaker`
- `Tuning2Report`, `Speaker2Report`
- `prepare_tuning2`, `apply_tuning2`
- Diagnostic boundaries for fixture testing: `MimoStages`, `design_paths`, `normal_equations`, `bisect`

Integration added:

- `RoomCorrection::tuning2`
- `room::room_correction_tuning2`, consuming uncropped omni measurements
- Application after `crop_tails`
- `StageObserver::on_room_tuned2`
- `ReadmeData::tuning2`
- Virtual-bass room gains restricted to `room_mode == "eq"`

No configuration, CLI, catalogue, feature registry, or service-behaviour changes.

### 2. Normal equations and solver

Gram blocks are constructed directly along diagonals using short-end prefix sums, without subtracting long accumulated sums. The solver uses **one sequential faer symmetric eigendecomposition**, reused for nineteen regularizations, with two residual-refinement passes.

No faer features or dependencies changed.

| Dense cross-check | Relative L2 error |
|---|---:|
| Normal matrix | 4.367886×10⁻¹⁶ |
| `Aᵀb` | 1.271838×10⁻¹⁵ |
| Kept `u` | 6.564296×10⁻¹⁴ |
| Bisection decisions | **19/19 identical** |
| Kept solve index | **15/15** |

Across fourteen path fixtures, normal-equation maximum absolute error was **2.602085×10⁻¹⁸**, with relative L2 **1.391072×10⁻¹⁵**.

One discarded, ill-conditioned trial’s ratio differs by **1.678269×10⁻⁹ relative**, without changing its decision. The provisional ratio allowance is `3e-9`; kept-`u` comparisons pass the existing `1e-9` relative plus `1e-12` component floor.

**Not completed:** head-to-head timing of eigendecomposition versus nineteen Cholesky solves.

### 3. MIMO reference results and blockers

Ranges match exactly for all three settings; channel gains agree within `1e-10 dB`. FR is off in both mixed settings, matching the oracle. No linear input is off.

| Setting | Changed guard steps | Worst oracle variant | Result |
|---|---:|---:|---|
| Linear / 150 Hz | 1 | 2 | Count gate passes |
| Mixed / 100 Hz / low 40 | 3 | 3 | Count gate passes |
| Mixed / 150 Hz | **4** | **2** | **Fails** |

Full per-input level differences, support relative/max errors, and late metrics are in `docs/rust/perf/room-tuning2.md`. The test also prints every oracle variant alongside Rust.

**Deterministic guard conflict:** the specification says:

> “modes = sorted set of `round(p, 1)` over `LP({x_e}, 6)` and `LP({y_e}, 6)`.”

For mixed/100, BL, the specified union includes a new after-peak at **44.8 Hz**:

- Rust: **1.006313368383 dB**, rejecting scale 0
- Independent SciPy calculation: **1.006313368383 dB**
- Oracle fixture: **0.906165519807 dB**, accepting scale 0

The oracle value matches the maximum over the before-only peaks. FC also differs at its first trial—**4.010703 vs 3.068970 dB**—although both accept −3 dB.

The implementation retains the written union rule. **The guard gate remains failing; its tolerance was not widened.**

### 4. Demo application results

The omni-prepared demo has sixteen PEQ bands per speaker, no weak flags, finite outputs, and **bit-identical samples with one and four rayon threads**. Geometry pairs were not populated in this test, so “no mismatch” is not a positive geometry validation.

| Speaker | Level dB | Trim dB | Strength | EDT: before / RC / MIMO, ms | MIMO scale | Late energy vs RC |
|---|---:|---:|---:|---|---:|---:|
| FL | −40.889121 | −0.418035 | .7 | 461.256 / 463.549 / 372.769 | −6 dB | −5.272899 dB |
| FR | −40.882314 | −0.424842 | .9 | 492.821 / 511.481 / 349.200 | −3 dB | −6.117236 dB |
| FC | −40.680217 | −0.626939 | .7 | 452.473 / 438.778 / 300.764 | −3 dB | −7.919117 dB |
| BL | −40.811125 | −0.496030 | .7 | 457.634 / 417.386 / 283.639 | −3 dB | −8.056308 dB |
| SL | −40.999350 | −0.307806 | .9 | 479.726 / 479.124 / 341.403 | −3 dB | −6.807418 dB |
| SR | −40.492690 | −0.814466 | .3 | 463.646 / 448.772 / 314.377 | −3 dB | −8.196525 dB |
| BR | −41.307156 | 0 | .9 | 457.112 / 469.150 / 398.477 | −3 dB | −6.440456 dB |

Every input has six supports in this demo. Excess-GD tables are in the report; per-ear pre-echo values are printed by the demo test.

### 5. Performance

Release-profile measurements:

| Operation | Rust time |
|---|---:|
| RC zero, FL/two positions | 0.857882 s |
| RC mixed | 0.407558 s |
| RC linear | 0.936585 s |
| RC long | 0.528135 s |
| RC ultra-60 | 0.841699 s |
| MIMO design, seven demo inputs | **1.080584 s** |
| Amortized per input | 0.154369 s |
| Complete demo application, excluding preparation | **1.364873 s** |
| Synthetic 24 speakers, one input | **23.479461 s** |

RC timings are three-run medians; MIMO timings are single runs. Peak process RSS was **1,162,252 KiB**, measured using `resource.getrusage`; `/usr/bin/time` is unavailable.

Against ADR 0008’s historical approximately six-second SECS ideal-FIR design, the demo result is roughly **5.6× faster**, but this is **not a fresh matched comparison**. Regenerated goldens lack the `secs_seconds` values expected by the existing RC benchmark. The 24-speaker case still needs optimization.

### 6. Verification results

- `cargo fmt --all -- --check`: **passed**
- Required strict clippy command: **passed**
- Complete DSP suite: **three failing targets**
- Complete service suite: **only the expected root-permission failure**

Relevant final results:

```text
golden_secs2_fit:
test result: FAILED. 3 passed; 1 failed
  b_actual_fit_gates
  fit_sr_both_ultra60_s0_1: evaluation 1 distance 1.2477142515202155e-6

golden_secs2_mimo:
test result: FAILED. 1 passed; 1 failed
  BL and FC mixed/100 tail-guard disagreements

room_tuning2_apply:
test result: FAILED. 8 passed; 1 failed
  room_tuning2_matches_secs2_mimo: mixed/fsiso150/auto: 4>2

room_tuning2_mimo:
test result: ok. 2 passed; 0 failed

room_tuning2_rc:
test result: ok. 6 passed; 0 failed

room_tuning_secs:
test result: ok. 3 passed; 0 failed

room_v2:
test result: ok. 14 passed; 0 failed

brir_outputs:
test result: FAILED. 2 passed; 1 failed
  output_readonly_file_failure_preserves_existing_bytes
  assertion failed: result.is_err()
```

The fit-prefix failure is already documented in the worktree’s A07c record; its implementation and test files were left untouched. Existing v1 and EQ tests passed.

### 7. `git status --porcelain`

```text
 M crates/impulcifer-dsp/benches/perf.rs
 M crates/impulcifer-dsp/src/pipeline.rs
 M crates/impulcifer-dsp/src/stages/readme.rs
 M crates/impulcifer-dsp/src/stages/room.rs
 M crates/impulcifer-dsp/src/stages/room_tuning.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/mod.rs
 M crates/impulcifer-dsp/src/stages/room_v2.rs
 M tests/migration/README-secs2.md
?? crates/impulcifer-dsp/src/stages/room_tuning2/apply.rs
?? crates/impulcifer-dsp/src/stages/room_tuning2/mimo.rs
?? crates/impulcifer-dsp/tests/golden_secs2_mimo.rs
?? crates/impulcifer-dsp/tests/room_tuning2_apply.rs
?? crates/impulcifer-dsp/tests/room_tuning2_mimo.rs
?? docs/rust/perf/room-tuning2.md
```

### 8. Remaining work

Beyond the failing gates:

- **“report how much of the support difference that explains”** — RC perturbation study not done.
- **“Set bounds as named constants”** — end-to-end support/late-error bounds not established.
- **“compare in dependency order (paths → normal equations → … → supports)”** — normal equations, solves and guards are tested from frozen paths; independent path and final-support fixture gates remain incomplete.
- **“tuned BRIR minus the RC-only result … beyond −80 dB”** — current test checks the support response’s frequency cutoff, not the stronger finite-output difference and sum identity.
- **“zero mode: no delay, unchanged lengths”** — explicit zero-mode application test remains missing.
- Positive geometry-pair coverage and a dedicated pipeline observer test remain missing.
- Public diagnostic APIs need input-shape hardening before being treated as general-purpose APIs.

The report exists at **`docs/rust/perf/room-tuning2.md`**. Its attachment upload failed with server HTTP 400, so it was **not delivered as an attachment**.

**This worktree is not ready for acceptance or merge.**
