"""Fixed BRIR layouts with optional, silent extension pairs removed.

HeSuVi always consumes fourteen positional IR channels. Equalizer APO cycles
shorter IRs, so even a stereo-only measurement needs those fourteen slots.
The generic HRIR layout keeps its sixteen base slots, including LFE.
"""

from __future__ import annotations

import json
import struct
from collections.abc import Sequence
from pathlib import Path

import numpy as np

from core.constants import HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER


def base_channel_count(order: Sequence[str]) -> int:
    """Return the fixed base size; custom output layouts are kept in full."""
    if tuple(order) == tuple(HESUVI_TRACK_ORDER):
        return 14
    if tuple(order) == tuple(HEXADECAGONAL_TRACK_ORDER):
        return 16
    return len(order)


def trim_silent_extensions(data: np.ndarray, order: Sequence[str]) -> np.ndarray:
    """Trim only trailing zero extension pairs, never holes or quiet responses."""
    minimum = base_channel_count(order)
    end = len(order)
    while end > minimum and not np.any(data[end - 2:end] != 0):
        end -= 2
    return data[:end]


def compact_tracks(data: np.ndarray, order: Sequence[str]) -> tuple[np.ndarray, tuple[str, ...]]:
    """Opt-in removal of all zero rows, including positional placeholders."""
    keep = np.any(data != 0, axis=1)
    if not np.any(keep):
        raise ValueError("All BRIR channels are silent; no channels remain to write.")
    return data[keep], tuple(name for name, active in zip(order, keep) if active)


def append_track_names(path, names: Sequence[str]) -> None:
    """Embed a versioned ICHL JSON channel map in a freshly written RIFF WAV."""
    payload = json.dumps({"version": 1, "tracks": list(names)}, separators=(",", ":")).encode()
    chunk = b"ICHL" + struct.pack("<I", len(payload)) + payload
    chunk += b"\0" * (len(payload) % 2)
    with Path(path).open("r+b") as handle:
        header = handle.read(12)
        if header[:4] != b"RIFF" or header[8:12] != b"WAVE":
            raise ValueError("Compact BRIR metadata requires a RIFF WAV file.")
        size = handle.seek(0, 2)
        if size + len(chunk) - 8 > 0xFFFFFFFF:
            raise ValueError("Compact BRIR exceeds the RIFF WAV size limit.")
        handle.write(chunk)
        handle.seek(4)
        handle.write(struct.pack("<I", size + len(chunk) - 8))


def read_track_names(path, canonical_order: Sequence[str], channel_count: int) -> tuple[str, ...] | None:
    """Validate an embedded map; None means a legacy positional layout.

    Map validation precedes positional inference, including compact files that
    happen to have 14/16 channels. Unknown RIFF chunks are skipped by seeking.
    """
    found = None
    with Path(path).open("rb") as handle:
        header = handle.read(12)
        if header[:4] != b"RIFF" or header[8:12] != b"WAVE":
            return None
        end = min(struct.unpack("<I", header[4:8])[0] + 8, Path(path).stat().st_size)
        while handle.tell() + 8 <= end:
            chunk, size = struct.unpack("<4sI", handle.read(8))
            next_chunk = handle.tell() + size + size % 2
            if next_chunk > end:
                raise ValueError("Truncated WAV chunk.")
            if chunk == b"ICHL":
                if found is not None or size > 4096:
                    raise ValueError("Duplicate or oversized BRIR channel mapping.")
                try:
                    value = json.loads(handle.read(size))
                except (ValueError, UnicodeError) as exc:
                    raise ValueError("Invalid BRIR channel mapping JSON.") from exc
                if not isinstance(value, dict) or type(value.get("version")) is not int or value["version"] != 1:
                    raise ValueError("Unsupported BRIR channel mapping version.")
                names = value.get("tracks")
                if (
                    not isinstance(names, list)
                    or not all(isinstance(name, str) for name in names)
                    or len(names) != channel_count
                    or len(set(names)) != len(names)
                    or any(name not in canonical_order for name in names)
                ):
                    raise ValueError("BRIR channel mapping does not match the WAV layout.")
                found = tuple(names)
            handle.seek(next_chunk)
    return found
