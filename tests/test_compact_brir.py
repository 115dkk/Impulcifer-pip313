"""Opt-in gap removal keeps enough information for exact channel recovery."""
from __future__ import annotations

import argparse
import json
import shutil
import struct

import numpy as np
import pytest
import soundfile as sf

from core.brir_layout import append_track_names, compact_tracks, read_track_names
from core.brir_recovery import BrirRecoveryError, recover_brir_outputs
from core.cli_builder import add_processing_config_arguments
from core.constants import HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER
from tests.test_silent_extensions import _hrir


@pytest.mark.parametrize('name,order', [('hrir', HEXADECAGONAL_TRACK_ORDER), ('hesuvi', HESUVI_TRACK_ORDER)])
def test_compact_source_restores_compatible_output_exactly(tmp_path, name, order):
    hrir = _hrir(('FL', 'FR', 'TBL'))
    hrir.irs['FL']['right'].data[:] = 0
    directory = tmp_path / 'source'
    directory.mkdir()
    source = directory / f'{name}.wav'
    hrir.write_wav(source, track_order=order, remove_silent_channels=True)
    data, _ = sf.read(source, always_2d=True)
    assert data.shape == (64, 5)
    names = read_track_names(source, order, 5)
    assert 'FL-right' not in names
    for index, track in enumerate(names):
        speaker, side = track.split('-')
        np.testing.assert_array_equal(data[:, index], hrir.irs[speaker][side].data)
    original = source.read_bytes()
    recover_brir_outputs(directory, include_hangloose=True)
    assert source.read_bytes() == original
    other = 'hesuvi' if name == 'hrir' else 'hrir'
    other_order = HESUVI_TRACK_ORDER if other == 'hesuvi' else HEXADECAGONAL_TRACK_ORDER
    reference = tmp_path / 'reference.wav'
    hrir.write_wav(reference, track_order=other_order, trim_extensions=True)
    assert (directory / f'{other}.wav').read_bytes() == reference.read_bytes()
    # The compact and compatible source layouts compare by track identity.
    assert not recover_brir_outputs(directory).created_files
    split, _ = sf.read(directory / 'Hangloose' / 'FL.wav', always_2d=True)
    assert split.shape[1] == 2 and not np.any(split[:, 1])


def test_recovery_can_compact_and_reexpand_after_file_move(tmp_path):
    original = tmp_path / 'original'
    original.mkdir()
    hrir = _hrir()
    hrir.write_wav(original / 'hrir.wav', trim_extensions=True)
    recover_brir_outputs(original, remove_silent_channels=True)
    assert sf.info(original / 'hesuvi.wav').channels == 4
    moved = tmp_path / 'moved'
    moved.mkdir()
    shutil.copyfile(original / 'hesuvi.wav', moved / 'hesuvi.wav')
    recover_brir_outputs(moved, remove_silent_channels=True)
    assert sf.info(moved / 'hrir.wav').channels == 4
    assert not recover_brir_outputs(moved).created_files
    (moved / 'hesuvi.wav').unlink()
    recover_brir_outputs(moved)
    reference = tmp_path / 'reference.wav'
    hrir.write_wav(reference, track_order=HESUVI_TRACK_ORDER, trim_extensions=True)
    assert (moved / 'hesuvi.wav').read_bytes() == reference.read_bytes()


def test_compact_hangloose_source_produces_mapped_pair(tmp_path):
    split = tmp_path / 'Hangloose'
    split.mkdir()
    sf.write(split / 'TFL.wav', np.ones((64, 2)) / 8, 48000, subtype='PCM_32')
    recover_brir_outputs(split, remove_silent_channels=True)
    for name, order in [('hrir', HEXADECAGONAL_TRACK_ORDER), ('hesuvi', HESUVI_TRACK_ORDER)]:
        path = tmp_path / f'{name}.wav'
        assert sf.info(path).channels == 2
        assert read_track_names(path, order, 2) == ('TFL-left', 'TFL-right')
    assert not recover_brir_outputs(tmp_path).created_files


def test_exact_zero_only_and_all_silent_rejected_before_writing(tmp_path):
    data = np.array([[0., 0.], [0., 1e-30], [0., -0.25]])
    compact, names = compact_tracks(data, ['zero', 'quiet', 'normal'])
    assert names == ('quiet', 'normal')
    np.testing.assert_array_equal(compact, data[1:])
    hrir = _hrir(('FL',))
    for ir in hrir.irs['FL'].values():
        ir.data[:] = 0
    with pytest.raises(ValueError, match='All BRIR channels'):
        hrir.write_wav(tmp_path / 'empty.wav', remove_silent_channels=True)
    assert not (tmp_path / 'empty.wav').exists()
    hrir.write_wav(tmp_path / 'hrir.wav')
    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path, remove_silent_channels=True)
    assert error.value.code == 'ALL_CHANNELS_SILENT'
    assert not (tmp_path / 'hesuvi.wav').exists()


