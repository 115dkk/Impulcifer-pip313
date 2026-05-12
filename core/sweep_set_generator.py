#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Generate the canonical 14-channel surround sweep set.

The set is four sweep WAV files routed to a 7.1.6 (14-channel) layout, one
per recording group:

* ``sweep-seg-FL,FR-7.1.6-…wav`` — FL then FR sequential, others silent.
* ``sweep-seg-FC-7.1.6-…wav`` — FC alone (single speaker, single sweep).
* ``sweep-seg-SL,SR-7.1.6-…wav`` — SL then SR sequential.
* ``sweep-seg-BL,BR-7.1.6-…wav`` — BL then BR sequential.

The recordings produced by these sweeps land in
``<speakers>.wav`` files (``FL,FR.wav`` / ``FC.wav`` / ``SL,SR.wav`` /
``BL,BR.wav``) which Impulcifer's BRIR pipeline scans natively. With the
typical 2-ear-mic setup, recordings are 2-channel (time-segmented for
the stereo groups). Users with a 14-channel parallel-mic rig instead get
4-channel files for the stereo groups and a 2-channel file for FC, for a
total of 14 simultaneous mic channels (4 + 2 + 4 + 4) — both modes are
accepted by ``HRIR.open_recording``.

Why a generator instead of bundled WAVs: at PCM_32 the four 14-channel
files weigh ~167 MB, more than doubling the repo's tracked data size.
This module is invoked once per user environment to materialize them.
"""

from __future__ import annotations

import argparse
import os
from typing import Sequence

from core.impulse_response_estimator import ImpulseResponseEstimator
from core.utils import write_wav


# Canonical recording groups for the 14-channel split. ``open_recording``
# accepts each via the speaker-list filename pattern (``FL,FR.wav`` etc.).
SWEEP_SET_GROUPS: tuple[tuple[str, ...], ...] = (
    ("FL", "FR"),
    ("FC",),
    ("SL", "SR"),
    ("BL", "BR"),
)

DEFAULT_TRACKS = "7.1.6"  # 14-channel Atmos layout
DEFAULT_DURATION = 5.0
DEFAULT_FS = 48000
DEFAULT_BIT_DEPTH = 32


def _format_filename(speakers: Sequence[str], tracks: str, ire: ImpulseResponseEstimator, bit_depth: int) -> str:
    """Match the existing ``sweep-seg-…`` naming so derivation works."""
    return f'sweep-seg-{",".join(speakers)}-{tracks}-{ire.file_name(bit_depth)}.wav'


def generate_sweep_set(
    dir_path: str,
    *,
    tracks: str = DEFAULT_TRACKS,
    duration: float = DEFAULT_DURATION,
    fs: int = DEFAULT_FS,
    bit_depth: int = DEFAULT_BIT_DEPTH,
    groups: Sequence[Sequence[str]] = SWEEP_SET_GROUPS,
) -> list[str]:
    """Write the surround sweep set into ``dir_path`` and return the file paths.

    Reuses one ``ImpulseResponseEstimator`` so every group shares the same
    underlying sweep — important because Impulcifer's deconvolution stage
    needs every recording in the directory to be paired with the same
    test signal.
    """
    if not os.path.isdir(dir_path):
        raise NotADirectoryError(f"Sweep output directory does not exist: {dir_path}")

    ire = ImpulseResponseEstimator(min_duration=duration, fs=fs)

    output_paths: list[str] = []
    for speakers in groups:
        speakers = list(speakers)
        data = ire.sweep_sequence(speakers, tracks)
        filename = _format_filename(speakers, tracks, ire, bit_depth)
        path = os.path.join(dir_path, filename)
        write_wav(path, ire.fs, data, bit_depth=bit_depth)
        output_paths.append(path)
    return output_paths


def main(argv: Sequence[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        description=(
            "Generate the canonical 14-channel surround sweep set "
            "(FL,FR / FC / SL,SR / BL,BR) at the requested layout."
        )
    )
    parser.add_argument(
        "--dir_path",
        type=str,
        required=True,
        help="Output directory for the generated sweep WAV files.",
    )
    parser.add_argument(
        "--tracks",
        type=str,
        default=DEFAULT_TRACKS,
        help=(
            "Track layout passed to sweep_sequence(). Defaults to "
            "'7.1.6' (14-channel Atmos). Other valid values: '7.1' (8-ch), "
            "'5.1' (6-ch), 'stereo' (2-ch)."
        ),
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
        tracks=args.tracks,
        duration=args.duration,
        fs=args.fs,
        bit_depth=args.bit_depth,
    )
    print("Generated sweep set:")
    for path in paths:
        print(f"  {path}")


if __name__ == "__main__":
    main()
