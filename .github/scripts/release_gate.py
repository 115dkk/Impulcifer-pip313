#!/usr/bin/env python3
"""Release gate for the consolidated ``publish.yml`` pipeline.

Decides whether a push to ``master`` should publish to PyPI + build the
cross-platform Nuitka distribution, and performs an automatic PATCH bump as a
safety net when a *shippable* change was merged without a manual version bump.

Decision policy (path-based PATCH safety net):

* ``workflow_dispatch``           -> release the current pyproject version
                                     (unless it is already on PyPI).
* manual bump (version changed)   -> release that version, respecting the
                                     developer's MINOR/MAJOR choice (unless it
                                     is already on PyPI).
* shippable change, no bump       -> auto-bump to the next free PATCH, commit
                                     the version + an auto CHANGELOG entry to
                                     ``master`` with ``[skip ci]``, release it.
* docs / CI / tests only          -> do nothing (no publish, no build).

A change is "shippable" when it touches a file that ends up in the shipped
wheel or the bundled standalone app. Doc/CI/test/meta files are excluded (see
``EXCLUDE``), mirroring the heuristic already used by ``biweekly-audit.yml``.

The pure functions (:func:`decide`, :func:`next_free_patch`,
:func:`classify_shippable`) are unit-tested in ``tests/test_release_gate.py``;
the ``__main__`` wrapper wires them to git, PyPI and ``$GITHUB_OUTPUT``.
"""
from __future__ import annotations

import fnmatch
import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path

PYPI_PROJECT = "impulcifer-py313"
ZERO_SHA = "0" * 40

# Paths that do NOT change the shipped product. A push that only touches these
# triggers neither a PyPI publish nor a Nuitka build. Directory patterns end in
# ``/*`` and match at any depth; the rest match basenames/paths via fnmatch.
#
# NB: ``*.txt`` is deliberately NOT excluded (unlike biweekly-audit's
# code-change heuristic): ``requirements.txt`` is the dependency list and
# ``README.txt`` / ``data/**/*.txt`` are bundled into the shipped artifacts, so
# a change there is genuinely release-worthy.
EXCLUDE = (
    "*.md", "*.rst", "*.adoc", "*.ipynb",
    "docs/*", ".github/*", "tests/*", "research/*", ".claude/*",
    "packaging/*",
    "LICENSE*", "CHANGELOG*", "CONTEXT.md", ".gitignore", ".gitattributes",
)


# ---------------------------------------------------------------------------
# Pure logic (unit-tested)
# ---------------------------------------------------------------------------
def _excluded(path: str) -> bool:
    base = path.rsplit("/", 1)[-1]
    for pat in EXCLUDE:
        if pat.endswith("/*"):
            prefix = pat[:-2]
            if path == prefix or path.startswith(prefix + "/"):
                return True
        elif fnmatch.fnmatch(base, pat) or fnmatch.fnmatch(path, pat):
            return True
    return False


def classify_shippable(changed_files):
    """Return the subset of ``changed_files`` that affects the shipped product."""
    return [f for f in changed_files if f and not _excluded(f)]


def parse_version(v: str):
    m = re.match(r"^\s*(\d+)\.(\d+)\.(\d+)", v.strip())
    if not m:
        raise ValueError(f"unparseable version: {v!r}")
    return tuple(int(x) for x in m.groups())


def next_free_patch(base: str, is_published) -> str:
    """Next PATCH after ``base`` that is not already published on PyPI."""
    major, minor, patch = parse_version(base)
    while True:
        patch += 1
        cand = f"{major}.{minor}.{patch}"
        if not is_published(cand):
            return cand


def decide(event_name, old_ver, new_ver, shippable, is_published):
    """Return ``(action, version)`` where action is ``none``/``release``/``bump``."""
    if event_name == "workflow_dispatch":
        return ("none", new_ver) if is_published(new_ver) else ("release", new_ver)
    if old_ver != new_ver:  # developer bumped manually (any SemVer level)
        return ("none", new_ver) if is_published(new_ver) else ("release", new_ver)
    if shippable:
        return ("bump", next_free_patch(new_ver, is_published))
    return ("none", new_ver)


# ---------------------------------------------------------------------------
# I/O wrapper (runs in CI)
# ---------------------------------------------------------------------------
def _run(*args, check=True, capture=False):
    # Force UTF-8 decoding: git output (e.g. pyproject.toml with Korean author
    # names) is UTF-8, but text=True would otherwise use the platform locale
    # codec (cp949 on Korean Windows) and fail. CI runs UTF-8 anyway; this keeps
    # the gate deterministic and locally testable.
    return subprocess.run(
        list(args), check=check, text=True, encoding="utf-8", errors="replace",
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )


def _git(*args, check=True):
    return _run("git", *args, check=check, capture=True).stdout.strip()


def _read_version_from_text(toml_text: str) -> str:
    m = re.search(r'(?m)^version\s*=\s*"([^"]+)"', toml_text)
    if not m:
        raise ValueError("no [project] version found in pyproject.toml")
    return m.group(1)


def _version_at(ref: str) -> str | None:
    try:
        txt = _git("show", f"{ref}:pyproject.toml")
    except subprocess.CalledProcessError:
        return None
    try:
        return _read_version_from_text(txt)
    except ValueError:
        return None


