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


def test_bridge_public_surface_includes_all_service_api_methods() -> None:
    from application import ImpulciferApplicationService
    from impulcifer_webview import WebviewBridge

    service_methods = {
        name
        for name, value in vars(ImpulciferApplicationService).items()
        if callable(value) and not name.startswith("_")
    }
    bridge_methods = {
        name
        for name, value in vars(WebviewBridge).items()
        if callable(value) and not name.startswith("_")
    }

    assert service_methods <= bridge_methods


def test_bridge_native_window_helpers_are_not_public() -> None:
    from impulcifer_webview import WebviewBridge

    bridge_methods = {
        name
        for name, value in vars(WebviewBridge).items()
        if callable(value) and not name.startswith("_")
    }

    assert "attach_window" not in bridge_methods
    assert "apply_titlebar_theme" not in bridge_methods


class _FakeWindow:
    def __init__(self, selection) -> None:
        self.selection = selection
        self.calls: list[tuple] = []

    def create_file_dialog(self, dialog_type, **kwargs):
        self.calls.append((dialog_type, kwargs))
        return self.selection


class _FakeEvent:
    def __init__(self) -> None:
        self.handlers = []

    def __iadd__(self, handler):
        self.handlers.append(handler)
        return self

    def fire(self) -> None:
        for handler in self.handlers:
            handler()


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
    bridge._attach_window(window)

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
    bridge._attach_window(window)

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

    def create_window(*args, **kwargs):
        calls.append(("create", args, kwargs))
        return SimpleNamespace(events=SimpleNamespace(before_show=_FakeEvent()))

    fake_webview = SimpleNamespace(
        create_window=create_window,
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
    # The title-bar callback belongs to before_show, not webview.start(): the
    # latter starts its worker before pywebview constructs the native form.
    assert start_args == ()


def test_create_window_applies_titlebar_after_pywebview_setup_before_show(monkeypatch) -> None:
    import impulcifer_webview

    before_show = _FakeEvent()
    window = SimpleNamespace(events=SimpleNamespace(before_show=before_show))
    fake_webview = SimpleNamespace(create_window=lambda *args, **kwargs: window)
    bridge = impulcifer_webview.WebviewBridge(_FakeService())
    calls = []

    monkeypatch.setattr(impulcifer_webview, "_index_uri", lambda: "file:///index.html")
    monkeypatch.setattr(impulcifer_webview, "resolve_effective_theme", lambda: "dark")
    monkeypatch.setattr(
        impulcifer_webview,
        "apply_windows_titlebar_theme",
        lambda attached_window, dark: calls.append((attached_window, dark)) or True,
    )

    created = impulcifer_webview.create_app_window(fake_webview, bridge)

    assert created is window
    assert calls == []
    assert before_show.handlers == [bridge._apply_titlebar_theme]

    # pywebview fires this only after BrowserForm.__init__ has applied its own
    # system theme, and immediately before BrowserForm.Show().
    before_show.fire()
    assert calls == [(window, True)]


def test_main_rejects_unsupported_platform(monkeypatch) -> None:
    import impulcifer_webview

    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Java")
    with pytest.raises(SystemExit, match="does not support"):
        impulcifer_webview.main()


def test_windows_titlebar_repaint_preserves_64_bit_handle(monkeypatch) -> None:
    import ctypes
    from ctypes import wintypes

    import impulcifer_webview

    class _FakeFunction:
        def __init__(self, result) -> None:
            self.result = result
            self.calls: list[tuple] = []

        def __call__(self, *args):
            self.calls.append(args)
            return self.result

    set_attribute = _FakeFunction(0)
    set_window_pos = _FakeFunction(1)
    redraw_window = _FakeFunction(1)
    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Windows")
    monkeypatch.setattr(
        ctypes,
        "windll",
        SimpleNamespace(
            dwmapi=SimpleNamespace(DwmSetWindowAttribute=set_attribute),
            user32=SimpleNamespace(
                SetWindowPos=set_window_pos,
                RedrawWindow=redraw_window,
            ),
        ),
        raising=False,
    )
    native_handle = 0x1_0000_1234
    window = SimpleNamespace(
        native=SimpleNamespace(Handle=SimpleNamespace(ToInt64=lambda: native_handle)),
    )

    assert impulcifer_webview.apply_windows_titlebar_theme(window, dark=True)
    assert set_window_pos.argtypes == [
        wintypes.HWND,
        wintypes.HWND,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        wintypes.UINT,
    ]
    assert set_window_pos.restype is wintypes.BOOL
    assert set_window_pos.calls == [
        (native_handle, 0, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0004 | 0x0020),
    ]
    assert redraw_window.argtypes == [
        wintypes.HWND,
        ctypes.c_void_p,
        ctypes.c_void_p,
        wintypes.UINT,
    ]
    assert redraw_window.restype is wintypes.BOOL
    assert redraw_window.calls == [
        (native_handle, None, None, 0x0001 | 0x0100 | 0x0400),
    ]


def test_windows_titlebar_retries_when_frame_repaint_fails(monkeypatch) -> None:
    import ctypes

    import impulcifer_webview

    class _FakeFunction:
        def __init__(self, result) -> None:
            self.result = result

        def __call__(self, *args):
            return self.result

    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Windows")
    monkeypatch.setattr(
        ctypes,
        "windll",
        SimpleNamespace(
            dwmapi=SimpleNamespace(DwmSetWindowAttribute=_FakeFunction(0)),
            user32=SimpleNamespace(
                SetWindowPos=_FakeFunction(0),
                RedrawWindow=_FakeFunction(0),
            ),
        ),
        raising=False,
    )
    window = SimpleNamespace(native=SimpleNamespace(Handle=1234))

    assert not impulcifer_webview.apply_windows_titlebar_theme(window, dark=True)


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
    bridge._attach_window(window)

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
    assert "window.events.before_show += bridge._apply_titlebar_theme" in source
    assert "webview.start(gui=backend" in source
