#!/usr/bin/env python3
"""Check the SOFA files of the demo with the readers players use (ADR 0019).

``brir-3x.yml`` runs this after the integrity comparison with the PR's release
CLI. It processes a fresh copy of ``data/demo`` with ``--sofa
--sofa_split_headphone_eq`` and then requires, for ``brir.sofa`` and
``brir-no-headphone-eq.sofa``:

* libmysofa (``mysofa2json -c`` from the distribution's libmysofa-utils) reads
  the file and opens it the way players do;
* pyfar ``sofar`` reads it with ``verify=True`` (the AES69 rules, through
  netCDF-C and libhdf5);
* the content follows docs/rust/SOFA.md: SimpleFreeFieldHRIR, the demo's seven
  speakers at their nominal directions and 2 m, 48 kHz, two ears.

``headphone-eq.wav`` must exist next to them. Local use, with a venv that has
``sofar`` and with ``mysofa2json`` on PATH::

    python .github/scripts/check_sofa.py --cli target/release/impulcifer
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import sofar

PROJECT_ROOT = Path(__file__).resolve().parents[2]
DEMO = PROJECT_ROOT / "data" / "demo"
TEST_SIGNAL = PROJECT_ROOT / "data" / "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
# The demo's speakers in SLOTS order with their nominal directions.
EXPECTED_POSITIONS = np.array(
    [[30, 0, 2], [-30, 0, 2], [0, 0, 2], [135, 0, 2], [-135, 0, 2], [90, 0, 2], [-90, 0, 2]],
    dtype=float,
)


def check_file(path: Path, mysofa2json: str, headphone_included: bool) -> list[str]:
    problems = []
    result = subprocess.run(
        [mysofa2json, "-c", str(path)], capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        problems.append(f"{path.name}: libmysofa refused it ({result.returncode}): {result.stderr.strip()}")
    try:
        sofa = sofar.read_sofa(str(path), verify=True)
    except Exception as error:  # noqa: BLE001 - report every reader failure
        return problems + [f"{path.name}: pyfar sofar refused it: {error}"]
    if sofa.GLOBAL_SOFAConventions != "SimpleFreeFieldHRIR" or sofa.GLOBAL_DataType != "FIR":
        problems.append(f"{path.name}: convention {sofa.GLOBAL_SOFAConventions}/{sofa.GLOBAL_DataType}")
    if sofa.GLOBAL_RoomType != "free field":
        problems.append(f"{path.name}: RoomType {sofa.GLOBAL_RoomType!r}")
    included = "Headphone compensation is included" in sofa.GLOBAL_Comment
    if included != headphone_included:
        problems.append(f"{path.name}: Comment does not say whether headphone compensation is included: {sofa.GLOBAL_Comment!r}")
    if sofa.Data_IR.shape[:2] != (len(EXPECTED_POSITIONS), 2):
        problems.append(f"{path.name}: Data.IR shape {sofa.Data_IR.shape}")
    if not np.array_equal(np.asarray(sofa.SourcePosition), EXPECTED_POSITIONS):
        problems.append(f"{path.name}: SourcePosition {np.asarray(sofa.SourcePosition).tolist()}")
    if float(np.asarray(sofa.Data_SamplingRate).ravel()[0]) != 48000.0:
        problems.append(f"{path.name}: sampling rate {sofa.Data_SamplingRate}")
    if not np.all(np.isfinite(sofa.Data_IR)) or not np.any(sofa.Data_IR):
        problems.append(f"{path.name}: Data.IR is empty or not finite")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--cli", type=Path, required=True, help="the impulcifer CLI to run")
    parser.add_argument("--mysofa2json", default=shutil.which("mysofa2json") or "mysofa2json")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as temp:
        work = Path(temp) / "demo"
        shutil.copytree(DEMO, work)
        result = subprocess.run(
            [str(args.cli), f"--dir_path={work}", f"--test_signal={TEST_SIGNAL}",
             "--sofa", "--sofa_split_headphone_eq"],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=900, check=False,
        )
        if result.returncode != 0:
            print((result.stdout + result.stderr)[-4000:])
            print("::error::the CLI failed on the SOFA scenario")
            return 1
        problems = []
        for name, included in (("brir.sofa", True), ("brir-no-headphone-eq.sofa", False)):
            path = work / name
            if not path.is_file():
                problems.append(f"{name} was not saved")
                continue
            problems += check_file(path, args.mysofa2json, included)
            print(f"{name}: {path.stat().st_size} bytes checked")
        if not (work / "headphone-eq.wav").is_file():
            problems.append("headphone-eq.wav was not saved")
    for problem in problems:
        print(f"::error::{problem}")
    if not problems:
        print("SOFA files open in libmysofa and pass pyfar sofar's verification.")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
