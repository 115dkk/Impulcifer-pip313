# -*- coding: utf-8 -*-
"""Runtime environment detection for the updater (issue #87 follow-up).

Split out of ``updater/updater_core.py`` so the four updater backends
(:mod:`updater.velopack`, :mod:`updater.pip_updater`, :mod:`updater.legacy`,
:mod:`updater.executors`) can share these checks without circular imports.

Public API:
    - ``_is_standalone_build()`` — Nuitka-marker / sys.frozen probe
    - ``is_velopack_environment()`` — Velopack ``Update.exe`` next to the app
    - ``get_velopack_update_exe()`` — Path to ``Update.exe`` if present
    - ``is_pip_environment()`` — Whether the current install is a pip package
"""

import subprocess
import sys
from pathlib import Path
from typing import Optional


def _is_standalone_build() -> bool:
    """빌드 마커 기반 스탠드얼론(Nuitka) 빌드 감지."""
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "standalone"
    except ImportError:
        pass
    # 폴백: 기존 런타임 감지 (마커 없는 구 빌드 호환)
    if getattr(sys, 'frozen', False):
        return True
    if '__nuitka__' in sys.modules:
        return True
    return False


def is_velopack_environment() -> bool:
    """
    Check if running in a Velopack-installed environment.
    Velopack creates Update.exe in the app's parent directory.
    """
    if not _is_standalone_build():
        return False

    app_dir = Path(sys.executable).parent
    # Velopack 구조: {packId}/current/app.exe, {packId}/Update.exe
    update_exe = app_dir.parent / "Update.exe"
    return update_exe.exists()


def get_velopack_update_exe() -> Optional[Path]:
    """Get path to Velopack's Update.exe if available."""
    if not _is_standalone_build():
        return None

    app_dir = Path(sys.executable).parent
    update_exe = app_dir.parent / "Update.exe"

    if update_exe.exists():
        return update_exe
    return None


def is_pip_environment() -> bool:
    """Check if running as a pip-installed package."""
    # 빌드 마커 우선
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "pip"
    except ImportError:
        pass
    # 스탠드얼론 빌드에 번들된 pip은 무시
    if _is_standalone_build():
        return False
    # 폴백: 기존 pip 확인 로직
    try:
        import pip  # noqa: F401
        return True
    except ImportError:
        pass

    try:
        result = subprocess.run(
            [sys.executable, '-m', 'pip', '--version'],
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except Exception:
        return False
