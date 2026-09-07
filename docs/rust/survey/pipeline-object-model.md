# Impulcifer 2.x DSP object model and pipeline stages (survey for the Rust port)

Repo root: `E:/Impulcifer`. Line numbers are 1-based inclusive and refer to the tree at 2.14.1 (2026-09-07). This document is the design input for the pipeline packets (estimator, impulse response, HRIR, room correction, virtual bass, mic deviation, stage table, outputs). `core/channel_balance.py` does not exist; channel balance lives in `core/hrir.py`.

## Global object graph

```
ImpulseResponseEstimator (fs, low, high, n_octaves, w1, w2, test_signal, duration, inverse_filter)
   └─ HRIR (estimator, fs, irs: {speaker: {"left"|"right": ImpulseResponse}})
         └─ ImpulseResponse (data: f64[], fs: int, recording: f64[]|None)
BRIRPipeline (config: ProcessingConfig, dir_path, estimator, hrir, room_frs,
              hp_left, hp_right, eq_left, eq_right, target, applied_gain)
```

Shared primitives (`core/audio_io.py:18-118`): `to_db(x) = 20*log10(|x| + 1e-10)`; `db_to_gain(x) = 10**(x/20)`; `read_wav(path, expand=False) -> (fs, data)` (rows = tracks, samples in -1..1); `write_wav(path, fs, data, bit_depth=32)` (PCM_16/24/32, transposes when `data.shape[1] > data.shape[0]`, `os.makedirs` first); `magnitude_response(x, fs)` (100-113: `nfft = len(x)`, `half = ceil(nfft/2)`, `X = rfft(x)`, `f = arange(half)*(fs/nfft)`, `20*log10(|X[:half]|)` with no epsilon, so `-inf` is possible); `running_mean(x, N)` (116-118: cumulative-sum boxcar, output length `len(x)-N+1`).

## core/constants.py

- `SPEAKER_NAMES` (5): 15 entries `FL FR FC BL BR SL SR WL WR TFL TFR TSL TSR TBL TBR`.
- `SPEAKER_PATTERN` (7): the 15 names plus `X`. `SPEAKER_LIST_PATTERN` (8): `([A-Z]{2,3}(,[A-Z]{2,3})*)`, the discovery regex for `FL,FR.wav` and `room-FL,FR-left.wav`.
- `LEFT_SIDE_SPEAKERS` (14): `FL SL BL WL TFL TSL TBL`; `RIGHT_SIDE_SPEAKERS` (15): `FR SR BR WR TFR TSR TBR`; `CENTER_SPEAKERS` (16): `FC LFE`; `speaker_side(name)` (19-26) uppercases and returns `left|right|center` (default center).
- `TRUEHD_11CH_ORDER` (28), `TRUEHD_13CH_ORDER` (29), `CHANNEL_LAYOUT_MAP` (31-34) `{11: ..., 13: ...}`.
- `HESUVI_TRACK_ORDER` (95-98): 30 names; the first 14 are the HeSuVi base `FL-left, FL-right, SL-left, SL-right, BL-left, BL-right, FC-left, FR-right, FR-left, SR-right, SR-left, BR-right, BR-left, FC-right`, then `WL, WR, TFL, TFR, TSL, TSR, TBL, TBR` as `-left, -right` pairs.
- `HEXADECAGONAL_TRACK_ORDER` (100-104): 32 names, speaker-major pairs in order `FL FR FC LFE BL BR SL SR WL WR TFL TFR TSL TSR TBL TBR`; the `LFE` pair is always a silent placeholder.
- `SEQUENCE_TRACK_ORDERS` (`impulse_response_estimator.py:18-23`): `5.1` = 6, `7.1` = 8, `7.1.4` = 12, `7.1.6` = 14.
- `SPEAKER_ANGLES` (36-52), `SPEAKER_DELAYS` (55-57, all 0 seconds), `IPSILATERAL_PAIRS` (66-75): `(FL,FR) (SL,SR) (BL,BR) (TFL,TFR) (TSL,TSR) (TBL,TBR) (FC,FC) (WL,WR)`; `IR_ROOM_SPL` (80-83): all zero; `HEADPHONES_FILENAME`, `HEADPHONES_FALLBACK_FILENAMES = ('headphones.wav','headphone.wav','hp.wav','compensation.wav')`; `track_name(speaker, side) = f"{speaker}-{side}"` (116-118); `DEFAULT_SWEEP_FS = 48000`, `DEFAULT_SWEEP_DURATION = 5.0`, `SWEEP_TRACK_LAYOUTS`; `TEST_SIGNALS` (126-139), default `sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`.

## core/impulse_response.py

`class ImpulseResponse` (15); `EPSILON = 1e-20` (12). Rust: `{ data: Vec<f64>, fs: u32, recording: Option<Vec<f64>> }`.

