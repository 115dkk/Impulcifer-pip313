"""PA04 direct 2.x service calls; no oracle production source modifications.

Three warmup batches and eleven measured batches. Rows are per call, pair,
or job (five jobs per batch). Worker coordination and cleanup are untimed.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import platform
import queue
import shutil
import statistics
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
OPS = [("bootstrap", 200), ("get_ui_settings", 200),
       ("set_language_round_trip", 100), ("resolve_recording_paths", 200),
       ("start_brir_to_first_event", 5), ("poll_job_drain", 5),
       ("job_event_emit", 5), ("detect_sweep", 20), ("catalog_translate", 100000)]
MANIFEST = ["FL,FR.wav", "FC.wav", "BL,SL.wav", "SR,BR.wav", "headphones.wav", "room.wav",
            "room-BL,SL-left.wav", "room-BL,SL-right.wav", "room-FC-left.wav", "room-FC-right.wav",
            "room-FL,FR-left.wav", "room-FL,FR-right.wav", "room-SR,BR-left.wav", "room-SR,BR-right.wav"]


def environment():
    import numpy as np
    import psutil
    import scipy
    import scipy.fft._basic_backend as fft_backend

    print("executable", sys.executable, "version", sys.version)
    print("GIL", sys._is_gil_enabled())
    print("OS", platform.platform(), "processor", platform.processor())
    print("cores physical/logical", psutil.cpu_count(logical=False), psutil.cpu_count())
    if sys.platform == "win32":
        import winreg
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
            print("CPU", winreg.QueryValueEx(key, "ProcessorNameString")[0])
    print("numpy", np.__version__, "scipy", scipy.__version__, "FFT", fft_backend.__file__)
    print("threads", {k: os.environ.get(k) for k in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "NUMEXPR_NUM_THREADS", "RAYON_NUM_THREADS")})
    np.show_config()


def join_workers():
    # Real _start_job threads are named by production code; join after terminal
    # state too, so neither language leaves workers alive between samples.
    for thread in threading.enumerate():
        if thread.name.startswith("impulcifer-") or thread.name == "dsp-prewarm":
            thread.join(timeout=60)
            assert not thread.is_alive(), thread.name


class Fixture:
    def __init__(self, temp):
        from application.impulcifer_service import ImpulciferApplicationService
        from i18n.localization import get_localization_manager

        self.temp = Path(temp)
        self.service = ImpulciferApplicationService()
        self.loc = get_localization_manager()
        assert self.service.get_ui_settings()["data"]["language"] == "en"
        self.service.bootstrap()
        join_workers()

    def start(self):
        with tempfile.TemporaryDirectory(prefix="demo-", dir=self.temp) as directory:
            for name in MANIFEST:
                source = ROOT / "data/demo" / name
                if source.is_file():
                    shutil.copyfile(source, Path(directory) / name)
            request = {"dir_path": directory,
                       "test_signal": str(ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")}
            start = time.perf_counter()
            response = self.service.start_brir(request)
            job_id = response["data"]["job"]["job_id"]
            while True:
                first = self.service.poll_job(job_id, 0)
                if first["data"]["events"]:
                    break
            ms = (time.perf_counter() - start) * 1000
            assert first["data"]["events"][0]["seq"] == 1
            assert first["data"]["events"][0]["type"] == "status"
            self.service.cancel_job(job_id)
            join_workers()
            assert self.service.poll_job(job_id)["data"]["job"]["status"] in ("cancelled", "succeeded")
            return ms

    def drain(self):
        ready, ack = queue.Queue(), queue.Queue()

        def target(job_id, _cancel):
            for end in range(500, 2001, 500):
                for i in range(end - 500, end):
                    self.service._emit(job_id, "log", {"level": "INFO", "message": "PA04 event", "index": i})
                ready.put(None)
                ack.get(timeout=30)
            return {}

        job_id = self.service._start_job("brir", False, target)["data"]["job"]["job_id"]
        seq = logs = 0
        ms = 0.0
        for _ in range(4):
            ready.get(timeout=30)
            start = time.perf_counter()
            value = self.service.poll_job(job_id, seq)
            ms += (time.perf_counter() - start) * 1000
            assert value["ok"]
            for event in value["data"]["events"]:
                assert event["seq"] == seq + 1
                seq += 1
                if event["type"] == "log":
                    assert event["payload"]["index"] == logs
                    assert event["payload"]["message"] == "PA04 event"
                    logs += 1
            assert value["data"]["next_seq"] == seq
            ack.put(None)
        join_workers()
        assert logs == 2000
        final = self.service.poll_job(job_id, seq)
        assert len(final["data"]["events"]) == 1
        assert final["data"]["job"]["status"] == "succeeded"
        return ms

    def emit(self):
        result = queue.Queue()

        def target(job_id, _cancel):
            start = time.perf_counter()
            for i in range(10000):
                self.service._emit(job_id, "log", {"level": "INFO", "message": "PA04 event", "index": i})
            result.put((time.perf_counter() - start) * 1000)
            return {}

        job_id = self.service._start_job("brir", False, target)["data"]["job"]["job_id"]
        ms = result.get(timeout=30)
        join_workers()
        value = self.service.poll_job(job_id)["data"]
        assert value["next_seq"] == 10002 and len(value["events"]) == 2000
        assert value["events"][-2]["payload"]["index"] == 9999
        return ms

    def sample(self, op, calls):
        jobs = {"start_brir_to_first_event": self.start, "poll_job_drain": self.drain, "job_event_emit": self.emit}
        if op in jobs:
            return sum(jobs[op]() for _ in range(calls)) / calls
        demo = str(ROOT / "data/demo")
        args = {"date": "2026-09-08", "fs": 48000}
        start = time.perf_counter()
        for _ in range(calls):
            if op == "bootstrap":
                value = self.service.bootstrap()
            elif op == "get_ui_settings":
                value = self.service.get_ui_settings()
            elif op == "set_language_round_trip":
                self.service.set_language("ko")
                value = self.service.set_language("en")
            elif op == "resolve_recording_paths":
                value = self.service.resolve_recording_paths(demo, "FL,FR.wav")
            elif op == "detect_sweep":
                value = self.service.detect_sweep(demo)
            elif op == "catalog_translate":
                value = self.loc.get("cli_readme_processed").format(**args)
            else:
                raise AssertionError(op)
        ms = (time.perf_counter() - start) * 1000 / calls
        if op == "catalog_translate":
            assert value == "Processed on 2026-09-08. Output sampling rate is 48000 Hz."
        else:
            assert value["ok"], value
            data = value["data"]
            if op == "detect_sweep":
                assert data["found"] and data["fs"] == 48000
            elif op == "resolve_recording_paths":
                assert data["record_path"].endswith("FL,FR.wav")
            else:
                ui = data["ui"] if op == "bootstrap" else data
                assert ui["language"] == "en" and isinstance(ui["strings"]["cli_readme_processed"], str)
        return ms


def main():
    environment()
    with tempfile.TemporaryDirectory(prefix="impulcifer-pa04-") as temp:
        # Scope real preferences to the temporary profile BEFORE importing the
        # localization singleton. No fake settings methods or patched oracle.
        old = {k: os.environ.get(k) for k in ("HOME", "USERPROFILE")}
        try:
            for key in old:
                os.environ[key] = temp
            settings = Path(temp) / ".impulcifer"
            settings.mkdir()
            (settings / "settings.json").write_text(json.dumps({"language": "en", "language_selected": True}), encoding="utf-8")
            fixture = Fixture(temp)
            print("GIL after production imports", sys._is_gil_enabled())
            print("| op | size | python median ms | python min ms |", flush=True)
            for op, calls in OPS:
                values = []
                for i in range(14):
                    ms = fixture.sample(op, calls)
                    print(f"PA04_RAW {op} {i} {ms:.9f}", flush=True)
                    if i >= 3:
                        values.append(ms)
                print(f"| {op} | {calls}/batch; per unit | {statistics.median(values):.9f} | {min(values):.9f} |", flush=True)
        finally:
            join_workers()
            for key, value in old.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value


if __name__ == "__main__":
    main()
