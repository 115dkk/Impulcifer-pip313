# P08 (ASTRA): `impulcifer-dsp::{estimator, ir, decay, hrir}` — the 2.x measurement objects, file-free

You are extending the Impulcifer 3.x Rust workspace at `E:/Impulcifer`. Read first, in this order: `E:/Impulcifer/docs/rust/survey/pipeline-object-model.md` (the design input: every method with line ranges and formulas), the 2.x oracle files it cites (`core/impulse_response_estimator.py`, `core/impulse_response.py`, `core/decay.py`, `core/hrir.py`, `core/brir_layout.py`, `core/constants.py`, `core/sweep_signal.py` lines 130 to 205, `core/audio_io.py` lines 18 to 118), the finished primitives in `E:/Impulcifer/crates/impulcifer-dsp/src/{conv,fft,windows,peaks,stats,resample}.rs` (use them; do not reimplement), `E:/Impulcifer/crates/impulcifer-types/src/constants.rs`, and the golden conventions in `E:/Impulcifer/tests/migration/README.md` and `E:/Impulcifer/tests/migration/export_goldens.py` (read only).

Run every command in the foreground. Never use background execution.

Other workers may be editing `crates/impulcifer-dsp/src/fr.rs`, `tests/migration/export_goldens.py`, `tests/migration/README.md` and `features.toml`. You must not touch those files. The modules `estimator`, `ir`, `decay`, `hrir` are already declared in `crates/impulcifer-dsp/src/lib.rs` as stub files; replace their contents.

## Design rule: no file I/O in `impulcifer-dsp`

`impulcifer-dsp` depends on `impulcifer-types` only (docs/rust/ARCHITECTURE.md section 2). Everything that 2.x does with files (`read_wav`, `write_wav`, `from_wav`, `open_recording(file_path)`, the `ICHL` chunk) is split: the DSP object takes or returns sample arrays, and a later packet in the service layer does the reading and writing. Method names keep the Python names with a `_samples`/`_tracks` suffix where the Python took a path.

## Scope

### `crates/impulcifer-types/src/constants.rs` (append-only, exact copies of `core/constants.py`, each pinned by the P08 golden `p08_constants.json` the way P07 pinned the TrueHD orders)
`HESUVI_TRACK_ORDER: [&str; 30]`, `HEXADECAGONAL_TRACK_ORDER: [&str; 32]`, `LEFT_SIDE_SPEAKERS`, `RIGHT_SIDE_SPEAKERS`, `CENTER_SPEAKERS`, `IPSILATERAL_PAIRS: [(&str, &str); 8]`, `SPEAKER_DELAYS` (15 zeros, keep them as an array anyway), `SEQUENCE_TRACK_ORDERS` (the four named layouts from `impulse_response_estimator.py:18-23`), `pub fn speaker_side(name: &str) -> Side` with `pub enum Side { Left, Right, Center }`, `pub fn track_name(speaker, side) -> String`, `pub fn base_channel_count(order: &[&str]) -> usize` (14 for HeSuVi, 16 for hexadecagonal, `order.len()` otherwise; read `core/brir_layout.py:20-27`). Only add what is absent.

