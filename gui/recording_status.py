#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Recording status helpers shared by Stable and Studio recorder tabs."""

from __future__ import annotations

import math
import os
import time
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

import numpy as np
import soundfile as sf

from core.utils import read_wav


ACTIVE_CHANNEL_THRESHOLD = 1e-6
_SUMMARY_NOT_PROVIDED = object()


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


class RecordingStatusController:
    """Drive recording progress text from sweep duration and completion summary."""

    def __init__(
        self,
        *,
        root: Any,
        loc: Any,
        set_status: Callable[[str], None],
        set_detail: Callable[[str], None],
        set_progress: Callable[[float], None],
    ) -> None:
        self.root = root
        self.loc = loc
        self.set_status = set_status
        self.set_detail = set_detail
        self.set_progress = set_progress

        self._after_id: str | None = None
        self._started_at = 0.0
        self._duration: float | None = None
        self._active = False

    def reset(self) -> None:
        """Show the idle state."""
        self._cancel_timer()
        self._active = False
        self._safe_set_progress(0.0)
        self._safe_set_status(self.loc.get("recording_status_ready"))
        self._safe_set_detail("")

    def start(self, play_file: str) -> None:
        """Start estimated progress for an in-flight recording."""
        self._cancel_timer()
        playback_info = inspect_playback_file(play_file)
        self._duration = playback_info.duration if playback_info is not None else None
        self._started_at = time.monotonic()
        self._active = True

        self._safe_set_progress(0.0)
        self._safe_set_status(self.loc.get("recording_status_preparing"))
        self._safe_set_detail("")
        self._schedule_tick(delay_ms=300)

    def complete(
        self,
        record_file: str,
        summary: RecordingSummary | None | object = _SUMMARY_NOT_PROVIDED,
    ) -> str:
        """Stop progress and show a summary for the saved recording."""
        self._cancel_timer()
        self._active = False
        self._safe_set_progress(1.0)
        self._safe_set_status(self.loc.get("recording_status_complete"))

        detail = self.summary_text(record_file, summary)
        self._safe_set_detail(detail)
        return detail

    def summary_text(
        self,
        record_file: str,
        summary: RecordingSummary | None | object = _SUMMARY_NOT_PROVIDED,
    ) -> str:
        """Return localized summary text for a completed recording."""
        recording_summary = (
            analyze_recording(record_file)
            if summary is _SUMMARY_NOT_PROVIDED
            else summary
        )

        basename = os.path.basename(record_file)
        if recording_summary is None:
            return self.loc.get("recording_status_summary_unavailable", file=basename)

        return self.loc.get(
            "recording_status_summary",
            file=basename,
            channels=recording_summary.channels,
            duration=format_duration(recording_summary.duration),
            peak_db=f"{recording_summary.peak_db:.1f}",
            active=recording_summary.active_channels,
            total=recording_summary.channels,
        )

    def error(self, error_msg: str) -> None:
        """Stop progress and leave the error visible in the status area."""
        self._cancel_timer()
        self._active = False
        self._safe_set_progress(0.0)
        self._safe_set_status(self.loc.get("recording_status_error"))
        self._safe_set_detail(error_msg)

    def handle_event(self, event: Any) -> None:
        """Apply a core recorder progress event to the status widgets."""
        self._cancel_timer()
        self._active = event.phase == "recording"
        self._safe_set_progress(event.progress)

        if event.phase == "recording" and event.speaker:
            self._safe_set_status(
                self.loc.get(
                    "recording_status_recording_speaker",
                    speaker=event.speaker,
                    index=event.segment_index or 0,
                    total=event.segment_total,
                )
            )
            self._safe_set_detail(
                self.loc.get(
                    "recording_status_recording",
                    elapsed=format_duration(event.elapsed),
                    duration=format_duration(event.duration),
                )
            )
        elif event.phase == "recording":
            self._safe_set_status(self.loc.get("recording_status_recording_gap"))
            self._safe_set_detail(
                self.loc.get(
                    "recording_status_recording",
                    elapsed=format_duration(event.elapsed),
                    duration=format_duration(event.duration),
                )
            )
        elif event.phase == "devices":
            self._safe_set_status(self.loc.get("recording_status_devices_ready"))
            self._safe_set_detail(event.message)
        elif event.phase == "saving":
            self._safe_set_status(self.loc.get("recording_status_saving"))
            self._safe_set_detail("")
        elif event.phase == "complete":
            self._safe_set_status(self.loc.get("recording_status_complete"))
            self._safe_set_detail("")
        elif event.phase == "error":
            self.error(event.message)
        else:
            self._safe_set_status(self.loc.get("recording_status_preparing"))
            self._safe_set_detail(event.message)

    def _schedule_tick(self, *, delay_ms: int = 500) -> None:
        try:
            self._after_id = self.root.after(delay_ms, self._tick)
        except Exception:
            self._after_id = None

    def _tick(self) -> None:
        self._after_id = None
        if not self._active:
            return

        elapsed = max(0.0, time.monotonic() - self._started_at)
        if self._duration is not None and self._duration > 0:
            if elapsed <= self._duration:
                progress = min(0.98, elapsed / self._duration)
                self._safe_set_progress(progress)
                self._safe_set_status(
                    self.loc.get(
                        "recording_status_recording",
                        elapsed=format_duration(elapsed),
                        duration=format_duration(self._duration),
                    )
                )
            else:
                self._safe_set_progress(0.98)
                self._safe_set_status(self.loc.get("recording_status_saving"))
        else:
            self._safe_set_progress(0.15)
            self._safe_set_status(
                self.loc.get(
                    "recording_status_recording_unknown",
                    elapsed=format_duration(elapsed),
                )
            )

        self._schedule_tick()

    def _cancel_timer(self) -> None:
        if self._after_id is None:
            return
        try:
            self.root.after_cancel(self._after_id)
        except Exception:
            pass
        self._after_id = None

    def _safe_set_status(self, text: str) -> None:
        try:
            self.set_status(text)
        except Exception:
            pass

    def _safe_set_detail(self, text: str) -> None:
        try:
            self.set_detail(text)
        except Exception:
            pass

    def _safe_set_progress(self, value: float) -> None:
        try:
            self.set_progress(max(0.0, min(1.0, value)))
        except Exception:
            pass
