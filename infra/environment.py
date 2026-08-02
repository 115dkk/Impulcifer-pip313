# -*- coding: utf-8 -*-
"""Runtime environment detection — the single owner (audit #138 C3/F012).

Three questions that used to be answered independently across ~10 files:

- :func:`is_standalone_build` — Nuitka standalone 빌드인가? 빌드 마커
  (:mod:`infra._build_info`) 우선, 마커 없는 구 빌드는 세 레거시 프로브의
  합집합(``sys.frozen`` / ``'__nuitka__' in sys.modules`` / 모듈 전역
  ``__compiled__``)으로 판정한다.
- :func:`get_install_kind` — 설치 종류: ``"velopack"`` / ``"pip"`` / ``"dev"``.
- :func:`normalized_platform` — 플랫폼 정본 어휘: ``"windows"`` /
  ``"darwin"`` / ``"linux"`` (application service의 JSON ``platform`` 필드와
  동일). 새 코드는 이 어휘를 쓸 것; ``updater/velopack.py``의
  ``win/osx/linux`` 채널 태그는 그 위의 로컬 어댑터다.

``updater/environment.py``는 이 모듈의 하위 호환 re-export 셸이다.
"""

import platform
import sys
from pathlib import Path
from typing import Optional


def is_standalone_build() -> bool:
    """빌드 마커 기반 스탠드얼론(Nuitka) 빌드 감지."""
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "standalone"
    except ImportError:
        pass
    # 폴백: 마커 없는 구 빌드 호환 — 과거 프로브들의 합집합.
    if getattr(sys, 'frozen', False):
        return True
    if '__nuitka__' in sys.modules:
        return True
    if '__compiled__' in globals():
        return True
    return False


def get_velopack_update_exe() -> Optional[Path]:
    """Get path to Velopack's Update.exe if available."""
    if not is_standalone_build():
        return None

    app_dir = Path(sys.executable).parent
    # Velopack 구조: {packId}/current/app.exe, {packId}/Update.exe
    update_exe = app_dir.parent / "Update.exe"

    if update_exe.exists():
        return update_exe
    return None


def is_velopack_environment() -> bool:
    """
    Check if running in a Velopack-installed environment.
    Velopack creates Update.exe in the app's parent directory.
    """
    return get_velopack_update_exe() is not None


def is_pip_environment() -> bool:
    """Check if running as a pip-installed package."""
    # 빌드 마커 우선
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "pip"
    except ImportError:
        pass
    # 스탠드얼론 빌드에 번들된 pip은 무시
    if is_standalone_build():
        return False
    # 폴백: 기존 pip 확인 로직
    try:
        import pip  # noqa: F401
        return True
    except ImportError:
        pass

    try:
        import subprocess
        result = subprocess.run(
            [sys.executable, '-m', 'pip', '--version'],
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except Exception:
        return False


def get_install_kind() -> str:
    """Classify the install: ``"velopack"`` / ``"pip"`` / ``"dev"``."""
    if is_velopack_environment():
        return "velopack"
    if is_pip_environment():
        return "pip"
    return "dev"


def normalized_platform() -> str:
    """플랫폼 정본 어휘 — ``"windows"`` / ``"darwin"`` / ``"linux"``."""
    return platform.system().lower()
