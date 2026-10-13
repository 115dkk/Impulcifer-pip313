Implemented and verified. No delegation, branch switch, commit, or stash; no SECS source/binary, exporter, or `/tmp/claude-0/` access. Only the six allowed files were changed.

## 1. `late_peaks`

`primitives.rs:162` now zeros the out-of-band spectrum and searches the **full masked array**, retaining:
- SciPy-style plateau midpoint, rounded down.
- Stable descending-height ordering.
- Existing caller-specific filters, including RC’s `lo ≤ p ≤ min(hi, 160)`.

Added `late_peaks_include_band_edges`, with documented SciPy 1.18.1 expectations:

| Synthetic late spectrum | Expected bin | Frequency |
|---|---:|---:|
| Rising through upper edge | 1092 | 159.9609375 Hz |
| Falling from lower edge | 137 | 20.068359375 Hz |
| Interior maximum | 1024 | 150 Hz |
| Flat in-band plateau | 614 | 89.94140625 Hz |

All four match exactly. Existing 14 peak values remain exact; zero/empty inputs and zero requested count are also checked.

## 2. Guard values

Removed the named exceptions, BL support rescaling and level offset, and superseded hypothesis diagnostic. The production golden test enforces **85 compared guard values**, without exceptions.

| Comparison | Values | Worst absolute difference | Result |
|---|---:|---:|---|
| MIMO guards | 85 | `1.850963826655061e-12` | All within `1e-9` |
| Final supports and level | 15,134 | `4.103384299014579e-13` | Pass, unadjusted expectations |

```text
MEASURE MIMO guard: max_abs=1.85096382665506098e-12, max_relative_l2=2.03032770368362084e-12, values=85
MEASURE MIMO final supports and level: max_abs=4.10338429901457857e-13, max_relative_l2=7.73696441388641681e-13, values=15134
```

BL low40 now selects the oracle’s **0 dB** scale.

## 3. End-to-end guard-step counts

Counts are changed input decisions, not summed 3 dB increments.

| Setting | Rust | NumPy AVX-512 | Haswell | Sandybridge | Nehalem | gesvd | Worst variant |
|---|---:|---:|---:|---:|---:|---:|---:|
| linear/150/auto | 1 | 1 | 2 | 1 | 1 | 1 | **2** |
| mixed/100/low40 | 2 | 1 | 1 | 1 | 1 | 3 | **3** |
| mixed/150/auto | 4 | 1 | 1 | 1 | 1 | 2 | **2** |

**`GUARD_STEP_ALLOWANCE = 2` is the smallest passing value.** The zero-allowance probe failed with:

```text
mixed/fsiso150/auto: 4>2+0
```

The deficit of two also excludes allowance 1. Normal and foreign-libm runs pass with 2. The comment now distinguishes this end-to-end allowance from the exception-free frozen-RC guard gate.

## 4. RC gates

- Long-mode bins: **exact across all 11 sessions, 89 values total**.
- End-to-end distribution unchanged and below the envelope.
- Strength changes: **2**; PEQ count changes: **0**.
- Post-fit stage comparisons retain their existing report-only status.

```text
MEASURE RC distribution: thresholds=[0.001, 0.01, 0.1, 1.0] ours=[56, 36, 19, 7] envelope=[61, 39, 32, 15] max_mag_db=[1.9942350486803289, 1.919082891777603, 0.7653187289206791, 0.057842252870441715] strength_changes=2 peq_count_changes=0
```

Magnitude maxima remain below caps `2.91 / 3.33 / 0.93 / 0.08 dB`.

## 5. Verification results

All commands completed in the foreground with the requested profile variables.

- Formatting, Clippy, policy and `git diff --check`: **pass**.
- Full DSP suite: **all targets pass**.
- Foreign-libm MIMO/primitives/application run: **all pass**.
- Service `room_tuning2`: **pass**.

Relevant `test result:` lines:

```text
# policy gates
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s

# full DSP: golden_secs2_mimo
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.47s
# full DSP: golden_secs2_primitives
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
# full DSP: golden_secs2_rc
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.96s
# full DSP: room_tuning2_apply
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.09s
# full DSP: room_tuning2_rc
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 63.59s

# foreign libm: MIMO / primitives / application
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.45s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.58s

# service room_tuning2
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 48.18s
```

Updated the MIMO README rows and performance report to mark the guard issue **resolved: band-edge peaks**. Did not edit `ROOM_CORRECTION.md`.

## 6. Final `git status --porcelain`

```text
 M crates/impulcifer-dsp/src/stages/room_tuning2/primitives.rs
 M crates/impulcifer-dsp/tests/golden_secs2_mimo.rs
 M crates/impulcifer-dsp/tests/golden_secs2_primitives.rs
 M crates/impulcifer-dsp/tests/room_tuning2_apply.rs
 M docs/rust/perf/room-tuning2.md
 M tests/migration/README-secs2.md
```
