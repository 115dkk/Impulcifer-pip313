# P09 FrequencyResponse oracle fixtures

Run `python E:/Impulcifer/tests/migration/export_goldens_fr.py` in the existing
2.x environment, in the foreground. This exporter writes only `p09_*` files.
It does not invoke the other batches' export functions or modify their files.
The frozen environment is Python 3.14.5, NumPy 2.5.3, SciPy 1.18.1 (re-exported 2026-09-07; identical values under 3.13.3 / 2.4.6 / 1.18.0), seed 909.
`p09_environment.json` records the actual NumPy build configuration and platform.

## Encoding and provenance

Every JSON fixture contains `inputs`, `outputs`, and `meta`, including the
Python source line range. All input states are copied before mutation.
Numbers retain Python round-trip precision; nonfinite values are strings
`nan`, `+inf`, and `-inf`. Use serde_json's workspace `float_roundtrip` feature.
Arrays longer than 4096 values use the P05 descriptor (`file`, `length`,
`sha256`, `first`, `last`). Sibling `.f64` files contain every sample as
little-endian IEEE float64, without headers. The first/last descriptors have
256 samples each. Tests compare the full arrays, not just their endpoints.
Identical bytes are not rewritten. The current export contains 69 files.

The synthetic response has 300 irregular log-spaced samples between 15 Hz and
22 kHz, a cosine trend and a 2.4 kHz notch. It tests log-grid extrapolation,
a 4800-point linear grid starting at zero, missing raw samples, all seven
interpolated fields, discarded smoothed data, and centering's reset sets.

The compensation target is `E:/Impulcifer/data/harman-in-room-loudspeaker-target.csv`.
CSV fixtures also cover all five recursively discovered data CSVs, including
`data/demo/room-target.csv`, with original byte SHA-256 and normalized text.
Synthetic strict/guess/single-digit files exercise both parsing branches.

Headphone measurements come from the actual `headphone_compensation` function
and bundled sweep estimator. The complete demo directory is copied to a
TemporaryDirectory before running that function, so its WAV/plot side effects
never touch the original demo. Both left error and right error are frozen.
The left response supplies heavy/light smoothing, the three application's
EQ parameter sets, 9600/4800-tap FIRs and the three standalone smoothing cases.
The repository does not contain the packet's `data/demo/FL.wav`; the magnitude
fixture uses the first track's first 8192 samples of the existing
`E:/Impulcifer/data/demo/FL,FR.wav`. Its actual source is recorded in the fixture.

## Numerical gates

Run from `E:/Impulcifer`, in the foreground.

```text
python E:/Impulcifer/tests/migration/export_goldens_fr.py
git -C E:/Impulcifer status --short data/demo
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --test golden_fr --test properties_fr
cargo test -p impulcifer-policy
```

For measured maxima, add `-- --nocapture --test-threads=1` to the golden command.
Frequency grids, window sizes and CSV parses are exact. All dB arrays enforce
`abs(actual-reference) <= 1e-9 + 1e-11*abs(reference)`. FIR taps enforce
`1e-9*max(1,reference peak)` with no relative term. FIR spectra compare the
complete padded final filter at twice the internal minimum-phase FFT length,
with error strictly below 1e-5 dB above the relative -100 dB floor; below that
floor complex absolute error must be at most `1e-9*max(1,spectral peak)`.
The FIR test collects every failing case before asserting, without relaxing
any tolerance. The current real fixtures have no spectral bins below that floor.

`p09_minimum_*` also save the exact linear mesh, linear gain and pre-transform
FIR for diagnosis. The current complete FIR chain fails its tap and spectral
gates. Calling the existing minimum_phase primitive on Python's frozen linear
FIR agrees within 1.333e-15; designing from Python's frozen gain with the existing
firwin2 then minimum_phase reproduces the failing chain. This remains an open
primitive-composition issue; no golden or tolerance is adjusted to hide it.

## Preserved AutoEQ behavior

- Repeated-multiplication grids have 783/695/713 points. Half-even window rounding
  and a naive ratio sum produce windows 7/13/15/23/91/139.
- Target shelves are designed/evaluated at 44100 Hz regardless of project rate.
  Existing RBJ primitives reproduce all four target fixtures exactly.
- Center re-grids to 20..20000 Hz; center_value does not. Error arrays shift
  oppositely to raw/smoothed, and target survives centering.
