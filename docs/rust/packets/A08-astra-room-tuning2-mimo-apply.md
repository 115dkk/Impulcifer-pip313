# A08 (ASTRA): virtual room tuning v2 — MIMO supports, applying to the BRIR, pipeline hook

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This continues A07 (`docs/rust/packets/A07-astra-room-tuning2-rc.md`), which you implemented: its rules, its clean-room rule (no SECS source, nothing under `/tmp/claude-0/`; numpy/scipy sources are fine) and its sequential-`faer` rule apply. The specification is `docs/rust/ROOM_CORRECTION.md`, section **"Virtual room tuning v2"**, subsections "MIMO supports", "Applying to the BRIR", "Diagnostics", and ADR 0008. Every command in the foreground.

Allowed files: `crates/impulcifer-dsp/src/stages/room_tuning2/**`, `crates/impulcifer-dsp/src/stages/room.rs`, `room_v2.rs` (only the new tuning-2 entry point and `RoomCorrection`'s new field), `crates/impulcifer-dsp/src/stages/room_tuning.rs` (only to make existing diagnostics reusable — v1 output must stay bit-identical), `crates/impulcifer-dsp/src/stages/readme.rs` (new data field), `crates/impulcifer-dsp/src/pipeline.rs`, `crates/impulcifer-dsp/benches/perf.rs` and its smoke test, new tests `crates/impulcifer-dsp/tests/room_tuning2_*.rs`, and in `impulcifer-service` only what is needed to keep it compiling (e.g. `tuning2: None` in struct literals). Everything else is a later packet: no config fields, no CLI, no service behaviour, no catalogue, no `features.toml`, no docs.

## MIMO design

```rust
pub struct MimoOptions { pub fsiso: f64, pub low: Option<f64> }            // 50..=150, None = automatic
pub struct MimoInput<'a> { pub speaker: &'a str, pub gain_db: f64, pub response: &'a [Complex64] } // F_total on the N grid
pub struct MimoSupport { pub speaker: String, pub low: f64, pub fir: Vec<f64> } // 210 taps at 600 Hz, after the guard
pub struct MimoInputResult {
    pub speaker: String,
    pub candidates: Vec<String>,
    pub supports: Vec<MimoSupport>,      // empty when the guard turned the input off or there was no candidate
    pub level_db: f64,                   // 10*log10(max(ratio*scale², 1e-30))
    pub scale_db: Option<f64>,           // 0, −3, −6, −9, −12; None = off
    pub band_change_db: f64,             // guard measures at the accepted scale (−99 when none)
    pub tail_change_db: f64,
}
pub struct MimoDesign { pub ranges: Vec<(String, Option<(f64, f64)>)>, pub inputs: Vec<MimoInputResult> }
pub fn design_mimo(brirs: &Hrir, inputs: &[MimoInput], opts: &MimoOptions) -> Result<MimoDesign, DspError>;
pub fn support_response(support: &MimoSupport, fs: u32, n: usize) -> Vec<Complex64>; // q_ij on the analysis grid
```

`brirs` are the aligned, untuned BRIRs (both ears of every speaker); candidates are every speaker with both ears, in HRIR order; inputs are processed independently (parallel over inputs with rayon `map`; inside an input keep the specified accumulation order).

**Solve fast, solve the same system.** The reference builds the stacked n×180R convolution matrices and forms `AᵀA` densely, then runs 19 dense solves for the bisection. Build the normal matrix and `Aᵀb` directly from the block structure instead (blocks `C(c_j)ᵀ diag(d) C(c_k)` with `d` = 1 in the late window and 10 in the early one: each block is a sum over a step-weighted, truncated range, computable along diagonals in O(180²) — accumulate from the short end so every entry is a short sum, not a difference of long ones). Factor with `faer` sequentially: per λ a Cholesky, or one symmetric eigendecomposition reused for all λ — pick by measurement and keep the bisection decisions identical. Add a test `room_tuning2_mimo_fast_normal_equations` that, on one demo input, compares your normal matrix and `Aᵀb` against a naive dense construction (relative 1e-12) and checks the 19 bisection decisions and the final u (relative 1e-9) against a naive dense solve.

## Reference test against SECS 2

`tests/migration/goldens/room_tuning_secs2_mimo.json` (read `provenance`, `ir`, `fields`): three settings on the aligned demo BRIRs. Build the BRIRs as `ir.construction` says. For each entry: run your RC on the BRIRs themselves (positions: left ear, right ear; `PositionPolicy::Listening { index: 0, weight: 1.0 }`, mode `rc_mode`, flat target, no calibration, automatic band, max boost 10), gains `min_j L_j − L_i` over the seven speakers (compare with `gain_db`), then `design_mimo` with `fsiso` and `f_low`. Compare `ranges` (exact after Python-style rounding to 0.1 Hz), which inputs are off, `level_db` (report the difference; the guard scale must match), each support FIR (relative L2 error and max abs error), and `late` (`worst_band_db` = your band change; `tail_change` = SECS's tail value multiplied by 0.15, i.e. compare it with `0.15*tail_change_db`). The RC feeding MIMO differs slightly from SECS's (A07's measured agreement), so report how much of the support difference that explains (e.g. rerun with the RC response perturbed by A07's error bound). Set bounds as named constants (measured × 1.5, `// provisional: set by the owner after review`).

## Plan and application

```rust
pub struct Tuning2Options { pub rc: RcOptions /* policy Symmetric */, pub level_match: bool, pub mimo: Option<MimoOptions>, pub pairs: Vec<room_tuning::PairMeasurement> }
pub struct Tuning2Plan { /* fs, band, per measured speaker: name, omni points (Side, IR), SpeakerRc; options */ }
pub fn prepare_tuning2(omni: &Hrir, options: &Tuning2Options, calibration: Option<&FrequencyResponse>, target: Option<&FrequencyResponse>) -> Result<Tuning2Plan, DspError>;
pub fn apply_tuning2(hrir: &mut Hrir, plan: &Tuning2Plan) -> Result<Tuning2Report, DspError>;
pub struct Tuning2Report { /* fs, mode, latency_samples, band, speakers: Vec<Speaker2Report>, mimo: Option<MimoDesign>, pair_checks */ }
```

- `prepare_tuning2` takes the room stage's **uncropped** split omni IRs (the room stage head-crops for EQ and v1; clone before that, or build from the uncropped split — the spec and the goldens use uncropped IRs), computes the band once over all measured speakers, then every speaker's RC in parallel.
- `apply_tuning2`: channel levels and the 2 dB consistency check against the aligned in-ear BRIR levels (reuse v1's in-ear level and rule), trims `min − L` over the consistent speakers when `level_match`, then MIMO (if enabled) on the untuned aligned BRIRs, then the application exactly as "Applying to the BRIR" says (all speakers computed from the untuned copies; tuned length = old length + D_s; last 5 ms falling half-Hann as v1; unmeasured speakers shifted by D_s; no measured speaker → unchanged, no delay).
- Report per speaker: points, level, trim (dB) and source (`omni` / `skipped` / `off`), level difference, PEQ band count, phase strength and resolution, long modes, pre-echo per ear (v1's direct-sound rule, untuned vs tuned), representativeness RMS and weak flag (v1's rule), LF EDT and median excess GD before/after (v1's functions), geometry mismatch from the pairs (v1's check). Make the v1 helpers you need public without changing what they compute.
- Pipeline: `RoomCorrection` gets `tuning2: Option<Tuning2Plan>` (None everywhere it is built today); the room stage gets an entry point `room_correction_tuning2(rir, target, mic, options: &Tuning2Options)` returning an empty `RoomFrs` (term `Gain`, no entries) and the plan. In `CropAndAlign`, after `crop_tails`, when `do_room_correction && room_mode == "tuning2"` and a plan exists, call `apply_tuning2` and a new `StageObserver::on_room_tuned2(&Tuning2Report)` (default `Ok(())`). The README data gets `tuning2: Option<Tuning2Report>` filled the way `tuning` is. Wherever the pipeline treats `room_mode == "tuning"` as "a tuning mode" (the virtual-bass room-gain filter), `tuning2` must behave the same (room gains only in `eq`).

## Tests (in `crates/impulcifer-dsp/tests/room_tuning2_apply.rs` unless said otherwise; names fixed)

- `room_tuning2_matches_secs2_mimo` (above) and `room_tuning2_mimo_fast_normal_equations`.
- `room_tuning2_one_filter_per_speaker`: without MIMO both ears of a speaker get the same filter; the left/right transfer ratio of the tuned BRIR equals the untuned one to 1e-9 relative before cropping (compare full convolutions).
- `room_tuning2_latency`: every unmeasured channel moves by exactly D_s; measured channels' design origin is D_s (`zero` mode: no delay, unchanged lengths); output length is old length + D_s.
- `room_tuning2_supports_are_band_limited`: with MIMO, the tuned BRIR minus the RC-only result has no energy above 300 Hz beyond −80 dB of its in-band level, and it is the sum of `BRIR_j ⊛ q_ij`.
- `room_tuning2_mimo_reduces_late_energy_at_the_ears`: on the demo, for every input with supports, the late (≥ 60 ms after onset) 20–Fsiso energy of the tuned BRIR at both ears (sum) is lower than RC alone; report per input in dB.
- `room_tuning2_no_data_changes_nothing`, `room_tuning2_level_rules` (min reference, skip on > 2 dB inconsistency, off → all zero).
- `room_tuning2_runs_on_the_demo_dsp`: full `prepare_tuning2` + `apply_tuning2` on the demo (mixed, MIMO off and on): finite outputs, no geometry mismatch, no weak, report the per-speaker table (level, trim, PEQ count, strength, pre-echo, EDT/excess GD before/after) and, with MIMO, supports per input.
- `room_tuning2_is_thread_count_free`: the tuned demo BRIR (with MIMO) is bit-identical with a 1-thread and a 4-thread rayon pool.
- All existing tests pass unchanged; v1 and EQ outputs stay bit-identical (`room_tuning*`, `room_v2*` tests, oracle tests).

## Benchmark and performance report

Add cases to `benches/perf.rs`: MIMO design for the demo (7 inputs), and a synthetic 24-speaker set (R = 23, m = 4140: e.g. demo BRIRs reused with different delays) per input. Then write `docs/rust/perf/room-tuning2.md`: same container, Rust vs SECS 2 timings (SECS's per-run times are in the goldens: `secs_seconds`; the SECS IIR stage that Impulcifer does not need had not finished after 30 minutes of two workers, about 1.9 CPU-hours, on the same 7-speaker demo), RC per mode per speaker, MIMO design total and per input, the 24-speaker scaling case, the whole `apply_tuning2` on the demo, and peak memory if you can measure it (`/usr/bin/time -v` on the bench binary). State plainly where Rust is faster and by how much.

## Verification (foreground; paste tails)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --no-fail-fast
cargo test -p impulcifer-service --no-fail-fast
git status --porcelain
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails here because the container runs as root; it must be the only failure.

## Report format

(1) API as landed; (2) the normal-equation construction and solver chosen, with the dense cross-check numbers; (3) MIMO reference table (ranges, off inputs, level differences, support errors, late metrics) and bounds; (4) demo tables (RC-only and with MIMO); (5) performance table vs SECS; (6) `test result:` lines; (7) `git status --porcelain`; (8) anything undone or doubtful in the spec (quote the line). Do not end your turn before the commands complete.
