## 1. Hypothesis table and rule kept

**None of H1, H2, or H3 reproduces every oracle value.** Following your instruction, I made no new runtime implementation or solver changes. H1 remains unchanged, **not accepted**.

Compared all **43 band values + 42 tail values** across all 14 per-input fixtures. Tail differences below use the fixture’s units: **0.15 × dB**.

| Rule | Values compared | Within absolute 1e-9 | Worst difference | Location |
|---|---:|---:|---:|---|
| H1: before ∪ after | 85 | 83 | 0.14126000335392147 | mixed/100 FC, trial 0, tail |
| H2: before only | 85 | 64 | 0.7034637652374921 | mixed/150 FL, trial 0, tail |
| H3: after only | 85 | 73 | 0.6622879601134717 | mixed/100 FR, trial 1, tail |

H2 fixes the two known H1 discrepancies but introduces **21 others**. Switching to before-only is therefore unsupported.

Every mismatch—including input, trial, before/after peaks, selected modes, and per-mode `a`, `b`—is recorded in **`docs/rust/perf/room-tuning2.md`**. The new `mimo_guard_hypotheses` test reproduces the table directly from frozen paths and kept `u`, independently of Rust RC and the solver.

## 2. MIMO gates in dependency order

| Stage | Result |
|---|---|
| Production path reconstruction | Not added after the stop condition |
| Frozen paths → normal equations | PASS |
| Normal equations → bisection/kept solution | PASS |
| Guard | **FAIL**, unchanged 1e-9 gate |
| Final supports/level gate | Not added |
| Application extensions | Not added |

```text
MEASURE MIMO normal equations:
max_abs=2.60208521396521064e-18
max_relative_l2=1.39107225567603637e-15
values=120960

MEASURE MIMO solve:
max_abs=4.69810181868979271e-8
max_relative_l2=1.67826904350741651e-9
values=15386

MEASURE MIMO guard:
max_abs=1.41260003353902486e-1
max_relative_l2=3.01715670796333379e-1
values=85

MEASURE dense normal matrix=4.367886386622643e-16
rhs=1.2718377008542517e-15
MEASURE dense kept u=6.564296201359275e-14
decisions=19/19 kept=15/15
```

## 3. End-to-end versus every oracle variant

Changed guard-step counts:

| Setting | Rust | NumPy AVX-512 | Haswell | Sandybridge | Nehalem | gesvd | Gate |
|---|---:|---:|---:|---:|---:|---:|---|
| linear/150/auto | 1 | 1 | 2 | 1 | 1 | 1 | PASS |
| mixed/100/low40 | 2 | 1 | 1 | 1 | 1 | 3 | PASS |
| mixed/150/auto | **4** | 1 | 1 | 1 | 1 | **2** | **FAIL** |

Ranges and gains pass their existing exact/1e-10 checks. The refreshed report contains all per-input support, level, and late metrics alongside every variant. **No new provisional bounds were introduced while the guard remains unresolved.**

## 4. RC attribution

The RC-substitution experiment was not performed. However, the hypothesis experiment uses oracle-frozen paths and `u`, so **Rust RC and solver differences cannot explain these guard mismatches**.

## 5. Solver comparison and MIMO timings

The sequential eigendecomposition remains unchanged. The alternative 19-Cholesky implementation was not added after the stop condition.

Existing release benchmarks, seconds:

| Case | One thread | Default pool: 4 |
|---|---:|---:|
| Demo design, seven inputs | 3.473190 | 1.744264 |
| Amortized per input | 0.496170 | 0.249181 |
| Existing apply-only demo | 4.331241 | 2.292334 |
| 24 speakers, one input, 4140-dimensional system | 20.881968 | 19.885931 |

Against ADR 0008’s historical ≈6-second ideal-FIR design, Rust is approximately **1.73× faster with one thread** and **3.44× with four**. This is not a fresh matched oracle run. The unfinished 30-minute IIR stage is excluded from speedup claims.

## 6. Refreshed performance table

RC on one speaker, two room positions; median of three:

| Mode | Oracle s | Rust 1 thread s | Faster by | Rust default s | Faster by |
|---|---:|---:|---:|---:|---:|
| mixed | 0.88 | 0.368211 | 2.390× | 0.352445 | 2.497× |
| zero | 1.55 | 0.622969 | 2.488× | 0.632818 | 2.449× |
| linear | 2.22 | 0.643376 | 3.451× | 0.350845 | 6.328× |
| long | 0.73 | 0.296264 | 2.464× | 0.296744 | 2.460× |
| ultra-60 | 1.91 | 0.585058 | 3.265× | 0.430246 | 4.439× |

Peak RSS, one/default:
- RC process: **177,140 / 227,096 KiB**
- Combined MIMO benchmark: **1,048,388 / 1,157,152 KiB**

The faer dirty-AVX-state policy check passes. The report documents the prior `std` → `private-gemm-x86`, 17 → 190 ns/point finding. A before/after-MIMO scalar probe was **not** added.

## 7. Verification results

All requested commands completed in the foreground.

- `cargo fmt --all -- --check`: PASS
- DSP clippy: PASS
- `git diff --check`: PASS
- Policy: **8 passed, 1 failed**—three existing A08 test roots lack `#![forbid(unsafe_code)]`.
- Complete DSP suite: two failed targets, `golden_secs2_mimo` and `room_tuning2_apply`; other targets passed, including A07d fits.
- Required combined targeted command stopped at the failed golden target; I then ran application tests separately.
- Required benchmark: completed successfully.

Relevant result tails:

```text
policy:
test result: FAILED. 8 passed; 1 failed

golden_secs2_mimo:
test result: FAILED. 1 passed; 2 failed
    golden_secs2_mimo
    mimo_guard_hypotheses

room_tuning2_apply:
test result: FAILED. 8 passed; 1 failed
    room_tuning2_matches_secs2_mimo

room_tuning2_mimo:
test result: ok. 2 passed; 0 failed
```

## 8. Final status

```text
 M crates/impulcifer-dsp/tests/golden_secs2_mimo.rs
 M docs/rust/perf/room-tuning2.md
 M tests/migration/README-secs2.md
```

No commits, branch switches, stashes, delegation, or forbidden-source access.

## 9. Undone

Per the no-matching-hypothesis stop condition: path/final gates, named end-to-end bounds, full-convolution band-limit coverage, zero-mode latency coverage, populated geometry coverage, observer coverage, diagnostic input checks, RC attribution, solver comparison, whole prepare+apply timings, and post-MIMO scalar probe remain unfinished.

**A08 is still blocked. The evidence does not justify replacing H1 with H2.**
