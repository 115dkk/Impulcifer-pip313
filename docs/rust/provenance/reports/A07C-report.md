## Status: partial — one fit gate still fails; speed target not met

All requested verification commands completed. Changes remain uncommitted on the original branch. No SECS source or exporter was opened or run.

Implemented:
- Horizon-based fit gates and exact synthetic `J0` assertions.
- SciPy PEQ16 `gesvd` reference and horizon gate.
- Optional production-chain RC stage capture and comparisons.
- File-driven end-to-end envelope gate.
- Opt-in profiling, reusable residual/Jacobian-probe buffers, cached band responses and exact-order prefix products, and scalar smoothing-power reuse.

### 1. Fit fixtures

Indices are zero-based `main_x` indices. The enforced prefix is `index < horizon − 1`.

| Fixture, omitting `fit_` | First >1e−9 | Horizon |
|---|---:|---:|
| synthetic_bumps | 5 | 6 |
| synthetic_bumps_low_fmax | 4 | 4 |
| synthetic_deep | 15 | 12 |
| synthetic_ripple | 1 | 1 |
| synthetic_zero_low_part | 3 | 3 |
| bl_both_zero_s0_0 | 2 | 2 |
| bl_both_zero_s0_1 | 1 | 1 |
| br_both_mixed_band30_300_s0_0 | 1 | 1 |
| br_both_mixed_band30_300_s0_1 | 3 | 3 |
| fc_both_long_s0_0 | 2 | 2 |
| fc_both_long_s0_1 | 2 | 2 |
| fl_both_mixed_main5_s0_0 | 4 | 4 |
| fl_both_mixed_main5_s0_1 | 2 | 2 |
| fl_both_mixed_s0_0 | 4 | 4 |
| fl_both_mixed_s0_1 | 4 | 3 |
| fl_both_mixed_target_volume40_s0_0 | 6 | 6 |
| fl_both_mixed_target_volume40_s0_1 | 6 | 6 |
| fl_left_mixed_s0_0 | 1 | 2 |
| fl_left_mixed_s0_1 | 5 | 6 |
| sl_both_linear_s0_0 | 5 | 5 |
| sl_both_linear_s0_1 | 2 | 2 |
| sl_both_linear_s1_0 | 5 | 5 |
| sl_both_linear_s1_1 | 3 | 3 |
| sr_both_ultra60_s0_0 | 3 | 1 |
| **sr_both_ultra60_s0_1** | **1** | **5** |
| sr_both_ultra60_s1_0 | 3 | 1 |
| sr_both_ultra60_s1_1 | 2 | 3 |

**26 of 27 traced fixtures pass.** Ripple needs no fix under the new contract.

The unresolved failure is:

```text
sr_both_ultra60_s0_1:
evaluation 1 distance 1.2477142515202155e-6
evaluation 2 distance 4.075258712704426e-6
evaluation 3 distance 1.6303774481388997e-4
horizon 5
```

Its initial Jacobian was independently compared against SciPy using the frozen inputs:

```text
J0 exact True max 0.0 different 0
```

That rules out an initial residual/Jacobian mismatch, but does **not** establish the later divergence’s cause. No pre-horizon defect was fixed; this failure remains enforced.

All five synthetic `J0` arrays remain exact. The independent PEQ16 problem passes with first divergence 1, horizon 1:

```text
MEASURE trf_peq16: first=Some(1) horizon=Some(1)
cost=637.5784553041383
reference=637.3967098266643
gesvd=637.2215464462947
```

### 2. RC stages

All 11 sessions pass the production-chain gates through the first fit’s input:

| Pre-fit field | Largest absolute difference |
|---|---:|
| Power | 5.4210e−19 |
| Both smoothings | 8.6736e−19 |
| Design curve | 5.6843e−14 |
| Long-mode bins | Exact |
| First fit `corr_db` | 4.3521e−14 |

First observed downstream differences follow. “Size” is maximum absolute real/imaginary component difference, not dB.

| Session | First divergent captured stage | Size | First-fit horizon |
|---|---|---:|---:|
| bl_both_zero_s0 | response | 2.9340e−4 | 2 |
| br_both_mixed_band30_300_s0 | M | 1.4119e−4 | 1 |
| fc_both_long_s0 | M | 0.176575 | 2 |
| fl_both_mixed_s0 | M | 1.2828e−4 | 4 |
| fl_both_mixed_main5_s0 | M | 3.6351e−4 | 4 |
| fl_both_mixed_target_volume40_s0 | M | 0.048877 | 6 |
| fl_left_mixed_s0 | M | 6.0610e−4 | 2 |
| sl_both_linear_s0 | M | 3.5382e−4 | 5 |
| sl_both_linear_s1 | M | 3.5382e−4 | 5 |
| sr_both_ultra60_s0 | M | 9.3052e−5 | 1 |
| sr_both_ultra60_s1 | M | 9.3052e−5 | 1 |

M, phasors, smoothed phasors, raw/smoothed delays, FIR realizations, touch-up minimum phases, final FIR, response and PEQ are compared report-only for these post-fit paths.

### 3. End-to-end envelope: PASS

```text
MEASURE RC distribution:
thresholds=[0.001, 0.01, 0.1, 1.0]
ours=[57, 37, 25, 9]
envelope=[61, 39, 32, 15]
strength_changes=1
peq_count_changes=0
```

