"""Pulse design tokens and asset loaders.

Centralizes colors, typography weights, and logo image loading for the
modern GUI. The design comes from ``Impulcifer Redesign.html`` (handed
off via Claude Design); the tokens here are the sRGB conversions of the
``oklch(L c 240)`` values defined in ``styles/tokens.css`` of that bundle.
"""
from __future__ import annotations

import sys
from pathlib import Path
from typing import Optional

# ---------------------------------------------------------------------------
# Color tokens — sRGB hex equivalents of oklch(L c 240) from tokens.css.
# Tuples are ``(light, dark)`` matching the CTk theme JSON convention so
# any callers building widgets directly can use ``COLORS["bg-1"][1]`` for
# the dark-mode value without re-deriving it.
# ---------------------------------------------------------------------------
COLORS: dict[str, tuple[str, str]] = {
    "bg-0":     ("#f3f5f7", "#101214"),
    "bg-1":     ("#e6e8eb", "#181b1d"),
    "bg-2":     ("#dde0e3", "#23272a"),
    "bg-3":     ("#cdd0d3", "#2b2e31"),
    "bg-4":     ("#b5b8ba", "#2f3337"),
    "fg-0":     ("#15171b", "#f3f5f7"),
    "fg-1":     ("#3a3d42", "#b5b8ba"),
    "fg-2":     ("#787b7d", "#787b7d"),
    "fg-3":     ("#9aa0a6", "#4b4d4f"),
    "line":     ("#cdd0d3", "#2b2e31"),
    "line-soft":("#dde0e3", "#1d2022"),
    "accent":         ("#3B82F6", "#3B82F6"),
    "accent-strong":  ("#2563EB", "#2563EB"),
    "accent-soft":    ("#dbeafe", "#1e3a5f"),
    "ok":   ("#16a34a", "#4ac776"),
    "warn": ("#d97706", "#e6ac3d"),
    "err":  ("#dc2626", "#fb594d"),
}


def color(token: str) -> tuple[str, str]:
    """Return ``(light, dark)`` hex pair for a token."""
    return COLORS[token]


# ---------------------------------------------------------------------------
# Asset paths
# ---------------------------------------------------------------------------
def _resolve_logo_dir() -> Optional[Path]:
    """Return the ``logo/`` directory across runtime modes (dev / pip / standalone)."""
    candidates: list[Path] = []
    try:
        from infra.resource_helper import get_resource_path

        candidates.append(Path(get_resource_path("logo")))
    except Exception:
        pass
    here = Path(__file__).resolve()
    candidates.extend([
        here.parent.parent.parent / "logo",
        Path(sys.executable).parent / "logo",
    ])
    for c in candidates:
        if c.is_dir():
            return c
    return None


def get_logo_path(name: str) -> Optional[Path]:
    """Return absolute path to a bundled logo file, or ``None`` if missing."""
    logo_dir = _resolve_logo_dir()
    if logo_dir is None:
        return None
    candidate = logo_dir / name
    return candidate if candidate.is_file() else None


def get_ico_path() -> Optional[Path]:
    """Return path to the multi-resolution Windows ``.ico`` icon."""
    return get_logo_path("pulse.ico")


def get_png_path(size: int) -> Optional[Path]:
    """Return path to the PNG at the given size (16/24/32/48/64/128/256)."""
    return get_logo_path(f"pulse-{size}.png")


# ---------------------------------------------------------------------------
# CTk theme JSON path
# ---------------------------------------------------------------------------
# ---------------------------------------------------------------------------
# Monospace font fallback chain
# ---------------------------------------------------------------------------
_mono_family_cache: Optional[str] = None


def get_mono_font_family() -> str:
    """Return the best available monospace family Tk can actually render.

    Why this exists. The redesign tokens spell out ``JetBrains Mono`` as
    the mono font, but it is NOT bundled with Windows / macOS / Linux —
    on a default Korean Windows install Tk's ``family="JetBrains Mono"``
    request misses, and the render layer falls through to the system
    default which is **Gulim (굴림)** in CP949 locales. That's the same
    fake-bold-Hangul regression we already fought once for the proportional
    font; it manifests for any user input that the redesign labelled as
    "mono" (file paths, numeric pills, version strings).

    This helper probes Tk's render layer (``tkfont.Font().actual()``) for
    a chain of cross-platform candidates and returns the first one that
    actually resolves. Result is cached at module level so widget builds
    don't re-probe.

    Priority order (best legibility first):

    1. ``JetBrains Mono``      — design token, only if user has it installed
    2. ``Cascadia Code``        — Windows 11 default + Office bundle
    3. ``Cascadia Mono``        — same family without ligatures
    4. ``Consolas``             — every Windows since Vista
    5. ``Menlo``                — macOS default
    6. ``DejaVu Sans Mono``     — Linux default
    7. ``Courier New``          — universal Tk fallback
    """
    global _mono_family_cache
    if _mono_family_cache is not None:
        return _mono_family_cache

    try:
        from tkinter import font as tkfont
    except Exception:
        # No Tk yet — return a safe default WITHOUT caching so a later
        # call (after ctk.CTk() exists) gets a real probe.
        return "Courier New"

    candidates = (
        "JetBrains Mono",
        "Cascadia Code",
        "Cascadia Mono",
        "Consolas",
        "Menlo",
        "DejaVu Sans Mono",
        "Courier New",
    )
    found_any_real_match = False
    for name in candidates:
        try:
            actual = tkfont.Font(family=name, size=10).actual("family")
        except Exception:
            # No default Tk root yet — Tk needs one to construct fonts.
            # Return a safe default but DON'T cache it; next call (after
            # the GUI builds its root) will re-probe and pick the real best.
            return "Courier New"
        if actual and actual.casefold() == name.casefold():
            _mono_family_cache = name
            return name
        if actual:
            found_any_real_match = True

    # We probed every candidate and none matched verbatim, but Tk did
    # respond to font queries — cache the safest universal fallback.
    if found_any_real_match:
        _mono_family_cache = "Courier New"
    return "Courier New"


def get_ctk_theme_json_path() -> Optional[Path]:
    """Return absolute path to the bundled CTk theme JSON, or None when absent."""
    candidates: list[Path] = []
    try:
        from infra.resource_helper import get_resource_path

        candidates.append(Path(get_resource_path("gui/theme/pulse.json")))
    except Exception:
        pass
    here = Path(__file__).resolve()
    candidates.extend([
        here.parent / "pulse.json",
        Path(sys.executable).parent / "gui" / "theme" / "pulse.json",
    ])
    for c in candidates:
        if c.is_file():
            return c
    return None
