# -*- coding: utf-8 -*-
"""
Android Bootstrap for Impulcifer
================================

This module patches sys.modules so that all 'import scipy' statements
throughout Impulcifer and AutoEq resolve to scipy_shim instead.

Call bootstrap() ONCE before importing any Impulcifer module.

Usage (from Chaquopy/Kotlin):
    from android_bootstrap import bootstrap
    bootstrap()
    import impulcifer
    impulcifer.main(dir_path="/sdcard/my_hrir", test_signal="default", ...)
"""

import sys
import os


def bootstrap(data_dir=None):
    """Initialize Impulcifer for Android.

    1. Patches scipy → scipy_shim in sys.modules
    2. Stubs out unavailable desktop-only modules (matplotlib, sounddevice, etc.)
    3. Sets up data paths for Android storage

    Parameters
    ----------
    data_dir : str, optional
        Path to Impulcifer data directory on Android.
        Defaults to app's internal storage.
    """
    _patch_scipy()
    _stub_desktop_modules()
    _setup_paths(data_dir)


def _patch_scipy():
    """Replace scipy with scipy_shim in sys.modules."""
    import scipy_shim
    from scipy_shim import fft, signal, interpolate, stats, ndimage
    from scipy_shim.signal import windows

    sys.modules['scipy'] = scipy_shim
    sys.modules['scipy.fft'] = fft
    sys.modules['scipy.signal'] = signal
    sys.modules['scipy.signal.windows'] = windows
    sys.modules['scipy.interpolate'] = interpolate
    sys.modules['scipy.stats'] = stats
    sys.modules['scipy.ndimage'] = ndimage


def _stub_desktop_modules():
    """Create stub modules for desktop-only dependencies.

    These modules are imported by Impulcifer but not needed for
    the processing pipeline on Android.
    """
    import types

    stub_modules = [
        # Visualization (not needed on Android)
        'matplotlib', 'matplotlib.pyplot', 'matplotlib.ticker',
        'matplotlib.gridspec', 'matplotlib.colors', 'matplotlib.cm',
        'seaborn',
        'bokeh', 'bokeh.plotting', 'bokeh.models', 'bokeh.layouts',
        'bokeh.io', 'bokeh.palettes', 'bokeh.transform',

        # Desktop GUI (replaced by Kotlin UI)
        'customtkinter', 'tkinter', 'tkinter.filedialog', 'tkinter.messagebox',
        'tkinter.font',

        # Desktop audio I/O (not needed — no recording on Android)
        'sounddevice',
    ]

    for mod_name in stub_modules:
        if mod_name not in sys.modules:
            stub = types.ModuleType(mod_name)
            # Add common no-op attributes
            stub.__path__ = []
            stub.__file__ = f'<stub:{mod_name}>'
            sys.modules[mod_name] = stub

    # soundfile may or may not work on Android (needs libsndfile)
    # Try importing it; if it fails, provide a WAV-only fallback
    try:
        import soundfile  # noqa: F401
    except ImportError:
        _install_soundfile_shim()


def _install_soundfile_shim():
    """Minimal soundfile replacement using Python's wave module."""
    import types
    import wave
    import numpy as np
    import struct

    sf = types.ModuleType('soundfile')

    def read(file, dtype='float64', always_2d=False, **kwargs):
        """Read a WAV file and return (data, samplerate)."""
        with wave.open(str(file), 'rb') as wf:
            n_channels = wf.getnchannels()
            sampwidth = wf.getsampwidth()
            fs = wf.getframerate()
            n_frames = wf.getnframes()
            raw = wf.readframes(n_frames)

        if sampwidth == 2:  # 16-bit
            fmt = f'<{n_frames * n_channels}h'
            data = np.array(struct.unpack(fmt, raw), dtype=np.float64)
            data /= 32768.0
        elif sampwidth == 3:  # 24-bit
            data = np.zeros(n_frames * n_channels, dtype=np.float64)
            for i in range(n_frames * n_channels):
                b = raw[i * 3:(i + 1) * 3]
                val = int.from_bytes(b, byteorder='little', signed=True)
                data[i] = val / 8388608.0
        elif sampwidth == 4:  # 32-bit
            fmt = f'<{n_frames * n_channels}i'
            data = np.array(struct.unpack(fmt, raw), dtype=np.float64)
            data /= 2147483648.0
        else:
            raise ValueError(f"Unsupported sample width: {sampwidth}")

        if n_channels > 1 or always_2d:
            data = data.reshape(-1, n_channels)

        return data, fs

    def write(file, data, samplerate, subtype=None, **kwargs):
        """Write a WAV file."""
        data = np.asarray(data)
        if data.ndim == 1:
            n_channels = 1
        else:
            n_channels = data.shape[1]

        # Default to 32-bit float PCM → 32-bit int
        if subtype and '16' in str(subtype):
            sampwidth = 2
            scale = 32767.0
        elif subtype and '24' in str(subtype):
            sampwidth = 3
            scale = 8388607.0
        else:
            sampwidth = 4
            scale = 2147483647.0

        flat = data.flatten()
        int_data = np.clip(flat * scale, -scale, scale).astype(np.int32)

        with wave.open(str(file), 'wb') as wf:
            wf.setnchannels(n_channels)
            wf.setsampwidth(sampwidth)
            wf.setframerate(samplerate)

            if sampwidth == 3:
                # 24-bit requires manual packing
                raw = b''
                for val in int_data:
                    raw += int(val).to_bytes(3, byteorder='little', signed=True)
                wf.writeframes(raw)
            elif sampwidth == 2:
                wf.writeframes(struct.pack(f'<{len(int_data)}h',
                                           *int_data.astype(np.int16)))
            else:
                wf.writeframes(struct.pack(f'<{len(int_data)}i', *int_data))

    sf.read = read
    sf.write = write
    sf.SoundFile = None  # Stub
    sys.modules['soundfile'] = sf


def _setup_paths(data_dir=None):
    """Configure paths for Android."""
    if data_dir is None:
        # Default: app's internal files directory
        # This will be overridden by Kotlin code passing the actual path
        data_dir = os.environ.get('IMPULCIFER_DATA_DIR', '/data/local/tmp/impulcifer')

    os.environ['IMPULCIFER_DATA_DIR'] = data_dir

    # Ensure the data directory exists
    os.makedirs(data_dir, exist_ok=True)
