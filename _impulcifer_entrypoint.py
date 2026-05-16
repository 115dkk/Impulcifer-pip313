"""Import bootstrap helpers for installed console entry points."""

from __future__ import annotations

import importlib
import sys
from pathlib import Path
from types import ModuleType


_PROJECT_IMPORT_ROOTS = ("autoeq", "core", "gui", "i18n", "infra", "updater")


def _same_path(raw_path: str, expected: Path) -> bool:
    if not raw_path:
        return False
    try:
        return Path(raw_path).resolve() == expected
    except (OSError, RuntimeError):
        return False


def _path_is_at_or_under(path: Path, directory: Path) -> bool:
    try:
        path.relative_to(directory)
        return True
    except ValueError:
        return False


def _module_lives_under(import_root: str, module: ModuleType, root: Path) -> bool:
    package_dir = root / import_root
    module_file = root / f"{import_root}.py"

    file_name = getattr(module, "__file__", None)
    if file_name:
        try:
            file_path = Path(file_name).resolve()
            if file_path == module_file or _path_is_at_or_under(file_path, package_dir):
                return True
        except (OSError, RuntimeError, ValueError):
            pass

    for raw_path in getattr(module, "__path__", ()) or ():
        try:
            if _path_is_at_or_under(Path(raw_path).resolve(), package_dir):
                return True
        except (OSError, RuntimeError):
            pass

    return False


def prefer_distribution_root() -> Path:
    """Prefer this wheel/source tree over generic modules earlier on sys.path."""
    root = Path(__file__).resolve().parent
    sys.path[:] = [path for path in sys.path if not _same_path(path, root)]
    sys.path.insert(0, str(root))

    for import_root in _PROJECT_IMPORT_ROOTS:
        module = sys.modules.get(import_root)
        if module is None or _module_lives_under(import_root, module, root):
            continue

        prefix = f"{import_root}."
        for name in list(sys.modules):
            if name == import_root or name.startswith(prefix):
                sys.modules.pop(name, None)

    importlib.invalidate_caches()
    return root


def import_distribution_module(module_name: str) -> ModuleType:
    """Import a project module after guarding against top-level name collisions."""
    prefer_distribution_root()
    return importlib.import_module(module_name)


def run_gui_entrypoint(module_name: str) -> None:
    """Run a GUI entry point from a package module."""
    module = import_distribution_module(module_name)
    module.main_gui()
