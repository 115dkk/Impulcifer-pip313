#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Tk-independent recording metadata and summary helpers."""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
import soundfile as sf

from core.audio_io import read_wav


ACTIVE_CHANNEL_THRESHOLD = 1e-6


@dataclass(frozen=True)
class PlaybackInfo:
    """Small metadata snapshot for the file that will be played."""

    sample_rate: int
    channels: int
    frames: int

    @property
    def duration(self) -> float:
        """Return playback duration in seconds."""
        if self.sample_rate <= 0:
            return 0.0
        return self.frames / self.sample_rate


@dataclass(frozen=True)
class RecordingSummary:
    """Post-recording summary derived from the written WAV file."""

    sample_rate: int
    channels: int
    duration: float
    peak_db: float
    active_channels: int


def inspect_playback_file(file_path: str) -> PlaybackInfo | None:
    """Return playback duration/channel metadata without loading samples."""
    try:
        info = sf.info(file_path)
    except Exception:
        return None

    return PlaybackInfo(
        sample_rate=int(info.samplerate or 0),
        channels=int(info.channels or 0),
        frames=int(info.frames or 0),
    )


def analyze_recording(file_path: str) -> RecordingSummary | None:
    """Read a completed recording and calculate a compact confidence summary."""
    try:
        sample_rate, data = read_wav(file_path, expand=True)
    except Exception:
        return None

    if data.size == 0 or sample_rate <= 0:
        return None

    if data.ndim == 1:
        data = np.expand_dims(data, axis=0)

    channels = int(data.shape[0])
    samples = int(data.shape[1]) if data.ndim > 1 else int(data.shape[0])
    duration = samples / sample_rate

    abs_data = np.abs(data)
    peak = float(np.max(abs_data))
    peak_db = 20 * math.log10(max(peak, 1e-10))

    if data.ndim > 1:
        channel_peaks = np.max(abs_data, axis=1)
        active_channels = int(np.count_nonzero(channel_peaks > ACTIVE_CHANNEL_THRESHOLD))
    else:
        active_channels = int(peak > ACTIVE_CHANNEL_THRESHOLD)

    return RecordingSummary(
        sample_rate=int(sample_rate),
        channels=channels,
        duration=duration,
        peak_db=peak_db,
        active_channels=active_channels,
    )


def format_duration(seconds: float | None) -> str:
    """Format seconds as ``M:SS`` or ``H:MM:SS`` for compact status labels."""
    if seconds is None or not math.isfinite(seconds) or seconds < 0:
        return "--:--"

    total_seconds = int(round(seconds))
    hours, remainder = divmod(total_seconds, 3600)
    minutes, secs = divmod(remainder, 60)
    if hours:
        return f"{hours}:{minutes:02d}:{secs:02d}"
    return f"{minutes}:{secs:02d}"
