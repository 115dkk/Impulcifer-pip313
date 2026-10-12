# SECS 2 oracle fixtures (s2_*)

Virtual room tuning 2 (ADR 0008) is ported the way the 2.x tree was: an oracle
is run on fixed inputs, every function and stage boundary is frozen as an
`inputs`/`outputs`/`meta` fixture, and the Rust port is gated fixture by
fixture. The oracle here is SECS 2 Beta 1.1.0 (Speaker Gallery, GPL-3.0,
commit ceac0ad), not the 2.x tree. Specification:
`docs/rust/ROOM_CORRECTION.md`, "Virtual room tuning v2".

## Regenerating

```text
cargo run --release -p impulcifer-dsp --example secs2_inputs -- <inputs>
NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL AVX512_SPR" OPENBLAS_NUM_THREADS=1 \
  SECS2_SRC=<SECS 2 checkout>/src SECS2_INPUTS=<inputs> python tests/migration/export_goldens_secs2.py
```

**NumPy dispatch.** On an AVX-512 machine NumPy computes float64 `log10` and
`exp` with SIMD kernels that differ from the C library in the last bit (12 %
of `log10` and 4.7 % of `exp` values on the export machine); `sin`, `cos`
and `log` agree. A PEQ fit that stops after 150 evaluations amplifies such a
bit into a different path. The oracle therefore runs with NumPy's dispatch
above the X86-64-v2 baseline disabled (list exactly the features
`numpy.show_runtime()` reports as found): its transcendental functions are
then the C library's, the same functions Rust's `f64` methods call on Linux.
The exporter checks this (`check_libm`) and refuses to run otherwise; every
fixture's `meta.numpy_dispatch` records it. "SECS against itself" below shows
what the two dispatches do to SECS's own results.

The same exporter writes the two end-to-end references
(`room_tuning_secs2_rc.json`, `room_tuning_secs2_mimo.json`), which are
integration checks and may exceed 200 kB.

`examples/secs2_inputs.rs` writes the demo inputs exactly as the Rust tests
rebuild them: the uncropped split omni ear-position IRs
(`open_recording_samples(…, 2.0)`, 391270 samples) and the aligned in-ear
BRIRs at the tuning point (`crop_heads(1.0)`, `align_ipsilateral_all`,
`align_onset_groups_peak_leftref`, `crop_tails`; 24000 samples). The exporter
holds no SECS code: it calls the oracle and observes values by interposing on
module-level functions and on SciPy's trust-region routine. **Clean room:
people porting this must not open or run the exporter or the oracle.** The
recorded runtime is Python 3.13.16, NumPy 2.5.3, SciPy 1.18.1 (per-fixture
`meta`). The export has 544 files, 19.4 MB, every file below 200 kB; it
refuses to leave stale `s2_*` files.

## Encoding

As `README.md`: repr-precision float64 JSON, complex `[real, imaginary]`,
nonfinite as strings. Arrays longer than inline use headerless little-endian
float64 sidecars `s2_*.f64` described by `{file, length, sha256, first, last}`
(first/last 32 values); arrays above 24000 values are a list of such
descriptors in order. Spectra on the 131072-point analysis grid (48 kHz,
0.366 Hz bins) are stored at the bins of `s2_bins.json`: every bin below
1 kHz (0…2730), then every 32nd, plus Nyquist. A complex spectrum is
`{re, im}`. Matrices are row-major.

## Families

