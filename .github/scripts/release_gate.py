#!/usr/bin/env python3
"""Release gate shared by the two release pipelines.

* ``release-3x.yml`` (3.x, Rust + Tauri, the stable line) runs this with
  ``--product 3x`` on every push to ``master``.
* ``publish.yml`` (2.x, Python + Nuitka, maintenance) runs it with
  ``--product 2x`` only when dispatched by hand.

The last release of a product is its tag ``v<version>``. Decision policy:

* the current version has no tag          -> release it (a manual bump, or a
                                             retry after a release that failed
                                             before its GitHub Release)
* tagged, shippable files changed since   -> bump PATCH to the next version that
                                             has no tag and is not on PyPI, commit
                                             the version + an auto CHANGELOG entry
                                             to ``master`` with ``[skip ci]``,
                                             release it. A prerelease version
                                             (``3.1.0-alpha.1``) is never bumped
                                             automatically.
* tagged, nothing shippable since the tag -> do nothing (no publish, no build).

A file is "shippable" when it ends up in that product's artifacts. 2.x ships
everything outside ``EXCLUDE``; 3.x ships the build inputs in ``SHIP_3X`` minus
their tests, benches and docs.

The pure functions (:func:`decide`, :func:`next_free_patch`,
:func:`classify_shippable` and the version rewriters) are unit-tested in
``tests/test_release_gate.py``; :func:`main` wires them to git, PyPI and
``$GITHUB_OUTPUT``.
"""
from __future__ import annotations

import argparse
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

# 2.x: paths that do NOT change the shipped wheel or Nuitka bundles. Directory
# patterns end in ``/*`` and match at any depth; the rest match basenames/paths
# via fnmatch.
#
# NB: ``*.txt`` is deliberately NOT excluded: ``requirements.txt`` is the
# dependency list and ``README.txt`` / ``data/**/*.txt`` are bundled into the
# shipped artifacts, so a change there is genuinely release-worthy.
EXCLUDE = (
    "*.md", "*.rst", "*.adoc", "*.ipynb",
    "docs/*", ".github/*", "tests/*", "research/*", ".claude/*",
    "packaging/*",
    "LICENSE*", "CHANGELOG*", "CONTEXT.md", ".gitignore", ".gitattributes",
    # 3.x Rust workspace (ADR 0002). It does not ship in the 2.x wheel or the
    # Nuitka bundles, so Rust-only changes never bump or publish 2.x.
    "crates/*", "apps/*", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
    "features.toml", "unsafe-budget.toml",
)

# 3.x: inputs of the Tauri apps and the PyO3 wheels. The service compiles the
# 2.x translation catalogue in, the app and the wheel bundle the sweeps and
# Harman targets, and the Windows package is made by pack_velopack.ps1.
SHIP_3X = (
    "crates/*", "apps/*", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
    "i18n/locales/*", "data/sweep*", "data/harman*",
    "build_scripts/pack_velopack.ps1",
)
# ...minus what inside them does not ship: tests, benches, docs and the
# type-check tooling of the static frontend.
NOT_SHIPPED_3X = (
    "*.md", "*.d.ts",
    "apps/impulcifer-app/package.json", "apps/impulcifer-app/package-lock.json",
    "apps/impulcifer-app/tsconfig.json",
)
NOT_SHIPPED_3X_DIRS = ("tests", "benches")

PRODUCTS = ("2x", "3x")


# ---------------------------------------------------------------------------
# Pure logic (unit-tested)
# ---------------------------------------------------------------------------
def _matches(path: str, patterns) -> bool:
    base = path.rsplit("/", 1)[-1]
    for pat in patterns:
        if pat.endswith("/*"):
            prefix = pat[:-2]
            if path == prefix or path.startswith(prefix + "/"):
                return True
        elif fnmatch.fnmatch(base, pat) or fnmatch.fnmatch(path, pat):
            return True
    return False


def _ships_3x(path: str) -> bool:
    if not _matches(path, SHIP_3X) or _matches(path, NOT_SHIPPED_3X):
        return False
    return not any(part in NOT_SHIPPED_3X_DIRS for part in path.split("/")[:-1])


def classify_shippable(changed_files, product: str = "2x"):
    """Return the subset of ``changed_files`` that ships in ``product``."""
    if product == "3x":
        return [f for f in changed_files if f and _ships_3x(f)]
    return [f for f in changed_files if f and not _matches(f, EXCLUDE)]


def parse_version(v: str):
    m = re.match(r"^\s*(\d+)\.(\d+)\.(\d+)", v.strip())
    if not m:
        raise ValueError(f"unparseable version: {v!r}")
    return tuple(int(x) for x in m.groups())


