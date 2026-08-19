# -*- coding: utf-8 -*-

import numpy as np
from scipy import signal
import nnresample
from copy import deepcopy
from autoeq.frequency_response import FrequencyResponse
from core import decay
from core.audio_io import magnitude_response
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
        # Copy only the searched range, because normalization mutates it below.
        data = self.data[start:end].copy()

        if len(data) == 0:
            return start

        max_abs_val = np.max(np.abs(data))
        if max_abs_val < EPSILON:
            return start

        data /= max_abs_val

        peaks_pos, _ = signal.find_peaks(data, height=peak_height)
        peaks_neg, _ = signal.find_peaks(data * -1.0, height=peak_height)
        peaks = np.concatenate([peaks_pos, peaks_neg])

        if len(peaks) == 0:
            return np.argmax(np.abs(data)) + start

        peaks += start
        return np.min(peaks)

    def decay_params(self):
        return decay.decay_params(self.data, self.fs)

    def decay_times(
        self, peak_ind=None, knee_point_ind=None, noise_floor=None, window_size=None
    ):
        return decay.decay_times(
            self.data, self.fs, peak_ind, knee_point_ind, noise_floor, window_size
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

    def shift(self, samples):
        """Shift the impulse response in time, preserving array length.

        ``samples > 0`` delays the response (prepends ``samples`` zeros and
        truncates the tail); ``samples < 0`` advances it (drops the leading
        ``-samples`` samples and zero-pads the tail); ``samples == 0`` is a
        no-op. The numpy ops are kept byte-identical across call sites so
        BRIR output is unchanged.
        """
        n = len(self.data)
        if samples > 0:
            self.data = np.concatenate((np.zeros(samples), self.data))[:n]
        elif samples < 0:
            trimmed = self.data[-samples:]
            if len(trimmed) < n:
                trimmed = np.pad(trimmed, (0, n - len(trimmed)))
            self.data = trimmed

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

    def decay_adjustment_params(self, target):
        return decay.decay_adjustment_params(self.data, self.fs, target)

    def adjust_decay(self, target):
        """Adjusts decay time in place.

        Args:
            target: Target 60 dB decay time in seconds

        Returns:
            None
        """
        from core.decay import apply_decay_window

        apply_decay_window(self.data, self.decay_adjustment_params(target))

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
