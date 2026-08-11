"""Tests for the experimental pywebview bridge and entrypoint."""

from __future__ import annotations

import importlib
from pathlib import Path
import sys
from types import SimpleNamespace

import pytest


class _FakeService:
    def __getattr__(self, name):
        return lambda *args: {"ok": True, "data": {"method": name, "args": list(args)}}


def test_module_import_does_not_require_pywebview(monkeypatch) -> None:
    monkeypatch.delitem(sys.modules, "webview", raising=False)
    module = importlib.reload(importlib.import_module("impulcifer_webview"))
    assert module.WebviewBridge
    assert "webview" not in sys.modules


@pytest.mark.parametrize(
    ("method", "args"),
    [
        ("bootstrap", ()),
        ("list_audio_devices", ("API",)),
        ("start_recording", ({"mode": "speakers"},)),
        ("start_brir", ({"dir_path": "C:/measurements"},)),
        (
            "start_output_recovery",
            ({"dir_path": "C:/outputs", "include_hangloose": True},),
        ),
        ("poll_job", ("job", 4)),
        ("cancel_job", ("job",)),
        ("get_ui_settings", ()),
        ("set_language", ("ko",)),
        ("set_theme", ("light",)),
        ("set_skin", ("stable",)),
        ("set_frontend", ("ctk",)),
        ("get_system_info", ()),
        ("resolve_recording_paths", ("dir", "play.wav", "speakers", {"mode": "default"})),
        ("generate_sweep_set", ("dir",)),
        ("detect_sweep", ("dir",)),
        ("open_path", ("dir",)),
        ("check_for_updates", ()),
        ("start_update", ({"latest_version": "9.9.9"},)),
        ("apply_pending_update", ()),
    ],
)
def test_bridge_delegates_only_public_service_methods(method, args) -> None:
    from impulcifer_webview import WebviewBridge

    bridge = WebviewBridge(_FakeService())
    response = getattr(bridge, method)(*args)
    assert response["data"]["method"] == method
    assert response["data"]["args"] == list(args)


class _FakeWindow:
    def __init__(self, selection) -> None:
        self.selection = selection
        self.calls: list[tuple] = []

    def create_file_dialog(self, dialog_type, **kwargs):
        self.calls.append((dialog_type, kwargs))
        return self.selection


def _install_fake_webview(monkeypatch) -> SimpleNamespace:
    fake_webview = SimpleNamespace(
        FileDialog=SimpleNamespace(OPEN="OPEN", FOLDER="FOLDER", SAVE="SAVE"),
    )
    monkeypatch.setitem(sys.modules, "webview", fake_webview)
    return fake_webview


def test_select_file_uses_native_open_dialog(monkeypatch) -> None:
    from impulcifer_webview import WebviewBridge

    _install_fake_webview(monkeypatch)
    window = _FakeWindow(["C:/sounds/sweep.wav"])
    bridge = WebviewBridge(_FakeService())
    bridge.attach_window(window)

    response = bridge.select_file("wav")

    assert response["ok"]
    assert response["data"]["path"] == "C:/sounds/sweep.wav"
    dialog_type, kwargs = window.calls[0]
    assert dialog_type == "OPEN"
    assert kwargs["allow_multiple"] is False
    assert any("*.wav" in file_type for file_type in kwargs["file_types"])


def test_select_directory_uses_folder_dialog_and_handles_cancel(monkeypatch) -> None:
    from impulcifer_webview import WebviewBridge

    _install_fake_webview(monkeypatch)
    window = _FakeWindow(None)
    bridge = WebviewBridge(_FakeService())
    bridge.attach_window(window)

    response = bridge.select_directory()

    assert response["ok"]
    assert response["data"]["path"] is None
    assert window.calls[0][0] == "FOLDER"


def test_dialogs_require_attached_window() -> None:
    from impulcifer_webview import WebviewBridge

    bridge = WebviewBridge(_FakeService())
    response = bridge.select_file("audio")
    assert not response["ok"]
    assert response["error"]["code"] == "NO_WINDOW"


