# -*- coding: utf-8 -*-
"""Regression test pinning Lion-equivalent behaviour for ``magnitude_response``.

The current implementation uses ``np.fft.rfft`` for speed, but it must return
exactly the same first ``ceil(N/2)`` magnitude bins as Lion's full-FFT path
(``scipy.fftpack.fft`` followed by a ``[:ceil(N/2)]`` slice). For real inputs
those two FFT paths agree bit-for-bit on the bins we expose, so this test
fails the moment someone introduces an epsilon, a windowed FFT, or a different
bin layout.
"""

from __future__ import annotations

import unittest

import numpy as np
from scipy.fftpack import fft

from core.utils import magnitude_response


def _lion_magnitude_response(x: np.ndarray, fs: int):
    nfft = len(x)
    df = fs / nfft
    f = np.arange(0, fs - df, df)
    X = fft(x)
    X_mag = 20 * np.log10(np.abs(X))
    half = int(np.ceil(nfft / 2))
    return f[:half], X_mag[:half]


class MagnitudeResponseLionParity(unittest.TestCase):
    def _assert_bit_identical(self, x: np.ndarray, fs: int):
        f_lion, m_lion = _lion_magnitude_response(x, fs)
        f_modern, m_modern = magnitude_response(x, fs)
        self.assertEqual(f_lion.shape, f_modern.shape)
        self.assertTrue(np.array_equal(f_lion, f_modern))
        self.assertEqual(m_lion.shape, m_modern.shape)
        self.assertTrue(np.array_equal(m_lion, m_modern))

    def test_even_length(self):
        rng = np.random.default_rng(0xA110)
        for nfft in (8, 1024, 48000):
            with self.subTest(nfft=nfft):
                self._assert_bit_identical(rng.standard_normal(nfft), 48000)

    def test_odd_length(self):
        rng = np.random.default_rng(0xA111)
        for nfft in (9, 1025, 48001):
            with self.subTest(nfft=nfft):
                self._assert_bit_identical(rng.standard_normal(nfft), 48000)

    def test_impulse(self):
        x = np.zeros(2048)
        x[0] = 1.0
        self._assert_bit_identical(x, 48000)

    def test_sweep_like(self):
        # Smooth, broadband signal that exercises every FFT bin.
        n = 4096
        t = np.arange(n) / 48000
        x = np.sin(2 * np.pi * np.linspace(20, 20000, n) * t)
        self._assert_bit_identical(x, 48000)


if __name__ == "__main__":
    unittest.main()