def is_prerelease(v: str) -> bool:
    return "-" in v.strip()


def next_free_patch(base: str, is_taken) -> str:
    """Next PATCH after ``base`` for which ``is_taken`` is false."""
    major, minor, patch = parse_version(base)
    while True:
        patch += 1
        cand = f"{major}.{minor}.{patch}"
        if not is_taken(cand):
            return cand


def decide(version, is_released, shippable, is_taken):
    """Return ``(action, version)`` where action is ``none``/``release``/``bump``.

    ``is_released(v)``: v already has its release tag.
    ``shippable``: shippable files changed since the tag of ``version``.
    ``is_taken(v)``: v cannot be used for an automatic bump (tagged or on PyPI).
    """
    if not is_released(version):
        return ("release", version)
    if not shippable or is_prerelease(version):
        return ("none", version)
    return ("bump", next_free_patch(version, is_taken))


def pyproject_version(text: str) -> str:
    m = re.search(r'(?m)^version\s*=\s*"([^"]+)"', text)
    if not m:
        raise ValueError("no [project] version found in pyproject.toml")
    return m.group(1)


def set_pyproject_version(text: str, new_version: str) -> str:
    new_text, n = re.subn(r'(?m)^version\s*=\s*"[^"]+"',
                          f'version = "{new_version}"', text, count=1)
    if n != 1:
        raise ValueError("no version line in pyproject.toml")
    return new_text


def workspace_version(text: str) -> str:
    """``[workspace.package].version`` of a Cargo.toml."""
    section = None
    for line in text.splitlines():
        header = re.match(r"^\s*\[([^\]]+)\]\s*$", line)
        if header:
            section = header.group(1).strip()
            continue
        m = re.match(r'^\s*version\s*=\s*"([^"]+)"', line)
        if m and section == "workspace.package":
            return m.group(1)
    raise ValueError("no [workspace.package] version in Cargo.toml")


def set_workspace_version(text: str, new_version: str) -> str:
    lines = text.splitlines(keepends=True)
    section = None
    for i, line in enumerate(lines):
        header = re.match(r"^\s*\[([^\]]+)\]\s*$", line)
        if header:
            section = header.group(1).strip()
            continue
        if section == "workspace.package" and re.match(r'^\s*version\s*=\s*"', line):
            lines[i] = re.sub(r'"[^"]+"', f'"{new_version}"', line, count=1)
            return "".join(lines)
    raise ValueError("no [workspace.package] version in Cargo.toml")


def set_lock_workspace_versions(text: str, old: str, new: str) -> str:
    """Rewrite ``old`` to ``new`` for the path (workspace) packages of a Cargo.lock.

    Registry and git packages carry a ``source`` line right after ``version``;
    workspace members do not, so a crates.io package that happens to share the
    version is left alone.
    """
    pattern = re.compile(
        r'(?m)^(\[\[package\]\]\r?\nname = "[^"]+"\r?\nversion = ")'
        + re.escape(old) + r'("\r?\n)(?!source = )'
    )
    new_text, n = pattern.subn(lambda m: m.group(1) + new + m.group(2), text)
    if n == 0:
        raise ValueError(f"no workspace package at {old} in Cargo.lock")
    return new_text


def set_tauri_version(text: str, old: str, new: str) -> str:
    new_text, n = re.subn(r'("version"\s*:\s*")' + re.escape(old) + r'"',
                          lambda m: m.group(1) + new + '"', text, count=1)
    if n != 1:
        raise ValueError(f'no "version": "{old}" in tauri.conf.json')
    return new_text


def prepend_changelog(text: str, new_version: str, date: str, commit_lines: str,
                      product: str) -> str:
    what = "3.x" if product == "3x" else "2.x"
    entry = (
        f"## {new_version} - {date}\n"
        f"### 🔧 자동 릴리스 (CI auto-bump, {what})\n\n"
        f"#### 🔧 빌드 / 설정 변경\n"
        f"- **CI 자동 PATCH bump**: 수동 버전 bump 없이 머지된 {what} 출하 변경에 "
        f"대해 릴리스 파이프라인이 PATCH를 자동 증가시켰다. 포함된 커밋:\n"
        f"{commit_lines}\n\n"
    )
    lines = text.splitlines(keepends=True)
    idx = next((i for i, ln in enumerate(lines) if ln.startswith("## ")), len(lines))
    lines.insert(idx, entry)
    return "".join(lines)


# ---------------------------------------------------------------------------
# I/O wrapper (runs in CI)
# ---------------------------------------------------------------------------
CARGO_TOML = Path("Cargo.toml")
CARGO_LOCK = Path("Cargo.lock")
TAURI_CONF = Path("apps/impulcifer-app/tauri.conf.json")
PYPROJECT = Path("pyproject.toml")
CHANGELOG = Path("CHANGELOG.md")


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


