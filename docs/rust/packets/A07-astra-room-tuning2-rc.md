# A07 (ASTRA): virtual room tuning v2 — channel correction (RC) core

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `CLAUDE.md` (section "3.x Rust 워크스페이스"), `docs/adr/0008-virtual-room-tuning-v2.md` and, in `docs/rust/ROOM_CORRECTION.md`, the whole section **"Virtual room tuning v2"** — it is the specification you implement, number for number. This packet is the first of three: here only the channel correction (RC) of one speaker plus the channel level, as a library in `impulcifer-dsp`, checked against the reference numbers. MIMO, applying to the BRIR, config/CLI/service wiring come in later packets.

Rules: `#![forbid(unsafe_code)]`, no shell subprocesses in library code, every command in the foreground. If a build runs out of disk, delete `target/debug/incremental` and retry.

**Clean-room rule.** v2 reproduces a GPL-3.0 program. Its source must not reach this implementation. Do not search for, download, open or read any SECS / "Sound Equalization and Correction System" source or binary, and do not open anything under `/tmp/claude-0/`. Implement only from the specification, this packet, the goldens and the repository. You **may** read the BSD-licensed numpy and scipy sources installed at `/usr/local/lib/python3.13/dist-packages/{numpy,scipy}` and run them in Python to make your own test references (that is how you get exact semantics of `least_squares`, `gradient`, `butter`, `sosfilt`, `find_peaks`, `firwin`, `geomspace`).

