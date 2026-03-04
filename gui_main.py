#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Impulcifer Modern GUI 엔트리 포인트"""

import sys


def _handle_velopack_lifecycle():
    """Velopack 설치/업데이트/제거 훅 처리.

    Velopack은 설치/업데이트/제거 시 실행 파일에 특수 인자를 전달합니다.
    이 인자가 감지되면 GUI를 실행하지 않고 즉시 종료합니다.
    """
    velopack_args = {
        '--velopack-install',
        '--velopack-updated',
        '--velopack-obsolete',
        '--velopack-uninstall',
    }
    for arg in sys.argv[1:]:
        if arg in velopack_args:
            if arg == '--velopack-uninstall':
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


if __name__ == "__main__":
    _handle_velopack_lifecycle()
    from gui.modern_gui import main_gui
    main_gui()
