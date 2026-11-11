"""Tests for .github/scripts/gallery_publish.py (UI gallery retention)."""

from __future__ import annotations

import importlib.util
import shutil
import subprocess
from pathlib import Path

import pytest

_SCRIPT = Path(__file__).resolve().parent.parent / ".github" / "scripts" / "gallery_publish.py"
_spec = importlib.util.spec_from_file_location("gallery_publish", _SCRIPT)
gallery_publish = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gallery_publish)


def test_run_dir_names_round_trip():
    assert gallery_publish.run_dir_name(58, 299) == "58-pr299"
    assert gallery_publish.run_dir_name(58, None) == "58-manual"
    assert gallery_publish.parse_run_dir("58-pr299") == (58, 299)
    assert gallery_publish.parse_run_dir("58-manual") == (58, None)
    assert gallery_publish.parse_run_dir("57") == (57, None)
    assert gallery_publish.parse_run_dir("README.md") is None


def test_retained_dirs_keep_current_and_newest_run_of_other_open_prs():
    existing = [
        "40",  # before this scheme
        "41-manual",
        "50-pr10",
        "52-pr10",
        "51-pr11",  # PR 11 is closed
        "53-pr12",
        "54-pr12",
        "55-pr13",  # the current PR's previous run
    ]
    kept = gallery_publish.retained_dirs(existing, {10, 12, 13}, "56-pr13")
    assert kept == ["52-pr10", "54-pr12", "56-pr13"]


def test_retained_dirs_after_merge_drop_the_merged_gallery_only_on_the_next_run():
    # The merged PR's gallery is the latest one until another gallery runs.
    assert gallery_publish.retained_dirs(["57-pr296"], set(), "60-pr300") == ["60-pr300"]
    assert gallery_publish.retained_dirs(["57-pr296"], set(), "61-manual") == ["61-manual"]


def test_retained_dirs_rerun_of_the_same_run_number_replaces_itself():
    assert gallery_publish.retained_dirs(["58-pr299"], {299}, "58-pr299") == ["58-pr299"]


def test_retained_dirs_keep_number_only_runs_of_open_prs_found_by_branch():
    existing = ["57", "58", "61", "59-pr302"]
    legacy = {58: 300, 61: 305}  # run 57 belonged to a merged PR: no entry
    assert gallery_publish.legacy_runs(existing) == [57, 58, 61]
    kept = gallery_publish.retained_dirs(existing, {300, 302, 305}, "62-pr302", legacy)
    assert kept == ["58", "61", "62-pr302"]
    # A newer new-style run of the same PR replaces its number-only run.
    kept = gallery_publish.retained_dirs(["61", "63-pr305"], {305}, "64-pr302", {61: 305})
    assert kept == ["63-pr305", "64-pr302"]


def test_legacy_pull_requests_match_the_run_branch_to_open_prs(monkeypatch):
    pages = {
        1: [
            {"run_number": 61, "event": "pull_request", "head_branch": "claude/t2", "pull_requests": []},
            {"run_number": 60, "event": "pull_request", "head_branch": "claude/gallery"},
            {"run_number": 58, "event": "pull_request", "head_branch": "claude/sub-lfe"},
        ],
        2: [
            {"run_number": 57, "event": "pull_request", "head_branch": "claude/merged"},
            {"run_number": 41, "event": "workflow_dispatch", "head_branch": "claude/t2"},
        ],
    }
    calls = []

    def fake_gh(*args):
        calls.append(args)
        page = int(args[1].rsplit("page=", 1)[1])
        return gallery_publish.json.dumps({"workflow_runs": pages.get(page, [])})

    monkeypatch.setattr(gallery_publish, "_gh", fake_gh)
    branches = {"claude/t2": 305, "claude/sub-lfe": 300, "claude/gallery": 302}
    found = gallery_publish.legacy_pull_requests("o/r", "ui-gallery-3x.yml", [41, 57, 58, 61], branches)
    assert found == {58: 300, 61: 305}
    assert calls[0][1] == "repos/o/r/actions/workflows/ui-gallery-3x.yml/runs?per_page=100&page=1"
    assert len(calls) == 2


def test_retained_dirs_reject_a_bad_current_name():
    with pytest.raises(ValueError):
        gallery_publish.retained_dirs([], set(), "latest")


