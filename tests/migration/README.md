# P03 migration goldens

Run `python E:/Impulcifer/tests/migration/export_goldens.py` from the existing
2.x environment. The exporter imports the repository's `core.audio_io`,
`core.virtual_bass` and `autoeq.biquad` oracles. It writes only its explicitly
named `p03_*.json` files. No timestamps or random global state are used;
noise comes from `numpy.random.default_rng(1234)`.

The recorded oracle is Python 3.14.5, NumPy 2.5.3, SciPy 1.18.1. The fixtures were re-exported on 2026-09-07 under that interpreter (`py -3.14`, the newest CPython on the reference machine, with the 2.x requirements installed); the earlier export under Python 3.13.3, NumPy 2.4.6, SciPy 1.18.0 produced numerically identical fixtures (616 files differed only in the recorded `meta` versions), so the interpreter version does not affect the oracle values. The performance audits use the same interpreter and the free-threaded 3.14t venv (docs/rust/packets/PA-astra-perf-audit.md). These are
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

## P05 DSP batch 2 goldens

Run the exporter with `IMPULCIFER_GOLDEN_BATCH=p05` in the environment to avoid
writing the concurrently developed P07 family. This selection skips P03 and
P07 entirely, preserving their bytes and original metadata. The no-argument
exporter retains P03 generation; it is not replaced by a read-only comparison.
In PowerShell, set `$env:IMPULCIFER_GOLDEN_BATCH = 'p05'` before the commands below.

P05 adds 175 files. Every file is below 200,000 bytes; the largest is a
131,080-byte FIR mesh. Inline arrays retain round-trip float64 JSON. Large
arrays use an object with `file`, `length`, `sha256`, `first`, and `last`.
`file` is a sibling `p05_*.f64` containing exactly `length` little-endian IEEE
float64 values, without a header. The hash covers all raw bytes; first/last
256 values permit inspection. Rust reads and compares the full arrays, never
only the ends. Exporting the same fixtures leaves equal bytes untouched.
`p05_environment.json` records the actual NumPy build/backend and platform.

The oracle runtime is Python 3.14.5, NumPy 2.5.3, SciPy 1.18.1. **Minimum phase
follows the installed SciPy 1.18.0 function**: its cepstral lifter midpoint is
one for even FFT lengths and two for odd lengths (`win[stop] = 1 + n_fft % 2`).
SciPy 1.17.1 and earlier used zero and one there; 2.x users run the fixed
version, so the port does not reproduce the old rule. The golden output `y` is
`scipy.signal.minimum_phase` itself; a transcription of the same branch (using
SciPy FFTs) supplies the intermediate log spectrum and lifter and is asserted to
agree with the function within 1e-12 of the peak tap.
The Rust unit test compares log spectra and exact lifters before the final
spectral test. FFT roundoff at the Type II near-zero Nyquist bin is amplified
by log; the minimum-phase helper reproduces pocketfft's real radix-2/3/4/5
Nyquist addition order for 235-smooth even lengths. Other bins/sizes retain
RustFFT, and no batch 1 FFT code changed. This is an O(n) single-bin reduction,
not a different logarithmic floor or a tolerance relaxation.

Fixtures cover actual AutoEQ 4800-point/9600-tap meshes with smooth, flat and
notched targets; duplicate/discontinuous meshes and explicit nfreqs; even,
odd and padded minimum-phase transforms with half/full outputs; W7/11/23,
695-sample noise, constant, linear and quadratic SG signals for 1/1000 passes;
additional W1/5/9/101 and polynomial degrees 0/1/3/4/6; exact plateau/threshold
peaks; irregular/minimal FITPACK grids; and the real Harman headphone CSV.
The log helper replaces an initial zero query by .001 Hz like AutoEQ without
mutating the caller. The Harman fixture retains its source-byte SHA-256.

Two packet/source discrepancies are explicit:

- `core/hrir.py::get_center_value` starts **k=1**, then retries k=1 only on
  `ValueError`; it never starts at k=3. The helper honors its explicit degree
  and does not turn FITPACK's insufficient-points error into a linear success.
  `get_center_value` returns the negative interpolated gain; its fixture
  reverses that centering sign to compare the interpolation helper itself.
- Both Python peak helpers return zero immediately for wholly empty data.
  P05 explicitly requires returning start. The empty/nonzero-start fixture
  records both results and applies the packet's return-start override.

Finite-input validation is explicit for fallible FIR/SG/spline APIs; all-zero
minimum-phase input is rejected (no invented spectral floor). Peak NaN and
infinity cases follow comparison and normalized NumPy argmax behavior. Odd SG
windows are required by P05 even though recent SciPy also accepts even ones.
Spline construction is a banded collocation solve, O(n*k^2) time/O(n*k) memory,
with O(log(n)+k^2) scalar de Boor evaluation and polynomial extrapolation.
An 8000-point property test checks knots; the negative natural-cubic fixture
fails the FITPACK budget both inside the domain and during extrapolation.

