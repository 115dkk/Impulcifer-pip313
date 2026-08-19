"""Contract tests for recording actions shared by both native recorder skins."""

from __future__ import annotations

import ast
import os
from pathlib import Path

import pytest

from gui.recording_actions import RecordingActionsMixin


class _Var:
    def __init__(self, value: object) -> None:
        self.value = value

    def get(self):
        return self.value

    def set(self, value: object) -> None:
        self.value = value


class _Loc:
    def __init__(self) -> None:
        self.calls: list[tuple[str, dict[str, object]]] = []

    def get(self, key: str, **kwargs) -> str:
        self.calls.append((key, kwargs))
        return key


class _Harness(RecordingActionsMixin):
    def __init__(self, record_dir: Path) -> None:
        self.loc = _Loc()
        self._sweep_mode_labels = {
            "default": "Default",
            "custom": "Custom",
            "file": "File",
        }
        self.sweep_source_var = _Var("Default")
        self.sweep_speakers_var = _Var("FL,FR")
        self.sweep_layout_var = _Var("stereo")
        self.sweep_fs_var = _Var("48000")
        self.sweep_duration_var = _Var("5.0")
        self.record_dir_var = _Var(str(record_dir))
        self.play_var = _Var("")
        self.resolved_record_var = _Var("")


@pytest.fixture
def harness(tmp_path: Path) -> _Harness:
    return _Harness(tmp_path)


def test_sweep_mode_maps_localized_label_to_code(harness: _Harness) -> None:
    harness.sweep_source_var.set("Custom")

    assert harness._sweep_mode() == "custom"


def test_sweep_mode_falls_back_to_default(harness: _Harness) -> None:
    harness.sweep_source_var.set("Unknown")

    assert harness._sweep_mode() == "default"


def test_default_sweep_spec_uses_selected_speakers_and_layout(harness: _Harness) -> None:
    harness.sweep_speakers_var.set("SL, SR")
    harness.sweep_layout_var.set("stereo")

    spec = harness._resolve_sweep_spec()

    assert spec.fs == 48_000
    assert spec.duration == 5.0
    assert spec.speakers == ("SL", "SR")
    assert spec.tracks == "stereo"


def test_custom_sweep_spec_uses_entered_signal_parameters(harness: _Harness) -> None:
    harness.sweep_source_var.set("Custom")
    harness.sweep_speakers_var.set("FC")
    harness.sweep_fs_var.set("96000")
    harness.sweep_duration_var.set("2.5")

    spec = harness._resolve_sweep_spec()

    assert spec.fs == 96_000
    assert spec.duration == 2.5
    assert spec.speakers == ("FC",)


def test_file_sweep_mode_has_no_generated_spec(harness: _Harness) -> None:
    harness.sweep_source_var.set("File")

    assert harness._resolve_sweep_spec() is None


def test_generated_sweep_refreshes_record_path_from_speakers(harness: _Harness) -> None:
    harness.sweep_speakers_var.set("BL,BR")

    harness._refresh_resolved_record_path()

    assert harness.resolved_record_var.get() == "label_record_resolved_path"
    assert harness.loc.calls[-1] == (
        "label_record_resolved_path",
        {"path": os.path.join(str(harness.record_dir_var.get()), "BL,BR.wav")},
    )


def test_file_sweep_refreshes_record_path_from_play_filename(harness: _Harness) -> None:
    harness.sweep_source_var.set("File")
    harness.play_var.set(
        str(
            Path("sweeps")
            / "sweep-seg-FL,FR,FC,SL,SR,BL,BR-7.1-6.15s-48000Hz-32bit.wav"
        )
    )

    harness._refresh_resolved_record_path()

    assert harness.resolved_record_var.get() == "label_record_resolved_path"
    assert harness.loc.calls[-1] == (
        "label_record_resolved_path",
        {
            "path": os.path.join(
                str(harness.record_dir_var.get()),
                "FL,FR,FC,SL,SR,BL,BR.wav",
            )
        },
    )


def test_refresh_clears_path_for_missing_directory_or_invalid_spec(
    harness: _Harness,
) -> None:
    harness.record_dir_var.set("")
    harness._refresh_resolved_record_path()
    assert harness.resolved_record_var.get() == ""

    harness.record_dir_var.set("recordings")
    harness.sweep_source_var.set("Custom")
    harness.sweep_fs_var.set("invalid")
    harness._refresh_resolved_record_path()
    assert harness.resolved_record_var.get() == ""


def _class_node(relative_path: str, class_name: str) -> ast.ClassDef:
    root = Path(__file__).resolve().parents[1]
    tree = ast.parse((root / relative_path).read_text(encoding="utf-8"))
    return next(
        node
        for node in tree.body
        if isinstance(node, ast.ClassDef) and node.name == class_name
    )


def test_tabs_inherit_mixin_without_redeclaring_shared_actions() -> None:
    shared_methods = {
        "start_recording",
        "start_recording_headphones",
        "generate_sweep_set",
        "_resolve_sweep_spec",
        "_sweep_mode",
        "_refresh_resolved_record_path",
    }

    for relative_path, class_name in (
        ("gui/tabs/recorder_tab.py", "RecorderTab"),
        ("gui/skins/studio_recorder_tab.py", "StudioRecorderTab"),
    ):
        class_node = _class_node(relative_path, class_name)
        assert [base.id for base in class_node.bases if isinstance(base, ast.Name)] == [
            "RecordingActionsMixin"
        ]
        assert not shared_methods.intersection(
            node.name for node in class_node.body if isinstance(node, ast.FunctionDef)
        )


def test_skins_keep_distinct_speaker_and_headphone_preparation_hooks() -> None:
    for relative_path, class_name in (
        ("gui/tabs/recorder_tab.py", "RecorderTab"),
        ("gui/skins/studio_recorder_tab.py", "StudioRecorderTab"),
    ):
        class_node = _class_node(relative_path, class_name)
        methods = {
            node.name: node
            for node in class_node.body
            if isinstance(node, ast.FunctionDef)
        }
        assert "_prepare_speaker_recording" in methods
        assert "_prepare_headphones_recording" in methods

        headphone_reads = {
            node.func.value.attr
            for node in ast.walk(methods["_prepare_headphones_recording"])
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr == "get"
            and isinstance(node.func.value, ast.Attribute)
            and isinstance(node.func.value.value, ast.Name)
            and node.func.value.value.id == "self"
        }
        assert "append_var" not in headphone_reads
