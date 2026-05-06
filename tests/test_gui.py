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


def test_setup_pretendard_font_uses_render_layer_check(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Bundled Pretendard is used only after Tk's render layer resolves it.

    The previous version of this test gated on ``tkfont.families()``. We now
    gate on a render-layer probe (``tkfont.Font(family=X).actual('family')``)
    because ``AddFontResourceExW`` does not always invalidate the families
    cache, but Windows GDI / Tk render WILL pick up the registered font
    immediately. That divergence was the root cause of the GUI falling
    back to Malgun Gothic / 명조 on default Windows machines.
    """
    font_path = Path("Pretendard-Regular.otf")
    # First probe: Tk can't render yet (mimics pre-registration). Second
    # probe: registration succeeded and Tk's render layer reports the
    # requested family.
    rendered = iter([None, "Pretendard"])

    gui_utils._font_cache.clear()
    gui_utils._bundled_fonts_registered_for_tk = False
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_tk_renders_family", lambda _: next(rendered))

    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"


def test_setup_pretendard_font_falls_back_when_render_layer_cannot_resolve(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """If Tk's render layer can't resolve Pretendard despite registration,
    return None so CTk widgets use the system default rather than silently
    pretending Pretendard is in effect."""
    font_path = Path("Pretendard-Regular.otf")

    gui_utils._font_cache.clear()
    gui_utils._bundled_fonts_registered_for_tk = False
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_tk_renders_family", lambda _: None)
    monkeypatch.setattr(gui_utils, "_get_tk_font_families", lambda: set())

    assert gui_utils.setup_pretendard_font("ko") is None


def test_setup_pretendard_font_caches_render_layer_hit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Successful render-layer resolution is cached per language, so the
    next call returns the same family without re-probing Tk."""
    font_path = Path("Pretendard-Regular.otf")

    gui_utils._font_cache.clear()
    gui_utils._bundled_fonts_registered_for_tk = False
    probe_calls = {"count": 0}

    def fake_renders(_):
        probe_calls["count"] += 1
        return "Pretendard"

    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_tk_renders_family", fake_renders)

    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"
    first_count = probe_calls["count"]
    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"
    assert probe_calls["count"] == first_count, "second call should hit the cache"


def test_set_matplotlib_font_picks_bundled_when_no_system_pretendard(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """End-user simulation: with no system Pretendard, the BUNDLED file must
    be the one matplotlib applies. Silent fall-through to Malgun / sans-serif
    is treated as a hard failure here because rendering Korean glyphs without
    Pretendard is the bug we are guarding against.
    """
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.font_manager as fm

    import core.utils as core_utils

    # Drop every Pretendard the dev machine has installed so the loader's
    # only path to a Pretendard family is the repo's bundled .otf.
    monkeypatch.setattr(
        fm.fontManager,
        "ttflist",
        [e for e in fm.fontManager.ttflist if "pretendard" not in (e.fname or "").lower()],
    )
    # Force re-run (the module memoizes the first call).
    monkeypatch.setattr(core_utils, "_font_configured", False)

    result = core_utils.set_matplotlib_font()

    assert result["source"] == "bundled", (
        f"Expected bundled source, got {result['source']!r}. "
        f"This means the loader couldn't find font/Pretendard-Regular.otf "
        f"via infra.resource_helper.get_font_path()."
    )
    assert result["is_pretendard"], (
        f"matplotlib didn't resolve a Pretendard file: {result['path']!r}"
    )
    assert result["path"] is not None and "pretendard" in str(result["path"]).lower()


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
