#!/usr/bin/env python3
"""3.x demo BRIR integrity: a PR must produce the same bytes as its base.

``brir-3x.yml`` builds the release CLI twice on one runner, from the PR's
base commit and from the PR, and runs this script with both binaries. Every
scenario processes a fresh copy of ``data/demo`` with
``data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav``, each binary with the
files of its own revision (``--base-root``), and hashes every WAV in the
directory afterwards (SHA-256 of the file bytes; the CLI overwrites tracked
files such as ``room-responses.wav``). A scenario passes when

* the PR's WAVs hash the same as the base's (the 3.x-to-3.x baseline), and
* the PR's WAVs hash the same again with ``RAYON_NUM_THREADS=1`` (the output
  must not depend on scheduling).

Both runs happen on the same machine because the bytes depend on the platform
math library; there is no stored hash. A PR whose output is meant to change
carries the ``brir-change`` label, which the workflow passes as
``--allow-change``: a baseline difference is then reported as a warning. The
thread check is never waived.

A base run that fails is a failure too, unless the base CLI predates the
scenario: every scenario names the first 3.x version whose CLI runs it, and
only a base older than that (``--version``) has no baseline. A PR that adds a
scenario for a new option gives it the version that ships the option. With
``--allow-change`` a failing base run is a warning (the PR may fix it).

Local use, after building both binaries::

    python .github/scripts/brir_3x_integrity.py --base /tmp/impulcifer-base \\
        --head target/release/impulcifer
"""
from __future__ import annotations

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
DEMO = Path("data") / "demo"
TEST_SIGNAL = Path("data") / "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"

# (name, CLI arguments, first 3.x version whose CLI runs it). The demo has
# speaker-ear room recordings, so every scenario without --no_room_correction
# runs a room correction mode.
SCENARIOS: tuple[tuple[str, tuple[str, ...], str], ...] = (
    ("default", (), "3.0.0"),
    ("room_modes", ("--room_range=modes",), "3.2.0"),
    ("room_extreme", ("--room_range=extreme",), "3.2.0"),
    ("room_legacy", ("--room_range=legacy",), "3.2.0"),
    ("room_tuning", ("--room_mode=tuning",), "3.2.0"),
    ("room_tuning_auto_delay", ("--room_mode=tuning", "--room_tuning_delay=auto"), "3.2.0"),
    (
        "room_tuning_schroeder",
        ("--room_mode=tuning", "--room_tuning_phase_limit=schroeder"),
        "3.2.0",
    ),
    (
        "room_tuning_magnitude_only",
        ("--room_mode=tuning", "--room_tuning_phase_limit=off", "--room_tuning_level_match=false"),
        "3.2.0",
    ),
    ("no_room_correction", ("--no_room_correction",), "3.0.0"),
    ("virtual_bass", ("--vbass", "--vbass_freq=250"), "3.0.0"),
    ("dsp_shaping", ("--decay=100", "--channel_balance=trend", "--bass_boost=4"), "3.0.0"),
    (
        "resample_and_extra_outputs",
        ("--fs=44100", "--output_truehd_layouts", "--jamesdsp", "--hangloose"),
        "3.0.0",
    ),
    ("no_headphone_compensation", ("--no_headphone_compensation",), "3.0.0"),
)


@dataclass
class Run:
    ok: bool
    seconds: float
    hashes: dict[str, str]
    log: str


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def combined(hashes: dict[str, str]) -> str:
    digest = hashlib.sha256()
    for name in sorted(hashes):
        digest.update(f"{name}\0{hashes[name]}\n".encode())
    return digest.hexdigest()


def run(binary: Path, root: Path, work: Path, args: tuple[str, ...], env: dict[str, str]) -> Run:
    shutil.copytree(root / DEMO, work)
    start = time.perf_counter()
    result = subprocess.run(
        [str(binary), f"--dir_path={work}", f"--test_signal={root / TEST_SIGNAL}", *args],
        env={**os.environ, **env},
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=900,
        check=False,
    )
    seconds = time.perf_counter() - start
    hashes = {str(p.relative_to(work)): sha256(p) for p in sorted(work.rglob("*.wav"))}
    log = (result.stdout + result.stderr)[-4000:]
    # A scenario writes about 100 MB; keep only the hashes.
    shutil.rmtree(work)
    return Run(result.returncode == 0 and bool(hashes), seconds, hashes, log)


def release(text: str) -> tuple[int, int, int] | None:
    """The X.Y.Z of a version string; a pre-release counts as its release."""
    match = re.search(r"(\d+)\.(\d+)\.(\d+)", text)
    return (int(match[1]), int(match[2]), int(match[3])) if match else None


