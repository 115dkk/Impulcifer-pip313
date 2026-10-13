Implemented and verified on the checked-out branch. No delegation, branch switch, commit, stash, or prohibited-source access.

**All fit-prefix gates pass; all 235 DSP tests and 9 policy tests pass. The 4× single-thread performance target remains unmet.**

## 1. Replay agreement, divergence, and fix

The specification-only replay reproduces the target fixture exactly:

```text
initial_cost = 473.05958254434665
main_cost[0] = 350.9546648772942 — exact
main_x first divergence = None
prefix_max_relative = 0
final cost = 186.51429844881537 — exact
```

Before the fix, the first traced quantity exceeding \(10^{-12}\) relative was **actual reduction**, at `6.866091888814059e-12`. The preceding step differed by only `2.982102055413248e-15`.

Both implementations selected **anti-gradient**. Thus, identical divergence under different SVD builds did not establish a formula mismatch: this selected step does not depend on the SVD step.

The effective fix replaces the long scalar dot-product accumulator with **eight independent accumulators and a balanced merge**, without changing residual arithmetic, introducing FMA, or changing a TRF branch rule. The tiny first-step difference had been amplified by the next finite-difference Jacobian.

Afterward:

```text
REPLAY_INTERNAL uf:               relative=2.34639164540836092e-13
REPLAY_INTERNAL step:             relative=1.35550093427874888e-15
REPLAY_INTERNAL x_new:            relative=2.23732662532509891e-17
REPLAY_INTERNAL actual_reduction: relative=4.65529316483426622e-16
FIRST_INTERNAL None
```

The replay JSON is **19,947 bytes**, at the requested path. Rust tracing is opt-in and records inner trials, including rejected trials. The regression test checks first-iteration quantities and selection diagnostics.

## 2. Final fit table

All **27/27** traced fixtures pass before `horizon − 1`. Maximum gated relative distance: **`2.3398629199314537e-14`**, against `1e-9`.

Names below omit `fit_`. Variant ranges include all recorded variants. Final costs remain diagnostic—not an additional gate.

| Fixture | First divergence / horizon | Rust cost | Reference | Variant cost range |
|---|---:|---:|---:|---:|
| synthetic_bumps | 5 / 6 | 5.244361 | 5.754195 | 5.427122–5.778734 |
| synthetic_bumps_low_fmax | 4 / 4 | 92.108498 | 113.441997 | 59.658200–110.719988 |
| synthetic_deep | 13 / 12 | 68.453834 | 64.788364 | 57.793379–69.468527 |
| synthetic_ripple | 1 / 1 | 0.179101 | 0.179149 | 0.179137–0.180039 |
| synthetic_zero_low_part | 3 / 3 | 3.826045 | 3.647454 | 3.602040–3.896615 |
| bl_both_zero_s0_0 | 4 / 2 | 216.682248 | 216.682187 | 216.681871–216.682993 |
| bl_both_zero_s0_1 | 1 / 1 | 1.694132 | 1.694129 | 1.693656–1.694133 |
| br_both_mixed_band30_300_s0_0 | 1 / 1 | 102.584253 | 102.584701 | 102.584212–102.584887 |
| br_both_mixed_band30_300_s0_1 | 4 / 3 | 77.516357 | 77.516356 | 77.516346–77.516376 |
| fc_both_long_s0_0 | 2 / 2 | 516.107983 | 533.777770 | 504.044550–534.106090 |
| fc_both_long_s0_1 | 2 / 2 | 17.036178 | 16.887985 | 16.869176–17.019826 |
| fl_both_mixed_main5_s0_0 | 3 / 4 | 142.017469 | 142.013817 | 142.012669–142.017226 |
| fl_both_mixed_main5_s0_1 | 2 / 2 | 102.496423 | 102.495217 | 102.494986–102.540863 |
| fl_both_mixed_s0_0 | 5 / 4 | 139.211470 | 139.211583 | 139.211448–139.211920 |
| fl_both_mixed_s0_1 | 4 / 3 | 100.716918 | 100.717054 | 100.717010–100.717183 |
| fl_both_mixed_target_volume40_s0_0 | 7 / 6 | 46.744317 | 45.761300 | 46.351653–46.975436 |
| fl_both_mixed_target_volume40_s0_1 | 7 / 6 | 30.069551 | 30.141802 | 30.016343–30.142594 |
| fl_left_mixed_s0_0 | 4 / 2 | 144.750560 | 144.686052 | 144.671306–144.820368 |
| fl_left_mixed_s0_1 | 5 / 6 | 124.706386 | 124.706237 | 124.706149–124.706331 |
| sl_both_linear_s0_0 | 6 / 5 | 113.873650 | 113.873643 | 113.873653–113.873690 |
| sl_both_linear_s0_1 | 2 / 2 | 132.797837 | 132.797766 | 132.797755–132.797784 |
| sl_both_linear_s1_0 | 6 / 5 | 113.873650 | 113.873643 | 113.873653–113.873690 |
| sl_both_linear_s1_1 | 3 / 3 | 115.248301 | 115.247564 | 115.247554–115.248349 |
| sr_both_ultra60_s0_0 | 3 / 1 | 172.336137 | 172.335721 | 172.335698–172.336019 |
| **sr_both_ultra60_s0_1** | **5 / 5** | **186.514695** | **186.514298** | **186.514099–186.514589** |
| sr_both_ultra60_s1_0 | 3 / 1 | 172.336137 | 172.335721 | 172.335698–172.336019 |
| sr_both_ultra60_s1_1 | 2 / 3 | 153.919294 | 154.779981 | 153.917976–154.780602 |