Allowed files: new `crates/impulcifer-dsp/src/stages/room_tuning2/` (`mod.rs` plus submodules of your choice, e.g. `primitives.rs`, `rc.rs`), new `crates/impulcifer-dsp/src/optimize/` (or `optimize.rs`) for the least-squares port, `crates/impulcifer-dsp/src/stages.rs` / `lib.rs` (module lines), `crates/impulcifer-dsp/src/filters.rs` (add a band-pass design; existing functions unchanged), `crates/impulcifer-dsp/Cargo.toml`, root `Cargo.toml` `[workspace.dependencies]` (only to add `faer`), `Cargo.lock`, new tests `crates/impulcifer-dsp/tests/room_tuning2_*.rs` and `crates/impulcifer-dsp/tests/trf.rs`, `crates/impulcifer-dsp/benches/perf.rs` and its smoke test, new `tests/migration/gen_trf_reference.py` and its output `tests/migration/goldens/trf_scipy_reference.json`, `THIRD_PARTY_LICENSES.md` (add faer, MIT, and the scipy port, BSD-3 — scipy's licence text). Do not touch anything else (in particular `room_tuning.rs` (v1), `impulcifer-types`, service, CLI, `apps/**`, locales, versions, `CHANGELOG.md`, `README.md`, `.github/**`, the Python 2.x tree, `docs/**`).

## Dependency

Add `faer` (latest 0.24.x, MIT) for dense linear algebra (thin SVD here, symmetric factorizations in the MIMO packet). Disable default features and enable only what you need (check what `std`/`linalg`/`rayon` provide in 0.24). Use it **sequentially** (`Par::Seq` or equivalent): outputs must not depend on the thread count. Note it in `THIRD_PARTY_LICENSES.md`.

## Public API (in `impulcifer_dsp::stages::room_tuning2`)

Names are fixed where given; add what you need around them.

```rust
pub enum Tuning2Mode { Zero, Mixed, Linear, Long, Ultra(f64) }   // Ultra = latency in ms, 30–200
impl Tuning2Mode {
    pub fn parse(s: &str) -> Option<Self>;   // "zero"|"mixed"|"linear"|"long"|number 30..=200
    pub fn params(&self, fs: u32) -> ModeParams;  // taps (0 for Zero), peq_bands, lift_ms (after the cap), phase_frac (6 or 24)
}
pub enum PositionPolicy { Symmetric, Listening { index: usize, weight: f64 } }
pub struct RcOptions {
    pub mode: Tuning2Mode,
    pub max_boost: f64,        // 0..=10, default 10
    pub low: f64,              // default 20
    pub high: Option<f64>,     // None = automatic
    pub schroeder: Option<f64>,// schroeder_freq override for the automatic band
    pub room_volume: Option<f64>,
    pub policy: PositionPolicy,
}
pub enum BandSource { Manual, Override, Volume, Default }
pub struct CorrectionBand { pub low: f64, pub high: f64, pub source: BandSource, pub schroeder: Option<f64> }
pub fn correction_band(points: &[&[ImpulseResponse]], opts: &RcOptions) -> CorrectionBand;
// points: every measured speaker's positions (all of them feed the automatic estimate)
pub fn channel_level(points: &[ImpulseResponse], policy: &PositionPolicy,
                     target: Option<&FrequencyResponse>, calibration: Option<&FrequencyResponse>) -> f64; // L_i in dB
pub struct SpeakerRc {
    pub points: usize,
    pub level_db: f64,
    pub design_db: Vec<f64>,           // U0 on the analysis grid
    pub peq: Vec<(f64, f64, f64)>,     // final PEQ bands (f0, Q, gain dB)
    pub peq_fits: Vec<Vec<(f64, f64, f64)>>, // every Fit output in call order (debug/reference)
    pub fir: Vec<f64>,                 // empty in Zero mode
    pub phase_strength: f64,
    pub phase_frac: f64,               // β used
    pub long_modes: Vec<(f64, f64)>,   // (Hz, T60 s) of the limited modes, in the mask
    pub response: Vec<Complex64>,      // F_total on the analysis grid (N/2+1 bins)
    pub lift_samples: usize,           // D_s
}
pub fn design_rc(points: &[ImpulseResponse], opts: &RcOptions, band: &CorrectionBand,
                 target: Option<&FrequencyResponse>, calibration: Option<&FrequencyResponse>)
                 -> Result<SpeakerRc, DspError>;
pub fn analysis_len(fs: u32) -> usize;  // N
```

`points` are the speaker's uncropped split omni IRs in position order (left, right). `Symmetric` and `Listening { index, weight }` follow the spec's "Position policy" (the listening-position weight is constant across frequency). Reject sample rates below 44100 Hz and inconsistent/empty/non-finite inputs with `DspError::InvalidArgument`. Speakers are independent: callers may run them in parallel; inside one speaker keep every floating-point accumulation in the specified order.

## The least-squares port

`least_squares` semantics are part of the result (the RC fits stop at `max_nfev = 150` most of the time). Port from scipy 1.18.1 (`optimize/_lsq/least_squares.py`, `trf.py` → `trf_bounds`, `common.py`, `optimize/_numdiff.py` → bounded `'2-point'`) exactly the path used here: `method='trf'`, bounds, `jac='2-point'` with the default relative step and its bound-aware one-sided adjustment, `x_scale=1`, `loss='linear'`, `ftol=xtol=gtol=1e-8`, `tr_solver='exact'`, `max_nfev`; return the final x, cost, nfev, status. Keep scipy's evaluation order (how many function calls per iteration and which x they see), its termination tests and its trust-region update; credit scipy in a module comment and in `THIRD_PARTY_LICENSES.md`.

Make it generic (residual closure over `&[f64] -> Vec<f64>`), and make the PEQ residual cheap without changing its value: a Jacobian column changes one band, so cache band responses and multiply the 16 band responses **in band order** as the spec says (do not divide one band out of a running product). Write `tests/migration/gen_trf_reference.py` (scipy only) producing `trf_scipy_reference.json`: at least (a) a bounded smooth curve fit that converges, (b) a problem with an active bound, (c) a 16-band PEQ refinement of a synthetic dB curve with the spec's ring boxes that stops at `max_nfev=150`. `trf_matches_scipy` checks x, cost and nfev (report the measured differences; set bounds from them).

## Reference test against SECS 2

`tests/migration/goldens/room_tuning_secs2_rc.json` (read its `provenance`, `ir`, `fields`): 62 runs of SECS 2 on the demo omni files, `n_fft` 131072, comparison `bins`, per entry `mode` (`zero|mixed|linear|long|ultra-60`), `sides` (positions in order), `main_weight` (SECS listening-position weight: use `PositionPolicy::Listening { index: 0, weight }`), `target` (`flat`, or `demo` = `data/demo/room-target.csv` as a `FrequencyResponse` interpolated linearly in Hz), `rc_lo`, `rc_hi_setting` (null = automatic), `room_volume_m3`, `max_boost_db`, and SECS's results `rc_hi`, `schroeder_hz`, and per speaker `gain_db`, `peq`, `phase_strength`, `phase_frac_used`, `fir_taps`, `mag_db`, `phase_rad` (angle of `F_total*exp(+j2πf·lift)`), `design_db` (one-position entries), `peq_fits`. Build the IRs as `ir.construction` says (same code as `crates/impulcifer-dsp/tests/room_tuning_secs.rs`: `SweepEstimator` from the sweep WAV, `Hrir::open_recording_samples(…, 2.0)` per file; no cropping). In the multi-speaker entry `all/mixed`, `gain_db` = `min_j L_j − L_i` over its seven speakers.

`room_tuning2_matches_secs2_rc`: for every entry and speaker compute `design_rc` and compare at `bins`: magnitude dB difference and wrapped phase difference, maxima per band 15–100, 100–500, 500–2000, 2000–23000 Hz; `rc_hi` (1e-6 Hz), `schroeder_hz`, `fir_taps`, `phase_frac_used`, `phase_strength` exactly; levels via `all/mixed` gains (report). Print a table per entry (worst band errors, which PEQ fit first differs and by how much). Work stage by stage until the chain agrees — the design curve (`design_db`, compare in single-position entries; expect ~1e-9 dB), then the first Fit, the realization, the second Fit, the touch-up — and report every divergence you found and its cause. The PEQ fits are where tiny floating-point differences can grow (finite-difference Jacobian, early stop); bring the port as close to scipy's arithmetic as is reasonable (e.g. numpy's complex division algorithm if it matters — measure), then set the bounds as named constants: measured maximum × 1.5, rounded up to 0.01 dB / 0.001 rad, minimum 0.01 / 0.001, with a `// provisional: set by the owner after review` comment. A band with a large error is a finding to report, not something to hide behind a loose bound.

