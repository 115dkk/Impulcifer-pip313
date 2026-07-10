"""Free-threaded BRIR output determinism checks.

On a free-threaded (no-GIL) runtime ``core.parallel_utils`` switches the
heavy pipeline stages to ``ThreadPoolExecutor``. A data race in any worker
would show up as run-to-run differences in the generated WAV bytes — the
classic heisenbug that a single test run cannot catch. This test generates
the demo BRIR several times in the same environment and requires every
produced ``.wav`` to be byte-identical (SHA-256) across runs, so repeated CI
executions accumulate chances to trap an intermittent race.

Opt-in via ``IMPULCIFER_RUN_BRIR_THREAD_DETERMINISM=1``; CI runs it in a
dedicated job on the free-threaded interpreter. Unlike
``test_brir_integrity`` this needs no reference ref — the comparison is
between repeated runs of the same code, so it is platform-agnostic by
construction (only the free-threaded runtime is required).
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

from tests.test_brir_integrity import (
    PROJECT_ROOT,
    SCENARIOS,
    _copy_demo_inputs,
    _required_paths,
    _run_impulcifer,
    _sha256_file,
)


RUN_ENV_VAR = "IMPULCIFER_RUN_BRIR_THREAD_DETERMINISM"
RUNS_ENV_VAR = "IMPULCIFER_BRIR_DETERMINISM_RUNS"
DEFAULT_RUNS = 3


def _is_free_threaded() -> bool:
    is_gil_enabled = getattr(sys, "_is_gil_enabled", None)
    return is_gil_enabled is not None and not is_gil_enabled()


def _hash_wav_outputs(demo_dir: Path) -> dict:
    """SHA-256 of every top-level .wav (inputs are identical copies, so
    including them costs nothing and catches accidental input mutation)."""
    return {path.name: _sha256_file(path) for path in sorted(demo_dir.glob("*.wav"))}


@pytest.mark.slow
@pytest.mark.skipif(
    os.environ.get(RUN_ENV_VAR) != "1",
    reason=f"set {RUN_ENV_VAR}=1 to run the BRIR thread-determinism test",
)
@pytest.mark.skipif(
    not _is_free_threaded(),
    reason="requires a free-threaded (no-GIL) runtime so the ThreadPoolExecutor path is active",
)
@pytest.mark.parametrize("scenario", SCENARIOS, ids=[scenario.name for scenario in SCENARIOS])
def test_brir_output_is_deterministic_under_threading(tmp_path, scenario) -> None:
    """Repeated demo runs on the threaded path must be byte-identical."""
    missing = [str(path) for path in _required_paths(PROJECT_ROOT) if not path.exists()]
    if missing:
        pytest.skip(f"demo integrity inputs are missing: {missing}")

    runs = max(2, int(os.environ.get(RUNS_ENV_VAR, str(DEFAULT_RUNS))))
    run_hashes = []
    for run_index in range(1, runs + 1):
        demo_dir = tmp_path / f"run-{run_index}" / scenario.name
        _copy_demo_inputs(PROJECT_ROOT, demo_dir)
        _run_impulcifer(PROJECT_ROOT, demo_dir, scenario)
        run_hashes.append(_hash_wav_outputs(demo_dir))

    baseline = run_hashes[0]
    assert "hesuvi.wav" in baseline, f"{scenario.name}: run 1 produced no hesuvi.wav"

    for run_index, current in enumerate(run_hashes[1:], start=2):
        if current == baseline:
            continue
        changed = sorted(
            name
            for name in set(baseline) | set(current)
            if baseline.get(name) != current.get(name)
        )
        details = "\n".join(
            f"  {name}: {baseline.get(name, '<absent>')} -> {current.get(name, '<absent>')}"
            for name in changed
        )
        pytest.fail(
            f"{scenario.name}: non-deterministic output under free-threaded "
            f"parallelism — run 1 vs run {run_index} differ in {changed}\n{details}"
        )
