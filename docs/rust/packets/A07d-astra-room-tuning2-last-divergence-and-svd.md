# A07d (ASTRA): virtual room tuning 2 — close the last fit divergence, then the SVD

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This continues A07, A07b and A07c, which you implemented; all their rules apply (clean room: no SECS source or binary, nothing under `/tmp/claude-0/`, do not open or run `tests/migration/export_goldens_secs2.py`; numpy/scipy sources are fine; `faer` sequential). Every command in the foreground. Do not delegate to another agent. Another worker is implementing A08 (MIMO, application, pipeline) in a separate worktree at the same time, so stay inside the allowed files.

Allowed files: `crates/impulcifer-dsp/src/optimize.rs`, `crates/impulcifer-dsp/src/stages/room_tuning2/{peq.rs,rc.rs,primitives.rs,profile.rs}`, `crates/impulcifer-dsp/tests/{golden_secs2_fit.rs,golden_secs2_rc.rs,golden_secs2_helpers.rs,room_tuning2_rc.rs,trf.rs}`, the `room_tuning2` function in `crates/impulcifer-dsp/benches/perf.rs`, `tests/migration/gen_trf_reference.py`, a new `tests/migration/replay_trf_fixture.py`, and `tests/migration/README-secs2.md` (observed maxima and a short section on the replay tool).

## What changed since A07c (read first)

Your residual was 6–8× slower than its arithmetic because of the build, not your code. faer's `std` feature pulled in `private-gemm-x86`, whose build-generated assembly kernels use zmm/ymm registers and never execute `vzeroupper`. After the first matrix product, every SSE instruction in the thread carried a false dependency on the dirty upper AVX state. The same `magnitude + log10` loop ran at 17 ns per point before a fit and 190 ns after one. faer now builds without `std` (root `Cargo.toml`); `pulp`, `gemm` and `faer-traits` enable their own `std`, so runtime SIMD detection stays. The policy gate `no_assembly_kernels_that_leave_avx_state_dirty` keeps it that way. Do not add features to faer or depend on any crate that runs hand-written SIMD assembly.

Current numbers (release bench, FL, two positions): mixed 0.46 s, zero 0.97 s, linear 0.90 s, long 0.43 s, ultra-60 1.04 s. The oracle's times are 0.88 / 1.55 / 2.22 / 0.73 / 1.91 s. The residual now costs ~19 µs per evaluation at 700 points (column path), and the SVD ~1.5 ms per outer iteration (748×48, thin U), which is now the largest single cost (mixed: SVD 197 ms of a 322 ms fit total; zero: 534 of 871 ms). End to end is inside the envelope: ours `[57, 37, 26, 10]` channels over 0.001/0.01/0.1/1 dB vs the envelope's `[61, 39, 32, 15]`, band maxima `[1.78, 1.60, 0.33, 0.040]` dB.

## 1. `fit_sr_both_ultra60_s0_1`: a real difference

It leaves the oracle at evaluation 1 (1.2477142515202155e-6), before its horizon 5. The distance is **identical to the last digit under two different SVD builds** (faer with and without its SIMD/asm path), so it is not rounding noise: some decision or formula in the first TRF iteration differs. `J0` and the initial residual are exact, so it lies between them and the first trial point. Find it with a replay, not by inspection:

1. `tests/migration/replay_trf_fixture.py <fixture.json> [--out <json>]`: a numpy residual written **from the specification** (`docs/rust/ROOM_CORRECTION.md`, "PEQ fit": the RBJ peaking product on the fixture's grid `g`, weight 1 inside `[lo, hi]` and 0.15 elsewhere, `20*log10(|H|+1e-12) − corr_db`, parameters `(ln f, ln Q, gain)` per band), then `scipy.optimize.least_squares` with exactly the call the specification names, starting from the fixture's `x0`/`lb`/`ub`/`max_nfev`. Run it with `NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL AVX512_SPR" OPENBLAS_NUM_THREADS=1`. First check the replay against the fixture: `main_cost[0]` exact and `main_x` within 1e-9 before `horizon − 1`. If the replay itself does not reproduce the fixture, your residual or the call differs from the specification. Report that with the first differing value, and do not change the specification.
2. Instrument the replay by wrapping module globals of `scipy/optimize/_lsq/trf.py`, which `trf_bounds` looks up at call time: the helpers it imports (`svd`, `solve_lsq_trust_region`, `make_strictly_feasible`, `step_size_to_bound`, `intersect_trust_region`, `build_quadratic_1d`, `minimize_quadratic_1d`, `CL_scaling_vector`, `evaluate_quadratic`, `update_tr_radius`, `check_termination`) and `select_step`, which the module defines itself. Record per outer iteration: `g_norm`, `v`, `dv`, `Delta`, `s`, `uf`, `alpha` in/out, `n_iter`, `p_h`, `full_rank`, the `select_step` branch taken (p / reflected / anti-gradient) with its three values and strides, the step, `x_new`, `actual_reduction`, `predicted_reduction`, `ratio`, and the new `Delta`. Do the same for every rejected inner step.
3. Record the same quantities in Rust's opt-in `LeastSquaresTrace` (none allocated when the trace is off), run both on `fit_sr_both_ultra60_s0_1`, print the first quantity that differs by more than 1e-12 relative, fix the port, and keep a test (`golden_secs2_fit.rs`) that compares the first iteration's internals against a small JSON the replay writes for this fixture (commit that JSON next to the other goldens as `tests/migration/goldens/trf_replay_sr_both_ultra60_s0_1.json`; keep it under 200 kB).
4. Re-run every fit fixture. All must pass before `horizon − 1`; print first-divergence vs horizon as before.

## 2. The SVD

SciPy's exact solver uses only `s`, `V` and `uf = Uᵀ f_aug` of the thin SVD of `J_aug = [J·d; diag(√diag_h)]` (748×48 or 948×48). Do not form `U`. Reduce first: a sequential Householder QR of `J_aug` (faer, `Par::Seq`), giving `R` (48×48) and `c = (Qᵀ f_aug)[..n]`; then the SVD of `R` gives `s`, `V` and `U_R`, and `uf = U_Rᵀ c`. This is the same mathematics LAPACK uses for tall matrices; only rounding changes, as it does between LAPACK drivers. Measure the per-call time against the current path, keep the faster one only if **every** fit gate still passes up to `horizon − 1` (with step 1's fix in place) and end to end stays inside the envelope, and report the per-call times.

## 3. The residual

It must stay **bit-identical** (`J0` exact is the gate). Inside that rule, try what is free: process the band, chain and magnitude loops through `pulp`'s safe runtime dispatch (`pulp::Arch::new().dispatch(...)`, no `unsafe`) so LLVM can vectorize them with AVX2. IEEE `+ − × ÷ sqrt` and `abs/min/max` give identical results in SIMD lanes, and Rust never contracts into FMA. `log10` stays the C library's, one call per element. Keep a change only if it measures faster and `J0` stays exact on all synthetic fits.

## 4. Concurrency that cannot change results

Linear and ultra modes always run steps 1–8 twice (β = 24 and β = 6) on the same inputs. Run the two sessions with `rayon::join` and keep everything else as is. Do not parallelize inside a fit; speakers will run in parallel in the pipeline (A08). Add a test that the linear-mode result is bit-identical with a 1-thread and a 4-thread rayon pool.

## 5. Timings

In the bench, print the oracle's seconds from constants (mixed 0.88, zero 1.55, linear 2.22, long 0.73, ultra-60 1.91) and the ratio, once with `RAYON_NUM_THREADS=1` and once with the default pool. Target: **every mode at least 4× faster than the oracle with one thread.** If a mode misses it, report the floor: residual evaluations × points × measured `log10` cost, plus the SVD calls × per-call time.

## Verification (foreground; paste tails and the MEASURE/FIT lines)

```
cd /home/user/Impulcifer-pip313
export CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --no-fail-fast
cargo test -p impulcifer-dsp --test golden_secs2_fit -- --nocapture --test-threads=1
cargo test -p impulcifer-dsp --test room_tuning2_rc -- --nocapture --test-threads=1
cargo bench -p impulcifer-dsp --bench perf -- room_tuning2
RAYON_NUM_THREADS=1 cargo bench -p impulcifer-dsp --bench perf -- room_tuning2
git status --porcelain
```

If the disk fills, delete `target/debug` (not `target/release`) and rerun.

## Report format

(1) the replay's agreement with the fixture, the first differing quantity, its cause and the fix; (2) the fit table (first divergence vs horizon, cost vs reference and variants) after the fix; (3) the SVD path chosen and per-call times; (4) residual changes kept or dropped, with times; (5) end-to-end distribution vs the envelope; (6) per-mode timings (one thread and default) vs the oracle, with the per-step profile; (7) `test result:` lines; (8) `git status --porcelain`; (9) anything undone. Do not end your turn before the commands complete.
