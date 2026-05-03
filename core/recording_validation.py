# -*- coding: utf-8 -*-
"""Pure recording setup validation helpers."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from typing import Optional

from core.constants import SPEAKER_LIST_PATTERN


@dataclass(frozen=True)
class ChannelValidationResult:
    """Result of filename-based recording channel validation."""

    has_mismatch: bool
    expected_speakers: list[str]
    expected_channels: int
    selected_channels: int


def validate_recording_setup(
    record_filename: str,
    selected_channels: int,
    force_channels: bool,
) -> Optional[ChannelValidationResult]:
    """Validate a recording filename against the selected input channel count.

    Args:
        record_filename: Recording output filename or path.
        selected_channels: Input channel count selected by the user.
        force_channels: Whether explicit channel count validation is enabled.

    Returns:
        ``None`` when the filename does not contain a speaker list. Otherwise,
        a validation result with ``has_mismatch`` set when forced channels do
        not match the speaker-list stereo pair count.
    """
    filename = os.path.basename(record_filename)
    match = re.search(SPEAKER_LIST_PATTERN, filename)
    if not match:
        return None

    expected_speakers = match.group(1).split(',')
    expected_channels = len(expected_speakers) * 2
    return ChannelValidationResult(
        has_mismatch=force_channels and selected_channels != expected_channels,
        expected_speakers=expected_speakers,
        expected_channels=expected_channels,
        selected_channels=selected_channels,
    )
