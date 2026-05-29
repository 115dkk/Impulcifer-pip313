"""Tests for the surround sweep set generator.

The generator emits two complementary forms (see
:mod:`core.sweep_set_generator`):

* per-group **stereo** 2-channel files for stereo-only capture, and
* one combined **7.1** 8-channel file for a real multichannel rig.
"""

from __future__ import annotations

import os

import numpy as np
import soundfile as sf

from core.impulse_response_estimator import ImpulseResponseEstimator
from core.recording_naming import derive_record_filename
from core.sweep_set_generator import (
    COMBINED_SPEAKERS,
    COMBINED_TRACKS,
    STEREO_GROUPS,
    generate_sweep_set,
)


def _group_token(path: str) -> str:
    """Pull the speaker-group token out of a ``sweep-seg-<group>-…`` name."""
    return os.path.basename(path).split("-")[2]


def test_sweep_set_writes_stereo_groups_plus_combined(tmp_path) -> None:
    paths = generate_sweep_set(
        str(tmp_path),
        duration=2.0,  # short to keep the test fast
        fs=48000,
        bit_depth=16,  # smaller files for the temporary test
    )

    # One file per stereo group plus a single combined file.
    assert len(paths) == len(STEREO_GROUPS) + 1
    for path in paths:
        assert os.path.exists(path), f"missing sweep file: {path}"
        assert os.path.basename(path).startswith("sweep-seg-")


def test_stereo_group_files_are_two_channel(tmp_path) -> None:
    paths = generate_sweep_set(str(tmp_path), duration=2.0, bit_depth=16)
    stereo_tokens = {",".join(group) for group in STEREO_GROUPS}

    seen_tokens = set()
    for path in paths:
        token = _group_token(path)
        if token not in stereo_tokens:
            continue
        seen_tokens.add(token)

        info = sf.info(path)
        assert info.channels == 2, f"{path} should be a stereo file"
        assert info.samplerate == 48000

        data, _ = sf.read(path)
        if data.ndim == 1:
            data = data[:, np.newaxis]
        # soundfile returns ``(samples, channels)``; transpose for easier
        # per-channel inspection.
        data = data.T

        # Speakers map positionally to the two output channels
        # (speakers[0] -> ch0, speakers[1] -> ch1).
        speakers = token.split(",")
        active = [i for i in range(data.shape[0]) if np.max(np.abs(data[i])) > 1e-6]
        assert active == list(range(len(speakers)))

    assert seen_tokens == stereo_tokens


def test_combined_file_is_eight_channel_71_with_all_ground_speakers(tmp_path) -> None:
    paths = generate_sweep_set(str(tmp_path), duration=2.0, bit_depth=16)
    layout = "FL FR FC LFE BL BR SL SR".split()

    combined = [p for p in paths if _group_token(p) == ",".join(COMBINED_SPEAKERS)]
    assert len(combined) == 1
    assert COMBINED_TRACKS == "7.1"

    info = sf.info(combined[0])
    assert info.channels == 8, "combined file should be a full 7.1 layout"
    assert info.samplerate == 48000

    data, _ = sf.read(combined[0])
    data = data.T
    active = {layout[i] for i in range(data.shape[0]) if np.max(np.abs(data[i])) > 1e-6}
    # Every ground speaker is present; LFE stays silent.
    assert active == set(COMBINED_SPEAKERS)


def test_sweep_set_filenames_round_trip_through_record_name_derivation(tmp_path) -> None:
    """Each generated sweep must map back to its canonical recording name.

    This is the contract that lets the recorder's folder mode auto-name
    output WAVs — the stereo groups derive ``FL,FR.wav`` / ``FC.wav`` /
    ``SL,SR.wav`` / ``BL,BR.wav`` and the combined file derives
    ``FL,FR,FC,SL,SR,BL,BR.wav``.
    """
    paths = generate_sweep_set(str(tmp_path), duration=2.0, bit_depth=16)
    derived = {derive_record_filename(path) for path in paths}
    assert derived == {
        "FL,FR.wav",
        "FC.wav",
        "SL,SR.wav",
        "BL,BR.wav",
        "FL,FR,FC,SL,SR,BL,BR.wav",
    }


def test_filenames_do_not_imply_height_channels(tmp_path) -> None:
    """Filenames must not advertise height channels (no ``7.1.6`` tag)."""
    paths = generate_sweep_set(str(tmp_path), duration=2.0, bit_depth=16)
    for path in paths:
        assert "7.1.6" not in os.path.basename(path)


def test_stereo_sweep_sequence_supports_repositioned_groups() -> None:
    """``stereo`` tracks must accept any one/two speakers, not just FL/FR."""
    ire = ImpulseResponseEstimator(min_duration=2.0, fs=48000)
    for group in (["SL", "SR"], ["BL", "BR"], ["FC"]):
        data = ire.sweep_sequence(group, "stereo")
        assert data.shape[0] == 2
        active = [i for i in range(2) if np.max(np.abs(data[i])) > 1e-6]
        assert active == list(range(len(group)))
