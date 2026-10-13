# A08c (ASTRA): virtual room tuning 2 — the two guard values H1 misses, then A08b's list

Same place and rules as A08b (`docs/rust/packets/A08b-astra-room-tuning2-mimo-guard-and-gates.md`): work in `/home/user/Impulcifer-pip313` on the checked-out branch, no commits, the same allowed files, clean room, foreground commands, no delegation. A09 is still running in `/home/user/wt-a09`.

## What your table shows

H1 (the specification) reproduces 83 of 85 guard values. Both misses are mixed/100/low40, trial 0, tail, and both are caused by **the sixth (last) after-peak, near 44 Hz**:

- BL: Rust's maximum 1.0063 dB comes from mode 44.8. The oracle's 0.9062 dB is mode 76.2's ratio, the largest once 44.8 is gone.
- FC: Rust's 4.0107 dB comes from mode 44.2. The oracle's 3.0690 dB is mode 50.7's ratio, the largest once 44.2 is gone.

So the oracle uses H1 but without those two peaks. That points to a detail of `LP` on the after signal. It is not the before/after choice. The MIMO lower edge in this setting is 40 Hz, and 40·2^(1/6) = 44.9 Hz lies just above both dropped peaks.

## 1. Test these variants of H1 on all 85 values

Change one thing at a time. Each variant keeps H1's union:

- **H1a:** `LP` of `y_e` uses the onset `k_e` found on `x_e` (the tail sums already use x's `k_e`), not y's own.
- **H1b:** the peak mask's lower bound follows the support range: `max(20, f_low)`, and separately `max(20, f_low·2^(1/6))`, applied to both sets (f_low = the input's support lower edge; for auto settings use the automatic value your code computes).
- **H1c:** the after set keeps fewer peaks (5), or the union is truncated to the 6 highest by before-height.
- **H1d (tie check):** print the after spectrum's top 10 peaks with heights for the two inputs. If the 6th and 7th heights differ by less than 1e-9 relative, the miss is a ranking tie and none of the above is needed. Report the margin.

Print the same table as before for every variant. If exactly one variant reproduces all 85 values within 1e-9, implement it and quote the table; I will amend the specification. If several do, prefer the one that changes the fewest lines and report all of them. If none does, keep H1, mark the two values as known oracle anomalies in the gate with their exact names (no tolerance change for anything else), report the H1d margins, and continue.

## 2. Then finish A08b's list

Do everything A08b's sections 2–4 ask that you stopped before: path and final-support gates in dependency order, named provisional end-to-end bounds, the full-convolution band-limit test with the sum identity, zero-mode latency, populated and constructed geometry tests, the observer test, input checks on the public diagnostic functions, the RC attribution, the eigendecomposition vs 19-Cholesky comparison on the 24-speaker matrix (keep the faster, decisions identical), the scalar-loop probe around `design_mimo`, the whole prepare+apply timing on the demo (MIMO off and on), and the refreshed `docs/rust/perf/room-tuning2.md`. Re-run the end-to-end count gate per setting after the guard decision.

Also add `#![forbid(unsafe_code)]` to the three test roots the policy gate flagged.

## Verification and report

The same as A08b. Add the variant table at the top of the report. Do not end your turn before the commands complete.
