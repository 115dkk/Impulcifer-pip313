# AutoEQ `FrequencyResponse`: the subset the 2.x core uses (survey for the Rust port)

Source: `autoeq/frequency_response.py` (1620 lines, pinned at AutoEQ 1.2.5 math; see the comment at lines 291 to 294). Companions: `autoeq/constants.py`, `autoeq/biquad.py`. Oracle environment at the time of the survey (2026-09-07): scipy 1.18.0, numpy 2.4.6.

This document is the design input for the `impulcifer-dsp::fr` packet. It records every method the application core calls, the exact call-site arguments, the algorithm of each method with line ranges, and the quirks that must be reproduced rather than fixed.

## Constants

All defaults come from `autoeq/constants.py:5-30`, imported at `frequency_response.py:17-23`.

| Constant | Value | Used by |
|---|---|---|
| `DEFAULT_F_MIN` | `20` | `generate_frequencies`, `interpolate`, `_tilt` |
| `DEFAULT_F_MAX` | `20000` | `generate_frequencies`, `interpolate`, `_tilt` |
| `DEFAULT_STEP` | `1.01` | `generate_frequencies`, `interpolate` |
| `DEFAULT_MAX_GAIN` | `6.0` | `equalize(max_gain=)` |
| `DEFAULT_TREBLE_F_LOWER` | `6000.0` | `equalize(treble_f_lower=)` |
| `DEFAULT_TREBLE_F_UPPER` | `8000.0` | `equalize(treble_f_upper=)` |
| `DEFAULT_TREBLE_MAX_GAIN` | `6.0` | `equalize(treble_max_gain=)`, never overridden by the app |
| `DEFAULT_TREBLE_GAIN_K` | `1.0` | `equalize(treble_gain_k=)`, never overridden |
| `DEFAULT_SMOOTHING_WINDOW_SIZE` | `1/3` | `smoothen`, `smoothen_fractional_octave`, `_smoothen_fractional_octave` |
| `DEFAULT_SMOOTHING_ITERATIONS` | `1` | same |
| `DEFAULT_TREBLE_SMOOTHING_F_LOWER` | `100.0` | same |
| `DEFAULT_TREBLE_SMOOTHING_F_UPPER` | `10000.0` | same |
| `DEFAULT_TREBLE_SMOOTHING_WINDOW_SIZE` | `1/3` | `smoothen`, `smoothen_fractional_octave` |
| `DEFAULT_TREBLE_SMOOTHING_ITERATIONS` | `1` | `smoothen_fractional_octave` |
| `DEFAULT_TILT` | `0.0` | `_tilt` |
| `DEFAULT_FS` | `44100` | `minimum_phase_impulse_response`, and the `create_target` biquad design rate |
| `DEFAULT_F_RES` | `10` | `minimum_phase_impulse_response` |
| `DEFAULT_BASS_BOOST_GAIN` | `0.0` | `create_target`, `compensate` |
| `DEFAULT_BASS_BOOST_FC` | `105.0` | `create_target`, `compensate` |
| `DEFAULT_BASS_BOOST_Q` | `0.71` | `create_target`, `compensate` |
| `DEFAULT_GRAPHIC_EQ_STEP` | `1.0563` | `eqapo_graphic_eq` only, not on any app path |

The app overrides the bass-boost Q: `ProcessingConfig.bass_boost_q` defaults to `0.76` (`core/pipeline.py:224-227`), not AutoEQ's `0.71`. `bass_boost_gain=0.0`, `bass_boost_fc=105`, `tilt=0.0`.

Dead for the port: `DEFAULT_GRAPHIC_EQ_STEP`, the Harman preference frequency lists, `DEFAULT_BIT_DEPTH`, `DEFAULT_PHASE`.

## Call sites

`impulcifer.py` and `application/` have no `FrequencyResponse` usage; everything is in `core/`. **D** = executed on a default `ProcessingConfig` BRIR run; **O** = optional, gated by the named option.

### Construction

