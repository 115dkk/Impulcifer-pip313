#!/usr/bin/env python3
"""Publish one UI gallery run and keep only the latest galleries.

Both gallery workflows call this after rendering their PNGs:
webview-gallery.yml (2.x ``webview_ui``, ``--kind 2x``) and ui-gallery-3x.yml
(3.x ``apps/impulcifer-app/ui``, ``--kind 3x``).

The ``webview-gallery`` branch holds one directory per kept run, named
``<run number>-pr<PR number>`` (or ``<run number>-manual``) under ``runs/``
(2.x) or ``runs-3x/`` (3.x). Each publish rewrites the branch as a single
commit without a parent, so PNGs of dropped runs leave the branch history too,
and pushes it with ``--force-with-lease`` (retrying when the other gallery
pushed in between). A publish keeps

* the run it is publishing, and
* the newest run of every other pull request that is still open,

and drops everything else of its own kind: older runs of the same pull
request, runs of closed or merged pull requests, manual runs and directories
from before this scheme. The other kind's directory is left as it is. So after
a pull request is merged its gallery stays until the next gallery run.

Review issues follow the same rule. A run of a pull request that already has
an open review issue replaces that issue's body instead of adding a comment.
Open review issues of the same kind whose pull request is closed or merged
are closed with a pointer to the new issue.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Set, Tuple

BRANCH = "webview-gallery"
BOT_NAME = "github-actions[bot]"
BOT_EMAIL = "41898282+github-actions[bot]@users.noreply.github.com"
# GitHub rejects issue bodies longer than 65536 characters.
BODY_LIMIT = 65000

KINDS = {
    "2x": {
        "root": "runs",
        "title": "WebView UI 갤러리 리뷰",
        "heading": "WebView 프론트엔드(`webview_ui`) 갤러리",
    },
    "3x": {
        "root": "runs-3x",
        "title": "WebView UI 갤러리 리뷰 (3.x)",
        "heading": "3.x 화면(`apps/impulcifer-app/ui`) 갤러리",
    },
}

_RUN_DIR = re.compile(r"^(\d+)(?:-pr(\d+)|-manual)?$")
_PR_TITLE = re.compile(r": PR #(\d+)$")


def run_dir_name(run_number: int, pr_number: Optional[int]) -> str:
    """Directory name of one run on the gallery branch."""
    if pr_number is None:
        return f"{run_number}-manual"
    return f"{run_number}-pr{pr_number}"


def parse_run_dir(name: str) -> Optional[Tuple[int, Optional[int]]]:
    """``(run number, PR number or None)`` for a run directory name."""
    match = _RUN_DIR.match(name)
    if match is None:
        return None
    pr = match.group(2)
    return int(match.group(1)), (int(pr) if pr is not None else None)


def retained_dirs(existing: Iterable[str], open_prs: Set[int], current: str) -> List[str]:
    """Run directories of one kind that survive publishing ``current``.

    ``current`` stays, and so does the newest run of every other pull request
    in ``open_prs``. Older runs of the current pull request, runs of closed
    pull requests, manual runs and names this scheme did not produce go.
    """
    parsed_current = parse_run_dir(current)
    if parsed_current is None:
        raise ValueError(f"not a run directory name: {current}")
    current_pr = parsed_current[1]
    newest: Dict[int, Tuple[int, str]] = {}
    for name in existing:
        parsed = parse_run_dir(name)
        if parsed is None:
            continue
        run, pr = parsed
        if pr is None or pr == current_pr or pr not in open_prs:
            continue
        if pr not in newest or run > newest[pr][0]:
            newest[pr] = (run, name)
    return sorted({current, *(name for _, name in newest.values())})


def issue_title(kind_title: str, pr_number: Optional[int], ref_name: str) -> str:
    context = f"PR #{pr_number}" if pr_number is not None else f"manual ({ref_name})"
    return f"{kind_title}: {context}"


def issue_actions(
    issues: Sequence[dict], kind_title: str, open_prs: Set[int], current_title: str
) -> Tuple[Optional[int], List[int]]:
    """``(issue to update, issues to close)`` among open review issues.

    Only titles of this kind count (``"<kind title>: ..."``). The oldest
    issue with the current title is updated and any duplicate is closed.
    Issues of other open pull requests stay open; the rest are closed.
    """
    prefix = f"{kind_title}: "
    update: Optional[int] = None
    close: List[int] = []
    for issue in sorted(issues, key=lambda item: item["number"]):
        title = issue["title"]
        if not title.startswith(prefix):
            continue
        if title == current_title:
            if update is None:
                update = issue["number"]
            else:
                close.append(issue["number"])
            continue
        match = _PR_TITLE.search(title)
        if match is not None and int(match.group(1)) in open_prs:
            continue
        close.append(issue["number"])
    return update, close


def issue_body(
    heading: str,
    repository: str,
    run_path: str,
    run_number: int,
    sha: str,
    source_ref: str,
    images: Sequence[str],
) -> str:
    """Review issue body; images become a link list past GitHub's size limit."""
    tree_url = f"https://github.com/{repository}/tree/{BRANCH}/{run_path}"
    raw_base = f"https://raw.githubusercontent.com/{repository}/{BRANCH}/{run_path}"
    head = [
        f"{heading}, run {run_number}, 커밋 `{sha}` (`{source_ref}`).",
        "",
        "각 스크린샷을 보고 디자인/레이아웃/번역을 리뷰해 주세요.",
        f"원본 PNG는 [{BRANCH} 브랜치]({tree_url})와 run artifact에 있습니다.",
        "같은 PR에서 갤러리가 다시 돌면 이 본문이 최신 갤러리로 바뀝니다. "
        "더 새 갤러리가 생기고 이 PR이 닫혀 있으면 이슈를 닫고 PNG를 브랜치에서 지웁니다.",
        "",
    ]
    blocks = []
    for name in images:
        blocks += [
            f"<details><summary><b>{name}</b></summary>",
            "",
            f"![{name}]({raw_base}/{name}.png)",
            "",
            "</details>",
        ]
    body = "\n".join(head + blocks) + "\n"
    if len(body) <= BODY_LIMIT:
        return body
    links = [f"- [{name}]({raw_base}/{name}.png)" for name in images]
    note = [
        f"스크린샷 {len(images)}장을 본문에 넣으면 GitHub 이슈 길이 한도를 넘어서 링크로 적습니다.",
        "",
    ]
    body = "\n".join(head + note + links) + "\n"
    if len(body) <= BODY_LIMIT:
        return body
    note = [
        f"스크린샷 {len(images)}장은 링크만 적어도 GitHub 이슈 길이 한도를 넘어서, "
        f"위의 {BRANCH} 브랜치 폴더에서 보아야 합니다.",
    ]
    return "\n".join(head + note) + "\n"


