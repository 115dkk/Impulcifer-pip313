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