- `__init__(data, fs, recording=None)` (16-19): plain assignment.
- `copy()` (21-22): deep copy of `data` and `recording`.
- `__len__`, `duration()` (24-30).
- `peak_index(start=0, end=None, peak_height=0.12589)` (32-70): ported as `impulcifer-dsp::peaks::first_peak_index`. Empty data returns 0; empty slice or `max|data| < 1e-20` returns `start`; normalise a copy; `find_peaks(data, height)` and `find_peaks(-data, height)`; return `min(peaks) + start`, else `argmax(|data|) + start`. Never returns `None`.
- `decay_params()` (72-73), `decay_times(...)` (75-80): delegate to `core/decay.py`.
- `crop_head(head_ms=1)` (82-90): `crop_start = max(0, peak_index() - int(fs*head_ms/1000))`; `data = data[crop_start:]`. No fade.
- `shift(samples)` (92-108): length preserving. Positive: `concat(zeros(samples), data)[:n]`. Negative: `trimmed = data[-samples:]`, `np.pad(trimmed, (0, n - len(trimmed)))`. Zero: no-op. Exact numpy ops matter for byte parity.
- `equalize(fir)` (110-119): `data = convolve(data, fir, mode="full")`, grows by `len(fir)-1`.
- `resample(fs)` (121-124): `data = nnresample.resample(data, fs_new, fs_old)`; `self.fs = fs` (P06).
- `convolve(x)` (126-135): returns `convolve(x, data, mode="full")`.
- `decay_adjustment_params(target)` (137-138), `adjust_decay(target)` (140-151): `apply_decay_window(data, params)` multiplies `data` in place.
- `magnitude_response()` (153-155).
- `frequency_response()` (157-188): guards (`len(data) < 2`, no bins, too short) return an all-zero FR on `generate_frequencies(f_step=1.01, f_min=10, f_max=fs/2)`. Otherwise decimate the linear FFT bins: `target_fr_points = (fs/2)/4.0`, `step = max(1, int(round(len(f)/target_fr_points)))`, take `f[1::step]`, `m[1::step]` (the DC bin is dropped), build `FrequencyResponse(name="Frequency response", frequency, raw)` and `interpolate(f_step=1.01, f_min=10, f_max=fs/2)` (k=1 on `log10(f)`).

## core/decay.py

`EPSILON = 1e-20` (9). `_peak_index` (12-41) duplicates `ImpulseResponse.peak_index`.

### `decay_params(data, fs)` (44-260) -> `(peak_index, knee_index_absolute, noise_floor_db, window_size)`

Lundeby-style estimate.

1. `len(data) < 10`: return `(0, len, -200.0, len or 1)`.
2. `peak_index = _peak_index(data)`; the analysis window is `data[peak_index : min(peak_index + 2*fs, len)]`, normalised by its `max|.|` when that exceeds `1e-20`.
3. `squared = data**2`; `t_squared = linspace(0, len/fs, len)`.
4. First pass with `wd = 0.03` s: `n = int(len/fs/wd)`, `w = int(len/n)`; reshape `squared[:n*w]` to `(n, w)`, mean over axis 1, `10*log10(max(., 1e-20))` gives `windows`; `t_windows = arange(n)*wd + wd/2`.
5. Initial `noise_floor = 10*log10(mean(squared[int(0.9*len):]))`.
6. `slope_end` = first window with `windows <= noise_floor + 10.0` (else all); `linregress(t_windows[:slope_end], windows[:slope_end])`; knee time `(noise_floor - intercept)/slope` clipped to `[t_squared[0], t_squared[-1]]`.
7. Re-size windows for 3 windows per 10 dB: `wd = 10 / (|slope| * 3)`; recompute `n`, `w`, `t_windows`, `windows`.
8. Up to 5 refinement iterations: `noise_floor_start_index` = first window with `windows <= knee_value - 5`; `noise_floor_start_time = max(t_windows[idx], 0.1*total)`; `noise_floor_end_time = min(start + knee_time, total)`; recompute the noise floor from `squared[start:end]`. Late slope between `slope_start` = (first index with `windows <= noise_floor + 28`) - 1 and `slope_end` = (first index with `windows <= noise_floor + 8`) - 1, needs at least 2 points. New knee time `(noise_floor - late_intercept)/late_slope` clipped to `[t_windows[0], t_windows[-1]]`; stop when the knee window index is unchanged.
9. Return `(peak_index, peak_index + argmin|t_squared - knee_time|, noise_floor, w)`. The second element is absolute.

### `decay_times(data, fs, peak_ind=None, knee_point_ind=None, noise_floor=None, window_size=None)` (263-352) -> `(EDT, RT20, RT30, RT60)` seconds or `None`

1. Any `None` parameter recomputes all four with `decay_params`.
2. `t = linspace(0, len/fs, len)`; `knee_point_ind -= peak_ind`; `data = data[peak_ind:]` normalised by `max|.|`.
3. Envelope is `|data|` (no Hilbert).
4. Schroeder backward integral: `cumsum(analytical[knee::-1]**2 / sum(analytical[:knee]**2))[:0:-1]`, then `10*log10`.
5. Moving-average power: `avg_head = min(window_size//2, peak_ind)`, `avg_tail = min(window_size//2, len - (peak_ind + knee))`, `avg_offset = window_size//2 - avg_head`; slice `ir_data[peak_ind - avg_head : peak_ind + knee + avg_tail]`, normalise, square, `running_mean(., window_size)`, `10*log10(avg + 1e-18)`.
6. Vertical alignment: `offset = mean(schroeder[fit_start:fit_end] - avg[fit_start - avg_offset : fit_end - avg_offset])` with `fit_start = max(0.1*len(schroeder), avg_offset)`, `fit_end = min(0.9*len(schroeder), avg_offset + len(avg))`.
7. For `(-1,-10,-10,EDT), (-5,-25,-20,RT20), (-5,-35,-30,RT30), (-5,-65,-60,RT60)`: skip if `end_target < noise_floor + offset + 10`; first indices where `schroeder <= start_target` and `<= end_target`; need `end - start >= 2`; `slope = linregress(t[start:end], schroeder[start:end]).slope`; result `decay_target / slope`.

### `decay_adjustment_params(data, fs, target)` (355-380) -> `None` or `(window_start, half_window, knee_point_index, window_level)`

