"""Focused tests for modern GUI support helpers."""

from __future__ import annotations

import os
from pathlib import Path
from typing import get_type_hints

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


class DummyVar:
    def __init__(self, value: object) -> None:
        self.value = value

    def get(self) -> object:
        return self.value


class DummyEqTab:
    def __init__(self, recording_dir: Path, eq_file: Path) -> None:
        self.do_equalization_var = DummyVar(True)
        self.dir_path_var = DummyVar(str(recording_dir))
        self.eq_file_var = DummyVar(str(eq_file))
        self.eq_left_file_var = DummyVar("eq-left.csv")
        self.eq_right_file_var = DummyVar("eq-right.csv")


class _ContractTab:
    pass


def test_brir_tab_protocol_attributes_are_snapshot_compatible() -> None:
    from gui.brir_args import BrirTabLike

    contract_attributes = set(get_type_hints(BrirTabLike))
    assert contract_attributes
    assert all(
        name.endswith("_var") or name.endswith("_vars")
        for name in contract_attributes
    )

    tab = _ContractTab()
    for name in contract_attributes:
        if name.endswith("_vars"):
            setattr(tab, name, {"FL": DummyVar("10")})
        else:
            setattr(tab, name, DummyVar(name))

    snapshot = gui_utils.snapshot_tk_vars(tab)
    assert set(snapshot) == contract_attributes


def test_brir_tab_classes_declare_protocol_attributes_in_initializers() -> None:
    import ast
    import inspect

    from gui.brir_args import BrirTabLike
    from gui.skins.studio_impulcifer_tab import StudioImpulciferTab
    from gui.tabs.impulcifer_tab import ImpulciferTab

    contract_attributes = set(get_type_hints(BrirTabLike))

    def assigned_self_attributes(tab_class) -> set[str]:
        tree = ast.parse(inspect.getsource(tab_class))
        return {
            node.attr
            for node in ast.walk(tree)
            if isinstance(node, ast.Attribute)
            and isinstance(node.ctx, ast.Store)
            and isinstance(node.value, ast.Name)
            and node.value.id == "self"
        }

    for tab_class in (ImpulciferTab, StudioImpulciferTab):
        assert contract_attributes <= assigned_self_attributes(tab_class)


def test_modern_gui_version_uses_shared_resolver(monkeypatch) -> None:
    import gui.modern_gui as modern_gui

    monkeypatch.setattr(modern_gui, "get_app_version", lambda: "9.8.7")
    app = modern_gui.ModernImpulciferGUI.__new__(modern_gui.ModernImpulciferGUI)

    assert app.get_current_version() == "9.8.7"


def test_stable_tab_key_lookup_uses_widget_identity() -> None:
    from gui.modern_gui import ModernImpulciferGUI

    class _FakeTabview:
        def __init__(self, selected_name: str, selected_widget: object) -> None:
            self.selected_name = selected_name
            self.selected_widget = selected_widget
            self.selected_by_set = None

        def get(self):
            return self.selected_name

        def tab(self, _name):
            return self.selected_widget

        def set(self, name):
            self.selected_by_set = name

    widget = object()
    app = ModernImpulciferGUI.__new__(ModernImpulciferGUI)
    app.tabview = _FakeTabview("중복 표시 라벨", widget)
    app.tab_widget_keys = {str(widget): "settings"}
    app.tab_labels = {"settings": "중복 표시 라벨"}

    assert app._current_stable_tab_key() == "settings"
    app.select_tab("settings")
    assert app.tabview.selected_by_set == "중복 표시 라벨"


def test_studio_custom_eq_selection_is_synced_to_recording_dir(tmp_path: Path) -> None:
    """A selected Studio EQ file must be the file the backend reads."""
    from gui.brir_args import sync_custom_eq_files

    recording_dir = tmp_path / "recordings"
    source_dir = tmp_path / "external"
    recording_dir.mkdir()
    source_dir.mkdir()
    selected_eq = source_dir / "my-eq.csv"
    selected_eq.write_text("frequency,raw\n20,0\n", encoding="utf-8")

    sync_custom_eq_files(DummyEqTab(recording_dir, selected_eq))

    assert (recording_dir / "eq.csv").read_text(encoding="utf-8") == "frequency,raw\n20,0\n"


def test_studio_processing_labels_are_localized() -> None:
    """Studio should not regress to raw English labels for shared controls."""
    studio_source = (Path(__file__).resolve().parents[1] / "gui/skins/studio_impulcifer_tab.py").read_text(
        encoding="utf-8"
    )

    for hardcoded in ("Specific limit", "Generic limit", "FR combination", "Crossover", "Sub HP", "Polarity"):
        assert f'label="{hardcoded}"' not in studio_source


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


