"""Lightweight array operations for impulse-response decay analysis and adjustment."""

import numpy as np
from scipy import signal, stats

from core.audio_io import running_mean


EPSILON = 1e-20


def _peak_index(data, start=0, end=None, peak_height=0.12589):
    """Find the first high peak without importing ``ImpulseResponse``."""
    if len(data) == 0:
        return 0

    if end is None:
        end = len(data)

    # Peak height threshold, relative to the data maximum value
    # Copy only the searched range, because normalization mutates it below.
    peak_data = data[start:end].copy()

    if len(peak_data) == 0:
        return start

    max_abs_val = np.max(np.abs(peak_data))
    if max_abs_val < EPSILON:
        return start

    peak_data /= max_abs_val

    peaks_pos, _ = signal.find_peaks(peak_data, height=peak_height)
    peaks_neg, _ = signal.find_peaks(peak_data * -1.0, height=peak_height)
    peaks = np.concatenate([peaks_pos, peaks_neg])

    if len(peaks) == 0:
        return np.argmax(np.abs(peak_data)) + start

    peaks += start
    return np.min(peaks)


def decay_params(data, fs):
    """Determine decay parameters with the Lundeby method."""
    ir_data = data
    if len(ir_data) < 10:
        return 0, len(ir_data), -200.0, len(ir_data) if len(ir_data) > 0 else 1

    peak_index = _peak_index(ir_data)

    # Analyze from the peak to at most two seconds after it.
    analysis_end = min(peak_index + int(2 * fs), len(ir_data))
    if peak_index >= analysis_end:
        if peak_index >= len(ir_data):
            peak_index = len(ir_data) - 1
        if peak_index < 0:
            peak_index = 0
        data = (
            ir_data[peak_index : peak_index + 1].copy()
            if peak_index < len(ir_data)
            else np.array([EPSILON])
        )
    else:
        data = ir_data[peak_index:analysis_end].copy()

    if len(data) == 0:
        data = np.array([EPSILON])

    max_abs = np.max(np.abs(data))
    if max_abs >= EPSILON:
        data = data / max_abs

    squared = data**2
    if len(squared) == 0:
        return peak_index, len(ir_data), -200.0, 100

    t_squared = np.linspace(0, len(squared) / fs, len(squared))

    wd = 0.03
    n = int(len(squared) / fs / wd) if fs > 0 and wd > 0 else 0
    if n == 0:
        noise_floor = 10 * np.log10(max(np.mean(squared), EPSILON))
        return peak_index, peak_index + len(squared), noise_floor, max(1, len(squared))

    w = int(len(squared) / n)
    if w == 0:
        w = 1
    w_fallback = w

    t_windows = np.arange(n) * wd + wd / 2
    windows = squared[: n * w]
    if len(windows) < n * w and n > 0:
        n = len(windows) // w if w > 0 else 0
        if n == 0:
            noise_floor = 10 * np.log10(max(np.mean(squared), EPSILON))
            return peak_index, peak_index + len(squared), noise_floor, w_fallback

    if n == 0:
        noise_floor = 10 * np.log10(max(np.mean(squared), EPSILON))
        return peak_index, peak_index + len(squared), noise_floor, w_fallback

    windows = np.reshape(windows, (n, w))
    windows = np.mean(windows, axis=1)
    windows = 10 * np.log10(np.maximum(windows, EPSILON))

    tail = squared[int(len(squared) * 0.9) :]
    if len(tail) == 0:
        tail = squared
    noise_floor = 10 * np.log10(np.maximum(np.mean(tail), EPSILON))

    candidates = np.where(windows <= noise_floor + 10.0)[0]
    slope_end = len(windows)
    if len(candidates) > 0 and candidates[0] > 0:
        slope_end = candidates[0]

    if slope_end < 2:
        if len(windows) >= 2:
            slope_end = len(windows)
        else:
            return peak_index, peak_index + len(squared), noise_floor, w_fallback

    slope, intercept, _, _, _ = stats.linregress(t_windows[:slope_end], windows[:slope_end])
    if np.isnan(slope) or abs(slope) < EPSILON:
        return peak_index, peak_index + len(squared), noise_floor, w_fallback

    knee_point_time = (noise_floor - intercept) / slope
    if len(t_squared) > 0:
        knee_point_time = np.clip(knee_point_time, t_squared[0], t_squared[-1])
    else:
        knee_point_time = 0

    n_windows_per_10dB = 3
    wd_denominator = abs(slope) * n_windows_per_10dB
    if wd_denominator < EPSILON:
        wd = (t_squared[-1] if len(t_squared) > 0 else 1.0) / 3.0
    else:
        wd = 10 / wd_denominator

    if fs <= 0 or wd <= EPSILON:
        n = 1
    else:
        n = int(len(squared) / fs / wd)
    if n == 0:
        n = 1

    w = int(len(squared) / n)
    if w == 0:
        w = 1

    t_windows = np.arange(n) * wd + wd / 2
    windows = squared[: n * w]
    if len(windows) < n * w and n > 0:
        n = len(windows) // w if w > 0 else 0
        if n == 0:
            knee_ind = np.argmin(np.abs(t_squared - knee_point_time)) if len(t_squared) > 0 else 0
            return peak_index, peak_index + knee_ind, noise_floor, w

    if n == 0:
        knee_ind = np.argmin(np.abs(t_squared - knee_point_time)) if len(t_squared) > 0 else 0
        return peak_index, peak_index + knee_ind, noise_floor, w_fallback

    windows = np.reshape(windows, (n, w))
    windows = np.mean(windows, axis=1)
    windows = 10 * np.log10(np.maximum(windows, EPSILON))

    try:
        knee_point_index = np.argwhere(t_windows >= knee_point_time)[0, 0]
        knee_point_value = windows[knee_point_index]
    except IndexError:
        if len(t_windows) > 0:
            knee_point_time = t_windows[-1]
            knee_point_index = len(t_windows) - 1
            knee_point_value = windows[-1]
        else:
            knee_ind = np.argmin(np.abs(t_squared - knee_point_time)) if len(t_squared) > 0 else 0
            return peak_index, peak_index + knee_ind, noise_floor, w

    noise_floor_iter = noise_floor
    knee_point_time_iter = knee_point_time
    knee_point_value_iter = knee_point_value
    knee_point_index_iter = knee_point_index

    for _ in range(5):
        try:
            noise_floor_start_index = np.argwhere(windows <= knee_point_value_iter - 5)[0, 0]
        except IndexError:
            break

        total_duration = t_squared[-1] if len(t_squared) > 0 else 0.0
        noise_floor_start_time = max(t_windows[noise_floor_start_index], 0.1 * total_duration)
        if noise_floor_start_time > t_windows[-1]:
            break
        noise_floor_end_time = min(noise_floor_start_time + knee_point_time_iter, total_duration)

        noise_start = np.argmin(np.abs(t_squared - noise_floor_start_time))
        noise_end = np.argmin(np.abs(t_squared - noise_floor_end_time))
        if noise_start >= noise_end:
            break
        noise_segment = squared[noise_start:noise_end]
        if len(noise_segment) == 0:
            noise_segment = np.array([EPSILON])
        noise_floor_iter = 10 * np.log10(np.maximum(np.mean(noise_segment), EPSILON))

        slope_end_headroom = 8
        slope_dynamic_range = 20
        try:
            slope_end = np.argwhere(windows <= noise_floor_iter + slope_end_headroom)[0, 0] - 1
            slope_start = (
                np.argwhere(windows <= noise_floor_iter + slope_end_headroom + slope_dynamic_range)[0, 0]
                - 1
            )
        except IndexError:
            break

        if slope_start < 0:
            slope_start = 0
        if slope_end <= slope_start + 1:
            break
        if len(t_windows[slope_start:slope_end]) < 2:
            break

        late_slope, late_intercept, _, _, _ = stats.linregress(
            t_windows[slope_start:slope_end], windows[slope_start:slope_end]
        )
        if np.isnan(late_slope) or abs(late_slope) < EPSILON:
            break

        new_knee_point_time = (noise_floor_iter - late_intercept) / late_slope
        if len(t_windows) == 0:
            break
        new_knee_point_time = np.clip(new_knee_point_time, t_windows[0], t_windows[-1])
        try:
            new_knee_point_index = np.argwhere(t_windows >= new_knee_point_time)[0, 0]
        except IndexError:
            new_knee_point_index = len(t_windows) - 1 if len(t_windows) > 0 else 0

        if new_knee_point_index == knee_point_index_iter:
            knee_point_index_iter = new_knee_point_index
            knee_point_time_iter = t_windows[knee_point_index_iter] if len(t_windows) > 0 else 0
            break

        knee_point_index_iter = new_knee_point_index
        knee_point_time_iter = (
            t_windows[knee_point_index_iter]
            if len(t_windows) > 0 and knee_point_index_iter < len(t_windows)
            else (t_windows[-1] if len(t_windows) > 0 else 0)
        )
        knee_point_value_iter = (
            windows[knee_point_index_iter]
            if len(windows) > 0 and knee_point_index_iter < len(windows)
            else (windows[-1] if len(windows) > 0 else -200)
        )

    if len(t_squared) > 0:
        knee_point_index = np.argmin(np.abs(t_squared - knee_point_time_iter))
    else:
        knee_point_index = 0

    return peak_index, peak_index + knee_point_index, noise_floor_iter, w