Picks `rt_slope = rt_level / rt_time` from the first available of `[(edt,-10),(rt20,-20),(rt30,-30),(rt60,-60)]`. `target_slope = -60 / target` (target in seconds); if `target_slope > rt_slope` return `None` (never lengthen decay). `knee_point_time = knee_point_index / fs`; `knee_point_level = rt_slope * knee_point_time`; `target_level = target_slope * knee_point_time`; `window_level = target_level - knee_point_level`; `window_start = peak_index + 2 * (fs // 1000)`; `half_window = knee_point_index - window_start`.

### `apply_decay_window(data, params)` (383-403)

`params is None` returns `data` untouched. Otherwise `window = concat([ones(window_start), hann(half_window*2)[half_window:], zeros(len(data) - knee_point_index)]) - 1.0`; `window *= -window_level`; `window = 10 ** (window / 20)`; `data *= window` (lengths must match exactly).

## core/impulse_response_estimator.py

Constructor (33-50): `fs` int, `high = fs/2`, `n_octaves = ceil(log2(high/5))` (`P`), `low = high / 2**n_octaves`, `w1`, `w2`, `test_signal = generate_test_signal(min_duration)` (86-147, Farina ESS with `M = ceil(min_duration*fs*(pi/2**P)/(pi*2*ln(2**P)))`, `L = M*pi*2*ln(2**P)/(pi/2**P)`, `N = round(L)`, `freqs = pi/2**P * L/ln(2**P) * exp(arange(N)/N * ln(2**P))`, `sin(freqs)`, half-octave Hann fade-in of `2*int(fs*seconds_per_octave*fade_in)` samples rounded up to even, no fade-out), `duration = len/fs`, `inverse_filter` (73-84: `flip(test_signal) * (2**(P/N))**(-arange(N)) * P*ln2/(1 - 2**-P)`, normalised by `|fft(convolve(inverse_filter, test_signal, method='auto'))[round(nfft/4)]|`). Already ported for sweep generation in `core/sweep_signal.py` terms; the Rust port must reproduce `generate_test_signal` and `generate_inverse_filter` bit-for-bit against `tests/test_sweep_signal.py` expectations.

- `estimate(recording)` (149-151): `convolve(recording, inverse_filter, mode='same', method='auto')`, output length `len(recording)`.
- `sweep_sequence(speakers, tracks)` (153-232): `tracks` in `SEQUENCE_TRACK_ORDERS` uses that order; `stereo` requires 1 to 2 speakers, positional; `mono` forces `['FL']`. Buffer length `int((fs*2.0 + len(self)) * len(speakers) + fs*2.0)`; speaker `i` row = `concat(zeros(int((fs*2 + len)*i + fs*2)), test_signal, zeros(int((fs*2 + len)*(n - i - 1) + fs*2)))`. The duplicate-speaker check at 186-189 never appends, so duplicates are not rejected.
- `from_wav(file_path)` (234-262): `read_wav`, row 0, `duration = (len(data) - 1)/fs`, construct; if `len(test_signal) != len(data)` adopt the WAV as `test_signal`, `duration = len/fs`, regenerate the inverse filter; else if `max|test_signal - data| > 1e-4` adopt the WAV and regenerate (duration not updated).
- `file_name(bit_depth)` (264-273): `f'{duration:.2f}s-{fs:d}Hz-{bit_depth:d}bit-{low:.2f}Hz-{high:.0f}Hz'`; `main` prefixes `sweep-` or `sweep-seg-{speakers}-{tracks}-` (ported in `impulcifer-io::sweep_files`).

## core/hrir.py

