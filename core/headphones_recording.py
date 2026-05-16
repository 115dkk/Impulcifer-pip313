#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Validation helpers for the headphone-compensation record path.

Headphone compensation requires playing a sweep through the headphone L
and R drivers — anything beyond stereo doesn't fit a normal headphone
output, and a true mono playback (same sweep on both drivers) only ever
yields a generic L=R EQ rather than per-driver response. This module
centralizes the gating decision so Stable / Studio / future CLI all
agree on what "valid headphones playback file" means.
"""

from __future__ import annotations

import os
from dataclasses import dataclass

import soundfile as sf


MAX_HEADPHONES_PLAYBACK_CHANNELS = 2


@dataclass(frozen=True)
class HeadphonesPlaybackInfo:
    """Result of inspecting a play file for the headphone record path."""

    is_valid: bool
    channels: int
    reason_key: str  # i18n key — empty when ``is_valid`` is True.
    is_mono: bool


def inspect_headphones_playback(play_path: str) -> HeadphonesPlaybackInfo:
    """Inspect ``play_path`` and decide whether it's safe for headphones.

    Returns a structured result so the GUI can pick between:

    * informational (mono playback warning),
    * fatal rejection (multi-channel surround sweep, missing file, etc.).

    The function never raises for "file is invalid" — those cases come
    back via ``reason_key`` so the GUI can localize the error.
    """
    if not play_path:
        return HeadphonesPlaybackInfo(False, 0, "error_headphones_play_file_missing", False)

    if not os.path.exists(play_path):
        return HeadphonesPlaybackInfo(False, 0, "error_headphones_play_file_missing", False)

    try:
        info = sf.info(play_path)
    except Exception:
        # Anything that ``soundfile`` can't open (TrueHD/MLP, corrupt
        # WAV, unsupported format) is also rejected.
        return HeadphonesPlaybackInfo(False, 0, "error_headphones_play_file_unreadable", False)

    channels = int(info.channels or 0)
    if channels <= 0:
        return HeadphonesPlaybackInfo(False, channels, "error_headphones_play_file_unreadable", False)

    if channels > MAX_HEADPHONES_PLAYBACK_CHANNELS:
        return HeadphonesPlaybackInfo(False, channels, "error_headphones_play_file_too_many_channels", False)

    return HeadphonesPlaybackInfo(True, channels, "", channels == 1)
