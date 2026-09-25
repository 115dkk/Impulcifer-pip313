"""Unit tests for crates/impulcifer-python/repair_record.py and build_backend.py.

maturin writes wheel RECORD rows without CSV quoting, so the bundled
``sweep-seg-FL,FR-stereo-...wav`` gets a row that CSV readers split in the
wrong place and PyPI reports the wheel's contents as not matching RECORD. The
workflows rewrite RECORD with repair_record.py after ``maturin build``, and pip
builds (from the sdist or a checkout) go through build_backend.py, which wraps
maturin's hooks. These tests pin both on synthetic wheels written the way
maturin writes them.
"""

from __future__ import annotations

import base64
import csv
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import sys
import types
import zipfile

import pytest

_CRATE = Path(__file__).resolve().parent.parent / "crates" / "impulcifer-python"
_spec = importlib.util.spec_from_file_location("repair_record", _CRATE / "repair_record.py")
repair_record = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(repair_record)

COMMA = "impulcifer/data/sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
RECORD = "impulcifer_py313-3.0.0.dist-info/RECORD"
FILES = {
    "impulcifer/__init__.py": b"print('init')\n",
    COMMA: b"RIFF comma",
    "impulcifer/data/sweep-seg-FL-mono-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav": b"RIFF mono",
    "impulcifer/impulcifer_native.abi3.so": b"\x7fELF native",
    "impulcifer_py313-3.0.0.dist-info/WHEEL": b"Wheel-Version: 1.0\n",
}


def _hash(data):
    return "sha256=" + base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()


