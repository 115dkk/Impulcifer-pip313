"""Tests for the headphone-compensation playback validator."""

from __future__ import annotations

import numpy as np
import soundfile as sf

from core.headphones_recording import (
    MAX_HEADPHONES_PLAYBACK_CHANNELS,
    inspect_headphones_playback,
)


def _write_wav(path: str, channels: int, samples: int = 1024, fs: int = 48000) -> str:
    if channels == 1:
        data = np.zeros(samples, dtype=np.float32)
    else:
        data = np.zeros((samples, channels), dtype=np.float32)
    sf.write(path, data, fs, subtype="PCM_16")
    return path


def test_empty_play_path_rejected_with_missing_reason() -> None:
    result = inspect_headphones_playback("")
    assert result.is_valid is False
    assert result.reason_key == "error_headphones_play_file_missing"
    assert result.channels == 0


def test_nonexistent_path_rejected_with_missing_reason(tmp_path) -> None:
    result = inspect_headphones_playback(str(tmp_path / "does_not_exist.wav"))
    assert result.is_valid is False
    assert result.reason_key == "error_headphones_play_file_missing"


def test_mono_play_file_is_accepted_with_mono_flag(tmp_path) -> None:
    wav = _write_wav(str(tmp_path / "mono.wav"), channels=1)
    result = inspect_headphones_playback(wav)
    assert result.is_valid is True
    assert result.is_mono is True
    assert result.channels == 1
    assert result.reason_key == ""


def test_stereo_play_file_is_accepted_without_mono_flag(tmp_path) -> None:
    wav = _write_wav(str(tmp_path / "stereo.wav"), channels=2)
    result = inspect_headphones_playback(wav)
    assert result.is_valid is True
    assert result.is_mono is False
    assert result.channels == 2


def test_multichannel_play_file_is_rejected_with_too_many_channels(tmp_path) -> None:
    """7.1.6 (14ch) sweep files must not flow into the headphones path."""
    # Use 8ch so it doesn't depend on which formats libsndfile supports for
    # larger channel counts on the CI machine.
    wav = _write_wav(str(tmp_path / "surround.wav"), channels=8)
    result = inspect_headphones_playback(wav)
    assert result.is_valid is False
    assert result.reason_key == "error_headphones_play_file_too_many_channels"
    assert result.channels == 8


def test_unreadable_file_rejected_with_unreadable_reason(tmp_path) -> None:
    junk = tmp_path / "junk.wav"
    junk.write_bytes(b"not a wav")
    result = inspect_headphones_playback(str(junk))
    assert result.is_valid is False
    assert result.reason_key == "error_headphones_play_file_unreadable"


def test_max_headphones_playback_channels_is_two() -> None:
    """The contract is exactly mono or stereo — nothing else."""
    assert MAX_HEADPHONES_PLAYBACK_CHANNELS == 2