- Interpolation prunes NaNs only in raw/frequency, discards smoothed, and changes
  only the first zero query to .001 Hz while evaluating, then restores zero.
  Other populated fields are not pruned and can fail the spline shape check.
- Compensation centers a copy at 1000 Hz and uses same-length arrays positionally;
  optional 100..10000 Hz error mean shifting does not re-grid.
- Smoothing uses two full-range quadratic Savitzky-Golay passes and sigmoid
  blending. Heavy/light chooses signed maximum, not maximum magnitude.
- Equalization prefers error_smoothed, clips only positive gains against its
  sigmoid ceiling, omits index-zero kinks, rescues the last two samples and
  re-splines with degree two. It never re-clamps or clears equalized_smoothed
  at entry. The default treble ceiling remains 6 dB even with max_gain=40.
- FIR synthesis halves f_res, uses floor fs//2 and legacy 235 lengths, extends
  the low-frequency gain flat, doubles dB before minimum phase and forces the
  Nyquist gain to zero. The implemented optional normalization subtracts the
  maximum then 0.5 dB; app fixtures use normalize=false.
- CSV floats require two digit characters; exponent notation is not recognized.
  Strict headers must start at byte zero and have only known columns. Literal
  CRLF/BOM strings miss the strict branch. File reading normalizes CRLF/CR like
  Python text mode; parse_csv retains supplied text. Empty parsed frequency
  selects the default grid, and error=-raw remains the caller's responsibility.
- Magnitude conversion drops DC, applies half-even decimation, then log-linear
  interpolation. Empty/single-sample guards return zeros on 10..fs/2.

Rust uses the packet's non-empty trimmed-name validation, f64 NaNs for missing
samples, InvalidArgument errors for shape/CSV failures, and existing spline/FIR
finite-input validation. Parametric/fixed-band arrays are omitted; their reset
flags are no-ops. No registry entries are edited by this packet.

## Oracle noise floor of `minimum_phase_impulse_response`

The full chain (`interpolate` on the linear mesh, `firwin2(2n)`, homomorphic `minimum_phase(n_fft=2n)`) is not reproducible to the primitive tolerances even in Python. The linear-phase FIR is Type II, so its Nyquist bin is an exact zero that only FFT rounding fills (about 1e-11 for the demo case), and that same bin sets the `1e-7 * min(|H| > 0)` log floor. `tests/migration/oracle_noise_minimum_phase.py` perturbs the Python linear FIR by 1e-12 (relative or absolute to the peak, the size of the Rust/Python `firwin2` difference) and measured on 2026-09-07 (Python 3.13.3, SciPy 1.18.0):

| perturbation | taps (relative to peak) | dB below 16 kHz | 16 to 20 kHz | 20 to 23 kHz | 23 to 23.9 kHz | above 23.9 kHz |
|---|---|---|---|---|---|---|
| `lin * (1 + 1e-12 noise)`, 3 trials | 7.0e-6 to 1.1e-4 | up to 1.1e-3 | up to 3.4e-4 | up to 1.7e-3 | up to 1.8e-2 | up to 1.8 |
| `lin + 1e-12 * peak * noise`, 2 trials | 4.2e-4 to 4.4e-4 | up to 4.9e-3 | up to 1.5e-3 | up to 7.6e-3 | up to 7.8e-2 | up to 9.2 |
| `lin[0] += 1e-13` | 9.7e-7 | 9.5e-6 | 2.9e-6 | 1.5e-5 | 1.6e-4 | 1.8e-2 |

The Rust chain against the fixtures lands inside that spread (taps 3.2e-5 relative, 3.1e-4 dB below 20 kHz, 4.8e-4 dB at 20 to 23 kHz, 5.1e-3 dB at 23 to 23.9 kHz, 0.74 dB in the last 100 Hz), so `golden_minimum_phase_impulse_response_matches_python` uses the measured envelope with a margin of about four (taps `1e-3 * peak`; 2e-2 dB below 20 kHz, 5e-2 dB at 20 to 23 kHz, 0.3 dB at 23 to 23.9 kHz, 20 dB above 23.9 kHz) instead of the primitive budget. The primitives themselves (`firwin2` on the Python gain, `minimum_phase` on the Python FIR) still meet their own budgets (4.2e-12 taps and 1.3e-15 taps respectively, printed as DIAGNOSTIC lines). The BRIR-level parity gate has to absorb the same spread.