| Fixture | What is frozen |
| --- | --- |
| `frac_for` | variable smoothing resolution on the 4096-point grid |
| `smooth_{power,signed}_{1,3,6,24,frac_for}`, `smooth_complex_6` | fractional-octave smoothing (cumulative-sum boxcar) of positive, signed and complex data |
| `minimum_phase_{smooth,zeros,notch}`, `minimum_phase_n131072` | cepstral minimum phase, 4096 and 131072 points (`mag[k] = 1+0.5*cos(k/300)`) |
| `peaking_{0..6}`, `peq_response`, `peq_response_grid` | RBJ peaking responses, including gain 0, Q 40, 15 Hz and near-Nyquist bands |
| `ring_q_cap`, `ring_boxes` | the ring-time Q cap and the per-band bound boxes |
| `fit_synthetic_*` | PEQ fits of synthetic curves with the full trust-region trace and the initial Jacobian |
| `fit_<case>_s<k>_<i>` | every PEQ fit inside the demo channel corrections, with the trace (no Jacobian) |
| `gd_curve_{6,24}` | group delay after peak-delay removal and complex smoothing |
| `onset_window` | onset index and the 500 ms RC window |
| `to_low_{300,720}`, `late_peaks`, `mode_t60`, `firwin_*`, `hp4`, `butter_band_*` | the 600 Hz helpers of the long-mode limit and MIMO |
| `rc_<case>` | a demo channel correction: settings, band, level, strength, resolution, final PEQ, final response, the oracle's notes, and its session list |
| `rc_<case>_s<k>` | one correction run (two for linear and ultra: resolution 1/24 then 1/6): power average, the two smoothings, design curve, long-mode bins, first-PEQ minimum phase M, phase phasor average and its smoothing, raw and smoothed delay, every FIR realization in order, touch-up minimum phases, final FIR, response |
| `levels_w{1,5}` | channel levels of the seven demo speakers, two positions, weight 1 and 5 |
| `mimo_<setting>` | support bands, channel trims and inputs for an RC on the demo BRIRs (left, right ear; weight 1) |
| `mimo_<setting>_<input>` | per input: x and g at 600 Hz per ear, the Fsiso low-pass, the RC energy, normal-matrix diagonal, the first row of every support block, Frobenius norm and trace, Aᵀb, the bisection's log10(λ/σ) per solve and the support ratios, the kept solve and its u, guard band and tail values per tried scale, the final supports and level |

Fit fixtures: `inputs` holds the log grid, the target curve, n and the
keyword arguments (lo, hi, boost, ring_ms, f_max). `outputs` holds the final
bands, the start point `x0`, bounds `lb`/`ub`, `max_nfev`, the initial cost,
`main_x` (every point where the solver evaluated the residual for a step, in
order, `params` values each), `main_cost` (half the squared residual norm at
those points), `jac_x` (every later point where it formed a Jacobian; the first one, at `x0`, is `J0`), `result`
(`x`, `cost`, `nfev`, `njev`, `status`, `optimality`) and, for synthetic fits,
the initial two-point Jacobian `J0` (`J0_shape`). A fit without bands has no
trace.

RC case names: `fl_both_mixed`, `fl_left_mixed`, `sl_both_linear`,
`bl_both_zero`, `fc_both_long`, `sr_both_ultra60`, `fl_both_mixed_main5`,
`fl_both_mixed_target_volume40` (demo room target, V = 40 m³),
`br_both_mixed_band30_300`. MIMO settings: `mixed_fsiso150_auto`,
`mixed_fsiso100_low40`. Guard lists have one value per tried scale
(0, −3, −6, −9, −12 dB) up to the accepted one; the tail value is the oracle's
scaled form (dB × 0.15, accepted at ≤ 0.15).

## SECS against itself

`room_tuning_secs2_variance.json` recomputes the end-to-end references with
the same oracle, data and machine, changing one computational detail at a
time the way other computers run SECS 2: NumPy's AVX-512 kernels instead of
the C library; OpenBLAS's Haswell, Sandybridge and Nehalem kernels instead of
this CPU's; LAPACK's `gesvd` instead of `gesdd`. Each differs from the
reference only in the last bits of some operations.

| Variant (61 per-speaker RC references) | Largest change, 15–100 / 100–500 / 500–2000 / 2000–23000 Hz (dB) | Channels > 0.1 dB | > 1 dB |
| --- | --- | --- | --- |
| NumPy AVX-512 | 1.86 / 1.55 / 0.41 / 0.049 | 22 | 8 |
| OpenBLAS Haswell | 2.33 / 2.66 / 0.23 / 0.040 | 24 | 7 |
| OpenBLAS Sandybridge | 2.09 / 1.60 / 0.41 / 0.041 | 22 | 9 |
| OpenBLAS Nehalem | 1.98 / 1.53 / 0.48 / 0.040 | 21 | 7 |
| LAPACK gesvd | 1.97 / 1.97 / 0.74 / 0.057 | 24 | 9 |
| Envelope (largest over the five, per channel) | 2.33 / 2.66 / 0.74 / 0.057 | 32 | 15 |