def _maturin_wheel(path, files=FILES):
    """A wheel whose RECORD is written the way maturin 1.15.0 writes it (no quoting)."""
    lines = "".join(f"{name},{_hash(data)},{len(data)}\n" for name, data in files.items()) + f"{RECORD},,\n"
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as archive:
        for name, data in files.items():
            info = zipfile.ZipInfo(name, (2026, 9, 24, 12, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = (0o100755 if name.endswith(".so") else 0o100644) << 16
            archive.writestr(info, data)
        archive.writestr(zipfile.ZipInfo(RECORD, (2026, 9, 24, 12, 0, 0)), lines)
    return path


def _record_rows(path):
    with zipfile.ZipFile(path) as archive:
        return list(csv.reader(io.StringIO(archive.read(RECORD).decode())))


def test_maturin_record_does_not_parse_as_csv(tmp_path):
    wheel = _maturin_wheel(tmp_path / "a.whl")
    problems = repair_record.verify(wheel)
    assert f"{COMMA} is missing from RECORD" in problems
    assert any(problem.startswith("RECORD row with 4 fields") for problem in problems)


def test_repair_quotes_the_comma_row_and_keeps_every_other_byte(tmp_path):
    wheel = _maturin_wheel(tmp_path / "a.whl")
    with zipfile.ZipFile(wheel) as archive:
        before = {info.filename: (archive.read(info), info.date_time, info.external_attr) for info in archive.infolist()}

    assert repair_record.repair(wheel) is True
    assert repair_record.verify(wheel) == []

    with zipfile.ZipFile(wheel) as archive:
        after = {info.filename: (archive.read(info), info.date_time, info.external_attr) for info in archive.infolist()}
        text = archive.read(RECORD).decode()
    assert list(after) == list(before)
    assert {name: value for name, value in after.items() if name != RECORD} == {
        name: value for name, value in before.items() if name != RECORD
    }
    assert f'"{COMMA}",{_hash(FILES[COMMA])},{len(FILES[COMMA])}\n' in text
    assert "impulcifer/__init__.py," in text and '"impulcifer/__init__.py"' not in text
    assert [row[0] for row in _record_rows(wheel)] == [*FILES, RECORD]
    assert not list(tmp_path.glob("*.partial"))


def test_repair_keeps_the_modification_time(tmp_path):
    """The wheel tests pick the newest wheel; a rewrite must not make a stale one newest."""
    wheel = _maturin_wheel(tmp_path / "a.whl")
    os.utime(wheel, ns=(1_000_000_000_000_000_000, 1_000_000_000_000_000_000))
    assert repair_record.repair(wheel) is True
    assert wheel.stat().st_mtime_ns == 1_000_000_000_000_000_000


def test_repair_is_idempotent(tmp_path):
    wheel = _maturin_wheel(tmp_path / "a.whl")
    repair_record.repair(wheel)
    first = wheel.read_bytes()
    assert repair_record.repair(wheel) is False
    assert wheel.read_bytes() == first


def test_a_wheel_without_commas_is_left_alone(tmp_path):
    files = {name: data for name, data in FILES.items() if name != COMMA}
    wheel = _maturin_wheel(tmp_path / "a.whl", files)
    original = wheel.read_bytes()
    assert repair_record.repair(wheel) is False
    assert wheel.read_bytes() == original
    assert repair_record.verify(wheel) == []


def test_verify_reports_a_hash_that_does_not_match(tmp_path):
    wheel = _maturin_wheel(tmp_path / "a.whl")
    repair_record.repair(wheel)
    tampered = tmp_path / "b.whl"
    with zipfile.ZipFile(wheel) as source, zipfile.ZipFile(tampered, "w") as target:
        for info in source.infolist():
            data = b"changed" if info.filename == "impulcifer/__init__.py" else source.read(info)
            target.writestr(info.filename, data)
    assert repair_record.verify(tampered) == ["impulcifer/__init__.py does not match its RECORD hash or size"]


def test_main_fails_on_a_mismatch_and_on_an_empty_directory(tmp_path, capsys):
    (tmp_path / "good").mkdir()
    good = _maturin_wheel(tmp_path / "good" / "a.whl")
    assert repair_record.main([str(good.parent)]) == 0
    assert "RECORD rewritten, matches the wheel" in capsys.readouterr().out

    bad = tmp_path / "bad.whl"
    with zipfile.ZipFile(good) as source, zipfile.ZipFile(bad, "w") as target:
        for info in source.infolist():
            if info.filename != "impulcifer/__init__.py":
                target.writestr(info.filename, source.read(info))
    assert repair_record.main([str(bad)]) == 1
    assert "RECORD lists impulcifer/__init__.py, which is not in the wheel" in capsys.readouterr().out

    empty = tmp_path / "empty"
    empty.mkdir()
    with pytest.raises(SystemExit, match="No wheels found"):
        repair_record.main([str(empty)])


HOOKS = [
    "build_editable",
    "build_sdist",
    "get_requires_for_build_sdist",
    "get_requires_for_build_wheel",
    "prepare_metadata_for_build_wheel",
]
WARNING = "MATURIN_NO_MISSING_BUILD_BACKEND_WARNING"


def test_build_backend_repairs_the_wheel_pip_builds(tmp_path, monkeypatch):
    name = "impulcifer_py313-3.0.0-cp39-abi3-linux_x86_64.whl"
    maturin = types.ModuleType("maturin")
    maturin.build_wheel = lambda directory, config=None, metadata=None: _maturin_wheel(Path(directory) / name).name
    for hook in HOOKS:
        setattr(maturin, hook, lambda *args, hook=hook: hook)
    monkeypatch.setitem(sys.modules, "maturin", maturin)
    monkeypatch.setitem(sys.modules, "repair_record", repair_record)
    monkeypatch.setenv(WARNING, "")  # restored (unset) after the test
    monkeypatch.delenv(WARNING)
    spec = importlib.util.spec_from_file_location("build_backend", _CRATE / "build_backend.py")
    backend = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(backend)

    assert backend.build_wheel(str(tmp_path)) == name
    assert repair_record.verify(tmp_path / name) == []
    assert all(getattr(backend, hook) is getattr(maturin, hook) for hook in HOOKS)
    assert os.environ[WARNING] == "1"


def test_pyproject_builds_through_the_repairing_backend():
    tomllib = pytest.importorskip("tomllib")
    config = tomllib.loads((_CRATE / "pyproject.toml").read_text(encoding="utf-8"))
    assert config["build-system"]["build-backend"] == "build_backend"
    assert config["build-system"]["backend-path"] == ["."]
    # The sdist moves pyproject.toml to its root, so the backend files must be there too.
    sdist = set()
    for entry in config["tool"]["maturin"]["include"]:
        formats = [entry["format"]] if isinstance(entry["format"], str) else entry["format"]
        if "sdist" in formats:
            sdist.add(entry["path"])
    assert {"build_backend.py", "repair_record.py"} <= sdist
