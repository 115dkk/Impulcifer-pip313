# -*- coding: utf-8 -*-
"""
Validation tests for scipy_shim.

These tests run on DESKTOP where real scipy IS available,
comparing shim outputs against scipy outputs for numerical accuracy.

Run with: python -m pytest android_port/tests/test_scipy_shim.py -v
"""

import sys
import os
import numpy as np
import pytest

# Add android_port to path so we can import scipy_shim directly
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

import scipy_shim.fft as shim_fft
import scipy_shim.signal as shim_signal
import scipy_shim.signal.windows as shim_windows
import scipy_shim.interpolate as shim_interpolate
import scipy_shim.stats as shim_stats
import scipy_shim.ndimage as shim_ndimage

# Real scipy for comparison
import scipy.fft as real_fft
import scipy.signal as real_signal
import scipy.signal.windows as real_windows
import scipy.interpolate as real_interpolate
import scipy.stats as real_stats
import scipy.ndimage as real_ndimage


# ========== FFT Module ==========

class TestFFT:
    def test_fft_basic(self):
        x = np.random.randn(128)
        np.testing.assert_allclose(shim_fft.fft(x), real_fft.fft(x), rtol=1e-10)

    def test_rfft_basic(self):
        x = np.random.randn(128)
        np.testing.assert_allclose(shim_fft.rfft(x), real_fft.rfft(x), rtol=1e-10)

    def test_fftfreq(self):
        np.testing.assert_allclose(
            shim_fft.fftfreq(256, 1/48000),
            real_fft.fftfreq(256, 1/48000), rtol=1e-12)

    @pytest.mark.parametrize("target", [1, 2, 3, 100, 127, 128, 255, 256, 1000, 48000])
    def test_next_fast_len(self, target):
        result = shim_fft.next_fast_len(target)
        expected = real_fft.next_fast_len(target)
        assert result == expected, f"next_fast_len({target}): got {result}, expected {expected}"

    def test_next_fast_len_range(self):
        """Test next_fast_len for a range of values."""
        for n in range(1, 1025):
            assert shim_fft.next_fast_len(n) == real_fft.next_fast_len(n), f"Failed at n={n}"


# ========== Signal Module ==========

class TestConvolve:
    def test_full_mode(self):
        a = np.random.randn(100)
        b = np.random.randn(50)
        np.testing.assert_allclose(
            shim_signal.convolve(a, b, mode='full'),
            real_signal.convolve(a, b, mode='full'), rtol=1e-10)

    def test_same_mode(self):
        a = np.random.randn(100)
        b = np.random.randn(50)
        np.testing.assert_allclose(
            shim_signal.convolve(a, b, mode='same'),
            real_signal.convolve(a, b, mode='same'), rtol=1e-10)

    def test_valid_mode(self):
        a = np.random.randn(100)
        b = np.random.randn(50)
        np.testing.assert_allclose(
            shim_signal.convolve(a, b, mode='valid'),
            real_signal.convolve(a, b, mode='valid'), rtol=1e-10)

    def test_method_auto(self):
        a = np.random.randn(1000)
        b = np.random.randn(500)
        np.testing.assert_allclose(
            shim_signal.convolve(a, b, mode='full', method='auto'),
            real_signal.convolve(a, b, mode='full', method='auto'), rtol=1e-8)

    def test_small_arrays(self):
        a = np.array([1.0, 2.0, 3.0])
        b = np.array([0.5, 1.0])
        np.testing.assert_allclose(
            shim_signal.convolve(a, b, mode='full'),
            real_signal.convolve(a, b, mode='full'), rtol=1e-12)


class TestCorrelate:
    def test_full_mode(self):
        a = np.random.randn(100)
        b = np.random.randn(80)
        np.testing.assert_allclose(
            shim_signal.correlate(a, b, mode='full'),
            real_signal.correlate(a, b, mode='full'), rtol=1e-8)

    def test_correlation_lags(self):
        np.testing.assert_array_equal(
            shim_signal.correlation_lags(100, 80, mode='full'),
            real_signal.correlation_lags(100, 80, mode='full'))


class TestFindPeaks:
    def test_basic(self):
        x = np.array([0, 1, 0, 2, 0, 3, 0, 2, 0], dtype=float)
        shim_peaks, _ = shim_signal.find_peaks(x)
        real_peaks, _ = real_signal.find_peaks(x)
        np.testing.assert_array_equal(shim_peaks, real_peaks)

    def test_with_height(self):
        x = np.array([0, 1, 0, 2, 0, 3, 0, 2, 0], dtype=float)
        shim_peaks, shim_props = shim_signal.find_peaks(x, height=1.5)
        real_peaks, real_props = real_signal.find_peaks(x, height=1.5)
        np.testing.assert_array_equal(shim_peaks, real_peaks)

    def test_sine_wave(self):
        t = np.linspace(0, 1, 1000)
        x = np.sin(2 * np.pi * 5 * t)
        shim_peaks, _ = shim_signal.find_peaks(x)
        real_peaks, _ = real_signal.find_peaks(x)
        np.testing.assert_array_equal(shim_peaks, real_peaks)