def _resolve_before(event_before: str, head: str) -> str:
    if event_before and event_before != ZERO_SHA:
        if _run("git", "cat-file", "-e", f"{event_before}^{{commit}}",
                check=False, capture=True).returncode == 0:
            return event_before
    # Fallbacks: previous commit, else the empty tree (everything is "added").
    parent = _run("git", "rev-parse", f"{head}~1", check=False, capture=True)
    if parent.returncode == 0:
        return parent.stdout.strip()
    return _git("hash-object", "-t", "tree", "/dev/null")


def _published_versions() -> set:
    url = f"https://pypi.org/pypi/{PYPI_PROJECT}/json"
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            data = json.load(resp)
        return set(data.get("releases", {}))
    except (urllib.error.URLError, json.JSONDecodeError, TimeoutError, OSError) as exc:
        # Network/parse failure: treat as "nothing published" so we don't block
        # a release. ``skip-existing: true`` on the publish step is the backstop.
        print(f"::warning::could not read PyPI release list ({exc}); "
              f"assuming version not yet published")
        return set()


def _bump_pyproject(new_version: str) -> None:
    p = Path("pyproject.toml")
    txt = p.read_text(encoding="utf-8")
    new_txt, n = re.subn(r'(?m)^version\s*=\s*"[^"]+"',
                         f'version = "{new_version}"', txt, count=1)
    if n != 1:
        raise SystemExit("failed to rewrite version in pyproject.toml")
    p.write_text(new_txt, encoding="utf-8")


def _prepend_changelog(new_version: str, date: str, commit_lines: str) -> None:
    p = Path("CHANGELOG.md")
    lines = p.read_text(encoding="utf-8").splitlines(keepends=True)
    entry = (
        f"## {new_version} - {date}\n"
        f"### 🔧 자동 릴리스 (CI auto-bump)\n\n"
        f"#### 🔧 빌드 / 설정 변경\n"
        f"- **CI 자동 PATCH bump**: 수동 버전 bump 없이 머지된 출하 변경에 대해 "
        f"릴리스 파이프라인이 PATCH를 자동 증가시켰다. 포함된 커밋:\n"
        f"{commit_lines}\n\n"
    )
    idx = next((i for i, ln in enumerate(lines) if ln.startswith("## ")), len(lines))
    lines.insert(idx, entry)
    p.write_text("".join(lines), encoding="utf-8")


def _commit_and_push(new_version: str) -> str:
    _run("git", "config", "user.name", "github-actions[bot]")
    _run("git", "config", "user.email",
         "41898282+github-actions[bot]@users.noreply.github.com")
    _run("git", "add", "pyproject.toml", "CHANGELOG.md")
    _run("git", "commit", "-m",
         f"chore(release): auto-bump to v{new_version} [skip ci]")
    for attempt in range(1, 4):
        _run("git", "fetch", "origin", "master", check=False)
        reb = _run("git", "rebase", "FETCH_HEAD", check=False, capture=True)
        if reb.returncode != 0:
            _run("git", "rebase", "--abort", check=False)
            print(f"::warning::rebase failed (attempt {attempt}); retrying")
            time.sleep(5)
            continue
        push = _run("git", "push", "origin", "HEAD:master", check=False, capture=True)
        if push.returncode == 0:
            return _git("rev-parse", "HEAD")
        print(f"::warning::push rejected (attempt {attempt}); retrying\n{push.stdout}")
        time.sleep(5)
    raise SystemExit("auto-bump push failed after 3 attempts")


def _emit(should_release: bool, version: str, release_sha: str) -> None:
    out = os.environ.get("GITHUB_OUTPUT")
    payload = (
        f"should_release={'true' if should_release else 'false'}\n"
        f"version={version}\n"
        f"release_sha={release_sha}\n"
    )
    if out:
        with open(out, "a", encoding="utf-8") as fh:
            fh.write(payload)
    print(payload, end="")


def main() -> int:
    event = os.environ.get("GITHUB_EVENT_NAME", "")
    head = os.environ.get("GITHUB_SHA") or _git("rev-parse", "HEAD")
    before = _resolve_before(os.environ.get("EVENT_BEFORE", ""), head)

    new_ver = _read_version_from_text(Path("pyproject.toml").read_text(encoding="utf-8"))
    old_ver = _version_at(before) or new_ver

    changed = _git("diff", "--name-only", before, head).splitlines() \
        if event != "workflow_dispatch" else []
    shippable = classify_shippable(changed)

    published = _published_versions()
    is_published = published.__contains__

    print(f"event={event} before={before[:12]} head={head[:12]}")
    print(f"old_ver={old_ver} new_ver={new_ver} "
          f"shippable={len(shippable)} files: {shippable[:20]}")

    action, version = decide(event, old_ver, new_ver, shippable, is_published)
    print(f"decision: action={action} version={version}")

    release_sha = head
    if action == "bump":
        date = _git("show", "-s", "--format=%cs", head)  # commit date YYYY-MM-DD
        commit_lines = _git("log", "--pretty=format:- %s", f"{before}..{head}") \
            or "- (no commit subjects)"
        _bump_pyproject(version)
        _prepend_changelog(version, date, commit_lines)
        release_sha = _commit_and_push(version)
        print(f"auto-bumped to v{version}; release_sha={release_sha}")

    _emit(action != "none", version, release_sha)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
