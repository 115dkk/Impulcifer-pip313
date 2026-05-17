"""Static checks for release build configuration."""

from __future__ import annotations

import re
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


def test_nuitka_build_workflows_use_python_314_and_nuitka_41() -> None:
    """Standalone release builders target regular CPython 3.14 with Nuitka 4.1+."""
    direct_workflows = [
        ".github/workflows/build-linux.yml",
        ".github/workflows/build-macos.yml",
    ]
    for path in direct_workflows:
        text = (PROJECT_ROOT / path).read_text(encoding="utf-8")
        assert "python-version: '3.14'" in text, path
        assert '"nuitka>=4.1"' in text, path

    release_text = (PROJECT_ROOT / ".github/workflows/release-cross-platform.yml").read_text(encoding="utf-8")
    assert release_text.count("python-version: '3.14'") >= 3
    assert release_text.count('"nuitka>=4.1"') >= 3


def test_nuitka_build_workflows_do_not_request_free_threaded_python() -> None:
    """Nuitka builds stay on the regular GIL-enabled 3.14 runtime."""
    config_paths = [
        ".github/workflows/build-linux.yml",
        ".github/workflows/build-macos.yml",
        ".github/workflows/release-cross-platform.yml",
        "build_scripts/build_nuitka.py",
        "build_scripts/nuitka_flags.py",
    ]
    forbidden = ("--disable-gil", "3.14t", "free-threaded")
    offenders = []
    for path in config_paths:
        text = (PROJECT_ROOT / path).read_text(encoding="utf-8").lower()
        if any(marker in text for marker in forbidden):
            offenders.append(path)

    assert offenders == []


def test_release_linux_build_uses_canonical_nuitka_script() -> None:
    """The release Linux job should not drift from build_scripts/nuitka_flags.py."""
    text = (PROJECT_ROOT / ".github/workflows/release-cross-platform.yml").read_text(encoding="utf-8")
    assert "python build_scripts/build_nuitka.py" in text
    assert "python -m nuitka \\" not in text


def test_source_archives_exclude_local_verification_worktrees() -> None:
    """Large local verification and worktree folders must stay out of releases."""
    gitignore = (PROJECT_ROOT / ".gitignore").read_text(encoding="utf-8")
    pyproject = (PROJECT_ROOT / "pyproject.toml").read_text(encoding="utf-8")

    for marker in ("_verification/", ".worktrees/", ".claude/worktrees/"):
        assert marker in gitignore

    for marker in (
        '"_verification/**/*"',
        '".worktrees/**/*"',
        '".claude/worktrees/**/*"',
    ):
        assert marker in pyproject


def test_changelog_versions_are_unique_and_date_ordered() -> None:
    """Release history headings should be unambiguous and newest first."""
    text = (PROJECT_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    headings = re.findall(r"^## (\d+\.\d+\.\d+) - (\d{4}-\d{2}-\d{2})", text, re.MULTILINE)

    versions = [version for version, _ in headings]
    duplicate_versions = sorted({version for version in versions if versions.count(version) > 1})
    assert duplicate_versions == []

    dates = [date for _, date in headings]
    assert dates == sorted(dates, reverse=True)
