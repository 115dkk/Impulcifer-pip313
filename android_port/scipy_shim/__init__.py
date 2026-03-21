# -*- coding: utf-8 -*-
"""
scipy_shim — Drop-in replacement for scipy on Android (via Chaquopy + NumPy only).

This package provides pure Python+NumPy implementations of the scipy functions
used by Impulcifer and AutoEq. It is designed to be injected via sys.modules
monkey-patching so that the original code runs unmodified.

Usage:
    import scipy_shim
    scipy_shim.install()  # patches sys.modules['scipy'] etc.
"""

from . import fft
from . import signal
from . import interpolate
from . import stats
from . import ndimage


def install():
    """Install scipy_shim into sys.modules so 'import scipy' uses this package."""
    import sys
    sys.modules['scipy'] = __import__(__name__)
    sys.modules['scipy.fft'] = fft
    sys.modules['scipy.signal'] = signal
    sys.modules['scipy.signal.windows'] = signal.windows
    sys.modules['scipy.interpolate'] = interpolate
    sys.modules['scipy.stats'] = stats
    sys.modules['scipy.ndimage'] = ndimage
