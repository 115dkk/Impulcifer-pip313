#!/usr/bin/env python3
"""Release notes for one version, taken from its CHANGELOG.md section.

release-3x.yml writes the GitHub Release body (and latest.json's ``notes``)
with this. The update dialog of an installed app shows that body as the
release notes, so it has to say what changed in the version rather than
point somewhere else.

    python .github/scripts/release_notes.py --version 3.0.2 --out release-notes.md

The notes are every ``## <version> - <date>`` block of CHANGELOG.md (a
version can appear more than once when an entry was added under an existing
version), without that heading line. A version with no block falls back to
one line linking the CHANGELOG, with a workflow warning, so a missing entry
never fails a release whose packages are already published.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

_VERSION_HEADING = re.compile(r"^## (\S+)")


def changelog_section(text: str, version: str) -> str | None:
    """Body of every ``## <version>`` block in file order, or None when absent."""
    blocks: list[list[str]] = []
    current: list[str] | None = None
    for line in text.splitlines():
        heading = _VERSION_HEADING.match(line)
        if heading:
            current = [] if heading.group(1) == version else None
            if current is not None:
                blocks.append(current)
        elif current is not None:
            current.append(line)
    parts = [body for body in ("\n".join(block).strip() for block in blocks) if body]
    return "\n\n".join(parts) if parts else None


def release_notes(text: str, version: str, repo: str, tag: str) -> tuple[str, bool]:
    """(notes, found). Without a section the notes link the tagged CHANGELOG."""
    section = changelog_section(text, version)
    if section is not None:
        return section + "\n", True
    link = f"https://github.com/{repo}/blob/{tag}/CHANGELOG.md" if repo else "CHANGELOG.md"
    return f"Impulcifer {version}. The changes are listed in {link}\n", False


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True)
    parser.add_argument("--changelog", default="CHANGELOG.md")
    parser.add_argument("--repo", default="", help="owner/name, for the fallback link")
    parser.add_argument("--tag", default="", help="release tag, for the fallback link")
    parser.add_argument("--out", required=True)
    args = parser.parse_args(argv)
    text = Path(args.changelog).read_text(encoding="utf-8")
    notes, found = release_notes(text, args.version, args.repo, args.tag or f"v{args.version}")
    Path(args.out).write_text(notes, encoding="utf-8")
    if not found:
        print(f"::warning::CHANGELOG.md has no '## {args.version}' section; "
              "the release notes only link the CHANGELOG.")
    print(notes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
