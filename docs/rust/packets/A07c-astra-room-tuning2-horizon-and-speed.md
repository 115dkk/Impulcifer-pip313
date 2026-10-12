# A07c (ASTRA): virtual room tuning 2 — gate fits at the oracle's own horizon, then make it fast

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This continues A07 and A07b, which you implemented; all their rules apply (clean room: no SECS source, nothing under `/tmp/claude-0/`, do not open or run `tests/migration/export_goldens_secs2.py`; numpy/scipy sources are fine; `faer` sequential). Every command in the foreground. If the full build runs out of disk, delete `target/debug` (not `target/release`) and use the test-profile overrides you used in A07b.

## What the oracle says about itself (new data)

Your A07b finding — SciPy alone moves its PEQ trace by 3e-7 at evaluation 6 when only the LAPACK SVD driver changes — is now part of every fit fixture and of a new reference:

- Every `s2_fit_*.json` with a trace has `outputs.oracle_variants`: the oracle's own trace for the **same inputs** as other computers run it (NumPy AVX-512; OpenBLAS Haswell, Sandybridge, Nehalem kernels; LAPACK `gesvd`, in process and in a fresh process), each with `first_index_over_1e-9` and `rel_distance` per evaluation (max abs parameter difference / max abs reference parameter), and `outputs.horizon`: the first evaluation where a variant that **keeps the residual's arithmetic** (a different factorization or BLAS kernel) leaves the reference by more than 1e-9. Before it those variants agree to ~1e-16; at it they jump to 1e-8–1e-4; on the demo it is evaluation 1–12. The AVX-512 variant changes the residual itself (`log10`), so it leaves at the first step; your residual is the C library's (your `J0` is exact), so it is not part of the horizon. The horizon is how far the path is fixed by the algorithm rather than by the last bit of a factorization or a BLAS reduction — which is exactly where your `faer` SVD and sequential sums differ from LAPACK/OpenBLAS.
- `tests/migration/goldens/room_tuning_secs2_variance.json`: the end-to-end references recomputed by the same oracle as five other computers would run it (NumPy AVX-512 kernels; OpenBLAS Haswell, Sandybridge, Nehalem kernels; LAPACK `gesvd`), compared with the references per band and channel, with a per-variant `summary`, the `rc_envelope` (largest difference over the variants per channel and band) and its summary, and the MIMO equivalents. Read its `provenance` and `summary` first.

## 1. Fit traces: gate up to the horizon

In `golden_secs2_fit.rs` replace the whole-trace gate by: for each fit fixture, your `main_x` must agree with the oracle's to 1e-9 relative (same definition as `rel_distance`: max abs difference over the parameters / max abs reference parameter) at **every evaluation before `horizon − 1`** (one evaluation of slack: the variants' own first indices for one fit spread over several evaluations, e.g. 3–8, so their minimum over five computers is itself uncertain by about one), with the same evaluation count up to that point. Beyond the horizon print, per fixture, your first index over 1e-9 next to the horizon and your final cost next to the reference and variant costs; report-only. A fixture where you diverge **before** the horizon is a real difference: find it (A07b's list: `ripple` at evaluation 1 is the first candidate — compare against the horizon) and fix it. `J0` stays exact. Do the same for `trf_scipy_reference.json`'s PEQ16 problem (add a gesvd run of the same problem to `gen_trf_reference.py` with the same environment and gate it the same way; the smooth and active-bound problems stay as they are).

## 2. RC stages: finish the gates A07b did not

Gate the `rc_<case>_s<k>` stage spectra (power average, both smoothings, design curve, long-mode bins, M, phasor average and its smoothing, raw and smoothed delay, every FIR realization in order, touch-up minimum phases, final FIR and response) and the `rc_<case>` results, fed from your own chain. Where the case's first fit diverges before the end (it almost always does: see its `oracle_gesvd`), the stages after the first fit are compared report-only; the stages up to and including the first fit's **inputs** (power, smoothings, design curve, long-mode bins, and `fit_*_0`'s `corr_db`) must agree to floating-point noise. Print a table: case, first divergent stage, its size, the fit horizon.

## 3. End to end: against the envelope

`room_tuning2_matches_secs2_rc`: you are one more computer. Your counts of channels above 0.1 dB and 1 dB must not exceed the **envelope's** (`summary.envelope.channels_over` in `room_tuning_secs2_variance.json`, read from the file), and your per-band maximum must not exceed the envelope's `max_mag_db` × 1.25 rounded up to 0.01 dB (five computers sample SECS's spread; they do not bound it). Print your distribution next to every variant's and the envelope's. Phase strength changes and PEQ band count changes: report next to the envelope's counts.

## 4. Speed

Your A07b timings (FL, two positions): mixed 1.18 s, zero 2.44 s, linear 2.84 s, long 1.13 s, ultra-60 2.53 s — slower than the oracle (0.88 / 1.55 / 2.22 / 0.73 / 1.91 s). Profile one speaker per mode (timers around every chain step: spectra, smoothing, long-mode analysis, each Fit (residual evaluations, Jacobian, SVD, the rest of the TRF iteration), minimum phases, realizations, touch-up), print the profile, and remove the waste without changing any result the gates check: reuse FFT plans and buffers (`realfft` planners per size), avoid per-evaluation allocations in the residual and Jacobian, evaluate only the band whose parameter moved for a Jacobian column while still multiplying the 16 cached responses in band order, precompute `z` and its powers per grid, compute `2^(0.5/b)` once per grid and resolution, keep the SVD on the small augmented matrix (consider a thin QR of `J` followed by the SVD of the 48×48 factor only if the fit gates up to the horizon still pass). Target: every mode at most a tenth of the oracle's time for one speaker. Report before/after per step.

## Verification (foreground; paste tails and the MEASURE lines)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --no-fail-fast
cargo test -p impulcifer-dsp --test golden_secs2_fit -- --nocapture --test-threads=1
cargo test -p impulcifer-dsp --test golden_secs2_rc -- --nocapture --test-threads=1
cargo test -p impulcifer-dsp --test room_tuning2_rc -- --nocapture --test-threads=1
cargo bench -p impulcifer-dsp --bench perf -- room_tuning2
git status --porcelain
```

## Report format

(1) fit fixtures: your first index over 1e-9 vs the oracle's horizon, per fixture, and every pre-horizon divergence found and fixed; (2) the RC stage table; (3) your end-to-end distribution vs every variant and the envelope; (4) the profile before/after and the per-mode timings vs the oracle; (5) `test result:` lines; (6) `git status --porcelain`; (7) anything undone. Do not end your turn before the commands complete.
