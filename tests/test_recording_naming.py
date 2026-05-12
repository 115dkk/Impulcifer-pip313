"""Tests for canonical recording filename derivation."""

from __future__ import annotations

import os

from core.recording_naming import (
    HEADPHONES_FILENAME,
    derive_record_filename,
    headphones_record_filename,
    resolve_headphones_record_path,
    resolve_record_path,
)


def test_segmented_sweep_with_speaker_pair_produces_pair_wav() -> None:
    name = derive_record_filename(
        "data/sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
    )
    assert name == "FL,FR.wav"


def test_segmented_sweep_with_single_speaker_produces_single_wav() -> None:
    name = derive_record_filename(
        "data/sweep-seg-FC-mono-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
    )
    assert name == "FC.wav"


def test_segmented_sweep_supports_three_letter_height_speakers() -> None:
    name = derive_record_filename(
        "data/sweep-seg-TFL,TFR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
    )
    assert name == "TFL,TFR.wav"


def test_plain_mono_sweep_no_longer_auto_maps_to_headphones() -> None:
    """Regression: speaker-side derivation must not silently emit ``headphones.wav``.

    Playing the bundled mono sweep simultaneously on both headphone
    drivers can't distinguish L vs R response, so producing
    ``headphones.wav`` from it was misleading. The dedicated
    ``Record headphones`` button is the only path that should yield
    that filename now.
    """
    name = derive_record_filename(
        "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
    )
    assert name != HEADPHONES_FILENAME
    assert name == "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"


def test_unknown_play_file_keeps_basename_with_wav_extension() -> None:
    name = derive_record_filename("custom_signal.flac")
    assert name == "custom_signal.wav"


def test_resolve_record_path_joins_folder_and_derived_name() -> None:
    path = resolve_record_path(
        os.path.join("data", "my_hrir"),
        "data/sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
    )
    assert os.path.basename(path) == "FL,FR.wav"
    assert os.path.dirname(path) == os.path.join("data", "my_hrir")


def test_headphones_filename_is_constant() -> None:
    assert headphones_record_filename() == HEADPHONES_FILENAME == "headphones.wav"


def test_resolve_headphones_record_path_uses_canonical_filename() -> None:
    path = resolve_headphones_record_path(os.path.join("data", "my_hrir"))
    assert os.path.basename(path) == HEADPHONES_FILENAME
    assert os.path.dirname(path) == os.path.join("data", "my_hrir")


def test_empty_play_path_falls_back_to_headphones_filename_for_preview_label() -> None:
    """Empty-path edge case is preserved so the GUI preview label has something to show."""
    assert derive_record_filename("") == HEADPHONES_FILENAME
