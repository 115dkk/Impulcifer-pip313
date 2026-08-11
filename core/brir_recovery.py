"""Recover Impulcifer output sets without running the DSP pipeline again.

Impulcifer writes the same speaker/ear impulse responses in three layouts:

* ``hrir.wav`` uses :data:`HEXADECAGONAL_TRACK_ORDER` (32 tracks, including
  two silent LFE placeholders).
* ``hesuvi.wav`` uses :data:`HESUVI_TRACK_ORDER` (30 tracks).
* ``Hangloose/<speaker>.wav`` stores one stereo file per measured speaker.

This module validates one surviving representation and rebuilds the missing
representations by channel reordering only.  It never applies gain, DSP, or
resampling.
"""

from __future__ import annotations

import os
import tempfile
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import soundfile as sf

from core.audio_io import read_wav
from core.constants import (
    HESUVI_TRACK_ORDER,
    HEXADECAGONAL_TRACK_ORDER,
    SPEAKER_NAMES,
)

_HRIR_FILE_NAME = "hrir.wav"
_HESUVI_FILE_NAME = "hesuvi.wav"
_HANGLOOSE_DIR_NAME = "Hangloose"
_LFE_TRACKS = ("LFE-left", "LFE-right")
_SPEAKER_TRACKS = tuple(
    f"{speaker}-{side}"
    for speaker in SPEAKER_NAMES
    for side in ("left", "right")
)