- `_channel_balance_groups()` (39-41): `[[l] if l == r else [l, r] for l, r in IPSILATERAL_PAIRS]`.
- `get_center_value(fr, frequency_range)` (44-74): k=1 spline on `log10(f)` (retry k=1 on `ValueError`); a 2+ element range averages `fr.raw` on the caller's grid; a scalar evaluates the spline; returns `-diff`.
- `_ingest_recording(estimator, expected_fs, file_path, speakers, side=None, silence_length=2.0, debug=False)` (77-364): `read_wav(expand=True)`; `fs` must equal `expected_fs`; `silence_length*fs` must be integral; `tracks_k = 2 if side is None else 1`; `n_columns = round(len(speakers) / (recording.shape[0] // tracks_k))`; drop the leading silence; `column_size = silence_length + len(estimator)` (shrunk to the whole recording when `n_columns <= 1`, else `shape[1] // n_columns`); columns `recording[:, i*column_size : min((i+1)*column_size, N)]` kept only if at least `len(estimator)` long; fallbacks (reduced silence `max(int(0.5*fs), N - len(estimator))` with `column_size = len(estimator)`; then `N > 0.8*len(estimator)`); `ValueError` if nothing survives. Track mapping: `while i < n_tracks` stepping `tracks_k`; for column `j`, `n = int(i // 2 * len(columns) + j)` (hard-coded `// 2`, reproduce), `speaker = speakers[n]` skipped if out of range or unknown; `side is None` fills left from track `i` and right from `i+1`; otherwise fills `side`. Each IR is `ImpulseResponse(estimator.estimate(column[track]), fs, column[track])`.
- `HRIR.__init__(estimator)` (368-371); `copy()` (373-381) shares the estimator and deep-copies left/right only; `subset(speakers, copy_irs=False)` (383-395).
- `open_recording(file_path, speakers, side=None, silence_length=2.0, debug=False)` (397-425): `ValueError` if `self.fs != estimator.fs`; merges `irs.setdefault(speaker, {}).update(sides)`.
- `write_wav(file_path, track_order=None, bit_depth=32, *, trim_extensions=False, remove_silent_channels=False)` (427-474): default `HEXADECAGONAL_TRACK_ORDER`; flatten to `{track_name: data}`; missing tracks become `zeros(reference_len)` (all IRs assumed equal length); `remove_silent_channels` drops every all-zero row, writes `PCM_{bit_depth}` and appends an `ICHL` RIFF chunk with `{"version":1,"tracks":[...]}` (returns early, skipping `trim_extensions`); otherwise `trim_extensions` pops trailing all-zero pairs down to the base count (14 HeSuVi, 16 hexadecagonal, `len(order)` custom) and writes with `audio_io.write_wav`.
- `normalize(peak_target=-0.1, avg_target=None)` (476-565) -> gain dB: sum left IRs and right IRs (zero-pad to the max length), `magnitude_response` of each; `peak_target`: `gain = -max(vstack([mr_l, mr_r])) + peak_target`; `avg_target`: `gain = -mean(concat(mr_l[80 < f < 6000], mr_r[80 < f < 6000])) + avg_target`; both or neither raise; apply `10**(gain/20)` to every `ir.data` in place (threaded when more than 4 speakers).
- `crop_heads(head_ms=1)` (567-631): per speaker `peak_left`, `peak_right`; `head = int(head_ms*fs/1000)`; `delay = int(round(SPEAKER_DELAYS[speaker]*fs)) + head`; if `peak_left < peak_right` use `crop_index = max(0, peak_left - delay)` (warn if the speaker is right-side), else mirror with `peak_right`; both ears cropped by the same index (ITD preserved); fade-in `hann(head*2)[:head]` on both ears when long enough.
- `crop_tails()` (633-672): `tail_ind` = element 1 of `decay_params()` per IR (fallback `len`); `seconds_per_octave = len(estimator)/estimator.fs/estimator.n_octaves`; `fade_out = 2*int(fs*seconds_per_octave*(1/24))`; `window = hann(fade_out)[fade_out//2:]`; `fft_len = fftpack.next_fast_len(max(tail_indices))`; `tail_ind = min(min(lengths), fft_len)`; each `ir.data = ir.data[:tail_ind]` then `*= concat(ones(len - len(window)), window)`.
- `channel_balance_firs(left_fr, right_fr, method)` (674-783) -> `[left_fir, right_fir]`: `mids` (gain from `get_center_value` over `[100, 3000]`, `unit_impulse(int(round(fs*0.1)))`), `trend` (2-octave `smoothen_fractional_octave`, treble bounds `20000..int(round(fs/2))`, `minimum_phase_impulse_response(fs, normalize=False)`), `left|right` (1/3 octave, `center([100,10000])`, `smoothen_heavy_light`, `equalize(max_gain=15, treble_f_lower=20000, treble_f_upper=fs/2)`), `avg|min` (gain averaged over both, 1/3 octave with `treble_f_upper=23999`, shared target `(l+r)/2` or elementwise min, `equalize(max_gain=15, treble_f_lower=2000, treble_f_upper=fs/2)`), numeric string (`10**(v/20)` gain on the right).
- `correct_channel_balance(method)` (785-818): per fully present group, average left data and right data (`mean(vstack, axis=0)`), `frequency_response()` of each, `channel_balance_firs`, then `equalize(firs[0|1])` on every speaker in the group (full convolution).
- `correct_microphone_deviation(correction_strength=0.7, anchor="auto", plot_analysis=False, plot_dir=None)` (820-875): wrapper around `apply_microphone_deviation_correction_to_hrir`.
- `equalize(fir)` (877-907): accepts a 2-row array, list of arrays, list of `ImpulseResponse` or a single row (tiled to both ears); `ir.equalize(fir[0|1])`.
- `resample(fs)` (909-938): per IR `nnresample`, threaded when more than 4 speakers; sets `self.fs`; must be the last step.
- `align_ipsilateral_all(speaker_pairs=None, segment_ms=30)` (940-974): `segment_len = int(fs*segment_ms/1000)`; self-pair `(FC, FC)`: `correlate(left[:seg], right[:seg], mode="full")`, `lags = arange(-len+1, len)`, `lag = lags[argmax]`; `lag > 0` shifts right by `lag`, `lag < 0` shifts left by `-lag`. Cross pair: correlate `sp1.left[:seg]` with `sp2.right[:seg]`; `lag > 0` shifts both sides of `sp2` by `+lag`; `lag < 0` shifts both sides of `sp1` by `-lag`.
- `align_onset_groups_peak_leftref(groups=None)` (976-1020): default groups `[(FL,FR), (SL,SR), (BL,BR), (WL,WR), (TFL,TFR), (TSL,TSR), (TBL,TBR), (FC,)]`; reference is `FL` left `peak_index()` (`RuntimeError` if missing); for each other group `shift = group_left_peak - ref_peak` and every speaker/side in the group `shift(-shift)`.
- `calculate_reflection_levels(direct_sound_duration_ms=2, early_ref_start_ms=20, early_ref_end_ms=50, late_ref_start_ms=50, late_ref_end_ms=150, epsilon=1e-12)` (1022-1109): RMS of segments after `peak_index()`; `db = 20*log10(rms_seg/rms_direct + 1e-12)`; used by `write_readme` only.

## core/room_correction.py

