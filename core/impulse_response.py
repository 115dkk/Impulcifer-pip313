# -*- coding: utf-8 -*-

import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
from scipy.signal import spectrogram
from matplotlib.ticker import LinearLocator, FormatStrFormatter, FuncFormatter
from mpl_toolkits.axes_grid1 import make_axes_locatable
from scipy import signal, stats, ndimage, interpolate
from scipy.signal.windows import hann
import nnresample
from copy import deepcopy
from autoeq.frequency_response import FrequencyResponse
from core.utils import magnitude_response, get_ylim, running_mean
from core.constants import COLORS
import os

EPSILON = 1e-20


class ImpulseResponse:
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

    def plot(
        self,
        fig=None,
        ax=None,
        plot_file_path=None,
        plot_recording=True,
        plot_spectrogram=True,
        plot_ir=True,
        plot_fr=True,
        plot_decay=True,
        plot_waterfall=True,
    ):
        """Plots all plots into the same figure

        Args:
            fig: Figure instance
            ax: Axes instance, must have 2 rows and 3 columns
            plot_file_path: Path to a file for saving the plot
            plot_recording: Plot recording waveform?
            plot_spectrogram: Plot recording spectrogram?
            plot_ir: Plot impulse response?
            plot_fr: Plot frequency response?
            plot_decay: Plot decay curve?
            plot_waterfall: Plot waterfall graph?

        Returns:
            Figure
        """
        if fig is None:
            # Create figure and axises for the plots
            fig = plt.figure()
            fig.set_size_inches(22, 10)
            ax = []
            for i in range(5):
                ax.append(fig.add_subplot(2, 3, i + 1))
            ax.append(fig.add_subplot(2, 3, 6, projection="3d"))
            ax = np.vstack([ax[:3], ax[3:]])
        if plot_recording:
            self.plot_recording(fig=fig, ax=ax[0, 0])
        if plot_spectrogram:
            self.plot_spectrogram(fig=fig, ax=ax[1, 0])
        if plot_ir:
            self.plot_ir(fig=fig, ax=ax[0, 1])
        if plot_fr:
            self.plot_fr(fig=fig, ax=ax[1, 1])
        if plot_decay:
            self.plot_decay(fig=fig, ax=ax[0, 2])
        if plot_waterfall:
            self.plot_waterfall(fig=fig, ax=ax[1, 2])
        if plot_file_path:
            # Ensure the directory exists before saving
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)
        return fig

    def plot_recording(self, fig=None, ax=None, plot_file_path=None):
        """Plots recording wave form

        Args:
            fig: Figure instance
            ax: Axes instance
            plot_file_path: Path to a file for saving the plot

        Returns:
            - Figure
            - Axes
        """
        if self.recording is None or len(np.nonzero(self.recording)[0]) == 0:
            return
        if fig is None:
            fig, ax = plt.subplots()

        t = np.linspace(0, len(self.recording) / self.fs, len(self.recording))
        ax.plot(t, self.recording, color=COLORS["blue"], linewidth=0.5)

        ax.grid(True)
        ax.set_xlabel("Time (s)")
        ax.set_ylabel("Amplitude")
        ax.set_title("Sine Sweep")

        # Save image
        if plot_file_path:
            # Ensure the directory exists before saving
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_spectrogram(
        self, fig=None, ax=None, plot_file_path=None, f_res=10, n_segments=200
    ):
        """Plots spectrogram of the recorded sweep.

        Args:
            fig: Matplotlib figure. If None, a new figure will be created
            ax: Matplotlib axis. If None, a new axis will be created
            plot_file_path: Path to save the plot to
            f_res: Frequency resolution in Hz
            n_segments: Number of spectrogram segments

        Returns:
            None
        """
        if self.recording is None:
            return
        if fig is None or ax is None:
            fig, ax = plt.subplots(figsize=(16 / 2.54, 9 / 2.54))

        # Spectrogram parameters
        target_f_res_nfft = round(self.fs / f_res)  # 주파수 해상도 목표 nfft
        min_time_segments = 3  # 최소한 3개의 시간축 세그먼트를 목표

        if len(self.recording) == 0:
            print(
                f"  Warning: self.recording is empty. Skipping spectrogram for {self.name if hasattr(self, 'name') else 'current IR'}."
            )
            if ax is not None:
                ax.text(
                    0.5,
                    0.5,
                    "Recording data is empty.",
                    horizontalalignment="center",
                    verticalalignment="center",
                    transform=ax.transAxes,
                )
            return fig, ax

        # nfft가 너무 커서 시간축이 안나오는 것을 방지 (min_time_segments 확보 위함)
        max_nfft_for_segments = (
            (2 * len(self.recording)) // (min_time_segments + 1)
            if min_time_segments > 0
            else len(self.recording)
        )
        if max_nfft_for_segments <= 0:  # 예를 들어 self.recording이 너무 짧으면
            max_nfft_for_segments = len(self.recording)

        nfft = target_f_res_nfft
        if nfft > max_nfft_for_segments and max_nfft_for_segments > 0:
            print(
                f"  Adjusting nfft from {nfft} to {max_nfft_for_segments} to ensure at least {min_time_segments} time segments (f_res will be higher)."
            )
            nfft = max_nfft_for_segments

        if nfft > len(self.recording):  # nfft는 녹음 길이보다 클 수 없음
            nfft = len(self.recording)

        if nfft == 0:  # nfft가 0이 되면 spectrogram 함수에서 오류 발생
            print(
                f"  Warning: nfft is 0 after adjustment. Skipping spectrogram for {self.name if hasattr(self, 'name') else 'current IR'}."
            )
            if ax is not None:
                ax.text(
                    0.5,
                    0.5,
                    "Spectrogram nfft is 0.",
                    horizontalalignment="center",
                    verticalalignment="center",
                    transform=ax.transAxes,
                )
            return fig, ax

        # noverlap 계산: n_segments를 사용하여 (len(recording) - nfft) / n_segments 가 step_size가 되도록 시도
        if n_segments > 0 and (len(self.recording) - nfft) > 0:
            step_size = (len(self.recording) - nfft) / n_segments
            if (
                step_size <= 1
            ):  # step_size가 너무 작거나 음수면 (n_segments가 너무 크거나, len(rec) <= nfft)
                # step_size는 1 이상이어야 noverlap < nfft 가 일반적으로 보장됨 (noverlap = nfft - step_size)
                noverlap = nfft // 2  # 기본 50% 오버랩으로 후퇴
                print(
                    f"  Info: Calculated step_size ({step_size:.2f}) for noverlap is too small or invalid. Falling back to 50% overlap."
                )
            else:
                noverlap = int(nfft - step_size)
                print(
                    f"  Info: Calculated noverlap using n_segments. step_size: {step_size:.2f}"
                )
        else:
            noverlap = nfft // 2  # 기본 50% 오버랩
            if n_segments <= 0:
                print(
                    f"  Info: n_segments ({n_segments}) is not positive. Using 50% overlap for noverlap calculation."
                )
            else:  # (len(self.recording) - nfft) <= 0
                print(
                    f"  Info: len(self.recording) ({len(self.recording)}) <= nfft ({nfft}). Using 50% overlap."
                )

        # noverlap 안전 장치
        if noverlap >= nfft:
            print(
                f"  Warning: Calculated noverlap ({noverlap}) was >= nfft ({nfft}). Setting to nfft - 1 (or 0 if nfft is 1)."
            )
            noverlap = max(0, nfft - 1)
        if noverlap < 0:
            print(f"  Warning: Calculated noverlap ({noverlap}) was < 0. Setting to 0.")
            noverlap = 0

        # print(f"Debug plot_spectrogram for {self.name if hasattr(self, 'name') else 'current IR'}:")
        # print(f"  self.fs: {self.fs}, len(self.recording): {len(self.recording)}")
        # print(f"  f_res (target): {f_res}, n_segments (for noverlap): {n_segments}")
        # print(f"  nfft: {nfft}, noverlap: {noverlap}")
        # print(f"  Input for spectrogram: recording.shape={self.recording.shape}, recording.ndim={self.recording.ndim}")

        # Get spectrogram data
        window_arg = signal.get_window("hann", nfft)  # 명시적으로 Hann 윈도우 사용
        # 올바른 반환 순서: f, t, Sxx
        freqs, t, spectrum = spectrogram(
            self.recording,
            fs=self.fs,
            window=window_arg,
            nperseg=nfft,  # nfft를 nperseg로 사용
            noverlap=noverlap,
            mode="psd",
        )
        # print(f"Debug: freqs.shape={freqs.shape}, t.shape={t.shape}, spectrum.shape={spectrum.shape}") # 디버깅용 추가 출력

        if (
            spectrum.ndim != 2 or spectrum.shape[0] <= 1 or spectrum.shape[1] == 0
        ):  # spectrum이 2D가 아니거나, 주파수 빈이 하나 이하이거나, 시간 축이 없으면
            # print(f"Warning: Spectrogram data is not suitable for plotting (actual spectrum shape: {spectrum.shape}, nfft: {nfft}, noverlap: {noverlap}, window: {window_arg.shape if hasattr(window_arg, 'shape') else 'default'}). Skipping spectrogram.")
            if ax is not None:
                ax.text(
                    0.5,
                    0.5,
                    f"Spectrogram not available\n(shape: {spectrum.shape})",
                    horizontalalignment="center",
                    verticalalignment="center",
                    transform=ax.transAxes,
                )
            return fig, ax

        # Remove zero frequency
        f = freqs[1:]
        z = spectrum[1:, :]

        if z.size == 0:
            print(
                f"Warning: Spectrogram data became empty after removing zero frequency for {self.name if hasattr(self, 'name') else 'current IR'}. Skipping spectrogram."
            )
            if ax is not None:
                ax.text(
                    0.5,
                    0.5,
                    "Spectrogram data empty.",
                    horizontalalignment="center",
                    verticalalignment="center",
                    transform=ax.transAxes,
                )
            return fig, ax

        z = 10 * np.log10(
            np.abs(z) + 1e-9
        )  # np.abs() 와 작은 값(1e-9)을 더해 log(0) 방지

        # Create spectrogram image
        # t와 f는 1D 배열이고, np.meshgrid를 통해 t_mesh, f_mesh (2D)를 만듭니다.
        # z는 이미 2D 배열입니다 (spectrum[1:, :]).
        # z의 shape은 (len(f), len(t)) 여야 합니다.
        t_mesh, f_mesh = np.meshgrid(t, f)

        if z.shape[0] != len(f) or z.shape[1] != len(t):
            print(
                f"Error: Shape mismatch for pcolormesh. z shape: {z.shape}, expected: ({len(f)}, {len(t)}) for {self.name if hasattr(self, 'name') else 'current IR'}."
            )
            if ax is not None:
                ax.text(
                    0.5,
                    0.5,
                    "Spectrogram shape error.",
                    horizontalalignment="center",
                    verticalalignment="center",
                    transform=ax.transAxes,
                )
            return fig, ax

        cs = ax.pcolormesh(
            t_mesh, f_mesh, z, cmap="gnuplot2", vmin=-150, shading="auto"
        )

        divider = make_axes_locatable(ax)
        cax = divider.append_axes("right", size="5%", pad=0.05)
        fig.colorbar(cs, cax=cax)

        ax.semilogy()
        ax.yaxis.set_major_formatter(ticker.StrMethodFormatter("{x:.0f}"))
        ax.set_xlabel("Time (s)")
        ax.set_ylabel("Frequency (Hz)")
        ax.set_title("Spectrogram")

        # Save image
        if plot_file_path:
            # Ensure the directory exists before saving
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_ir(self, fig=None, ax=None, start=0.0, end=None, plot_file_path=None):
        """Plots impulse response wave form.

        Args:
            fig: Figure instance
            ax: Axis instance
            start: Start of the plot in seconds
            end: End of the plot in seconds
            plot_file_path: Path to a file for saving the plot

        Returns:
            None
        """
        if end is None:
            end = len(self.data) / self.fs
        ir = self.data[int(start * self.fs) : int(end * self.fs)]

        if fig is None:
            fig, ax = plt.subplots()
        t = np.arange(
            start * 1000, start * 1000 + 1000 / self.fs * len(ir), 1000 / self.fs
        )
        ax.plot(t, ir, color=COLORS["blue"], linewidth=0.5)
        ax.set_xlabel("Time (ms)")
        ax.set_ylabel("Amplitude")
        ax.grid(True)
        ax.set_title("Impulse response".format())

        if plot_file_path:
            # Ensure the directory exists before saving
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_fr(
        self,
        fr=None,
        fig=None,
        ax=None,
        plot_file_path=None,
        plot_raw=True,
        raw_color="#7db4db",
        plot_smoothed=True,
        smoothed_color="#1f77b4",
        plot_error=True,
        error_color="#dd8081",
        plot_error_smoothed=True,
        error_smoothed_color="#d62728",
        plot_target=True,
        target_color="#ecdef9",
        plot_equalization=True,
        equalization_color="#2ca02c",
        plot_equalized=True,
        equalized_color="#680fb9",
        fix_ylim=False,
    ):
        """Plots frequency response

        Args:
            fr: FrequencyResponse instance. Useful for passing instance with taget, error, equalization etc...
            fig: Figure instance
            ax: Axes instance
            plot_file_path: Path to a file for saving the plot
            plot_raw: Include raw curve?
            raw_color: Color of raw curve
            plot_smoothed: Include smoothed curve?
            smoothed_color: Color of smoothed curve
            plot_error: Include unsmoothed error curve?
            error_color: Color of error curve
            plot_error_smoothed: Include smoothed error curve?
            error_smoothed_color: Color of smoothed error curve
            plot_target: Include target curve?
            target_color: Color of target curve
            plot_equalization: Include equalization curve?
            equalization_color: Color of equalization curve
            plot_equalized: Include equalized curve?
            equalized_color: Color of equalized curve
            fix_ylim: Fix Y-axis limits calculation?

        Returns:
            - Figure
            - Axes
        """
        if fr is None:
            fr = self.frequency_response()
            fr.smoothen(
                window_size=1 / 3,
                treble_f_lower=20000,
                treble_f_upper=23999,
                treble_window_size=1 / 3,
            )
        if fig is None:
            fig, ax = plt.subplots()
        ax.set_xlabel("Frequency (Hz)")
        ax.semilogx()
        ax.set_xlim([20, 20e3])
        ax.set_ylabel("Amplitude (dB)")
        ax.set_title(fr.name)
        ax.grid(True, which="major")
        ax.grid(True, which="minor")
        ax.xaxis.set_major_formatter(ticker.StrMethodFormatter("{x:.0f}"))
        legend = []
        v = []
        sl = np.logical_and(fr.frequency >= 20, fr.frequency <= 20000)

        if plot_target and len(fr.target):
            ax.plot(fr.frequency, fr.target, linewidth=5, color=target_color)
            legend.append("Target")
            v.append(fr.target[sl])
        if plot_raw and len(fr.raw):
            ax.plot(fr.frequency, fr.raw, linewidth=0.5, color=raw_color)
            legend.append("Raw")
            v.append(fr.raw[sl])
        if plot_error and len(fr.error):
            ax.plot(fr.frequency, fr.error, linewidth=0.5, color=error_color)
            legend.append("Error")
            v.append(fr.error[sl])
        if plot_smoothed and len(fr.smoothed):
            ax.plot(fr.frequency, fr.smoothed, linewidth=1, color=smoothed_color)
            legend.append("Raw Smoothed")
            v.append(fr.smoothed[sl])
        if plot_error_smoothed and len(fr.error_smoothed):
            ax.plot(
                fr.frequency, fr.error_smoothed, linewidth=1, color=error_smoothed_color
            )
            legend.append("Error Smoothed")
            v.append(fr.error_smoothed[sl])
        if plot_equalization and len(fr.equalization):
            ax.plot(
                fr.frequency, fr.equalization, linewidth=1, color=equalization_color
            )
            legend.append("Equalization")
            v.append(fr.equalization[sl])
        if plot_equalized and len(fr.equalized_raw) and not len(fr.equalized_smoothed):
            ax.plot(fr.frequency, fr.equalized_raw, linewidth=1, color=equalized_color)
            legend.append("Equalized raw")
            v.append(fr.equalized_raw[sl])
        if plot_equalized and len(fr.equalized_smoothed):
            ax.plot(
                fr.frequency, fr.equalized_smoothed, linewidth=1, color=equalized_color
            )
            legend.append("Equalized smoothed")
            v.append(fr.equalized_smoothed[sl])

        if fix_ylim:
            # Y axis limits
            lower, upper = get_ylim(v)
            ax.set_ylim([lower, upper])

        ax.legend(legend, fontsize=8)

        if plot_file_path:
            # Ensure the directory exists before saving
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_decay(self, fig=None, ax=None, plot_file_path=None):
        """Plots decay graph.

        Args:
            fig: Figure instance. New will be created if None is passed.
            ax: Axis instance. New will be created if None is passed to fig.
            plot_file_path: Save plot figure to a file.

        Returns:
            - Figure
            - Axes
        """
        if fig is None:
            fig, ax = plt.subplots()

        peak_ind, knee_point_ind, noise_floor, window_size = self.decay_params()

        start = max(0, (peak_ind - 2 * (knee_point_ind - peak_ind)))
        end = min(len(self), (peak_ind + 2 * (knee_point_ind - peak_ind)))
        t = np.arange(start, end) / self.fs

        squared = self.data.copy()
        squared /= np.max(np.abs(squared))
        squared = squared[start:end] ** 2
        avg = running_mean(squared, window_size)
        squared = 10 * np.log10(squared + 1e-24)
        avg = 10 * np.log10(avg + 1e-24)

        ax.plot(
            t * 1000,
            squared,
            color=COLORS["lightblue"],
            label="Squared impulse response",
        )
        ax.plot(
            t[window_size // 2 : window_size // 2 + len(avg)] * 1000,
            avg,
            color=COLORS["blue"],
            label=f"{window_size / self.fs * 1000:.0f} ms moving average",
        )

        ax.set_ylim([np.min(avg) * 1.2, 0])
        ax.set_xlim([start / self.fs * 1000, end / self.fs * 1000])
        ax.set_xlabel("Time (ms)")
        ax.set_ylabel("Amplitude (dBr)")
        ax.grid(True, which="major")
        ax.set_title("Decay")
        ax.legend(loc="upper right")

        if plot_file_path:
            # Ensure the directory exists before saving
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_waterfall(self, fig=None, ax=None):
        """Plots decay waterfall.

        Args:
            fig: Figure instance
            ax: Axis instance

        Returns:
            - Figure
            - Axis
        """
        if fig is None:
            fig = plt.figure()
            ax = fig.add_subplot(111, projection="3d")

        # Logarithmic sine sweep is the input signal
        window_length = 512
        hann(window_length)
        # 50% overlap means hop length = window length / 2
        hop_length = int(window_length / 2)
        # Copy and pad with zeros to include a few extra frames after the impulse response
        data = np.copy(self.data)
        s = np.zeros(hop_length * 5 + window_length)

        # s 배열의 크기를 넘지 않도록 data의 앞부분만 복사
        copy_len = min(len(data), len(s))
        s[0:copy_len] = data[0:copy_len]

        data = s  # 이제 data는 s와 동일한 (작아진) 데이터를 참조
        # Minimum number of frames
        n_min = 40
        # 50% overlap means hop length = window length / 2
        max(int(len(data) / hop_length) - 1, n_min)
        # n_segments = max(int(len(data) / hop_length) - 1, n_min) # 기존 n_segments 계산
        # nfft = window_length # 기존 nfft (512)
        # noverlap = nfft - hop_length # 기존 noverlap (256)

        # waterfall 용 nfft, noverlap 재조정 (len(data)가 짧은 것을 고려)
        nfft = 256
        if nfft > len(data):  # 데이터 길이보다 nfft가 클 수 없음
            nfft = len(data)

        if nfft == 0:
            print(
                f"Warning: Waterfall's nfft is 0. Skipping waterfall plot for {self.name if hasattr(self, 'name') else 'current IR'}."
            )
            if ax is not None:
                ax.text2D(
                    0.5,
                    0.5,
                    "Waterfall nfft is 0.",
                    transform=ax.transAxes,
                    ha="center",
                    va="center",
                )
            return fig, ax

        noverlap = nfft // 2  # 50% overlap
        if noverlap >= nfft:
            noverlap = max(0, nfft - 1)
        if noverlap < 0:
            noverlap = 0

        window_arg = signal.get_window("hann", nfft)  # 명시적으로 Hann 윈도우 사용

        # print(f"Debug plot_waterfall for {self.name if hasattr(self, 'name') else 'current IR'}:")
        # print(f"  self.fs: {self.fs}, len(data): {len(data)}")
        # print(f"  nfft (adjusted): {nfft}, noverlap (adjusted): {noverlap}")
        # print(f"  Input for waterfall spectrogram: data.shape={data.shape}, data.ndim={data.ndim}")

        # Get spectrogram data
        freqs, t, spectrum = spectrogram(
            data,
            fs=self.fs,
            window=window_arg,
            nperseg=nfft,
            noverlap=noverlap,
            mode="magnitude",
        )
        # print(f"Debug waterfall: freqs.shape={freqs.shape}, t.shape={t.shape}, spectrum.shape={spectrum.shape}") # 디버깅용 추가 출력

        if spectrum.ndim != 2 or spectrum.shape[0] <= 1 or spectrum.shape[1] == 0:
            # print(f"Warning: Waterfall's spectrogram data is not suitable for plotting (actual spectrum shape: {spectrum.shape}, nfft: {nfft}, noverlap: {noverlap}). Skipping waterfall plot.")
            if ax is not None:
                ax.text2D(
                    0.5,
                    0.5,
                    "Waterfall data not available.",
                    transform=ax.transAxes,
                    ha="center",
                    va="center",
                )
            return fig, ax

        # Remove 0 Hz component
        spectrum = spectrum[1:, :]
        freqs = freqs[1:]

        if spectrum.size == 0 or freqs.size == 0:
            print(
                f"Warning: Waterfall's spectrogram data became empty after removing zero frequency for {self.name if hasattr(self, 'name') else 'current IR'}. Skipping waterfall plot."
            )
            if ax is not None:
                ax.text2D(
                    0.5,
                    0.5,
                    "Waterfall data empty.",
                    transform=ax.transAxes,
                    ha="center",
                    va="center",
                )
            return fig, ax

        # Interpolate to logaritmic frequency scale
        f_max = self.fs / 2
        f_min = 10
        step = 1.03
        # Phase 3 optimization: Vectorize frequency array generation
        n_freqs = int(np.log(f_max / f_min) / np.log(step))
        f = f_min * step ** np.arange(n_freqs)
        log_f_spec = np.ones((len(f), spectrum.shape[1]))
        for i in range(spectrum.shape[1]):
            interpolator = interpolate.InterpolatedUnivariateSpline(
                np.log10(freqs), spectrum[:, i], k=1
            )
            log_f_spec[:, i] = interpolator(np.log10(f))
        z = log_f_spec
        f = np.log10(f)

        # Normalize and turn to dB scale
        z /= np.max(z)
        z = np.clip(z, 10 ** (-100 / 20), np.max(z))
        z = 20 * np.log10(z)

        # Smoothen
        z = ndimage.uniform_filter(z, size=3, mode="constant")
        t, f = np.meshgrid(t, f)

        # Smoothing creates "walls", remove them
        t = t[1:-1, :-1] * 1000  # Milliseconds
        f = f[1:-1, :-1]
        z = z[1:-1, :-1]

        # Surface plot
        ax.plot_surface(
            t,
            f,
            z,
            rcount=len(t),
            ccount=len(f),
            cmap="magma",
            antialiased=True,
            vmin=-100,
            vmax=0,
        )

        # Z axis
        ax.set_zlim([-100, 0])
        ax.zaxis.set_major_locator(LinearLocator(10))
        ax.zaxis.set_major_formatter(FormatStrFormatter("%.02f"))

        # X axis
        if t.size > 0:
            ax.set_xlim([0, np.max(t) * 1000])  # t의 최대값을 ms 단위로 사용
        else:
            ax.set_xlim([0, 100])  # t가 비어있을 경우 기본값 (예시)
        ax.set_xlabel("Time (ms)")

        # Y axis
        ax.set_ylim(np.log10([20, 20000]))
        ax.set_ylabel("Frequency (Hz)")
        ax.yaxis.set_major_formatter(FuncFormatter(lambda x, p: f"{10**x:.0f}"))

        # Orient
        ax.view_init(30, 30)

        return fig, ax
