#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Derive Impulcifer-compatible recording filenames from a play file path.

Impulcifer's BRIR pipeline scans the working directory for files matching
``<speakers>.wav`` (the comma-separated speaker list, e.g. ``FL,FR.wav`` or
``FC.wav``) plus the special ``headphones.wav`` for headphone compensation.
The recorder GUI now lets the user pick a *folder* and we write the file
inside with the canonical name — this module is the single source of truth
for that name so Stable + Studio + CLI agree.

``headphones.wav`` is intentionally *not* produced by the speaker-side
record path. Playing a plain mono sweep through both headphone drivers
simultaneously cannot distinguish left vs right driver response — that
recording is meaningless for headphone compensation. The GUI exposes a
separate "Record headphones" affordance that uses
:func:`headphones_record_filename` and constrains the play file to a
mono or stereo sweep (so the L/R drivers can actually be measured one at
a time via a stereo segmented sweep, or as a generic L=R EQ via a true
mono playback the user explicitly opts into).
"""

from __future__ import annotations

import os
import re

from core.constants import SPEAKER_NAMES


_SPEAKER_TOKEN = "|".join(re.escape(name) for name in sorted(SPEAKER_NAMES, key=len, reverse=True))
_SEGMENTED_SWEEP_RE = re.compile(
    rf"sweep-seg-(?P<speakers>(?:{_SPEAKER_TOKEN})(?:,(?:{_SPEAKER_TOKEN}))*)-",
    re.IGNORECASE,
)

HEADPHONES_FILENAME = "headphones.wav"


def derive_record_filename(play_path: str) -> str:
    """Return the canonical *speaker-side* recording filename for ``play_path``.

    The play file's basename drives the choice:

    * ``sweep-seg-FL,FR-stereo-…wav`` → ``FL,FR.wav``
    * ``sweep-seg-FC-mono-…wav`` → ``FC.wav``
    * Anything else (plain mono sweep, custom user file) →
      ``<basename without ext>.wav``. We intentionally never auto-derive
      ``headphones.wav`` from a speaker-side recording — see the module
      docstring. Use :func:`headphones_record_filename` for that.

    The empty-path edge case still falls back to ``headphones.wav`` only
    so the trace-driven preview label has something to show when the
    folder is unset; callers always re-validate before writing.
    """
    if not play_path:
        return HEADPHONES_FILENAME

    basename = os.path.basename(play_path)
    stem, _ = os.path.splitext(basename)

    match = _SEGMENTED_SWEEP_RE.search(basename)
    if match is not None:
        speakers = ",".join(s.upper() for s in match.group("speakers").split(","))
        return f"{speakers}.wav"

    # No speaker list in the basename — fall back to the play file's
    # stem with a forced ``.wav`` extension. The caller (the regular
    # speaker-side record button) should reject playing something that
    # doesn't match a known sweep convention, but we still hand back a
    # safe name so the resolved-path preview isn't blank.
    return f"{stem}.wav"


def headphones_record_filename() -> str:
    """Return Impulcifer's canonical headphone-compensation filename."""
    return HEADPHONES_FILENAME


def resolve_record_path(record_dir: str, play_path: str) -> str:
    """Join the recording folder with the derived canonical filename."""
    return os.path.join(record_dir, derive_record_filename(play_path))


def resolve_headphones_record_path(record_dir: str) -> str:
    """Join the recording folder with ``headphones.wav``."""
    return os.path.join(record_dir, headphones_record_filename())