- `RoomMeasurement` (16-22), `RoomMeasurementDiscovery` (25-33), `discover_room_measurements(dir_path)` (36-75): regex `^room-([A-Z]{2,3}(,[A-Z]{2,3})*)(-(left|right))?\.wav$`; `generic_path = <dir>/room.wav`; `mic_calibration_path = room-mic-calibration.csv` then `.txt`; `target_path = room-target.csv`; `responses_path = <dir>/room-responses.wav` (output).
- `room_correction(estimator, dir_path, target=None, mic_calibration=None, fr_combination_method='average', specific_limit=400, generic_limit=300, plot=False)` (78-182) -> `(rir, frs)`: open target and calibration; `_open_room_measurements`; `missing` speakers; `_open_generic_room_measurement`; return `(None, None)` when nothing exists; with specific IRs: `crop_head()` on each, `rir.crop_tails()`, write `room-responses.wav`, `calculate_specific_room_corrections(rir, target, mic_calibration, limit=specific_limit)`; missing speakers get `room_fr.copy()` for both sides.
- `calculate_specific_room_corrections(rir, target, mic_calibration=None, limit=400)` (185-210): per IR `fr = ir.frequency_response()`; subtract calibration; the first FR sets `reference_gain = fr.center([100,10000])`, later ones do `fr.raw += reference_gain`; `target_adjusted = target.copy()`, `+= IR_ROOM_SPL[speaker][side]`; `fr.compensate(target_adjusted, min_mean_error=False)`; `_apply_correction_limit(fr, limit)` when `limit > 0`.
- `_calculate_generic_room_correction(irs, target, mic_calibration=None, method='average', limit=1000, collect_raws=False)` (231-292): `room_fr = FrequencyResponse('generic_room', generate_frequencies(10, fs/2, 1.01), raw=0, error=0, target=target.raw)`; per IR `frequency_response()`, calibration, `center([100,10000])`, accumulate raw, `compensate(target, min_mean_error=True)`; `conservative` with more than one IR smooths each (`1/3, 1/3`) and collects `error_smoothed`, else `error`; `raw /= n`; combination: `conservative` takes `min` where all errors are positive, `max` where all are negative, zero elsewhere, then `smoothen(1/6, 1/6)` and `error = error_smoothed.copy()`; `average` takes the mean then `smoothen(1/3, 1/3)`; single IR uses it directly then `smoothen(1/3, 1/3)`; `limit > 0` applies `_apply_correction_limit` and multiplies `error_smoothed` by the mask.
- `_correction_limit_mask(frequency, limit)` (295-302): `start = argmax(f > limit/2)`, `end = argmax(f > limit)`, `mask = concat([ones(start), hann(end - start), zeros(len - end)])` (a full Hann over the octave, reproduce literally).
- `_open_room_measurements` (314-325), `_open_generic_room_measurement` (350-386): `n_cols = int(round((len(track)/fs - 2) / (estimator.duration + 2)))`; column `i`: `start = int(2*fs + i*(2*fs + len(estimator)))`, `end = min(int(start + 2*fs + len(estimator)), len)`; `ImpulseResponse(estimator.estimate(seg), fs, seg)`; `crop_head(head_ms=1)`.
- `_open_room_target` (431-442): CSV `read_csv`, `interpolate(f_step=1.01, f_min=10, f_max=fs/2)`, `center()`; flat zero target otherwise (no `center()`).
- `_open_mic_calibration` (450-461): `read_csv`, same `interpolate`, `center()`; explicit missing path raises `FileNotFoundError`.

## core/virtual_bass.py

- `_detect_polarity` (20-23) and `_shift` (26-28) are unused by the main path.
- `_delay_signal(sig, delay, length)` (31-43): non-wrapping delay/advance into a zero buffer.
- `_rfft_magnitude` (46-50), `_mag_at(ir, fs, freq_hz)` (53-57): magnitude at the nearest rfft bin.
- `_duplicate_sos(sos, times)` (60-62): `vstack([sos]*times)`.
- `_rbj_high_shelf(fc, fs, gain_db, q)` (65-79): RBJ cookbook high shelf via `tf2sos`.
- `synthesize_virtual_bass(irs, fs, crossover_freq=250, head_ms=1.0, hp_freq=15.0, invert_polarity=None)` (82-175): guards (`crossover >= fs/2` returns; `> 300` warns); pad every IR to the max length; `imp = unit impulse`; `sos_hp4_sub = butter(4, hp_freq/(fs/2), "high", sos)`; `mpbass_hp_only = sosfilt(sos_hp4_sub, imp)`; `sos_lp8_xo = _duplicate_sos(butter(4, xo/(fs/2), "low", sos), 2)`; `mpbass = sosfilt(sos_lp8_xo, mpbass_hp_only)`; ILD shelves `(150.0, -1.5, 0.760)`, `(400.0, -3.0, 0.660)`, `(800.0, -3.5, 0.610)` stacked; `sos_hp8_xo = _duplicate_sos(butter(4, xo/(fs/2), "high", sos), 2)`; global gain `mean(_mag_at(hp(ir), xo) over all pairs) / (_mag_at(mpbass, xo) + 1e-20)`; `head_samples = int(round(head_ms*1e-3*fs))`; `polarity = -1 if invert_polarity else 1`; per speaker the contralateral ear gets the ILD-shelved bass and the bass is delayed by the ITD (`right_peak - left_peak` from `peak_index()`) and `head_samples`; each ear becomes `sosfilt(sos_hp8_xo, ir) + polarity*global_gain*delayed_bass`. Read `core/virtual_bass.py:120-175` for the exact per-speaker loop before porting; `tests/test_virtual_bass.py` pins `_classify_speaker`, `_detect_polarity`, `_build_ild_shelf` and the full flow.
- `apply_virtual_bass_to_hrir(hrir, crossover_freq, head_ms, hp_freq, invert_polarity)` (178-198): calls the above on `hrir.irs`.

## core/microphone_deviation_correction.py

