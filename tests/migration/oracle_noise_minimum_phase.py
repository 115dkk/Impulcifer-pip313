# -*- coding: utf-8 -*-
"""How reproducible is AutoEQ minimum_phase_impulse_response in Python itself? (oracle noise floor)

Perturb the linear-phase FIR by relative noise of 1e-12 (the size of the Rust/Python
firwin2 difference) and measure the spread of the homomorphic minimum-phase result
per frequency band. Also compare numpy.fft vs scipy.fft backends for the same chain.
"""
import sys, os, shutil, tempfile
import numpy as np
from scipy import signal, fft as sfft

sys.path.insert(0, 'E:/Impulcifer')
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.pipeline_stages import headphone_compensation
from autoeq.frequency_response import FrequencyResponse

tmp = tempfile.mkdtemp()
demo = os.path.join(tmp, 'demo')
shutil.copytree('E:/Impulcifer/data/demo', demo)
est = ImpulseResponseEstimator.from_wav('E:/Impulcifer/data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav')
left, right = headphone_compensation(est, demo)
fr = left.copy()
fr.smoothen_heavy_light()
fr.equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=24000)

# Replicate minimum_phase_impulse_response(fs=48000, f_res=5, normalize=False) up to the linear FIR
fs, f_res = 48000, 5 / 2
from scipy.interpolate import InterpolatedUnivariateSpline
from scipy.fftpack import next_fast_len
d = FrequencyResponse(name='fr_data', frequency=fr.frequency.copy(), raw=fr.equalization.copy())
f_min = np.max([d.frequency[0], f_res])
gain_f_min = InterpolatedUnivariateSpline(np.log10(d.frequency), d.raw, k=1)(np.log10(f_min))
n = next_fast_len(round(fs // 2 / f_res))
f = np.linspace(0.0, fs // 2, n)
d.interpolate(f, pol_order=1)
d.raw[d.frequency <= f_min] = gain_f_min
d.raw *= 2
d.raw = 10 ** (d.raw / 20)
d.raw[-1] = 0.0
lin = signal.firwin2(len(d.frequency) * 2, d.frequency, d.raw, fs=fs)
ref = signal.minimum_phase(lin, n_fft=len(lin))
peak = np.max(np.abs(ref))
print('n', n, 'lin taps', len(lin), 'mp taps', len(ref), 'peak', peak)
H = np.abs(sfft.fft(lin, len(lin)))
print('|H| at Nyquist bin', H[len(lin) // 2], 'min positive', H[H > 0].min(), 'floor 1e-7*min', 1e-7 * H[H > 0].min())
print('|H| last 5 bins before Nyquist', H[len(lin) // 2 - 5: len(lin) // 2])


def spectrum_db(x, nfft):
    return 20 * np.log10(np.abs(sfft.rfft(x, nfft)) + 1e-300)


nfft = len(lin) * 2
freqs = sfft.rfftfreq(nfft, 1 / fs)
ref_db = spectrum_db(ref, nfft)
bands = [(0, 16000), (16000, 20000), (20000, 23000), (23000, 23900), (23900, 24001)]
rng = np.random.default_rng(1)


def report(label, y):
    diff = np.abs(spectrum_db(y, nfft) - ref_db)
    taps = np.max(np.abs(y - ref))
    cells = []
    for lo, hi in bands:
        m = (freqs >= lo) & (freqs < hi)
        cells.append(f'{lo}-{hi}: {diff[m].max():.3e}')
    print(f'{label}: taps {taps:.3e} ({taps / peak:.3e} rel) | dB diff ' + ' | '.join(cells))


for trial in range(3):
    pert = lin * (1 + 1e-12 * rng.standard_normal(len(lin)))
    report(f'lin*(1+1e-12 noise) #{trial}', signal.minimum_phase(pert, n_fft=len(lin)))
for trial in range(2):
    pert = lin + 1e-12 * peak * rng.standard_normal(len(lin))
    report(f'lin+1e-12*peak noise #{trial}', signal.minimum_phase(pert, n_fft=len(lin)))
# Type II symmetric FIR: exact Nyquist zero. What if the Nyquist bin residue is forced to a different value?
pert = lin.copy()
pert[0] += 1e-13
report('lin[0]+1e-13', signal.minimum_phase(pert, n_fft=len(lin)))
shutil.rmtree(tmp, ignore_errors=True)
