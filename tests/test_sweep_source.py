"""Tests for the skin-agnostic sweep-source/test-signal logic (F016).

``gui/sweep_source.py`` is meant to be importable and exercisable without a
display or a real localization manager — the label<->code mapping, SweepSpec
assembly and argument composition are pure logic shared by both CTk skins.
These tests use a minimal dict-based localization stub (mirroring
``i18n.localization.LocalizationManager.get``'s ``key, default=None,
**kwargs`` contract) instead of the real i18n stack, and simple
``SimpleNamespace`` stand-ins for the CTk tab objects the module's helpers
read attributes off of.
"""

from __future__ import annotations

import sys
from types import SimpleNamespace

import pytest

from gui import sweep_source


class StubLoc:
    """Minimal dict-backed localization stub."""

    def __init__(self, strings: dict) -> None:
        self.strings = strings

    def get(self, key: str, default: str | None = None, **kwargs) -> str:
        text = self.strings.get(key, key if default is None else default)
        if kwargs:
            text = text.format(**kwargs)
        return text


LABEL_STRINGS = {
    "option_sweep_default": "Default sweep",
    "option_sweep_custom": "Custom sweep",
    "option_sweep_file": "Play file",
    "option_test_signal_auto": "Auto-detect",
    "option_test_signal_default": "Bundled default",
    "option_test_signal_manual": "Manual parameters",
    "option_test_signal_file": "From file",
    "error_test_signal_manual_invalid": "Manual sweep parameters are invalid.",
    "label_sweep_generated_summary": "{speakers} ({tracks}) @ {fs} Hz, {duration}s",
    "message_sweep_detect_failed": "Could not detect sweep parameters.",
    "message_sweep_detect_confidence_high": "high confidence",
    "message_sweep_detect_confidence_low": "low confidence",
    "message_sweep_detected": (
        "fs={fs} duration={duration} segments={segments} files={files} ({confidence})"
    ),
}


def make_loc() -> StubLoc:
    return StubLoc(dict(LABEL_STRINGS))


def test_module_import_does_not_load_tkinter(monkeypatch: pytest.MonkeyPatch) -> None:
    for name in ("tkinter", "gui.sweep_source"):
        monkeypatch.delitem(sys.modules, name, raising=False)

    import importlib

    module = importlib.import_module("gui.sweep_source")

    assert module.RECORDER_SWEEP_MODES == ("default", "custom", "file")
    assert "tkinter" not in sys.modules


def test_recorder_sweep_mode_labels_uses_injected_localization() -> None:
    labels = sweep_source.recorder_sweep_mode_labels(make_loc())
    assert labels == {
        "default": "Default sweep",
        "custom": "Custom sweep",
        "file": "Play file",
    }


def test_test_signal_source_labels_uses_injected_localization() -> None:
    labels = sweep_source.test_signal_source_labels(make_loc())
    assert labels == {
        "auto": "Auto-detect",
        "default": "Bundled default",
        "manual": "Manual parameters",
        "file": "From file",
    }


def test_label_to_code_matches_known_label() -> None:
    labels = sweep_source.test_signal_source_labels(make_loc())
    assert sweep_source.label_to_code(labels, "Manual parameters", "auto") == "manual"


def test_label_to_code_falls_back_for_unknown_label() -> None:
    labels = sweep_source.test_signal_source_labels(make_loc())
    assert sweep_source.label_to_code(labels, "not a real label", "auto") == "auto"


def test_resolve_test_signal_arg_auto_selection() -> None:
    tab = SimpleNamespace(
        test_signal_source_var=SimpleNamespace(get=lambda: "Auto-detect"),
        test_signal_var=SimpleNamespace(get=lambda: "unused.wav"),
    )
    assert sweep_source.resolve_test_signal_arg(tab, make_loc()) == "auto"


def test_resolve_test_signal_arg_default_selection() -> None:
    tab = SimpleNamespace(
        test_signal_source_var=SimpleNamespace(get=lambda: "Bundled default"),
        test_signal_var=SimpleNamespace(get=lambda: "unused.wav"),
    )
    assert sweep_source.resolve_test_signal_arg(tab, make_loc()) == "default"


def test_resolve_test_signal_arg_manual_selection_builds_generate_spec() -> None:
    tab = SimpleNamespace(
        test_signal_source_var=SimpleNamespace(get=lambda: "Manual parameters"),
        test_signal_duration_var=SimpleNamespace(get=lambda: "6.15"),
        test_signal_fs_var=SimpleNamespace(get=lambda: "48000"),
        test_signal_var=SimpleNamespace(get=lambda: "unused.wav"),
    )
    assert sweep_source.resolve_test_signal_arg(tab, make_loc()) == "generate:6.15s@48000"