| Implementation | Max dB: 15–100 / 100–500 / 500–2000 / 2000–23000 Hz | >0.1 dB | >1 dB | Strength changes |
|---|---|---:|---:|---:|
| Rust | 1.79984 / 2.66235 / 0.23938 / 0.04051 | 25 | 9 | 1 |
| NumPy AVX-512 | 1.86293 / 1.54880 / 0.40903 / 0.04917 | 22 | 8 | 1 |
| OpenBLAS Haswell | 2.32659 / 2.66062 / 0.22957 / 0.04004 | 24 | 7 | 1 |
| OpenBLAS Sandybridge | 2.08735 / 1.60244 / 0.41141 / 0.04057 | 22 | 9 | 1 |
| OpenBLAS Nehalem | 1.98218 / 1.53148 / 0.47512 / 0.04044 | 21 | 7 | 1 |
| LAPACK gesvd | 1.96976 / 1.96913 / 0.74228 / 0.05708 | 24 | 9 | 2 |
| Envelope | 2.32659 / 2.66062 / 0.74228 / 0.05708 | 32 | 15 | 0¹ |
| Enforced magnitude caps | **2.91 / 3.33 / 0.93 / 0.08** | — | — | — |

¹ The envelope file reports zero; individual variants report one or two. PEQ-count changes are zero for Rust and every variant.

Coverage: **61 response references and 7 levels-only references**.

### 4. Performance: target missed

Release-profile medians, FL with two positions:

| Mode | Before | After | Oracle supplied in packet | Target |
|---|---:|---:|---:|---:|
| mixed | 1.188603 s | 1.144080 s | 0.88 s | 0.088 s |
| zero | 2.327410 s | 2.344558 s | 1.55 s | 0.155 s |
| linear | 2.947379 s | 2.790939 s | 2.22 s | 0.222 s |
| long | 1.131056 s | 1.053549 s | 0.73 s | 0.073 s |
| ultra-60 | 2.167144 s | 2.505206 s | 1.91 s | 0.191 s |

Profile before → after, milliseconds. These are **inclusive single-run timers**, not additive categories or benchmark medians.

| Step | mixed | zero | linear | long | ultra-60 |
|---|---:|---:|---:|---:|---:|
| Spectra | 6.77→8.75 | 7.41→8.77 | 5.80→8.62 | 7.07→8.54 | 6.66→6.56 |
| Smoothing | 22.16→17.84 | 3.28→2.82 | 45.31→30.33 | 23.28→13.22 | 38.88→31.31 |
| Long-mode analysis | .36→.42 | .31→.37 | .36→.42 | .32→.41 | .42→.50 |
| Fits, total | 1130→1027 | 2227→2207 | 2364→2276 | 995→967 | 2137→2134 |
| Residuals | 929→838 | 1841→1808 | 1946→1863 | 813→790 | 1757→1731 |
| Jacobians | 907→816 | 1801→1766 | 1900→1815 | 794→769 | 1718→1691 |
| SVD | 164→157 | 314→330 | 343→344 | 146→146 | 311→326 |
| Minimum phases | 35.20→34.98 | — | 66.66→64.27 | 43.03→27.59 | 65.33→72.66 |
| Realizations | 15.58→15.51 | — | 28.15→25.86 | 14.03→11.31 | 19.89→25.87 |
| Touch-up | 45.38→43.44 | — | 90.10→68.32 | 29.07→28.21 | 68.07→79.31 |

There is no convincing overall speed improvement across all modes. Residual/Jacobian work remains dominant. Shared FFT plans and scratch buffers were already cached; they were not rewritten. SVD remains sequential on the original augmented matrix.

### 5. Verification

Tests used:

```text
CARGO_PROFILE_TEST_OPT_LEVEL=2
CARGO_PROFILE_TEST_DEBUG=0
CARGO_PROFILE_TEST_INCREMENTAL=false
```

- `cargo fmt --all -- --check`: PASS
- Strict DSP clippy: PASS
- `git diff --check`: PASS
- Full DSP suite: **FAIL — only `golden_secs2_fit` failed**
- Benchmark: completed

Relevant result lines:

```text
golden_secs2_fit:
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

golden_secs2_primitives:
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

golden_secs2_rc:
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

room_tuning2_rc:
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

trf:
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

error: 1 target failed:
    `-p impulcifer-dsp --test golden_secs2_fit`
```

Every other full-suite target passed.

### 6. `git status --porcelain`

```text
 M crates/impulcifer-dsp/benches/perf.rs
 M crates/impulcifer-dsp/src/optimize.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/mod.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/peq.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/primitives.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/rc.rs
 M crates/impulcifer-dsp/tests/golden_secs2_fit.rs
 M crates/impulcifer-dsp/tests/golden_secs2_rc.rs
 M crates/impulcifer-dsp/tests/room_tuning2_rc.rs
 M crates/impulcifer-dsp/tests/trf.rs
 M tests/migration/README-secs2.md
 M tests/migration/gen_trf_reference.py
 M tests/migration/goldens/trf_scipy_reference.json
?? crates/impulcifer-dsp/src/stages/room_tuning2/profile.rs
```

### 7. Undone / limitations

- **`sr_both_ultra60_s0_1` pre-horizon divergence remains unresolved.**
- **No mode meets the 0.1× oracle timing target.**
- Profiling aggregates fits per mode; it does not yet separately report each fit or exclusive “rest of TRF” time.
- Thin-QR/SVD reduction and variable-resolution smoothing-window caching were not implemented.
- The end-result RC comparison is report-only for the current fixtures; it needs conditional gating if a future fixture has no oracle fit divergence.
- The initial worker launch recursively delegated and exceeded the foreground timeout. I stopped that process tree and continued directly; subsequent verification commands completed in the foreground.

This is a verified partial implementation, **not a completed A07c**.