def test_recording_status_helpers_summarize_saved_wav(tmp_path: Path) -> None:
    """Recording status summaries should detect duration, peak, and active channels."""
    import numpy as np
    import soundfile as sf

    from gui.recording_status import (
        analyze_recording,
        format_duration,
        inspect_playback_file,
    )

    sample_rate = 48_000
    samples = sample_rate // 10
    audio = np.zeros((samples, 2), dtype=np.float32)
    audio[:, 0] = 0.5

    wav_path = tmp_path / "FL,FR.wav"
    sf.write(wav_path, audio, sample_rate, subtype="FLOAT")

    playback_info = inspect_playback_file(str(wav_path))
    assert playback_info is not None
    assert playback_info.channels == 2
    assert playback_info.duration == pytest.approx(0.1)

    summary = analyze_recording(str(wav_path))
    assert summary is not None
    assert summary.channels == 2
    assert summary.active_channels == 1
    assert summary.duration == pytest.approx(0.1)
    assert summary.peak_db == pytest.approx(-6.02, abs=0.02)
    assert format_duration(summary.duration) == "0:00"


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
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda *a, **k: font_path)
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
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda *a, **k: font_path)
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

    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda *a, **k: font_path)
    monkeypatch.setattr(gui_utils, "_font_family_from_file", lambda _: "Pretendard")
    monkeypatch.setattr(gui_utils, "_register_font_file_for_tk", lambda _: True)
    monkeypatch.setattr(gui_utils, "_tk_renders_family", fake_renders)

    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"
    first_count = probe_calls["count"]
    assert gui_utils.setup_pretendard_font("ko") == "Pretendard"
    assert probe_calls["count"] == first_count, "second call should hit the cache"


def test_setup_pretendard_font_japanese_uses_jp_cut(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Japanese must resolve to the Pretendard JP family.

    The standard Pretendard build ships no kanji, so Japanese has to land on
    ``Pretendard JP Variable`` — the only bundled cut that renders ideographs.
    """
    gui_utils._font_cache.clear()
    gui_utils._bundled_fonts_registered_for_tk = False
    probed: list[str] = []

    def fake_renders(family):
        probed.append(family)
        return family if "JP" in family else None

    monkeypatch.setattr(gui_utils, "register_all_bundled_fonts_for_tk", lambda: [])
    monkeypatch.setattr(gui_utils, "_tk_renders_family", fake_renders)

    assert gui_utils.setup_pretendard_font("ja") == "Pretendard JP Variable"
    # The first family probed for Japanese must be the JP cut, never the
    # kanji-less standard "Pretendard Variable".
    assert probed[0] == "Pretendard JP Variable"
    assert all("JP" in family for family in probed)


def test_setup_pretendard_font_japanese_refuses_kanji_less_fallback(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """If the JP cut can't render, Japanese falls back to the OS font (None),
    never to the standard Pretendard that would tofu every kanji."""
    gui_utils._font_cache.clear()
    gui_utils._bundled_fonts_registered_for_tk = False

    monkeypatch.setattr(gui_utils, "register_all_bundled_fonts_for_tk", lambda: [])
    monkeypatch.setattr(gui_utils, "_tk_renders_family", lambda _: None)
    monkeypatch.setattr(gui_utils, "_find_pretendard_font_file", lambda *a, **k: None)
    # A standard (non-JP) Pretendard is visible to Tk, but Japanese must NOT
    # accept it as a fallback.
    monkeypatch.setattr(gui_utils, "_get_tk_font_families", lambda: {"Pretendard Variable"})

    assert gui_utils.setup_pretendard_font("ja") is None


def test_find_pretendard_font_file_selects_cut_by_variant(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """``_find_pretendard_font_file`` returns the JP file only when asked, and
    skips JP files for the standard (ko/en) path."""
    fonts = [
        Path("font/PretendardJPVariable.ttf"),
        Path("font/PretendardVariable.ttf"),
    ]
    monkeypatch.setattr(gui_utils, "_scan_bundled_fonts", lambda: fonts)

    assert gui_utils._find_pretendard_font_file(prefer_jp=True).name == "PretendardJPVariable.ttf"
    assert gui_utils._find_pretendard_font_file(prefer_jp=False).name == "PretendardVariable.ttf"


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
    import core.font_setup as core_font_setup

    # Drop every Pretendard the dev machine has installed so the loader's
    # only path to a Pretendard family is the repo's bundled .otf.
    monkeypatch.setattr(
        fm.fontManager,
        "ttflist",
        [e for e in fm.fontManager.ttflist if "pretendard" not in (e.fname or "").lower()],
    )
    # Force re-run (the module memoizes the first call). The memo gate lives in
    # core.font_setup after the #115-8 split; core_utils.set_matplotlib_font is
    # a re-export of the same function and reads the gate there.
    monkeypatch.setattr(core_font_setup, "_font_configured", False)

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


def test_resolve_recording_channels_stable_contract() -> None:
    """Shared channel contract (audit #138 F021/Q5): stereo unless forced."""
    from core.recording_validation import resolve_recording_channels

    assert resolve_recording_channels(False, 14) == (2, False)
    assert resolve_recording_channels(True, 14) == (14, True)
    assert resolve_recording_channels(True, 6) == (6, True)
    # Studio maps "preset != 2" to the force flag; stereo preset stays silent.
    assert resolve_recording_channels(2 != 2, 2) == (2, False)
