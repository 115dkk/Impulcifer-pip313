# P03 migration goldens

Run `python E:/Impulcifer/tests/migration/export_goldens.py` from the existing
2.x environment. The exporter imports the repository's `core.audio_io`,
`core.virtual_bass` and `autoeq.biquad` oracles. It writes only its explicitly
named `p03_*.json` files. No timestamps or random global state are used;
noise comes from `numpy.random.default_rng(1234)`.

The recorded oracle is Python 3.13.3, NumPy 2.4.6, SciPy 1.18.0. These are
actual installed versions, not the SciPy 1.16 documentation version requested
for API research. Every fixture contains `inputs`, `outputs`, and `meta`.
Arrays use repr-precision JSON numbers; complex numbers use `[real, imaginary]`.
Nonfinite values are strings `"-inf"`, `"+inf"`, and `"nan"`. Rust must enable
serde_json's `float_roundtrip` feature: its default fast parser can change
coefficients by an ULP and corrupt ill-conditioned RBJ response comparisons.

There are 468 fixtures, each below 200,000 bytes. The 4096-sample linear chirp
and individual SOS output share one fixture. The 32001-tap Kaiser fixtures
store first/last 64 taps plus the sum; other windows store every tap. Noise
lengths include 257, 1024 and 1025. Tests also cover impulses, silence, scalar
and empty cases where meaningful, FFT spectrum padding/truncation, asymmetric
convolution/correlation, odd/even Butterworth sections, pipeline shelves,
Q=20, Q=0.1, gain +/-60 dB, and a near-Nyquist filter.

## Semantic decisions

- `next_fast_len(n)` means `scipy.fft.next_fast_len(n, real=True)`. Both it and
  legacy fftpack use **2,3,5** for this oracle. P03's 2,3,5,7,11 real-input
  shortcut is incorrect; those additional primes describe complex transforms.
  The [SciPy 1.16 prev_fast_len reference](https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.fft.prev_fast_len.html)
  explicitly distinguishes the sets. Installed results include 7 -> 8,
  11 -> 12, 1001 -> 1024, 295270 -> 300000. The modern complex result for
  295270 is 295680 and is saved only as a diagnostic, not the implemented API.
- NumPy 2.4.6 `irfft(empty, n)` returned allocator-dependent values across
  fresh exporter processes on this machine. Those four fixtures record an
  explicit empty-spectrum error policy, and Rust asserts rather than freezing
  nondeterministic values. Nonempty spectrum padding/truncation follows NumPy.
- `Same` convolution/correlation has the first argument's length, beginning
  at `(len(second)-1)/2` in the full result. `correlation_lags(Same)` separately
  follows SciPy's midpoint algorithm, including its one-index discrepancy for
  odd first/even second lengths. No index correction is applied.
- `Biquad.a` is `[1,a1,a2]` in the conventional denominator. AutoEQ's returned
  feedback terms have reversed signs, which `digital_coeffs` reverses again.
  Response evaluation preserves AutoEQ's polynomial magnitude algebra.
- Butterworth design uses analog prototype, LP/HP scaling, prewarping,
  bilinear mapping and nearest pole/zero pairing. Odd orders retain the
  origin-root padding and potentially split zero/pole placement of SciPy.
  `tf2sos` is only the real biquad subset: normalization and polynomial
  reconstruction avoid a redundant root round trip, within coefficient gates.
- Installed SciPy 1.18 `get_window` rejects N=0; individual Hann/Hamming/Kaiser
  functions return empty. The wrapper returns `DspError::InvalidArgument`.
  Empty `sosfilt` samples are rejected with an explicit assertion.
- Installed SciPy 1.18 regression returns NaNs for empty/singleton/constant-x
  data; constant-y rvalue/stderr are NaN, and two-sample stderr is NaN. This
  differs from older SciPy (notably two-sample stderr zero). Goldens freeze
  the installed behavior; pvalue is intentionally `None`.
- Kaiser I0 uses the positive power series, not an imported approximation.
  Very large beta preserves the oracle's intermediate `exp(abs(beta))`
  overflow and NaN masks rather than silently using a scaled window.
- Infallible functions document argument assertions. Nonfinite samples
  propagate where arithmetic is defined; uniform filtering rejects nonfinite
  input because SciPy documents that behavior as undefined. Shape mismatches,
  invalid cutoffs/Q/fs, and impossible lengths panic explicitly.
- Complex64 is `rustfft::num_complex::Complex64`, the existing re-export.
  A new direct dependency would modify the tracked Cargo.lock outside this
  packet's allowed writes. No lockfile update is needed for float_roundtrip.

## Gates and measurements

Run from E:/Impulcifer in the foreground.

```text
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --test golden_batch1 -- --nocapture --test-threads=1
```

`MEASURE` lines print maximum absolute error, worst fixture relative L2 error,
and compared finite-value count. Nonfinite masks and output shapes must match
separately. Every failure identifies fixture/index, values and tolerance.
FFT and convolution also enforce relative L2 <= 1e-11. The chained
Butterworth/chirp test complements isolated SOS coefficient and stored-SOS
filtering comparisons.

| Family | Relative / absolute gate |
|---|---|
| FFT complex/real | 1e-11 / 1e-12 * max(1,input L1) |
| Magnitude dB | 1e-11 / 1e-12; exact infinity mask |
| Convolution/correlation | 0 / 1e-11 * max(1,input L2 * kernel L2) |
| Lengths/lags | exact integers |
| SOS/RBJ/tf2sos coefficients | 1e-11 / 1e-13 |
| Stored and designed SOS filtering | 1e-9 / 1e-11 * reference peak |
| RBJ dB response | 1e-11 / 1e-9 dB (isolated frozen coefficients) |
| Window taps | 0 / 1e-12 |
| Kaiser sum | 0 / N * 1e-12 (sum of per-tap budgets) |
| Linregress | 1e-10 / 0; exact NaN masks |
| Expit | 1e-14 / 1e-15; exact NaN masks |
| Running mean | 1e-11 / 1e-12 |
| Uniform filter | 1e-9 / 1e-12 |

A unit test compares the I0 series directly with seven scipy.special.i0 values.
Pure property tests cover Parseval, inverse round trips, convolution linearity,
lowpass DC gain, filter state reset, window symmetry/periodicity, fast-length
minimality/overflow, shape and domain assertions, and nonfinite behavior.
