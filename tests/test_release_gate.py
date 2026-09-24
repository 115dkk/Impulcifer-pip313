"""Unit tests for the release gate (.github/scripts/release_gate.py).

The gate decides whether a release pipeline publishes + builds: release-3x.yml
runs it on every master push (3.x, the stable line), publish.yml only when
dispatched by hand (2.x). It auto-bumps PATCH when shippable files changed
since the last release tag. The CI workflows can't be exercised locally, so
these tests pin the *pure* functions (path classification, free-patch search,
the tag/shippable decision tree, the version rewriters) and the workflow
triggers that decide which pipeline runs on its own.
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


def test_rust_workspace_is_not_shippable():
    """3.x Rust crates do not ship in the 2.x wheel/bundles (ADR 0002)."""
    assert release_gate.classify_shippable([
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "features.toml", "unsafe-budget.toml",
        "crates/impulcifer-dsp/src/fft.rs", "apps/impulcifer-app/src/main.rs",
        "apps/impulcifer-app/tauri.conf.json",
    ]) == []


def test_packaging_metadata_is_not_shippable():
    # AUR PKGBUILD 등 배포 채널 메타데이터는 출하물(wheel/standalone)에
    # 포함되지 않으므로 릴리스를 트리거하면 안 된다.
    assert release_gate.classify_shippable([
        "packaging/aur/PKGBUILD.in", "packaging/aur/README.md",
    ]) == []


# ── parse_version / next_free_patch ──────────────────────────────────────────

def test_parse_version():
    assert release_gate.parse_version("2.7.7") == (2, 7, 7)
    assert release_gate.parse_version(" 10.0.123 ") == (10, 0, 123)
    with pytest.raises(ValueError):
        release_gate.parse_version("not-a-version")


def test_next_free_patch_increments_once_when_free():
    assert release_gate.next_free_patch("2.7.7", lambda v: False) == "2.7.8"


def test_next_free_patch_skips_taken_versions():
    published = {"2.7.8", "2.7.9"}
    assert release_gate.next_free_patch("2.7.7", published.__contains__) == "2.7.10"


# ── decide ───────────────────────────────────────────────────────────────────

NEVER = lambda v: False  # noqa: E731 - nothing tagged / published


def tagged(*versions):
    return {*versions}.__contains__


def test_untagged_version_is_released():
    # A manual bump (or a retry after a release that died before its tag).
    assert release_gate.decide("3.0.2", NEVER, [], NEVER) == ("release", "3.0.2")
    assert release_gate.decide("3.1.0", NEVER, ["crates/x/src/lib.rs"], NEVER) == ("release", "3.1.0")


def test_tagged_version_with_nothing_shippable_does_nothing():
    assert release_gate.decide("3.0.1", tagged("3.0.1"), [], NEVER) == ("none", "3.0.1")


def test_tagged_version_with_shippable_changes_autobumps_patch():
    assert release_gate.decide(
        "3.0.1", tagged("3.0.1"), ["crates/x/src/lib.rs"], tagged("3.0.1")
    ) == ("bump", "3.0.2")


def test_autobump_skips_versions_taken_by_a_tag_or_pypi():
    taken = {"2.14.3", "2.14.4", "2.14.5"}.__contains__
    assert release_gate.decide("2.14.3", tagged("2.14.3"), ["core/hrir.py"], taken) == ("bump", "2.14.6")


def test_prerelease_is_never_autobumped():
    assert release_gate.decide(
        "3.1.0-alpha.1", tagged("3.1.0-alpha.1"), ["crates/x/src/lib.rs"], NEVER
    ) == ("none", "3.1.0-alpha.1")
    assert release_gate.decide("3.1.0-alpha.2", NEVER, [], NEVER) == ("release", "3.1.0-alpha.2")


def test_is_prerelease():
    assert release_gate.is_prerelease("3.0.0-alpha.2")
    assert not release_gate.is_prerelease("3.0.1")


# ── version rewrite helper ───────────────────────────────────────────────────

def test_read_version_from_text_picks_project_version():
    toml = '[build-system]\nrequires = ["hatchling"]\n\n[project]\nversion = "2.7.7"\n'
    assert release_gate.pyproject_version(toml) == "2.7.7"


def test_set_pyproject_version_rewrites_the_first_version_line():
    toml = '[project]\nname = "x"\nversion = "2.14.3"\n'
    assert release_gate.set_pyproject_version(toml, "2.14.4") == '[project]\nname = "x"\nversion = "2.14.4"\n'


# ── 3.x ──────────────────────────────────────────────────────────────────────

def test_3x_build_inputs_are_shippable():
    changed = [
        "crates/impulcifer-dsp/src/fft.rs", "crates/impulcifer-service/locales/ko.json",
        "crates/impulcifer-python/python/impulcifer/__init__.py", "crates/impulcifer-python/Cargo.toml",
        "apps/impulcifer-app/src/main.rs", "apps/impulcifer-app/ui/app.js",
        "apps/impulcifer-app/tauri.conf.json", "apps/impulcifer-app/icons/icon.ico",
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
        "i18n/locales/ko.json", "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
        "data/harman-in-room-headphone-target.csv", "build_scripts/pack_velopack.ps1",
    ]
    assert release_gate.classify_shippable(changed, "3x") == changed


def test_3x_tests_docs_tooling_and_2x_code_are_not_shippable():
    assert release_gate.classify_shippable([
        "crates/impulcifer-service/tests/ui_catalog.rs", "crates/impulcifer-dsp/benches/perf.rs",
        "apps/impulcifer-app/tests/ui_gallery.py", "crates/impulcifer-cli/README.md",
        "apps/impulcifer-app/ui/ipc.d.ts", "apps/impulcifer-app/package.json",
        "apps/impulcifer-app/package-lock.json", "apps/impulcifer-app/tsconfig.json",
        "features.toml", "unsafe-budget.toml", ".github/workflows/release-3x.yml",
        "core/hrir.py", "gui/modern_gui.py", "webview_ui/app.js", "pyproject.toml",
        "data/demo/FL,FR.wav", "build_scripts/nuitka_flags.py", "docs/rust/ARCHITECTURE.md",
        "tests/app_smoke/smoke.py", "CHANGELOG.md",
    ], "3x") == []


CARGO_TOML = """[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "3.0.1"
edition = "2024"

[workspace.dependencies]
serde = { version = "1.0.2" }
"""

CARGO_LOCK = """[[package]]
name = "impulcifer-app"
version = "3.0.1"
dependencies = [
 "json-patch",
]

[[package]]
name = "impulcifer-types"
version = "3.0.1"

[[package]]
name = "json-patch"
version = "3.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"
"""


def test_workspace_version_reads_and_rewrites_only_workspace_package():
    assert release_gate.workspace_version(CARGO_TOML) == "3.0.1"
    bumped = release_gate.set_workspace_version(CARGO_TOML, "3.0.2")
    assert release_gate.workspace_version(bumped) == "3.0.2"
    assert 'serde = { version = "1.0.2" }' in bumped
    assert bumped.replace('version = "3.0.2"', 'version = "3.0.1"', 1) == CARGO_TOML


def test_lock_rewrite_leaves_registry_packages_alone():
    bumped = release_gate.set_lock_workspace_versions(CARGO_LOCK, "3.0.1", "3.0.2")
    assert bumped.count('version = "3.0.2"') == 2
    assert 'name = "json-patch"\nversion = "3.0.1"\nsource' in bumped
    with pytest.raises(ValueError):
        release_gate.set_lock_workspace_versions(CARGO_LOCK, "9.9.9", "9.9.10")


def test_repository_files_are_consistent_and_rewritable():
    """The 3.x bump must find its version in every file it rewrites."""
    root = _GATE_PATH.parents[2]
    version = release_gate.workspace_version((root / "Cargo.toml").read_text(encoding="utf-8"))
    lock = (root / "Cargo.lock").read_text(encoding="utf-8")
    conf = (root / "apps/impulcifer-app/tauri.conf.json").read_text(encoding="utf-8")
    assert '"version": "' + version + '"' in conf
    bumped = release_gate.set_lock_workspace_versions(lock, version, "99.0.0")
    members = [p.parent.name for p in (root / "crates").glob("*/Cargo.toml")] + ["impulcifer-app"]
    for name in members:
        assert f'name = "{name}"\nversion = "99.0.0"' in bumped.replace("\r\n", "\n"), name
    assert '"version": "99.0.0"' in release_gate.set_tauri_version(conf, version, "99.0.0")


def test_changelog_entry_goes_above_the_newest_heading():
    text = "# Changelog\nintro\n\n## 3.0.1 - 2026-09-24\nold\n"
    out = release_gate.prepend_changelog(text, "3.0.2", "2026-09-25", "- feat: x", "3x")
    assert out.index("## 3.0.2 - 2026-09-25") < out.index("## 3.0.1 - 2026-09-24")
    assert "3.x" in out.split("## 3.0.1")[0]


# ── which pipeline runs on its own ───────────────────────────────────────────

def _triggers(workflow: str) -> str:
    text = (_GATE_PATH.parents[1] / "workflows" / workflow).read_text(encoding="utf-8")
    block = text.split("\non:\n", 1)[1]
    return block.split("\nconcurrency:", 1)[0]


def test_3x_releases_on_master_pushes_and_2x_only_by_hand():
    assert "push:" in _triggers("release-3x.yml")
    assert "- master" in _triggers("release-3x.yml")
    assert "tags:" not in _triggers("release-3x.yml")
    assert "push:" not in _triggers("publish.yml")
    assert "workflow_dispatch:" in _triggers("publish.yml")


def test_release_jobs_build_the_gated_commit():
    """Every job after the gate checks out release_sha (the auto-bump commit)."""
    text = (_GATE_PATH.parents[1] / "workflows" / "release-3x.yml").read_text(encoding="utf-8")
    gate, after_gate = text.split("\njobs:\n", 1)[1].split("\n  build-windows:\n", 1)
    assert "release_gate.py --product 3x" in gate
    assert after_gate.count("uses: actions/checkout@v5") == 6
    assert after_gate.count("ref: ${{ needs.gate.outputs.release_sha }}") == 6
    assert after_gate.count("if: needs.gate.outputs.should_release == 'true'") == 7
    assert "GITHUB_SHA" not in after_gate


def test_2x_release_never_takes_releases_latest():
    text = (_GATE_PATH.parents[1] / "workflows" / "publish.yml").read_text(encoding="utf-8")
    assert "make_latest: false" in text
    assert "release_gate.py --product 2x" in text
