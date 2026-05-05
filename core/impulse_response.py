# -*- coding: utf-8 -*-

import numpy as np
from scipy import signal, stats
from scipy.signal.windows import hann
import nnresample
from copy import deepcopy
from autoeq.frequency_response import FrequencyResponse
from core.utils import magnitude_response, running_mean
from core.plotting.impulse_response_plotter import ImpulseResponsePlotter

EPSILON = 1e-20


class ImpulseResponse(ImpulseResponsePlotter):
    def __init__(self, data, fs, recording=None):
        self.fs = fs
        self.data = data
        self.recording = recording

    def copy(self):
        return deepcopy(self)

    def __len__(self):
        """Impulse response length in samples."""
        return len(self.data)

    def duration(self):
        """Impulse response duration in seconds."""
        return len(self) / self.fs

    def peak_index(self, start=0, end=None, peak_height=0.12589):
        """Finds the first high (negative or positive) peak in the impulse response wave form.

        Args:
            start: Index for start of search range
            end: Index for end of search range
            peak_height: Minimum peak height. Default is -18 dBFS

        Returns:
            Peak index to impulse response data.
        """
        if len(self.data) == 0:
            return 0

        if end is None:
            end = len(self.data)

        # Peak height threshold, relative to the data maximum value
        # Copy to avoid manipulating the original data here
        data = self.data.copy()
        # Limit search to given range
        data = data[start:end]

        if len(data) == 0:
            return start

        max_abs_val = np.max(np.abs(data))
        if max_abs_val < EPSILON:
            return start

        # Normalize to 1.0
        data /= max_abs_val

        # Find positive peaks
        peaks_pos, _ = signal.find_peaks(data, height=peak_height)
        # Find negative peaks that are at least
        peaks_neg, _ = signal.find_peaks(data * -1.0, height=peak_height)
        # Combine positive and negative peaks
        peaks = np.concatenate([peaks_pos, peaks_neg])

        if len(peaks) == 0:
            return np.argmax(np.abs(data)) + start

        # Add start delta to peak indices
        peaks += start
        # Return the first one
        return np.min(peaks)

    def decay_params(self):
        """Determines decay parameters with Lundeby method

        https://www.ingentaconnect.com/content/dav/aaua/1995/00000081/00000004/art00009
        http://users.spa.aalto.fi/mak/PUB/AES_Modal9992.pdf

        Returns:
            - peak_ind: Fundamental starting index
            - knee_point_ind: Index where decay reaches noise floor
            - noise_floor: Noise floor in dBFS, also peak to noise ratio
            - window_size: Averaging window size as determined by Lundeby method
        """
        if len(self.data) < 10:
            return 0, len(self.data), -200.0, len(self.data) if len(self.data) > 0 else 1

        peak_index = self.peak_index()

        # Analyze from the peak to at most two seconds after it.
        data = self.data.copy()
        analysis_end = min(peak_index + int(2 * self.fs), len(self))
        if peak_index >= analysis_end:
            if peak_index >= len(self.data):
                peak_index = len(self.data) - 1
            if peak_index < 0:
                peak_index = 0
            data = data[peak_index : peak_index + 1] if peak_index < len(data) else np.array([EPSILON])
        else:
            data = data[peak_index:analysis_end]

        if len(data) == 0:
            data = np.array([EPSILON])

        max_abs = np.max(np.abs(data))
        if max_abs >= EPSILON:
            data = data / max_abs

        squared = data**2
        if len(squared) == 0:
            return peak_index, len(self.data), -200.0, 100

        t_squared = np.linspace(0, len(squared) / self.fs, len(squared))

        wd = 0.03
        n = int(len(squared) / self.fs / wd) if self.fs > 0 and wd > 0 else 0
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

        if self.fs <= 0 or wd <= EPSILON:
            n = 1
        else:
            n = int(len(squared) / self.fs / wd)
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
                    np.argwhere(windows <= noise_floor_iter + slope_end_headroom + slope_dynamic_range)[0, 0] - 1
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

    def decay_times(
        self, peak_ind=None, knee_point_ind=None, noise_floor=None, window_size=None
    ):
        """Calculates decay times EDT, RT20, RT30, RT60

        Args:
            peak_ind: Peak index as returned by `decay_params()`. Optional.
            knee_point_ind: Knee point index as returned by `decay_params()`. Optional.
            noise_floor: Noise floor as returned by `decay_params()`. Optional.
            window_size: Moving average window size as returned by `decay_params()`. Optional.

        Returns:
            - EDT, None if SNR < 10 dB
            - RT20, None if SNR < 35 dB
            - RT30, None if SNR < 45 dB
            - RT60, None if SNR < 75 dB

        """
        if peak_ind is None or knee_point_ind is None or noise_floor is None:
            peak_ind, knee_point_ind, noise_floor, window_size = self.decay_params()

        t = np.linspace(0, self.duration(), len(self))

        knee_point_ind -= peak_ind + 0
        data = self.data.copy()
        data = data[peak_ind - 0 * self.fs // 1000 :]
        data /= np.max(np.abs(data))
        # analytical = np.abs(signal.hilbert(data))  # Hilbert doesn't work will with broadband signa
        analytical = np.abs(data)

        schroeder = np.cumsum(
            analytical[knee_point_ind::-1] ** 2
            / np.sum(analytical[:knee_point_ind] ** 2)
        )[:0:-1]
        schroeder = 10 * np.log10(schroeder)

        # Moving average of the squared impulse response
        avg = self.data.copy()
        # Truncate data to avoid unnecessary computations
        # Ideally avg_head is the half window size but this might not be possible if the IR has been truncated already
        # and the peak is closer to the start than half window
        avg_head = min((window_size // 2), peak_ind)
        avg_tail = min((window_size // 2), len(avg) - (peak_ind + knee_point_ind))
        # We need an index offset for average curve if the avg_head is not half window
        avg_offset = window_size // 2 - avg_head
        avg = avg[
            peak_ind - avg_head : peak_ind + knee_point_ind + avg_tail
        ]  # Truncate
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

            # Check if we have valid data range for linear regression
            if start >= end or (end - start) < 2:
                # Need at least 2 points for linear regression
                continue

            # Check if the sliced arrays are not empty
            t_slice = t[start:end]
            schroeder_slice = schroeder[start:end]

            if len(t_slice) == 0 or len(schroeder_slice) == 0:
                # Empty arrays, skip this decay time calculation
                continue

            slope, intercept, _, _, _ = stats.linregress(t_slice, schroeder_slice)
            decay_times[name] = decay_target / slope

        return (
            decay_times["EDT"],
            decay_times["RT20"],
            decay_times["RT30"],
            decay_times["RT60"],
        )

    def crop_head(self, head_ms=1):
        """Crops away head."""
        if len(self.data) == 0:
            return
        peak_idx = self.peak_index()
        crop_start = peak_idx - int(self.fs * head_ms / 1000)
        if crop_start < 0:
            crop_start = 0
        self.data = self.data[crop_start:]

    def equalize(self, fir):
        """Equalizes this impulse response with give FIR filter.

        Args:
            fir: FIR filter as an single dimensional array

        Returns:
            None
        """
        self.data = signal.convolve(self.data, fir, mode="full")

    def resample(self, fs):
        """Resamples this impulse response to the given sampling rate."""
        self.data = nnresample.resample(self.data, fs, self.fs)
        self.fs = fs

    def convolve(self, x):
        """Convolves input data with this impulse response

        Args:
            x: Input data to be convolved

        Returns:
            Convolved data
        """
        return signal.convolve(x, self.data, mode="full")

    def adjust_decay(self, target):
        """Adjusts decay time in place.

        Args:
            target: Target 60 dB decay time in seconds

        Returns:
            None
        """
        peak_index, knee_point_index, _, _ = self.decay_params()
        edt, rt20, rt30, rt60 = self.decay_times()
        rt_slope = None
        # Finds largest available decay time parameter
        for rt_time, rt_level in [(edt, -10), (rt20, -20), (rt30, -30), (rt60, -60)]:
            if not rt_time:
                break
            rt_slope = rt_level / rt_time

        target_slope = -60 / target  # Target dB/s
        if target_slope > rt_slope:
            # We're not going to adjust decay and noise floor up
            return
        knee_point_time = knee_point_index / self.fs
        knee_point_level = (
            rt_slope * knee_point_time
        )  # Extrapolated level at knee point
        target_level = target_slope * knee_point_time  # Target level at knee point
        window_level = target_level - knee_point_level  # Adjustment level at knee point
        window_start = peak_index + 2 * (self.fs // 1000)
        half_window = (
            knee_point_index - window_start
        )  # Half Hanning window length, from peak to knee
        window = (
            np.concatenate(
                [  # Adjustment window
                    np.ones(window_start),  # Start with ones until peak
                    hann(half_window * 2)[half_window:],  # Slope down to knee point
                    np.zeros(
                        len(self) - knee_point_index
                    ),  # Fill with zeros to full length
                ]
            )
            - 1.0
        )  # Slopes down from 0.0 to -1.0
        window *= -window_level  # Scale with adjustment level at knee point
        window = 10 ** (window / 20)  # Linear scale
        self.data *= window  # Scale impulse response data wit the window

    def magnitude_response(self):
        """Calculates magnitude response for the data."""
        return magnitude_response(self.data, self.fs)

    def frequency_response(self):
        """Creates FrequencyResponse instance."""
        if len(self.data) < 2:
            frequency = FrequencyResponse.generate_frequencies(f_step=1.01, f_min=10, f_max=self.fs / 2)
            return FrequencyResponse(name="Frequency response (short IR)", frequency=frequency, raw=np.zeros_like(frequency))

        f, m = self.magnitude_response()
        if len(f) == 0:
            frequency = FrequencyResponse.generate_frequencies(f_step=1.01, f_min=10, f_max=self.fs / 2)
            return FrequencyResponse(name="Frequency response (empty FFT)", frequency=frequency, raw=np.zeros_like(frequency))

        target_fr_points = (self.fs / 2) / 4.0
        if target_fr_points < 2 or len(f) < 2:
            step = 1
        else:
            step = int(round(len(f) / target_fr_points))
            if step == 0:
                step = 1

        if len(f[1::step]) == 0:
            frequency = f[1:]
            raw = m[1:]
            if len(frequency) == 0:
                frequency = FrequencyResponse.generate_frequencies(f_step=1.01, f_min=10, f_max=self.fs / 2)
                return FrequencyResponse(name="Frequency response (FFT too short)", frequency=frequency, raw=np.zeros_like(frequency))
        else:
            frequency = f[1::step]
            raw = m[1::step]

        fr = FrequencyResponse(name="Frequency response", frequency=frequency, raw=raw)
        fr.interpolate(f_step=1.01, f_min=10, f_max=self.fs / 2)
        return fr