```text
MEASURE fit_sr_both_ultra60_s0_1/prefix:
max_relative=1.82834102374497141e-16, evaluations=4, budget=1e-9
FIT fit_sr_both_ultra60_s0_1:
first=Some(5) horizon=Some(5)
cost=1.86514695448074178e2 reference=1.86514298448815367e2
nfev=74/78
```

## 3. SVD path retained

Retained **sequential Householder QR → SVD of R → U_Rᵀc**. No tall U is formed; every faer operation explicitly uses `Par::Seq`.

Observed release-profile per-call averages:

| Augmented shape | Previous tall thin-U SVD | Retained QR + small SVD |
|---|---:|---:|
| 748×48, mixed | 1.533 ms | 1.028 ms |
| 948×48, zero | 1.796 ms | 1.387 ms |

These are profile averages, not a fixed-matrix microbenchmark; changed rounding also changes later iteration counts. Fit gates and the end-to-end envelope both pass.

## 4. Residual changes

Retained safe `pulp::Arch::new().dispatch(...)` around band/product/magnitude work. Magnitude and scalar `log10` are separate loops; `log10` remains the scalar library call.

- All five synthetic **J0 arrays remain bit-exact**.
- Mixed residual average: approximately **28.0 µs → 19.8 µs**.
- Final zero-mode residual average: **26.45 µs**, at 900 points.
- Host-load variability was visible; no arithmetic relaxation was used.

## 5. End-to-end distribution

```text
MEASURE RC distribution:
thresholds=[0.001, 0.01, 0.1, 1.0]
ours=[56, 36, 19, 7]
envelope=[61, 39, 32, 15]
max_mag_db=[
  1.9942350486803289,
  1.919082891777603,
  0.7653187289206791,
  0.057842252870441715
]
strength_changes=2
peq_count_changes=0
```

All counts are below the envelope. Band maxima pass the existing envelope-derived caps: `[2.91, 3.33, 0.93, 0.08]` dB. The upper two maxima slightly exceed the raw variant maxima, but not those established caps.

## 6. Timings and profiles

FL, two positions; medians of three release runs. Default pool: **4 threads**.

| Mode | Oracle | 1 thread | Speedup | Default pool | Speedup |
|---|---:|---:|---:|---:|---:|
| Mixed | 0.88 s | 0.358733 s | 2.453× | 0.340503 s | 2.584× |
| Zero | 1.55 s | 0.612948 s | 2.529× | 0.631765 s | 2.453× |
| Linear | 2.22 s | 0.591165 s | 3.755× | 0.329069 s | 6.746× |
| Long | 0.73 s | 0.228424 s | 3.196× | 0.283438 s | 2.576× |
| Ultra-60 | 1.91 s | 0.606082 s | 3.151× | 0.348594 s | 5.479× |

