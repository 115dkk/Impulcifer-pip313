"""Tests for rebuilding Impulcifer outputs from surviving representations."""

from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest
import soundfile as sf

from core.audio_io import read_wav, write_wav
from core.brir_recovery import BrirRecoveryError, recover_brir_outputs
from core.constants import (
    HESUVI_TRACK_ORDER,
    HEXADECAGONAL_TRACK_ORDER,
    SPEAKER_NAMES,
)

FS = 48_000
SAMPLE_COUNT = 16


def _speaker_tracks(speakers: tuple[str, ...]) -> dict[str, np.ndarray]:
    tracks: dict[str, np.ndarray] = {}
    for index, speaker in enumerate(speakers):
        left = np.zeros(SAMPLE_COUNT, dtype=np.float64)
        right = np.zeros(SAMPLE_COUNT, dtype=np.float64)
        left[0] = (index * 2 + 1) / 256
        left[3] = -(index * 2 + 1) / 512
        right[1] = (index * 2 + 2) / 256
        right[4] = -(index * 2 + 2) / 512
        tracks[f"{speaker}-left"] = left
        tracks[f"{speaker}-right"] = right
    return tracks


def _stack(
    tracks: dict[str, np.ndarray],
    order: tuple[str, ...] | list[str],
) -> np.ndarray:
    silence = np.zeros(SAMPLE_COUNT, dtype=np.float64)
    return np.vstack([tracks.get(name, silence) for name in order])


def _write_combined(path: Path, tracks: dict[str, np.ndarray], order: list[str]) -> None:
    sf.write(str(path), _stack(tracks, order).T, FS, subtype="PCM_32")


def _read_matrix(path: Path) -> tuple[int, np.ndarray]:
    sample_rate, data = read_wav(str(path), expand=True)
    return sample_rate, np.asarray(data)


def test_hangloose_directory_rebuilds_both_canonical_layouts(tmp_path: Path) -> None:
    output_dir = tmp_path / "measurement"
    split_dir = output_dir / "Hangloose"
    split_dir.mkdir(parents=True)
    speakers = ("FL", "FR", "FC", "SL", "SR", "TFL", "TFR")
    tracks = _speaker_tracks(speakers)
    for speaker in speakers:
        write_wav(
            str(split_dir / f"{speaker}.wav"),
            FS,
            np.vstack(
                (tracks[f"{speaker}-left"], tracks[f"{speaker}-right"])
            ),
            bit_depth=32,
        )

    result = recover_brir_outputs(split_dir)

    assert result.source_kind == "hangloose"
    assert Path(result.output_dir) == output_dir.resolve()
    assert result.speakers == speakers
    assert {Path(path).name for path in result.created_files} == {
        "hrir.wav",
        "hesuvi.wav",
    }

    hrir_rate, hrir = _read_matrix(output_dir / "hrir.wav")
    hesuvi_rate, hesuvi = _read_matrix(output_dir / "hesuvi.wav")
    assert hrir_rate == hesuvi_rate == FS
    np.testing.assert_array_equal(hrir, _stack(tracks, HEXADECAGONAL_TRACK_ORDER))
    np.testing.assert_array_equal(hesuvi, _stack(tracks, HESUVI_TRACK_ORDER))


def test_prefixed_hangloose_filenames_rebuild_both_layouts(tmp_path: Path) -> None:
    speakers = ("FL", "FR", "FC", "BL", "BR", "SL", "SR")
    tracks = _speaker_tracks(speakers)
    prefix = "400se7chdelay"
    for speaker in speakers:
        write_wav(
            str(tmp_path / f"{prefix}{speaker}.wav"),
            FS,
            np.vstack(
                (tracks[f"{speaker}-left"], tracks[f"{speaker}-right"])
            ),
            bit_depth=32,
        )

    result = recover_brir_outputs(tmp_path)

    assert result.source_kind == "hangloose"
    assert result.speakers == speakers
    assert {Path(path).name for path in result.created_files} == {
        "hrir.wav",
        "hesuvi.wav",
    }
    assert {Path(path).name for path in result.existing_files} == {
        f"{prefix}{speaker}.wav" for speaker in speakers
    }
    _, hrir = _read_matrix(tmp_path / "hrir.wav")
    _, hesuvi = _read_matrix(tmp_path / "hesuvi.wav")
    np.testing.assert_array_equal(hrir, _stack(tracks, HEXADECAGONAL_TRACK_ORDER))
    np.testing.assert_array_equal(hesuvi, _stack(tracks, HESUVI_TRACK_ORDER))


def test_longest_speaker_suffix_wins_for_prefixed_top_channel(tmp_path: Path) -> None:
    tracks = _speaker_tracks(("TFL",))
    write_wav(
        str(tmp_path / "profileTFL.wav"),
        FS,
        np.vstack((tracks["TFL-left"], tracks["TFL-right"])),
        bit_depth=32,
    )

    result = recover_brir_outputs(tmp_path)

    assert result.speakers == ("TFL",)


def test_mixed_hangloose_filename_prefixes_are_ambiguous(tmp_path: Path) -> None:
    tracks = _speaker_tracks(("FL", "FR"))
    for filename, speaker in (("firstFL.wav", "FL"), ("secondFR.wav", "FR")):
        write_wav(
            str(tmp_path / filename),
            FS,
            np.vstack(
                (tracks[f"{speaker}-left"], tracks[f"{speaker}-right"])
            ),
            bit_depth=32,
        )

    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path)

    assert error.value.code == "AMBIGUOUS_SOURCE"
    assert not (tmp_path / "hrir.wav").exists()
    assert not (tmp_path / "hesuvi.wav").exists()


