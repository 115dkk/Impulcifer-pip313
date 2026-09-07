"""End-to-end gates for the installed P14 wheel (no Python DSP imports)."""
from concurrent.futures import ThreadPoolExecutor
from contextlib import redirect_stderr, redirect_stdout
from importlib.metadata import distribution
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import sysconfig
import threading
import time

import pytest

ROOT = Path(__file__).resolve().parents[3]
SWEEP = "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"


@pytest.fixture
def demo(tmp_path):
    destination = tmp_path / "demo"
    shutil.copytree(ROOT / "data" / "demo", destination)
    # Never let a checked-out/generated output satisfy an assertion.
    for name in ["hesuvi.wav", "hrir.wav"]:
        (destination / name).unlink(missing_ok=True)
    return destination


def config(package, demo):
    return {"dir_path": str(demo), "test_signal": str(Path(package.__file__).parent / "data" / SWEEP)}


def samples(path):
    raw = path.read_bytes()
    assert raw[:4] == b"RIFF" and raw[8:12] == b"WAVE"
    offset = 12
    while offset + 8 <= len(raw):
        tag, size = struct.unpack_from("<4sI", raw, offset)
        body = raw[offset + 8:offset + 8 + size]
        if tag == b"fmt ":
            fmt, channels, rate, _, _, bits = struct.unpack_from("<HHIIHH", body)
            assert fmt in (1, 0xFFFE) and bits == 32
        if tag == b"data":
            frames = [struct.unpack_from("<i", body, i * channels * 4)[0] / 2147483648 for i in range(256)]
            return rate, channels, frames
        offset += 8 + size + size % 2
    raise AssertionError("WAV data chunk missing")


def test_version(package):
    assert package.impulcifer_native.version() == package.__version__ == "3.0.0-alpha.0"
    assert distribution("impulcifer").version == "3.0.0a0"
    # Resolve both sides: the package sets the variable from a resolved path,
    # while __file__ may be the 8.3 short form of a temp directory on Windows.
    assert Path(os.environ["IMPULCIFER_DATA_DIR"]).resolve() == (
        Path(package.__file__).resolve().parent / "data"
    )
    if sysconfig.get_config_var("Py_GIL_DISABLED"):
        assert not sys._is_gil_enabled()


def test_main_runs_demo_and_writes_hesuvi(package, demo):
    output = package.main(**config(package, demo))
    assert Path(output) == demo / "hesuvi.wav"
    rate, channels, actual = samples(Path(output))
    golden = ROOT / "tests" / "migration" / "goldens" / "p11_default.json"
    tracks = json.loads(golden.read_text(encoding="utf-8"))["products"]["hesuvi.wav"]["tracks"]
    expected = next(track["first"] for track in tracks if track["name"] == "FL-left")[:256]
    assert len(expected) == len(actual) == 256
    tolerance = 1e-3 * max(map(abs, expected))
    assert rate == 48000 and channels == 14
    assert max(abs(a - b) for a, b in zip(actual, expected)) <= tolerance


def test_progress_and_log_callbacks_receive_cli_keys(package, demo):
    progress, logs, ids = [], [], []
    caller = threading.get_ident()

    def on_progress(event):
        progress.append(event)
        ids.append(threading.get_ident())

    result = package.impulcifer_native.run(config(package, demo), progress=on_progress, log=logs.append)
    assert set(result) == {"output_path"}
    assert progress and logs and set(ids) == {caller}
    assert all(type(e) is dict and set(e) == {"progress", "message", "key"} for e in progress)
    assert all(type(e) is dict and set(e) == {"level", "message", "key"} for e in logs)
    assert all(0 <= e["progress"] <= 1 for e in progress)
    assert [e["progress"] for e in progress] == sorted(e["progress"] for e in progress)
    assert "cli_writing_brirs" in {e["key"] for e in progress}
    assert "cli_starting_brir_generation" in {e["key"] for e in logs}


def test_error_raises_runtime_error_with_code(package, tmp_path, demo):
    with pytest.raises(RuntimeError, match="FILE_NOT_FOUND: Measurement directory does not exist"):
        package.impulcifer_native.run({"dir_path": str(tmp_path / "missing")})
    with pytest.raises(RuntimeError, match="INVALID_REQUEST:"):
        package.impulcifer_native.run({"dir_path": str(demo), "plot": "invalid"})
    with pytest.raises(RuntimeError, match="INVALID_REQUEST:"):
        package.impulcifer_native.run({"dir_path": str(demo), "head_ms": float("nan")})
    # Fail inside the worker, not just request validation.
    with pytest.raises(RuntimeError, match="[A-Z_]+: .+"):
        package.impulcifer_native.run({"dir_path": str(demo), "test_signal": "absent.wav"})


