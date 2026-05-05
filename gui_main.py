#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Impulcifer Modern GUI 엔트리 포인트"""

import sys


def _handle_velopack_lifecycle():
    """Velopack 설치/업데이트/제거 훅 처리.

    Velopack 공식 문서: https://docs.velopack.io/integrating/hooks
    인자 형식: --veloapp-install, --veloapp-updated, --veloapp-obsolete, --veloapp-uninstall
    각 인자 뒤에 버전 문자열이 따라옴 (예: --veloapp-install 2.4.9)
    타임아웃: install/uninstall 30초, updated/obsolete 15초
    """
    for arg in sys.argv[1:]:
        if arg.startswith('--veloapp-'):
            if arg.startswith('--veloapp-uninstall'):
                _cleanup_on_uninstall()
            sys.exit(0)


def _cleanup_on_uninstall():
    """제거 시 사용자 설정 파일 정리."""
    try:
        import os
        import shutil
        from pathlib import Path

        local_app_data = os.environ.get('LOCALAPPDATA', '')
        if local_app_data:
            config_dir = Path(local_app_data) / 'Impulcifer' / 'config'
            if config_dir.exists():
                shutil.rmtree(config_dir, ignore_errors=True)
    except Exception:
        pass


def _smoke_test():
    """Non-interactive import-chain verification for Nuitka standalone builds.

    Imports the full GUI tree without opening a Tk window. Used in CI to
    confirm the bundled binary has all transitive modules available — if any
    ``--include-module`` was wrongly trimmed, this exits non-zero.
    """
    import importlib

    for mod in (
        "gui.modern_gui",
        "gui.tabs.impulcifer_tab",
        "gui.tabs.recorder_tab",
        "gui.tabs.settings_tab",
        "gui.tabs.info_tab",
        "impulcifer",
        "core.hrir",
        "core.impulse_response",
        "core.parallel_workers",
        "core.pipeline",
        "core.cli_builder",
        "core.plotting.hrir_plotter",
        "core.plotting.impulse_response_plotter",
        "core.ffmpeg_utils",
        "i18n.localization",
        "updater.update_checker",
        "updater.updater_core",
        "infra.logger",
    ):
        importlib.import_module(mod)
    print("smoke-test OK")


if __name__ == "__main__":
    _handle_velopack_lifecycle()
    if "--smoke-test" in sys.argv[1:]:
        _smoke_test()
        sys.exit(0)
    from gui.modern_gui import main_gui
    main_gui()