| P05 family | Required budget | Observed maximum |
|---|---|---|
| firwin2 coefficients | atol 1e-11*max(1,peak), rtol 1e-9 | 1.813e-13 |
| firwin2 spectrum | 1e-5 dB above relative -100 dB | 1.021e-12 dB |
| minimum_phase spectrum | 1e-5 dB above relative -100 dB | 9.856e-13 dB |
| homomorphic log/lifter | 1e-9 log diagnostic; exact lifter | 9.859e-14; exact |
| savgol single / 1000 passes | atol 1e-10 / 1e-7 | 5.879e-13 / 5.320e-11 |
| peaks / first peak / window size / default nfft | exact | 0 |
| FITPACK k1 / k2 / k3 | atol 1e-9, rtol 1e-11 | 3.553e-15 / 3.553e-15 / 1.333e-14 |
| log-axis Harman/center | atol 1e-9, rtol 1e-11 | 1.457e-9 (combined budget passes) |

Spectral tests compare the entire final FIR at twice n_fft. Below the -100 dB
floor they enforce complex absolute error <= 1e-9*max(1,reference spectral
peak). The log-axis maximum occurs in cubic extrapolation, where the relative
term in the specified combined budget matters. No budget was widened.

```text
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --test golden_batch2 -- --nocapture --test-threads=1
cargo test -p impulcifer-dsp --lib -- --nocapture
cargo test -p impulcifer-dsp --test properties_batch2 -- --nocapture
```

## P06 resampling and spectrogram goldens

The exporter adds 55 `p06_*` files (full arrays, not just endpoints). The
installed oracle is nnresample **0.2.4.1**, SciPy **1.18.1**, NumPy **2.5.3**,
Python **3.14.5** (values identical under 3.13.3 / 2.4.6 / 1.18.0). `IMPULCIFER_GOLDEN_BATCH=p06` selects only this family;
without that variable all existing families still run. P06 binaries use P05's
headerless little-endian f64 format and descriptor fields `file`, `length`,
`sha256`, `first`, `last`. P06 binaries are not capped at 200 kB: a complete
32001-tap design requires 256008 bytes. JSON files remain below 200 kB.

The four successful designs include both directions of 44.1/48 and 48/96 kHz.
Metadata includes reduced factors, N=32001, beta=**5.65326**, cutoff, search
bounds, local argmin and absolute null bin. Both symmetric Kaiser-sinc designs
use unity-DC scaling. The initial design is zero-padded to 2^19; the null
frequency divides by H=262145, not 262144. The default attenuation is 60 dB;
`compute_filt`'s direct beta=5 default is not used. No optional df-to-N behavior
is implemented or changed. The composition cache is keyed by reduced ratio
and locked during first design, ensuring one successful design/FFT per ratio.
Cloning cached f64 taps does not alter numerical values.

Explicit-tap polyphase tests compare all output samples with SciPy independently
of Rust's designer, then separately compare the full nnresample composition.
The kernel computes retained output phases directly in ascending input-sample
order, O(n_out*ceil(N/up)) time and O(N+n_out) storage. No zero-inserted input
is allocated. Zero pre/post taps are implicit; minimal post-pad length is
solved from SciPy's output-length inequality. 48k -> 44.1k has reduced factors
147/160, half_len=16000, pre_pad=160, pre_remove=101. Short one-tap, even-tap,
gcd, empty-input, and same-rate cases exercise cropping and post-padding.

Spectrogram parameters are captured from the actual
`ImpulseResponsePlotter.plot_spectrogram` function (lines 114-222), stopping
at its SciPy call, not from a second transcription. Default f_res=10 and
n_segments=200 cases include lengths 0,1,100,4800,48000,295000, plus zero
segments, negative computed overlap, rounded-zero and ties-to-even cases.
The fixed min_time_segments=3 formula is (2*n)//4, with its n=1 fallback.
PSD fixtures cover a 0.5-second windowed 1 kHz sine and seeded noise (606),
4800-point periodic Hann with overlap 2400, plus odd and singleton windows.
They retain full frequency-major row-major Sxx, shape, frequencies and times.
The implementation preserves constant detrend per segment, density scaling,
one-sided interior doubling, no boundary extension and no tail padding.

### Packet discrepancies established by the installed oracle

