# -*- coding: utf-8 -*-
"""scipy.signal.windows shim — window functions via NumPy."""

import numpy as np


def hann(M, sym=True):
    """Hann (Hanning) window.

    Matches scipy.signal.windows.hann behavior.
    """
    if M < 1:
        return np.array([], dtype=np.float64)
    if M == 1:
        return np.ones(1, dtype=np.float64)

    if not sym:
        M_use = M + 1
    else:
        M_use = M

    w = 0.5 * (1.0 - np.cos(2.0 * np.pi * np.arange(M_use) / (M_use - 1)))

    if not sym:
        w = w[:M]
    return w


def hamming(M, sym=True):
    """Hamming window."""
    if M < 1:
        return np.array([], dtype=np.float64)
    if M == 1:
        return np.ones(1, dtype=np.float64)

    if not sym:
        M_use = M + 1
    else:
        M_use = M

    w = 0.54 - 0.46 * np.cos(2.0 * np.pi * np.arange(M_use) / (M_use - 1))

    if not sym:
        w = w[:M]
    return w


def blackman(M, sym=True):
    """Blackman window."""
    if M < 1:
        return np.array([], dtype=np.float64)
    if M == 1:
        return np.ones(1, dtype=np.float64)

    if not sym:
        M_use = M + 1
    else:
        M_use = M

    n = np.arange(M_use)
    w = 0.42 - 0.5 * np.cos(2 * np.pi * n / (M_use - 1)) + 0.08 * np.cos(4 * np.pi * n / (M_use - 1))

    if not sym:
        w = w[:M]
    return w


def bartlett(M, sym=True):
    """Bartlett (triangular) window."""
    if M < 1:
        return np.array([], dtype=np.float64)
    if M == 1:
        return np.ones(1, dtype=np.float64)
    return np.bartlett(M).astype(np.float64)


def get_window(window, Nx, fftbins=True):
    """Return a window of a given length and type.

    Parameters
    ----------
    window : str or tuple or array_like
        Window type name or window values.
    Nx : int
        Number of points in the window.
    fftbins : bool
        If True, create a 'periodic' window (sym=False).
    """
    sym = not fftbins

    if isinstance(window, str):
        name = window.lower()
        if name in ('hann', 'hanning'):
            return hann(Nx, sym=sym)
        elif name == 'hamming':
            return hamming(Nx, sym=sym)
        elif name == 'blackman':
            return blackman(Nx, sym=sym)
        elif name in ('bartlett', 'triangular'):
            return bartlett(Nx, sym=sym)
        elif name in ('boxcar', 'ones', 'rectangular'):
            return np.ones(Nx, dtype=np.float64)
        else:
            raise ValueError(f"Unknown window type: {window}")
    elif isinstance(window, (list, np.ndarray)):
        return np.asarray(window, dtype=np.float64)
    elif isinstance(window, tuple):
        # Handle parameterized windows like ('kaiser', beta)
        name = window[0].lower()
        if name == 'kaiser':
            return np.kaiser(Nx, window[1]).astype(np.float64)
        else:
            raise ValueError(f"Unknown parameterized window: {name}")
    else:
        raise ValueError(f"Invalid window specification: {window}")
