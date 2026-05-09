#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Progress event helpers for recorder playback/capture sessions."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from typing import Literal

from core.constants import SPEAKER_NAMES


RecordingPhase = Literal[
    "loading",
    "devices",
    "recording",
    "saving",
    "complete",
    "error",
]

_SPEAKER_TOKEN = "|".join(re.escape(name) for name in sorted(SPEAKER_NAMES, key=len, reverse=True))
_SEGMENTED_SWEEP_RE = re.compile(
    rf"sweep-seg-(?P<speakers>(?:{_SPEAKER_TOKEN})(?:,(?:{_SPEAKER_TOKEN}))*)-"
    r"(?P<tracks>[^-]+)-(?P<duration>\d+(?:\.\d+)?)s-",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class SweepSegment:
    """A single active speaker region inside a segmented sweep file."""

    speaker: str
    index: int
    total: int
    start: float
    end: float

    def contains(self, elapsed: float) -> bool:
        """Return whether ``elapsed`` seconds is inside this active sweep."""
        return self.start <= elapsed < self.end


@dataclass(frozen=True)
class RecorderProgressEvent:
    """Thread-safe recorder progress payload emitted by ``play_and_record``."""

    phase: RecordingPhase
    elapsed: float = 0.0
    duration: float = 0.0
    progress: float = 0.0
    speaker: str | None = None
    segment_index: int | None = None
    segment_total: int = 0
    segment_progress: float | None = None
    speakers: tuple[str, ...] = ()
    message: str = ""


def infer_sweep_segments(play_file: str, total_duration: float) -> tuple[SweepSegment, ...]:
    """Infer active speaker intervals from an Impulcifer segmented sweep name.

    ``ImpulseResponseEstimator.sweep_sequence`` creates a 2-second leading
    silence, then repeats ``sweep + 2 seconds silence`` for each requested
    speaker. The generated file name carries the speaker list and sweep
    duration, so we can reconstruct the user-visible timeline without touching
    the audio stream itself.
    """
    file_name = os.path.basename(play_file)
    match = _SEGMENTED_SWEEP_RE.search(file_name)
    if match is None:
        return ()

    speakers = tuple(speaker.upper() for speaker in match.group("speakers").split(","))
    if not speakers:
        return ()

    try:
        sweep_duration = float(match.group("duration"))
    except ValueError:
        sweep_duration = 0.0

    if sweep_duration <= 0.0:
        # Fallback for unexpected future file names with a valid speaker list.
        # The sequence shape is: 2s lead + N sweeps + N inter/trailing 2s gaps.
        sweep_duration = max(0.0, (total_duration - 2.0 * (len(speakers) + 1)) / len(speakers))

    segments: list[SweepSegment] = []
    for index, speaker in enumerate(speakers):
        start = 2.0 + index * (sweep_duration + 2.0)
        end = min(total_duration, start + sweep_duration)
        if end <= start:
            continue
        segments.append(
            SweepSegment(
                speaker=speaker,
                index=index + 1,
                total=len(speakers),
                start=start,
                end=end,
            )
        )

    return tuple(segments)


def event_for_elapsed(
    *,
    elapsed: float,
    duration: float,
    segments: tuple[SweepSegment, ...],
) -> RecorderProgressEvent:
    """Build a recording progress event for the current playback timestamp."""
    elapsed = max(0.0, elapsed)
    duration = max(0.0, duration)
    progress = min(0.98, elapsed / duration) if duration > 0 else 0.0
    speakers = tuple(segment.speaker for segment in segments)

    for segment in segments:
        if segment.contains(elapsed):
            segment_duration = max(0.001, segment.end - segment.start)
            segment_progress = min(1.0, max(0.0, (elapsed - segment.start) / segment_duration))
            return RecorderProgressEvent(
                phase="recording",
                elapsed=elapsed,
                duration=duration,
                progress=progress,
                speaker=segment.speaker,
                segment_index=segment.index,
                segment_total=segment.total,
                segment_progress=segment_progress,
                speakers=speakers,
            )

    return RecorderProgressEvent(
        phase="recording",
        elapsed=elapsed,
        duration=duration,
        progress=progress,
        segment_total=len(segments),
        speakers=speakers,
    )
