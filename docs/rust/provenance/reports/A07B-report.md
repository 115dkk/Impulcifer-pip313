## A07b is incomplete — fit gates remain red

Implemented the primitive gates, complete sidecar validation, optional fit tracing, fallible APIs, per-position onset correction, and the revised integration criteria. **I did not widen failing budgets or claim equivalence.** No commit, stash, or branch switch was made.

### 1. Fixture measurements versus budgets

The loader reads every sidecar value and verifies byte length and SHA-256.

| Family | Maximum absolute error | Maximum relative L2 | Gate |
|---|---:|---:|---|
| Smoothing | 0 | 0 | `1e-12` relative + absolute floor |
| `frac_for` | 0 | 0 | Same |
| Peaking / PEQ response | 0 | 0 | Same |
| Ring caps / boxes | 0 | 0 | Same |
| Onset / window | 0 | 0 | Exact indices; window `1e-12` |
| Late peaks | 0 | 0 | Exact |
| FIR / HP4 | `6.66134e-16` | `3.36657e-16` | `1e-12` |
| Butterworth band helpers | `6.43929e-15` | `1.46787e-14` | `1e-12` |
| Group-delay curve | `3.46390e-14` | `3.33576e-15` | `1e-12` |
| Low-rate conversion | `4.26326e-14` | `4.95603e-16` | `1e-12` |
| Mode T60 | `5.15143e-14` | `3.62426e-15` | `1e-12` |
| Minimum phase, combined dB/rad | `6.39488e-14` | `5.77879e-14` diagnostic | `1e-9` dB/rad |
| `levels_w1` | `7.10543e-15` dB | `9.29366e-17` | `1e-12` absolute |
| `levels_w5` | `7.10543e-15` dB | `9.20462e-17` | `1e-12` absolute |
| RC correction bands | 0 | 0 | `1e-12` absolute |
| RC levels | `1.42109e-14` dB | `1.53008e-16` | `1e-12` absolute |
| Synthetic J0, 165,000 values | **0** | **0** | `1e-9` relative |
| Full fit traces | **FAIL** | **FAIL** | Unchanged `1e-9` relative |

Passing measurements × 1.5 remain below the retained floors. Recorded them in `tests/migration/README-secs2.md:134`.

**Not implemented:** downstream RC stage capture and gates for every realization, touch-up, session selection, and final RC result. `golden_secs2_rc.rs` currently gates levels and bands only.

### 2. Divergences found

**Fixed**
- **PEQ phasor arithmetic:** NumPy’s complex division by `fs` multiplies by its reciprocal. Matching that evaluation order removed the peaking-response difference.
- **PEQ complex magnitude:** the observed NumPy baseline array kernel uses a scaled square root rather than libm `hypot`. Matching it made all five synthetic initial Jacobians exact.
- **Trace indexing:** fixture `jac_x` excludes initial J0; capture and count checks now reflect that.
- **Adjacent-bound feasibility:** a second bound check used an already modified value. Fixed it to preserve SciPy’s original active-bound decision, with a regression test.
- **Time origin:** each position now uses its own onset under both policies. Independently padding positions preserves spectra, levels, and low-rate analysis inputs exactly.
- `channel_level` and `correction_band` now return `Result<_, DspError>`.

**Unresolved synthetic trace divergences** — zero-based indices:

| Fixture | First failing `main_x` evaluation | Parameter | Absolute error |
|---|---:|---:|---:|
| `bumps` | 5 | 0 | `7.92236e-9` |
| `bumps_low_fmax` | 4 | 0 | `7.66158e-8` |
| `deep` | 15 | 0 | `9.11833e-8` |
| `ripple` | 1 | 0 | `2.48771e-6` |
| `zero_low_part` | 3 | 0 | `1.72767e-8` |

All actual RC fit fixtures also exceed the trace gate; their first failing evaluations and `MEASURE` lines were printed during verification.

The largest combined fit-field absolute error was **48.2611** (`fc_both_long_s0_0`); largest relative L2 was **7.57725** (`synthetic_zero_low_part`). These combine diagnostic fields with different units and **are not proposed budgets**.

I independently reproduced the SciPy sensitivity experiment:
- `gesdd`: all seven trial points matched the fixture exactly.
- Change only to `gesvd`: first violation at evaluation 6, maximum relative error `3.02005e-7`.
- Shift only singular values upward one ULP: first violation at evaluation 6, `2.99149e-7`.

