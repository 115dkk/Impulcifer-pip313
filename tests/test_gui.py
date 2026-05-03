"""Focused tests for modern GUI support helpers."""

from __future__ import annotations

import os

import pytest

from core.recording_validation import validate_recording_setup
from gui.event_bus import EventBus


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
