"""End-to-end BRIR output integrity checks.

The test is opt-in because it runs the full demo processing pipeline. CI enables
it in a dedicated job so the expensive baseline check runs once instead of once
per Python-version matrix entry.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEMO_SOURCE_DIR = PROJECT_ROOT / "data" / "demo"
TEST_SIGNAL_PATH = (
    PROJECT_ROOT / "data" / "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"
)
BASELINE_HESUVI_MD5 = "d295982d021a6d16ab2c194c3517c162"
RUN_ENV_VAR = "IMPULCIFER_RUN_BRIR_INTEGRITY"
TIMEOUT_ENV_VAR = "IMPULCIFER_BRIR_INTEGRITY_TIMEOUT"
CANONICAL_PLATFORM = "linux"
CANONICAL_PYTHON = (3, 13)


def _md5_file(path: Path) -> str:
    digest = hashlib.md5()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _tail(text: str, max_chars: int = 4000) -> str:
    return text[-max_chars:] if len(text) > max_chars else text


def _copy_demo_inputs(destination: Path) -> None:
    shutil.copytree(
        DEMO_SOURCE_DIR,
        destination,
        ignore=shutil.ignore_patterns(
            "hesuvi.wav",
            "hrir.wav",
            "responses.wav",
            "jamesdsp.wav",
            "plots",
            "interactive_plots",
            "Hangloose",
        ),
    )


@pytest.mark.slow
@pytest.mark.skipif(
    os.environ.get(RUN_ENV_VAR) != "1",
    reason=f"set {RUN_ENV_VAR}=1 to run the BRIR integrity test",
)
@pytest.mark.skipif(
    sys.platform != CANONICAL_PLATFORM or sys.version_info[:2] != CANONICAL_PYTHON,
    reason="BRIR md5 is canonicalized to Linux CPython 3.13",
)
def test_demo_brir_matches_baseline_md5(tmp_path: Path) -> None:
    """The canonical demo BRIR should stay bit-identical to the baseline."""
    required_paths = [
        DEMO_SOURCE_DIR,
        TEST_SIGNAL_PATH,
        DEMO_SOURCE_DIR / "headphones.wav",
        DEMO_SOURCE_DIR / "FL,FR.wav",
        DEMO_SOURCE_DIR / "FC.wav",
        DEMO_SOURCE_DIR / "BL,SL.wav",
        DEMO_SOURCE_DIR / "SR,BR.wav",
    ]
    missing = [str(path) for path in required_paths if not path.exists()]
    if missing:
        pytest.skip(f"demo integrity inputs are missing: {missing}")

    demo_dir = tmp_path / "demo"
    _copy_demo_inputs(demo_dir)

    env = os.environ.copy()
    env.setdefault("MPLBACKEND", "Agg")
    timeout = int(env.get(TIMEOUT_ENV_VAR, "600"))

    command = [
        sys.executable,
        str(PROJECT_ROOT / "impulcifer.py"),
        f"--dir_path={demo_dir}",
        f"--test_signal={TEST_SIGNAL_PATH}",
        "--vbass",
        "--vbass_freq=250",
    ]
    result = subprocess.run(
        command,
        cwd=PROJECT_ROOT,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        check=False,
    )

    assert result.returncode == 0, (
        "demo BRIR generation failed\n"
        f"stdout tail:\n{_tail(result.stdout)}\n"
        f"stderr tail:\n{_tail(result.stderr)}"
    )

    hesuvi_path = demo_dir / "hesuvi.wav"
    assert hesuvi_path.is_file(), "demo BRIR generation did not create hesuvi.wav"
    actual_md5 = _md5_file(hesuvi_path)
    assert actual_md5 == BASELINE_HESUVI_MD5, (
        f"expected demo hesuvi.wav md5 {BASELINE_HESUVI_MD5}, got {actual_md5}"
    )
