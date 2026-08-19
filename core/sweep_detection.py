#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Estimate which sweep an Impulcifer recording folder was captured with.

Impulcifer's exponential sine sweep lengths form a discrete grid: for a
given sampling rate the generator produces ``N = round(M · L1)`` samples
where ``L1 = 2 · ln(2^P) · 2^P`` and ``P`` is the octave count derived from
``fs`` (~3.08 s steps at 48 kHz; the bundled 6.15 s sweep is ``M = 2``).
Recovering the original sweep therefore only requires a rough estimate of
the sweep length — rounding to the nearest grid point ``M`` reconstructs
the exact test signal.

Two estimates are tried per recording, most robust first:

1. **File length**: ``sweep_sequence`` output is exactly
   ``2 s + (sweep + 2 s) × n_speakers`` long and ``play_and_record``
   records exactly that many samples, so unmodified captures yield the
   sweep length to sub-sample precision from the WAV header alone.
2. **Envelope onsets**: for trimmed/edited files, sweep onsets are found
   from the amplitude envelope; consecutive onsets are ``sweep + 2 s``
   apart independent of the room's decay tail.

Off-grid signals (e.g. REW's own sweep exports) land far from any grid
point and come back with ``confidence == "low"`` — callers should fall
back to the bundled default and let the user pick the actual file.
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass

import numpy as np
import soundfile as sf

from core.constants import DEFAULT_SWEEP_FS, SPEAKER_LIST_PATTERN

CONFIDENCE_HIGH = "high"
CONFIDENCE_LOW = "low"

# Silence used by ``sweep_sequence``: 2 s lead + 2 s after each sweep.
_SILENCE_SECONDS = 2.0

# Relative distance (in grid units) below which a snapped estimate is
# trusted. Half a unit (~1.5 s at 48 kHz) would be ambiguous; 15% keeps a
# wide margin over measurement noise while rejecting off-grid signals.
_HIGH_CONFIDENCE_DEVIATION = 0.15

_RECORDING_PATTERN = re.compile(rf"^{SPEAKER_LIST_PATTERN}\.wav$")


def sweep_octaves(fs: int) -> int:
    """Octave count P used by ``ImpulseResponseEstimator`` for ``fs``."""
    return int(np.ceil(np.log2((fs / 2) / 5)))


def sweep_grid_unit(fs: int) -> float:
    """Sweep length in samples for M=1 (the grid unit) at ``fs``."""
    p = sweep_octaves(fs)
    return float(2 * np.log(2**p) * 2**p)


def snap_sweep_samples(estimate_samples: float, fs: int) -> tuple[int, int, float]:
    """Snap a sweep-length estimate to the generator grid.

    Returns:
        - M (grid multiple, >= 1)
        - N (exact generated sweep length in samples for that M)
        - relative deviation of the estimate from the grid point, in units
    """
    unit = sweep_grid_unit(fs)
    m = max(1, int(round(estimate_samples / unit)))
    n = int(round(m * unit))
    deviation = abs(estimate_samples - m * unit) / unit
    return m, n, deviation


@dataclass(frozen=True)
class SweepDetectionResult:
    """Recovered sweep parameters for a recording folder."""

    fs: int
    m: int
    sweep_samples: int
    duration_seconds: float
    n_segments: int
    speakers: tuple
    confidence: str
    deviation: float
    source_files: tuple

    @property
    def is_default(self) -> bool:
        """Whether this is the bundled default sweep (48 kHz, M=2 → 6.15 s)."""
        return self.fs == DEFAULT_SWEEP_FS and self.m == 2

    def to_estimator(self):
        """Reconstruct the exact estimator, ``from_wav``-style ((N-1)/fs)."""
        from core.impulse_response_estimator import ImpulseResponseEstimator

        return ImpulseResponseEstimator(
            min_duration=(self.sweep_samples - 1) / self.fs, fs=self.fs
        )

    def generate_spec(self) -> str:
        """Spec string accepted by ``open_impulse_response_estimator``."""
        return f"generate:{self.duration_seconds:.2f}s@{self.fs}"


def list_recording_files(dir_path: str) -> list:
    """Speaker-list recordings (``FL,FR.wav`` …) in ``dir_path``."""
    if not os.path.isdir(dir_path):
        return []
    return sorted(
        file_name
        for file_name in os.listdir(dir_path)
        if _RECORDING_PATTERN.match(file_name)
    )


def _estimate_from_length(frames: int, fs: int, n_speakers: int) -> float:
    """Sweep-length estimate assuming an unmodified full-sequence capture."""
    silence = _SILENCE_SECONDS * fs
    return (frames - silence * (n_speakers + 1)) / n_speakers


def _estimate_from_envelope(file_path: str, fs: int) -> tuple:
    """Sweep-length estimate from amplitude-envelope onsets.

    Returns ``(estimate_samples, n_segments)`` or ``(None, 0)`` when no
    active region is found.
    """
    data, _ = sf.read(file_path, dtype="float64", always_2d=True)
    envelope = np.max(np.abs(data), axis=1)
    # ~50 ms moving average smoothing
    kernel = max(1, int(0.05 * fs))
    envelope = np.convolve(envelope, np.ones(kernel) / kernel, mode="same")

    peak = float(np.max(envelope))
    if peak <= 0:
        return None, 0
    active = envelope > peak * 0.01  # -40 dB

    # Merge sub-300 ms gaps so a single sweep never splits in two.
    edges = np.flatnonzero(np.diff(active.astype(np.int8)))
    runs = []
    start = 0 if active[0] else None
    for edge in edges:
        if active[edge]:  # falling edge
            runs.append((start, edge + 1))
            start = None
        else:  # rising edge
            start = edge + 1
    if start is not None:
        runs.append((start, len(active)))
    merged = []
    for run in runs:
        if merged and run[0] - merged[-1][1] < 0.3 * fs:
            merged[-1] = (merged[-1][0], run[1])
        else:
            merged.append(run)
    # Ignore blips shorter than half a second.
    segments = [run for run in merged if run[1] - run[0] >= 0.5 * fs]
    if not segments:
        return None, 0

    if len(segments) >= 2:
        # Consecutive onsets are exactly (sweep + 2 s) apart — immune to
        # each sweep's decay tail bleeding into the following silence.
        onsets = np.array([run[0] for run in segments], dtype=np.float64)
        interval = float(np.median(np.diff(onsets)))
        return interval - _SILENCE_SECONDS * fs, len(segments)

    # Single sweep: fall back to the active-region length (overestimates
    # by the room decay, which grid snapping usually absorbs).
    run = segments[0]
    return float(run[1] - run[0]), 1


def detect_sweep_parameters(dir_path: str):
    """Detect the sweep parameters used for the recordings in ``dir_path``.

    Returns a :class:`SweepDetectionResult` or ``None`` when the folder has
    no speaker-list recordings. Disagreement between recordings (mixed fs
    or mixed grid points) degrades confidence to ``"low"``.
    """
    file_names = list_recording_files(dir_path)
    if not file_names:
        return None

    per_file = []
    speakers = []
    for file_name in file_names:
        file_path = os.path.join(dir_path, file_name)
        file_speakers = file_name[: -len(".wav")].split(",")
        speakers.extend(file_speakers)
        try:
            info = sf.info(file_path)
        except Exception:
            continue
        fs = int(info.samplerate)

        estimate = _estimate_from_length(info.frames, fs, len(file_speakers))
        n_segments = len(file_speakers)
        if estimate > 0:
            m, n, deviation = snap_sweep_samples(estimate, fs)
        else:
            deviation = None
        if estimate <= 0 or deviation > _HIGH_CONFIDENCE_DEVIATION:
            envelope_estimate, envelope_segments = _estimate_from_envelope(file_path, fs)
            if envelope_estimate is not None:
                m, n, deviation = snap_sweep_samples(envelope_estimate, fs)
                n_segments = envelope_segments
            elif estimate <= 0:
                continue
        per_file.append((fs, m, n, deviation, n_segments))

    if not per_file:
        return None

    fs_values = {entry[0] for entry in per_file}
    m_values = {entry[1] for entry in per_file}
    fs, m, n, deviation, _ = per_file[0]
    n_segments = sum(entry[4] for entry in per_file)
    consistent = len(fs_values) == 1 and len(m_values) == 1
    worst_deviation = max(entry[3] for entry in per_file)
    confidence = (
        CONFIDENCE_HIGH
        if consistent and worst_deviation <= _HIGH_CONFIDENCE_DEVIATION
        else CONFIDENCE_LOW
    )

    return SweepDetectionResult(
        fs=fs,
        m=m,
        sweep_samples=n,
        duration_seconds=n / fs,
        n_segments=n_segments,
        speakers=tuple(speakers),
        confidence=confidence,
        deviation=worst_deviation,
        source_files=tuple(file_names),
    )
