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
        ("poll_job", ("job", 4)),
        ("cancel_job", ("job",)),
    ],
)
def test_bridge_delegates_only_public_service_methods(method, args) -> None:
    from impulcifer_webview import WebviewBridge

    bridge = WebviewBridge(_FakeService())
    response = getattr(bridge, method)(*args)
    assert response["data"]["method"] == method
    assert response["data"]["args"] == list(args)


def test_main_forces_edgechromium_backend(monkeypatch) -> None:
    import impulcifer_webview

    calls = []
    fake_webview = SimpleNamespace(
        create_window=lambda *args, **kwargs: calls.append(("create", args, kwargs)),
        start=lambda **kwargs: calls.append(("start", kwargs)),
    )
    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Windows")
    monkeypatch.setattr(impulcifer_webview, "_index_uri", lambda: "file:///index.html")
    monkeypatch.setitem(sys.modules, "webview", fake_webview)

    impulcifer_webview.main()

    assert calls[0][0] == "create"
    assert calls[1] == ("start", {"gui": "edgechromium", "debug": False})


def test_main_rejects_non_windows_platform(monkeypatch) -> None:
    import impulcifer_webview

    monkeypatch.setattr(impulcifer_webview.platform, "system", lambda: "Linux")
    with pytest.raises(SystemExit, match="Windows only"):
        impulcifer_webview.main()


def test_webview_entrypoint_contains_no_qt_backend() -> None:
    root = Path(__file__).resolve().parents[1]
    source = (root / "impulcifer_webview.py").read_text(encoding="utf-8")
    pyproject = (root / "pyproject.toml").read_text(encoding="utf-8")

    for forbidden in ("PySide", "PyQt", "QtWebEngine", 'gui="qt"'):
        assert forbidden not in source
        assert forbidden not in pyproject
    assert 'webview.start(gui="edgechromium"' in source
