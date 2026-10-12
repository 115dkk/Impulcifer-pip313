# A08b (ASTRA): virtual room tuning 2 — settle the MIMO guard from the oracle, finish A08's gates

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This continues A08, which you implemented (now committed on this branch on top of A07d). All earlier rules apply: clean room (no SECS source or binary, nothing under `/tmp/claude-0/`, do not open or run `tests/migration/export_goldens_secs2.py`); numpy/scipy sources are fine; faer sequential and without its `std` feature. Every command in the foreground. Do not delegate to another agent.

Another worker is wiring config, CLI and service (A09) in `/home/user/wt-a09` at the same time. Allowed files here: `crates/impulcifer-dsp/src/stages/room_tuning2/{mimo.rs,apply.rs}`, `room_tuning2/mod.rs` (re-exports only; do not touch `Tuning2Mode`, A09 may move it), `crates/impulcifer-dsp/src/{pipeline.rs,stages/room.rs,stages/readme.rs}` (fixes to your own A08 code only), the tests `crates/impulcifer-dsp/tests/{golden_secs2_mimo.rs,room_tuning2_apply.rs,room_tuning2_mimo.rs}`, your MIMO functions in `benches/perf.rs`, `docs/rust/perf/room-tuning2.md`, and the MIMO rows of `tests/migration/README-secs2.md`. Nothing in `impulcifer-types`, `impulcifer-cli`, `impulcifer-service`, `features.toml`, `optimize.rs`, `peq.rs`, `rc.rs`.

Use `export CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false`. If the disk fills, delete `target/debug` and rerun.

## 1. The tail-change mode set: let the oracle decide

Your A08 finding: with the specification's rule (modes from `LP({x_e}, 6)` **and** `LP({y_e}, 6)`), mixed/100 BL's first tail change is 1.006313368383 dB where the oracle has 0.906165519807 dB. The oracle's value equals the maximum over the before-only peaks. FC's first trial also differs (4.010703 vs 3.068970 dB). The specification may be wrong on this line; the oracle's numbers are the authority. Test every rule below against **every** guard value in every `mimo_<setting>_<input>` fixture (all settings, inputs and tried scales: band change and tail change):

- H1: modes from `LP({x_e}, 6) ∪ LP({y_e}, 6)` (the current text);
- H2: modes from `LP({x_e}, 6)` only (before);
- H3: modes from `LP({y_e}, 6)` only (after).

Print a table: rule, values compared, values within 1e-9 absolute, worst difference and where. If exactly one rule reproduces all values, implement that rule, keep the 1e-9 gate unchanged, and quote the table in the report; I will amend the specification. If none does, implement nothing new, and report the remaining mismatches with their inputs, the peaks each rule produced, and the per-mode `a`, `b` values.

Then re-run the MIMO end to end (`room_tuning2_matches_secs2_mimo`): the guard-step difference count per setting must not exceed the worst oracle variant's. Report it per setting.

## 2. Finish A08's open items (your report, section 8)

- **Dependency order.** Gate the path stage (`x`, `g` at 600 Hz per ear, the low-pass, the RC energy) from the fixtures' frozen inputs before the normal equations, and the final supports and level after the guard. MEASURE lines as for RC.
- **Bounds.** Named constants for the end-to-end support relative L2, max abs, level difference and late metrics: measured × 1.5, `// provisional: set by the owner after review`.
- **`room_tuning2_supports_are_band_limited`** as specified: the tuned BRIR minus the RC-only result has no energy above 300 Hz beyond −80 dB of its in-band level, and equals `Σ_j BRIR_j ⊛ q_ij` (compare full convolutions, 1e-9 relative).
- **Zero mode in the application.** `room_tuning2_latency` must cover `zero`: no delay, unchanged lengths.
- **Geometry.** A positive test with pairs populated from the demo's per-recording room files (no mismatch on the demo), and a constructed mismatch that is reported.
- **Observer.** A pipeline test that `on_room_tuned2` fires once with the report when `room_mode == "tuning2"` and never otherwise.
- **Input checks.** Public diagnostic functions (`design_paths`, `normal_equations`, `bisect`, `support_response`) return `DspError::InvalidArgument` on inconsistent lengths instead of panicking; one test.
- **RC attribution (report only).** Rerun the MIMO reference with the RC responses replaced by the oracle's own (from `room_tuning_secs2_mimo.json`/fixtures where present). How much of the support difference remains?

## 3. MIMO speed

Your A08 timings: demo design 1.08 s for seven inputs, the synthetic 24-speaker case 23.5 s for one input. Compare head to head, on the 24-speaker normal matrix, one symmetric eigendecomposition reused over the 19 regularizations against 19 sequential Cholesky solves, and keep the faster one. The decisions must stay identical to the dense cross-check and the gates must still pass. Inputs already run in parallel (rayon `map`). Report one thread and the default pool. Also check that nothing in MIMO triggers the dirty-AVX-state stall: time a fixed scalar loop before and after `design_mimo`.

## 4. Performance report

Refresh `docs/rust/perf/room-tuning2.md` on the current branch, same container:
- RC per mode for one speaker, with one thread and with the default pool. The oracle's times are mixed 0.88, zero 1.55, linear 2.22, long 0.73, ultra-60 1.91 s.
- MIMO demo design total and per input, and the 24-speaker case.
- The whole `prepare_tuning2` plus `apply_tuning2` on the demo, MIMO off and on.
- Peak RSS.
- A short section on the AVX-state finding (faer `std` → `private-gemm-x86`, the 17 → 190 ns/point loop, the policy gate).

State plainly where Rust is faster and by how much. The SECS 2 MIMO design reference is the one recorded in ADR 0008. Its IIR stage, which Impulcifer does not use, had not finished after 30 minutes on the 7-speaker demo.

## Verification (foreground; paste tails and MEASURE lines)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --no-fail-fast
cargo test -p impulcifer-dsp --test golden_secs2_mimo --test room_tuning2_apply -- --nocapture --test-threads=1
cargo bench -p impulcifer-dsp --bench perf -- room_tuning2
git status --porcelain
```

## Report format

(1) the hypothesis table and the rule kept; (2) MIMO gates in dependency order with MEASURE maxima; (3) the end-to-end table per setting vs every oracle variant, with bounds; (4) the RC attribution; (5) solver comparison and MIMO timings; (6) the refreshed performance table; (7) `test result:` lines; (8) `git status --porcelain`; (9) anything undone. Do not end your turn before the commands complete.