`_CENTER_SPEAKERS`, `_SINGLE_SPEAKER = "SINGLE"` (45).

- `MicrophoneMatchingCorrector.__init__(sample_rate, correction_strength=0.7, max_correction_db=6.0, smoothing_octave=1/6, f_min=200.0, f_max=16000.0, window_ms=5.0, pre_ms=0.5, anchor="auto")` (57-104): clips strength to `[0,1]`, `f_min` to `[1, nyq*0.5]`, `f_max` to `[f_min*2, nyq*0.98]`; `win_samples = max(round(window_ms*fs/1000), 32)`; `pre_samples = max(round(pre_ms*fs/1000), 0)`; `frequency = generate_frequencies(f_step=1.01, f_min=20.0, f_max=nyq)`.
- `_windowed_power(ir, peak_index)` (106-140): segment `[max(peak - pre, 0), min(peak + win, n))`, zeros if shorter than 8; taper with `np.hanning` halves (`fade_in = min(pre_samples, len//4)`, `fade_out = max(len//4, 1)`); `nfft = scipy.fft.next_fast_len(max(len, 8192))`; `|rfft|` linearly interpolated (`np.interp`, edge values held) onto `self.frequency`; returns power.
- `collect_speaker` (142-151), `collect_speaker_deviation` (154-167, compatibility), `_band_weight` (169-193: raised-cosine in log frequency, 1 inside `[f_min, f_max]`, 0 outside `[f_min/2, min(2*f_max, fs/2*0.999)]`), `estimate_interaural_mismatch` (195-238: frontal anchor when `FC` exists and anchor is `auto|frontal`, else diffuse mean over all speakers; `raw_delta = 10*log10((L+1e-20)/(R+1e-20))`; `smoothen(window_size=1/6, treble_window_size=1/6)` with fallback to raw; `*= _band_weight()`; clip to `±2*max_correction_db`), `design_correction_filters` (247-262: `half = clip(mismatch*strength/2, ±max_correction_db)`, left `-half`, right `+half`), `_fir_from_curve` (264-276: `equalization = curve`, `minimum_phase_impulse_response(fs, normalize=False)`, truncate to `min(2048, fs // 10)`), `get_analysis_summary` (278-292).
- `MicrophoneDeviationCorrector` (295-384): compatibility wrapper and single-pair path (`convolve(mode="same")`), not used by the pipeline.
- `apply_microphone_deviation_correction_to_hrir(hrir, correction_strength=0.7, anchor="auto", plot_analysis=False, plot_dir=None)` (387-454): collect every speaker with both peak indices; estimate; skip entirely when `max_error_db < 0.05`; otherwise one FIR pair applied to every speaker with `ImpulseResponse.equalize` (full convolution).

## core/pipeline_stages.py

- `open_impulse_response_estimator(dir_path, file_path=None)` (85-180): `TEST_SIGNALS` aliases, `generate:<duration>s@<fs>` (snap to the sweep grid, `min_duration=(n-1)/fs`), `None|auto` (sidecar `test.wav`, then `detect_sweep_parameters` with high confidence and not default, else the bundled default WAV through `from_wav`), `.wav` via `from_wav`, `.mlp/.thd/.truehd` via ffmpeg to a temp WAV.
- `_find_eq_settings_file(dir, base)` (183-192): `.csv` then `.txt`.
- `_read_eq_settings(file_path, estimator)` (199-291): decode `utf-8-sig` then `cp1252`; plain CSV through `read_from_csv` (with `error = -raw` when `error` is missing); EqualizerAPO configs through `parse_eqapo_config` on the `(10, fs/2, 1.01)` grid, `error = -db`.
- `equalization(estimator, dir_path)` (294-350): `eq.csv|txt`, `eq-left`, `eq-right`; `interpolate(f_step=1.01, f_min=10, f_max=fs/2, pol_order=1)` on left and (if distinct) right; plot.
- `headphone_compensation(estimator, dir_path, headphone_file_path=None)` (353-482): resolve the file (directory probing over `HEADPHONES_FALLBACK_FILENAMES`, then first `.wav`; relative paths join `dir_path`; fallback `<dir>/headphones.wav`; missing returns `(None, None)`); `HRIR.open_recording(file, speakers=["FL","FR"])`; write `headphone-responses.wav`; `left = irs["FL"]["left"].frequency_response()`, `right = irs["FR"]["right"].frequency_response()`; `gain = left.center([100,10000])`; `right.raw += gain`; `compensate(zero, min_mean_error=False)` on both.
- `create_target(estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt)` (485-501): `FrequencyResponse("bass_and_tilt", generate_frequencies(10, fs/2, 1.01))`, `raw = create_target(...)`.
- `open_binaural_measurements(estimator, dir_path, debug=False)` (504-522): `os.listdir` order, regex `^([A-Z]{2,3}(,[A-Z]{2,3})*)\.wav$`, `open_recording(file, speakers)`; `ValueError` if nothing.
- `write_readme(file_path, hrir, fs, estimator, applied_gain)` (525-687): per-speaker table (PNR from `20*log10(|data[peak]| + 1e-9) - noise_floor_db`, ITD in µs on the contralateral ear, length `(tail - peak)/fs*1000`, first available of RT60/RT30/RT20/EDT), reflection levels, `tabulate(tablefmt="pipe")`, i18n labels.

## core/pipeline.py

`VBASS_POLARITY_MAP = {'auto': None, 'normal': False, 'invert': True}` (28). `ProcessingConfig` (31-393) fields and defaults are already mirrored in `impulcifer-types::config` (see ARCHITECTURE 3.1). `from_kwargs` drops unknown keys.