def test_resolve_test_signal_arg_manual_selection_invalid_duration_raises() -> None:
    tab = SimpleNamespace(
        test_signal_source_var=SimpleNamespace(get=lambda: "Manual parameters"),
        test_signal_duration_var=SimpleNamespace(get=lambda: "not-a-number"),
        test_signal_fs_var=SimpleNamespace(get=lambda: "48000"),
        test_signal_var=SimpleNamespace(get=lambda: "unused.wav"),
    )
    with pytest.raises(ValueError, match="Manual sweep parameters are invalid."):
        sweep_source.resolve_test_signal_arg(tab, make_loc())


def test_resolve_test_signal_arg_file_selection_returns_raw_path() -> None:
    tab = SimpleNamespace(
        test_signal_source_var=SimpleNamespace(get=lambda: "From file"),
        test_signal_var=SimpleNamespace(get=lambda: "C:/signals/my-sweep.wav"),
    )
    assert (
        sweep_source.resolve_test_signal_arg(tab, make_loc())
        == "C:/signals/my-sweep.wav"
    )


def test_resolve_test_signal_arg_without_selector_falls_back_to_raw_path_entry() -> None:
    tab = SimpleNamespace(test_signal_var=SimpleNamespace(get=lambda: "legacy.wav"))
    assert sweep_source.resolve_test_signal_arg(tab, make_loc()) == "legacy.wav"


def test_resolve_sweep_selection_file_mode_returns_none() -> None:
    result = sweep_source.resolve_sweep_selection(
        "file", speakers_text="FL,FR", tracks="stereo"
    )
    assert result is None


def test_resolve_sweep_selection_default_mode_uses_default_signal_params() -> None:
    from core.sweep_signal import DEFAULT_SWEEP_DURATION, DEFAULT_SWEEP_FS

    spec = sweep_source.resolve_sweep_selection(
        "default", speakers_text="FL, FR", tracks="stereo"
    )
    assert spec.speakers == ("FL", "FR")
    assert spec.tracks == "stereo"
    assert spec.fs == DEFAULT_SWEEP_FS
    assert spec.duration == DEFAULT_SWEEP_DURATION


def test_resolve_sweep_selection_custom_mode_parses_fs_and_duration() -> None:
    spec = sweep_source.resolve_sweep_selection(
        "custom",
        speakers_text="FC",
        tracks="stereo",
        fs_text=" 44100 ",
        duration_text=" 3.5 ",
    )
    assert spec.speakers == ("FC",)
    assert spec.fs == 44100
    assert spec.duration == 3.5


def test_resolve_sweep_selection_custom_mode_invalid_speaker_raises() -> None:
    with pytest.raises(ValueError):
        sweep_source.resolve_sweep_selection(
            "custom",
            speakers_text="NOT_A_SPEAKER",
            tracks="stereo",
            fs_text="48000",
            duration_text="5.0",
        )


def test_headphones_sweep_selection_forces_stereo_fl_fr() -> None:
    spec = sweep_source.headphones_sweep_selection(
        "custom", fs_text="48000", duration_text="5.0"
    )
    assert spec.speakers == ("FL", "FR")
    assert spec.tracks == "stereo"
    assert spec.fs == 48000
    assert spec.duration == 5.0


def test_headphones_sweep_selection_file_mode_returns_none() -> None:
    assert sweep_source.headphones_sweep_selection("file") is None


def test_sweep_summary_text_formats_spec_fields() -> None:
    spec = SimpleNamespace(speakers=("FL", "FR"), tracks="stereo", fs=48000, duration=6.15)
    summary = sweep_source.sweep_summary_text(make_loc(), spec)
    assert summary == "FL,FR (stereo) @ 48000 Hz, 6.15s"


def test_detect_summary_text_none_detection_reports_failure() -> None:
    assert (
        sweep_source.detect_summary_text(make_loc(), None)
        == "Could not detect sweep parameters."
    )


def test_detect_summary_text_high_confidence_includes_confidence_label() -> None:
    detection = SimpleNamespace(
        fs=48000,
        duration_seconds=6.15,
        n_segments=2,
        source_files=("FL,FR.wav",),
        confidence="high",
    )
    summary = sweep_source.detect_summary_text(make_loc(), detection)
    assert summary == (
        "fs=48000 duration=6.15 segments=2 files=FL,FR.wav (high confidence)"
    )


def test_detect_summary_text_low_confidence_includes_confidence_label() -> None:
    detection = SimpleNamespace(
        fs=44100,
        duration_seconds=3.0,
        n_segments=1,
        source_files=("a.wav", "b.wav"),
        confidence="low",
    )
    summary = sweep_source.detect_summary_text(make_loc(), detection)
    assert "low confidence" in summary
    assert "files=a.wav, b.wav" in summary
