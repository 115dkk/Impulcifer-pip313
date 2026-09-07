"""Install a real wheel into a disposable target, never import the 2.x tree."""
import importlib
import os
from pathlib import Path
import subprocess
import sys
import sysconfig
import tempfile

import pytest

CRATE = Path(__file__).resolve().parents[1]
ROOT = CRATE.parents[1]
# Windows retains the loaded .pyd until process exit; do not emit an atexit
# traceback while attempting to unlink it. The OS temp directory owns remnants.
_TARGET = tempfile.TemporaryDirectory(prefix="impulcifer-p14-wheel-", ignore_cleanup_errors=True)
_TAG = "cp314-cp314t" if sysconfig.get_config_var("Py_GIL_DISABLED") else "cp39-abi3"
_WHEEL_DIR = Path(os.environ.get("IMPULCIFER_WHEEL_DIR", ROOT / "target" / "wheels"))
_WHEELS = sorted(_WHEEL_DIR.glob(f"impulcifer-*-{_TAG}-*.whl"), key=lambda p: p.stat().st_mtime)
if not _WHEELS:
    raise RuntimeError(f"Build the {_TAG} wheel first in {_WHEEL_DIR}")
subprocess.run(
    [sys.executable, "-m", "pip", "install", "--no-deps", "--target", _TARGET.name, str(_WHEELS[-1])],
    check=True,
)
sys.path.insert(0, _TARGET.name)
for name in list(sys.modules):
    if name == "impulcifer" or name.startswith("impulcifer."):
        del sys.modules[name]
importlib.invalidate_caches()
# The tests exercise package defaults, not a previously selected checkout data dir.
os.environ.pop("IMPULCIFER_DATA_DIR", None)
impulcifer = importlib.import_module("impulcifer")
assert Path(impulcifer.__file__).is_relative_to(_TARGET.name)
assert Path(impulcifer.impulcifer_native.__file__).suffix in {".pyd", ".so"}


@pytest.fixture(autouse=True)
def isolated_settings(monkeypatch, tmp_path):
    monkeypatch.setenv("USERPROFILE", str(tmp_path))
    monkeypatch.setenv("HOME", str(tmp_path))


@pytest.fixture
def package():
    return impulcifer
