# -*- coding: utf-8 -*-
"""scipy.fft shim — wraps numpy.fft with scipy-compatible API."""

import numpy as np

# Direct re-exports from numpy.fft
fft = np.fft.fft
ifft = np.fft.ifft
rfft = np.fft.rfft
irfft = np.fft.irfft
fftfreq = np.fft.fftfreq
rfftfreq = np.fft.rfftfreq
fftshift = np.fft.fftshift
ifftshift = np.fft.ifftshift


def next_fast_len(target, real=False):
    """Find the next fast size for FFT.

    For complex transforms (real=False): uses {2,3,5,7,11}-smooth numbers.
    For real transforms (real=True): uses {2,3,5}-smooth numbers.

    This matches scipy.fft.next_fast_len behavior.
    """
    if target <= 1:
        return 1

    if real:
        factors = [2, 3, 5]
    else:
        factors = [2, 3, 5, 7, 11]

    # Generate smooth numbers up to a generous bound
    limit = max(target * 2, 16)
    best = limit

    def _search(val, factor_idx):
        nonlocal best
        if val >= target:
            if val < best:
                best = val
            return
        if val > limit:
            return
        for i in range(factor_idx, len(factors)):
            _search(val * factors[i], i)

    _search(1, 0)
    return best
