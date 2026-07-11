"""Tests for the gui_main frontend launcher (webview default + CTk fallback)."""

from __future__ import annotations

import sys
from types import SimpleNamespace

import gui_main


def test_resolve_frontend_cli_flag_wins(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py", "--frontend=ctk"])
    assert gui_main._resolve_frontend() == "ctk"


def test_resolve_frontend_rejects_unknown_flag_value(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py", "--frontend=qt"])
    assert gui_main._resolve_frontend() == "webview"


def test_resolve_frontend_reads_persisted_setting(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py"])
    fake_loc = SimpleNamespace(get_frontend=lambda: "ctk")
    monkeypatch.setitem(
        sys.modules,
        "i18n.localization",
        SimpleNamespace(get_localization_manager=lambda: fake_loc),
    )
    assert gui_main._resolve_frontend() == "ctk"


def test_resolve_frontend_defaults_to_webview_when_settings_unavailable(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py"])

    def _boom():
        raise OSError("settings unreadable")

    monkeypatch.setitem(
        sys.modules,
        "i18n.localization",
        SimpleNamespace(get_localization_manager=_boom),
    )
    assert gui_main._resolve_frontend() == "webview"


def test_launch_frontend_runs_webview_by_default(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py", "--frontend=webview"])
    launched: list[str] = []
    monkeypatch.setitem(sys.modules, "webview", SimpleNamespace())
    monkeypatch.setitem(
        sys.modules,
        "impulcifer_webview",
        SimpleNamespace(
            main=lambda: launched.append("webview"),
            select_gui_backend=lambda: "edgechromium",
        ),
    )
    monkeypatch.setitem(
        sys.modules,
        "gui.modern_gui",
        SimpleNamespace(main_gui=lambda: launched.append("ctk")),
    )

    gui_main._launch_frontend()
    assert launched == ["webview"]


def test_launch_frontend_falls_back_when_pywebview_missing(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py", "--frontend=webview"])
    launched: list[str] = []
    # A None entry in sys.modules makes ``import webview`` raise ImportError.
    monkeypatch.setitem(sys.modules, "webview", None)
    monkeypatch.setitem(
        sys.modules,
        "gui.modern_gui",
        SimpleNamespace(main_gui=lambda: launched.append("ctk")),
    )

    gui_main._launch_frontend()
    assert launched == ["ctk"]


def test_launch_frontend_falls_back_when_webview_start_fails(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py", "--frontend=webview"])
    launched: list[str] = []

    def _failing_main() -> None:
        raise RuntimeError("system WebKit missing")

    monkeypatch.setitem(sys.modules, "webview", SimpleNamespace())
    monkeypatch.setitem(
        sys.modules,
        "impulcifer_webview",
        SimpleNamespace(main=_failing_main, select_gui_backend=lambda: "gtk"),
    )
    monkeypatch.setitem(
        sys.modules,
        "gui.modern_gui",
        SimpleNamespace(main_gui=lambda: launched.append("ctk")),
    )

    gui_main._launch_frontend()
    assert launched == ["ctk"]


def test_launch_frontend_honours_ctk_choice(monkeypatch) -> None:
    monkeypatch.setattr(gui_main.sys, "argv", ["gui_main.py", "--frontend=ctk"])
    launched: list[str] = []
    monkeypatch.setitem(
        sys.modules,
        "gui.modern_gui",
        SimpleNamespace(main_gui=lambda: launched.append("ctk")),
    )

    gui_main._launch_frontend()
    assert launched == ["ctk"]