TITLE_3X = "WebView UI 갤러리 리뷰 (3.x)"
TITLE_2X = "WebView UI 갤러리 리뷰"


def test_issue_actions_update_current_close_closed_prs_and_keep_open_ones():
    issues = [
        {"number": 280, "title": f"{TITLE_3X}: PR #279"},
        {"number": 295, "title": f"{TITLE_3X}: PR #294"},
        {"number": 297, "title": f"{TITLE_3X}: PR #296"},
        {"number": 301, "title": f"{TITLE_3X}: PR #300"},
        {"number": 302, "title": f"{TITLE_3X}: manual (master)"},
        {"number": 303, "title": f"{TITLE_2X}: PR #279"},  # other kind
        {"number": 304, "title": "unrelated issue"},
    ]
    update, close = gallery_publish.issue_actions(
        issues, TITLE_3X, {294, 300}, f"{TITLE_3X}: PR #300"
    )
    assert update == 301
    assert close == [280, 297, 302]


def test_issue_actions_kinds_do_not_touch_each_other():
    issues = [
        {"number": 1, "title": f"{TITLE_3X}: PR #5"},
        {"number": 2, "title": f"{TITLE_2X}: PR #5"},
    ]
    assert gallery_publish.issue_actions(issues, TITLE_2X, set(), f"{TITLE_2X}: PR #9") == (None, [2])
    assert gallery_publish.issue_actions(issues, TITLE_3X, set(), f"{TITLE_3X}: PR #9") == (None, [1])


def test_issue_actions_close_duplicates_of_the_current_title():
    issues = [
        {"number": 9, "title": f"{TITLE_3X}: PR #7"},
        {"number": 4, "title": f"{TITLE_3X}: PR #7"},
    ]
    assert gallery_publish.issue_actions(issues, TITLE_3X, {7}, f"{TITLE_3X}: PR #7") == (4, [9])


def test_issue_title():
    assert gallery_publish.issue_title(TITLE_3X, 299, "x") == f"{TITLE_3X}: PR #299"
    assert gallery_publish.issue_title(TITLE_3X, None, "master") == f"{TITLE_3X}: manual (master)"


def _body(images):
    return gallery_publish.issue_body(
        "3.x 화면 갤러리", "owner/repo", "runs-3x/58-pr299", 58, "abc", "branch", images
    )


def test_issue_body_embeds_every_image_from_the_run_directory():
    body = _body(["main-dark-en", "main-light-ko"])
    raw = "https://raw.githubusercontent.com/owner/repo/webview-gallery/runs-3x/58-pr299"
    assert f"![main-dark-en]({raw}/main-dark-en.png)" in body
    assert f"![main-light-ko]({raw}/main-light-ko.png)" in body
    assert "https://github.com/owner/repo/tree/webview-gallery/runs-3x/58-pr299" in body
    assert "—" not in body


def test_issue_body_falls_back_to_links_past_the_issue_size_limit():
    images = [f"scenario-{index:04d}-with-a-long-name" for index in range(320)]
    body = _body(images)
    assert len(body) <= 65536
    assert "<details>" not in body
    assert body.count("](https://raw.githubusercontent.com/") == 320


def test_issue_body_points_at_the_branch_when_even_links_do_not_fit():
    images = [f"scenario-{index:04d}-with-a-long-name" for index in range(900)]
    body = _body(images)
    assert len(body) <= 65536
    assert "raw.githubusercontent.com" not in body
    assert "https://github.com/owner/repo/tree/webview-gallery/runs-3x/58-pr299" in body


# Branch rewrite against a local bare repository.

needs_git = pytest.mark.skipif(shutil.which("git") is None, reason="git is not installed")


def _run(*args, cwd):
    return subprocess.run(
        args, cwd=cwd, check=True, capture_output=True, encoding="utf-8", errors="replace"
    ).stdout


@pytest.fixture
def repos(tmp_path):
    remote = tmp_path / "remote.git"
    work = tmp_path / "work"
    _run("git", "init", "--bare", "-q", str(remote), cwd=tmp_path)
    _run("git", "init", "-q", str(work), cwd=tmp_path)
    _run("git", "remote", "add", "origin", str(remote), cwd=work)
    return remote, work


