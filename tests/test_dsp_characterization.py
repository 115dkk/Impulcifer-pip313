"""Characterization tests for core HRIR DSP paths (audit #138 F039).

``channel_balance_firs`` (all seven method branches), ``crop_heads``,
``crop_tails`` and ``resample`` previously had zero direct tests — they were
covered only by the opaque Linux-only end-to-end hash. These tests pin their
numeric contracts with synthetic inputs so regressions surface on every
platform. Tolerances are deliberately loose (±1 dB / ±2 samples) — the tests
characterize behavior, they do not redefine it.
"""

from __future__ import annotations

import warnings

import numpy as np
import pytest

from autoeq.frequency_response import FrequencyResponse
from core.audio_io import magnitude_response
from core.hrir import HRIR
from core.impulse_response import ImpulseResponse

FS = 48_000


class _DummyEstimator:
    fs = FS


class _TailEstimator:
    fs = FS
    n_octaves = 10

    def __len__(self) -> int:
        return FS * 6


def _hrir(channels: dict, estimator=None) -> HRIR:
    hrir = HRIR(estimator or _DummyEstimator())
    hrir.irs = channels
    return hrir


def _ir(data: np.ndarray) -> ImpulseResponse:
    return ImpulseResponse(np.asarray(data, dtype=np.float64), FS)


def _impulse_at(index: int, length: int) -> np.ndarray:
    data = np.zeros(length)
    data[index] = 1.0
    return data


def _flat_fr(level_db: float) -> FrequencyResponse:
    frequency = FrequencyResponse.generate_frequencies(f_min=10, f_max=FS / 2, f_step=1.01)
    return FrequencyResponse(
        name="flat",
        frequency=frequency,
        raw=np.full(len(frequency), float(level_db)),
    )


def _fir_gain_db(fir: np.ndarray, at_hz: float = 1000.0) -> float:
    freqs, mags = magnitude_response(np.asarray(fir), FS)
    return float(mags[np.argmin(np.abs(freqs - at_hz))])


# ── channel_balance_firs() ─────────────────────────────────────────────────

def test_channel_balance_numeric_scales_right_side() -> None:
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(0.0), "6")

    assert len(firs) == 2
    assert np.argmax(np.abs(firs[0])) == 0 and firs[0][0] == pytest.approx(1.0)
    assert firs[1][0] == pytest.approx(10 ** (6 / 20), rel=1e-6)
    assert len(firs[0]) == int(round(FS * 0.1))


def test_channel_balance_invalid_method_raises() -> None:
    hrir = _hrir({})
    with pytest.raises(ValueError):
        hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(0.0), "not-a-number")


def test_channel_balance_mids_guesses_gain_from_mid_levels() -> None:
    """Right side 6 dB below left → right FIR boosts by ~+6 dB."""
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(-6.0), "mids")

    assert firs[0][0] == pytest.approx(1.0)
    assert firs[1][0] == pytest.approx(10 ** (6 / 20), rel=1e-3)


def test_channel_balance_trend_equalizes_right_to_left() -> None:
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(-6.0), "trend")

    # Left stays a unit impulse; right gets ~+6 dB broadband.
    assert np.argmax(np.abs(firs[0])) == 0
    assert _fir_gain_db(firs[1]) == pytest.approx(6.0, abs=1.0)


def test_channel_balance_left_reference() -> None:
    """method='left': right side is equalized towards the left response."""
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(-6.0), "left")

    assert np.argmax(np.abs(firs[0])) == 0
    assert _fir_gain_db(firs[1]) == pytest.approx(6.0, abs=1.0)


def test_channel_balance_right_reference() -> None:
    """method='right': left side is equalized towards the right response."""
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(-6.0), "right")

    assert np.argmax(np.abs(firs[1])) == 0
    assert _fir_gain_db(firs[0]) == pytest.approx(-6.0, abs=1.0)


def test_channel_balance_avg_meets_in_the_middle() -> None:
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(-6.0), "avg")

    assert _fir_gain_db(firs[0]) == pytest.approx(-3.0, abs=1.0)
    assert _fir_gain_db(firs[1]) == pytest.approx(3.0, abs=1.0)


