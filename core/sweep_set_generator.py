#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Generate the canonical surround sweep set for HRIR capture.

The set covers all seven main surround speakers (FL, FR, FC, SL, SR, BL,
BR) and is written in two complementary forms so both real multichannel
rigs and stereo-only setups can measure a full surround layout:

* One **combined** 8-channel (7.1) file that plays every speaker in
  sequence on its own physical channel:
  ``sweep-seg-FL,FR,FC,SL,SR,BL,BR-7.1-…wav``. A user with a real 7.1
  output device records this once and Impulcifer splits the single
  ``FL,FR,FC,SL,SR,BL,BR.wav`` recording into per-speaker responses.

* Several **stereo** 2-channel files, one per recording group:
  ``sweep-seg-FL,FR-stereo-…wav`` / ``sweep-seg-FC-stereo-…wav`` /
  ``sweep-seg-SL,SR-stereo-…wav`` / ``sweep-seg-BL,BR-stereo-…wav``.
  A user with only a stereo pair physically repositions the two speakers
  to each group's location and records the groups one at a time on
  ordinary 2-channel hardware — no multichannel sound card required.

Both forms derive Impulcifer-native recording filenames (``<speakers>.wav``)
through :mod:`core.recording_naming`, so either workflow drops straight
into the BRIR pipeline. The two forms are alternatives — record *either*
the combined file *or* the stereo groups into a capture folder, not both
(the same speaker must not appear in two recordings of one session).

The filenames intentionally tag only the playback layout (``7.1`` /
``stereo``); they no longer advertise height channels, because this set
measures the seven ground speakers only.

Why a generator instead of bundled WAVs: the combined 8-channel file
alone is tens of MB at PCM_32. These are materialized once per user
environment rather than tracked in the repo.
"""

from __future__ import annotations

import argparse
import os
from typing import Sequence

from core.impulse_response_estimator import ImpulseResponseEstimator
from core.utils import write_wav


# Combined file: all seven ground speakers played sequentially on a 7.1
# (8-channel) layout. The order here is the playback/time order and is
# echoed in the filename, so ``open_recording`` splits the recording's
# time segments back onto these speakers in the same order.
COMBINED_SPEAKERS: tuple[str, ...] = ("FL", "FR", "FC", "SL", "SR", "BL", "BR")
COMBINED_TRACKS = "7.1"

# Per-group stereo files for stereo-only capture. Each group is written as
# a 2-channel sweep; the user repositions a stereo pair to the group's
# physical location before recording. ``open_recording`` accepts each via
# the speaker-list filename pattern (``FL,FR.wav`` / ``FC.wav`` / …).
STEREO_GROUPS: tuple[tuple[str, ...], ...] = (
    ("FL", "FR"),
    ("FC",),
    ("SL", "SR"),
    ("BL", "BR"),
)

DEFAULT_DURATION = 5.0
DEFAULT_FS = 48000
DEFAULT_BIT_DEPTH = 32


def _format_filename(speakers: Sequence[str], tracks: str, ire: ImpulseResponseEstimator, bit_depth: int) -> str:
    """Match the existing ``sweep-seg-…`` naming so derivation works."""
    return f'sweep-seg-{",".join(speakers)}-{tracks}-{ire.file_name(bit_depth)}.wav'


def generate_sweep_set(
    dir_path: str,
    *,
    duration: float = DEFAULT_DURATION,
    fs: int = DEFAULT_FS,
    bit_depth: int = DEFAULT_BIT_DEPTH,
) -> list[str]:
    """Write the surround sweep set into ``dir_path`` and return the file paths.

    Produces the per-group stereo files first (so a caller can default the
    play picker to the universally playable ``FL,FR`` stereo sweep) and the
    combined 7.1 file last.

    Reuses one ``ImpulseResponseEstimator`` so every file shares the same
    underlying sweep — important because Impulcifer's deconvolution stage
    needs every recording in a directory to be paired with the same test
    signal.
    """
    if not os.path.isdir(dir_path):
        raise NotADirectoryError(f"Sweep output directory does not exist: {dir_path}")

    ire = ImpulseResponseEstimator(min_duration=duration, fs=fs)

    output_paths: list[str] = []

    # Stereo per-group files (2-channel) for stereo-only capture.
    for group in STEREO_GROUPS:
        speakers = list(group)
        data = ire.sweep_sequence(speakers, "stereo")
        filename = _format_filename(speakers, "stereo", ire, bit_depth)
        path = os.path.join(dir_path, filename)
        write_wav(path, ire.fs, data, bit_depth=bit_depth)
        output_paths.append(path)

    # Combined 7.1 (8-channel) file for a real multichannel output device.
    combined = list(COMBINED_SPEAKERS)
    data = ire.sweep_sequence(combined, COMBINED_TRACKS)
    filename = _format_filename(combined, COMBINED_TRACKS, ire, bit_depth)
    path = os.path.join(dir_path, filename)
    write_wav(path, ire.fs, data, bit_depth=bit_depth)
    output_paths.append(path)

    return output_paths


def main(argv: Sequence[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        description=(
            "Generate the surround sweep set: a combined 7.1 file plus "
            "per-group stereo files (FL,FR / FC / SL,SR / BL,BR)."
        )
    )
    parser.add_argument(
        "--dir_path",
        type=str,
        required=True,
        help="Output directory for the generated sweep WAV files.",
    )
    parser.add_argument(
        "--duration",
        type=float,
        default=DEFAULT_DURATION,
        help="Sweep duration in seconds (default: 5.0; matches bundled sweeps).",
    )
    parser.add_argument(
        "--fs",
        type=int,
        default=DEFAULT_FS,
        help="Sample rate in Hz (default: 48000).",
    )
    parser.add_argument(
        "--bit_depth",
        type=int,
        default=DEFAULT_BIT_DEPTH,
        help="WAV bit depth (default: 32).",
    )
    args = parser.parse_args(argv)

    paths = generate_sweep_set(
        args.dir_path,
        duration=args.duration,
        fs=args.fs,
        bit_depth=args.bit_depth,
    )
    print("Generated sweep set:")
    for path in paths:
        print(f"  {path}")


if __name__ == "__main__":
    main()