def test_cli_main_help_exits_zero(package):
    out, err = io.StringIO(), io.StringIO()
    with redirect_stdout(out), redirect_stderr(err):
        assert package.impulcifer_native.cli_main(["--help"]) == 0
        assert package.impulcifer_native.cli_main(["--not-a-real-option"]) == 2
    assert "--dir_path" in out.getvalue()
    assert "unrecognized arguments" in err.getvalue()


def test_cli_entry_point(package, monkeypatch, capsys):
    entry = next(e for e in distribution("impulcifer").entry_points if e.name == "impulcifer")
    assert entry.value == "impulcifer:cli"
    monkeypatch.setattr(sys, "argv", ["impulcifer", "--help"])
    with pytest.raises(SystemExit) as result:
        entry.load()()
    assert result.value.code == 0
    assert "--dir_path" in capsys.readouterr().out


def test_detect_sweep_on_demo(package, demo):
    result = package.detect_sweep(str(demo))
    assert type(result) is dict and result["found"] is True


def test_generate_sweep_set_and_recovery(package, demo, tmp_path):
    generated = package.generate_sweep_set(str(demo))
    assert type(generated) is dict and (demo / "test.wav").is_file()
    package.main(**config(package, demo))
    (demo / "hrir.wav").unlink()
    recovered = package.recover_brir_outputs(str(demo), include_hangloose=True)
    assert recovered["source_kind"] == "hesuvi"
    assert str(demo / "hrir.wav") in recovered["created_files"]
    assert (demo / "Hangloose").is_dir()
    with pytest.raises(RuntimeError, match="INVALID_REQUEST"):
        package.recover_brir_outputs(str(demo), unknown=True)
    empty = tmp_path / "empty"
    empty.mkdir()
    with pytest.raises(RuntimeError, match="NO_RECOVERY_SOURCE"):
        package.recover_brir_outputs(str(empty))


def test_numeric_channel_balance_is_accepted(package, demo):
    # 2.x: impulcifer.main(channel_balance=3) means a +3 dB correction.
    path = package.main(dir_path=str(demo), test_signal="auto", channel_balance=3)
    assert Path(path).name == "hesuvi.wav" and Path(path).is_file()


@pytest.mark.parametrize("decay", [0.35, {"FL": 0.35, "FR": 0.42}])
def test_unknown_objects_and_decay_forms(package, demo, decay):
    circular = []
    circular.append(circular)
    result = package.impulcifer_native.run({**config(package, demo), "decay": decay, "retired": object(), "unknown": circular})
    assert Path(result["output_path"]).is_file()


def test_callback_error_cancels_and_drains_worker(package, demo):
    calls = []

    def fail(event):
        calls.append(event)
        raise LookupError("callback failure")

    with pytest.raises(LookupError, match="callback failure"):
        package.impulcifer_native.run(config(package, demo), log=fail)
    assert len(calls) == 1
    before = {p.name: p.stat().st_mtime_ns for p in demo.iterdir()}
    time.sleep(0.1)
    assert before == {p.name: p.stat().st_mtime_ns for p in demo.iterdir()}
    assert Path(package.main(**config(package, demo))).is_file()


def test_cli_broken_stream_propagates(package, demo):
    class Broken:
        def write(self, text):
            raise OSError("capture stream failed")

    with redirect_stdout(Broken()), pytest.raises(OSError, match="capture stream failed"):
        package.impulcifer_native.cli_main(["--dir_path", str(demo), "--test_signal", config(package, demo)["test_signal"]])


def test_concurrent_runs_release_interpreter(package, demo, tmp_path):
    other = tmp_path / "other"
    shutil.copytree(demo, other)
    barrier = threading.Barrier(2)

    def execute(directory):
        first = True

        def callback(event):
            nonlocal first
            if first:
                first = False
                barrier.wait(timeout=30)

        return package.impulcifer_native.run(config(package, directory), log=callback)

    with ThreadPoolExecutor(max_workers=2) as pool:
        one = pool.submit(execute, demo)
        two = pool.submit(execute, other)
        assert Path(one.result(timeout=60)["output_path"]).is_file()
        assert Path(two.result(timeout=60)["output_path"]).is_file()


def test_package_data_override_is_preserved(package, tmp_path):
    env = os.environ.copy()
    env["IMPULCIFER_DATA_DIR"] = str(tmp_path)
    env["PYTHONIOENCODING"] = "utf-8"
    target = str(Path(package.__file__).parents[1])
    code = f"import sys; sys.path.insert(0, {target!r}); import impulcifer, os; print(os.environ['IMPULCIFER_DATA_DIR'])"
    output = subprocess.check_output([sys.executable, "-c", code], env=env, cwd=tmp_path, encoding="utf-8")
    assert output.strip() == str(tmp_path)
    files = sorted(p.name for p in (Path(package.__file__).parent / "data").iterdir())
    assert len(files) == 9
    assert all(name.startswith(("sweep", "harman")) for name in files)
