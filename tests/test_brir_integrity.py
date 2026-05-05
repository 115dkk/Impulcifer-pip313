"""End-to-end BRIR output integrity checks.

The test is opt-in because it runs the full demo processing pipeline. CI enables
it in a dedicated job so the expensive check runs once instead of once per
Python-version matrix entry.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import pytest


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEMO_SOURCE_DIR = PROJECT_ROOT / "data" / "demo"
TEST_SIGNAL_PATH = (
    PROJECT_ROOT / "data" / "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"
)
RUN_ENV_VAR = "IMPULCIFER_RUN_BRIR_INTEGRITY"
REFERENCE_REF_ENV_VAR = "IMPULCIFER_BRIR_REFERENCE_REF"
TIMEOUT_ENV_VAR = "IMPULCIFER_BRIR_INTEGRITY_TIMEOUT"
CANONICAL_PLATFORM = "linux"
CANONICAL_PYTHON = (3, 13)


@dataclass(frozen=True)
class BrirScenario:
    name: str
    extra_args: tuple[str, ...]


SCENARIOS = [
    BrirScenario(
        name="default_headphone_compensation",
        extra_args=(),
    ),
    BrirScenario(
        name="virtual_bass",
        extra_args=("--vbass", "--vbass_freq=250"),
    ),
]


def _md5_file(path: Path) -> str:
    digest = hashlib.md5()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _tail(text: str, max_chars: int = 4000) -> str:
    return text[-max_chars:] if len(text) > max_chars else text


def _copy_demo_inputs(project_root: Path, destination: Path) -> None:
    shutil.copytree(
        project_root / "data" / "demo",
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


def _required_paths(project_root: Path) -> list[Path]:
    demo_dir = project_root / "data" / "demo"
    return [
        demo_dir,
        project_root / "data" / "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl",
        demo_dir / "headphones.wav",
        demo_dir / "FL,FR.wav",
        demo_dir / "FC.wav",
        demo_dir / "BL,SL.wav",
        demo_dir / "SR,BR.wav",
    ]


def _run_impulcifer(project_root: Path, demo_dir: Path, scenario: BrirScenario) -> str:
    env = os.environ.copy()
    env.setdefault("MPLBACKEND", "Agg")
    timeout = int(env.get(TIMEOUT_ENV_VAR, "600"))
    test_signal_path = project_root / "data" / "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"

    command = [
        sys.executable,
        str(project_root / "impulcifer.py"),
        f"--dir_path={demo_dir}",
        f"--test_signal={test_signal_path}",
        *scenario.extra_args,
    ]
    result = subprocess.run(
        command,
        cwd=project_root,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        check=False,
    )

    assert result.returncode == 0, (
        f"{scenario.name} BRIR generation failed in {project_root}\n"
        f"command: {' '.join(command)}\n"
        f"stdout tail:\n{_tail(result.stdout)}\n"
        f"stderr tail:\n{_tail(result.stderr)}"
    )

    hesuvi_path = demo_dir / "hesuvi.wav"
    assert hesuvi_path.is_file(), (
        f"{scenario.name} BRIR generation did not create {hesuvi_path}"
    )
    return _md5_file(hesuvi_path)


def _add_reference_worktree(tmp_path: Path) -> Path:
    reference_ref = os.environ.get(REFERENCE_REF_ENV_VAR)
    if not reference_ref:
        pytest.skip(f"set {REFERENCE_REF_ENV_VAR} to compare against a verified ref")

    reference_root = tmp_path / "reference"
    result = subprocess.run(
        ["git", "worktree", "add", "--detach", str(reference_root), reference_ref],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    assert result.returncode == 0, (
        f"failed to create reference worktree for {reference_ref}\n"
        f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
    return reference_root


@pytest.mark.slow
@pytest.mark.skipif(
    os.environ.get(RUN_ENV_VAR) != "1",
    reason=f"set {RUN_ENV_VAR}=1 to run the BRIR integrity test",
)
@pytest.mark.skipif(
    sys.platform != CANONICAL_PLATFORM or sys.version_info[:2] != CANONICAL_PYTHON,
    reason="BRIR md5 is canonicalized to Linux CPython 3.13",
)
@pytest.mark.parametrize("scenario", SCENARIOS, ids=[scenario.name for scenario in SCENARIOS])
def test_demo_brir_matches_reference_ref_md5(
    tmp_path: Path,
    scenario: BrirScenario,
) -> None:
    """The canonical demo BRIR should match the verified reference ref."""
    missing = [str(path) for path in _required_paths(PROJECT_ROOT) if not path.exists()]
    if missing:
        pytest.skip(f"demo integrity inputs are missing: {missing}")

    reference_root = _add_reference_worktree(tmp_path)
    try:
        reference_missing = [
            str(path) for path in _required_paths(reference_root) if not path.exists()
        ]
        if reference_missing:
            pytest.skip(f"reference demo integrity inputs are missing: {reference_missing}")

        current_demo_dir = tmp_path / "current" / scenario.name
        reference_demo_dir = tmp_path / "reference-output" / scenario.name
        _copy_demo_inputs(PROJECT_ROOT, current_demo_dir)
        _copy_demo_inputs(reference_root, reference_demo_dir)

        reference_md5 = _run_impulcifer(reference_root, reference_demo_dir, scenario)
        current_md5 = _run_impulcifer(PROJECT_ROOT, current_demo_dir, scenario)

        assert current_md5 == reference_md5, (
            f"{scenario.name} hesuvi.wav md5 changed from verified reference "
            f"{reference_md5} to {current_md5}"
        )
    finally:
        subprocess.run(
            ["git", "worktree", "remove", "--force", str(reference_root)],
            cwd=PROJECT_ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