class BrirRecoveryError(ValueError):
    """A user-correctable problem with a recovery source or destination."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        details: dict[str, object] | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.details = details or {}


@dataclass(frozen=True)
class BrirRecoveryResult:
    """Summary returned after a validated recovery operation."""

    source_kind: str
    source_path: str
    output_dir: str
    sample_rate: int
    sample_count: int
    speakers: tuple[str, ...]
    created_files: tuple[str, ...]
    existing_files: tuple[str, ...]


@dataclass(frozen=True)
class _TrackSet:
    tracks: dict[str, np.ndarray]
    sample_rate: int
    sample_count: int
    speakers: tuple[str, ...]


def recover_brir_outputs(
    directory: str | os.PathLike[str],
    *,
    include_hangloose: bool = False,
) -> BrirRecoveryResult:
    """Rebuild missing Impulcifer outputs from a surviving output format.

    ``directory`` may be the original Impulcifer output directory, its
    ``Hangloose`` subdirectory, or a directory containing the split speaker
    files directly.  Existing files are validated and preserved; this
    function only creates missing files.
    """
    selected = _validate_directory(directory)
    output_dir, split_dir = _locate_output_and_split_dirs(selected)
    hrir_path = _find_named_file(output_dir, _HRIR_FILE_NAME)
    hesuvi_path = _find_named_file(output_dir, _HESUVI_FILE_NAME)

    existing_files: list[Path] = []
    source_kind: str
    source_path: Path

    if hrir_path is not None and hesuvi_path is not None:
        hrir_tracks = _read_combined(hrir_path, HEXADECAGONAL_TRACK_ORDER, "hrir")
        hesuvi_tracks = _read_combined(hesuvi_path, HESUVI_TRACK_ORDER, "hesuvi")
        _verify_combined_pair(hrir_tracks, hesuvi_tracks, hrir_path, hesuvi_path)
        track_set = hrir_tracks
        source_kind = "hrir+hesuvi"
        source_path = output_dir
        existing_files.extend((hrir_path, hesuvi_path))
    elif hrir_path is not None:
        track_set = _read_combined(hrir_path, HEXADECAGONAL_TRACK_ORDER, "hrir")
        source_kind = "hrir"
        source_path = hrir_path
        existing_files.append(hrir_path)
    elif hesuvi_path is not None:
        track_set = _read_combined(hesuvi_path, HESUVI_TRACK_ORDER, "hesuvi")
        source_kind = "hesuvi"
        source_path = hesuvi_path
        existing_files.append(hesuvi_path)
    elif split_dir is not None:
        track_set, split_paths = _read_split_files(split_dir)
        source_kind = "hangloose"
        source_path = split_dir
        existing_files.extend(split_paths)
    else:
        raise BrirRecoveryError(
            "NO_RECOVERY_SOURCE",
            "No hrir.wav, hesuvi.wav, or Hangloose speaker WAV files were found.",
            details={"directory": str(selected)},
        )

    write_plan: list[tuple[Path, np.ndarray]] = []
    if hrir_path is None:
        hrir_target = output_dir / _HRIR_FILE_NAME
        write_plan.append(
            (hrir_target, _stack_tracks(track_set, HEXADECAGONAL_TRACK_ORDER))
        )
    if hesuvi_path is None:
        hesuvi_target = output_dir / _HESUVI_FILE_NAME
        write_plan.append(
            (hesuvi_target, _stack_tracks(track_set, HESUVI_TRACK_ORDER))
        )

    if include_hangloose and source_kind != "hangloose":
        hangloose_dir = split_dir or output_dir / _HANGLOOSE_DIR_NAME
        existing_split = _find_split_files(hangloose_dir)
        if existing_split:
            split_tracks, split_paths = _read_split_files(hangloose_dir)
            _verify_split_subset(track_set, split_tracks, split_paths)
            existing_files.extend(split_paths)
        for speaker in track_set.speakers:
            if speaker in existing_split:
                continue
            write_plan.append(
                (
                    hangloose_dir / f"{speaker}.wav",
                    np.vstack(
                        (
                            track_set.tracks[f"{speaker}-left"],
                            track_set.tracks[f"{speaker}-right"],
                        )
                    ),
                )
            )

    created_files = _write_all(write_plan, track_set.sample_rate)
    return BrirRecoveryResult(
        source_kind=source_kind,
        source_path=str(source_path),
        output_dir=str(output_dir),
        sample_rate=track_set.sample_rate,
        sample_count=track_set.sample_count,
        speakers=track_set.speakers,
        created_files=tuple(str(path) for path in created_files),
        existing_files=tuple(str(path) for path in _unique_paths(existing_files)),
    )


def _validate_directory(directory: str | os.PathLike[str]) -> Path:
    try:
        path = Path(directory).expanduser().resolve()
    except (OSError, TypeError, ValueError) as exc:
        raise BrirRecoveryError(
            "INVALID_DIRECTORY",
            "The selected recovery directory is invalid.",
        ) from exc
    if not path.is_dir():
        raise BrirRecoveryError(
            "INVALID_DIRECTORY",
            "The selected recovery directory does not exist.",
            details={"directory": str(path)},
        )
    return path


def _locate_output_and_split_dirs(selected: Path) -> tuple[Path, Path | None]:
    if _has_combined_file(selected):
        return selected, _find_split_dir(selected)

    selected_splits = _find_split_files(selected)
    if selected.name.casefold() == _HANGLOOSE_DIR_NAME.casefold() and selected_splits:
        return selected.parent, selected

    nested_split_dir = _find_named_dir(selected, _HANGLOOSE_DIR_NAME)
    if nested_split_dir is not None and _find_split_files(nested_split_dir):
        return selected, nested_split_dir
    if selected_splits:
        return selected, selected
    return selected, None


def _has_combined_file(directory: Path) -> bool:
    return (
        _find_named_file(directory, _HRIR_FILE_NAME) is not None
        or _find_named_file(directory, _HESUVI_FILE_NAME) is not None
    )


def _find_split_dir(output_dir: Path) -> Path | None:
    nested = _find_named_dir(output_dir, _HANGLOOSE_DIR_NAME)
    if nested is not None and _find_split_files(nested):
        return nested
    if _find_split_files(output_dir):
        return output_dir
    return None


def _find_named_file(directory: Path, name: str) -> Path | None:
    if not directory.is_dir():
        return None
    matches = [
        child
        for child in directory.iterdir()
        if child.is_file() and child.name.casefold() == name.casefold()
    ]
    if len(matches) > 1:
        raise BrirRecoveryError(
            "AMBIGUOUS_SOURCE",
            f"Multiple files match {name}.",
            details={"files": [str(path) for path in matches]},
        )
    return matches[0] if matches else None


def _find_named_dir(directory: Path, name: str) -> Path | None:
    if not directory.is_dir():
        return None
    matches = [
        child
        for child in directory.iterdir()
        if child.is_dir() and child.name.casefold() == name.casefold()
    ]
    if len(matches) > 1:
        raise BrirRecoveryError(
            "AMBIGUOUS_SOURCE",
            f"Multiple directories match {name}.",
            details={"directories": [str(path) for path in matches]},
        )
    return matches[0] if matches else None


def _find_split_files(directory: Path) -> dict[str, Path]:
    if not directory.is_dir():
        return {}
    canonical = {speaker.casefold(): speaker for speaker in SPEAKER_NAMES}
    found: dict[str, Path] = {}
    for path in directory.iterdir():
        if not path.is_file() or path.suffix.casefold() != ".wav":
            continue
        speaker = canonical.get(path.stem.casefold())
        if speaker is None:
            continue
        if speaker in found:
            raise BrirRecoveryError(
                "AMBIGUOUS_SOURCE",
                f"Multiple Hangloose files match speaker {speaker}.",
                details={"files": [str(found[speaker]), str(path)]},
            )
        found[speaker] = path
    return found


def _read_combined(path: Path, order: Iterable[str], kind: str) -> _TrackSet:
    order = tuple(order)
    sample_rate, data = _read_audio_matrix(path)
    if data.shape[0] != len(order):
        raise BrirRecoveryError(
            "INVALID_CHANNEL_COUNT",
            f"{path.name} must contain exactly {len(order)} channels.",
            details={
                "path": str(path),
                "expected": len(order),
                "actual": int(data.shape[0]),
            },
        )
    tracks = {name: data[index] for index, name in enumerate(order)}
    if kind == "hrir":
        non_silent_lfe = [name for name in _LFE_TRACKS if np.any(tracks[name] != 0.0)]
        if non_silent_lfe:
            raise BrirRecoveryError(
                "NON_SILENT_LFE",
                "hrir.wav contains non-silent LFE tracks that cannot be represented in hesuvi.wav.",
                details={"path": str(path), "tracks": non_silent_lfe},
            )
    speakers = _active_speakers(tracks)
    return _TrackSet(
        tracks={name: tracks[name] for name in _SPEAKER_TRACKS},
        sample_rate=sample_rate,
        sample_count=int(data.shape[1]),
        speakers=speakers,
    )


def _read_split_files(directory: Path) -> tuple[_TrackSet, tuple[Path, ...]]:
    split_files = _find_split_files(directory)
    if not split_files:
        raise BrirRecoveryError(
            "NO_RECOVERY_SOURCE",
            "No Hangloose speaker WAV files were found.",
            details={"directory": str(directory)},
        )

    tracks: dict[str, np.ndarray] = {}
    sample_rate: int | None = None
    sample_count: int | None = None
    ordered_paths: list[Path] = []
    ordered_speakers = tuple(speaker for speaker in SPEAKER_NAMES if speaker in split_files)
    for speaker in ordered_speakers:
        path = split_files[speaker]
        file_rate, data = _read_audio_matrix(path)
        if data.shape[0] != 2:
            raise BrirRecoveryError(
                "INVALID_CHANNEL_COUNT",
                f"Hangloose file {path.name} must be stereo.",
                details={"path": str(path), "actual": int(data.shape[0])},
            )
        if sample_rate is None:
            sample_rate = file_rate
            sample_count = int(data.shape[1])
        elif file_rate != sample_rate:
            raise BrirRecoveryError(
                "SAMPLE_RATE_MISMATCH",
                "Hangloose files must use the same sample rate.",
                details={
                    "path": str(path),
                    "expected": sample_rate,
                    "actual": file_rate,
                },
            )
        elif data.shape[1] != sample_count:
            raise BrirRecoveryError(
                "SAMPLE_COUNT_MISMATCH",
                "Hangloose files must contain the same number of samples.",
                details={
                    "path": str(path),
                    "expected": sample_count,
                    "actual": int(data.shape[1]),
                },
            )
        tracks[f"{speaker}-left"] = data[0]
        tracks[f"{speaker}-right"] = data[1]
        ordered_paths.append(path)

    assert sample_rate is not None and sample_count is not None
    silence = np.zeros(sample_count, dtype=np.float64)
    for track in _SPEAKER_TRACKS:
        tracks.setdefault(track, silence)
    return (
        _TrackSet(
            tracks=tracks,
            sample_rate=sample_rate,
            sample_count=sample_count,
            speakers=ordered_speakers,
        ),
        tuple(ordered_paths),
    )


def _read_audio_matrix(path: Path) -> tuple[int, np.ndarray]:
    try:
        sample_rate, data = read_wav(str(path), expand=True)
    except Exception as exc:
        raise BrirRecoveryError(
            "INVALID_WAV",
            f"Could not read {path.name} as a WAV file.",
            details={"path": str(path), "reason": str(exc)},
        ) from exc
    matrix = np.asarray(data)
    if matrix.ndim != 2 or matrix.shape[1] == 0:
        raise BrirRecoveryError(
            "INVALID_WAV",
            f"{path.name} contains no usable audio samples.",
            details={"path": str(path), "shape": list(matrix.shape)},
        )
    if not np.all(np.isfinite(matrix)):
        raise BrirRecoveryError(
            "INVALID_WAV",
            f"{path.name} contains NaN or infinite samples.",
            details={"path": str(path)},
        )
    return int(sample_rate), matrix


def _active_speakers(tracks: dict[str, np.ndarray]) -> tuple[str, ...]:
    return tuple(
        speaker
        for speaker in SPEAKER_NAMES
        if np.any(tracks[f"{speaker}-left"] != 0.0)
        or np.any(tracks[f"{speaker}-right"] != 0.0)
    )


def _verify_combined_pair(
    hrir: _TrackSet,
    hesuvi: _TrackSet,
    hrir_path: Path,
    hesuvi_path: Path,
) -> None:
    if hrir.sample_rate != hesuvi.sample_rate:
        raise BrirRecoveryError(
            "SAMPLE_RATE_MISMATCH",
            "hrir.wav and hesuvi.wav use different sample rates.",
            details={
                "hrir": hrir.sample_rate,
                "hesuvi": hesuvi.sample_rate,
            },
        )
    if hrir.sample_count != hesuvi.sample_count:
        raise BrirRecoveryError(
            "SAMPLE_COUNT_MISMATCH",
            "hrir.wav and hesuvi.wav contain different sample counts.",
            details={
                "hrir": hrir.sample_count,
                "hesuvi": hesuvi.sample_count,
            },
        )
    mismatched = [
        track
        for track in _SPEAKER_TRACKS
        if not np.array_equal(hrir.tracks[track], hesuvi.tracks[track])
    ]
    if mismatched:
        raise BrirRecoveryError(
            "SOURCE_MISMATCH",
            "hrir.wav and hesuvi.wav do not contain the same impulse responses.",
            details={
                "hrir_path": str(hrir_path),
                "hesuvi_path": str(hesuvi_path),
                "tracks": mismatched,
            },
        )


def _verify_split_subset(
    combined: _TrackSet,
    split: _TrackSet,
    split_paths: tuple[Path, ...],
) -> None:
    if combined.sample_rate != split.sample_rate:
        raise BrirRecoveryError(
            "SAMPLE_RATE_MISMATCH",
            "Existing Hangloose files use a different sample rate.",
            details={"files": [str(path) for path in split_paths]},
        )
    if combined.sample_count != split.sample_count:
        raise BrirRecoveryError(
            "SAMPLE_COUNT_MISMATCH",
            "Existing Hangloose files use a different sample count.",
            details={"files": [str(path) for path in split_paths]},
        )
    mismatched = [
        track
        for speaker in split.speakers
        for track in (f"{speaker}-left", f"{speaker}-right")
        if not np.array_equal(combined.tracks[track], split.tracks[track])
    ]
    if mismatched:
        raise BrirRecoveryError(
            "SOURCE_MISMATCH",
            "Existing Hangloose files do not match the combined BRIR source.",
            details={"files": [str(path) for path in split_paths], "tracks": mismatched},
        )


def _stack_tracks(track_set: _TrackSet, order: Iterable[str]) -> np.ndarray:
    silence = np.zeros(track_set.sample_count, dtype=np.float64)
    return np.vstack([track_set.tracks.get(track, silence) for track in order])


def _write_all(
    write_plan: list[tuple[Path, np.ndarray]],
    sample_rate: int,
) -> tuple[Path, ...]:
    if not write_plan:
        return ()

    temporary: list[tuple[Path, Path]] = []
    created: list[Path] = []
    try:
        for target, data in write_plan:
            target.parent.mkdir(parents=True, exist_ok=True)
            if _find_named_file(target.parent, target.name) is not None:
                raise BrirRecoveryError(
                    "OUTPUT_CONFLICT",
                    f"Refusing to overwrite existing output {target.name}.",
                    details={"path": str(target)},
                )
            with tempfile.NamedTemporaryFile(
                prefix=f".{target.stem}-",
                suffix=".wav",
                dir=target.parent,
                delete=False,
            ) as handle:
                temp_path = Path(handle.name)
            try:
                sf.write(
                    str(temp_path),
                    np.asarray(data).T,
                    samplerate=sample_rate,
                    subtype="PCM_32",
                )
            except Exception as exc:
                raise BrirRecoveryError(
                    "OUTPUT_WRITE_FAILED",
                    f"Could not write recovered output {target.name}.",
                    details={"path": str(target), "reason": str(exc)},
                ) from exc
            temporary.append((temp_path, target))

        for temp_path, target in temporary:
            if _find_named_file(target.parent, target.name) is not None:
                raise BrirRecoveryError(
                    "OUTPUT_CONFLICT",
                    f"Refusing to overwrite existing output {target.name}.",
                    details={"path": str(target)},
                )
            os.replace(temp_path, target)
            created.append(target)
        return tuple(created)
    except Exception:
        for temp_path, _target in temporary:
            try:
                temp_path.unlink(missing_ok=True)
            except OSError:
                pass
        for target in created:
            try:
                target.unlink(missing_ok=True)
            except OSError:
                pass
        raise


def _unique_paths(paths: Iterable[Path]) -> tuple[Path, ...]:
    seen: set[str] = set()
    unique: list[Path] = []
    for path in paths:
        key = os.path.normcase(str(path.resolve()))
        if key in seen:
            continue
        seen.add(key)
        unique.append(path)
    return tuple(unique)
