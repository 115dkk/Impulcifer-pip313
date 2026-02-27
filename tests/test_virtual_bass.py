# -*- coding: utf-8 -*-
"""Unit tests for virtual_bass module."""

import numpy as np
import pytest
from unittest.mock import MagicMock


class TestClassifySpeaker:
    """Test _classify_speaker() returns correct classification."""

    def test_left_speakers(self):
        from core.virtual_bass import _classify_speaker
        assert _classify_speaker('FL') == 'left'
        assert _classify_speaker('SL') == 'left'
        assert _classify_speaker('BL') == 'left'
        assert _classify_speaker('WL') == 'left'
        assert _classify_speaker('TFL') == 'left'
        assert _classify_speaker('TSL') == 'left'
        assert _classify_speaker('TBL') == 'left'

    def test_right_speakers(self):
        from core.virtual_bass import _classify_speaker
        assert _classify_speaker('FR') == 'right'
        assert _classify_speaker('SR') == 'right'
        assert _classify_speaker('BR') == 'right'
        assert _classify_speaker('WR') == 'right'
        assert _classify_speaker('TFR') == 'right'
        assert _classify_speaker('TSR') == 'right'
        assert _classify_speaker('TBR') == 'right'

    def test_center_speakers(self):
        from core.virtual_bass import _classify_speaker
        assert _classify_speaker('FC') == 'center'
        assert _classify_speaker('LFE') == 'center'

    def test_unknown_defaults_to_center(self):
        from core.virtual_bass import _classify_speaker
        assert _classify_speaker('UNKNOWN') == 'center'

    def test_case_insensitive(self):
        from core.virtual_bass import _classify_speaker
        assert _classify_speaker('fl') == 'left'
        assert _classify_speaker('fr') == 'right'
        assert _classify_speaker('fc') == 'center'


class TestDetectPolarity:
    """Test polarity detection."""

    def test_positive_peak(self):
        from core.virtual_bass import _detect_polarity
        ir = np.zeros(100)
        ir[10] = 1.0  # Positive peak
        assert _detect_polarity(ir) == 1.0

    def test_negative_peak(self):
        from core.virtual_bass import _detect_polarity
        ir = np.zeros(100)
        ir[10] = -1.0  # Negative peak
        assert _detect_polarity(ir) == -1.0

    def test_mixed_signal_positive_dominant(self):
        from core.virtual_bass import _detect_polarity
        ir = np.zeros(100)
        ir[10] = 0.8
        ir[20] = -0.5
        assert _detect_polarity(ir) == 1.0

    def test_mixed_signal_negative_dominant(self):
        from core.virtual_bass import _detect_polarity
        ir = np.zeros(100)
        ir[10] = 0.3
        ir[20] = -0.9
        assert _detect_polarity(ir) == -1.0


class TestBuildIldShelf:
    """Test ILD shelf building."""

    def test_high_crossover_returns_shelf(self):
        from core.virtual_bass import _build_ild_shelf
        # xo_hz=250 >= 160 threshold, so 150Hz shelf should be active
        result = _build_ild_shelf(250, 48000)
        assert result is not None
        sos_lp, gain_linear, shelf_gain_db = result
        assert shelf_gain_db == 6.0  # 150 Hz shelf at 6 dB
        assert gain_linear == pytest.approx(10 ** (6.0 / 20.0), rel=1e-6)

    def test_low_crossover_returns_none(self):
        from core.virtual_bass import _build_ild_shelf
        # xo_hz=50 < 80 (minimum threshold), so no shelf
        result = _build_ild_shelf(50, 48000)
        assert result is None

    def test_medium_crossover(self):
        from core.virtual_bass import _build_ild_shelf
        # xo_hz=100 >= 80 threshold
        result = _build_ild_shelf(100, 48000)
        assert result is not None
        _, _, shelf_gain_db = result
        assert shelf_gain_db == 3.0  # 50 Hz shelf at 3 dB


