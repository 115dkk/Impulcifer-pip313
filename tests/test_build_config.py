"""Static checks for release build configuration."""

from __future__ import annotations

from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def test_nuitka_build_configs_do_not_use_onefile() -> None:
    """Standalone-folder packaging must not be mixed with Nuitka onefile mode."""
    config_paths = [
        "build_scripts/build_nuitka.py",
        ".github/workflows/build-linux.yml",
        ".github/workflows/build-macos.yml",
        ".github/workflows/release-cross-platform.yml",
    ]

    offenders = [
        path
        for path in config_paths
        if "--onefile" in (PROJECT_ROOT / path).read_text(encoding="utf-8")
    ]

    assert offenders == []
