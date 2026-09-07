"""PA03 sequential whole-process and in-process audit; no DSP/plot shortcuts.

Default: build release example, Rust 1 warmup + 5 measurements per scenario,
each Python interpreter/mode 1 warmup + 3 measurements. Raw JSON stays in a
system temporary result directory printed at startup. --label distinguishes
baseline/final runs; --only permits natural filtering of long foreground runs.
"""
from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SIGNAL = ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
INPUTS = (
    "FL,FR.wav", "FC.wav", "BL,SL.wav", "SR,BR.wav", "headphones.wav",
    "room.wav", "room-BL,SL-left.wav", "room-BL,SL-right.wav",
    "room-FC-left.wav", "room-FC-right.wav", "room-FL,FR-left.wav",
    "room-FL,FR-right.wav", "room-SR,BR-left.wav", "room-SR,BR-right.wav",
    "room-target.csv", "room-mic-calibration.csv", "room-mic-calibration.txt",
    "eq.csv", "eq.txt", "eq-left.csv", "eq-left.txt", "eq-right.csv", "eq-right.txt",
)
THREAD_ENV = ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS",
              "BLIS_NUM_THREADS", "NUMEXPR_NUM_THREADS", "RAYON_NUM_THREADS")


def environment():
    import numpy as np
    import scipy
    import scipy.fft
    import psutil
    from core.parallel_utils import get_parallelization_info
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        np.show_config()
    cpu = platform.processor()
    if sys.platform == "win32":
        import winreg
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,
                            r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
            cpu = winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()
    return dict(executable=sys.executable, python=sys.version, numpy=np.__version__,
                scipy=scipy.__version__, numpy_config=out.getvalue(),
                fft="NumPy/SciPy pocketfft", scipy_fft_workers=scipy.fft.get_workers(),
                parallel=get_parallelization_info(), eq_workers=min(os.cpu_count() or 4, 14),
                threads={k: os.environ.get(k) for k in THREAD_ENV},
                os=platform.platform(), cpu=cpu, logical=os.cpu_count(),
                physical=psutil.cpu_count(logical=False), psutil=psutil.__version__)


def inprocess(directory, vbass):
    import impulcifer
    from core.pipeline import BRIRPipeline
    from core.parallel_utils import get_parallelization_info
    import core.parallel_utils as parallel_utils
    pools = []
    original_pool = parallel_utils._run_parallel_map

    def observed_pool(func, items, max_workers, **kwargs):
        threads = kwargs.get("use_threads", False) or parallel_utils.is_gil_disabled()
        pools.append(dict(tasks=len(items), max_workers=max_workers,
                          function=f"{func.__module__}.{func.__qualname__}",
                          use_threads=kwargs.get("use_threads", False),
                          executor="ThreadPoolExecutor" if threads else "ProcessPoolExecutor"))
        return original_pool(func, items, max_workers, **kwargs)

    parallel_utils._run_parallel_map = observed_pool
    # Observe each actual method, including zero-progress writes and real plots.
    stages = []
    for name in dir(BRIRPipeline):
        if not name.startswith("_stage_") or name == "_stage_table":
            continue
        original = getattr(BRIRPipeline, name)

        def observed(self, _original=original, _name=name):
            start = time.perf_counter()
            result = _original(self)
            stages.append(dict(key=_name, ms=(time.perf_counter() - start) * 1000))
            return result

        setattr(BRIRPipeline, name, observed)
    start = time.perf_counter()
    impulcifer.main(dir_path=str(directory), test_signal=str(SIGNAL), vbass=vbass, vbass_freq=250)
    seconds = time.perf_counter() - start
    print("PA03_RESULT " + json.dumps(dict(seconds=seconds, stages=stages,
                                          parallel=get_parallelization_info(), pools=pools)))


