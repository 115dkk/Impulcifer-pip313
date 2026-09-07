# -*- coding: utf-8 -*-
"""How stable are the README numbers (PNR, tail length) under a 5e-8 perturbation of
the final IR? Runs the 2.x decay analysis on the Python-side final tracks of the
vbass demo golden (p11) and on perturbed copies. Oracle noise-floor evidence."""
import glob, json, os, sys
import numpy as np

sys.path.insert(0, 'E:/Impulcifer')
from core import decay

GOLD = 'E:/Impulcifer/tests/migration/goldens'
fs = 48000


def load_track(prefix):
    # find the .f64 for the FL-left / FR-left tracks of the scenario (names vary; list candidates)
    files = sorted(glob.glob(os.path.join(GOLD, prefix)))
    return files


def readme_numbers(x):
    peak, tail, noise_db, _w = decay.decay_params(x, fs)
    pnr = 20 * np.log10(abs(x[peak]) + 1e-9) - noise_db
    length_ms = (tail - peak) / fs * 1000 if tail > peak else float('nan')
    return pnr, length_ms, noise_db, peak, tail


rng = np.random.default_rng(11)
for scenario in ('default', 'vbass'):
    cands = load_track(f'p11_{scenario}_readme_*.f64')
    if not cands:
        print(scenario, 'no FL .f64 fixture found; listing', load_track(f'p11_{scenario}*')[:8])
        continue
    for path in cands[:2]:
        x = np.fromfile(path, dtype='<f8')
        base = readme_numbers(x)
        print(f'{scenario} {os.path.basename(path)}: n={len(x)} max={np.max(np.abs(x)):.3e} '
              f'PNR={base[0]:.3f} dB len={base[1]:.3f} ms noise={base[2]:.2f} dB peak={base[3]} tail={base[4]}')
        tail_rms = np.sqrt(np.mean(x[len(x)//2:] ** 2))
        print(f'   tail RMS (second half) = {tail_rms:.3e} ({20*np.log10(tail_rms+1e-30):.1f} dBFS)')
        for eps in (1e-9, 5e-8, 1e-6):
            spread = []
            for _ in range(6):
                y = x + eps * rng.standard_normal(len(x))
                p = readme_numbers(y)
                spread.append((p[0] - base[0], p[1] - base[1]))
            d_pnr = max(abs(s[0]) for s in spread)
            d_len = max(abs(s[1]) for s in spread if not np.isnan(s[1]))
            print(f'   perturb {eps:.0e}: max |dPNR| = {d_pnr:.3f} dB, max |dlen| = {d_len:.3f} ms')
