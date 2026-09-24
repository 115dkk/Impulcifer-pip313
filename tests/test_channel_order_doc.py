"""docs/brir-channel-order.md must match the track orders in core/constants.py.

The document lists every hrir.wav/hesuvi.wav track with its speaker and ear.
A wrong row sends a speaker to the wrong ear for whoever trusts it, so each
table is compared with the canonical constants row by row.
"""

import re
from pathlib import Path

import pytest

from core.constants import HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, speaker_side

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs" / "brir-channel-order.md"
EAR = {"left": "왼쪽 귀", "right": "오른쪽 귀"}


def _table(name):
    text = DOC.read_text(encoding="utf-8")
    match = re.search(
        rf"<!-- channel-table:{name} -->\n(.*?)\n<!-- /channel-table:{name} -->", text, re.DOTALL
    )
    assert match, f"table marker {name!r} missing from {DOC.name}"
    lines = match.group(1).strip().splitlines()[2:]  # skip header and separator
    return [[cell.strip() for cell in line.strip().strip("|").split("|")] for line in lines]


def _relation(speaker, ear):
    side = speaker_side(speaker)
    if side == "center":
        return "가운데"
    return "같은 쪽" if side == ear else "반대쪽"


@pytest.mark.parametrize("name,order", [("hrir", HEXADECAGONAL_TRACK_ORDER), ("hesuvi", HESUVI_TRACK_ORDER)])
def test_track_table_matches_constants(name, order):
    rows = _table(name)
    assert len(rows) == len(order)
    for i, (row, track) in enumerate(zip(rows, order)):
        speaker, ear = track.rsplit("-", 1)
        assert row[:6] == [str(i + 1), str(i), f"`{track}`", speaker, EAR[ear], _relation(speaker, ear)], row


def test_hesuvi_virtual_channels_name_the_ear_they_feed():
    """HeSuVi's L*/SL*/RL*/C0 feed the left ear, digit 0 = same-side speaker."""
    for row, track in zip(_table("hesuvi"), HESUVI_TRACK_ORDER):
        speaker, ear = track.rsplit("-", 1)
        channel = row[6]
        if HESUVI_TRACK_ORDER.index(track) >= 14:
            assert channel == "(쓰지 않음)"
            continue
        if channel.startswith("C"):
            assert (speaker, ear) == ("FC", "left" if channel == "C0" else "right")
            continue
        assert ear == ("left" if channel.rstrip("01") in ("L", "SL", "RL") else "right"), row
        assert _relation(speaker, ear) == ("같은 쪽" if channel.endswith("0") else "반대쪽"), row


def test_pair_table_matches_constants():
    for row in _table("pairs"):
        speaker, numbers = row[0], row[1:]
        expected = [
            str(order.index(f"{speaker}-{ear}") + 1) if f"{speaker}-{ear}" in order else "없음"
            for order in (HEXADECAGONAL_TRACK_ORDER, HESUVI_TRACK_ORDER)
            for ear in ("left", "right")
        ]
        assert numbers == expected, row


@pytest.mark.parametrize("order", [HEXADECAGONAL_TRACK_ORDER, HESUVI_TRACK_ORDER])
def test_odd_wav_numbers_are_left_ear(order):
    """The document promises odd 1-based numbers are always the left ear."""
    assert all(track.endswith("-left") == (i % 2 == 0) for i, track in enumerate(order))


def test_readme_links_channel_order_doc():
    assert "docs/brir-channel-order.md" in (ROOT / "README.md").read_text(encoding="utf-8")
