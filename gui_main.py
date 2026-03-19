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


if __name__ == "__main__":
    _handle_velopack_lifecycle()
    from gui.modern_gui import main_gui
    main_gui()
