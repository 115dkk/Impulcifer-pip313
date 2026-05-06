# -*- coding: utf-8 -*-
"""Plotting mixin for :class:`core.impulse_response.ImpulseResponse`.

Hosts every matplotlib visualization that used to live directly on
``ImpulseResponse``. The mixin pattern keeps the public API identical while
moving ~700 lines out of ``core/impulse_response.py``.
"""

import os

import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
from matplotlib.ticker import LinearLocator, FormatStrFormatter, FuncFormatter
from mpl_toolkits.axes_grid1 import make_axes_locatable
from scipy import signal, ndimage, interpolate
from scipy.signal import spectrogram
from scipy.signal.windows import hann

from core.utils import get_ylim, running_mean
from core.constants import COLORS


class ImpulseResponsePlotter:
    """Mixin providing matplotlib plotting for ``ImpulseResponse``.

    Methods access ``self.data``, ``self.fs`` and ``self.recording`` defined on
    the host class and remain backward compatible with the previous in-class
    implementation.
    """

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
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)
        return fig

    def plot_recording(self, fig=None, ax=None, plot_file_path=None):
        """Plots recording wave form."""
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

        if plot_file_path:
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_spectrogram(
        self, fig=None, ax=None, plot_file_path=None, f_res=10, n_segments=200
    ):
        """Plots spectrogram of the recorded sweep."""
        if self.recording is None:
            return
        if fig is None or ax is None:
            fig, ax = plt.subplots(figsize=(16 / 2.54, 9 / 2.54))

        target_f_res_nfft = round(self.fs / f_res)
        min_time_segments = 3

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

        max_nfft_for_segments = (
            (2 * len(self.recording)) // (min_time_segments + 1)
            if min_time_segments > 0
            else len(self.recording)
        )
        if max_nfft_for_segments <= 0:
            max_nfft_for_segments = len(self.recording)

        nfft = target_f_res_nfft
        if nfft > max_nfft_for_segments and max_nfft_for_segments > 0:
            print(
                f"  Adjusting nfft from {nfft} to {max_nfft_for_segments} to ensure at least {min_time_segments} time segments (f_res will be higher)."
            )
            nfft = max_nfft_for_segments

        if nfft > len(self.recording):
            nfft = len(self.recording)

        if nfft == 0:
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

        if n_segments > 0 and (len(self.recording) - nfft) > 0:
            step_size = (len(self.recording) - nfft) / n_segments
            if step_size <= 1:
                noverlap = nfft // 2
                print(
                    f"  Info: Calculated step_size ({step_size:.2f}) for noverlap is too small or invalid. Falling back to 50% overlap."
                )
            else:
                noverlap = int(nfft - step_size)
                print(
                    f"  Info: Calculated noverlap using n_segments. step_size: {step_size:.2f}"
                )
        else:
            noverlap = nfft // 2
            if n_segments <= 0:
                print(
                    f"  Info: n_segments ({n_segments}) is not positive. Using 50% overlap for noverlap calculation."
                )
            else:
                print(
                    f"  Info: len(self.recording) ({len(self.recording)}) <= nfft ({nfft}). Using 50% overlap."
                )

        if noverlap >= nfft:
            print(
                f"  Warning: Calculated noverlap ({noverlap}) was >= nfft ({nfft}). Setting to nfft - 1 (or 0 if nfft is 1)."
            )
            noverlap = max(0, nfft - 1)
        if noverlap < 0:
            print(f"  Warning: Calculated noverlap ({noverlap}) was < 0. Setting to 0.")
            noverlap = 0

        window_arg = signal.get_window("hann", nfft)
        freqs, t, spectrum = spectrogram(
            self.recording,
            fs=self.fs,
            window=window_arg,
            nperseg=nfft,
            noverlap=noverlap,
            mode="psd",
        )

        if (
            spectrum.ndim != 2 or spectrum.shape[0] <= 1 or spectrum.shape[1] == 0
        ):
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

        z = 10 * np.log10(np.abs(z) + 1e-9)

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

        if plot_file_path:
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_ir(self, fig=None, ax=None, start=0.0, end=None, plot_file_path=None):
        """Plots impulse response wave form."""
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
        """Plots frequency response."""
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
            lower, upper = get_ylim(v)
            ax.set_ylim([lower, upper])

        ax.legend(legend, fontsize=8)

        if plot_file_path:
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_decay(self, fig=None, ax=None, plot_file_path=None):
        """Plots decay graph."""
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
            os.makedirs(os.path.dirname(plot_file_path), exist_ok=True)
            fig.savefig(plot_file_path)

        return fig, ax

    def plot_waterfall(self, fig=None, ax=None):
        """Plots decay waterfall."""
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

        copy_len = min(len(data), len(s))
        s[0:copy_len] = data[0:copy_len]

        data = s
        n_min = 40
        max(int(len(data) / hop_length) - 1, n_min)

        nfft = 256
        if nfft > len(data):
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

        noverlap = nfft // 2
        if noverlap >= nfft:
            noverlap = max(0, nfft - 1)
        if noverlap < 0:
            noverlap = 0

        window_arg = signal.get_window("hann", nfft)

        freqs, t, spectrum = spectrogram(
            data,
            fs=self.fs,
            window=window_arg,
            nperseg=nfft,
            noverlap=noverlap,
            mode="magnitude",
        )

        if spectrum.ndim != 2 or spectrum.shape[0] <= 1 or spectrum.shape[1] == 0:
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
            ax.set_xlim([0, np.max(t) * 1000])
        else:
            ax.set_xlim([0, 100])
        ax.set_xlabel("Time (ms)")

        # Y axis
        ax.set_ylim(np.log10([20, 20000]))
        ax.set_ylabel("Frequency (Hz)")
        ax.yaxis.set_major_formatter(FuncFormatter(lambda x, p: f"{10**x:.0f}"))

        ax.view_init(30, 30)

        return fig, ax
