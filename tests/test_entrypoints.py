"""Tests for installed console entry point bootstrapping."""

from __future__ import annotations

import importlib
import sys
from pathlib import Path

try:
    import tomllib
except ImportError:  # pragma: no cover - Python < 3.11
    import tomli as tomllib


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_gui_console_scripts_use_collision_resistant_bootstrap() -> None:
    pyproject = tomllib.loads((PROJECT_ROOT / "pyproject.toml").read_text(encoding="utf-8"))

    scripts = pyproject["project"]["scripts"]

    assert scripts["impulcifer_gui"] == "impulcifer_gui:main_gui"
    assert scripts["impulcifer_gui_legacy"] == "impulcifer_gui_legacy:main_gui"


def test_gui_bootstrap_recovers_from_shadow_gui_module(
    monkeypatch,
    tmp_path: Path,
) -> None:
    shadow_dir = tmp_path / "python-prefix"
    shadow_dir.mkdir()
    (shadow_dir / "gui.py").write_text("MARKER = 'shadow gui module'\n", encoding="utf-8")
    monkeypatch.syspath_prepend(str(shadow_dir))

    for name in list(sys.modules):
        if name == "gui" or name.startswith("gui."):
            sys.modules.pop(name, None)

    shadow_gui = importlib.import_module("gui")
    assert getattr(shadow_gui, "MARKER") == "shadow gui module"
    assert not hasattr(shadow_gui, "__path__")

    import _impulcifer_entrypoint

    root = _impulcifer_entrypoint.prefer_distribution_root()
    gui_package = importlib.import_module("gui")

    assert root == PROJECT_ROOT
    assert Path(gui_package.__file__).resolve() == PROJECT_ROOT / "gui" / "__init__.py"
    assert hasattr(gui_package, "__path__")
