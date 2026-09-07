"""PA01 whole-call oracle and shared, system-temp fixtures (foreground only)."""
from __future__ import annotations

import sys
sys.dont_write_bytecode = True

import argparse
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

import numpy as np
import scipy
import soundfile as sf
from core.audio_io import read_wav, write_wav
from core.sweep_signal import _quantize_like_bundled_wav
from core.recording_progress import _SEGMENTED_SWEEP_RE

SWEEP = ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
DEMO = ROOT / "data/demo/FL,FR.wav"
NAME = "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"


def input_data(channels, frames):
    state = 7
    data = np.empty(channels * frames, dtype=np.float64)
    for i in range(data.size):
        state = (1664525 * state + 1013904223) & 0xffffffff
        data[i] = state / 2147483648.0 - 1.0
    return data.reshape(channels, frames)


def save_expected(directory, name, data):
    data = np.ascontiguousarray(data, dtype="<f8")
    data.tofile(directory / f"{name}.f64")
    checksum = int(data.view(np.uint64).sum(dtype=np.uint64))
    print(f"fixture {name}: {data.shape}, checksum={checksum:016x}", flush=True)


def fixtures(directory):
    # Caller must supply a directory below system temp; never create fixtures in the repo.
    directory = directory.resolve()
    if not directory.is_relative_to(Path(tempfile.gettempdir()).resolve()):
        raise ValueError("fixture directory must be under system temp")
    directory.mkdir(parents=True, exist_ok=True)
    a, b, c = input_data(32, 96000), input_data(30, 96000), input_data(2, 295000)
    for name, data in [("input32", a), ("input30", b), ("input2", c)]:
        save_expected(directory, name, data)
    write_wav(str(directory / "pcm32.wav"), 48000, a, bit_depth=32)
    float_data = input_data(8, 480000)
    sf.write(directory / "float32.wav", float_data.T, 48000, subtype="FLOAT")
    for name, path in [("pcm32", directory / "pcm32.wav"), ("sweep", SWEEP), ("demo", DEMO), ("float32", directory / "float32.wav")]:
        data, rate = sf.read(path, always_2d=True)
        assert rate == 48000
        save_expected(directory, f"{name}-expected", data.T)
    save_expected(directory, "roundtrip-expected", _quantize_like_bundled_wav(b, 48000))
    return a, b, c


def measure(op, size, function):
    for _ in range(3):
        result = function()
        del result
    times = []
    for _ in range(11):
        start = time.perf_counter()
        result = function()
        times.append((time.perf_counter() - start) * 1000)
        del result
    median, minimum = statistics.median(times), min(times)
    print(f"| {op} | {size} | {median:.6f} | {minimum:.6f} |", flush=True)
    return median


def environment():
    print(f"OS={platform.platform()} CPU={platform.processor()} logical_cores={os.cpu_count()}")
    if os.name == "nt":
        subprocess.run(["powershell.exe", "-NoProfile", "-Command", "Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors | Format-List"], check=True)
    subprocess.run(["rustc", "--version"], check=True)
    print(f"Python={sys.version} numpy={np.__version__} scipy={scipy.__version__} soundfile={sf.__version__} libsndfile={sf.__libsndfile_version__}")
    print("FFT: numpy/scipy pocketfft; no FFT in measured IO operations")
    print("thread env:", {key: os.environ.get(key, "<unset>") for key in ["OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "BLIS_NUM_THREADS", "NUMEXPR_NUM_THREADS", "RAYON_NUM_THREADS"]})
    np.show_config()


def run(directory, arrays):
    a, b, c = arrays
    print("| op | size | python median ms | python min ms |", flush=True)
    medians = {}
    for op, data, bits in [("write_pcm32_32tracks", a, 32), ("write_pcm32_30tracks", b, 32), ("write_pcm16_2tracks", c, 16)]:
        medians[op] = measure(op, f"{data.shape[0]}x{data.shape[1]}", lambda: write_wav(str(directory / "python-output.wav"), 48000, data, bit_depth=bits))
    for op, path in [("read_pcm32_32tracks", directory / "pcm32.wav"), ("read_bundled_sweep", SWEEP), ("read_demo_recording", DEMO), ("read_float32_8tracks", directory / "float32.wav")]:
        info = sf.info(path)
        call = (lambda: sf.read(path)) if op == "read_float32_8tracks" else (lambda: read_wav(str(path), expand=True))
        medians[op] = measure(op, f"{info.channels}x{info.frames}", call)
    medians["pcm32_round_trip"] = measure("pcm32_round_trip", "30x96000", lambda: _quantize_like_bundled_wav(b, 48000))
    names = [NAME for _ in range(10000)]
    # Actual 2.x regex, not a reimplementation. It parses speakers/layout/duration
    # only, whereas Rust additionally parses and validates rate/bits/frequencies.
    medians["sweep_file_name_parse"] = measure("sweep_file_name_parse", "10000", lambda: [_SEGMENTED_SWEEP_RE.search(name) for name in names])
    return medians


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture-only", type=Path)
    args = parser.parse_args()
    if args.fixture_only:
        fixtures(args.fixture_only)
        return
    environment()
    print("PCM32 oracle is actual BytesIO sf.write/read, not np.int32 cast.")
    print("Parse comparator: actual recording_progress._SEGMENTED_SWEEP_RE.search; partial fields, nonidentical to full Rust parser.")
    with tempfile.TemporaryDirectory(prefix="impulcifer-pa01-") as tmp:
        directory = Path(tmp)
        arrays = fixtures(directory)
        env = dict(os.environ, IMPULCIFER_PERF_DIR=str(directory), PYTHONDONTWRITEBYTECODE="1")
        command = ["cargo", "bench", "-p", "impulcifer-io", "--bench", "perf"]
        result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, check=True)
        print(result.stderr, end="", flush=True)
        print(result.stdout, end="", flush=True)
        rust = {fields[1].strip(): float(fields[3]) for line in result.stdout.splitlines() if line.startswith("| ") and not line.startswith("| op ") for fields in [line.split("|")]}
        python = run(directory, arrays)
        print("| op | python/rust median |", flush=True)
        for op, median in python.items():
            print(f"| {op} | {median / rust[op]:.6f} |", flush=True)


if __name__ == "__main__":
    main()
