"""Preserve HeSuVi routing while removing only unused extension pairs."""
from __future__ import annotations

import numpy as np
import pytest
import soundfile as sf

from core.brir_layout import trim_silent_extensions
from core.brir_recovery import BrirRecoveryError, recover_brir_outputs
from core.constants import HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER
from core.hrir import HRIR
from core.impulse_response import ImpulseResponse


def _hrir(speakers=("FL", "FR")):
    class Estimator:
        fs = 48000
    hrir = HRIR(Estimator())
    for i, speaker in enumerate(speakers):
        hrir.irs[speaker] = {}
        for j, side in enumerate(("left", "right")):
            samples = np.zeros(64)
            samples[4 + i] = (2 * i + j + 1) / 32
            hrir.irs[speaker][side] = ImpulseResponse(samples, hrir.fs)
    return hrir


@pytest.mark.parametrize("order,minimum", [(HESUVI_TRACK_ORDER, 14), (HEXADECAGONAL_TRACK_ORDER, 16)])
@pytest.mark.parametrize("speakers,extra", [(("FL", "FR"), 0), (("TFL",), 6), (("TBR",), 16)])
def test_generation_keeps_base_holes_and_last_active_pair(tmp_path, order, minimum, speakers, extra):
    hrir = _hrir(speakers)
    # A speaker with just one active ear must retain both ear slots.
    hrir.irs[speakers[-1]]["right"].data[:] = 0
    path = tmp_path / "output.wav"
    hrir.write_wav(path, track_order=order, trim_extensions=True)
    data, _ = sf.read(path, always_2d=True)
    assert data.shape == (64, minimum + extra)
    for index, track in enumerate(order[:minimum + extra]):
        speaker, side = track.split("-")
        expected = hrir.irs[speaker][side].data if speaker in hrir.irs else np.zeros(64)
        np.testing.assert_array_equal(data[:, index], expected)


def test_quiet_extension_and_custom_layout_are_never_pruned():
    data = np.zeros((30, 64))
    data[18, -1] = 1e-30
    shortened = trim_silent_extensions(data, HESUVI_TRACK_ORDER)
    assert shortened.shape[0] == 20
    assert shortened[18, -1] == 1e-30
    custom = np.zeros((4, 64))
    assert trim_silent_extensions(custom, ["FL-left", "FL-right", "FR-left", "FR-right"]).shape[0] == 4


def test_all_zero_base_channels_are_kept():
    assert trim_silent_extensions(np.zeros((30, 64)), HESUVI_TRACK_ORDER).shape[0] == 14
    assert trim_silent_extensions(np.zeros((32, 64)), HEXADECAGONAL_TRACK_ORDER).shape[0] == 16


def test_hesuvi_stereo_routing_requires_all_fourteen_slots():
    # HeSuVi 2.0.0.1 hesuvi.txt selects this fixed order of source signals.
    # Equalizer APO ConvolutionFilter applies IR channel i % file_channels.
    sources = np.array([0, 0, 2, 2, 4, 4, 6, 1, 1, 3, 3, 5, 5, 6])
    # Distinct FL/FR ear responses; no assumption of head/room symmetry.
    gains = np.zeros(14)
    gains[[0, 1, 7, 8]] = [1, 2, 3, 4]
    # mix.txt: L0 SL0 RL0 C0 L1 SL1 RL1 -> L; the rest -> R.
    left_slots = {0, 2, 4, 6, 8, 10, 12}
    def routing(ir):
        matrix = np.zeros((2, 7))
        for slot, source in enumerate(sources):
            ear = 0 if slot in left_slots else 1
            matrix[ear, source] += ir[slot % len(ir)]
        return matrix
    expected = routing(gains)
    np.testing.assert_array_equal(expected[:, :2], [[1, 4], [2, 3]])
    assert not np.any(expected[:, 2:])
    for length in range(1, 14):
        assert not np.array_equal(routing(gains[:length]), expected)
    assert not np.array_equal(routing(gains[gains != 0]), expected)
    # Removing only the sixteen unused extension tracks is exactly equivalent.
    np.testing.assert_array_equal(routing(np.pad(gains, (0, 16))), expected)


@pytest.mark.parametrize("source,order", [("hrir", HEXADECAGONAL_TRACK_ORDER), ("hesuvi", HESUVI_TRACK_ORDER)])
@pytest.mark.parametrize("speakers", [("FL", "FR"), ("TFL",), ("TBR",)])
def test_trimmed_sources_roundtrip_with_channel_identity(tmp_path, source, order, speakers):
    hrir = _hrir(speakers)
    hrir.write_wav(tmp_path / f"{source}.wav", track_order=order, trim_extensions=True)
    before = (tmp_path / f"{source}.wav").read_bytes()
    result = recover_brir_outputs(tmp_path, include_hangloose=True)
    assert set(result.speakers) == set(speakers)
    assert (tmp_path / f"{source}.wav").read_bytes() == before
    other = "hesuvi" if source == "hrir" else "hrir"
    expected = tmp_path / "expected.wav"
    hrir.write_wav(expected, track_order=HESUVI_TRACK_ORDER if other == "hesuvi" else HEXADECAGONAL_TRACK_ORDER, trim_extensions=True)
    assert (tmp_path / f"{other}.wav").read_bytes() == expected.read_bytes()
    # Matching mixed/full/trimmed lengths are compared after zero padding.
    assert not recover_brir_outputs(tmp_path).created_files
    for speaker in speakers:
        data, _ = sf.read(tmp_path / "Hangloose" / f"{speaker}.wav", always_2d=True)
        np.testing.assert_array_equal(data[:, 0], hrir.irs[speaker]["left"].data)
        np.testing.assert_array_equal(data[:, 1], hrir.irs[speaker]["right"].data)


@pytest.mark.parametrize("source,count", [("hrir", 14), ("hrir", 17), ("hrir", 34), ("hesuvi", 4), ("hesuvi", 9), ("hesuvi", 15), ("hesuvi", 32)])
def test_ambiguous_or_incompatible_channel_counts_rejected(tmp_path, source, count):
    sf.write(tmp_path / f"{source}.wav", np.zeros((64, count)), 48000, subtype="PCM_32")
    with pytest.raises(BrirRecoveryError) as exc:
        recover_brir_outputs(tmp_path)
    assert exc.value.code == "INVALID_CHANNEL_COUNT"
    assert len(list(tmp_path.glob('*.wav'))) == 1


def test_integrity_hash_ignores_only_zero_extension_padding(tmp_path):
    from tests.test_brir_integrity import _sha256_brir_samples
    full = np.zeros((64, 30), dtype=np.int32)
    full[3, 0] = 1234567
    full[5, 8] = -7654321
    old, new = tmp_path / 'old.wav', tmp_path / 'new.wav'
    sf.write(old, full, 48000, subtype='PCM_32')
    sf.write(new, full[:, :14], 48000, subtype='PCM_32')
    assert _sha256_brir_samples(old) == _sha256_brir_samples(new)
    # A one-bit PCM change must still fail the integrity check.
    full[5, 8] += 1
    sf.write(new, full[:, :14], 48000, subtype='PCM_32')
    assert _sha256_brir_samples(old) != _sha256_brir_samples(new)
    # Discarding a real extension signal must also fail.
    full[5, 8] -= 1
    full[6, 22] = 1
    sf.write(old, full, 48000, subtype='PCM_32')
    sf.write(new, full[:, :14], 48000, subtype='PCM_32')
    assert _sha256_brir_samples(old) != _sha256_brir_samples(new)