def cli_version(binary: Path) -> tuple[int, int, int] | None:
    result = subprocess.run([str(binary), "--version"], capture_output=True, text=True,
                            encoding="utf-8", errors="replace", timeout=60, check=False)
    return release(result.stdout) if result.returncode == 0 else None


def differing(a: dict[str, str], b: dict[str, str]) -> list[str]:
    return sorted(name for name in a.keys() | b.keys() if a.get(name) != b.get(name))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--base", type=Path, required=True, help="CLI built from the base commit")
    parser.add_argument("--head", type=Path, required=True, help="CLI built from the PR")
    parser.add_argument("--base-root", type=Path, default=PROJECT_ROOT,
                        help="checkout of the base commit; its data/ feeds the base CLI")
    parser.add_argument("--allow-change", action="store_true",
                        help="report a baseline difference as a warning (PR label brir-change)")
    parser.add_argument("--scenario", action="append", default=[],
                        help="run only these scenarios (repeatable)")
    options = parser.parse_args()

    scenarios = [s for s in SCENARIOS if not options.scenario or s[0] in options.scenario]
    if not scenarios:
        parser.error(f"no scenario matches {options.scenario}")

    base_version = cli_version(options.base)
    print(f"base CLI version: {'.'.join(map(str, base_version)) if base_version else 'unknown'}")
    rows = []
    failures = []
    warnings = []
    with tempfile.TemporaryDirectory(prefix="brir-3x-") as tmp:
        root = Path(tmp)
        for name, args, since in scenarios:
            base = run(options.base, options.base_root, root / "base" / name, args, {})
            head = run(options.head, PROJECT_ROOT, root / "head" / name, args, {})
            single = run(options.head, PROJECT_ROOT, root / "single" / name, args,
                         {"RAYON_NUM_THREADS": "1"})
            if not head.ok or not single.ok:
                verdict = "PR run failed"
                failures.append(f"{name}: the PR CLI failed\n{head.log if not head.ok else single.log}")
            elif differing(head.hashes, single.hashes):
                verdict = "depends on threads"
                failures.append(
                    f"{name}: one-thread output differs in {', '.join(differing(head.hashes, single.hashes))}"
                )
            elif not base.ok and base_version is not None and base_version < release(since):
                verdict = "no baseline"
                warnings.append(f"{name}: the base CLI predates this scenario ({since})\n{base.log}")
            elif not base.ok and options.allow_change:
                verdict = "base failed (allowed)"
                warnings.append(f"{name}: the base CLI failed (brir-change label)\n{base.log}")
            elif not base.ok:
                verdict = "BASE FAILED"
                failures.append(f"{name}: the base CLI failed on a scenario it supports since {since}"
                                f"\n{base.log}")
            elif differing(base.hashes, head.hashes):
                files = ", ".join(differing(base.hashes, head.hashes))
                if options.allow_change:
                    verdict = "changed (allowed)"
                    warnings.append(f"{name}: output changed in {files} (brir-change label)")
                else:
                    verdict = "CHANGED"
                    failures.append(f"{name}: output differs from the base in {files}")
            else:
                verdict = "identical"
            rows.append((name, len(head.hashes), combined(base.hashes)[:16] if base.ok else "-",
                         combined(head.hashes)[:16], verdict, base.seconds, head.seconds))
            print(f"{name}: {verdict} (base {base.seconds:.2f}s, PR {head.seconds:.2f}s)", flush=True)
            if verdict != "identical":
                for file in differing(base.hashes, head.hashes) + differing(head.hashes, single.hashes):
                    print(f"  {file}\n    base     {base.hashes.get(file, '-')}"
                          f"\n    PR       {head.hashes.get(file, '-')}"
                          f"\n    1 thread {single.hashes.get(file, '-')}")

    lines = [
        "## 3.x demo BRIR SHA-256 (base vs PR)",
        "",
        "| Scenario | WAVs | Base | PR | Result | Base s | PR s |",
        "|:--|--:|:--|:--|:--|--:|--:|",
        *(f"| {n} | {c} | `{b}` | `{h}` | {v} | {bs:.2f} | {hs:.2f} |" for n, c, b, h, v, bs, hs in rows),
    ]
    summary = "\n".join(lines) + "\n"
    print("\n" + summary)
    if path := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(path, "a", encoding="utf-8") as file:
            file.write(summary)
    for message in warnings:
        print(f"::warning::{message.splitlines()[0]}")
        print(message)
    for message in failures:
        print(f"::error::{message.splitlines()[0]}")
        print(message)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