| Site | Kwargs | Path |
|---|---|---|
| `core/impulse_response.py:161,166,181` | guards: `name=..., frequency=frequency, raw=np.zeros_like(frequency)` | D (degenerate inputs) |
| `core/impulse_response.py:186` | `name="Frequency response", frequency=frequency, raw=raw` | D |
| `core/room_correction.py:238-248` | `name='generic_room', frequency=generate_frequencies(f_min=10, f_max=irs[0].fs/2, f_step=1.01), raw=0, error=0, target=target.raw` (scalar `0` exercises the scalar branch of `_init_data`) | O (room correction on and a generic `room.wav` exists) |
| `core/room_correction.py:439` | `name='room-target'` (frequency defaults to 20 Hz to 20 kHz) | D |
| `core/pipeline_stages.py:276-290` | `name, frequency=frequency.copy(), raw=result.left_db, error=-result.left_db` (and right) | O (EqualizerAPO config as eq file) |
| `core/pipeline_stages.py:432-436` | `name="zero", frequency=left.frequency, raw=np.zeros(...)` | D (headphone compensation, requires `headphones.wav`) |
| `core/pipeline_stages.py:487-492` | `name="bass_and_tilt", frequency=generate_frequencies(f_min=10, f_max=estimator.fs/2, f_step=1.01)` | D (`_stage_target` is unconditional) |
| `core/parallel_workers.py:101-105` | `name=f'{speaker}-{side} eq', frequency=common_freq.copy(), raw=0, error=0` | D (`_stage_equalize`) |
| `core/hrir.py:698-702` | `name="trend", frequency=left_fr.frequency, raw=left_fr.raw - right_fr.raw` | O (`--channel_balance trend`) |
| `core/microphone_deviation_correction.py:223-225, 265-266` | `name, frequency=self.frequency.copy(), raw=...` | O (`--microphone_deviation_correction`, only when headphone compensation is off) |

### `generate_frequencies`

| Site | Kwargs | Path |
|---|---|---|
| `core/impulse_response.py:160,165,180` | `f_step=1.01, f_min=10, f_max=self.fs/2` | D |
| `core/room_correction.py:240-244` | `f_min=10, f_max=irs[0].fs/2, f_step=1.01` | O |
| `core/pipeline_stages.py:238-240, 489-491` | `f_step=1.01, f_min=10, f_max=estimator.fs/2` | O / D |
| `core/pipeline.py:670-672` | `f_step=1.01, f_min=10, f_max=self.estimator.fs/2` | D |
| `core/microphone_deviation_correction.py:96-98` | `f_step=1.01, f_min=20.0, f_max=nyq` | O |
| implicit in `__init__`, `interpolate`, `center` | no kwargs, so `(20, 20000, 1.01)` | D |

### `interpolate`

| Site | Kwargs | Path |
|---|---|---|
| `core/impulse_response.py:187` | `f_step=1.01, f_min=10, f_max=self.fs/2` | D |
| `core/pipeline_stages.py:322, 334` | `f_step=1.01, f_min=10, f_max=estimator.fs/2, pol_order=1` | O |
| `core/room_correction.py:436, 441, 457` | `f_step=1.01, f_min=10, f_max=estimator.fs/2` | O / D / O |
| `frequency_response.py:664` (in `minimum_phase_impulse_response`) | positional `f`, `pol_order=1` | D |
| `frequency_response.py:914` (in `center`) | no kwargs | D |

### `center`

| Site | Kwargs | Path |
|---|---|---|
| `core/pipeline_stages.py:427` | `left.center([100, 10000])`, result assigned to `gain` and added to `right.raw` | D |
| `core/room_correction.py:198, 256` | `fr.center([100, 10000])` | O |
| `core/room_correction.py:437, 458` | `target.center()`, `mic_calibration.center()` (scalar 1000 Hz) | O |
| `core/hrir.py:725` | `ref.center([100, 10000])` | O |
| `frequency_response.py:995` (in `compensate`) | no args (1000 Hz) on the copy of the compensation curve | D |