Also add, in `crates/impulcifer-dsp/tests/room_tuning2_rc.rs`:
- `room_tuning2_symmetric_one_point_is_secs`: one position, `Symmetric` and `Listening{0, 5.0}` give identical responses.
- `room_tuning2_symmetric_is_order_free`: swapping the two positions under `Symmetric` changes `response` by < 1e-9 dB / 1e-9 rad.
- `room_tuning2_modes_have_their_latency`: `params` for every mode at 48 kHz (taps, PEQ count, lift; 60 ms → 4096 taps, 200 ms → 16384) and at 96 kHz / 44.1 kHz (N even, taps scaled, lift in ms unchanged); `Tuning2Mode::parse` accepts/rejects the documented values.
- `room_tuning2_band_rules`: manual band, `schroeder` override, volume estimate, default 500, the 100 Hz floor.

## Benchmark

Add RC cases to `benches/perf.rs` (one demo speaker, two positions, each mode) and keep the smoke test passing. Report the time per speaker per mode next to SECS's own time from the golden (`secs_seconds`, which is per run of that entry on this container's CPU).

## Verification (foreground; paste tails)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --no-fail-fast
cargo bench -p impulcifer-dsp --bench perf -- room_tuning2   # or however the bench filters; report its lines
git status --porcelain
```

## Report format

(1) public API as landed; (2) the least-squares port: what you ported, how you verified it, trf reference differences; (3) every divergence from SECS found while bringing the stages together and its cause; (4) the reference table: per entry worst magnitude/phase error per band, level/gain differences, PEQ count equality; the bounds you set; (5) timing per mode vs SECS; (6) `test result:` lines; (7) `git status --porcelain`; (8) anything undone or doubtful in the spec (quote the line). Do not end your turn before the commands complete.