### `_stage_table` (424-470)

`any_equalization = do_headphone_compensation or do_room_correction or do_equalization`; `mic_deviation_active = microphone_deviation_correction and not do_headphone_compensation`; `mic_deviation_skipped = microphone_deviation_correction and do_headphone_compensation`.

| # | Stage | Gate | Steps |
|---|---|---|---|
| 1 | estimator | always | 1 |
| 2 | room_correction | `do_room_correction` | 1 |
| 3 | headphone_compensation | `do_headphone_compensation` | 1 |
| 4 | equalization_files | `do_equalization` | 1 |
| 5 | target | always | 1 |
| 6 | open_measurements | always | 1 |
| 7 | plot_pre | `plot` | 1 |
| 8 | crop_and_align | always | 1 |
| 9 | virtual_bass | `vbass` | 1 |
| 10 | mic_deviation_skipped | `mic_deviation_skipped` | 0 |
| 11 | mic_deviation | `mic_deviation_active` | 1 |
| 12 | write_responses | always | 0 |
| 13 | equalize | `any_equalization` | 1 |
| 14 | decay | `bool(decay)` | 1 |
| 15 | channel_balance | `channel_balance is not None` | 1 |
| 16 | normalize | always | 1 |
| 17 | write_readme | always | 0 |
| 18 | plot_post | `plot` | 1 |
| 19 | plot_results | always | 1 |
| 20 | plot_additional | `plot` | 1 |
| 21 | interactive_plots | `interactive_plots` | 1 |
| 22 | resample | `fs is not None` | 1 |
| 23 | write_brirs | always | 1 |
| 24 | truehd_layouts | `output_truehd_layouts` | 1 |
| 25 | jamesdsp | `jamesdsp` | 1 |
| 26 | hangloose | `hangloose` | 1 |

Default run total steps: 11.

### Stage bodies (472-997)

- `run` (472-511): total steps, `set_total_steps`, `NotADirectoryError` guard, `abspath`, `check_cancelled()`, run stages in order, `_cleanup`.
- `_stage_estimator` (516-524): `self.estimator = open_impulse_response_estimator(dir_path, file_path=cfg.test_signal)`.
- `_stage_room_correction` (526-542): `self.room_frs` from `room_correction(...)`; writes `room-responses.wav`.
- `_stage_headphone_compensation` (544-552): `hp_left`, `hp_right`; writes `headphone-responses.wav`.
- `_stage_equalization_files` (554-560): `eq_left`, `eq_right`.
- `_stage_target` (562-571): `self.target`.
- `_stage_open_measurements` (573-581): `self.hrir`.
- `_stage_crop_and_align` (593-609): `crop_heads(head_ms)`, `align_ipsilateral_all(list(IPSILATERAL_PAIRS), segment_ms=30)`, `align_onset_groups_peak_leftref()`, `crop_tails()`, in that order.
- `_stage_virtual_bass` (611-624): `apply_virtual_bass_to_hrir(hrir, crossover_freq=vbass_freq, head_ms, hp_freq=vbass_hp, invert_polarity=VBASS_POLARITY_MAP[vbass_polarity])`.
- `_stage_mic_deviation_skipped` (626-627): warning only.
- `_stage_mic_deviation` (629-644): `correct_microphone_deviation(correction_strength=mic_deviation_strength, ...)`, anchor stays `auto`.
- `_stage_write_responses` (646-653): `responses.wav` (hexadecagonal, no trim).
- `_stage_equalize` (655-700): `common_freq = generate_frequencies(f_step=1.01, f_min=10, f_max=fs/2)`; per `(speaker, side)` the worker (`core/parallel_workers.py:69-131`) builds `fr = FrequencyResponse(name, frequency=common_freq.copy(), raw=0, error=0)`, adds `room_frs[speaker][side].error`, `hp_{side}.error`, `eq_{side}.error` when present, subtracts `target.raw`, `smoothen_heavy_light()`, `equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=fs/2)`, `minimum_phase_impulse_response(fs, normalize=False, f_res=5)`; then `ir.equalize(fir)` (full convolution).
- `_stage_decay` (702-724): per speaker in the `decay` dict, `decay_adjustment_params` + `apply_decay_window` on a copy, assigned back.
- `_stage_channel_balance` (726-731): `correct_channel_balance(method)`.
- `_stage_normalize` (733-743): `applied_gain = normalize(peak_target=None if target_level is not None else -0.1, avg_target=target_level)`.
- `_stage_write_readme` (745-760): `README.md`.
- `_stage_plot_results` (791-798): always runs `plot_result(<dir>/plots)`.
- `_stage_resample` (859-871): return when `cfg.fs == hrir.fs`; else `resample(cfg.fs)` and a second `normalize(...)` whose gain is discarded.
- `_stage_write_brirs` (873-895): `hrir.wav` (hexadecagonal, `trim_extensions=True`, `remove_silent_channels=cfg`) and `hesuvi.wav` (HeSuVi order, same flags), both 32-bit.
- `_stage_truehd_layouts` (897-925): `("11ch", TRUEHD_11CH_ORDER, min 8)`, `("13ch", TRUEHD_13CH_ORDER, min 10)`; `truehd_{11ch|13ch}_{n}ch.wav` with `[track_name(ch, side) for ch in available for side in (left, right)]`.
- `_stage_jamesdsp` (927-951): `subset(["FL","FR"], copy_irs=True)`, re-normalised independently, `jamesdsp.wav` with `["FL-left","FL-right","FR-left","FR-right"]`.
- `_stage_hangloose` (953-972): `Hangloose/<SPEAKER>.wav` per present speaker in `SPEAKER_NAMES` order, `["<SP>-left","<SP>-right"]`.
- `_cleanup` (974-997): drop references, close figures, `gc.collect()`; keeps `applied_gain` and `dir_path`.

