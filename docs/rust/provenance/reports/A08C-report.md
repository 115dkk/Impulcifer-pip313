## Guard decision: retain H1

No tested variant reproduces all 85 values.

| Variant | Within 1e-9 | Worst absolute difference |
|---|---:|---:|
| H1 | 83/85 | 0.141260003354 |
| H1a: x onset | 83/85 | 0.141260003354 |
| H1b: lower edge | 78/85 | 0.587071129138 |
| H1b: lower edge × 2^(1/6) | 61/85 | 1.000078546192 |
| H1c: after five | 84/85 | 0.295957993860 |
| H1c: union six | 80/85 | 0.703463765237 |

H1d sixth/seventh relative height margins:
- **BL:** 0.0190417840267
- **FC:** 0.155149657871

Neither is a tie. Only the two authorized trial-0 tail anomalies are exempted; all other guard tolerances remain unchanged.

## Implemented and verified

- Final-support and level gates, including BL’s explicitly named −3 dB consequence.
- Reconstructed `g` and low-pass gates.
- Named provisional end-to-end bounds.
- Full-convolution sum-identity and band-limit test.
- Zero-mode latency, populated/constructed geometry, and pipeline observer tests.
- Invalid-input checks on all four public diagnostic functions.
- `forbid(unsafe_code)` on the three test roots.
- RC-boundary substitution experiment.
- Sequential solver comparison, scalar probes, whole prepare+apply benchmarks, and refreshed report.

Key measurements:

| Gate | Maximum |
|---|---:|
| Reconstructed g/lp absolute error | 5.55e-17 |
| Normal-equation relative L2 | 1.39e-15 |
| Guard absolute error, remaining 83 values | 1.85e-12 |
| Final-support/level absolute error | 4.10e-13 |
| Full-convolution sum relative error | 2.21e-14 |
| Support relative error after frozen RC-boundary substitution | 5.56e-13 |

## Two gates remain failing

1. **End-to-end changed-step count**

   | Setting | Rust | Maximum allowed |
   |---|---:|---:|
   | linear/150/auto | 1 | 2 |
   | mixed/100/low40 | 2 | 3 |
   | mixed/150/auto | **4** | **2** |

2. **Full-convolution band limit:** all 14 ears fail −80 dB, ranging from **−78.46 to −71.26 dB**. The sum identity passes. Endpoint fading passed experimentally, but I reverted it because it changes the specified support formula.

Matching full MIMO RC spectra are not exported. I substituted their frozen `x` and direct-energy outputs instead; this isolates the mixed-setting differences upstream of the MIMO solver. A literal full-spectrum substitution and complete frozen-input `x`/energy reconstruction gate remain undone. RC implementation was not modified.

## Performance

Retained EVD: median observed **17.85 s**, versus **19.13 s** for 19 Cholesky solves. Every comparison had identical decisions and kept solve. One noisy run favored Cholesky; both default-pool runs favored EVD.

| Operation | One thread | Default pool |
|---|---:|---:|
| Demo MIMO design | 3.044 s | 0.911 s |
| Whole prepare+apply, MIMO off | 5.072 s | 1.528 s |
| Whole prepare+apply, MIMO on | 8.661 s | 2.859 s |
| 24 speakers, one input | 19.801 s | 18.361 s |

No dirty-AVX scalar-loop stall was observed. All five RC modes beat the supplied oracle timings.

## Verification

All commands completed in the foreground.

- Formatting, strict clippy, policy: **PASS**
- Policy: **9 passed**
- MIMO golden tests: **3 passed**
- Application tests: **11 passed, 2 failed**
- Full DSP suite: only those two application tests fail
- Benchmarks: completed for both thread settings

Report with tables, bounds, attribution, and command excerpts:  
`docs/rust/perf/room-tuning2.md`

The report attachment failed with server HTTP 400; the repository file is saved.

Final porcelain:
```text
 M crates/impulcifer-dsp/src/stages/room_tuning2/apply.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/mimo.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/mod.rs
 M crates/impulcifer-dsp/tests/golden_secs2_mimo.rs
 M crates/impulcifer-dsp/tests/room_tuning2_apply.rs
 M docs/rust/perf/room-tuning2.md
 M tests/migration/README-secs2.md
```

I made **no commits**. A concurrent external WIP commit captured earlier edits during execution, so porcelain shows only changes after that snapshot. **A08c is not fully accepted while the two gates and missing path-reconstruction coverage remain open.**