def _git(
    *args: str,
    env: Optional[Dict[str, str]] = None,
    cwd: Optional[Path] = None,
    stdin: Optional[str] = None,
) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, encoding="utf-8", errors="replace", env=env, cwd=cwd, input=stdin
    ).stdout


def _bot_env() -> Dict[str, str]:
    env = dict(os.environ)
    env.update(
        GIT_AUTHOR_NAME=BOT_NAME,
        GIT_AUTHOR_EMAIL=BOT_EMAIL,
        GIT_COMMITTER_NAME=BOT_NAME,
        GIT_COMMITTER_EMAIL=BOT_EMAIL,
    )
    return env


def fetch_branch(remote: str, cwd: Optional[Path] = None) -> Optional[str]:
    """Tip of the gallery branch on ``remote``, or None when it does not exist."""
    tracking = f"refs/remotes/{remote}/{BRANCH}"
    fetched = subprocess.run(
        ["git", "fetch", "--depth=1", "--no-tags", remote, f"+refs/heads/{BRANCH}:{tracking}"],
        capture_output=True,
        encoding="utf-8", errors="replace",
        cwd=cwd,
    )
    if fetched.returncode != 0:
        if "couldn't find remote ref" in fetched.stderr:
            return None
        raise RuntimeError(f"git fetch {BRANCH} failed:\n{fetched.stderr}")
    return _git("rev-parse", tracking, cwd=cwd).strip()


def _tree_entries(treeish: Optional[str], cwd: Optional[Path] = None) -> Dict[str, str]:
    """``{name: ls-tree line}`` of one tree level; empty when it does not exist."""
    if treeish is None:
        return {}
    listed = subprocess.run(
        ["git", "ls-tree", "-z", treeish], capture_output=True, encoding="utf-8", errors="replace", cwd=cwd
    )
    if listed.returncode != 0:
        return {}
    entries = {}
    for line in listed.stdout.split("\0"):
        if line:
            entries[line.split("\t", 1)[1]] = line
    return entries


def _mktree(lines: Iterable[str], cwd: Optional[Path] = None) -> str:
    return _git("mktree", "-z", cwd=cwd, stdin="".join(f"{line}\0" for line in lines)).strip()