Phase strength flips in one or two channels per variant; the number of PEQ
bands never changes. MIMO supports inherit the RC differences (bands always
identical; supports up to tens of percent apart where the guard settles one
step differently).

The cause is the PEQ refinement: it usually stops at its 150-evaluation cap,
and its trust-region path is determined only for the first few evaluations.
Every fit fixture records, per variant, the oracle's own distance from the
reference at each evaluation (`oracle_variants`) and the `horizon`: the first
evaluation where a variant that keeps the residual's arithmetic (another
factorization or BLAS kernel) leaves the reference by more than 1e-9. Before
the horizon those variants agree to about 1e-16; at the horizon they jump to
1e-8–1e-4. On the demo the horizon is evaluation 1–12. The AVX-512 variant
changes the residual itself (`log10`) and leaves at the first step; it is
recorded but not part of the horizon. This sets what
"equivalent" can mean, and why the port is gated stage by stage:

1. **Per stage**, fed the oracle's own inputs, the port reproduces the
   oracle's outputs bit for bit or to floating-point noise; a fit's trace
   matches up to its horizon (one evaluation of slack).
2. **End to end**, the port is one more computer: its deviation from the
   reference stays within the envelope (per band, and in the count of
   channels above 0.1 dB and 1 dB).

## Gates

Set from the measured agreement like every other family (measured maximum ×
1.5, rounded up, with a floor) and printed as `MEASURE` lines; the table below
is the starting budget, not a licence. Arithmetic that has one result
(smoothing, peaking, minimum phase on identical input, 600 Hz helpers, window
and onset, bounds) is expected at the 1e-12 relative level; the fit traces and
everything after the first fit are compared per evaluation, so a divergence
is found at its first step instead of at the end.

