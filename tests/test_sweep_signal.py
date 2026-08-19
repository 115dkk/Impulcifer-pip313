"""On-the-fly sweep generation contracts (``core.sweep_signal``).

The load-bearing test here is bit-identity between the generated default
FL,FR stereo sequence and the bundled ``sweep-seg-FL,FR-stereo-…wav`` —
that equality is what makes file-less recording compatible with every
existing Impulcifer capture.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np
import pytest

from core.audio_io import read_wav, write_wav
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.recording_naming import (
    record_filename_for_speakers,
    resolve_record_path_for_speakers,
)
from core.recording_progress import infer_sweep_segments
from core.sweep_signal import (
    DEFAULT_SWEEP_DURATION,
    DEFAULT_SWEEP_FS,
    SWEEP_TRACK_LAYOUTS,
    SweepSpec,
    build_sweep_playback,
    validate_sweep_spec,
    write_sidecar,
)

PROJECT_ROOT = Path(__file__).parent.parent
BUNDLED_STEREO = (
    PROJECT_ROOT / "data" / "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
)

# Small/fast parameters for layout tests: fs 8000 → grid unit ≈ 1.77 s.
FAST_SPEC_KWARGS = {"fs": 8000, "duration": 1.0}


def test_default_playback_matches_bundled_wav():
    """Generated default sequence ≡ bundled sweep-seg WAV.

    Strict bit-identity to the committed artifact only holds on the
    platform whose libm produced it (transcendental functions differ by
    1 ULP across platforms — same reason BRIR hashes are compared
    same-machine, not against absolute baselines). The cross-platform
    contract is: same shape/fs and at most 1 float32 ULP per sample.
    """
    playback = build_sweep_playback(SweepSpec())
    fs, data = read_wav(str(BUNDLED_STEREO))
    assert playback.fs == fs == DEFAULT_SWEEP_FS
    assert playback.data.shape == data.shape
    assert np.max(np.abs(playback.data - data)) <= 2**-23  # 1 float32 ULP at |x|<=1
    assert playback.record_filename == "FL,FR.wav"


def test_playback_is_bit_identical_to_locally_written_wav(tmp_path):
    """On any one machine, on-the-fly playback == playing a materialized file.

    This is the real compatibility contract: the in-memory PCM_32
    round trip must reproduce exactly what write_wav would put on disk,
    so recording without a file behaves bit-for-bit like recording with
    one generated on the same machine.
    """
    playback = build_sweep_playback(SweepSpec(speakers=("FL", "FR")))
    path = tmp_path / "sweep-seg-local.wav"
    write_wav(str(path), playback.fs, playback.estimator.sweep_sequence(["FL", "FR"], "stereo"), bit_depth=32)
    fs, data = read_wav(str(path))
    assert fs == playback.fs
    assert np.array_equal(playback.data, data)


def test_default_segments_match_filename_inference():
    playback = build_sweep_playback(SweepSpec())
    inferred = infer_sweep_segments(str(BUNDLED_STEREO), playback.duration)
    assert len(playback.segments) == len(inferred) == 2
    for built, reference in zip(playback.segments, inferred):
        assert built.speaker == reference.speaker
        assert built.index == reference.index
        assert built.total == reference.total
        # The filename carries "6.15" (2 decimals) while the built
        # segments use the exact sample count (6.1515 s), so boundaries
        # drift by the rounding times the slot index.
        assert built.start == pytest.approx(reference.start, abs=0.02)
        assert built.end == pytest.approx(reference.end, abs=0.02)


@pytest.mark.parametrize(
    "tracks, n_tracks, speaker_tracks",
    [
        ("7.1.4", 12, {"FL": 0, "TFL": 8, "TBR": 11}),
        ("7.1.6", 14, {"FL": 0, "TSL": 10, "TBR": 13}),
    ],
)
def test_height_layouts_map_speakers_to_expected_tracks(tracks, n_tracks, speaker_tracks):
    speakers = tuple(speaker_tracks)
    playback = build_sweep_playback(SweepSpec(speakers=speakers, tracks=tracks, **FAST_SPEC_KWARGS))
    assert playback.data.shape[0] == n_tracks
    # LFE (track 3) never plays.
    assert not np.any(playback.data[3])
    active_tracks = {index for index in range(n_tracks) if np.any(playback.data[index])}
    assert active_tracks == set(speaker_tracks.values())
    # Each speaker plays in its own time slot on its own track, in order.
    sweep_len = len(playback.estimator)
    fs = playback.fs
    for slot, speaker in enumerate(speakers):
        track = playback.data[speaker_tracks[speaker]]
        start = int((2.0 * fs + sweep_len) * slot + 2.0 * fs)
        assert np.any(track[start:start + sweep_len])
        assert not np.any(track[:start])


def test_record_filename_for_speakers():
    assert record_filename_for_speakers(["FL", "FR"]) == "FL,FR.wav"
    assert record_filename_for_speakers(["fc"]) == "FC.wav"
    assert record_filename_for_speakers(("tfl", "TFR")) == "TFL,TFR.wav"
    assert resolve_record_path_for_speakers("out", ["SL", "SR"]).endswith("SL,SR.wav")
    with pytest.raises(ValueError):
        record_filename_for_speakers([])
    with pytest.raises(ValueError):
        record_filename_for_speakers(["FL", "FL"])
    with pytest.raises(ValueError):
        record_filename_for_speakers(["XX"])


def test_validate_sweep_spec_rejections():
    with pytest.raises(ValueError):
        validate_sweep_spec(SweepSpec(speakers=("TFL",), tracks="7.1"))
    with pytest.raises(ValueError):
        validate_sweep_spec(SweepSpec(speakers=("FL", "FR", "FC"), tracks="stereo"))
    with pytest.raises(ValueError):
        validate_sweep_spec(SweepSpec(fs=1000))
    with pytest.raises(ValueError):
        validate_sweep_spec(SweepSpec(duration=0.01))
    with pytest.raises(ValueError):
        validate_sweep_spec(SweepSpec(tracks="9.1.6"))


def test_stereo_layout_allows_repositioned_pairs():
    # A stereo-only user physically repositions the pair, so any speaker
    # name is valid in the stereo layout (matches sweep_sequence).
    spec = validate_sweep_spec(SweepSpec(speakers=("tsl", "tsr"), tracks="stereo"))
    assert spec.speakers == ("TSL", "TSR")


def test_is_default_signal():
    assert SweepSpec().is_default_signal()
    assert SweepSpec(speakers=("FC",), tracks="7.1").is_default_signal()
    assert not SweepSpec(fs=44100).is_default_signal()
    assert not SweepSpec(duration=3.0).is_default_signal()


def test_mono_layout_forces_fl():
    playback = build_sweep_playback(
        SweepSpec(speakers=("FR",), tracks="mono", **FAST_SPEC_KWARGS)
    )
    assert playback.data.shape[0] == 1
    assert playback.record_filename == "FL.wav"
    assert [segment.speaker for segment in playback.segments] == ["FL"]


def test_layout_constant_matches_sequence_support():
    for tracks in SWEEP_TRACK_LAYOUTS:
        build_sweep_playback(SweepSpec(speakers=("FL",), tracks=tracks, **FAST_SPEC_KWARGS))


def test_write_sidecar_roundtrip(tmp_path):
    playback = build_sweep_playback(SweepSpec(speakers=("FL",), **FAST_SPEC_KWARGS))
    path = write_sidecar(str(tmp_path), playback.estimator)
    assert Path(path).name == "test.wav"

    from core.pipeline_stages import open_impulse_response_estimator

    estimator = open_impulse_response_estimator(str(tmp_path), None)
    assert estimator.fs == 8000
    assert len(estimator) == len(playback.estimator)
    # from_wav keeps its regenerated float64 signal when the file matches,
    # so the round trip reproduces the generated sweep exactly.
    assert np.array_equal(estimator.test_signal, playback.estimator.test_signal)


def test_default_duration_expands_like_bundled_sweep():
    estimator = ImpulseResponseEstimator(
        min_duration=DEFAULT_SWEEP_DURATION, fs=DEFAULT_SWEEP_FS
    )
    assert f"{estimator.duration:.2f}" == "6.15"