This demonstrates factorization sensitivity. **It does not establish that all remaining Rust divergence comes from faer**; sequential versus BLAS reductions remain unresolved.

The SciPy-only reference was regenerated with the requested disabled-dispatch environment. Its PEQ16 test still fails: maximum x difference **0.121588**, cost difference **0.181745**, with `nfev=150` and status `0` matching.

### 3. End-to-end results

Coverage: **61 response references plus seven `all/mixed` levels-only references**.

| Band | Magnitude maximum | SECS limit | Median | SECS median |
|---|---:|---:|---:|---:|
| 15–100 Hz | 1.799839 dB | 1.86 | 0.018709 | 0.0066 |
| 100–500 Hz | **2.662352 dB** | **1.55** | 0.014428 | 0.0092 |
| 500–2000 Hz | 0.239375 dB | 0.41 | 0.001408 | 0.0012 |
| 2000–23000 Hz | 0.040506 dB | 0.049 | 0.000077 | 0.00004 |

| Channels exceeding | Rust/reference | SECS self-variance |
|---|---:|---:|
| 0.001 dB | 57 | 56 |
| 0.01 dB | 37 | 35 |
| **0.1 dB** | **25** | **22** |
| **1 dB** | **9** | **8** |

Failures:
- `two/BL/mixed`: 2.662352 dB in 100–500 Hz.
- `two/FR/mixed`: strength **0.9**, reference **0.7**.
- Both enforced distribution counts exceed their limits.

Phase maxima, report-only: **0.576677 / 0.859891 / 0.714015 / 0.700944 rad**.

### 3b. Performance

Current benchmark medians, FL with two positions:

| Mode | Rust | Supplied SECS timing | Required ≤ |
|---|---:|---:|---:|
| Mixed | 1.182287 s | 0.88 s | 0.088 s |
| Zero | 2.438057 s | 1.55 s | 0.155 s |
| Linear | 2.840953 s | 2.22 s | 0.222 s |
| Long | 1.134732 s | 0.73 s | 0.073 s |
| Ultra-60 | 2.532668 s | 1.91 s | 0.191 s |

**Every mode misses the target.** Per-step profiling and optimization were not completed; the prerequisite fit gates remain failing.

The regenerated integration fixture no longer contains `secs_seconds`. The comparison above uses the timings supplied in your request; the benchmark now reports missing timing metadata explicitly instead of printing `nulls`.

### 4. Contract questions

- README says `jac_x` is “every point where it formed a Jacobian,” but fixtures exclude the initial Jacobian point and store J0 separately. Capture follows the fixture.
- “Measured maximum × 1.5, rounded up, with a floor” does not explicitly enumerate all floors. Passing gates retain the existing primitive and spectral floors; failing trace budgets were not enlarged.
- No specification behavior was changed merely to make a fixture pass.

### 5. Verification

Final formatting and DSP Clippy checks passed.

The default full-suite build exhausted disk space. After clearing generated artifacts, an unoptimized retry exceeded the foreground tool limit and was stopped. The **complete suite subsequently finished** with test-profile optimization enabled and debug information disabled:

```text
golden_secs2_primitives:
test result: ok. 13 passed; 0 failed

golden_secs2_rc:
test result: ok. 1 passed; 0 failed

golden_secs2_fit:
test result: FAILED. 3 passed; 3 failed

room_tuning2_rc:
test result: FAILED. 5 passed; 1 failed

trf:
test result: FAILED. 0 passed; 1 failed

error: 3 targets failed:
    golden_secs2_fit
    room_tuning2_rc
    trf
```

All other DSP test targets passed in that completed run. No commands remain running.

### 6. `git status --porcelain`

```text
 M crates/impulcifer-dsp/benches/perf.rs
 M crates/impulcifer-dsp/src/optimize.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/mod.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/peq.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/primitives.rs
 M crates/impulcifer-dsp/tests/room_tuning2_rc.rs
 M crates/impulcifer-dsp/tests/trf.rs
 M tests/migration/README-secs2.md
 M tests/migration/gen_trf_reference.py
 M tests/migration/goldens/trf_scipy_reference.json
?? crates/impulcifer-dsp/tests/golden_secs2_fit.rs
?? crates/impulcifer-dsp/tests/golden_secs2_helpers.rs
?? crates/impulcifer-dsp/tests/golden_secs2_primitives.rs
?? crates/impulcifer-dsp/tests/golden_secs2_rc.rs
```
