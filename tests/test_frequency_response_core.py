"""Targeted unit tests for the autoeq FrequencyResponse functions the BRIR
pipeline actually uses (#115 finding 5).

``autoeq/frequency_response.py`` (~1622 lines) computes the EQ FIR filters
applied by ``process_equalization_worker``; previously only two
allocation-regression tests touched it. These assert the numeric contracts of
the pipeline-exercised surface (frequency grid, smoothing, peaking biquad,
equalize clamp/inversion, minimum-phase IR). The vendored module is not
modified.
"""

from __future__ import annotations

import numpy as np
import pytest

from autoeq.frequency_response import FrequencyResponse


def _fr(**kw) -> FrequencyResponse:
    return FrequencyResponse(name="t", **kw)


# ── generate_frequencies ─────────────────────────────────────────────────────

def test_generate_frequencies_is_log_spaced_and_bounded() -> None:
    f = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.05)
    assert f[0] == 20
    assert f[-1] <= 20000
    assert np.all(np.diff(f) > 0)                       # strictly increasing
    np.testing.assert_allclose(f[1:] / f[:-1], 1.05)    # constant log step


# ── _biquad_eq_response ──────────────────────────────────────────────────────

def test_biquad_peaking_response_hits_target_gain_at_center() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.01)
    fc, q, gain = 1000.0, 1.0, 6.0
    response, coeffs_a, coeffs_b = FrequencyResponse._biquad_eq_response(
        freq, [fc], [q], [gain], fs=48000
    )
    idx = int(np.argmin(np.abs(freq - fc)))
    assert abs(response[idx] - gain) < 0.5      # +6 dB peak realized at fc
    assert coeffs_a.shape == (1, 3)
    assert coeffs_b.shape == (1, 3)


# ── center ───────────────────────────────────────────────────────────────────

def test_center_sets_reference_frequency_to_zero_db() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.02)
    fr = _fr(frequency=freq, raw=np.full(freq.shape, 5.0))
    fr.center(1000)
    idx = int(np.argmin(np.abs(freq - 1000)))
    assert abs(fr.raw[idx]) < 0.5               # flat +5 dB curve recentred to 0


# ── smoothen_fractional_octave ───────────────────────────────────────────────

def test_smoothen_preserves_shape_and_reduces_variation() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.01)
    rng = np.random.default_rng(7)
    noisy = rng.standard_normal(len(freq)) * 3.0
    fr = _fr(frequency=freq, raw=noisy.copy(), error=noisy.copy())
    fr.smoothen_fractional_octave(window_size=1 / 3, iterations=1)
    assert fr.smoothed.shape == freq.shape
    assert np.all(np.isfinite(fr.smoothed))
    assert np.std(fr.smoothed) < np.std(noisy)  # smoothing removes ripple


# ── equalize ─────────────────────────────────────────────────────────────────

def test_equalize_clamps_positive_gain_to_max_gain() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.02)
    # error -20 dB everywhere → wants +20 dB boost → must clamp to +6 dB
    fr = _fr(frequency=freq, raw=np.zeros(freq.shape), error=np.full(freq.shape, -20.0))
    fr.equalize(max_gain=6.0, smoothen=False, treble_max_gain=6.0, treble_gain_k=1.0)
    eq = np.asarray(fr.equalization)
    assert eq.shape == freq.shape
    assert np.all(np.isfinite(eq))
    np.testing.assert_allclose(eq, 6.0, atol=1e-9)


def test_equalize_inverts_unclipped_error() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.02)
    # mild +3 dB error → wants -3 dB cut → unclipped → equalization == -error
    fr = _fr(frequency=freq, raw=np.zeros(freq.shape), error=np.full(freq.shape, 3.0))
    fr.equalize(max_gain=6.0, smoothen=False, treble_max_gain=6.0, treble_gain_k=1.0)
    np.testing.assert_allclose(np.asarray(fr.equalization), -3.0, atol=1e-9)


def test_equalize_without_error_raises() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.05)
    fr = _fr(frequency=freq, raw=np.zeros(freq.shape))
    with pytest.raises(ValueError):
        fr.equalize()


# ── minimum_phase_impulse_response ───────────────────────────────────────────

def test_minimum_phase_ir_is_real_finite_and_front_loaded() -> None:
    freq = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.01)
    fr = _fr(frequency=freq, raw=np.zeros(freq.shape))
    fr.equalization = 3.0 * np.sin(np.log10(freq) * 2.0)   # mild non-trivial EQ
    ir = fr.minimum_phase_impulse_response(fs=48000, f_res=10, normalize=True)

    assert np.isrealobj(ir)
    assert np.all(np.isfinite(ir))
    assert len(ir) > 0
    energy = np.cumsum(np.asarray(ir) ** 2)
    # minimum-phase energy is front-loaded: >half the energy in the first half
    assert energy[len(ir) // 2] / energy[-1] > 0.5