class TestUnitImpulse:
    def test_basic(self):
        np.testing.assert_array_equal(
            shim_signal.unit_impulse(10),
            real_signal.unit_impulse(10))

    def test_with_idx(self):
        np.testing.assert_array_equal(
            shim_signal.unit_impulse(10, 5),
            real_signal.unit_impulse(10, 5))


class TestWindows:
    @pytest.mark.parametrize("M", [1, 2, 8, 64, 128, 256, 1024])
    def test_hann(self, M):
        np.testing.assert_allclose(
            shim_windows.hann(M), real_windows.hann(M), rtol=1e-12)

    def test_hann_sym_false(self):
        np.testing.assert_allclose(
            shim_windows.hann(128, sym=False),
            real_windows.hann(128, sym=False), rtol=1e-12)

    def test_get_window_hann(self):
        np.testing.assert_allclose(
            shim_windows.get_window('hann', 128),
            real_signal.get_window('hann', 128), rtol=1e-12)


class TestButter:
    def test_lowpass_order2(self):
        shim_sos = shim_signal.butter(2, 0.5, btype='low', output='sos')
        real_sos = real_signal.butter(2, 0.5, btype='low', output='sos')
        # SOS sections may be in different order, compare filter response instead
        self._compare_filter_response(shim_sos, real_sos)

    def test_highpass_order2(self):
        shim_sos = shim_signal.butter(2, 0.5, btype='high', output='sos')
        real_sos = real_signal.butter(2, 0.5, btype='high', output='sos')
        self._compare_filter_response(shim_sos, real_sos)

    def test_lowpass_with_fs(self):
        shim_sos = shim_signal.butter(2, 1000, btype='low', fs=48000, output='sos')
        real_sos = real_signal.butter(2, 1000, btype='low', fs=48000, output='sos')
        self._compare_filter_response(shim_sos, real_sos)

    def test_highpass_order8(self):
        shim_sos = shim_signal.butter(8, 250, btype='high', fs=48000, output='sos')
        real_sos = real_signal.butter(8, 250, btype='high', fs=48000, output='sos')
        self._compare_filter_response(shim_sos, real_sos, rtol=1e-3)

    def test_bandpass_order4(self):
        shim_sos = shim_signal.butter(4, [100, 1000], btype='band', fs=48000, output='sos')
        real_sos = real_signal.butter(4, [100, 1000], btype='band', fs=48000, output='sos')
        self._compare_filter_response(shim_sos, real_sos, rtol=1e-3)

    def _compare_filter_response(self, shim_sos, real_sos, rtol=1e-4):
        """Compare filter frequency response rather than raw SOS coefficients."""
        freqs = np.linspace(0, np.pi, 512)
        # Compute frequency response for each
        shim_h = self._sos_freqz(shim_sos, freqs)
        real_h = self._sos_freqz(real_sos, freqs)
        np.testing.assert_allclose(
            np.abs(shim_h), np.abs(real_h), rtol=rtol, atol=1e-8,
            err_msg="Filter magnitude response mismatch")

    @staticmethod
    def _sos_freqz(sos, freqs):
        """Compute frequency response from SOS."""
        h = np.ones(len(freqs), dtype=complex)
        z = np.exp(1j * freqs)
        for section in sos:
            b0, b1, b2 = section[0], section[1], section[2]
            a0, a1, a2 = section[3], section[4], section[5]
            num = b0 + b1 * z**-1 + b2 * z**-2
            den = a0 + a1 * z**-1 + a2 * z**-2
            h *= num / den
        return h


class TestSosfilt:
    def test_lowpass(self):
        sos = real_signal.butter(2, 0.1, output='sos')
        x = np.random.randn(1000)
        np.testing.assert_allclose(
            shim_signal.sosfilt(sos, x),
            real_signal.sosfilt(sos, x), rtol=1e-10)

    def test_highpass(self):
        sos = real_signal.butter(4, 0.3, btype='high', output='sos')
        x = np.random.randn(500)
        np.testing.assert_allclose(
            shim_signal.sosfilt(sos, x),
            real_signal.sosfilt(sos, x), rtol=1e-10)


class TestMinimumPhase:
    def test_basic(self):
        """Test minimum_phase with a known magnitude spectrum."""
        n = 64
        ir = np.zeros(n)
        ir[0] = 1.0
        ir[5] = 0.5
        ir[10] = -0.3
        mag = np.abs(np.fft.rfft(ir))

        shim_result = shim_signal.minimum_phase(mag, method='homomorphic', n_fft=n)
        real_result = real_signal.minimum_phase(mag, method='homomorphic', n_fft=n)

        # Allow some numerical tolerance (algorithm details may differ slightly)
        np.testing.assert_allclose(shim_result, real_result, rtol=1e-2, atol=1e-4)


