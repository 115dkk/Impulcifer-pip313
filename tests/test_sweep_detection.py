"""Sweep parameter detection (``core.sweep_detection``) and the
``auto`` / ``generate:`` test-signal specs in ``pipeline_stages``.

Impulcifer sweep lengths form a discrete grid (M × ~3.08 s at 48 kHz), so
recovery only needs a rough length estimate — these tests synthesize
loopback-style recordings and assert exact (fs, M) recovery, plus the
low-confidence verdict for off-grid (non-Impulcifer) signals.
"""

from __future__ import annotations

import numpy as np
import pytest

from core.audio_io import write_wav
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.pipeline_stages import open_impulse_response_estimator
from core.sweep_detection import (
    detect_sweep_parameters,
    snap_sweep_samples,
    sweep_grid_unit,
)
from core.sweep_signal import SweepSpec, build_sweep_playback


def _write_loopback(dir_path, spec: SweepSpec) -> None:
    """Write a perfect-loopback recording for the spec's speaker list.

    The 'recording' is the mono mixdown of the sequence duplicated onto
    two mic channels — same length as the playback, exactly what
    ``play_and_record`` produces in a loopback."""
    playback = build_sweep_playback(spec)
    mix = np.sum(playback.data, axis=0)
    recording = np.vstack([mix, mix])
    write_wav(
        str(dir_path / playback.record_filename), playback.fs, recording, bit_depth=32
    )


def test_grid_snap_matches_bundled_sweep():
    unit = sweep_grid_unit(48000)
    m, n, deviation = snap_sweep_samples(2 * unit, 48000)
    assert (m, n) == (2, 295270)
    assert deviation < 1e-6
    # The bundled 6.15 s sweep sits exactly on the M=2 grid point.
    assert n == len(ImpulseResponseEstimator(min_duration=5.0, fs=48000))


def test_detects_default_sweep_from_loopback(tmp_path):
    _write_loopback(tmp_path, SweepSpec())
    result = detect_sweep_parameters(str(tmp_path))
    assert result is not None
    assert result.fs == 48000
    assert result.m == 2
    assert result.confidence == "high"
    assert result.is_default
    assert result.speakers == ("FL", "FR")
    assert result.generate_spec() == "generate:6.15s@48000"


def test_detects_custom_sweep_and_reconstructs_estimator(tmp_path):
    spec = SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR"))
    _write_loopback(tmp_path, spec)
    result = detect_sweep_parameters(str(tmp_path))
    assert result is not None
    assert result.fs == 8000
    assert result.m == 1
    assert result.confidence == "high"
    assert not result.is_default

    reference = ImpulseResponseEstimator(min_duration=1.0, fs=8000)
    estimator = result.to_estimator()
    assert estimator.fs == reference.fs
    assert len(estimator) == len(reference)
    assert np.array_equal(estimator.test_signal, reference.test_signal)


def test_detects_across_multiple_consistent_files(tmp_path):
    spec_kwargs = {"fs": 8000, "duration": 1.0}
    _write_loopback(tmp_path, SweepSpec(speakers=("FL", "FR"), **spec_kwargs))
    _write_loopback(tmp_path, SweepSpec(speakers=("FC",), tracks="stereo", **spec_kwargs))
    result = detect_sweep_parameters(str(tmp_path))
    assert result is not None
    assert result.confidence == "high"
    assert result.n_segments == 3
    assert set(result.speakers) == {"FL", "FR", "FC"}


def test_trimmed_recording_recovers_via_envelope_onsets(tmp_path):
    spec = SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR"))
    playback = build_sweep_playback(spec)
    mix = np.sum(playback.data, axis=0)
    # Chop 1.5 s off the front — the file-length grid check now misses,
    # but consecutive envelope onsets are still exactly (sweep + 2 s) apart.
    trimmed = mix[int(1.5 * playback.fs):]
    write_wav(str(tmp_path / "FL,FR.wav"), playback.fs, np.vstack([trimmed, trimmed]), bit_depth=32)
    result = detect_sweep_parameters(str(tmp_path))
    assert result is not None
    assert result.fs == 8000
    assert result.m == 1
    assert result.confidence == "high"