### `estimator.rs`
```rust
pub struct SweepEstimator { pub fs: u32, pub low: f64, pub high: f64, pub n_octaves: f64,
    pub test_signal: Vec<f64>, pub duration: f64, pub inverse_filter: Vec<f64> }
```
- `pub fn new(min_duration: f64, fs: u32) -> Result<Self, DspError>`: `impulse_response_estimator.py:33-50` (`high = fs/2`, `low = 5`, `n_octaves = ceil(log2(high/low))`, `low = high / 2^n_octaves`).
- `pub fn generate_test_signal(fs, n_octaves, min_duration, fade_in: Option<f64>, fade_out: Option<f64>) -> Vec<f64>`: lines 86 to 147, formulas `M`, `L`, `N = round(L)`, `freqs = pi/2^P * L/ln(2^P) * exp(arange(N)/N * ln(2^P))`, `sin`, the half-octave Hann fade-in (`2*int(fs*seconds_per_octave*fade_in)` rounded up to even, `hann(n)[:n/2]`), no fade-out by default. Use `windows::hann(n, true)` (scipy `hann` symmetric). Reproduce numpy's `np.round` (half to even) for `N`.
- `pub fn generate_inverse_filter(test_signal, n_octaves) -> Vec<f64>`: lines 73 to 84: `flip(x) * (2^(P/N))^(-i) * P*ln2/(1 - 2^-P)`, then divide by `|fft(convolve(inverse, x, mode=full))[round(nfft/4)]|` where `nfft = 2N - 1` and `round` is Python's half-to-even on `nfft/4`. Use `conv::convolve` and `fft::fft`.
- `pub fn estimate(&self, recording: &[f64]) -> Vec<f64>`: `conv::convolve(recording, inverse_filter, Mode::Same)`.
- `pub fn sweep_sequence(&self, speakers: &[&str], tracks: &str) -> Result<Vec<Vec<f64>>, DspError>`: lines 153 to 232 including the `stereo` positional rule and `mono` forcing `FL`; reproduce the non-rejection of duplicate speakers (document it).
- `pub fn from_samples(fs: u32, data: &[f64]) -> Result<Self, DspError>`: the `from_wav` repair logic (lines 234 to 262) on an already-read first track: `duration = (len - 1)/fs`, construct, adopt the samples when the length differs (regenerate the inverse filter, `duration = len/fs`), or when `max|test_signal - data| > 1e-4` (regenerate; duration not updated).
- `pub fn file_name(&self, bit_depth: u16) -> String`: `f'{duration:.2f}s-{fs}Hz-{bits}bit-{low:.2f}Hz-{high:.0f}Hz'` with Python's `.2f`/`.0f` rounding (`format!("{:.2}")` matches for these values; pin against the golden, including the bundled name `6.15s-48000Hz-32bit-2.93Hz-24000Hz`).

### `ir.rs`
```rust
#[derive(Clone, Debug)]
pub struct ImpulseResponse { pub data: Vec<f64>, pub fs: u32, pub recording: Option<Vec<f64>> }
```
Methods from `impulse_response.py`: `len`, `duration`, `peak_index(start, end, peak_height)` delegating to `peaks::first_peak_index`, `crop_head(head_ms)`, `shift(samples: i64)` (exact `concat`/`pad` semantics), `equalize(fir)` (`conv::convolve(.., Mode::Full)`), `resample(fs)` (`resample::nnresample(&data, fs, self.fs)`; if that function is not yet in the tree, stop and report instead of writing your own), `convolve(x)`, `magnitude_response(&self) -> (Vec<f64>, Vec<f64>)` (`audio_io.magnitude_response`: `fft::magnitude_response` plus the `f = arange(half) * fs/nfft` axis), `decay_params`, `decay_times`, `decay_adjustment_params`, `adjust_decay(target_seconds)`. No `frequency_response` here (it needs the `fr` module from another packet; it will be added later as a free function).

### `decay.rs`
`pub struct DecayParams { pub peak_index: usize, pub knee_index: usize, pub noise_floor_db: f64, pub window_size: usize }`, `pub fn decay_params(data: &[f64], fs: u32) -> DecayParams` (lines 44 to 260, every constant from the survey: 2 s analysis window, `wd = 0.03`, 10 dB slope end, 3 windows per 10 dB, 5 iterations, `-5` dB knee margin, `0.1*total` floor start, `8` and `20` dB headrooms; `stats::linregress`), `pub struct DecayTimes { pub edt: Option<f64>, pub rt20: Option<f64>, pub rt30: Option<f64>, pub rt60: Option<f64> }`, `pub fn decay_times(data, fs, params: Option<&DecayParams>) -> DecayTimes` (lines 263 to 352, Schroeder integral, moving average with `stats::running_mean`, the alignment offset, the 10 dB headroom rule), `pub struct DecayAdjustment { pub window_start: usize, pub half_window: usize, pub knee_point_index: usize, pub window_level: f64 }`, `pub fn decay_adjustment_params(data, fs, target_seconds) -> Option<DecayAdjustment>` (355 to 380), `pub fn apply_decay_window(data: &mut [f64], params: Option<&DecayAdjustment>)` (383 to 403, `windows::hann`). Integer arithmetic must follow Python (`int()` truncation toward zero, `//` floor, `round` half-even); write a private helper for each and test them.