def test_one_remaining_ear_is_mono_and_recoverable(tmp_path):
    hrir = _hrir(('FR',))
    hrir.irs['FR']['left'].data[:] = 0
    source = tmp_path / 'hrir.wav'
    hrir.write_wav(source, remove_silent_channels=True)
    assert sf.info(source).channels == 1
    recover_brir_outputs(tmp_path)
    data, _ = sf.read(tmp_path / 'hesuvi.wav', always_2d=True)
    assert data.shape[1] == 14
    assert np.flatnonzero(np.any(data, axis=0)).tolist() == [7]


@pytest.mark.parametrize('names', [['FL-left', 'FL-left'], ['FL-left', 'UNKNOWN'], ['FL-left']])
def test_invalid_map_rejected_before_writes(tmp_path, names):
    source = tmp_path / 'hrir.wav'
    sf.write(source, np.ones((64, 2)) / 8, 48000, subtype='PCM_32')
    append_track_names(source, names)
    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path)
    assert error.value.code == 'INVALID_CHANNEL_MAP'
    assert not (tmp_path / 'hesuvi.wav').exists()


@pytest.mark.parametrize('payload', [b'{', json.dumps({'version': 2, 'tracks': []}).encode(), b' ' * 4097])
def test_bad_json_version_or_oversized_map_rejected(tmp_path, payload):
    source = tmp_path / 'hrir.wav'
    sf.write(source, np.zeros((64, 16)), 48000, subtype='PCM_32')
    with source.open('r+b') as file:
        file.seek(0, 2)
        file.write(b'ICHL' + struct.pack('<I', len(payload)) + payload + b'\0' * (len(payload) % 2))
        end = file.tell()
        file.seek(4)
        file.write(struct.pack('<I', end - 8))
    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path)
    assert error.value.code == 'INVALID_CHANNEL_MAP'


def test_fourteen_compact_channels_are_not_mistaken_for_base_layout(tmp_path):
    hrir = _hrir(('FL', 'FR', 'FC', 'SL', 'SR', 'TFL', 'TFR'))
    source = tmp_path / 'hesuvi.wav'
    hrir.write_wav(source, track_order=HESUVI_TRACK_ORDER, remove_silent_channels=True)
    assert sf.info(source).channels == 14
    result = recover_brir_outputs(tmp_path)
    assert 'TFL' in result.speakers and 'BL' not in result.speakers
    assert sf.info(tmp_path / 'hrir.wav').channels == 24


def test_metadata_failure_rolls_back_recovery_outputs(tmp_path, monkeypatch):
    from core import brir_recovery
    split = tmp_path / 'Hangloose'
    split.mkdir()
    sf.write(split / 'FL.wav', np.ones((64, 2)) / 8, 48000, subtype='PCM_32')
    def fail(*_args):
        raise OSError('simulated metadata write failure')
    monkeypatch.setattr(brir_recovery, 'append_track_names', fail)
    with pytest.raises(BrirRecoveryError) as error:
        recover_brir_outputs(tmp_path, remove_silent_channels=True)
    assert error.value.code == 'OUTPUT_WRITE_FAILED'
    assert list(tmp_path.glob('*.wav')) == []
    assert list(tmp_path.glob('.*.wav')) == []


def test_cli_compatibility_default_and_explicit_opt_in():
    parser = argparse.ArgumentParser()
    add_processing_config_arguments(parser)
    assert parser.parse_args([]).remove_silent_channels is False
    assert parser.parse_args(['--remove_silent_channels']).remove_silent_channels is True


def test_combined_output_recovery_ignores_raw_measurements_in_same_folder(tmp_path):
    hrir = _hrir()
    hrir.write_wav(tmp_path / 'hesuvi.wav', track_order=HESUVI_TRACK_ORDER, remove_silent_channels=True)
    # These share measurement speaker suffixes but are not Hangloose outputs.
    for name in ['FC.wav', 'room-FC.wav', 'FL,FR.wav']:
        sf.write(tmp_path / name, np.ones((64, 2)) / 4, 48000, subtype='PCM_32')
    before = {name: (tmp_path / name).read_bytes() for name in ['FC.wav', 'room-FC.wav', 'FL,FR.wav']}
    result = recover_brir_outputs(tmp_path, include_hangloose=True)
    assert result.source_kind == 'hesuvi'
    assert sf.info(tmp_path / 'hrir.wav').channels == 16
    assert (tmp_path / 'Hangloose' / 'FL.wav').is_file()
    for name, contents in before.items():
        assert (tmp_path / name).read_bytes() == contents