Linear and ultra sessions now use `rayon::join`. The new 1-thread/4-thread linear regression test passes, including bitwise FIR and complex-response comparisons.

### Single-thread profile, milliseconds

Timers are inclusive; **do not sum columns**.

| Mode | Fit total | Jacobian | Residual | QR/SVD | Spectra | Smoothing | Min phase | Realization | Touch-up |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Mixed | 259.85 | 114.87 | 114.60 | 119.29 | 6.34 | 16.95 | 13.97 | 15.64 | 21.55 |
| Zero | 587.19 | 260.14 | 259.41 | 274.54 | 6.57 | 2.89 | — | — | — |
| Linear | 402.34 | 173.38 | 172.35 | 186.42 | 9.17 | 35.40 | 28.46 | 39.83 | 29.19 |
| Long | 200.00 | 87.06 | 87.36 | 92.28 | 7.00 | 15.94 | 12.25 | 9.87 | 11.88 |
| Ultra-60 | 468.08 | 201.24 | 201.87 | 216.51 | 9.18 | 34.52 | 32.87 | 20.54 | 27.24 |

Default-pool profiles include both concurrent sessions:

| Mode | Fit total | Jacobian | Residual | QR/SVD |
|---|---:|---:|---:|---:|
| Mixed | 257.81 | 112.67 | 112.24 | 119.96 |
| Zero | 641.52 | 284.62 | 284.29 | 296.42 |
| Linear | 414.78 | 181.71 | 181.15 | 191.72 |
| Long | 214.57 | 93.85 | 94.30 | 98.54 |
| Ultra-60 | 490.38 | 211.89 | 212.29 | 226.50 |

### Requested compute floors

Measured scalar `log10`: **7.707877 ns/point**.

| Mode | Residual evaluations × points | SVD calls × ms/call | Combined floor |
|---|---:|---:|---:|
| Mixed | 5,798 × 700 | 116 × 1.028393 | 0.150577 s |
| Zero | 9,808 × 900 | 198 × 1.386555 | 0.342577 s |
| Linear | 9,096 × 700 | 181 × 1.029960 | 0.235500 s |
| Long | 4,384 × 700 | 88 × 1.048681 | 0.115938 s |
| Ultra-60 | 9,247 × 700 | 185 × 1.170340 | 0.266405 s |

These floors omit band arithmetic, products, Jacobian assembly, FFTs, and other work.

## 7. Verification

All requested commands completed in the foreground.

- `cargo fmt --all -- --check`: pass.
- DSP clippy, all targets, warnings denied: pass.
- Replay Python ruff: pass.
- Policy: **9 passed**.
- Full DSP suite: **235 passed**, zero failures.
- Explicit fit and RC `--nocapture --test-threads=1` runs: pass.
- Both release benchmarks: completed.
- `git diff --check`: pass.

Relevant tails:

```text
# Policy
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s

# Final full-suite fit target
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.18s

# Final full-suite RC target
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 70.34s

# TRF
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.04s
```

An initial policy run found missing `#![forbid(unsafe_code)]` in the fit test and shared helper; both were fixed and policy reran green. An intermediate benchmark compile failed from a shadowed variable; that was fixed before both final runs.

## 8. `git status --porcelain`

```text
 M crates/impulcifer-dsp/benches/perf.rs
 M crates/impulcifer-dsp/src/optimize.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/peq.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/profile.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/rc.rs
 M crates/impulcifer-dsp/tests/golden_secs2_fit.rs
 M crates/impulcifer-dsp/tests/golden_secs2_helpers.rs
 M crates/impulcifer-dsp/tests/room_tuning2_rc.rs
 M tests/migration/README-secs2.md
 M tests/migration/goldens/trf_replay_sr_both_ultra60_s0_1.json
 M tests/migration/replay_trf_fixture.py
```

## 9. Undone / limits

- **Every mode still misses the requested 4× single-thread target.**
- Final fit costs are not necessarily inside the oracle variants’ cost range; only the specified prefix and RC-envelope gates establish acceptance.
- SVD timings are workload-profile averages, not controlled fixed-matrix microbenchmarks.
- No version bump, changelog, commit, or A08-file changes were made, respecting this packet’s file and workflow restrictions.
