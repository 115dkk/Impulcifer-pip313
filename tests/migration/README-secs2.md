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

The same oracle, data and machine, exported once with NumPy's AVX-512
dispatch and once with the baseline (C library) dispatch, i.e. differing
only in the last bit of some `log10`/`exp` values:

| Measure (61 per-speaker RC references) | AVX-512 vs baseline |
| --- | --- |
| largest magnitude change, 15–100 / 100–500 / 500–2000 / 2000–23000 Hz | 1.86 / 1.55 / 0.41 / 0.049 dB |
| median of the per-channel maxima, same bands | 0.0066 / 0.0092 / 0.0012 / 0.00004 dB |
| channels changed by more than 0.001 / 0.01 / 0.1 / 1 dB | 56 / 35 / 22 / 8 |
| largest phase change (lift removed), same bands | 0.58 / 0.86 / 0.71 / 0.70 rad |
| phase strength changed | 1 channel (two/FR/mixed: 0.9 → 0.7) |
| PEQ band count changed | none |
| MIMO (3 settings × 7 inputs) | bands identical; supports change up to 52 % (relative L2); the guard settles one step apart for 2 inputs |

The cause is the PEQ refinement: it usually stops at its 150-evaluation cap,
so the trust-region path is part of the answer, and a last-bit change early
in the path ends in a different (equally good) set of bands. SECS's results on
another computer differ from these references by this much. This sets what
"equivalent" can mean end to end, and why the port is gated stage by stage:

1. **Per stage**, fed the oracle's own inputs, the port reproduces the
   oracle's outputs bit for bit or to floating-point noise (the fixtures).
2. **End to end**, where FFT and factorization roundoff necessarily differ in
   the last bit, the port's deviation from the oracle stays within the
   oracle's own deviation above (per band, and in the count of channels
   above 0.1 dB and 1 dB).

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
| fit traces | FAIL; no accepted measured budget. Existing check: every `main_x` within rel 1e-9 of its index; same `nfev`, `njev`, `status`; final `x` rel 1e-9 | FAIL on full rerun: bumps first exceeds at evaluation 5, parameter 0, absolute error 7.922355e-9. Combined fit-field max absolute 4.826111812403349e1 (fc_both_long_s0_0), max relative L2 7.577253599902453 (synthetic_zero_low_part); these are diagnostics, not accepted budgets |
| RC stage spectra / results after fitting | Not gated; starting 1e-6 dB / 1e-6 rad remains pending fit parity | Not measured in this partial run |
| MIMO normal equations | Out of scope; starting rel 1e-12 (diagonal, block rows, Aᵀb), identical bisection decisions, u rel 1e-9 remain pending | Not measured |
| MIMO guard values | Out of scope; starting 1e-9 absolute remains pending | Not measured |
| downstream integer results (bins, counts, chosen strength and resolution) | Exact starting requirement; not gated here | Not measured |

Measured on 2026-10-12 with `golden_secs2_primitives`, `golden_secs2_rc`,
and `golden_secs2_fit synthetic_initial_jacobian`. Existing floors are kept:
all passing observed maxima × 1.5 remain below their applicable floors.
Relative L2 is diagnostic for minimum phase and levels, not an extra gate.
This is a measured partial gate record, not completed fit, RC, or MIMO parity.

The end-to-end references `room_tuning_secs2_rc.json` and
`room_tuning_secs2_mimo.json` stay as the integration check on top of these.
