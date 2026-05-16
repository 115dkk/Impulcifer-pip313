"""Regression tests for allocation-focused FrequencyResponse optimizations."""

from __future__ import annotations

import numpy as np

from autoeq import biquad
from autoeq.frequency_response import FrequencyResponse


def _reference_smoothen_heavy_light(fr: FrequencyResponse) -> tuple[np.ndarray, np.ndarray]:
    """Mirror the previous copy-heavy implementation for parity checks."""
    light = fr.copy()
    light.name = "Light"
    light.smoothen_fractional_octave(
        window_size=1 / 6,
        iterations=1,
        treble_f_lower=100,
        treble_f_upper=10000,
        treble_window_size=1 / 3,
        treble_iterations=1,
    )

    heavy = fr.copy()
    heavy.name = "Heavy"
    heavy.smoothen_fractional_octave(
        window_size=1 / 3,
        iterations=1,
        treble_f_lower=1000,
        treble_f_upper=6000,
        treble_window_size=1.3,
        treble_iterations=1,
    )

    combination = fr.copy()
    combination.name = "Combination"
    combination.error = np.max(
        np.vstack([light.error_smoothed, heavy.error_smoothed]),
        axis=0,
    )
    combination.smoothen_fractional_octave(
        window_size=1 / 3,
        iterations=1,
        treble_f_lower=100,
        treble_f_upper=10000,
        treble_window_size=1 / 3,
        treble_iterations=1,
    )
    return combination.smoothed.copy(), combination.error_smoothed.copy()


def test_smoothen_heavy_light_matches_copy_based_reference() -> None:
    frequency = FrequencyResponse.generate_frequencies(f_min=10, f_max=24000, f_step=1.01)
    log_frequency = np.log10(frequency)
    raw = 2.5 * np.sin(log_frequency * 3.0) + 0.7 * np.cos(log_frequency * 11.0)
    error = -1.7 * np.cos(log_frequency * 4.0) + 0.4 * np.sin(log_frequency * 17.0)

    original = FrequencyResponse(name="original", frequency=frequency, raw=raw, error=error)
    expected_smoothed, expected_error_smoothed = _reference_smoothen_heavy_light(original)

    optimized = FrequencyResponse(name="optimized", frequency=frequency, raw=raw, error=error)
    optimized.smoothen_heavy_light()

    assert np.array_equal(optimized.smoothed, expected_smoothed)
    assert np.array_equal(optimized.error_smoothed, expected_error_smoothed)


def test_biquad_response_avoids_repeated_frequency_rows_without_changing_output() -> None:
    frequency = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.03)
    fc = np.array([55.0, 160.0, 1000.0, 4600.0])
    q = np.array([0.7, 1.1, 1.8, 3.0])
    gain = np.array([3.0, -2.5, 1.2, -4.0])

    fc_column = np.expand_dims(fc, axis=1)
    q_column = np.expand_dims(np.abs(q), axis=1)
    gain_column = np.expand_dims(gain, axis=1)
    a0, a1, a2, b0, b1, b2 = biquad.peaking(fc_column, q_column, gain_column)
    frequency_rows = np.repeat(np.expand_dims(frequency, axis=0), len(fc_column), axis=0)
    expected = np.sum(
        biquad.digital_coeffs(frequency_rows, 48000, a0, a1, a2, b0, b1, b2),
        axis=0,
    )

    actual, coeffs_a, coeffs_b = FrequencyResponse._biquad_eq_response(
        frequency,
        fc,
        q,
        gain,
        fs=48000,
    )

    assert np.array_equal(actual, expected)
    assert coeffs_a.shape == (len(fc), 3)
    assert coeffs_b.shape == (len(fc), 3)