## core/brir_recovery.py (reads and writes only)

Pure channel reordering; never applies gain, DSP or resampling. Reads `hrir.wav` and `hesuvi.wav` (case-insensitive, ambiguity is an error) with `read_wav(expand=True)`, the optional `ICHL` chunk (`brir_layout.read_track_names`), and Hangloose split files (stems ending in a speaker name, longest-first matching, one shared prefix, stereo, common rate and length). Without a channel map the count must satisfy `base <= count <= len(order)` and be even; `hrir.wav` rejects non-silent LFE. Source precedence: `hrir + hesuvi` (cross-verified) > `hrir` > `hesuvi` > `hangloose`. Writes missing `hrir.wav`, `hesuvi.wav` (stacked by order, `trim_silent_extensions`) and optionally `Hangloose/<speaker>.wav`, always `PCM_32` via temp file + `os.replace`, refusing to overwrite (`OUTPUT_CONFLICT`). Error codes: `INVALID_DIRECTORY, NO_RECOVERY_SOURCE, AMBIGUOUS_SOURCE, INVALID_WAV, INVALID_CHANNEL_MAP, INVALID_CHANNEL_COUNT, NON_SILENT_LFE, SAMPLE_RATE_MISMATCH, SAMPLE_COUNT_MISMATCH, SOURCE_MISMATCH, ALL_CHANNELS_SILENT, OUTPUT_CONFLICT, OUTPUT_WRITE_FAILED`.

## Files read

| Pattern | Rule | Reader |
|---|---|---|
| `<dir>/<SPEAKERLIST>.wav` | `^([A-Z]{2,3}(,[A-Z]{2,3})*)\.wav$` over `os.listdir` | `open_binaural_measurements` |
| `<dir>/test.wav` | only when `test_signal` is `None|auto` | `open_impulse_response_estimator` |
| bundled `data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav` | alias or fallback | same |
| `.wav/.mlp/.thd/.truehd` path | explicit `--test_signal` | same |
| `<dir>/headphones.wav` (+ fallbacks) | option or default | `headphone_compensation` |
| `<dir>/room-<SPEAKERLIST>[-left|-right].wav` | `^room-([A-Z]{2,3}(,[A-Z]{2,3})*)(-(left|right))?\.wav$` | `discover_room_measurements` |
| `<dir>/room.wav` | exact | `_open_generic_room_measurement` |
| `<dir>/room-target.csv` | option or discovery | `_open_room_target` |
| `<dir>/room-mic-calibration.csv` then `.txt` | option or discovery | `_open_mic_calibration` |
| `<dir>/eq.csv|txt`, `eq-left.*`, `eq-right.*` | `.csv` first | `equalization` |
| `<dir>/eq.wav` | only a deprecation warning | `equalization` |

Recording layout: 2 s lead silence, then per speaker `len(estimator)` sweep samples followed by 2 s of silence; column size = 2 s + sweep.

## Files written

| Path | Order | Channels | Bits | Writer |
|---|---|---|---|---|
| `room-responses.wav` | hexadecagonal | 32, no trim | 32 | `room_correction` |
| `headphone-responses.wav` | hexadecagonal | 32, no trim | 32 | `headphone_compensation` |
| `responses.wav` | hexadecagonal | 32, no trim | 32 | `_stage_write_responses` |
| `README.md` | text | | | `_stage_write_readme` |
| `hrir.wav` | hexadecagonal | 16 to 32, trailing silent pairs trimmed; compaction + `ICHL` when requested | 32 | `_stage_write_brirs` |
| `hesuvi.wav` | HeSuVi | 14 to 30, same | 32 | `_stage_write_brirs` |
| `truehd_11ch_<N>ch.wav`, `truehd_13ch_<N>ch.wav` | available channels | 2N (N ≥ 8 / 10) | 32 | `_stage_truehd_layouts` |
| `jamesdsp.wav` | `FL-left, FL-right, FR-left, FR-right` | 4 | 32 | `_stage_jamesdsp` |
| `Hangloose/<SPEAKER>.wav` | `<SP>-left, <SP>-right` | 2 | 32 | `_stage_hangloose` |
| `plots/**`, `interactive_plots/interactive_summary.html` | | | | plotting stages |

`HRIR.write_wav` always builds the full `track_order` matrix (zeros for absent tracks), so the channel count is a property of the order list before trimming.

## Numeric primitive inventory

`scipy.signal`: `convolve(mode='full'|'same')`, `correlate(mode='full')`, `find_peaks(height)`, `windows.hann`, `unit_impulse`, `butter(output='sos')`, `sosfilt`, `tf2sos`, `firwin2`, `minimum_phase(n_fft)`, `savgol_filter(window, 2)`. FFT: `rfft`, `rfftfreq`, `fft`, `next_fast_len` (both `scipy.fftpack` 2/3/5 and `scipy.fft` variants are used). `scipy.stats.linregress` (4 uses in `decay.py`). `scipy.interpolate.InterpolatedUnivariateSpline(log10(f), y, k=1|2)`, `np.interp` in mic deviation. `scipy.special.expit`. `nnresample.resample`. `soundfile` PCM_16/24/32 plus the hand-written `ICHL` chunk (`core/brir_layout.py`). Byte-exactness of `hrir.wav`/`hesuvi.wav` is pinned by `tests/test_brir_integrity.py`; the Rust port's acceptance test mirrors it against 2.x output on `data/demo`.
