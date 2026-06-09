# -*- coding: utf-8 -*-
"""Matplotlib font setup (Korean-capable, bundled-Pretendard-first).

Split out of ``core/utils.py`` (audit #115 finding 8). ``core.utils``
re-exports ``set_matplotlib_font`` / ``font_setup_result`` for compatibility.
"""

import os
import platform
from pathlib import Path

import matplotlib.pyplot as plt
import matplotlib.font_manager as fm

plt.rcParams['axes.unicode_minus'] = False

_font_configured = False
# Result of the most recent set_matplotlib_font() call. Public read-only
# diagnostic surface so the smoke-test in gui_main.py can verify Pretendard
# was actually applied (not just silently fell back to a system font).
font_setup_result: dict = {
    "source": None,
    "family": None,
    "path": None,
    "is_pretendard": False,
}


def _resolve_bundled_font_dir() -> "Path | None":
    """Return the bundled ``font/`` directory across all runtime modes.

    Delegates to :func:`infra.resource_helper.resolve_bundled_font_dir`
    (resource-path first), passing repo-root ``font``/``fonts`` fallbacks. The
    local import keeps a graceful degradation path for the rare case ``infra``
    itself can't be imported (e.g. ad-hoc scripts).
    """
    project_root = Path(__file__).parent.parent
    extras = (project_root / "font", project_root / "fonts")
    try:
        from infra.resource_helper import resolve_bundled_font_dir

        return resolve_bundled_font_dir(extras)
    except Exception:
        for legacy in extras:
            if legacy.is_dir():
                return legacy
        return None


def _scan_bundled_fonts() -> "list[Path]":
    """List every ``.otf`` / ``.ttf`` / ``.ttc`` bundled in the ``font/`` dir.

    Thin wrapper over the shared :func:`infra.resource_helper.scan_bundled_fonts`
    (case-insensitive name order, so matplotlib ``findfont`` scoring stays
    deterministic across trees).
    """
    project_root = Path(__file__).parent.parent
    extras = (project_root / "font", project_root / "fonts")
    try:
        from infra.resource_helper import scan_bundled_fonts

        return scan_bundled_fonts(extras)
    except Exception:
        return []


def _resolve_bundled_pretendard_path() -> "Path | None":
    """Find the bundled Pretendard font (legacy compat name).

    Kept as a thin wrapper over :func:`_scan_bundled_fonts` so existing tests
    and old callers keep working. Priority: PretendardVariable.ttf →
    Pretendard-Regular.* → any Pretendard-shaped file.
    """
    fonts = _scan_bundled_fonts()
    for path in fonts:
        stem = path.stem.lower()
        if "pretendard" in stem and "variable" in stem:
            return path
    for path in fonts:
        if "pretendard-regular" in path.stem.lower():
            return path
    for path in fonts:
        if "pretendard" in path.stem.lower():
            return path
    return None


def _register_bundled_fonts_with_matplotlib() -> "list[Path]":
    """addfont() every bundled font for matplotlib and return the registered list.

    matplotlib's ``fontManager.addfont`` is idempotent (it deduplicates by
    file path), so this is safe to call multiple times. Used by
    :func:`set_matplotlib_font` so that BOTH Pretendard AND any extra font
    the user drops into ``font/`` are available to matplotlib code that may
    want to reference them by family name.
    """
    registered: list[Path] = []
    for path in _scan_bundled_fonts():
        try:
            fm.fontManager.addfont(str(path))
            registered.append(path)
        except Exception:
            continue
    return registered


def set_matplotlib_font():
    """한글을 지원하는 폰트를 matplotlib에 설정한다.

    번들 Pretendard 우선 → 시스템 Pretendard → OS별 한글 폴백 순으로
    시도하며, 한 번만 실행되고 이후 호출은 캐시된 결과를 반환한다.

    이전 구현은 silent fallback이라 "Pretendard 적용에 실패해 Malgun으로
    떨어졌다"를 추적할 방법이 없었다. 이번 리팩토링은:

    1. 번들 경로 해석을 ``infra.resource_helper.get_font_path`` 로 일원화해
       Nuitka standalone 환경에서도 같은 경로 규칙을 따른다.
    2. 어떤 source가 채택됐는지(``bundled`` / ``system`` / ``fallback``)와
       findfont가 실제로 어떤 파일을 골랐는지를 ``font_setup_result`` 모듈
       전역에 기록한다 — smoke-test가 이를 보고 "Pretendard 보장" 검증을
       수행할 수 있다.

    Returns:
        ``font_setup_result`` 의 사본. ``is_pretendard`` 가 ``True`` 일 때
        만 호출자는 Pretendard 적용이 보장되었다고 간주해야 한다.
    """
    global _font_configured
    if _font_configured:
        return dict(font_setup_result)

    _font_configured = True

    system = platform.system()

    # Register EVERY bundled font (Pretendard + any user-dropped extras).
    # matplotlib only renders text in the family set on rcParams, but
    # registering the others makes them addressable when code explicitly
    # opts-in via FontProperties(family=...).
    registered = _register_bundled_fonts_with_matplotlib()
    bundled_pretendard = next(
        (p for p in registered if "pretendard" in p.stem.lower()),
        None,
    )

    chosen_source = None
    chosen_family = None

    # 1) 번들 Pretendard (the default sans-serif body font)
    if bundled_pretendard is not None:
        try:
            prop = fm.FontProperties(fname=str(bundled_pretendard))
            family = prop.get_name()
            plt.rcParams["font.family"] = family
            chosen_source = "bundled"
            chosen_family = family
        except Exception:
            chosen_source = None  # fall through to system

    # 2) 시스템 설치 Pretendard
    if chosen_source is None:
        try:
            if any(f.name == "Pretendard" for f in fm.fontManager.ttflist):
                plt.rcParams["font.family"] = "Pretendard"
                chosen_source = "system"
                chosen_family = "Pretendard"
        except Exception:
            pass

    # 3) OS 한글 폴백
    if chosen_source is None:
        chosen_source = "fallback"
        if system == "Windows":
            win_font = "C:/Windows/Fonts/malgun.ttf"
            if os.path.exists(win_font):
                prop = fm.FontProperties(fname=win_font)
                family = prop.get_name()
                plt.rcParams["font.family"] = family
                chosen_family = family
            else:
                plt.rcParams["font.family"] = "Malgun Gothic"
                chosen_family = "Malgun Gothic"
        elif system == "Darwin":
            plt.rcParams["font.family"] = "AppleGothic"
            chosen_family = "AppleGothic"
        elif system == "Linux":
            try:
                if any(f.name == "NanumGothic" for f in fm.fontManager.ttflist):
                    plt.rcParams["font.family"] = "NanumGothic"
                    chosen_family = "NanumGothic"
            except Exception:
                pass

    # 4) findfont로 실제 결과 검증
    resolved_path = None
    is_pretendard = False
    try:
        resolved = fm.findfont(
            fm.FontProperties(family=chosen_family or "Pretendard"),
            fallback_to_default=True,
        )
        if resolved:
            resolved_path = Path(resolved)
            is_pretendard = "pretendard" in resolved_path.name.lower()
    except Exception:
        pass

    font_setup_result.update(
        {
            "source": chosen_source,
            "family": chosen_family,
            "path": resolved_path,
            "is_pretendard": is_pretendard,
        }
    )
    return dict(font_setup_result)