def test_channel_balance_min_takes_the_lower_response() -> None:
    hrir = _hrir({})
    firs = hrir.channel_balance_firs(_flat_fr(0.0), _flat_fr(-6.0), "min")

    assert _fir_gain_db(firs[0]) == pytest.approx(-6.0, abs=1.0)
    assert _fir_gain_db(firs[1]) == pytest.approx(0.0, abs=1.0)


# ── crop_heads() ───────────────────────────────────────────────────────────

def test_crop_heads_moves_first_peak_to_head_offset() -> None:
    """Left-side speaker: crop leaves head_ms of lead-in before the first peak
    and preserves the interaural delay."""
    length = 4800
    hrir = _hrir({
        "FL": {
            "left": _ir(_impulse_at(480, length)),
            "right": _ir(_impulse_at(528, length)),
        },
    })

    hrir.crop_heads(head_ms=1)

    head = int(FS * 1 / 1000)
    assert hrir.irs["FL"]["left"].peak_index() == head
    assert hrir.irs["FL"]["right"].peak_index() == head + 48  # ITD preserved
    assert len(hrir.irs["FL"]["left"].data) == length - (480 - head)


def test_crop_heads_warns_on_side_mismatch_including_top_layer() -> None:
    """A right-side speaker whose sound reaches the left ear first must warn.

    Includes a 3-letter top-layer name: the old positional speaker[1] check
    silently skipped TFR/TSL/… (audit #138 F024 — fixed via speaker_side()).
    """
    length = 4800
    for speaker in ("FR", "TFR"):
        hrir = _hrir({
            speaker: {
                "left": _ir(_impulse_at(480, length)),
                "right": _ir(_impulse_at(528, length)),
            },
        })
        with pytest.warns(UserWarning, match=speaker):
            hrir.crop_heads(head_ms=1)


def test_crop_heads_correct_side_does_not_warn() -> None:
    length = 4800
    hrir = _hrir({
        "FL": {
            "left": _ir(_impulse_at(480, length)),
            "right": _ir(_impulse_at(528, length)),
        },
    })
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        hrir.crop_heads(head_ms=1)


# ── crop_tails() ───────────────────────────────────────────────────────────

def test_crop_tails_trims_to_uniform_faded_length() -> None:
    rng = np.random.default_rng(7)
    length = FS  # 1 second
    t = np.arange(length) / FS

    def _decaying() -> np.ndarray:
        data = rng.standard_normal(length) * np.exp(-t * 30.0) * 0.5
        data[100] = 1.0
        return data

    hrir = _hrir(
        {
            "FL": {"left": _ir(_decaying()), "right": _ir(_decaying())},
            "FR": {"left": _ir(_decaying()), "right": _ir(_decaying())},
        },
        estimator=_TailEstimator(),
    )

    tail_ind = hrir.crop_tails()

    assert 0 < tail_ind <= length
    for pair in hrir.irs.values():
        for ir in pair.values():
            assert len(ir.data) == tail_ind
            # Fade-out window pulls the very end to (near) zero.
            assert abs(ir.data[-1]) < 1e-6


def test_crop_tails_rejects_mismatched_sampling_rate() -> None:
    hrir = _hrir({"FL": {"left": _ir(_impulse_at(0, 100)), "right": _ir(_impulse_at(0, 100))}})
    hrir.fs = 44_100
    with pytest.raises(ValueError):
        hrir.crop_tails()


# ── resample() ─────────────────────────────────────────────────────────────

def test_resample_updates_rate_and_scales_length() -> None:
    length = FS // 2
    hrir = _hrir({
        "FL": {"left": _ir(_impulse_at(1000, length)), "right": _ir(_impulse_at(1000, length))},
    })

    hrir.resample(44_100)

    assert hrir.fs == 44_100
    expected = length * 44_100 / FS
    for ir in hrir.irs["FL"].values():
        assert len(ir.data) == pytest.approx(expected, abs=2)
        assert ir.fs == 44_100