def sample_child(command, log):
    import psutil
    peaks = {}
    known = {}
    identities = {}
    concurrent = 0
    parent_key = None
    start = time.perf_counter()
    with log.open("wb") as stream:
        child = subprocess.Popen(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT)
        parent = psutil.Process(child.pid)
        parent_key = (parent.pid, parent.create_time())
        known[parent_key] = parent

        def sample():
            nonlocal concurrent
            try:
                for proc in parent.children(recursive=True):
                    known[(proc.pid, proc.create_time())] = proc
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass
            rss_sum = 0
            for key, proc in list(known.items()):
                try:
                    if not proc.is_running():
                        continue
                    if key not in identities:
                        identities[key] = dict(exe=proc.exe(), command=proc.cmdline(), ppid=proc.ppid())
                    info = proc.memory_info()
                    peak = max(info.rss, getattr(info, "peak_wset", 0))
                    peaks[key] = max(peaks.get(key, 0), peak)
                    rss_sum += info.rss
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass
            concurrent = max(concurrent, rss_sum)

        while True:
            sample()
            try:
                child.wait(timeout=0.05)
                sample()
                break
            except subprocess.TimeoutExpired:
                pass
        seconds = time.perf_counter() - start
    text = log.read_text(encoding="utf-8", errors="replace")
    if child.returncode:
        raise RuntimeError(f"{command} failed ({child.returncode}):\n{text}")
    results = [json.loads(line.removeprefix("PA03_RESULT "))
               for line in text.splitlines() if line.startswith("PA03_RESULT ")]
    return dict(process_seconds=seconds, result=results[-1] if results else None,
                parent_peak_bytes=peaks.get(parent_key, 0),
                sum_historical_peaks_bytes=sum(peaks.values()),
                concurrent_tree_rss_bytes=concurrent, descendants=len(known) - 1,
                process_peaks=[dict(pid=k[0], created=k[1], peak_bytes=v, **identities.get(k, {}))
                               for k, v in peaks.items()],
                workload_process_peak_bytes=max(peaks.values(), default=0))


