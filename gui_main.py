#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Impulcifer GUI 엔트리 포인트 (launcher).

기본값은 WebView 프론트엔드다(2.10+, 바로가기/Velopack 실행 포함).
CustomTkinter는 ``--frontend=ctk`` 또는 설정(~/.impulcifer/settings.json의
``frontend`` 키)으로 계속 사용할 수 있으며, 버전 2 동안 유지보수와 기능
추가를 포함해 완전히 지원된다(버전 3부터는 지금의 레거시 GUI처럼 업데이트
없이 동결 유지 — 제거 아님). WebView 스택을 쓸 수 없는 환경(pywebview
미설치, 시스템 WebKit 부재 등)에서는 자동으로 CustomTkinter로 폴백한다.
"""

import sys

from _impulcifer_entrypoint import prefer_distribution_root


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
       ``--include-module`` was wrongly trimmed this part exits non-zero.

    2. **Pretendard application** — the bundled Pretendard font must be the
       one matplotlib actually applies. The smoke-test masks every system
       Pretendard from matplotlib's ``fontManager`` first, then re-runs
       :func:`core.utils.set_matplotlib_font`. We trust the bundled font
       loader only when the resulting :data:`core.utils.font_setup_result`
       reports ``is_pretendard=True`` AND the resolved path is the bundled
       file — silent fall-through to Malgun / sans-serif is an explicit
       failure here.
    """
    prefer_distribution_root()
    import importlib

    smoke_modules = (
        "gui.modern_gui",
        "gui.tabs.impulcifer_tab",
        "gui.tabs.recorder_tab",
        "gui.tabs.settings_tab",
        "gui.tabs.info_tab",
        "gui.theme",
        "impulcifer",
        "core.hrir",
        "core.impulse_response",
        "core.parallel_workers",
        "core.pipeline",
        "core.cli_builder",
        "core.plotting.hrir_plotter",
        "core.plotting.impulse_response_plotter",
        "core.ffmpeg_discovery",
        "core.audio_truehd",
        "i18n.localization",
        "updater.update_checker",
        "updater.updater_core",
        "infra.logger",
        # WebView frontend stack (default since 2.10). ``webview`` proves
        # pywebview and its backend shims survived the Nuitka trim.
        "webview",
        "application.impulcifer_service",
        "impulcifer_webview",
    )
    for mod in smoke_modules:
        importlib.import_module(mod)

    # WebView UI assets — without index.html the default frontend would
    # silently fall back to CustomTkinter on every launch.
    import os as os_mod

    from infra.resource_helper import get_resource_path

    index_html = get_resource_path("webview_ui/index.html")
    if not os_mod.path.isfile(index_html):
        print(f"smoke-test FAIL: webview_ui/index.html missing from bundle ({index_html})")
        sys.exit(2)

    # Pulse redesign assets — verify the bundle ships logo + CTk theme JSON.
    # If a packaging change drops these, the GUI silently falls back to a
    # generic icon and the default blue theme, which is exactly the
    # regression the redesign was meant to fix.
    from gui.theme import get_ctk_theme_json_path, get_ico_path, get_png_path

    if get_ico_path() is None:
        print("smoke-test FAIL: logo/pulse.ico missing from bundle")
        sys.exit(2)
    if get_png_path(256) is None:
        print("smoke-test FAIL: logo/pulse-256.png missing from bundle")
        sys.exit(2)
    if get_ctk_theme_json_path() is None:
        print("smoke-test FAIL: gui/theme/pulse.json missing from bundle")
        sys.exit(2)

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
    import core.font_setup as core_font_setup  # noqa: E402

    # Reset the one-shot gate so set_matplotlib_font re-runs for this probe.
    # The lazy font state lives in core.font_setup;
    # core.utils.set_matplotlib_font is a re-export of the same function.
    core_font_setup._font_configured = False
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
                "smoke-test FAIL: setup_pretendard_font returned None — "
                "Tk render layer cannot resolve bundled Pretendard."
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
        f"smoke-test OK (imports={len(smoke_modules)}, "
        f"font.matplotlib={result['source']}, "
        f"font.gui={gui_family!r}, font.path={result['path']})"
    )


def _resolve_frontend() -> str:
    """Pick the frontend: CLI flag > persisted setting > webview default."""
    for arg in sys.argv[1:]:
        if arg.startswith("--frontend="):
            value = arg.split("=", 1)[1].strip().lower()
            if value in ("webview", "ctk"):
                return value
            print(f"Unknown --frontend value {value!r}; using 'webview'.")
            return "webview"
    try:
        from i18n.localization import get_localization_manager

        return get_localization_manager().get_frontend()
    except Exception:
        return "webview"


def _record_webview_fallback(stage: str, exc: BaseException) -> str:
    """Persist the WebView→CTk fallback reason and return the user notice.

    Packaged apps have no console, so a bare ``print`` makes the fallback
    invisible — the exact failure mode that already shipped once with the
    Nuitka pywebview-whitelist bug. The reason goes to a file under
    ``~/.impulcifer`` and a localized summary is handed to the CTk GUI for a
    one-time warning dialog.
    """
    import datetime
    import traceback
    from pathlib import Path

    reason = f"{type(exc).__name__}: {exc}"
    log_path = Path.home() / ".impulcifer" / "webview-fallback.log"
    try:
        log_path.parent.mkdir(parents=True, exist_ok=True)
        with open(log_path, "a", encoding="utf-8") as fh:
            fh.write(f"[{datetime.datetime.now():%Y-%m-%d %H:%M:%S}] {stage}: {reason}\n")
            fh.write(traceback.format_exc())
            fh.write("\n")
    except Exception:
        pass

    try:
        from i18n.localization import t

        return t(
            "message_webview_fallback_body",
            default=(
                "The WebView interface could not be started, so the CustomTkinter "
                "interface is shown instead.\n\nReason: {reason}\n\n"
                "A detailed log was written to:\n{log_path}"
            ),
            reason=reason,
            log_path=str(log_path),
        )
    except Exception:
        return f"WebView unavailable: {reason}\nLog: {log_path}"


def _launch_frontend() -> None:
    frontend = _resolve_frontend()
    fallback_notice = None
    if frontend == "webview":
        try:
            import webview  # noqa: F401
            from impulcifer_webview import main as webview_main, select_gui_backend

            select_gui_backend()
        except (SystemExit, Exception) as exc:
            # SystemExit comes from select_gui_backend on unsupported
            # platforms; ImportError from a missing pywebview install.
            print(f"WebView frontend unavailable ({exc}); falling back to CustomTkinter.")
            fallback_notice = _record_webview_fallback("unavailable", exc)
        else:
            try:
                webview_main()
                return
            except Exception as exc:
                # Backend initialization can fail inside webview.start()
                # (missing WebView2 runtime / system WebKit). Fall back so a
                # broken WebView stack never leaves the user with nothing.
                print(f"WebView frontend failed to start ({exc}); falling back to CustomTkinter.")
                fallback_notice = _record_webview_fallback("failed to start", exc)
    from gui.modern_gui import main_gui

    main_gui(startup_notice=fallback_notice)


if __name__ == "__main__":
    _handle_velopack_lifecycle()
    if "--smoke-test" in sys.argv[1:]:
        _smoke_test()
        sys.exit(0)
    prefer_distribution_root()
    _launch_frontend()