- Same-rate `nnresample.resample` 0.2.4.1 **raises ValueError** at the initial
  firwin cutoff=1. `nnresample_design` and `nnresample` preserve that error;
  `p06_design_48000_48000.json` records the exception and null taps/cutoff/bin.
  There is deliberately no fabricated same-rate tap binary. In contrast,
  SciPy resample_poly returns a copy before validating taps. Its copy property
  has zero group-delay alignment. The requested same-rate nnresample delayed
  copy cannot be tested as a success without changing the pinned algorithm.
- The requested DC per-sample relative deviation <1e-6 is **false in Python**.
  With 4800 unit samples, 48k -> 44.1k and 200 samples trimmed at either end,
  the oracle deviation is 1.531320028669292e-5. `p06_dc.json` freezes the full
  output and this negative result. Rust reproduces it within 1.533e-14; the
  interior mean error is 3.212e-9 and same-phase periodic error is exactly zero.
  Those passing mean/phase properties do not replace a claim of samplewise
  unity, and no coefficients or golden tolerances were changed to force it.
- `data/demo/FL.wav` does not exist. The actual file is `data/demo/FL,FR.wav`;
  its first soundfile track, cropped to 8192 samples, is used. The descriptor
  records that source path and SHA-256.
- The existing P07 float-WAV exporter was not deterministic: libsndfile writes
  wall-clock time in PEAK chunks. Exporting all families changed four P07 JSON
  inputs only at that timestamp. The exporter now preserves an existing
  fixture's PEAK timestamp, retaining its original bytes. Those accidental
  timestamp changes were undone only after verifying every other byte matched.

Invalid fs, zero/oversized explicit spectrogram windows and invalid factors
return DspError. Explicit Hann arrays do not shrink to short input. Parameter
selection returns None for invalid numeric domains (Python raises). Finite f64
resampling is the pinned parity contract; nonfinite data is not rejected and
uses arithmetic propagation, but implicit-zero optimization does not promise
SciPy's NaN masks for products involving padded zeros. Spectrogram NaNs spread
through their containing segment. No unsafe code, dependencies, SIMD intrinsics,
front-end changes, or batch 1/2 behavior changes were added.

| Function / fixture | Required tolerance | Measured maximum absolute error |
|---|---|---|
| firwin_lowpass + nnresample_design / p06_design_*, p06_taps_* | rtol 1e-9, atol 1e-11*max(1,peak) | 2.4424906541753444e-15 |
| design cutoff/beta / p06_design_* | atol 1e-12 | 0 |
| design argmin / p06_design_* | exact integer | 0 |
| resample_poly impulse/sine / p06_poly_* | atol 1e-10 | 0 |
| resample_poly demo / p06_poly_demo_* | atol 1e-9*max(abs(input)) | 0 |
| nnresample impulse/sine / p06_poly_* | atol 1e-10 | 1.4876988529977098e-14 |
| nnresample demo / p06_poly_demo_* | atol 1e-9*max(abs(input)) | 2.2768245622195593e-17 |
| nnresample DC / p06_dc | atol 1e-10 against Python | 1.532107773982716e-14 |
| resample_poly edges / p06_edge_poly_* | atol 1e-14 | 0 |
| spectrogram_params / p06_params | exact integer/None | 0 |
| spectrogram power / p06_spectrogram_* | rtol 1e-9, atol 1e-14 | 3.469446951953614e-17 |
| spectrogram frequencies/times / p06_spectrogram_* | atol 1e-12 | 0 |

The release timing example includes both FIR designs, FFT planning, allocation,
and the 2^19 real FFT, without using the composition cache. Three runs per
ratio measured 9.3774-11.1396 ms on the worker's Windows machine, all below
200 ms. Ratios 48000/44100, 44100/48000, 96000/48000, 48000/96000 respectively
measured [11.1396,10.8861,10.1967], [10.3945,10.8405,9.3774],
[10.1266,9.5039,10.0905], [9.5943,10.0024,10.5360] ms.

Run in the foreground from the repository root:

```text
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt -p impulcifer-dsp -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --test golden_batch3 --test properties_batch3 -- --nocapture --test-threads=1
cargo run -p impulcifer-dsp --release --example p06_design_timing
```

Official references consulted alongside installed source:
[firwin](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.firwin.html),
[resample_poly](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.resample_poly.html),
[spectrogram](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.spectrogram.html).
The source, not the rolling reference page, pins the SciPy version.

## P07 audio I/O goldens

The same no-argument exporter command also writes 28 `p07_*.json` fixtures.
P07 does not change existing P03/P05 fixture content. Its observed environment
is soundfile 0.14.0 / libsndfile 1.2.2, Python 3.14.5, NumPy 2.5.3 (byte-identical fixtures under soundfile 0.13.1, Python 3.13.3, NumPy 2.4.6).

### PCM conversion and header evidence