| Family | Actual gate / pending budget | Observed maximum (absolute; relative L2 unless noted) |
| --- | --- | --- |
| smoothing | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 0; 0 |
| frac_for | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 0; 0 |
| peaking / PEQ response | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 0; 0 |
| ring caps / boxes | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 0; 0 |
| firwin / hp4 | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 6.661338147750939e-16; 3.366572554664735e-16 |
| butter_band | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 6.439293542825908e-15; 1.467870505896583e-14 |
| gd_curve | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 3.463895836830488e-14; 3.335756460109128e-15 |
| to_low | rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 4.263256414560601e-14; 4.956033304240086e-16 |
| mode_t60 | rel 1e-12 / abs floor 1e-12; relative L2 1e-12; matching None | PASS: 5.151434834260726e-14; 3.624258339372087e-15 |
| onset_window | exact onset/index; window rel 1e-12 / abs floor 1e-12; relative L2 1e-12 | PASS: 0; 0 |
| late_peaks | exact | PASS: 0; 0 |
| minimum phase | absolute 1e-9 dB; absolute 1e-9 rad above −100 dB | PASS: combined dB/rad max 6.394884621840902e-14; diagnostic relative L2 5.778793496074114e-14 |
| levels_w1 | absolute 1e-12 | PASS: 7.105427357601002e-15; 9.293661446750417e-17 |
| levels_w5 | absolute 1e-12 | PASS: 7.105427357601002e-15; 9.204617316077994e-17 |
| RC pre-fit bands | absolute 1e-12; matching Schroeder presence | PASS: 0; 0 |
| RC pre-fit levels | absolute 1e-12 | PASS: 1.421085471520200e-14; 1.530080109777270e-16 |
| synthetic initial Jacobian J0 (five fixtures) | rel 1e-9, no absolute allowance | PASS: 0; 0 — bit-exact |
| fit traces | max absolute parameter difference / max absolute reference parameter ≤ 1e-9 at each evaluation before horizon − 1; prefix counts enforced; later evaluations and final costs report-only | A07d: 27/27 traced fixtures pass; maximum gated relative distance 2.3398629199314537e-14. `sr_both_ultra60_s0_1` first differs at evaluation 5, equal to horizon 5; its gated maximum is 1.8283410237449714e-16. Synthetic J0 remains bit-exact. |
| RC pre-fit production-chain spectra | relative 1e-12 / absolute floor 1e-12; relative L2 1e-12; exact long-mode bins | PASS across 11 sessions: power max 5.421010862427522e-19; smoothings max 8.673617379884035e-19; design max 5.684341886080802e-14; first fit input max 4.352074256530614e-14 |
| RC stage spectra / results after fitting | All 11 sessions have oracle factorization divergence: M, phasors, delays, realization sequences, touch-up minimum phases, final FIR/response and PEQ compared report-only on the production chain | First observed downstream differences are M (1e-4 to 0.1766 in complex components), or the zero-mode response (2.934e-4). No downstream equivalence claim. |
| RC end-to-end envelope | band caps = ceil(envelope max × 1.25 × 100)/100; >0.1 and >1 dB channel counts ≤ envelope | A07d PASS: maxima 1.994235049 / 1.919082892 / 0.765318729 / 0.057842253 dB versus caps 2.91 / 3.33 / 0.93 / 0.08. Counts above 0.001/0.01/0.1/1 dB: 56/36/19/7 versus envelope 61/39/32/15. Strength changes 2, PEQ count changes 0. |
| MIMO normal equations | rel 1e-12 (diagonal, block rows, Aᵀb); identical bisection decisions; kept u rel 1e-9 + abs floor 1e-12; trial ratios rel 3e-9 provisional | PASS: abs 2.6020852139652106e-18, relative L2 1.3910722556760364e-15 across 14 fixtures. Dense FL cross-check: matrix 4.367886386622643e-16, rhs 1.2718377008542517e-15, kept u 6.564296201359275e-14; 19/19 decisions. Largest discarded-trial ratio relative difference 1.6782690435074165e-9. |
| MIMO guard values | 1e-9 absolute, unchanged; failure remains blocking | A08b: none of the three hypotheses reproduces all 85 frozen band/tail values across 14 fixtures. H1 before∪after: 83/85, worst 0.14126000335392147 (mixed/100 FC trial 0); H2 before: 64/85, worst 0.7034637652374921 (mixed/150 FL trial 0); H3 after: 73/85, worst 0.6622879601134717 (mixed/100 FR trial 1). Tail units are 0.15 × dB. Production H1 unchanged, not accepted. Full peaks and per-mode a/b diagnostics: `docs/rust/perf/room-tuning2.md`. |
| MIMO end-to-end | Exact ranges and gains within 1e-10; guard-step difference count no greater than worst oracle variant | FAIL: mixed/150 changes 4 steps vs worst variant 2; linear/150 and mixed/100 count gates pass. Support/late errors printed, not yet bounded; perturbation attribution pending. |
| downstream integer results (bins, counts, chosen strength and resolution) | Exact starting requirement; not gated here | Not measured |

Measured on 2026-10-12 with `golden_secs2_primitives`, `golden_secs2_rc`,
and `golden_secs2_fit synthetic_initial_jacobian`. Existing floors are kept:
all passing observed maxima × 1.5 remain below their applicable floors.
Relative L2 is diagnostic for minimum phase and levels, not an extra gate.
A07c updates the fit and RC rows above. The synthetic J0 arrays stay exact;
the independent SciPy PEQ16 reference now carries a gesvd horizon (1), and
smooth/active-bound tests retain their previous gates. The former remaining fit-prefix failure is closed by A07d below; downstream
stage spectra remain report-only as described in the table.

