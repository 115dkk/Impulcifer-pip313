#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""On-the-fly sweep sequence generation for the recorder.

Impulcifer's exponential sine sweep is fully determined by ``(fs,
min_duration)`` — the bundled ``sweep-…wav`` files are just materialized
outputs of :class:`core.impulse_response_estimator.ImpulseResponseEstimator`.
This module builds the same sequences in memory so recording no longer
requires a sweep file on disk. With the default :class:`SweepSpec` the
generated float64 sequence quantizes (via sounddevice's float32 output
conversion) to exactly the samples of the bundled 32-bit
``sweep-seg-…wav`` files, so on-the-fly captures stay bit-compatible with
existing Impulcifer workflows.

Custom-parameter captures write a ``test.wav`` sidecar
(:func:`write_sidecar`) into the recording folder; the BRIR pipeline's
``open_impulse_response_estimator`` already picks that filename up
automatically, so non-default recordings stay self-describing.
"""

from __future__ import annotations

import io
import os
from dataclasses import dataclass, field

import numpy as np
import soundfile as sf

from core.constants import (  # noqa: F401 — re-exported for existing importers
    DEFAULT_SWEEP_DURATION,
    DEFAULT_SWEEP_FS,
    SPEAKER_NAMES,
    SWEEP_TRACK_LAYOUTS,
)
from core.impulse_response_estimator import SEQUENCE_TRACK_ORDERS, ImpulseResponseEstimator
from core.recording_naming import record_filename_for_speakers
from core.recording_progress import SweepSegment
from core.utils import write_wav

# Silence used by ``ImpulseResponseEstimator.sweep_sequence``: 2 s lead
# followed by (sweep + 2 s) per speaker.
SEQUENCE_SILENCE_SECONDS = 2.0

SIDECAR_FILENAME = "test.wav"
SIDECAR_BIT_DEPTH = 32


@dataclass(frozen=True)
class SweepSpec:
    """User-selectable sweep sequence parameters.

    ``duration`` carries ``ImpulseResponseEstimator``'s *minimum duration*
    semantics: the generated sweep is the shortest valid exponential sweep
    at least this long (5.0 s at 48 kHz yields the familiar 6.15 s sweep).
    """

    fs: int = DEFAULT_SWEEP_FS
    duration: float = DEFAULT_SWEEP_DURATION
    speakers: tuple = ("FL", "FR")
    tracks: str = "stereo"

    def is_default_signal(self) -> bool:
        """Whether (fs, duration) match the bundled sweep.

        Only the signal parameters matter here — the speaker order and
        track layout do not change the underlying test signal, so captures
        with default (fs, duration) need no ``test.wav`` sidecar.
        """
        return int(self.fs) == DEFAULT_SWEEP_FS and float(self.duration) == DEFAULT_SWEEP_DURATION


@dataclass(frozen=True)
class SweepPlayback:
    """A fully materialized in-memory sweep sequence ready for playback."""

    estimator: ImpulseResponseEstimator
    spec: SweepSpec
    data: np.ndarray = field(repr=False)  # (n_tracks, samples)
    segments: tuple  # tuple[SweepSegment, ...]
    record_filename: str
    display_name: str

    @property
    def fs(self) -> int:
        return self.estimator.fs

    @property
    def duration(self) -> float:
        return self.data.shape[1] / self.estimator.fs


def normalize_speakers(speakers) -> tuple:
    """Uppercase, strip and validate a speaker name sequence."""
    names = [str(speaker).strip().upper() for speaker in speakers if str(speaker).strip()]
    if not names:
        raise ValueError("At least one speaker name is required.")
    for name in names:
        if name not in SPEAKER_NAMES:
            raise ValueError(f'"{name}" is not a recognised speaker name.')
    if len(set(names)) != len(names):
        raise ValueError("Speaker names must be unique.")
    return tuple(names)


def validate_sweep_spec(spec: SweepSpec) -> SweepSpec:
    """Validate and normalize a spec without generating any audio.

    Raises ``ValueError`` with a user-presentable message on any invalid
    field, so request validators can reject bad specs before a recording
    job starts.
    """
    speakers = normalize_speakers(spec.speakers)
    if spec.tracks not in SWEEP_TRACK_LAYOUTS:
        raise ValueError(
            f'Unsupported track configuration "{spec.tracks}". '
            f'Supported: {", ".join(SWEEP_TRACK_LAYOUTS)}.'
        )
    if spec.tracks == "stereo":
        if not 1 <= len(speakers) <= 2:
            raise ValueError('"stereo" track configuration requires one or two speakers.')
    elif spec.tracks in SEQUENCE_TRACK_ORDERS:
        order = SEQUENCE_TRACK_ORDERS[spec.tracks]
        for speaker in speakers:
            if speaker not in order:
                raise ValueError(
                    f'Speaker "{speaker}" is not available in the "{spec.tracks}" layout.'
                )
    fs = int(spec.fs)
    if not 8000 <= fs <= 384000:
        raise ValueError("Sampling rate must be between 8000 and 384000 Hz.")
    duration = float(spec.duration)
    if not 0.1 <= duration <= 60.0:
        raise ValueError("Sweep duration must be between 0.1 and 60 seconds.")
    return SweepSpec(fs=fs, duration=duration, speakers=speakers, tracks=spec.tracks)


def _quantize_like_bundled_wav(data: np.ndarray, fs: int) -> np.ndarray:
    """Round-trip the sequence through an in-memory PCM_32 WAV.

    The bundled sweep files are written as 32-bit integer PCM, so playing
    them goes float64 → int32 → float. Without this round trip the
    generated float64 sequence differs from file playback by 1 float32 ULP
    (~-144 dB) — physically irrelevant, but running the exact same
    libsndfile conversion keeps on-the-fly playback bit-identical to
    playing a materialized sweep file.
    """
    buffer = io.BytesIO()
    sf.write(buffer, data.T, samplerate=fs, format="WAV", subtype="PCM_32")
    buffer.seek(0)
    quantized, _ = sf.read(buffer, dtype="float64", always_2d=True)
    return np.ascontiguousarray(quantized.T)


def build_sweep_playback(spec: SweepSpec) -> SweepPlayback:
    """Materialize the sweep sequence described by ``spec`` in memory."""
    spec = validate_sweep_spec(spec)
    speakers = spec.speakers
    fs = spec.fs
    duration = spec.duration

    estimator = ImpulseResponseEstimator(min_duration=duration, fs=fs)
    data = _quantize_like_bundled_wav(estimator.sweep_sequence(list(speakers), spec.tracks), fs)
    if spec.tracks == "mono":
        # sweep_sequence forces mono sequences onto FL.
        speakers = ("FL",)

    sweep_seconds = len(estimator) / estimator.fs
    segments = tuple(
        SweepSegment(
            speaker=speaker,
            index=index + 1,
            total=len(speakers),
            start=SEQUENCE_SILENCE_SECONDS + index * (sweep_seconds + SEQUENCE_SILENCE_SECONDS),
            end=SEQUENCE_SILENCE_SECONDS
            + index * (sweep_seconds + SEQUENCE_SILENCE_SECONDS)
            + sweep_seconds,
        )
        for index, speaker in enumerate(speakers)
    )

    display_name = (
        f'sweep-seg-{",".join(speakers)}-{spec.tracks}-'
        f'{estimator.file_name(SIDECAR_BIT_DEPTH)} (generated)'
    )
    return SweepPlayback(
        estimator=estimator,
        spec=SweepSpec(fs=fs, duration=duration, speakers=speakers, tracks=spec.tracks),
        data=data,
        segments=segments,
        record_filename=record_filename_for_speakers(speakers),
        display_name=display_name,
    )


def write_sidecar(record_dir: str, estimator: ImpulseResponseEstimator) -> str:
    """Write the mono sweep as ``test.wav`` next to the recordings.

    ``open_impulse_response_estimator`` resolves ``<dir>/test.wav`` before
    any bundled default, so custom-parameter captures processed later pick
    up the exact sweep they were recorded with — no manual configuration.
    """
    path = os.path.join(record_dir, SIDECAR_FILENAME)
    write_wav(path, estimator.fs, estimator.test_signal, bit_depth=SIDECAR_BIT_DEPTH)
    return path