The packet's claim of nearest rounding at each target bit depth is not what
this oracle does. For finite normalized doubles, multiply by 2147483648,
round to nearest with ties to even, saturate to [-2147483648, 2147483647], then
arithmetic-right-shift by 16/8/0 bits for PCM_16/24/32. Reading divides the
signed stored integer by 2^(bits-1). Examples in units of the target LSB:

| Input | PCM_16 | PCM_24 | PCM_32 |
|---|---|---|---|
| -2.5, -1.5, -0.5 | -3, -2, -1 | -3, -2, -1 | -2, -2, 0 |
| 0.5, 1.5, 2.5 | 0, 1, 2 | 0, 1, 2 | 0, 2, 2 |

For -1.1 and -1.0, the stored integers are respectively -32768, -8388608,
-2147483648 at the three depths. For +1.0 and +1.1, they are 32767, 8388607,
2147483647. Boundary-neighbor and i32-halfway fixtures distinguish this rule
from simply flooring at the target bit depth. These observations also match
libsndfile's normalized clipping converters `d2les_clip_array`,
`d2let_clip_array`, and `d2lei_clip_array` in
[libsndfile 1.2.2 pcm.c](https://raw.githubusercontent.com/libsndfile/libsndfile/1.2.2/src/pcm.c).
Nonfinite write inputs are explicitly rejected, rather than depending on
platform-specific float-to-integer NaN conversion; the infallible in-memory
round-trip asserts that samples are finite.

`core.audio_io.write_wav` writes a plain PCM RIFF header even for three tracks.
P07 instead requires extensible DIRECTOUT above two channels. Each writer
fixture therefore keeps both unmodified Python WAV bytes and supplemental
soundfile `format="WAVEX"` bytes (base64). Tests compare the default WAV's data
payload exactly and the WAVEX file byte-for-byte, including its `fact` chunk,
zero channel mask, GUID and padding. Mono/stereo default WAV files are compared
byte-for-byte as well. There is no public float writer; ffmpeg's pcm_f32le WAV
is only a decode intermediate, matching the Python conversion arguments.

The reader fixtures include all 15 combinations of RIFF/WAVEX/RF64 and
PCM_16/24/32/FLOAT/DOUBLE. Tests reject every physical truncation of those files,
validate declared RIFF/chunk/data limits and RF64 ds64 sizes/tables before
sample allocation, accept arbitrary extensible masks, and exercise odd padding,
unknown/reordered chunks, 30/32/40 tracks and invalid arguments. The bundled
mono sweep is checked by first/last 64 samples and SHA-256 of its entire LE-f64
track, not merely by endpoints. The test-only SHA-256 helper has empty, abc and
million-a known-answer checks.

### Process and sweep contracts

Discovery adds the packet's app-local priority (current executable directory,
its `ffmpeg/bin`, then `bin`) before Python's PATH/common-location search.
The current Python oracle itself has no app-local branch. FFmpeg version 4.0
or later is required; Windows common locations include WinGet package folders.
Discovery only prints installation instructions when requested. Processes use
absolute executables and argument arrays, null stdin, drained stdout/stderr,
and CREATE_NO_WINDOW on Windows. Nonzero exit errors retain the executable,
ExitStatus and final 20 stderr lines. Probe failures remain typed errors instead
of Python's broad exception-to-False behavior. There is currently no process
timeout; unlike the oracle's 10/60-second subprocess timeouts, Command::output
waits for process exit.

The Unix fake .sh tests exercise decode errors, codec/profile JSON and exact
decode arguments. Windows .cmd/.bat programs are rejected because they require
a shell; its error test runs the native test executable with invalid FFmpeg
arguments and checks the resulting stderr. Actual FFmpeg decoding and Unix-only
process tests need their respective environments; they are not inferred from
Windows passing tests.

`infer_sweep_segments(play_file: &str, total_duration: f64)` is implemented:
`core.recording_progress` is pure and does not import the estimator. Segment
metadata, case-insensitive partial/display-name matching, clipping to recording
duration, and zero-duration fallback are compared with a Python dump. Filename
formatting uses the estimator's .2f/.2f/.0f precision.

Optional ordered output writers are not implemented. `output.hrir_wav`,
`output.hesuvi_wav` and `output.responses_wav` remain planned; generic PCM I/O
alone does not prove those output contracts. Only the required TrueHD 11/13
track-order constants were appended to types and verified against Python JSON.
No dependency or Cargo.lock changes were needed. RF64 is read-only; writers
reject files exceeding RIFF's 32-bit size limit before opening the output.

Required P07 gates (foreground):

```text
python E:/Impulcifer/tests/migration/export_goldens.py
cargo fmt -p impulcifer-io -p impulcifer-types -- --check
cargo clippy -p impulcifer-io -p impulcifer-types --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io -p impulcifer-types
cargo test -p impulcifer-policy
```
