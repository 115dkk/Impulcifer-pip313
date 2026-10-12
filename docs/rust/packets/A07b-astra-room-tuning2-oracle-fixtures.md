# A07b (ASTRA): virtual room tuning 2 — gate the port fixture by fixture

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This continues A07 (`docs/rust/packets/A07-astra-room-tuning2-rc.md`), which you implemented; its rules, its clean-room rule (no SECS source, nothing under `/tmp/claude-0/`; numpy/scipy sources are fine; **do not open or run `tests/migration/export_goldens_secs2.py`**) and its allowed files apply, plus new test files `crates/impulcifer-dsp/tests/golden_secs2_*.rs` and `tests/migration/README-secs2.md` (only to record measured budgets). Every command in the foreground.

## Why

A07 compared only end results (`room_tuning_secs2_rc.json`): when a band differed, the cause had to be hunted from the outside. This repository ports by oracle fixtures instead — the P03…P12 families: every function and stage boundary frozen as `inputs`/`outputs`/`meta`, gated one fixture at a time with `MEASURE` lines (see `tests/migration/README.md` and `crates/impulcifer-dsp/tests/golden_batch1.rs` for the pattern and the helpers). The oracle of tuning 2 is now exported the same way: `tests/migration/goldens/s2_*` (544 files), documented in **`tests/migration/README-secs2.md`** — read it completely, it is the contract for this packet.

## Task

1. Add `crates/impulcifer-dsp/tests/golden_secs2_primitives.rs`, `golden_secs2_fit.rs`, `golden_secs2_rc.rs` (split further if you like) that load the `s2_*` fixtures (sidecars: read every value, check `length` and `sha256` of the bytes you read, never only first/last) and compare the corresponding Rust functions family by family, printing `MEASURE <family>: max_abs=…, max_relative_l2=…, values=…` like `golden_batch1.rs`. Expose what the tests need from `room_tuning2` (`pub` helpers, or an optional stage capture like v1's `TuningStages`) without changing what the production path computes.
2. Make every family pass, **in dependency order**: primitives → bounds → fit traces → RC stages → RC results. For the fits, compare `main_x` evaluation by evaluation (and `nfev`, `njev`, `status`, `x`, `cost`, `J0` for the synthetic fits): the first index where your iterate leaves the oracle's is the bug to fix, not a budget to widen. Typical causes to check against the numpy/scipy sources: the two-point step and its bound-aware one-sided switch, `x_scale`/`CL_scaling_vector`, the reflective step and the step-size heuristics in `trf_bounds`, `update_tr_radius`, the `exact` solver's SVD use, numpy's complex division and `log10`, and the order the residual multiplies the bands.
3. RC stages (`rc_<case>_s<k>`): compare power average, both smoothings, design curve, long-mode bins, M, the phasor average and its smoothing, raw and smoothed delay, every FIR realization in order (its count tells which strengths were tried), the touch-up minimum phases, the final FIR and response; then `rc_<case>` (band, level, strength, resolution, PEQ, final response). The linear and ultra cases have two sessions (β = 24 then 6); the chosen one is `phase_frac_used`. `levels_w1`/`levels_w5` gate the channel level.
4. Record the measured maxima in a table in `tests/migration/README-secs2.md` ("Gates": add an "Observed maximum" column) and set the test budgets from them (measured × 1.5, rounded up, floors as the README says). Keep `room_tuning2_matches_secs2_rc` (end to end) and tighten its bounds to what the fixtures now allow.

Do not change the specification's behaviour to make a fixture pass; if a fixture contradicts `docs/rust/ROOM_CORRECTION.md`, report the line and the fixture.

## Verification (foreground; paste tails)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --no-fail-fast
cargo test -p impulcifer-dsp --test golden_secs2_fit -- --nocapture --test-threads=1   # and the other golden_secs2_* targets; paste the MEASURE lines
git status --porcelain
```

## Report format

(1) the fixture families and their `MEASURE` maxima vs budgets; (2) every divergence found, at which fixture/stage/evaluation index, and its cause and fix; (3) the end-to-end table after the fixes; (4) anything in the README contract that is ambiguous (quote it); (5) `test result:` lines; (6) `git status --porcelain`. Do not end your turn before the commands complete.
