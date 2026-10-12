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
those points), `jac_x` (every point where it formed a Jacobian), `result`
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

| Family | Starting budget |
| --- | --- |
| smoothing, frac_for, peaking, ring caps/boxes, firwin, hp4 | rel 1e-12 |
| minimum phase | 1e-9 dB, 1e-9 rad above −100 dB |
| fit traces | every `main_x` within rel 1e-9 of its index; same `nfev`, `njev`, `status`; final `x` rel 1e-9 |
| RC stage spectra | 1e-6 dB / 1e-6 rad (after the first fit agrees) |
| MIMO normal equations | rel 1e-12 (diagonal, block rows, Aᵀb); identical bisection decisions; u rel 1e-9 |
| guard values | 1e-9 absolute |
| integer results (onsets, bins, counts, chosen strength and resolution) | exact |

The end-to-end references `room_tuning_secs2_rc.json` and
`room_tuning_secs2_mimo.json` stay as the integration check on top of these.