def test_hrir_only_restores_hesuvi_and_optional_hangloose(tmp_path: Path) -> None:
    speakers = ("FL", "FR", "FC", "BL", "BR", "SL", "SR")
    tracks = _speaker_tracks(speakers)
    hrir_path = tmp_path / "hrir.wav"
    _write_combined(hrir_path, tracks, HEXADECAGONAL_TRACK_ORDER)
    original_hrir = hrir_path.read_bytes()

    result = recover_brir_outputs(tmp_path, include_hangloose=True)

    assert result.source_kind == "hrir"
    assert hrir_path.read_bytes() == original_hrir
    _, hesuvi = _read_matrix(tmp_path / "hesuvi.wav")
    np.testing.assert_array_equal(hesuvi, _stack(tracks, HESUVI_TRACK_ORDER))
    assert {
        path.stem for path in (tmp_path / "Hangloose").glob("*.wav")
    } == set(speakers)
    for speaker in speakers:
        _, split = _read_matrix(tmp_path / "Hangloose" / f"{speaker}.wav")
        np.testing.assert_array_equal(
            split,
            np.vstack((tracks[f"{speaker}-left"], tracks[f"{speaker}-right"])),
        )


def test_hesuvi_only_restores_hrir_with_silent_lfe(tmp_path: Path) -> None:
    speakers = ("FL", "FR", "SL", "SR")
    tracks = _speaker_tracks(speakers)
    hesuvi_path = tmp_path / "hesuvi.wav"
    _write_combined(hesuvi_path, tracks, HESUVI_TRACK_ORDER)
    original_hesuvi = hesuvi_path.read_bytes()

    result = recover_brir_outputs(tmp_path)

    assert result.source_kind == "hesuvi"
    assert hesuvi_path.read_bytes() == original_hesuvi
    _, hrir = _read_matrix(tmp_path / "hrir.wav")
    np.testing.assert_array_equal(hrir, _stack(tracks, HEXADECAGONAL_TRACK_ORDER))
    lfe_left = HEXADECAGONAL_TRACK_ORDER.index("LFE-left")
    lfe_right = HEXADECAGONAL_TRACK_ORDER.index("LFE-right")
    assert not np.any(hrir[[lfe_left, lfe_right]])


def test_complete_matching_pair_is_validated_without_overwrite(tmp_path: Path) -> None:
    tracks = _speaker_tracks(tuple(SPEAKER_NAMES))
    hrir_path = tmp_path / "hrir.wav"
    hesuvi_path = tmp_path / "hesuvi.wav"
    _write_combined(hrir_path, tracks, HEXADECAGONAL_TRACK_ORDER)
    _write_combined(hesuvi_path, tracks, HESUVI_TRACK_ORDER)
    before = {path: path.read_bytes() for path in (hrir_path, hesuvi_path)}

    result = recover_brir_outputs(tmp_path)

    assert result.source_kind == "hrir+hesuvi"
    assert result.created_files == ()
    assert {Path(path) for path in result.existing_files} == {hrir_path, hesuvi_path}
    assert {path: path.read_bytes() for path in before} == before


def test_mismatched_split_files_fail_before_creating_outputs(tmp_path: Path) -> None:
    split_dir = tmp_path / "Hangloose"
    split_dir.mkdir()
    tracks = _speaker_tracks(("FL", "FR"))
    write_wav(
        str(split_dir / "FL.wav"),
        FS,
        np.vstack((tracks["FL-left"], tracks["FL-right"])),
        bit_depth=32,
    )
    write_wav(
        str(split_dir / "FR.wav"),
        44_100,
        np.vstack((tracks["FR-left"], tracks["FR-right"])),
        bit_depth=32,
    )

    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path)

    assert error.value.code == "SAMPLE_RATE_MISMATCH"
    assert not (tmp_path / "hrir.wav").exists()
    assert not (tmp_path / "hesuvi.wav").exists()


def test_non_silent_hrir_lfe_is_not_discarded_silently(tmp_path: Path) -> None:
    tracks = _speaker_tracks(("FL", "FR"))
    data = _stack(tracks, HEXADECAGONAL_TRACK_ORDER)
    data[HEXADECAGONAL_TRACK_ORDER.index("LFE-left"), 0] = 0.25
    sf.write(str(tmp_path / "hrir.wav"), data.T, FS, subtype="PCM_32")

    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path)

    assert error.value.code == "NON_SILENT_LFE"
    assert not (tmp_path / "hesuvi.wav").exists()


def test_existing_hangloose_file_must_match_combined_source(tmp_path: Path) -> None:
    tracks = _speaker_tracks(("FL", "FR"))
    _write_combined(tmp_path / "hrir.wav", tracks, HEXADECAGONAL_TRACK_ORDER)
    split_dir = tmp_path / "Hangloose"
    split_dir.mkdir()
    wrong = np.vstack((tracks["FL-left"], tracks["FL-right"])).copy()
    wrong[0, 0] *= -1
    write_wav(str(split_dir / "FL.wav"), FS, wrong, bit_depth=32)

    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path, include_hangloose=True)

    assert error.value.code == "SOURCE_MISMATCH"
    assert not (tmp_path / "hesuvi.wav").exists()
    assert not (split_dir / "FR.wav").exists()