A read-only twin exists at `core/hrir.py:44-74` (`get_center_value(fr, frequency_range)`), used at `hrir.py:692, 738, 739` and `pipeline_stages.py:453, 454`. It reproduces only the two-frequency branch of `center` without the internal `interpolate()` re-grid (it averages `fr.raw` directly on the caller's grid) and returns `-diff`. The two are not numerically identical; mirror both deliberately.

### `compensate`

| Site | Kwargs | Path |
|---|---|---|
| `core/room_correction.py:204` | `fr.compensate(target_adjusted, min_mean_error=False)` | O |
| `core/room_correction.py:260` | `fr.compensate(target, min_mean_error=True)` | O |
| `core/pipeline_stages.py:437-438` | `left.compensate(zero, min_mean_error=False)`, same for right | D |

`bass_boost_*`, `tilt` and `sound_signature` are never passed here, so they are always `0.0 / 105.0 / 0.71 / None / None`.

### `create_target`

| Site | Kwargs | Path |
|---|---|---|
| `core/pipeline_stages.py:494-499` | `bass_boost_gain=cfg (0.0), bass_boost_fc=cfg (105), bass_boost_q=cfg (0.76), tilt=cfg (0.0)`, result assigned to `target.raw` | D |
| `frequency_response.py:998-1003` (in `compensate`) | all defaults | D |

`tilt=0.0` (not `None`) is passed from the pipeline, so the `_tilt` branch is taken and produces zeros.

### `smoothen` (compat wrapper)

Room correction (`room_correction.py:151, 262, 276, 280, 286, 408`) with `window_size=1/3` or `1/6` and the same `treble_window_size`; mic deviation (`microphone_deviation_correction.py:227-228`) with `1/6`; plotting (`impulse_response_plotter.py:343-348`, `hrir_plotter.py:335-348, 908-922`) with `treble_f_lower=20000` and `treble_f_upper=23999` or `max(20001, int(fs/2 - 1))`. All optional paths.

### `smoothen_fractional_octave`

`core/hrir.py:703-707` (`window_size=2, treble_f_lower=20000, treble_f_upper=int(round(fs/2))`), `hrir.py:720-724` (`window_size=1/3`, same treble bounds), `hrir.py:744-749, 762-764` (`window_size=1/3, treble_f_lower=20000, treble_f_upper=23999`). All `--channel_balance` paths.

### `smoothen_heavy_light`

`core/parallel_workers.py:125` (no arguments): **the smoother on the default BRIR path**. Also `core/hrir.py:729` (`--channel_balance left|right`).

### `equalize` (FrequencyResponse)

| Site | Kwargs | Path |
|---|---|---|
| `core/parallel_workers.py:126` | `max_gain=40, treble_f_lower=10000, treble_f_upper=estimator_fs/2` (`smoothen=True`, `treble_max_gain=6.0`, `treble_gain_k=1.0` by default) | D |
| `core/hrir.py:730` | `max_gain=15, treble_f_lower=20000, treble_f_upper=self.fs/2` | O |
| `core/hrir.py:765-767` | `max_gain=15, treble_f_lower=2000, treble_f_upper=self.fs/2` | O |

Do not confuse with `ImpulseResponse.equalize(fir)` (`core/impulse_response.py:110`, a time-domain full convolution).

### `minimum_phase_impulse_response`

| Site | Kwargs | Path |
|---|---|---|
| `core/parallel_workers.py:129` | `fs=estimator_fs, normalize=False, f_res=5` | D |
| `core/hrir.py:709, 731, 769` | `fs=self.fs, normalize=False` (`f_res=10`) | O |
| `core/microphone_deviation_correction.py:269` | `fs=self.fs, normalize=False` | O |

`normalize=True` is never used by the app.

### `read_from_csv` / `read_csv`

`core/pipeline_stages.py:225` (eq files that are not EqualizerAPO configs), `core/room_correction.py:435` (`room-target.csv`), `core/room_correction.py:456` (mic calibration). All optional.

### `copy`, `reset`

`copy` at `core/room_correction.py:150, 165, 202, 259, 396` (optional paths). `reset` is only called internally by `interpolate` (line 901), `center` (938), `compensate` (1020), `smoothen_fractional_octave` (1153), `smoothen_heavy_light` (1228).

### Attribute reads and writes on the app side

`raw`, `error`, `error_smoothed`, `smoothed`, `target`, `equalization`, `equalized_raw`, `equalized_smoothed`, `frequency`, `name`. The Rust struct needs all of them as `Vec<f64>` fields (empty vector = Python's empty array).

### Not used by the app (omit from the port)

`to_dict`, `write_to_csv`, `eqapo_graphic_eq`, `write_eqapo_graphic_eq`, `_empty_biquad_result`, `_biquad_eq_response`, `_optimize_biquad_filters_scipy`, `optimize_biquad_filters`, `optimize_parametric_eq`, `optimize_fixed_band_eq`, `write_eqapo_parametric_eq`, `_split_path`, `linear_phase_impulse_response`, `write_readme`, `kwarg_defaults`, `plot_graph`, `harman_*_preference_score`, `process`. `find_peaks` and `least_squares` appear only inside the optimizer; this vendored `equalize` (AutoEQ 1.2.5) has no peak/dip protection, no limit curve and no `concha_interference` logic. The only "protection" is the sigmoid-blended max-gain clip plus kink re-splining described below.

## Methods

### `__init__` (`frequency_response.py:42-73`)

```python
def __init__(self, name=None, frequency=None, raw=None, error=None, smoothed=None,
             error_smoothed=None, equalization=None, parametric_eq=None, fixed_band_eq=None,
             equalized_raw=None, equalized_smoothed=None, target=None):
```

1. `if not name: raise TypeError(...)`; `self.name = name.strip()`.
2. `self.frequency = self._init_data(frequency)`; if empty, `self.frequency = self.generate_frequencies()` (20 Hz to 20 kHz, ratio 1.01, 695 points).
3. Every other field through `_init_data` in this order: `raw, smoothed, error, error_smoothed, equalization, parametric_eq, fixed_band_eq, equalized_raw, equalized_smoothed, target`. Order matters because the scalar branch of `_init_data` multiplies by `self.frequency.shape`.
4. `self._sort()`.

### `copy` (`75-89`)

New instance with `name = self.name + '_copy'` when `name is None`. Every array goes through `_init_data` again, which for a float array without NaN is a fresh copy (no aliasing). `_sort` runs again.

### `_init_data` (`91-117`)

1. `None` returns an empty float64 array.
2. `float` or `int` (not `bool`) returns `np.ones(self.frequency.shape) * data`.
3. Fast path: a numeric ndarray without NaN is copied.
4. Slow path (object arrays, masked arrays, floats containing NaN): NaN and `None` become `None`, producing an object array.

For Rust: represent missing samples as `f64::NAN` and reproduce the "reject if any NaN" checks in `_smoothen_fractional_octave` and `equalize`; the app never supplies `None` inputs.

### `_sort` (`119-146`)

`sorted_inds = self.frequency.argsort()` (unstable sort), reorder `frequency`, raise `ValueError` on duplicate frequencies (message names the first duplicate), and reorder each non-empty data array with the same permutation.

### `reset` (`148-179`)

```python
def reset(self, raw=False, smoothed=True, error=True, error_smoothed=True, equalization=True,
          fixed_band_eq=True, parametric_eq=True, equalized_raw=True, equalized_smoothed=True, target=True)
```

Each truthy flag sets the attribute to an empty array. Effective clear sets of the internal callers:

| Caller | Cleared |
|---|---|
| `interpolate` (901) | `smoothed`, `parametric_eq`, `fixed_band_eq` (`smoothed` is neither interpolated nor preserved) |
| `center` (938) | `equalization`, `parametric_eq`, `fixed_band_eq`, `equalized_raw`, `equalized_smoothed` |
| `compensate` (1020-1031) | `error_smoothed`, `equalization`, `parametric_eq`, `fixed_band_eq`, `equalized_raw`, `equalized_smoothed` |
| `smoothen_fractional_octave` (1153-1164) | `equalization`, `parametric_eq`, `fixed_band_eq`, `equalized_raw`, `equalized_smoothed` |
| `smoothen_heavy_light` (1228-1239) | same as above |

### `read_from_csv` / `read_csv` (`181-242`)

1. `name` = filename minus the last extension.
2. Whole file read as UTF-8 text (no BOM stripping).
3. Regexes:
   ```python
   header_pattern = r'frequency(,(raw|smoothed|error|error_smoothed|equalization|parametric_eq|fixed_band_eq|equalized_raw|equalized_smoothed|target))+'
   float_pattern  = r'-?\d+\.?\d+'
   data_2_pattern = r'{fl}[ ,;:\t]+{fl}?'.format(fl=float_pattern)
   data_n_pattern = r'{fl}([ ,;:\t]+{fl})+?'.format(fl=float_pattern)
   autoeq_pattern = r'^{header}(\n{data})+\n*$'
   ```
   `float_pattern` requires at least two digits (`0`, `5`, `-3` do not match; `0.0`, `20`, `-3.5` do). The header must start at position 0 with `frequency` followed by known columns only; CRLF files break the `\n`-anchored groups.
4. AutoEq branch: `csv.DictReader` over the whole text, each of the eleven column names pulled as a float list (missing columns become `None` and thus empty arrays).
5. Guess branch: per line, `re.match(data_2_pattern, line)`; `re.findall(float_pattern, line)`; `floats[0]` = frequency, `floats[1]` = raw. Non-matching lines are dropped.

Post-read fixup in the app (`core/pipeline_stages.py:226-231`): if the parsed FR has an empty `error` and a non-empty `raw`, the pipeline sets `fr.error = -fr.raw.copy()`.

### `generate_frequencies` (`850-857`)

```python
freq = []; f = f_min
while f <= f_max:
    freq.append(f); f *= f_step
return np.array(freq)
```

A geometric grid by repeated in-place multiplication, not a closed form. Reproduce the loop exactly in `f64`. `f_min` is always included; `f_max` generally is not. Grids at fs 48000: `(10, 24000, 1.01)` = 783 points (last about 23949.4 Hz), `(20, 20000, 1.01)` = 695 points (last about 19955.4 Hz), `(20, 24000, 1.01)` = 713 points.

### `interpolate` (`859-901`)

```python
def interpolate(self, f=None, f_step=DEFAULT_STEP, pol_order=1, f_min=DEFAULT_F_MIN, f_max=DEFAULT_F_MAX)
```

1. NaN pruning of `raw` only (864-869): `valid = ~isnan(raw)`; if not all valid, `raw = raw[valid]` and `frequency = frequency[valid]`. Other arrays keep their old length.
2. Interpolators (872-877) over `keys = ['raw', 'error', 'error_smoothed', 'equalization', 'equalized_raw', 'equalized_smoothed', 'target']` (`smoothed`, `parametric_eq`, `fixed_band_eq` are absent): `InterpolatedUnivariateSpline(log10(frequency), values, k=pol_order)` for each non-empty key.
3. New grid (879-882): `generate_frequencies(f_min, f_max, f_step)` when `f is None`, else `np.array(f)`.
4. Zero-frequency fix (885-888): if `frequency[0] == 0`, set it to `0.001` and remember.
5. Evaluate each interpolator at `log10(frequency)`; values outside the original range extrapolate (`ext=0`).
6. Restore `frequency[0] = 0` if fixed.
7. `reset` clears `smoothed`, `parametric_eq`, `fixed_band_eq`.

### `center` (`903-940`)

1. `equal_energy_fr = FrequencyResponse(name='equal_energy', frequency=self.frequency.copy(), raw=self.raw.copy())`.
2. `equal_energy_fr.interpolate()` with no arguments, re-gridding the copy onto `(20, 20000, 1.01)`; a constant-ratio grid makes a plain mean an equal-energy-per-octave average. Out-of-range data extrapolates.
3. `interpolator = InterpolatedUnivariateSpline(log10(equal_energy_fr.frequency), equal_energy_fr.raw, k=1)` (used only in the scalar branch).
4. Two-element list or array: `diff = mean(raw[(f >= lo) & (f <= hi)])` on the re-gridded copy. For `[100, 10000]` that is grid indices 162 to 624 (463 points). Otherwise a one-element list unwraps and `diff = interpolator(log10(frequency))`.
5. `raw -= diff`; `smoothed -= diff` if present; `error += diff` and `error_smoothed += diff` if present (an error is `raw - target`, so it moves the other way).
6. `reset(raw=False, smoothed=False, error=False, error_smoothed=False, target=False)`.
7. Returns `-diff`, which the app adds to the other channel's raw.

### `_tilt` (`942-955`)

```python
c = DEFAULT_F_MIN * np.sqrt(DEFAULT_F_MAX / DEFAULT_F_MIN)   # 20 * sqrt(1000) = 632.4555320336759
n_oct = np.log2(self.frequency / c)
return n_oct * tilt
```

Centred on the hard-coded 20 Hz to 20 kHz band regardless of the object's grid.

### `create_target` (`957-982`)

```python
def create_target(self, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=None):
    bass_boost = biquad.digital_coeffs(self.frequency, DEFAULT_FS,
                                       *biquad.low_shelf(bass_boost_fc, bass_boost_q, bass_boost_gain, DEFAULT_FS))
    tilt = self._tilt(tilt=tilt) if tilt is not None else np.zeros(len(self.frequency))
    return bass_boost + tilt
```

The shelf is designed and evaluated at `DEFAULT_FS = 44100`, not at the project sample rate. Do not fix this.

`biquad.low_shelf(fc, Q, gain, fs)` (`biquad.py:52-79`), RBJ low shelf, returns `1.0, a1, a2, b0, b1, b2` with `a1`, `a2` already negated and normalised:

```
A = 10 ** (gain / 40); w0 = 2*pi*fc/fs; alpha = sin(w0) / (2*Q)
a0 = (A+1) + (A-1)*cos(w0) + 2*sqrt(A)*alpha
a1 = -(-2*((A-1) + (A+1)*cos(w0))) / a0
a2 = -((A+1) + (A-1)*cos(w0) - 2*sqrt(A)*alpha) / a0
b0 = (A*((A+1) - (A-1)*cos(w0) + 2*sqrt(A)*alpha)) / a0
b1 = (2*A*((A-1) - (A+1)*cos(w0))) / a0
b2 = (A*((A+1) - (A-1)*cos(w0) - 2*sqrt(A)*alpha)) / a0
```

`biquad.digital_coeffs(f, fs, a0, a1, a2, b0, b1, b2)` (`biquad.py:112-131`), magnitude in dB without complex arithmetic:

```
w = 2*pi*f/fs; phi = 4*sin(w/2)**2; a1 *= -1; a2 *= -1
c = 10*log10((b0+b1+b2)**2 + (b0*b2*phi - (b1*(b0+b2) + 4*b0*b2))*phi)
  - 10*log10((a0+a1+a2)**2 + (a0*a2*phi - (a1*(a0+a2) + 4*a0*a2))*phi)
```

With the app's gain 0.0 the curve is 0 dB everywhere and with tilt 0.0 the target is all zeros, but the general path must exist (Q is 0.76 in the app).

### `compensate` (`984-1031`)

```python
def compensate(self, compensation, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71,
               tilt=None, sound_signature=None, min_mean_error=False)
```

1. `compensation = FrequencyResponse(name='compensation', frequency=compensation.frequency, raw=compensation.raw)`; `compensation.center()` (scalar 1000 Hz branch). The caller's object is not mutated. The compensation curve must already be on the same grid as `self`.
2. `self.target = compensation.raw + self.create_target(bass_boost_gain, bass_boost_fc, bass_boost_q, tilt)`, evaluated on `self.frequency`.
3. `sound_signature` branch (1004-1009) is never exercised by the app (it mutates the caller's object); omit.
4. `self.error = self.raw - self.target`.
5. `min_mean_error=True`: `delta = mean(error[(f >= 100) & (f <= 10000)])` on `self.frequency` (no re-grid), then `error -= delta`, `target += delta`.
6. `reset(raw=False, smoothed=False, error=False, error_smoothed=True, equalization=True, parametric_eq=True, fixed_band_eq=True, equalized_raw=True, equalized_smoothed=True, target=False)`.

The headphone-compensation calls pass a zero curve with `min_mean_error=False`, so they reduce to `target = 0`, `error = raw`.

### `_window_size` (`1033-1050`)

```python
k = 2 ** octaves
steps = [self.frequency[i] / self.frequency[i-1] for i in range(1, len(self.frequency))]
step_size = sum(steps) / len(steps)
window_size = round(math.log(k) / math.log(step_size))
if not window_size % 2: window_size += 1
return window_size
```

Python `round` is half-to-even; `sum` is a naive left-to-right float sum. Window sizes on the 1.01 grid: 1/12 octave = 7, 1/6 = 13, 1/5 = 15, 1/3 = 23, 1.3 = 91, 2 = 139.

### `_sigmoid` (`1052-1058`)

```python
f_center = np.sqrt(f_upper / f_lower) * f_lower
half_range = np.log10(f_upper) - np.log10(f_center)
f_center = np.log10(f_center)
a = expit((np.log10(self.frequency) - f_center) / (half_range / 4))
a = a * -(a_normal - a_treble) + a_normal
```

Equivalent to `a_normal + s * (a_treble - a_normal)` with a logistic that reaches `expit(4)` at `f_upper`. Requires `f_upper > f_lower` (`equalize` does not guard this).

### `_smoothen_fractional_octave` (`1060-1105`)

```python
def _smoothen_fractional_octave(self, data, window_size=1/3, iterations=1, treble_window_size=None,
                                treble_iterations=None, treble_f_lower=100.0, treble_f_upper=10000.0)
```

1. Raise `ValueError` if `frequency` or `data` contains NaN.
2. Normal pass: `y_normal = data`; `for _ in range(iterations): y_normal = savgol_filter(y_normal, self._window_size(window_size), 2)`.
3. Treble pass: the same with `treble_window_size` and `treble_iterations`. Both passes run over the full range.
4. `k_treble = self._sigmoid(treble_f_lower, treble_f_upper)`; `k_normal = 1 - k_treble`; return `y_normal * k_normal + y_treble * k_treble`.

`savgol_filter(y, window_length, 2)` with scipy defaults: `deriv=0`, `delta=1.0`, `mode='interp'` (edge samples from a least-squares quadratic fit of the first/last `window_length` samples), requires `window_length <= len(y)`. Filtering runs on the index axis; the geometric grid makes a fixed index window a fixed fractional-octave window.

### `smoothen_fractional_octave` (`1107-1164`)

Raises if `treble_f_upper <= treble_f_lower`; `self.smoothed = self._smoothen_fractional_octave(self.raw, ...)`; `self.error_smoothed = ...(self.error, ...)` when `error` is present; `reset` clears the five EQ arrays.

### `smoothen` (`1166-1179`)

Forwards to `smoothen_fractional_octave` with `iterations=1` and `treble_iterations=1` hard-coded.

### `smoothen_heavy_light` (`1181-1239`), the default-path smoother

```python
light = self._smoothen_fractional_octave(self.error, window_size=1/6, iterations=1,
        treble_f_lower=100, treble_f_upper=10000, treble_window_size=1/3, treble_iterations=1)   # windows 13 and 23
heavy = self._smoothen_fractional_octave(self.error, window_size=1/3, iterations=1,
        treble_f_lower=1000, treble_f_upper=6000, treble_window_size=1.3, treble_iterations=1)   # windows 23 and 91
combination = np.max(np.vstack([light, heavy]), axis=0)                                            # signed max
self.smoothed = self._smoothen_fractional_octave(self.raw, window_size=1/3, iterations=1,
        treble_f_lower=100, treble_f_upper=10000, treble_window_size=1/3, treble_iterations=1)
self.error_smoothed = self._smoothen_fractional_octave(combination, same parameters as smoothed)
```

Then `reset(raw=False, smoothed=False, error=False, error_smoothed=False, target=False, everything else True)`.

### `equalize` (`1241-1310`)

```python
def equalize(self, max_gain=6.0, smoothen=True, treble_f_lower=6000.0, treble_f_upper=8000.0,
             treble_max_gain=6.0, treble_gain_k=1.0)
```

1. `self.equalization = []`, `self.equalized_raw = []` (`equalized_smoothed` is not cleared here).
2. `error = error_smoothed` if present, else `error`, else `ValueError`.
3. `ValueError` if `error` contains NaN.
4. Gain limiting (1277-1284):
   ```python
   max_gain = self._sigmoid(treble_f_lower, treble_f_upper, a_normal=max_gain, a_treble=treble_max_gain)
   gain_k   = self._sigmoid(treble_f_lower, treble_f_upper, a_normal=1.0, a_treble=treble_gain_k)
   gain     = -error * gain_k
   clipped  = gain > max_gain
   kink_inds = np.flatnonzero(np.concatenate(([clipped[0]], clipped[1:] != clipped[:-1])))
   if len(kink_inds) and kink_inds[0] == 0: kink_inds = kink_inds[1:]
   self.equalization = np.where(clipped, max_gain, gain)
   ```
   Only positive gain is clipped; the treble ceiling is `treble_max_gain = 6.0` (never overridden by the app), so on the default path the ceiling is 40 dB below 10 kHz blending to 6 dB above 24 kHz. `gain_k` is identically 1.0 for every app call.
5. Kink smoothing (1286-1305) when `smoothen=True`:
   ```python
   window_size = self._window_size(1 / 12)           # 7 on the 1.01 grid
   doomed_inds = set()
   for i in kink_inds:
       start = i - min(i, (window_size - 1) // 2)
       end   = i + 1 + min(len(self.equalization) - i - 1, (window_size - 1) // 2)
       doomed_inds.update(range(start, end))
   doomed_inds = sorted(doomed_inds)
   for i in range(1, 3):
       if len(self.frequency) - i in doomed_inds:
           del doomed_inds[doomed_inds.index(len(self.frequency) - i)]
   keep = np.ones(len(self.frequency), dtype=bool); keep[doomed_inds] = False
   interpolator = InterpolatedUnivariateSpline(np.log10(self.frequency[keep]), self.equalization[keep], k=2)
   self.equalization = interpolator(np.log10(self.frequency))
   ```
   The last two samples are rescued from deletion; the k=2 re-spline can push the curve slightly above `max_gain` near a kink and there is no re-clamp.
6. `self.equalized_raw = self.raw + self.equalization`; `self.equalized_smoothed = self.smoothed + self.equalization` when `smoothed` is present.

### `minimum_phase_impulse_response` (`637-681`)

```python
def minimum_phase_impulse_response(self, fs=44100, f_res=10, normalize=True):
    f_res /= 2
    fr = FrequencyResponse(name='fr_data', frequency=self.frequency.copy(), raw=self.equalization.copy())
    f_min = np.max([fr.frequency[0], f_res])
    interpolator = InterpolatedUnivariateSpline(np.log10(fr.frequency), fr.raw, k=1)
    gain_f_min = interpolator(np.log10(f_min))
    n = round(fs // 2 / f_res)
    n = next_fast_len(n)                          # scipy.fftpack.next_fast_len (2, 3, 5)
    f = np.linspace(0.0, fs // 2, n)
    fr.interpolate(f, pol_order=1)
    fr.raw[fr.frequency <= f_min] = gain_f_min
    if normalize:
        fr.raw -= np.max(fr.raw); fr.raw -= 0.5
    fr.raw *= 2
    fr.raw = 10 ** (fr.raw / 20)
    fr.raw[-1] = 0.0
    ir = firwin2(len(fr.frequency) * 2, fr.frequency, fr.raw, fs=fs)
    ir = minimum_phase(ir, n_fft=len(ir))
    return ir
```

The source curve is `self.equalization`. Default path (fs 48000, `f_res=5` halved to 2.5): `n = 9600`, `next_fast_len(9600) = 9600`, `numtaps = 19200`, output length `(19200 + 1) // 2 = 9600` taps. Channel balance and mic deviation (`f_res=10`): `n = 4800`, output 4800 taps (mic deviation truncates afterwards to `min(2048, fs // 10)`). `fs // 2` is integer floor division. `firwin2` uses scipy defaults (`nfreqs = 1 + 2**ceil(log2(numtaps))`, Hamming, not antisymmetric); `minimum_phase` is the homomorphic method with `half=True` (the installed SciPy 1.18.0 lifter rule, see `tests/migration/README.md`).

## Default-path summary

On a bare run with no `room*.wav`, no `headphones.wav`, no `eq*.csv`, no `--channel_balance`, no `--plot`, no mic deviation, the reachable set is: `__init__`, `_init_data`, `_sort`, `generate_frequencies`, `interpolate` (+ `reset`), `create_target`, `_tilt`, `biquad.low_shelf`, `biquad.digital_coeffs`, `smoothen_heavy_light`, `_smoothen_fractional_octave`, `_window_size`, `_sigmoid`, `savgol_filter`, `equalize` (+ `InterpolatedUnivariateSpline(k=2)`), `minimum_phase_impulse_response` (+ `firwin2`, `minimum_phase`). The default-on file stages add `center`, `compensate`, `copy`, `smoothen`, `read_from_csv`.

Quirks to keep, not fix:

- `create_target` designs its shelf at 44100 Hz regardless of project fs.
- `center()` re-grids onto 20 Hz to 20 kHz / 1.01 before averaging; `get_center_value` does not.
- `interpolate` silently discards `smoothed`.
- `equalize`'s treble ceiling is `DEFAULT_TREBLE_MAX_GAIN = 6.0`, so `max_gain=40` only applies below 10 kHz.
- `equalize` does not clear `equalized_smoothed` at entry.
- `_window_size` uses Python's half-to-even `round`.
- `read_from_csv`'s float pattern rejects single-digit numbers.
- The `normalize=True` branch of `minimum_phase_impulse_response` is dead in this application.