def test_off_grid_signal_reports_low_confidence(tmp_path):
    fs = 8000
    t = np.arange(int(3.0 * fs)) / fs
    chirp = np.sin(2 * np.pi * (100 + 400 * t) * t)
    silence = np.zeros(2 * fs)
    signal = np.concatenate([silence, chirp, silence])
    write_wav(str(tmp_path / "FL.wav"), fs, np.vstack([signal, signal]), bit_depth=32)
    result = detect_sweep_parameters(str(tmp_path))
    assert result is not None
    assert result.confidence == "low"


def test_mixed_sample_rates_report_low_confidence(tmp_path):
    _write_loopback(tmp_path, SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR")))
    _write_loopback(tmp_path, SweepSpec(fs=12000, duration=1.0, speakers=("FC",), tracks="stereo"))
    result = detect_sweep_parameters(str(tmp_path))
    assert result is not None
    assert result.confidence == "low"


def test_empty_or_irrelevant_directory_returns_none(tmp_path):
    assert detect_sweep_parameters(str(tmp_path)) is None
    (tmp_path / "headphones.wav").write_bytes(b"not a recording name match")
    assert detect_sweep_parameters(str(tmp_path)) is None
    assert detect_sweep_parameters(str(tmp_path / "missing")) is None


# ------------------------------------------------------------------
# open_impulse_response_estimator spec strings
# ------------------------------------------------------------------

def test_generate_spec_builds_exact_default_estimator(tmp_path):
    estimator = open_impulse_response_estimator(str(tmp_path), "generate:6.15s@48000")
    reference = ImpulseResponseEstimator(min_duration=5.0, fs=48000)
    assert estimator.fs == 48000
    assert len(estimator) == len(reference)
    assert np.array_equal(estimator.test_signal, reference.test_signal)


def test_generate_spec_snaps_to_grid(tmp_path):
    # 1.5 s at 8 kHz is nearest to the M=1 (~1.77 s) grid point.
    estimator = open_impulse_response_estimator(str(tmp_path), "generate:1.5s@8000")
    assert estimator.fs == 8000
    assert len(estimator) == len(ImpulseResponseEstimator(min_duration=1.0, fs=8000))


def test_auto_uses_detection_for_non_default_recordings(tmp_path):
    _write_loopback(tmp_path, SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR")))
    estimator = open_impulse_response_estimator(str(tmp_path), "auto")
    assert estimator.fs == 8000
    assert len(estimator) == len(ImpulseResponseEstimator(min_duration=1.0, fs=8000))


def test_auto_prefers_sidecar_over_detection(tmp_path):
    from core.sweep_signal import write_sidecar

    _write_loopback(tmp_path, SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR")))
    sidecar_estimator = ImpulseResponseEstimator(min_duration=1.0, fs=12000)
    write_sidecar(str(tmp_path), sidecar_estimator)
    estimator = open_impulse_response_estimator(str(tmp_path), "auto")
    assert estimator.fs == 12000


def test_auto_falls_back_to_bundled_default_on_empty_dir(tmp_path):
    estimator = open_impulse_response_estimator(str(tmp_path), "auto")
    assert estimator.fs == 48000
    assert len(estimator) == 295270


def test_none_behaves_like_auto(tmp_path):
    _write_loopback(tmp_path, SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR")))
    estimator = open_impulse_response_estimator(str(tmp_path), None)
    assert estimator.fs == 8000


def test_pkl_paths_are_rejected(tmp_path):
    pkl_path = tmp_path / "old-signal.pkl"
    pkl_path.write_bytes(b"legacy")
    with pytest.raises(TypeError):
        open_impulse_response_estimator(str(tmp_path), str(pkl_path))
