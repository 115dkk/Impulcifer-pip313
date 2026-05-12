"""Tests for the 14-channel surround sweep set generator."""

from __future__ import annotations

import os

import numpy as np
import soundfile as sf

from core.impulse_response_estimator import ImpulseResponseEstimator
from core.recording_naming import derive_record_filename
from core.sweep_set_generator import (
    DEFAULT_TRACKS,
    SWEEP_SET_GROUPS,
    generate_sweep_set,
)


def test_default_sweep_set_writes_four_groups(tmp_path) -> None:
    paths = generate_sweep_set(
        str(tmp_path),
        duration=2.0,  # short to keep the test fast
        fs=48000,
        bit_depth=16,  # smaller files for the temporary test
    )

    assert len(paths) == len(SWEEP_SET_GROUPS) == 4
    expected_groups = {",".join(group) for group in SWEEP_SET_GROUPS}
    written_groups = set()
    for path in paths:
        assert os.path.exists(path), f"missing sweep file: {path}"
        basename = os.path.basename(path)
        assert basename.startswith("sweep-seg-")
        # Strip prefix and pull out the speaker group token.
        group_token = basename.split("-")[2]
        written_groups.add(group_token)
    assert written_groups == expected_groups


def test_sweep_set_uses_14_channel_atmos_layout(tmp_path) -> None:
    paths = generate_sweep_set(str(tmp_path), duration=2.0, bit_depth=16)
    layout = "FL FR FC LFE BL BR SL SR TFL TFR TBL TBR TSL TSR".split()

    for path in paths:
        info = sf.info(path)
        assert info.channels == 14, f"{path} should be 14-channel"
        assert info.samplerate == 48000

        data, _ = sf.read(path)
        if data.ndim == 1:
            data = data[:, np.newaxis]
        # soundfile returns ``(samples, channels)``; transpose for easier
        # per-channel inspection.
        data = data.T

        # Active channels should match the speaker group encoded in the
        # filename — every other channel must be empty.
        basename = os.path.basename(path)
        group_token = basename.split("-")[2]
        expected_active = group_token.split(",")
        active = [
            layout[i]
            for i in range(data.shape[0])
            if np.max(np.abs(data[i])) > 1e-6
        ]
        assert active == expected_active


def test_sweep_set_filenames_round_trip_through_record_name_derivation(tmp_path) -> None:
    """Each generated sweep should map back to its canonical recording name.

    This is the contract that lets the recorder's folder mode auto-name
    output WAVs — ``sweep-seg-FL,FR-7.1.6-…wav`` must produce
    ``FL,FR.wav`` and ``sweep-seg-FC-7.1.6-…wav`` must produce
    ``FC.wav``.
    """
    paths = generate_sweep_set(str(tmp_path), duration=2.0, bit_depth=16)
    derived = {derive_record_filename(path) for path in paths}
    assert derived == {"FL,FR.wav", "FC.wav", "SL,SR.wav", "BL,BR.wav"}


def test_sweep_sequence_supports_height_speaker_in_716_layout() -> None:
    """The new ``7.1.6`` layout must accept TFL/TFR/TBL/TBR/TSL/TSR."""
    ire = ImpulseResponseEstimator(min_duration=2.0, fs=48000)
    data = ire.sweep_sequence(["TFL", "TFR"], "7.1.6")
    layout = "FL FR FC LFE BL BR SL SR TFL TFR TBL TBR TSL TSR".split()

    assert data.shape[0] == 14
    active = [layout[i] for i in range(data.shape[0]) if np.max(np.abs(data[i])) > 1e-6]
    assert active == ["TFL", "TFR"]


def test_sweep_sequence_default_tracks_is_716_for_generator() -> None:
    """The generator default must stay ``7.1.6`` so callers get 14-ch files."""
    assert DEFAULT_TRACKS == "7.1.6"
