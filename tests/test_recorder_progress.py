"""Tests for recorder progress events and segmented sweep inference."""

from __future__ import annotations

import importlib
import sys
from types import SimpleNamespace

import numpy as np
import pytest

from core.recording_progress import event_for_elapsed, infer_sweep_segments


def _import_recorder_without_portaudio(monkeypatch):
    """Import core.recorder with a fake sounddevice module for CI."""
    core_package = sys.modules.get("core")
    previous_recorder_module = sys.modules.pop("core.recorder", None)
    had_recorder_attr = core_package is not None and hasattr(core_package, "recorder")
    previous_recorder_attr = (
        getattr(core_package, "recorder") if had_recorder_attr else None
    )

    fake_sounddevice = SimpleNamespace(
        play=lambda *_args, **_kwargs: None,
        rec=lambda *_args, **_kwargs: np.zeros((10, 2)),
    )
    monkeypatch.setitem(sys.modules, "sounddevice", fake_sounddevice)
    try:
        recorder = importlib.import_module("core.recorder")
    finally:
        sys.modules.pop("core.recorder", None)
        if previous_recorder_module is not None:
            sys.modules["core.recorder"] = previous_recorder_module
        if core_package is not None:
            if had_recorder_attr:
                setattr(core_package, "recorder", previous_recorder_attr)
            elif hasattr(core_package, "recorder"):
                delattr(core_package, "recorder")
    return recorder


def _patch_recorder_hardware(monkeypatch, recorder):
    fake_input = {"name": "Input", "hostapi": 0, "max_input_channels": 2}
    fake_output = {"name": "Output", "hostapi": 0, "max_output_channels": 2}
    monkeypatch.setattr(recorder, "get_devices", lambda **_kwargs: (fake_input, fake_output))
    monkeypatch.setattr(recorder, "set_default_devices", lambda *_args: ("Input API", "Output API"))
    monkeypatch.setattr(recorder, "record_target", lambda *_args, **_kwargs: None)


def test_infer_sweep_segments_from_segmented_file_name() -> None:
    """Segmented sweep names should map to active speaker time windows."""
    segments = infer_sweep_segments(
        "data/sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
        total_duration=18.3,
    )

    assert [segment.speaker for segment in segments] == ["FL", "FR"]
    assert segments[0].start == pytest.approx(2.0)
    assert segments[0].end == pytest.approx(8.15)
    assert segments[1].start == pytest.approx(10.15)
    assert segments[1].end == pytest.approx(16.3)


def test_event_for_elapsed_reports_active_speaker() -> None:
    """Progress events should name the currently active speaker when possible."""
    segments = infer_sweep_segments(
        "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
        total_duration=18.3,
    )

    fl_event = event_for_elapsed(elapsed=3.0, duration=18.3, segments=segments)
    assert fl_event.speaker == "FL"
    assert fl_event.segment_index == 1
    assert fl_event.segment_total == 2

    gap_event = event_for_elapsed(elapsed=9.0, duration=18.3, segments=segments)
    assert gap_event.speaker is None
    assert gap_event.segment_total == 2


def test_play_and_record_emits_lifecycle_events(monkeypatch) -> None:
    """The recorder callback should expose lifecycle events without audio hardware."""
    recorder = _import_recorder_without_portaudio(monkeypatch)
    events = []

    monkeypatch.setattr(recorder, "read_audio", lambda _path, expand=False: (10, np.zeros((2, 10)), None))
    _patch_recorder_hardware(monkeypatch, recorder)

    recorder.play_and_record(
        play="sweep.wav",
        record="out.wav",
        channels=2,
        progress_callback=events.append,
        progress_interval=0.01,
    )

    phases = [event.phase for event in events]
    assert phases[0] == "loading"
    assert "devices" in phases
    assert "recording" in phases
    assert phases[-2:] == ["saving", "complete"]


def test_play_and_record_wav_does_not_probe_truehd(monkeypatch) -> None:
    """Regular WAV playback must not trigger eager TrueHD probing."""
    recorder = _import_recorder_without_portaudio(monkeypatch)

    def fail_if_called(_path):
        raise AssertionError("WAV path must not call is_truehd_file")

    monkeypatch.setattr(recorder, "is_truehd_file", fail_if_called, raising=False)
    monkeypatch.setattr(recorder, "read_audio", lambda _path, expand=False: (10, np.zeros((2, 10)), None))
    _patch_recorder_hardware(monkeypatch, recorder)

    recorder.play_and_record(
        play="sweep.wav",
        record="out.wav",
        channels=2,
        progress_interval=0.01,
    )


def test_play_and_record_handles_mono_sweep_without_index_error(monkeypatch) -> None:
    """A 1-D mono sweep must not raise ``IndexError`` on shape probing.

    Regression: ``data/sweep-6.15s-...wav`` is the bundled headphone-comp
    sweep and lands on the soundfile fast path as a 1-D ``(samples,)``
    array. The recorder previously asked for ``data.shape[1]`` directly
    and crashed with "tuple index out of range" before any audio I/O.
    """
    recorder = _import_recorder_without_portaudio(monkeypatch)

    def fake_read_audio(_path, expand=False):
        # Only ``expand=True`` callers should reach the recorder math —
        # the fix in core.recorder must pass that explicitly.
        assert expand is True, "play_and_record must call read_audio(expand=True)"
        return 10, np.zeros((1, 16)), None

    monkeypatch.setattr(recorder, "read_audio", fake_read_audio)
    _patch_recorder_hardware(monkeypatch, recorder)

    events = []
    recorder.play_and_record(
        play="data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
        record="out.wav",
        channels=2,
        progress_callback=events.append,
        progress_interval=0.01,
    )

    phases = [event.phase for event in events]
    assert phases[-1] == "complete"


def test_play_and_record_rejects_truehd_atmos_with_unknown_layout(monkeypatch) -> None:
    """MLP files whose layout isn't recognized should fail with a clear error.

    The bundled ``11cmaster.mlp`` / ``13cmaster.mlp`` files are
    TrueHD+Atmos masters whose extra channels live in object metadata
    that FFmpeg can't decode — the recorder gets a 7.1 bed (8 ch) and
    no channel layout. Recording it silently would discard the height
    speakers; instead we tell the user to pick a real multi-channel
    sweep WAV.
    """
    recorder = _import_recorder_without_portaudio(monkeypatch)

    def fake_read_audio(_path, expand=False):
        # Mimic ``read_audio`` for an Atmos MLP: 8 channels, no layout.
        return 48000, np.zeros((8, 16)), None

    monkeypatch.setattr(recorder, "read_audio", fake_read_audio)
    _patch_recorder_hardware(monkeypatch, recorder)

    import pytest

    with pytest.raises(ValueError, match="Atmos object channels"):
        recorder.play_and_record(
            play="data/11cmaster.mlp",
            record="out.wav",
            channels=2,
            progress_interval=0.01,
        )
