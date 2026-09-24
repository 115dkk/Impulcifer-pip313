"""Release notes come from CHANGELOG.md (.github/scripts/release_notes.py).

The update dialog of an installed app shows the GitHub Release body as the
release notes. release-3x.yml writes that body from the version's CHANGELOG
section, so these tests pin the section extraction, the fallback, the wiring
in the workflow, and that the current 3.x version has a section to publish.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

_ROOT = Path(__file__).resolve().parent.parent
_SCRIPTS = _ROOT / ".github" / "scripts"


def _load(name: str):
    spec = importlib.util.spec_from_file_location(name, _SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release_notes = _load("release_notes")
release_gate = _load("release_gate")

_CHANGELOG = """# Changelog
intro

## 3.0.10 - 2026-10-01
### ten

## 3.0.1 - 2026-09-24
### 3.x 화면 다듬기

#### ⭐ 화면
- **오류 문장**: `errorSentence()`
  - nested

## 2.14.3 - 2026-09-23
### 2.x

## 3.0.1 - 2026-09-20
#### ⭐ 문서 추가
- docs
"""


def test_section_is_the_version_body_without_its_heading():
    notes = release_notes.changelog_section(_CHANGELOG, "3.0.1")
    assert notes.startswith("### 3.x 화면 다듬기\n\n#### ⭐ 화면\n- **오류 문장**")
    assert "## 3.0.1" not in notes and "## 2.14.3" not in notes and "### 2.x" not in notes


def test_every_block_of_the_version_is_kept_in_file_order():
    notes = release_notes.changelog_section(_CHANGELOG, "3.0.1")
    assert notes.index("  - nested") < notes.index("#### ⭐ 문서 추가")
    assert notes.endswith("- docs")


def test_a_version_prefix_is_not_a_match():
    assert release_notes.changelog_section(_CHANGELOG, "3.0.10") == "### ten"
    assert release_notes.changelog_section(_CHANGELOG, "3.0") is None


def test_missing_section_links_the_tagged_changelog(tmp_path, capsys):
    changelog = tmp_path / "CHANGELOG.md"
    changelog.write_text(_CHANGELOG, encoding="utf-8")
    out = tmp_path / "notes.md"
    assert release_notes.main(["--version", "3.0.0", "--changelog", str(changelog),
                               "--repo", "owner/name", "--tag", "v3.0.0", "--out", str(out)]) == 0
    assert out.read_text(encoding="utf-8") == (
        "Impulcifer 3.0.0. The changes are listed in "
        "https://github.com/owner/name/blob/v3.0.0/CHANGELOG.md\n")
    assert "::warning::" in capsys.readouterr().out


def test_found_section_is_written_as_is(tmp_path):
    changelog = tmp_path / "CHANGELOG.md"
    changelog.write_text(_CHANGELOG, encoding="utf-8")
    out = tmp_path / "notes.md"
    release_notes.main(["--version", "3.0.10", "--changelog", str(changelog), "--out", str(out)])
    assert out.read_text(encoding="utf-8") == "### ten\n"


def test_current_3x_version_has_release_notes():
    """A 3.x version bump without a CHANGELOG section would ship a release
    whose update dialog only links the CHANGELOG."""
    version = release_gate.workspace_version((_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    notes = release_notes.changelog_section((_ROOT / "CHANGELOG.md").read_text(encoding="utf-8"), version)
    assert notes, f"CHANGELOG.md has no '## {version} - <date>' section"


def test_release_body_and_latest_json_use_the_changelog_notes():
    text = (_ROOT / ".github" / "workflows" / "release-3x.yml").read_text(encoding="utf-8")
    release = text.split("\n  create-release:\n", 1)[1]
    assert 'release_notes.py --version "$VERSION"' in release
    assert release.count("--notes-file release-notes.md") == 2  # create, and edit on a rerun
    assert 'pathlib.Path("release-notes.md").read_text' in release
    assert "Details in CHANGELOG.md" not in text and "See CHANGELOG.md" not in text
