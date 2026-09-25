"""PEP 517 backend: maturin's hooks, with each built wheel's RECORD rewritten as CSV.

pip builds wheels from the sdist (and from a checkout) through these hooks, so
they get the same repair as the release workflow (see repair_record.py). A
direct `maturin build` bypasses them, which is why the workflows and verify.py
run repair_record.py after it.
"""
import os
from pathlib import Path

from maturin import (  # noqa: F401  (maturin's own hooks, re-exported unchanged)
    build_editable,
    build_sdist,
    get_requires_for_build_editable,
    get_requires_for_build_sdist,
    get_requires_for_build_wheel,
    prepare_metadata_for_build_editable,
    prepare_metadata_for_build_wheel,
)
from maturin import build_wheel as _maturin_build_wheel
from repair_record import repair, verify

# maturin warns that pip will not use it unless build-backend is "maturin";
# every hook here runs maturin, so the warning would be wrong.
os.environ.setdefault("MATURIN_NO_MISSING_BUILD_BACKEND_WARNING", "1")


def build_wheel(wheel_directory, config_settings=None, metadata_directory=None):
    name = _maturin_build_wheel(wheel_directory, config_settings, metadata_directory)
    wheel = Path(wheel_directory) / name
    repair(wheel)
    problems = verify(wheel)
    if problems:
        raise RuntimeError(f"{name}: RECORD does not match the wheel: {problems}")
    return name