def test_open_url_is_allowlist_only(monkeypatch) -> None:
    import webbrowser

    from impulcifer_webview import WebviewBridge

    opened: list[str] = []
    monkeypatch.setattr(webbrowser, "open", lambda url: opened.append(url))
    bridge = WebviewBridge(_FakeService())

    response = bridge.open_url("fork_repo")
    assert response["ok"]
    assert opened == ["https://github.com/115dkk/Impulcifer-pip313"]

    rejected = bridge.open_url("https://evil.example.com")
    assert not rejected["ok"]
    assert rejected["error"]["code"] == "INVALID_REQUEST"
    assert opened == ["https://github.com/115dkk/Impulcifer-pip313"]


@pytest.mark.parametrize(
    ("system", "expected_backend"),
    [
        ("Windows", "edgechromium"),
        ("Darwin", "cocoa"),
        ("Linux", "gtk"),
    ],
)
def test_main_forces_platform_backend(monkeypatch, system, expected_backend) -> None:
    import impulcifer_webview

    calls = []
    fake_webview = SimpleNamespace(
        create_window=lambda *args, **kwargs: calls.append(("create", args, kwargs)),
        start=lambda *args, **kwargs: calls.append(("start", args, kwargs)),
    )
    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: system)
    monkeypatch.setattr(impulcifer_webview, "_index_uri", lambda: "file:///index.html")
    monkeypatch.setitem(sys.modules, "webview", fake_webview)

    impulcifer_webview.main()

    assert calls[0][0] == "create"
    # The pre-load window background matches the resolved Pulse theme token.
    assert calls[0][2]["background_color"] in ("#101214", "#f3f5f7")
    start_name, start_args, start_kwargs = calls[1]
    assert start_name == "start"
    assert start_kwargs == {"gui": expected_backend, "debug": False}
    # First positional arg is the on-shown callback that themes the title bar.
    assert len(start_args) == 1 and callable(start_args[0])


def test_main_rejects_unsupported_platform(monkeypatch) -> None:
    import impulcifer_webview

    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Java")
    with pytest.raises(SystemExit, match="does not support"):
        impulcifer_webview.main()


def test_bootstrap_reports_webview_backend(monkeypatch) -> None:
    import impulcifer_webview
    from impulcifer_webview import WebviewBridge

    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Darwin")
    bridge = WebviewBridge(_FakeService())
    response = bridge.bootstrap()
    assert response["ok"]
    assert response["data"]["webview_backend"] == "cocoa"


def test_apply_pending_update_closes_window_on_restart(monkeypatch) -> None:
    import threading

    from impulcifer_webview import WebviewBridge

    class _RestartingService:
        def apply_pending_update(self):
            return {"ok": True, "data": {"restarting": True}}

    timers: list[tuple[float, object]] = []

    class _FakeTimer:
        def __init__(self, interval, function):
            timers.append((interval, function))

        def start(self):
            pass

    monkeypatch.setattr(threading, "Timer", _FakeTimer)

    destroyed: list[bool] = []
    window = SimpleNamespace(destroy=lambda: destroyed.append(True))
    bridge = WebviewBridge(_RestartingService())
    bridge.attach_window(window)

    response = bridge.apply_pending_update()
    assert response["ok"]
    assert len(timers) == 1

    _interval, close_window = timers[0]
    close_window()
    assert destroyed == [True]


def test_apply_pending_update_without_window_does_not_crash(monkeypatch) -> None:
    from impulcifer_webview import WebviewBridge

    class _RestartingService:
        def apply_pending_update(self):
            return {"ok": True, "data": {"restarting": True}}

    bridge = WebviewBridge(_RestartingService())
    response = bridge.apply_pending_update()
    assert response["ok"]


def test_webview_entrypoint_contains_no_qt_backend() -> None:
    root = Path(__file__).resolve().parents[1]
    source = (root / "impulcifer_webview.py").read_text(encoding="utf-8")
    pyproject = (root / "pyproject.toml").read_text(encoding="utf-8")

    for forbidden in ("PySide", "PyQt", "QtWebEngine", 'gui="qt"'):
        assert forbidden not in source
        assert forbidden not in pyproject
    # Only the three natively-validated engines may appear in the backend map.
    assert '"Windows": "edgechromium"' in source
    assert '"Darwin": "cocoa"' in source
    assert '"Linux": "gtk"' in source
    assert "webview.start(bridge.apply_titlebar_theme, gui=backend" in source
