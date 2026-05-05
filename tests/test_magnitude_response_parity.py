# -*- coding: utf-8 -*-
"""Regression test pinning verified ``magnitude_response`` behaviour.

The BRIR integrity baseline was established with the one-sided NumPy ``rfft``
path below. A full FFT can be numerically close, but it is not bit-identical on
all supported platforms and changes the generated ``hesuvi.wav`` md5.
"""

from __future__ import annotations

import unittest

import numpy as np

from core.utils import magnitude_response


def _verified_magnitude_response(x: np.ndarray, fs: int):
    nfft = len(x)
    half = int(np.ceil(nfft / 2))
    spectrum = np.fft.rfft(x)
    magnitude = 20 * np.log10(np.abs(spectrum[:half]))
    frequency = np.arange(half) * (fs / nfft)
    return frequency, magnitude


class MagnitudeResponseVerifiedParity(unittest.TestCase):
    def _assert_bit_identical(self, x: np.ndarray, fs: int):
        f_expected, m_expected = _verified_magnitude_response(x, fs)
        f_actual, m_actual = magnitude_response(x, fs)
        self.assertEqual(f_expected.shape, f_actual.shape)
        self.assertTrue(np.array_equal(f_expected, f_actual))
        self.assertEqual(m_expected.shape, m_actual.shape)
        self.assertTrue(np.array_equal(m_expected, m_actual))

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
        n = 4096
        t = np.arange(n) / 48000
        x = np.sin(2 * np.pi * np.linspace(20, 20000, n) * t)
        self._assert_bit_identical(x, 48000)


if __name__ == "__main__":
    unittest.main()