def _read(path: Path) -> str:
    # newline="" here and in _write keeps the file's own line endings.
    with open(path, encoding="utf-8", newline="") as fh:
        return fh.read()


def _write(path: Path, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)


def _current_version(product: str) -> str:
    if product == "3x":
        return workspace_version(_read(CARGO_TOML))
    return pyproject_version(_read(PYPROJECT))


def _tags() -> set:
    # The checkout may be shallow or tag-less; ask the remote as well.
    _run("git", "fetch", "--tags", "--force", "--quiet", "origin", check=False)
    return {t for t in _git("tag", "-l", "v*").splitlines() if t}


def _published_versions() -> set:
    url = f"https://pypi.org/pypi/{PYPI_PROJECT}/json"
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            data = json.load(resp)
        return set(data.get("releases", {}))
    except (urllib.error.URLError, json.JSONDecodeError, TimeoutError, OSError) as exc:
        # Only the bump candidate uses this list; the tags still rule out every
        # version released through a pipeline, and ``skip-existing: true`` on
        # the publish step is the backstop.
        print(f"::warning::could not read PyPI release list ({exc})")
        return set()


def _bump(product: str, old: str, new: str) -> list:
    if product == "3x":
        _write(CARGO_TOML, set_workspace_version(_read(CARGO_TOML), new))
        _write(CARGO_LOCK, set_lock_workspace_versions(_read(CARGO_LOCK), old, new))
        _write(TAURI_CONF, set_tauri_version(_read(TAURI_CONF), old, new))
        return [str(CARGO_TOML), str(CARGO_LOCK), str(TAURI_CONF)]
    _write(PYPROJECT, set_pyproject_version(_read(PYPROJECT), new))
    return [str(PYPROJECT)]


def _commit_and_push(files: list, message: str) -> str:
    _run("git", "config", "user.name", "github-actions[bot]")
    _run("git", "config", "user.email",
         "41898282+github-actions[bot]@users.noreply.github.com")
    _run("git", "add", *files)
    _run("git", "commit", "-m", message)
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
    payload = (
        f"should_release={'true' if should_release else 'false'}\n"
        f"version={version}\n"
        f"tag=v{version}\n"
        f"prerelease={'true' if is_prerelease(version) else 'false'}\n"
        f"release_sha={release_sha}\n"
    )
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as fh:
            fh.write(payload)
    print(payload, end="")


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--product", choices=PRODUCTS, default="2x")
    product = parser.parse_args(argv).product

    head = os.environ.get("GITHUB_SHA") or _git("rev-parse", "HEAD")
    version = _current_version(product)
    tags = _tags()
    tag = f"v{version}"

    changed = []
    if tag in tags:
        changed = _git("diff", "--name-only", tag, head).splitlines()
    shippable = classify_shippable(changed, product)

    published = _published_versions()
    is_released = lambda v: f"v{v}" in tags  # noqa: E731
    is_taken = lambda v: f"v{v}" in tags or v in published  # noqa: E731

    print(f"product={product} head={head[:12]} version={version} "
          f"tagged={tag in tags}")
    print(f"shippable since {tag}: {len(shippable)} files {shippable[:20]}")

    action, new_version = decide(version, is_released, shippable, is_taken)
    if action == "none" and shippable and is_prerelease(version):
        print(f"::notice::{version} is a prerelease; bump it by hand to release "
              f"these changes")
    print(f"decision: action={action} version={new_version}")

    release_sha = head
    if action == "bump":
        ref = os.environ.get("GITHUB_REF", "refs/heads/master")
        if ref != "refs/heads/master":
            # The bump commit is rebased onto master and pushed there; from any
            # other ref that would push the ref's own commits to master too.
            raise SystemExit(f"{ref}: an automatic bump only runs on master; "
                             f"bump the version by hand or dispatch on master")
        date = _git("show", "-s", "--format=%cs", head)  # commit date YYYY-MM-DD
        commit_lines = _git("log", "--pretty=format:- %s", f"{tag}..{head}",
                            "--", *shippable) or "- (no commit subjects)"
        files = _bump(product, version, new_version)
        _write(CHANGELOG, prepend_changelog(_read(CHANGELOG), new_version, date,
                                            commit_lines, product))
        what = "3.x" if product == "3x" else "2.x"
        release_sha = _commit_and_push(
            files + [str(CHANGELOG)],
            f"chore(release): auto-bump {what} to v{new_version} [skip ci]",
        )
        print(f"auto-bumped to v{new_version}; release_sha={release_sha}")

    _emit(action != "none", new_version, release_sha)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
