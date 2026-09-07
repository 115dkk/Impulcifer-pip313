"""PA02 single-thread oracle; run ONLY with py -3.14, no fixtures required."""
import math
import os
from pathlib import Path
import platform
import statistics
import sys
import time
import winreg

import nnresample
import numpy as np
import scipy
from scipy import fft, fftpack, signal, special, stats
from scipy.interpolate import InterpolatedUnivariateSpline

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from core.audio_io import magnitude_response  # noqa: E402
from core.decay import _peak_index  # noqa: E402


def noise(n):
    state = 7
    values = []
    for _ in range(n):
        state = (state * 1664525 + 1013904223) & 0xFFFFFFFF
        values.append(state / 2147483648.0 - 1.0)
    return np.array(values, dtype=np.float64)


def sinc_fir(n):
    values = []
    for i in range(n):
        z = 0.1 * (i - (n - 1) / 2.0)
        sinc = 1.0 if z == 0.0 else math.sin(math.pi * z) / (math.pi * z)
        values.append(0.1 * sinc * (0.54 + 0.46 * math.cos(-math.pi + i * (2.0 * math.pi / (n - 1)))))
    return np.array(values)


def main():
    assert sys.version_info[:2] == (3, 14)
    print('Interpreter:', sys.executable)
    print('Python:', sys.version)
    print('OS:', platform.platform(), 'logical CPUs:', os.cpu_count())
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r'HARDWARE\DESCRIPTION\System\CentralProcessor\0') as key:
        print('CPU:', winreg.QueryValueEx(key, 'ProcessorNameString')[0])
    print('NumPy:', np.__version__, 'SciPy:', scipy.__version__)
    for key in ['OMP_NUM_THREADS', 'MKL_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'NUMEXPR_NUM_THREADS']:
        print(key, '=', os.environ.get(key, '<unset>'))
    print('FFT: numpy.fft pocketfft; scipy.fft pocketfft (default workers)')
    np.show_config()
    x, rec, inv, corr, short = [noise(n) for n in [96000, 391000, 295000, 1440, 783]]
    taps, h = sinc_fir(9600), sinc_fir(19200)
    freq = np.linspace(0.0, 24000.0, 9600)
    gain = np.array([1.0 + 0.2 * math.cos(f / 24000.0 * math.pi * 4.0) for f in freq])
    gain[-1] = 0.0
    knots = np.linspace(0.0, 3.38, 783) + 1.0
    queries = np.linspace(0.0, 3.38, 4800) + 1.0
    rx, ry = np.linspace(0.0, 1.0, 1000), noise(1000)
    poly_taps = nnresample.compute_filt(147, 160, fc='nn', beta=5.65326, N=32001)
    nnresample.resample(x, 96000, 48000)  # Explicit cache warm-up outside timing.
    nperseg = min(round(48000 / 10.0), len(inv) // 2, len(inv))
    overlap = max(0, min(nperseg - 1, int(nperseg - (len(inv) - nperseg) / 200)))

    def correlate():
        for _ in range(8):
            signal.correlate(corr, corr, mode='full')

    def butter():
        sos = np.vstack([signal.butter(4, 250.0 / 24000.0, btype='highpass', output='sos') for _ in range(2)])
        return signal.sosfilt(sos, x)

    def savgol():
        for w in [13, 23, 23, 91, 23, 23]:
            signal.savgol_filter(short, w, 2)

    cases = [
        ('convolve_full_ir_fir', '96000 x 9600', lambda: signal.convolve(x, taps, mode='full')),
        ('convolve_same_estimate', '391000 x 295000', lambda: signal.convolve(rec, inv, mode='same', method='auto')),
        ('correlate_full_30ms', '8 x 1440 x 1440', correlate),
        ('rfft_irfft_96000', '96000', lambda: np.fft.irfft(np.fft.rfft(x), n=len(x))),
        ('magnitude_response_96000', '96000', lambda: magnitude_response(x, 48000)),
        ('butter8_sosfilt_96000', '96000', butter),
        ('firwin2_19200', '19200 taps / 9600 mesh', lambda: signal.firwin2(19200, freq, gain, fs=48000)),
        ('minimum_phase_19200', '19200', lambda: signal.minimum_phase(h, n_fft=19200)),
        ('savgol_heavy_light_783', '783', savgol),
        ('find_peaks_96000', '96000', lambda: (signal.find_peaks(x, height=0.12589), signal.find_peaks(-x, height=0.12589))),
        ('first_peak_index_96000', '96000', lambda: _peak_index(x)),
        ('spline_k1_783_to_4800', '783 to 4800', lambda: InterpolatedUnivariateSpline(knots, short, k=1)(queries)),
        ('spline_k2_783', '783', lambda: InterpolatedUnivariateSpline(knots, short, k=2)(knots)),
        ('spline_k3_783', '783', lambda: InterpolatedUnivariateSpline(knots, short, k=3)(knots)),
        ('linregress_1000', '1000', lambda: stats.linregress(rx, ry)),
        ('nnresample_design_147_160', '32001 / FFT 524288', lambda: nnresample.compute_filt(147, 160, fc='nn', beta=5.65326, N=32001)),
        ('resample_poly_96000_48k_44k1', '96000', lambda: signal.resample_poly(x, 147, 160, window=poly_taps)),
        ('nnresample_96000_48k_96k', '96000', lambda: nnresample.resample(x, 96000, 48000)),
        ('spectrogram_295000_4800', f'295000 / 4800 / overlap {overlap}', lambda: signal.spectrogram(inv, 48000, window=signal.get_window('hann', nperseg), nperseg=nperseg, noverlap=overlap, mode='psd')),
        ('windows_32001', '32001 / 19200 / 19200', lambda: (signal.windows.kaiser(32001, 5.65326, sym=True), signal.windows.hann(19200, sym=True), signal.windows.hamming(19200, sym=True))),
        ('expit_783', '783', lambda: special.expit(short)),
        ('next_fast_len_1e6', '1000000 (real + legacy)', lambda: (fft.next_fast_len(1000000, real=True), fftpack.next_fast_len(1000000))),
    ]
    print('SciPy convolution choices:', signal.choose_conv_method(x, taps), signal.choose_conv_method(rec, inv), signal.choose_conv_method(corr, corr))
    print('| op | size | python median ms | python min ms |')
    print('|---|---|---:|---:|')
    for name, size, run in cases:
        if os.environ.get('PA02_FILTER', '') not in name:
            continue
        for _ in range(3):
            run()
        timings = []
        batch = 1000 if name == 'next_fast_len_1e6' else 1
        for _ in range(11):
            start = time.perf_counter()
            for _ in range(batch):
                run()
            timings.append((time.perf_counter() - start) * 1000 / batch)
        print(f'| {name} | {size} | {statistics.median(timings):.6f} | {min(timings):.6f} |', flush=True)


if __name__ == '__main__':
    main()
