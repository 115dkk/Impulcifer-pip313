# -*- coding: utf-8 -*-
"""WAV/audio I/O and core DSP primitives.

Split out of ``core/utils.py`` (audit #115 finding 8). ``magnitude_response``
is kept byte-identical — its rfft path is pinned by
``tests/test_magnitude_response_parity.py`` and feeds the BRIR md5.
``core.utils`` re-exports these names.
"""

import os

import numpy as np
import soundfile as sf
from scipy import signal

from core.audio_truehd import read_audio


def to_db(x):
    """Convert amplitude to dB

    Args:
        x: Amplitude value

    Returns:
        Value in dB
    """
    return 20 * np.log10(np.abs(x) + 1e-10)


def db_to_gain(x):
    """Convert dB to amplitude gain

    Args:
        x: Value in dB

    Returns:
        Amplitude gain
    """
    return 10 ** (x / 20)


def convolve(x, y):
    """Convolve two signals

    Args:
        x: First signal
        y: Second signal

    Returns:
        Convolved signal
    """
    return signal.convolve(x, y, mode='full')


def dB_unweight(x):
    """Remove dB weighting from a signal

    Args:
        x: Signal with dB weighting

    Returns:
        Signal without dB weighting
    """
    return 10 ** (x / 20)


def read_wav(file_path, expand=False):
    """Reads WAV file (backward compatibility wrapper)

    Args:
        file_path: Path to WAV file as string
        expand: Expand dimensions of a single track recording to produce 2-D array?

    Returns:
        - sampling frequency as integer
        - wav data as numpy array with one row per track, samples in range -1..1
    """
    fs, data, _ = read_audio(file_path, expand=expand)
    return fs, data


def write_wav(file_path, fs, data, bit_depth=32):
    """Writes WAV file."""
    # Ensure the directory exists before saving
    os.makedirs(os.path.dirname(file_path), exist_ok=True)
    
    if bit_depth == 16:
        subtype = "PCM_16"
    elif bit_depth == 24:
        subtype = "PCM_24"
    elif bit_depth == 32:
        subtype = "PCM_32"
    else:
        raise ValueError('Invalid bit depth. Accepted values are 16, 24 and 32.')
    if len(data.shape) > 1 and data.shape[1] > data.shape[0]:
        # We have tracks on rows, soundfile want's them on columns
        data = np.transpose(data)
    sf.write(file_path, data, samplerate=fs, subtype=subtype)


def magnitude_response(x, fs):
    """Calculates frequency magnitude response.

    Returns the same first ``ceil(N/2)`` bins as Lion's full-FFT implementation
    while only computing the one-sided real spectrum (``rfft`` is ~2x faster on
    real input). For real ``x`` we have ``fft(x)[k] == rfft(x)[k]`` for
    ``0 <= k <= N/2``, so the slice we expose here is bit-identical to Lion.
    """
    nfft = len(x)
    half = int(np.ceil(nfft / 2))
    X = np.fft.rfft(x)
    X_mag = 20 * np.log10(np.abs(X[:half]))
    f = np.arange(half) * (fs / nfft)
    return f, X_mag


def running_mean(x, N):
    cumsum = np.cumsum(np.insert(x, 0, 0))
    return (cumsum[N:] - cumsum[:-N]) / float(N)