### `hrir.rs`
```rust
pub struct SpeakerIrs { pub speaker: String, pub left: Option<ImpulseResponse>, pub right: Option<ImpulseResponse> }
pub struct Hrir { pub fs: u32, pub speakers: Vec<SpeakerIrs> }   // insertion order like the Python dict
```
with `get(&self, speaker) -> Option<&SpeakerIrs>`, `get_mut`, `pair(&self, speaker) -> Option<(&ImpulseResponse, &ImpulseResponse)>`, `for_each_ir`, `subset(&self, speakers) -> Hrir` (clone), and the methods:
- `pub fn ingest_recording(estimator: &SweepEstimator, expected_fs: u32, fs: u32, tracks: &[Vec<f64>], speakers: &[&str], side: Option<Side>, silence_length: f64) -> Result<Vec<SpeakerIrs>, DspError>`: `core/hrir.py:77-364` with every rule in the survey (integral silence, `tracks_k`, `n_columns = round(len(speakers) / (n_tracks // tracks_k))` using Python's half-even `round`, the column split, the two fallbacks, the `i // 2` quirk, unknown speakers skipped). Log strings are not needed; return a `DspError::InvalidArgument` with the 2.x message when no column survives.
- `pub fn open_recording_samples(&mut self, estimator, fs, tracks, speakers, side, silence_length) -> Result<(), DspError>`: the `HRIR.open_recording` merge (`setdefault(...).update(...)`, keep speaker insertion order).
- `pub fn stack_tracks(&self, track_order: &[&str], trim_extensions: bool) -> Result<Vec<Vec<f64>>, DspError>`: the pure part of `write_wav` (427 to 474): flatten to `track_name`, zeros of the reference length for missing tracks, `InvalidArgument` when empty, then `trim_silent_extensions` (`core/brir_layout.py:29-36`: pop trailing all-zero pairs down to `base_channel_count(order)`). Also `pub fn compact_tracks(tracks: &[Vec<f64>], order: &[&str]) -> (Vec<Vec<f64>>, Vec<String>)` (`brir_layout.py:38-44`, drop every all-zero row and return the surviving names). Writing the WAV and the `ICHL` chunk is the caller's job.
- `pub fn normalize(&mut self, peak_target: Option<f64>, avg_target: Option<f64>) -> Result<f64, DspError>` (476 to 565, sums per ear with zero padding, `magnitude_response`, the 80 to 6000 Hz band for `avg_target`, `InvalidArgument` for both/neither, gain applied in place; sum in speaker insertion order).
- `pub fn crop_heads(&mut self, head_ms: f64) -> Result<(), DspError>` (567 to 631, same crop index for both ears, rising half-Hann fade of `head` samples).
- `pub fn crop_tails(&mut self, estimator: &SweepEstimator) -> Result<usize, DspError>` (633 to 672: `decay_params` knee per IR with the `len` fallback, `seconds_per_octave`, `fade_out = 2*int(fs*seconds_per_octave/24)`, `hann(fade_out)[fade_out/2..]`, `fft::next_fast_len_legacy(max(tails))`, `tail = min(min(lengths), fft_len)`).
- `pub fn equalize(&mut self, left_fir: &[f64], right_fir: &[f64])` and `pub fn equalize_same(&mut self, fir: &[f64])` (877 to 907).
- `pub fn resample(&mut self, fs: u32) -> Result<(), DspError>` (909 to 938; sequential is fine, or `rayon` over speakers since the outputs are independent).
- `pub fn align_ipsilateral_all(&mut self, pairs: &[(&str, &str)], segment_ms: f64)` (940 to 974, `conv::correlate(.., Mode::Full)`, lag sign rules for self-pairs and cross-pairs).
- `pub fn align_onset_groups_peak_leftref(&mut self, groups: Option<&[&[&str]]>) -> Result<(), DspError>` (976 to 1020, default groups, `RuntimeError` when `FL` is missing becomes `InvalidArgument`).
- `pub struct ReflectionLevels { pub early_db: f64, pub late_db: f64 }` and `pub fn calculate_reflection_levels(&self, ...) -> Vec<(String, Side, ReflectionLevels)>` (1022 to 1109).
No `correct_channel_balance`, `correct_microphone_deviation` or `channel_balance_firs` (they need `fr`; later packet).

### Golden exporter and tests
Write `E:/Impulcifer/tests/migration/export_goldens_brir.py` (standalone; may `import export_goldens` for the helpers and output directory; must not modify it). Fixtures `tests/migration/goldens/p08_*.json` plus `.f64` files; document the format and the demo-stage protocol in `tests/migration/README-brir.md` (new file). Work on a temporary copy of `E:/Impulcifer/data/demo` so the repository directory stays untouched (`git status --short data/demo` must be empty afterwards).

Fixtures (all from the 2.x code, never from your own transcription):
- `p08_constants.json`: every constant listed above dumped from `core.constants` and `core.impulse_response_estimator.SEQUENCE_TRACK_ORDERS`.
- Estimator: `ImpulseResponseEstimator(min_duration=5.0, fs=48000)` and `(min_duration=(N-1)/44100, fs=44100)` for the bundled 44.1 kHz sweep if one exists under `data/`; store `low, high, n_octaves, duration, len`, the first/last 256 samples and SHA-256 of `test_signal` and `inverse_filter` (full arrays as `.f64`; the 48 kHz test signal is about 2.4 MB, that is acceptable once), `file_name(32)`, and `estimate` of a synthetic recording (the test signal delayed by 1000 samples plus a -20 dB echo at 3000 samples, `mode='same'`, first/last 256 + SHA + full `.f64`). Also `from_wav("E:/Impulcifer/data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")` recording which repair branch ran and the resulting `duration`, and `sweep_sequence(["FL","FR"], "stereo")` and `(["FL"], "mono")` and `(["FL","FR","FC"], "7.1")` as shapes plus per-track SHA/first/last.
- Impulse response: `shift(+37)`, `shift(-37)`, `crop_head(1)`, `equalize` with a short FIR, `magnitude_response` on the demo `FL` left IR after the crop stage (below).
- Decay: `decay_params`, `decay_times`, `decay_adjustment_params(target=0.3)` and `apply_decay_window` for six demo IRs (`FL`/`FR`/`FC` both ears) after the crop stage, plus the short-input guard (`len < 10`) and a synthetic exponential decay with known RT60 (`exp(-6.9078*t/0.5)` times noise, seed 808) where you also record the intermediate `windows`, `t_windows`, `noise_floor` of the first pass.
- Demo stage protocol (the acceptance path for M2): with `estimator = ImpulseResponseEstimator.from_wav(<bundled sweep>)`, `hrir = HRIR(estimator)`, open every `<SPEAKERLIST>.wav` in the demo copy in sorted file-name order (record the order), then run `crop_heads(head_ms=1)`, `align_ipsilateral_all(speaker_pairs=list(IPSILATERAL_PAIRS), segment_ms=30)`, `align_onset_groups_peak_leftref()`, `crop_tails()`, `normalize(peak_target=-0.1)`. After **each** stage store, per track (speaker, side): length, `peak_index()`, max abs, argmax, RMS, first/last 256 samples, SHA-256 of the LE-f64 bytes; after `open` and after `crop_tails` additionally store the full `FL` left and right arrays as `.f64`. Store the `normalize` gain and the `crop_tails` return value. Also store `stack_tracks(HESUVI_TRACK_ORDER, trim_extensions=True)` and `(HEXADECAGONAL_TRACK_ORDER, True)` as the list of track names in output order and per-track SHA, and `compact_tracks` names.

Rust tests in `crates/impulcifer-dsp/tests/golden_brir_objects.rs`: `golden_constants_match_python`, `golden_sweep_estimator_matches_python` (test signal and inverse filter `atol 1e-12` on the f64 arrays; additionally report the count of PCM_32 LSB mismatches after `impulcifer-io`-style quantisation, computed inline as `round_ties_even(x * 2^31)` saturated, against the Python test signal; it must be reported, and the assertion threshold is `<= 8` mismatches out of 295,000), `golden_estimate_matches_scipy` (`atol 1e-9 * peak`), `golden_sweep_sequence_matches_python` (exact shapes, arrays `atol 1e-12`), `golden_from_samples_repair_matches_python`, `golden_file_name_matches_python`, `golden_shift_crop_equalize_match_python` (exact for shift/crop, `atol 1e-12` for equalize), `golden_magnitude_response_matches_python` (`atol 1e-9` dB, `-inf` handled), `golden_decay_params_match_python` (integers exact, dB `atol 1e-9`), `golden_decay_times_match_python` (`atol 1e-9` s or both `None`), `golden_decay_adjustment_matches_python` (exact ints, `atol 1e-9`, `apply_decay_window` `atol 1e-12`), `golden_demo_stages_match_python` (per stage and track: length exact, `peak_index` exact, argmax exact, max abs and RMS `rtol 1e-9`, first/last 256 samples `atol 1e-9 * max_abs`; the full `FL` arrays after `open` and after `crop_tails` `atol 1e-9 * max_abs`; the normalize gain `atol 1e-9`), `golden_stack_tracks_match_python` (names exact; per-track SHA may differ by rounding, so compare the arrays you already verified instead of the hash). The demo test reads the recordings with `impulcifer-io::read_wav` (a dev-dependency you may add to `crates/impulcifer-dsp/Cargo.toml` under `[dev-dependencies]` only; not a normal dependency).

`crates/impulcifer-dsp/tests/properties_brir_objects.rs`: `python_rounding_helpers_match_cpython` (int truncation, floor division on negatives, half-even round), `shift_is_length_preserving_and_invertible_for_zeros`, `crop_heads_keeps_itd`, `normalize_hits_peak_target`, `align_ipsilateral_self_pair_converges` (a right ear delayed by 7 samples is realigned to lag 0), `stack_tracks_trims_only_trailing_pairs`.

Do not edit `features.toml`; report the test names and I will register the entries.

## Allowed files
`E:/Impulcifer/crates/impulcifer-dsp/src/{estimator,ir,decay,hrir}.rs`, `E:/Impulcifer/crates/impulcifer-dsp/tests/golden_brir_objects.rs`, `E:/Impulcifer/crates/impulcifer-dsp/tests/properties_brir_objects.rs`, `E:/Impulcifer/crates/impulcifer-dsp/Cargo.toml` (`[dev-dependencies]` only), `E:/Impulcifer/crates/impulcifer-types/src/constants.rs` (append-only), `E:/Impulcifer/tests/migration/export_goldens_brir.py`, `E:/Impulcifer/tests/migration/goldens/p08_*`, `E:/Impulcifer/tests/migration/README-brir.md`. Nothing else.

## Hard rules
- `#![forbid(unsafe_code)]`; no unsafe; f64 only; no file I/O in `src/` (tests may read files).
- Reproduce 2.x behaviour including quirks (the `i // 2` mapping, the never-rejected duplicate speakers, the `from_wav` duration not updated on the value-mismatch branch, integer truncations). Do not "improve"; list every quirk you reproduced in the report.
- Every function has a doc comment naming the Python function, its line range and the fixture that pins it.
- Golden files above 200 kB must be `.f64`; total new fixture size under 12 MB.

## Verification (foreground, paste output)
```
python E:/Impulcifer/tests/migration/export_goldens_brir.py
git -C E:/Impulcifer status --short data/demo
cargo fmt -p impulcifer-dsp -p impulcifer-types -- --check
cargo clippy -p impulcifer-dsp -p impulcifer-types --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-dsp --test golden_brir_objects --test properties_brir_objects
cargo test -p impulcifer-types
cargo test -p impulcifer-policy
```

## Report format
(1) files changed; (2) verification outputs verbatim; (3) table function → fixture → tolerance → max observed error, including the PCM_32 mismatch count of the generated sweep; (4) quirks reproduced and anything you could not reproduce; (5) the exact test function names for the registry; (6) anything undone. Do not end your turn before the commands complete.