def _images(directory: Path, names, content: bytes):
    directory.mkdir(parents=True, exist_ok=True)
    paths = []
    for name in names:
        path = directory / f"{name}.png"
        path.write_bytes(content + name.encode())
        paths.append(path)
    return paths


def _tree(remote: Path):
    listed = _run("git", "ls-tree", "-r", "--name-only", "webview-gallery", cwd=remote)
    return sorted(listed.splitlines())


def _publish(work, root, run_dir, images, open_prs):
    return gallery_publish.publish(root, run_dir, images, open_prs, f"run {run_dir}", cwd=work)


@needs_git
def test_publish_rewrites_the_branch_as_one_commit_with_only_the_kept_runs(repos, tmp_path):
    remote, work = repos
    shots = _images(tmp_path / "shots", ["main", "settings"], b"png")
    other = _images(tmp_path / "other", ["legacy"], b"2x")

    # First publish creates the branch.
    assert _publish(work, "runs-3x", "10-pr5", shots, {5}) == ["10-pr5"]
    assert _tree(remote) == ["runs-3x/10-pr5/main.png", "runs-3x/10-pr5/settings.png"]

    # The other kind is added next to it and left alone afterwards.
    _publish(work, "runs", "3-pr5", other, {5})
    # A second open PR keeps the first PR's newest run.
    _publish(work, "runs-3x", "11-pr6", shots, {5, 6})
    assert _tree(remote) == [
        "runs-3x/10-pr5/main.png",
        "runs-3x/10-pr5/settings.png",
        "runs-3x/11-pr6/main.png",
        "runs-3x/11-pr6/settings.png",
        "runs/3-pr5/legacy.png",
    ]

    # PR 5 is merged: its 3.x run goes on the next 3.x publish, its 2.x run stays.
    assert _publish(work, "runs-3x", "12-pr6", shots, {6}) == ["12-pr6"]
    assert _tree(remote) == [
        "runs-3x/12-pr6/main.png",
        "runs-3x/12-pr6/settings.png",
        "runs/3-pr5/legacy.png",
    ]

    # Every publish leaves a single commit without a parent.
    assert _run("git", "rev-list", "--count", "webview-gallery", cwd=remote).strip() == "1"
    parents = _run("git", "log", "-1", "--format=%P", "webview-gallery", cwd=remote).strip()
    assert parents == ""
    author = _run("git", "log", "-1", "--format=%an", "webview-gallery", cwd=remote).strip()
    assert author == "github-actions[bot]"


@needs_git
def test_publish_asks_once_about_number_only_runs_and_keeps_open_ones(repos, tmp_path):
    remote, work = repos
    shots = _images(tmp_path / "shots", ["main"], b"png")
    # Number-only directories, as the workflow named them before this script.
    blob = _run("git", "hash-object", "-w", str(shots[0]), cwd=work).strip()
    run = gallery_publish._mktree([f"100644 blob {blob}\tmain.png"], work)
    root = gallery_publish._mktree([f"040000 tree {run}\t57", f"040000 tree {run}\t58"], work)
    top = gallery_publish._mktree([f"040000 tree {root}\truns-3x"], work)
    env = gallery_publish._bot_env()
    commit = gallery_publish._git("commit-tree", top, "-m", "legacy", env=env, cwd=work).strip()
    _run("git", "push", "-q", "origin", f"{commit}:refs/heads/webview-gallery", cwd=work)

    asked = []

    def lookup(runs):
        asked.append(list(runs))
        return {58: 300}

    kept = gallery_publish.publish(
        "runs-3x", "62-pr302", shots, {300, 302}, "run 62", cwd=work, legacy_lookup=lookup
    )
    assert kept == ["58", "62-pr302"]
    assert asked == [[57, 58]]
    assert _tree(remote) == ["runs-3x/58/main.png", "runs-3x/62-pr302/main.png"]


@needs_git
def test_publish_keeps_the_file_bytes(repos, tmp_path):
    remote, work = repos
    shots = _images(tmp_path / "shots", ["main"], b"\x89PNG\r\n\x1a\n")
    _publish(work, "runs-3x", "7-manual", shots, set())
    blob = subprocess.run(
        ["git", "show", "webview-gallery:runs-3x/7-manual/main.png"],
        cwd=remote,
        check=True,
        capture_output=True,
    ).stdout
    assert blob == shots[0].read_bytes()