def publish(
    root: str,
    run_dir: str,
    images: Sequence[Path],
    open_prs: Set[int],
    message: str,
    remote: str = "origin",
    cwd: Optional[Path] = None,
    attempts: int = 5,
) -> List[str]:
    """Rewrite the gallery branch with the kept runs; returns the kept names."""
    run_lines = []
    for image in images:
        blob = _git("hash-object", "-w", "--", str(image), cwd=cwd).strip()
        run_lines.append(f"100644 blob {blob}\t{image.name}")
    run_tree = _mktree(run_lines, cwd)
    for attempt in range(1, attempts + 1):
        base = fetch_branch(remote, cwd)
        top = _tree_entries(base, cwd)
        runs = _tree_entries(f"{base}:{root}" if base and root in top else None, cwd)
        keep = retained_dirs(runs, open_prs, run_dir)
        root_lines = [runs[name] for name in keep if name in runs and name != run_dir]
        root_lines.append(f"040000 tree {run_tree}\t{run_dir}")
        top[root] = f"040000 tree {_mktree(root_lines, cwd)}\t{root}"
        tree = _mktree(top.values(), cwd)
        commit = _git("commit-tree", tree, "-m", message, env=_bot_env(), cwd=cwd).strip()
        lease = f"--force-with-lease=refs/heads/{BRANCH}:{base or ''}"
        pushed = subprocess.run(
            ["git", "push", lease, remote, f"{commit}:refs/heads/{BRANCH}"],
            capture_output=True,
            encoding="utf-8", errors="replace",
            cwd=cwd,
        )
        if pushed.returncode == 0:
            dropped = sorted(set(runs) - set(keep))
            print(f"published {root}/{run_dir}; kept {keep}; dropped {dropped}")
            return keep
        print(f"push attempt {attempt} rejected:\n{pushed.stderr}", file=sys.stderr)
        time.sleep(attempt * 3)
    raise RuntimeError(f"could not publish {root}/{run_dir} after {attempts} attempts")


def _gh(*args: str) -> str:
    for attempt in range(1, 4):
        result = subprocess.run(["gh", *args], capture_output=True, encoding="utf-8", errors="replace")
        if result.returncode == 0:
            return result.stdout
        print(f"gh {args[0]} {args[1]} failed (attempt {attempt}):\n{result.stderr}", file=sys.stderr)
        if attempt < 3:
            time.sleep(attempt * 5)
    raise RuntimeError(f"gh {' '.join(args[:2])} failed")


def open_pull_requests(repository: str) -> Set[int]:
    listed = _gh(
        "pr", "list", "--repo", repository, "--state", "open", "--limit", "500", "--json", "number"
    )
    return {item["number"] for item in json.loads(listed)}


def sync_issues(
    repository: str,
    kind_title: str,
    title: str,
    body: str,
    open_prs: Set[int],
    run_number: int,
) -> int:
    listed = _gh(
        "issue", "list", "--repo", repository, "--state", "open", "--limit", "500",
        "--search", f'in:title "{kind_title}"', "--json", "number,title",
    )
    update, close = issue_actions(json.loads(listed), kind_title, open_prs, title)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".md", delete=False) as handle:
        handle.write(body)
        body_file = handle.name
    try:
        if update is not None:
            _gh("issue", "edit", str(update), "--repo", repository, "--body-file", body_file)
            current = update
            print(f"Updated review issue #{current}")
        else:
            try:
                url = _gh(
                    "issue", "create", "--repo", repository, "--title", title,
                    "--body-file", body_file, "--label", "webview-gallery",
                )
            except RuntimeError:
                url = _gh(
                    "issue", "create", "--repo", repository, "--title", title,
                    "--body-file", body_file,
                )
            current = int(url.strip().rstrip("/").rsplit("/", 1)[-1])
            print(f"Opened review issue #{current}")
    finally:
        os.unlink(body_file)
    for number in close:
        _gh(
            "issue", "close", str(number), "--repo", repository, "--comment",
            f"새 갤러리 #{current}(run {run_number})가 생겨 닫습니다. "
            f"이 이슈의 PNG는 `{BRANCH}` 브랜치에서 지웠습니다.",
        )
        print(f"Closed review issue #{number}")
    return current


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--kind", choices=sorted(KINDS), required=True)
    parser.add_argument("--images", type=Path, required=True, help="directory of rendered PNGs")
    args = parser.parse_args(argv)
    kind = KINDS[args.kind]

    repository = os.environ["GITHUB_REPOSITORY"]
    run_number = int(os.environ["GITHUB_RUN_NUMBER"])
    sha = os.environ["GITHUB_SHA"]
    pr_text = os.environ.get("PR_NUMBER", "").strip()
    pr_number = int(pr_text) if pr_text else None
    ref_name = os.environ.get("GITHUB_REF_NAME", "")
    source_ref = os.environ.get("GITHUB_HEAD_REF") or ref_name

    images = sorted(args.images.glob("*.png"))
    if not images:
        raise SystemExit(f"no PNG files in {args.images}")
    open_prs = open_pull_requests(repository)
    run_dir = run_dir_name(run_number, pr_number)
    run_path = f"{kind['root']}/{run_dir}"
    publish(
        kind["root"],
        run_dir,
        images,
        open_prs,
        f"gallery {kind['root']}: run {run_number} ({sha})",
    )
    title = issue_title(kind["title"], pr_number, ref_name)
    body = issue_body(
        kind["heading"],
        repository,
        run_path,
        run_number,
        sha,
        source_ref,
        [image.stem for image in images],
    )
    sync_issues(repository, kind["title"], title, body, open_prs, run_number)
    return 0


if __name__ == "__main__":
    sys.exit(main())