def orchestrate(args):
    # Only this process orchestrates. Spawned EQ workers import this module but
    # never enter this function (safe __main__ guard).
    result_dir = Path(tempfile.mkdtemp(prefix=f"impulcifer-pa03-{args.label}-"))
    print(f"RAW_DIRECTORY {result_dir}", flush=True)
    subprocess.run(["cargo", "build", "--release", "-p", "impulcifer-service",
                    "--example", "demo_brir"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    rust = Path(metadata["target_directory"]) / "release/examples/demo_brir.exe"
    regular = json.loads(subprocess.check_output(
        ["py", "-3.14", "-c", "import sys,json; print(json.dumps(sys.executable))"], text=True))
    free = Path(os.environ["LOCALAPPDATA"]) / "impulcifer-bench/py314t/Scripts/python.exe"
    interpreters = {"python314": regular, "python314t": str(free)}
    env = {"rust": subprocess.check_output(["rustc", "-Vv"], text=True),
           "cargo": subprocess.check_output(["cargo", "-V"], text=True)}
    for name, exe in interpreters.items():
        text = subprocess.check_output([exe, str(Path(__file__)), "--environment"],
                                       cwd=ROOT, text=True, encoding="utf-8")
        env[name] = json.loads(text.split("PA03_ENV ")[-1])
    (result_dir / "environment.json").write_text(json.dumps(env, indent=2), encoding="utf-8")
    print("PA03_ENV " + json.dumps(env), flush=True)
    rows = []
    for scenario in ("default", "vbass"):
        for side in ("rust", "python314", "python314t"):
            if args.only and side != args.only:
                continue
            for mode in (("service",) if side == "rust" else ("cli", "inprocess")):
                for iteration in range(6 if side == "rust" else 4):
                    with tempfile.TemporaryDirectory(prefix="impulcifer-pa03-input-") as directory:
                        directory = Path(directory)
                        for name in sorted(INPUTS):
                            if (ROOT / "data/demo" / name).is_file():
                                shutil.copy2(ROOT / "data/demo" / name, directory / name)
                        if side == "rust":
                            command = [str(rust), str(directory)]
                            if scenario == "vbass":
                                command.append("--vbass")
                        elif mode == "cli":
                            command = [interpreters[side], str(ROOT / "impulcifer.py"),
                                       f"--dir_path={directory}", f"--test_signal={SIGNAL}"]
                            if scenario == "vbass":
                                command += ["--vbass", "--vbass_freq=250"]
                        else:
                            command = [interpreters[side], str(Path(__file__)),
                                       "--inprocess", str(directory)]
                            if scenario == "vbass":
                                command.append("--vbass")
                        log = result_dir / f"{scenario}-{side}-{mode}-{iteration}.log"
                        row = sample_child(command, log)
                        row.update(scenario=scenario, side=side, mode=mode,
                                   iteration=iteration, warmup=iteration == 0,
                                   command=command, log=str(log))
                        if side != "rust":
                            row["outputs"] = sorted(str(p.relative_to(directory))
                                                    for p in directory.rglob("*")
                                                    if p.is_file() and p.name not in INPUTS)
                            for required in ("hrir.wav", "hesuvi.wav", "responses.wav", "README.md"):
                                if not (directory / required).is_file():
                                    raise RuntimeError(f"missing output {required}")
                        rows.append(row)
                        (result_dir / "rows.json").write_text(json.dumps(rows, indent=2), encoding="utf-8")
                        print("PA03_RAW " + json.dumps(row), flush=True)
    print("| scenario | side | mode | process median ms | process min ms | in-process median ms | root peak MiB | largest workload process peak MiB | sum historical peaks MiB | concurrent tree RSS MiB |", flush=True)
    for scenario in ("default", "vbass"):
        for side in ("rust", "python314", "python314t"):
            for mode in ("service", "cli", "inprocess"):
                selected = [r for r in rows if not r["warmup"] and
                            (r["scenario"], r["side"], r["mode"]) == (scenario, side, mode)]
                if not selected:
                    continue
                median = statistics.median(r["process_seconds"] for r in selected) * 1000
                minimum = min(r["process_seconds"] for r in selected) * 1000
                inner = statistics.median(r["result"]["seconds"] for r in selected) * 1000 if selected[0]["result"] else 0
                memory = [max(r[key] for r in selected) / 1024**2 for key in
                          ("parent_peak_bytes", "workload_process_peak_bytes",
                           "sum_historical_peaks_bytes", "concurrent_tree_rss_bytes")]
                print(f"| {scenario} | {side} | {mode} | {median:.6f} | {minimum:.6f} | {inner:.6f} | " + " | ".join(f"{v:.3f}" for v in memory) + " |", flush=True)
    print("| scenario | comparator | Python CLI / Rust process | Python main / Rust service |", flush=True)
    for scenario in ("default", "vbass"):
        rust_rows = [r for r in rows if not r["warmup"] and r["scenario"] == scenario and r["side"] == "rust"]
        if not rust_rows:
            continue
        for side in interpreters:
            cli = [r for r in rows if not r["warmup"] and (r["scenario"], r["side"], r["mode"]) == (scenario, side, "cli")]
            inner = [r for r in rows if not r["warmup"] and (r["scenario"], r["side"], r["mode"]) == (scenario, side, "inprocess")]
            if cli and inner:
                ratio = statistics.median(r["process_seconds"] for r in cli) / statistics.median(r["process_seconds"] for r in rust_rows)
                inner_ratio = statistics.median(r["result"]["seconds"] for r in inner) / statistics.median(r["result"]["seconds"] for r in rust_rows)
                print(f"| {scenario} | {side} | {ratio:.6f} | {inner_ratio:.6f} |", flush=True)
    print(f"RAW_DIRECTORY {result_dir}", flush=True)


def main():
    sys.path.insert(0, str(ROOT))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--label", default="baseline")
    parser.add_argument("--only", choices=("rust", "python314", "python314t"))
    parser.add_argument("--environment", action="store_true")
    parser.add_argument("--inprocess", type=Path)
    parser.add_argument("--vbass", action="store_true")
    args = parser.parse_args()
    if args.environment:
        print("PA03_ENV " + json.dumps(environment()))
    elif args.inprocess:
        inprocess(args.inprocess, args.vbass)
    else:
        orchestrate(args)


if __name__ == "__main__":
    main()
