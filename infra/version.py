#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Lightweight application version resolution.

Extracted from ``impulcifer.py`` so frontends (notably the WebView
bootstrap) can report the version without importing the whole DSP stack —
``from impulcifer import __version__`` costs seconds of scipy/matplotlib/
bokeh imports and was the dominant share of the WebView's first-load
delay. Import order: build marker → pyproject.toml → package metadata.
"""

from __future__ import annotations

_FALLBACK_VERSION = "2.5.0"


def get_app_version() -> str:
    """Get version from build marker, pyproject.toml, or package metadata."""
    # Method 0: 빌드 마커 (Nuitka/pip 빌드에서 가장 확실)
    try:
        from infra._build_info import VERSION as build_version

        if build_version is not None:
            return build_version
    except ImportError:
        pass

    # Method 1: pyproject.toml (개발 환경)
    try:
        import tomllib
    except ImportError:
        try:
            import tomli as tomllib
        except ImportError:
            tomllib = None

    if tomllib:
        try:
            from pathlib import Path

            repo_root = Path(__file__).parent.parent
            possible_paths = [
                repo_root / "pyproject.toml",
                repo_root.parent / "pyproject.toml",
            ]
            for pyproject_path in possible_paths:
                if pyproject_path.exists():
                    with open(pyproject_path, "rb") as f:
                        data = tomllib.load(f)
                        version_str = data.get("project", {}).get("version")
                        if version_str:
                            return version_str
        except Exception:
            pass

    # Method 2: Package metadata (pip 설치, 마커 없는 경우)
    try:
        from importlib.metadata import version as get_version

        return get_version("impulcifer-py313")
    except Exception:
        pass

    return _FALLBACK_VERSION
