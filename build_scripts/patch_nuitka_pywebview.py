"""Nuitka pywebview 플러그인 화이트리스트 핫픽스.

Nuitka(4.1.x, 2026-07 기준 upstream main 포함)의 내장 pywebview 플러그인은
Windows에서 ``webview.platforms.{winforms,edgechromium,edgehtml,mshtml,cef}``
만 포함하는 화이트리스트를 갖는데, pywebview 6.x의 winforms 백엔드는 헬퍼
모듈 ``webview.platforms.win32``를 import한다. 화이트리스트에 이 항목이
없으면 해당 모듈이 "actively excluded"로 번들에서 빠지고, 패키징된 앱이
기동 시 ImportError를 맞아 CustomTkinter로 폴백한다(로컬 Windows 빌드에서
관찰: "Module 'webview.platforms.win32' was actively excluded from Nuitka
compilation").

``--include-module=webview.platforms.win32``로는 해결할 수 없다 — 명시적
include는 Nuitka의 follow 패턴에 들어가 플러그인의 제외 결정과 충돌해
"Conflict between user and plugin decision" FATAL로 죽는다. 업스트림에
항목이 추가될 때까지, 빌드 직전에 설치된 플러그인 파일의 화이트리스트에
누락 모듈을 삽입한다. 이미 패치되었거나 업스트림이 고쳐진 경우 no-op.

build_scripts/build_nuitka.py(릴리스 CI)와 build_scripts/build_local.py가
Nuitka 호출 전에 이 모듈의 :func:`patch_pywebview_plugin`을 실행한다.
"""

from __future__ import annotations

from pathlib import Path

_ANCHOR = '"webview.platforms.winforms",'
_MISSING_ENTRY = '"webview.platforms.win32",'


def patch_source(text: str) -> tuple[str, str]:
    """Return ``(patched_text, status)`` for the plugin source.

    status is one of ``"patched"``, ``"already-ok"``, ``"anchor-missing"``.
    """
    if _MISSING_ENTRY in text:
        return text, "already-ok"
    if _ANCHOR not in text:
        return text, "anchor-missing"
    indent = text.split(_ANCHOR)[0].rsplit("\n", 1)[-1]
    patched = text.replace(_ANCHOR, _ANCHOR + "\n" + indent + _MISSING_ENTRY, 1)
    return patched, "patched"


def patch_pywebview_plugin() -> bool:
    """Patch the installed Nuitka plugin in place. Returns success."""
    try:
        import nuitka.plugins.standard as standard_plugins
    except ImportError:
        print("patch_nuitka_pywebview: nuitka is not installed; nothing to patch")
        return False

    plugin_path = Path(standard_plugins.__file__).parent / "PywebViewPlugin.py"
    if not plugin_path.is_file():
        print(f"patch_nuitka_pywebview: plugin file not found at {plugin_path}")
        return False

    text = plugin_path.read_text(encoding="utf-8")
    patched, status = patch_source(text)
    if status == "already-ok":
        print("patch_nuitka_pywebview: webview.platforms.win32 already whitelisted")
        return True
    if status == "anchor-missing":
        print(
            "patch_nuitka_pywebview: WARNING — plugin layout changed, anchor "
            f"{_ANCHOR!r} not found in {plugin_path}; skipping patch. If the "
            "packaged app falls back to CustomTkinter on Windows, check "
            "whether webview.platforms.win32 is still being excluded."
        )
        return False

    plugin_path.write_text(patched, encoding="utf-8")
    print(f"patch_nuitka_pywebview: whitelisted webview.platforms.win32 in {plugin_path}")
    return True


if __name__ == "__main__":
    raise SystemExit(0 if patch_pywebview_plugin() else 1)
