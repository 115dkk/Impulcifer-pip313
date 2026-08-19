"""Cross-platform per-stage DSP numeric assertions (#115 finding 1).

The end-to-end BRIR SHA-256 guard (``tests/test_brir_integrity.py``) only runs on
Linux/CPython 3.13, so a one-sample alignment error or a gain bug is invisible
on every other platform. These tests pin the *numeric contract* of individual,
pure DSP stages so they run everywhere. They assert behavior of existing code
only — no runtime code is modified, so the BRIR hash is unaffected.
"""

from __future__ import annotations

import numpy as np
from scipy import signal

from core.audio_io import magnitude_response
from core.hrir import HRIR
from core.impulse_response import ImpulseResponse


class _DummyEstimator:
    fs = 48_000


def _hrir(channels: dict) -> HRIR:
    hrir = HRIR(_DummyEstimator())
    hrir.irs = channels
    return hrir


def _ir(data: np.ndarray) -> ImpulseResponse:
    return ImpulseResponse(np.asarray(data, dtype=np.float64), 48_000)


def _summed_peak_db(hrir: HRIR) -> float:
    left = np.sum(np.vstack([p["left"].data for p in hrir.irs.values()]), axis=0)
    right = np.sum(np.vstack([p["right"].data for p in hrir.irs.values()]), axis=0)
    _, mr_l = magnitude_response(left, 48_000)
    _, mr_r = magnitude_response(right, 48_000)
    return float(np.max(np.vstack([mr_l, mr_r])))


def _summed_mid_avg_db(hrir: HRIR) -> float:
    left = np.sum(np.vstack([p["left"].data for p in hrir.irs.values()]), axis=0)
    right = np.sum(np.vstack([p["right"].data for p in hrir.irs.values()]), axis=0)
    f_l, mr_l = magnitude_response(left, 48_000)
    f_r, mr_r = magnitude_response(right, 48_000)
    band = np.concatenate(
        [
            mr_l[np.logical_and(f_l > 80, f_l < 6000)],
            mr_r[np.logical_and(f_r > 80, f_r < 6000)],
        ]
    )
    return float(np.mean(band))


# ── normalize() ────────────────────────────────────────────────────────────

def test_normalize_peak_target_brings_summed_peak_to_target() -> None:
    """normalize(peak_target=X) must leave the summed-channel magnitude peak at X dB."""
    rng = np.random.default_rng(1)
    hrir = _hrir(
        {
            "FL": {"left": _ir(rng.standard_normal(4096) * 0.3), "right": _ir(rng.standard_normal(4096) * 0.1)},
            "FR": {"left": _ir(rng.standard_normal(4096) * 0.05), "right": _ir(rng.standard_normal(4096) * 0.2)},
        }
    )
    target = -0.1
    gain_db = hrir.normalize(peak_target=target)

    assert isinstance(gain_db, float)
    assert abs(_summed_peak_db(hrir) - target) < 1e-6


def test_normalize_avg_target_brings_mid_average_to_target() -> None:
    """normalize(avg_target=X) must leave the 80-6000 Hz summed average at X dB."""
    rng = np.random.default_rng(2)
    hrir = _hrir(
        {
            "FL": {"left": _ir(rng.standard_normal(4096) * 0.4), "right": _ir(rng.standard_normal(4096) * 0.25)},
            "FR": {"left": _ir(rng.standard_normal(4096) * 0.15), "right": _ir(rng.standard_normal(4096) * 0.3)},
        }
    )
    target = -12.0
    hrir.normalize(peak_target=None, avg_target=target)

    assert abs(_summed_mid_avg_db(hrir) - target) < 1e-6


def test_normalize_requires_exactly_one_target() -> None:
    hrir = _hrir({"FL": {"left": _ir([1.0, 0.0]), "right": _ir([1.0, 0.0])}})
    import pytest

    with pytest.raises(ValueError):
        hrir.normalize(peak_target=-0.1, avg_target=-12.0)


# ── align_ipsilateral_all() ──────────────────────────────────────────────────

def _residual_lag(a: np.ndarray, b: np.ndarray, segment_len: int) -> int:
    corr = signal.correlate(a[:segment_len], b[:segment_len], mode="full")
    lags = np.arange(-len(a[:segment_len]) + 1, len(a[:segment_len]))
    return int(lags[np.argmax(corr)])


def test_align_ipsilateral_reduces_cross_pair_lag_to_zero() -> None:
    """After alignment the ipsilateral pair (FL.left vs FR.right) has ~0 residual lag."""
    n = 2048
    delay = 7

    def impulse_at(i: int) -> ImpulseResponse:
        d = np.zeros(n)
        d[i] = 1.0
        return _ir(d)

    hrir = _hrir(
        {
            "FL": {"left": impulse_at(60), "right": impulse_at(60)},
            "FR": {"left": impulse_at(60), "right": impulse_at(60 + delay)},
        }
    )
    before = _residual_lag(hrir.irs["FL"]["left"].data, hrir.irs["FR"]["right"].data, 1440)
    assert abs(before) == delay  # sanity: the injected lag is present

    hrir.align_ipsilateral_all(speaker_pairs=[("FL", "FR")], segment_ms=30)

    after = _residual_lag(hrir.irs["FL"]["left"].data, hrir.irs["FR"]["right"].data, 1440)
    assert abs(after) <= 1


def test_align_ipsilateral_preserves_length() -> None:
    n = 1024
    rng = np.random.default_rng(3)

    def r() -> ImpulseResponse:
        return _ir(rng.standard_normal(n))

    hrir = _hrir({"FL": {"left": r(), "right": r()}, "FR": {"left": r(), "right": r()}})
    hrir.align_ipsilateral_all(speaker_pairs=[("FL", "FR")], segment_ms=30)

    for pair in hrir.irs.values():
        for ir in pair.values():
            assert len(ir.data) == n


# ── ImpulseResponse.shift() (A4) ─────────────────────────────────────────────

def test_shift_positive_delays_and_truncates() -> None:
    ir = _ir([1.0, 2.0, 3.0, 4.0])
    ir.shift(2)  # delay by 2 → prepend 2 zeros, drop the tail to keep length
    np.testing.assert_array_equal(ir.data, np.array([0.0, 0.0, 1.0, 2.0]))


def test_shift_negative_advances_and_zero_pads() -> None:
    ir = _ir([1.0, 2.0, 3.0, 4.0])
    ir.shift(-1)  # advance by 1 → drop the leading sample, zero-pad the tail
    np.testing.assert_array_equal(ir.data, np.array([2.0, 3.0, 4.0, 0.0]))


def test_shift_zero_is_noop_and_length_is_preserved() -> None:
    ir = _ir([1.0, 2.0, 3.0, 4.0])
    ir.shift(0)
    np.testing.assert_array_equal(ir.data, np.array([1.0, 2.0, 3.0, 4.0]))
    ir.shift(3)
    assert len(ir.data) == 4
    ir.shift(-3)
    assert len(ir.data) == 4


# ── _ingest_recording sample-rate guard (f-4 regression) ─────────────────────

def test_ingest_recording_rejects_mismatched_sample_rate(tmp_path) -> None:
    """A recording whose rate != the estimator rate must raise (the guard that
    f-4's extraction must preserve)."""
    import pytest
    import soundfile as sf
    from core.hrir import _ingest_recording

    class _Est:
        fs = 48_000

    wav = tmp_path / "rec.wav"
    sf.write(str(wav), np.zeros(4096, dtype=np.float32), 44_100)  # != estimator 48 kHz
    with pytest.raises(ValueError):
        _ingest_recording(_Est(), 48_000, str(wav), ["FL"], side="left")