A07c release-profile FL/two-position medians (seconds): mixed 1.144080,
zero 2.344558, linear 2.790939, long 1.053549, ultra-60 2.505206.
These do **not** meet the 0.1× oracle targets (0.088 / 0.155 / 0.222 /
0.073 / 0.191 s). Residual/Jacobian work still dominates. Opt-in per-thread
profiling is printed by the RC benchmark; timers are inclusive and must not
be summed. Residual output, Jacobian probe and band-response buffers are
reused; cached exact-order prefixes skip unchanged earlier bands without
regrouping products. Scalar smoothing powers are computed once per call.
FFT plans and scratch buffers were already cached in the shared FFT module.
That A07c SVD was sequential on the original augmented matrix.

## Specification-only TRF replay (A07d)

`replay_trf_fixture.py` reads only a fit fixture and its checked sidecars, builds
the RBJ residual from the specification, and calls SciPy 1.18.1. It does not
load the oracle or its exporter:

```sh
NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL AVX512_SPR" OPENBLAS_NUM_THREADS=1 \
  python tests/migration/replay_trf_fixture.py \
  tests/migration/goldens/s2_fit_sr_both_ultra60_s0_1.json \
  --out tests/migration/goldens/trf_replay_sr_both_ultra60_s0_1.json
```

The replay reproduces every recorded trial point and cost exactly (final cost
186.51429844881537). Its first-iteration JSON is 19,947 bytes; `--all-iterations`
retains every accepted/rejected trial. Wrappers observe SciPy module globals,
with return-frame capture for branch-local quantities. `replay_first_iteration`
compares the Rust diagnostics with that JSON; U's arbitrary column signs are
ignored only for `uf`, not for steps or trial points.

Before the fix the first quantity exceeding 1e-12 relative was actual reduction
(6.866091888814059e-12); the anti-gradient branch and the step already agreed
within 3e-15. Thus the identical divergence under two SVD builds did not imply
a different TRF formula: this branch does not use the SVD step. Replacing the
long scalar dot accumulator with eight independent accumulators and a balanced
merge closes the prefix gate, without changing residual arithmetic. The new
first-iteration maximum is 2.346391645408361e-13 (`uf`); actual reduction differs
by 4.655293164834266e-16. Final fit costs are still report-only, not constrained
to the range of the oracle variants.

A07d retains sequential Householder QR of the augmented Jacobian, applies Qᵀ
to the augmented residual, then takes the small R SVD; no tall U is formed.
Measured release-profile SVD-call averages (including QR) were approximately
1.03 ms at 748×48 and 1.39 ms at 948×48, versus 1.53 and 1.80 ms for the
original thin-U path. Runtime-dispatched band/product/magnitude loops retain
bit-exact synthetic J0; scalar libm log10 is separate. Mixed-mode residual
averages dropped from 28.0 to 19.8 µs (measurements vary under host load).
Linear/ultra sessions use `rayon::join`; a 1-thread/4-thread linear test compares
FIR and complex response bits and the chosen result. Profiling follows both
sessions and merges inclusive durations in fixed session order.

Final measured FL/two-position medians, seconds (2026-10-12):

| mode | one thread | default (4 threads) | oracle | one-thread speedup |
| --- | ---: | ---: | ---: | ---: |
| mixed | 0.358733 | 0.340503 | 0.88 | 2.453× |
| zero | 0.612948 | 0.631765 | 1.55 | 2.529× |
| linear | 0.591165 | 0.329069 | 2.22 | 3.755× |
| long | 0.228424 | 0.283438 | 0.73 | 3.196× |
| ultra-60 | 0.606082 | 0.348594 | 1.91 | 3.151× |

The requested 4× **one-thread** target remains unmet in every mode. At measured
log10 cost 7.707877 ns/point, residual-evaluation count × grid points × log10
cost plus measured total SVD time gives floors of 0.150577 / 0.342577 / 0.235500 /
0.115938 / 0.266405 s (mixed/zero/linear/long/ultra). These exclude all other work;
profiling timers are inclusive and must not be added together.

The end-to-end references `room_tuning_secs2_rc.json` and
`room_tuning_secs2_mimo.json` stay as the integration check on top of these.