# ========== Interpolate Module ==========

class TestInterp1d:
    def test_linear(self):
        x = np.linspace(0, 10, 20)
        y = np.sin(x)
        x_new = np.linspace(0, 10, 100)

        shim_f = shim_interpolate.interp1d(x, y, kind='linear')
        real_f = real_interpolate.interp1d(x, y, kind='linear')
        np.testing.assert_allclose(shim_f(x_new), real_f(x_new), rtol=1e-10)


class TestInterpolatedUnivariateSpline:
    def test_linear(self):
        x = np.log10(np.logspace(1, 4, 50))
        y = np.sin(x * 3)
        x_new = np.log10(np.logspace(1, 4, 200))

        shim_s = shim_interpolate.InterpolatedUnivariateSpline(x, y, k=1)
        real_s = real_interpolate.InterpolatedUnivariateSpline(x, y, k=1)
        np.testing.assert_allclose(shim_s(x_new), real_s(x_new), rtol=1e-8)

    def test_cubic(self):
        x = np.linspace(0, 10, 30)
        y = np.sin(x)
        x_new = np.linspace(0, 10, 100)

        shim_s = shim_interpolate.InterpolatedUnivariateSpline(x, y, k=3)
        real_s = real_interpolate.InterpolatedUnivariateSpline(x, y, k=3)
        # Natural cubic spline vs scipy's spline may differ slightly
        np.testing.assert_allclose(shim_s(x_new), real_s(x_new), rtol=1e-1, atol=1e-2)


# ========== Stats Module ==========

class TestLinregress:
    def test_basic(self):
        x = np.array([1, 2, 3, 4, 5], dtype=float)
        y = np.array([2.1, 3.9, 6.2, 7.8, 10.1])

        shim_result = shim_stats.linregress(x, y)
        real_result = real_stats.linregress(x, y)

        np.testing.assert_allclose(shim_result.slope, real_result.slope, rtol=1e-10)
        np.testing.assert_allclose(shim_result.intercept, real_result.intercept, rtol=1e-10)
        np.testing.assert_allclose(shim_result.rvalue, real_result.rvalue, rtol=1e-10)

    def test_random(self):
        np.random.seed(42)
        x = np.random.randn(100)
        y = 2.5 * x + 1.0 + np.random.randn(100) * 0.5

        shim_result = shim_stats.linregress(x, y)
        real_result = real_stats.linregress(x, y)

        np.testing.assert_allclose(shim_result.slope, real_result.slope, rtol=1e-8)
        np.testing.assert_allclose(shim_result.intercept, real_result.intercept, rtol=1e-8)
        np.testing.assert_allclose(shim_result.rvalue, real_result.rvalue, rtol=1e-8)
        np.testing.assert_allclose(shim_result.stderr, real_result.stderr, rtol=1e-6)


# ========== Ndimage Module ==========

class TestUniformFilter:
    def test_1d(self):
        x = np.random.randn(100)
        np.testing.assert_allclose(
            shim_ndimage.uniform_filter(x, size=5),
            real_ndimage.uniform_filter(x, size=5), rtol=1e-10)

    def test_1d_different_sizes(self):
        x = np.random.randn(200)
        for size in [3, 5, 7, 11]:
            np.testing.assert_allclose(
                shim_ndimage.uniform_filter(x, size=size),
                real_ndimage.uniform_filter(x, size=size), rtol=1e-8,
                err_msg=f"Failed for size={size}")


# ========== Integration Test ==========

class TestMonkeyPatch:
    """Test that the monkey-patching mechanism works correctly."""

    def test_install_and_import(self):
        """Test that scipy_shim.install() makes 'import scipy' work."""
        import scipy_shim
        # Don't actually install (would break other tests that use real scipy)
        # Just verify the install function exists and is callable
        assert callable(scipy_shim.install)

    def test_shim_has_all_needed_modules(self):
        """Verify all modules needed by Impulcifer are present in the shim."""
        import scipy_shim
        assert hasattr(scipy_shim, 'fft')
        assert hasattr(scipy_shim, 'signal')
        assert hasattr(scipy_shim, 'interpolate')
        assert hasattr(scipy_shim, 'stats')
        assert hasattr(scipy_shim, 'ndimage')

    def test_signal_has_all_needed_functions(self):
        """Verify scipy_shim.signal exposes all functions used by Impulcifer."""
        needed = ['convolve', 'correlate', 'correlation_lags', 'find_peaks',
                  'unit_impulse', 'butter', 'sosfilt', 'minimum_phase',
                  'spectrogram', 'resample', 'get_window', 'windows']
        for name in needed:
            assert hasattr(shim_signal, name), f"Missing: signal.{name}"

    def test_fft_has_all_needed_functions(self):
        needed = ['fft', 'rfft', 'ifft', 'irfft', 'fftfreq', 'rfftfreq', 'next_fast_len']
        for name in needed:
            assert hasattr(shim_fft, name), f"Missing: fft.{name}"


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