class TestSynthesizeVirtualBass:
    """Smoke test for the full synthesis pipeline."""

    def _make_dummy_ir(self, fs=48000, duration_ms=50):
        """Create a dummy IR object with a simple impulse."""
        n_samples = int(fs * duration_ms / 1000)
        data = np.zeros(n_samples)
        # Place a peak at 1ms
        peak_idx = int(fs * 0.001)
        data[peak_idx] = 1.0
        # Add some low-frequency content
        t = np.arange(n_samples) / fs
        data += 0.1 * np.sin(2 * np.pi * 100 * t) * np.exp(-t * 50)

        ir = MagicMock()
        ir.data = data
        return ir

    def _make_dummy_hrir(self, speakers=None, fs=48000):
        """Create a dummy HRIR-like object."""
        if speakers is None:
            speakers = ['FL', 'FR', 'FC']

        hrir = MagicMock()
        hrir.fs = fs
        hrir.irs = {}
        for sp in speakers:
            hrir.irs[sp] = {
                'left': self._make_dummy_ir(fs),
                'right': self._make_dummy_ir(fs),
            }
        return hrir

    def test_smoke_no_exception(self):
        """Apply virtual bass to a dummy HRIR and verify no exception."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        hrir = self._make_dummy_hrir()
        # Should complete without error
        apply_virtual_bass_to_hrir(hrir, crossover_freq=250)

    def test_data_modified(self):
        """Verify that IR data is actually modified."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        hrir = self._make_dummy_hrir(speakers=['FL'])
        original = hrir.irs['FL']['left'].data.copy()
        apply_virtual_bass_to_hrir(hrir, crossover_freq=250)
        # Data should have changed
        assert not np.array_equal(hrir.irs['FL']['left'].data, original)

    def test_polarity_normal(self):
        """Test with forced normal polarity."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        hrir = self._make_dummy_hrir(speakers=['FL'])
        apply_virtual_bass_to_hrir(hrir, crossover_freq=250, invert_polarity=False)

    def test_polarity_invert(self):
        """Test with forced inverted polarity."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        hrir = self._make_dummy_hrir(speakers=['FL'])
        apply_virtual_bass_to_hrir(hrir, crossover_freq=250, invert_polarity=True)

    def test_various_crossover_frequencies(self):
        """Test with various crossover frequencies."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        for freq in [50, 100, 200, 300, 500]:
            hrir = self._make_dummy_hrir(speakers=['FL'])
            apply_virtual_bass_to_hrir(hrir, crossover_freq=freq)

    def test_crossover_exceeds_nyquist(self):
        """Crossover >= Nyquist should bail out gracefully."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        hrir = self._make_dummy_hrir(speakers=['FL'], fs=48000)
        original = hrir.irs['FL']['left'].data.copy()
        apply_virtual_bass_to_hrir(hrir, crossover_freq=25000)
        # Data should NOT change (function bailed out)
        assert np.array_equal(hrir.irs['FL']['left'].data, original)

    def test_all_speaker_types(self):
        """Test with all speaker types to verify classification works."""
        from core.virtual_bass import apply_virtual_bass_to_hrir
        speakers = ['FL', 'FR', 'FC', 'SL', 'SR', 'BL', 'BR',
                     'WL', 'WR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR']
        hrir = self._make_dummy_hrir(speakers=speakers)
        apply_virtual_bass_to_hrir(hrir, crossover_freq=250)


class TestShift:
    """Test _shift function."""

    def test_shift_right(self):
        from core.virtual_bass import _shift
        ir = np.array([1.0, 0.0, 0.0, 0.0])
        shifted = _shift(ir, 1)
        expected = np.array([0.0, 1.0, 0.0, 0.0])
        np.testing.assert_array_equal(shifted, expected)

    def test_shift_left(self):
        from core.virtual_bass import _shift
        ir = np.array([0.0, 1.0, 0.0, 0.0])
        shifted = _shift(ir, -1)
        expected = np.array([1.0, 0.0, 0.0, 0.0])
        np.testing.assert_array_equal(shifted, expected)

    def test_shift_zero(self):
        from core.virtual_bass import _shift
        ir = np.array([1.0, 2.0, 3.0])
        shifted = _shift(ir, 0)
        np.testing.assert_array_equal(shifted, ir)


class TestRfftMagnitude:
    """Test _rfft_magnitude function."""

    def test_returns_correct_shape(self):
        from core.virtual_bass import _rfft_magnitude
        ir = np.zeros(1024)
        ir[0] = 1.0  # Dirac delta
        mag, freqs = _rfft_magnitude(ir, 48000)
        assert len(mag) == len(freqs)
        assert len(mag) == 1024 // 2 + 1

    def test_dirac_flat_magnitude(self):
        from core.virtual_bass import _rfft_magnitude
        ir = np.zeros(1024)
        ir[0] = 1.0  # Dirac delta should have flat magnitude
        mag, freqs = _rfft_magnitude(ir, 48000)
        np.testing.assert_allclose(mag, 1.0, atol=1e-10)


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
