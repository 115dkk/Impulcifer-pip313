"""Unit tests for the release gate decision logic (.github/scripts/release_gate.py).

The gate decides whether a push to master publishes + builds a release, and
auto-bumps PATCH when a shippable change merged without a manual bump. The CI
workflow can't be exercised locally, so these tests pin the *pure* decision
functions (the part where subtle bugs hide: path classification, free-patch
search, and the event/manual-bump/shippable decision tree).
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

# Load the gate module from .github/scripts/ (not an importable package).
_GATE_PATH = Path(__file__).resolve().parent.parent / ".github" / "scripts" / "release_gate.py"
_spec = importlib.util.spec_from_file_location("release_gate", _GATE_PATH)
release_gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(release_gate)


# ── classify_shippable ───────────────────────────────────────────────────────

def test_docs_ci_tests_only_is_not_shippable():
    changed = [
        "README.md", "CLAUDE.md", "AGENTS.md", "docs/BUILD_README.md",
        ".github/workflows/publish.yml", ".github/scripts/release_gate.py",
        "tests/test_release_gate.py", "CHANGELOG.md", "research/scratch.py",
        ".gitignore", ".claude/settings.json", "LICENSE",
    ]
    assert release_gate.classify_shippable(changed) == []


def test_runtime_and_asset_changes_are_shippable():
    changed = [
        "core/hrir.py", "autoeq/frequency_response.py", "impulcifer.py",
        "gui/modern_gui.py", "i18n/locales/en.json", "infra/logger.py",
        "updater/update_checker.py", "data/demo/headphones.wav",
        "pyproject.toml", "requirements.txt", "build_scripts/nuitka_flags.py",
        "logo/pulse.ico", "font/NotoSansKR.otf",
    ]
    assert set(release_gate.classify_shippable(changed)) == set(changed)


def test_mixed_change_keeps_only_shippable():
    changed = ["docs/x.md", "core/utils.py", "tests/test_x.py", "gui/utils.py"]
    assert sorted(release_gate.classify_shippable(changed)) == ["core/utils.py", "gui/utils.py"]


def test_nested_excluded_dirs_match_at_any_depth():
    assert release_gate.classify_shippable([
        ".github/workflows/sub/deep.yml", "docs/a/b/c.png", "tests/sub/helper.py",
    ]) == []


# ── parse_version / next_free_patch ──────────────────────────────────────────

def test_parse_version():
    assert release_gate.parse_version("2.7.7") == (2, 7, 7)
    assert release_gate.parse_version(" 10.0.123 ") == (10, 0, 123)
    with pytest.raises(ValueError):
        release_gate.parse_version("not-a-version")


def test_next_free_patch_increments_once_when_free():
    assert release_gate.next_free_patch("2.7.7", lambda v: False) == "2.7.8"


def test_next_free_patch_skips_published_versions():
    published = {"2.7.8", "2.7.9"}
    assert release_gate.next_free_patch("2.7.7", published.__contains__) == "2.7.10"


# ── decide ───────────────────────────────────────────────────────────────────

NEVER = lambda v: False  # noqa: E731 - nothing published


def test_dispatch_releases_current_when_not_published():
    assert release_gate.decide("workflow_dispatch", "2.7.7", "2.7.7", [], NEVER) == ("release", "2.7.7")


def test_dispatch_noop_when_current_already_published():
    assert release_gate.decide("workflow_dispatch", "2.7.7", "2.7.7", [], {"2.7.7"}.__contains__) == ("none", "2.7.7")


def test_manual_bump_is_respected():
    # version changed (2.7.7 -> 2.8.0, a MINOR done by hand) -> release as-is.
    assert release_gate.decide("push", "2.7.7", "2.8.0", ["core/x.py"], NEVER) == ("release", "2.8.0")


def test_manual_bump_to_already_published_is_noop():
    assert release_gate.decide("push", "2.7.7", "2.8.0", ["core/x.py"], {"2.8.0"}.__contains__) == ("none", "2.8.0")


def test_shippable_change_without_bump_autobumps_patch():
    assert release_gate.decide("push", "2.7.7", "2.7.7", ["core/hrir.py"], NEVER) == ("bump", "2.7.8")


def test_shippable_autobump_skips_published_patch():
    assert release_gate.decide("push", "2.7.7", "2.7.7", ["core/hrir.py"], {"2.7.8"}.__contains__) == ("bump", "2.7.9")


def test_docs_only_without_bump_does_nothing():
    assert release_gate.decide("push", "2.7.7", "2.7.7", [], NEVER) == ("none", "2.7.7")


# ── version rewrite helper ───────────────────────────────────────────────────

def test_read_version_from_text_picks_project_version():
    toml = '[build-system]\nrequires = ["hatchling"]\n\n[project]\nversion = "2.7.7"\n'
    assert release_gate._read_version_from_text(toml) == "2.7.7"
