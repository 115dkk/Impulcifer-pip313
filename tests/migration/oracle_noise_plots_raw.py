# -*- coding: utf-8 -*-
"""How stable is the unsmoothed results-plot magnitude series (dB) under tiny
perturbations of the summed IR? Uses the P19 golden summed IRs and the 2.x
ImpulseResponse.frequency_response() path. Oracle noise-floor evidence for the
pipeline raw-series budget in crates/impulcifer-service/tests/brir_plots.rs."""
import sys

import numpy as np

sys.path.insert(0, 'E:/Impulcifer')
from core.impulse_response import ImpulseResponse  # noqa: E402

GOLD = 'E:/Impulcifer/tests/migration/goldens'
fs = 48000
rng = np.random.default_rng(7)

for side in ('left', 'right'):
    x = np.fromfile(f'{GOLD}/p19_results_{side}.f64', dtype='<f8')
    base = ImpulseResponse(x.copy(), fs).frequency_response()
    f, raw = base.frequency, base.raw
    top = raw.max()
    print(f'{side}: n={len(x)} peak={np.max(np.abs(x)):.3e} bins={len(f)} raw max={top:.2f} dB min={raw.min():.2f} dB')
    for rel in (1e-12, 1e-10, 1e-9, 1e-8):
        eps = rel * np.max(np.abs(x))
        worst = 0.0
        where = None
        for _ in range(4):
            y = x + eps * rng.standard_normal(len(x))
            r2 = ImpulseResponse(y, fs).frequency_response().raw
            d = np.abs(r2 - raw)
            i = int(np.argmax(d))
            if d[i] > worst:
                worst, where = float(d[i]), (float(f[i]), float(raw[i] - top))
        deep = raw < top - 60
        print(f'   perturb {rel:.0e} of peak: max |d raw| = {worst:.4f} dB at {where[0]:.1f} Hz ({where[1]:.1f} dB below max); '
              f'bins below max-60 dB: {int(deep.sum())}')
        # budget by level band
        for lo, hi in ((0, 40), (40, 60), (60, 200)):
            m = (top - raw >= lo) & (top - raw < hi)
            if m.any():
                y = x + eps * rng.standard_normal(len(x))
                r2 = ImpulseResponse(y, fs).frequency_response().raw
                print(f'      level {lo}-{hi} dB below max: {int(m.sum())} bins, max |d| = {np.max(np.abs(r2 - raw)[m]):.4f} dB')
