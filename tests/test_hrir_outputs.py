"""Focused tests for HRIR output helpers."""

from __future__ import annotations

import numpy as np

from core.hrir import HRIR
from core.impulse_response import ImpulseResponse


class DummyEstimator:
    fs = 48_000


def _make_hrir() -> HRIR:
    hrir = HRIR(DummyEstimator())
    hrir.irs = {
        "FL": {
            "left": ImpulseResponse(np.array([1.0, 2.0, 3.0, 4.0]), hrir.fs),
            "right": ImpulseResponse(np.array([5.0, 6.0, 7.0, 8.0]), hrir.fs),
        },
        "FR": {
            "left": ImpulseResponse(np.array([9.0, 10.0, 11.0, 12.0]), hrir.fs),
            "right": ImpulseResponse(np.array([13.0, 14.0, 15.0, 16.0]), hrir.fs),
        },
    }
    return hrir


def test_write_wav_stacks_only_requested_track_order(monkeypatch) -> None:
    """Subset outputs should not first materialize every HRIR channel."""
    hrir = _make_hrir()
    captured = {}

    def fake_write_wav(file_path, fs, data, bit_depth=32):
        captured["file_path"] = file_path
        captured["fs"] = fs
        captured["data"] = data
        captured["bit_depth"] = bit_depth

    monkeypatch.setattr("core.hrir.write_wav", fake_write_wav)

    hrir.write_wav("subset.wav", track_order=["FR-left", "MISSING-right"], bit_depth=24)

    assert captured["file_path"] == "subset.wav"
    assert captured["fs"] == 48_000
    assert captured["bit_depth"] == 24
    np.testing.assert_array_equal(
        captured["data"],
        np.array(
            [
                [9.0, 10.0, 11.0, 12.0],
                [0.0, 0.0, 0.0, 0.0],
            ]
        ),
    )


def test_subset_can_copy_only_requested_speakers() -> None:
    """JamesDSP-style subsets should avoid deep-copying unrelated speakers."""
    hrir = _make_hrir()

    subset = hrir.subset(["FL"], copy_irs=True)
    assert list(subset.irs) == ["FL"]
    assert subset.irs["FL"]["left"] is not hrir.irs["FL"]["left"]

    subset.irs["FL"]["left"].data[0] = 99.0
    assert hrir.irs["FL"]["left"].data[0] == 1.0
