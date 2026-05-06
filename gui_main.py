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
    """Non-interactive verification used in CI / standalone-build sanity checks.

    Two-part guarantee:

    1. **Import chain** — the full GUI tree must be importable. If any
       ``--include-module`` was wrongly trimmed (or a Phase-N rename broke a
       reference) this part exits non-zero.

    2. **Pretendard application** — the bundled Pretendard font must be the
       one matplotlib actually applies. The smoke-test masks every system
       Pretendard from matplotlib's ``fontManager`` first, then re-runs
       :func:`core.utils.set_matplotlib_font`. We trust the bundled font
       loader only when the resulting :data:`core.utils.font_setup_result`
       reports ``is_pretendard=True`` AND the resolved path is the bundled
       file — silent fall-through to Malgun / sans-serif is an explicit
       failure here.
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

    # Pretendard guarantee — simulate an end-user without system-installed
    # Pretendard so the bundled file is the ONLY way to reach the font.
    import matplotlib  # noqa: E402  (matplotlib not part of the Tk path)

    matplotlib.use("Agg")
    import matplotlib.font_manager as fm  # noqa: E402

    fm.fontManager.ttflist = [
        e for e in fm.fontManager.ttflist
        if "pretendard" not in (e.fname or "").lower()
    ]

    import core.utils as core_utils  # noqa: E402

    core_utils._font_configured = False
    result = core_utils.set_matplotlib_font()

    if result.get("source") != "bundled":
        print(
            f"smoke-test FAIL: bundled Pretendard not picked up "
            f"(source={result.get('source')!r}, family={result.get('family')!r}, "
            f"path={result.get('path')!r})"
        )
        sys.exit(2)

    if not result.get("is_pretendard"):
        print(
            f"smoke-test FAIL: matplotlib didn't resolve Pretendard "
            f"(family={result.get('family')!r}, path={result.get('path')!r})"
        )
        sys.exit(2)

    bundled_path = result.get("path")
    if bundled_path is None or "pretendard" not in str(bundled_path).lower():
        print(
            f"smoke-test FAIL: resolved font is not Pretendard "
            f"(path={bundled_path!r})"
        )
        sys.exit(2)

    # Tk / CTkFont render-layer guarantee — this is the path the GUI
    # actually uses. Open a hidden Tk root, run setup_pretendard_font, and
    # confirm Tk's render path resolves "Pretendard" (not Malgun / 명조).
    import tkinter as tk_mod  # noqa: E402
    import tkinter.font as tkfont_mod  # noqa: E402

    tk_root = tk_mod.Tk()
    tk_root.withdraw()
    try:
        from gui.utils import setup_pretendard_font  # noqa: E402

        gui_family = setup_pretendard_font("ko")
        if not gui_family:
            print(
                f"smoke-test FAIL: setup_pretendard_font returned None — "
                f"Tk render layer cannot resolve bundled Pretendard."
            )
            sys.exit(2)
        actual = tkfont_mod.Font(family=gui_family, size=12).actual("family")
        if not actual or actual.casefold() != gui_family.casefold():
            print(
                f"smoke-test FAIL: Tk renders {gui_family!r} as {actual!r} — "
                f"GUI would fall back to system default."
            )
            sys.exit(2)
    finally:
        tk_root.destroy()

    print(
        f"smoke-test OK (imports=18, font.matplotlib={result['source']}, "
        f"font.gui={gui_family!r}, font.path={result['path']})"
    )


if __name__ == "__main__":
    _handle_velopack_lifecycle()
    if "--smoke-test" in sys.argv[1:]:
        _smoke_test()
        sys.exit(0)
    from gui.modern_gui import main_gui
    main_gui()
