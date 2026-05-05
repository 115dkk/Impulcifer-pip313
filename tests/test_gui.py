"""Focused tests for modern GUI support helpers."""

from __future__ import annotations

import os
from pathlib import Path

import pytest

from core.recording_validation import validate_recording_setup
from gui.event_bus import EventBus
from gui import utils as gui_utils


class DummyLoc:
    """Minimal localization object for dialog construction tests."""

    current_language = "en"

    def get(self, key: str, **kwargs: object) -> str:
        """Return a useful fallback for tests."""
        default = kwargs.get("default")
        return str(default if default is not None else key)


def test_event_bus_emits_and_unsubscribes() -> None:
    """EventBus delivers payloads and supports unsubscribe callbacks."""
    bus = EventBus()
    calls: list[dict[str, str]] = []

    unsubscribe = bus.on("language_changed", lambda **kwargs: calls.append(kwargs))
    bus.emit("language_changed", code="ko")
    unsubscribe()
    bus.emit("language_changed", code="en")

    assert calls == [{"code": "ko"}]


def test_validate_recording_setup_detects_channel_mismatch() -> None:
    """Filename speaker lists are converted into expected stereo channel counts."""
    result = validate_recording_setup("data/my_hrir/FL,FR,FC.wav", 4, True)

    assert result is not None
    assert result.has_mismatch is True
    assert result.expected_speakers == ["FL", "FR", "FC"]
    assert result.expected_channels == 6
    assert result.selected_channels == 4


def test_validate_recording_setup_ignores_unknown_filenames() -> None:
    """Non-speaker filenames are not force-validated."""
    assert validate_recording_setup("recording.wav", 2, True) is None


def test_setup_pretendard_font_requires_tk_visible_family(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Bundled Pretendard is used only after Tk can see the registered family."""
    font_path = Path("Pretendard-Regular.otf")
    families = iter([set(), {"Pretendard"}])

    gui_utils._font_cache.clear()
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_get_tk_font_families", lambda: next(families))

    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"


def test_setup_pretendard_font_falls_back_when_tk_cannot_see_font(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A found font file should not be treated as a GUI font unless Tk sees it."""
    font_path = Path("Pretendard-Regular.otf")

    gui_utils._font_cache.clear()
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_get_tk_font_families", lambda: set())

    assert gui_utils.setup_pretendard_font("ko") is None


def test_setup_pretendard_font_does_not_cache_unvalidated_family(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Rootless registration should not poison the language cache."""
    font_path = Path("Pretendard-Regular.otf")

    gui_utils._font_cache.clear()
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_get_tk_font_families", lambda: None)

    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"
    assert "ko" not in gui_utils._font_cache


@pytest.fixture
def ctk_root():
    """Create a CustomTkinter root when a display is available."""
    if os.name != "nt" and not os.environ.get("DISPLAY"):
        pytest.skip("No display available for GUI widget tests")

    import customtkinter as ctk

    root = ctk.CTk()
    root.withdraw()
    try:
        yield root
    finally:
        root.destroy()


def test_processing_dialog_cancel_sets_event(ctk_root) -> None:
    """The processing dialog exposes a cancellation event for workers."""
    from gui.dialogs import ProcessingDialog

    dialog = ProcessingDialog(ctk_root, DummyLoc(), fonts=None)
    dialog.withdraw()
    try:
        assert dialog.cancel_event.is_set() is False
        dialog.on_cancel()
        assert dialog.cancel_event.is_set() is True
    finally:
        dialog.destroy()