def decay_times(data, fs, peak_ind=None, knee_point_ind=None, noise_floor=None, window_size=None):
    """Calculate decay times EDT, RT20, RT30, and RT60."""
    ir_data = data
    if peak_ind is None or knee_point_ind is None or noise_floor is None:
        peak_ind, knee_point_ind, noise_floor, window_size = decay_params(ir_data, fs)

    t = np.linspace(0, len(ir_data) / fs, len(ir_data))

    knee_point_ind -= peak_ind + 0
    data = ir_data[peak_ind - 0 * fs // 1000 :].copy()
    data /= np.max(np.abs(data))
    # Use the absolute-value envelope; Hilbert is unreliable for broadband signals.
    analytical = np.abs(data)

    schroeder = np.cumsum(
        analytical[knee_point_ind::-1] ** 2
        / np.sum(analytical[:knee_point_ind] ** 2)
    )[:0:-1]
    schroeder = 10 * np.log10(schroeder)

    # Moving average of the squared impulse response
    # Truncate data to avoid unnecessary computations
    # Ideally avg_head is the half window size but this might not be possible if the IR has been truncated already
    # and the peak is closer to the start than half window
    avg_head = min((window_size // 2), peak_ind)
    avg_tail = min((window_size // 2), len(ir_data) - (peak_ind + knee_point_ind))
    # We need an index offset for average curve if the avg_head is not half window
    avg_offset = window_size // 2 - avg_head
    avg = ir_data[
        peak_ind - avg_head : peak_ind + knee_point_ind + avg_tail
    ].copy()  # Truncate
    avg /= np.max(np.abs(avg))  # Normalize
    avg = avg**2
    avg = running_mean(avg, window_size)
    avg = 10 * np.log10(avg + 1e-18)
    # Find offset which minimizes difference between Schroeder backward integral and the moving average
    # ie. offset which moves Schroeder curve to same vertical position as the decay power curve
    # Limit the range 10% -> 90% of Schroeder and avg start and end
    fit_start = max(
        int(len(schroeder) * 0.1), avg_offset
    )  # avg could start after 10% of Schroeder
    fit_end = min(
        int(len(schroeder) * 0.9), avg_offset + (len(avg))
    )  # avg could end before 90% of Schroeder
    offset = np.mean(
        schroeder[fit_start:fit_end]
        - avg[
            fit_start - avg_offset : fit_end - avg_offset
        ]  # Shift avg indexes by the offset length
    )

    decay_times = dict()
    limits = [
        (-1, -10, -10, "EDT"),
        (-5, -25, -20, "RT20"),
        (-5, -35, -30, "RT30"),
        (-5, -65, -60, "RT60"),
    ]
    for start_target, end_target, decay_target, name in limits:
        decay_times[name] = None
        if end_target < noise_floor + offset + 10:
            # There has to be at least 10 dB of headroom between the end target point and noise floor,
            # in this case there is not. Current decay time shall remain undefined.
            continue
        try:
            start = np.argwhere(schroeder <= start_target)[0, 0]
            end = np.argwhere(schroeder <= end_target)[0, 0]
        except IndexError:
            # Targets not found on the Schroeder curve
            continue

        # Need at least 2 points for linear regression
        if start >= end or (end - start) < 2:
            continue

        t_slice = t[start:end]
        schroeder_slice = schroeder[start:end]

        if len(t_slice) == 0 or len(schroeder_slice) == 0:
            continue

        slope, intercept, _, _, _ = stats.linregress(t_slice, schroeder_slice)
        decay_times[name] = decay_target / slope

    return (
        decay_times["EDT"],
        decay_times["RT20"],
        decay_times["RT30"],
        decay_times["RT60"],
    )


def decay_adjustment_params(data, fs, target):
    """Calculate the array-only parameters for a decay adjustment."""
    peak_index, knee_point_index, _, _ = decay_params(data, fs)
    edt, rt20, rt30, rt60 = decay_times(data, fs)
    rt_slope = None
    # Finds largest available decay time parameter
    for rt_time, rt_level in [(edt, -10), (rt20, -20), (rt30, -30), (rt60, -60)]:
        if not rt_time:
            break
        rt_slope = rt_level / rt_time

    target_slope = -60 / target  # Target dB/s
    if target_slope > rt_slope:
        # We're not going to adjust decay and noise floor up
        return None
    knee_point_time = knee_point_index / fs
    knee_point_level = (
        rt_slope * knee_point_time
    )  # Extrapolated level at knee point
    target_level = target_slope * knee_point_time  # Target level at knee point
    window_level = target_level - knee_point_level  # Adjustment level at knee point
    window_start = peak_index + 2 * (fs // 1000)
    half_window = (
        knee_point_index - window_start
    )  # Half Hanning window length, from peak to knee
    return window_start, half_window, knee_point_index, window_level


def apply_decay_window(data, params):
    """Apply precomputed decay parameters to ``data`` in place."""
    if params is None:
        return data

    window_start, half_window, knee_point_index, window_level = params

    window = (
        np.concatenate(
            [
                np.ones(window_start),
                signal.windows.hann(half_window * 2)[half_window:],
                np.zeros(len(data) - knee_point_index),
            ]
        )
        - 1.0
    )
    window *= -window_level
    window = 10 ** (window / 20)
    data *= window
    return data
