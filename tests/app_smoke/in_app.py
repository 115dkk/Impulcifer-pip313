"""In-app DOM driver, NDJSON reports and independently verified checkpoints."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import traceback

from smoke import ROOT, INPUTS, Smoke, frozen_hashes, load_catalog, real_update_check, wav_info, write_json
from win32_capture import capture, accept_confirmation


class ExternalProcess:
    """A process the harness did not start (the updater relaunched it): the same
    poll/pid/returncode surface the drive loop and cleanup use for Popen."""
    STILL_ACTIVE = 259
    PROCESS_QUERY_LIMITED_INFORMATION = 0x1000

    def __init__(self, pid):
        import ctypes
        self.pid = pid
        self.returncode = None
        self._kernel32 = ctypes.windll.kernel32
        self._handle = self._kernel32.OpenProcess(self.PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
        assert self._handle, f"OpenProcess failed for pid {pid}"

    def poll(self):
        import ctypes
        code = ctypes.c_ulong()
        assert self._kernel32.GetExitCodeProcess(self._handle, ctypes.byref(code))
        if code.value == self.STILL_ACTIVE:
            return None
        self.returncode = code.value
        return self.returncode

    def wait(self, timeout=None):
        deadline = None if timeout is None else time.monotonic() + timeout
        while self.poll() is None:
            if deadline is not None and time.monotonic() >= deadline:
                raise subprocess.TimeoutExpired(f"pid {self.pid}", timeout)
            time.sleep(0.05)
        return self.returncode


def processes_running(exe):
    """(pid, executable path) of every running process whose image is `exe`."""
    script = ("Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath } | "
              "Select-Object ProcessId, ExecutablePath | ConvertTo-Json -Compress")
    result = subprocess.run(["powershell.exe", "-NoProfile", "-Command", script],
                            capture_output=True, timeout=60)
    assert result.returncode == 0, result.stderr
    listed = json.loads(result.stdout.decode("utf-8-sig") or "[]")
    if isinstance(listed, dict):
        listed = [listed]
    wanted = os.path.normcase(str(exe))
    return [(int(p["ProcessId"]), p["ExecutablePath"]) for p in listed
            if os.path.normcase(p["ExecutablePath"]) == wanted]


class InAppSmoke(Smoke):
    def __init__(self, exe, hardware, launch_command=None):
        super().__init__(exe, hardware)
        self.launch_command = launch_command

    def checkpoint(self, record):
        name = record["step"]
        # Driver reports its entire observation buffer at every checkpoint, so a
        # later crash/timeout cannot erase startup errors or earlier diagnostics.
        self.events = record.get("events", self.events)
        result = {"step": name, "id": record["checkpoint"], "ok": True}
        try:
            image = self.output / f"{name}.png"
            result["capture"] = capture(self.process.pid, image)
            shutil.copy2(image, ROOT / "target/app-smoke" / image.name)
            if record.get("status") == "fail":
                raise AssertionError(record.get("error", "driver step failed"))
            if name == "brir":
                outputs = {}
                for filename in ("hesuvi.wav", "README.md"):
                    source = self.demo / filename
                    assert source.is_file() and source.stat().st_size, f"missing {filename}"
                    shutil.copy2(source, self.output / filename)
                    outputs[filename] = source.stat().st_size
                outputs["wav"] = wav_info(self.demo / "hesuvi.wav")
                assert outputs["wav"]["channels"] == 14, outputs
                self.recovery.mkdir()
                shutil.copy2(self.demo / "hesuvi.wav", self.recovery / "hesuvi.wav")
                assert [p.name for p in self.recovery.iterdir()] == ["hesuvi.wav"]
                result["verified"] = outputs
            elif name == "recovery":
                assert record["details"]["ledger_hrir"] == "created"
                info = wav_info(self.recovery / "hrir.wav")
                assert info["channels"] == 16, info
                assert (self.demo / "hesuvi.wav").read_bytes() == (self.recovery / "hesuvi.wav").read_bytes()
                shutil.copy2(self.recovery / "hrir.wav", self.output / "recovered-hrir.wav")
                result["verified"] = info
            elif name in ("settings-ko", "settings-en"):
                code = name.removeprefix("settings-")
                persisted = json.loads((self.home / ".impulcifer/settings.json").read_text(encoding="utf-8"))
                expected = load_catalog(code)["label_select_language"]
                assert persisted["language"] == code and record["label"] == expected, (persisted, record)
                write_json(self.output / f"settings-{code}.json", persisted)
                result["verified"] = {"persisted": code, "label": expected}
            elif name == "recording":
                info = wav_info(self.recording / "headphones.wav")
                assert info["channels"] == 2 and info["sample_rate"] == 48000, info
                assert self.summary["native_confirmations"], "headphone confirmation was not observed"
                shutil.copy2(self.recording / "headphones.wav", self.output / "headphones.wav")
                result["verified"] = info
            elif name == "updates":
                envelope = record["details"]["envelope"]
                assert real_update_check(envelope), envelope
                result["verified"] = {"ok": envelope.get("ok"), "ui_text": record["details"].get("ui_text")}
        except Exception as error:
            result.update(ok=False, error=str(error), traceback=traceback.format_exc())
        if name == "recording-ready":
            self.confirm_pending = True
        self.summary["checkpoints"].append(result)
        ack = self.ack_directory / f"{record['checkpoint']}.json"
        temporary = ack.with_suffix(".tmp")
        write_json(temporary, result)
        temporary.replace(ack)
        write_json(self.output / "events.json", self.events)
        write_json(self.output / "summary.json", self.summary)
        print(json.dumps({"checkpoint": name, **result}, ensure_ascii=False), flush=True)

    def start_app(self, env):
        """Start the app directly, or run the launch command (the updater's apply with
        its normal restart) and attach to the app it relaunches."""
        if not self.launch_command:
            self.process = subprocess.Popen([str(self.exe)], cwd=ROOT, env=env,
                                            stdout=self.app_log, stderr=subprocess.STDOUT)
            return {"pid": self.process.pid, "home": str(self.home), "mode": "in-app"}
        assert not processes_running(self.exe), f"{self.exe} is already running"
        launcher = subprocess.Popen(self.launch_command, cwd=ROOT, env=env,
                                    stdout=self.app_log, stderr=subprocess.STDOUT)
        try:
            launcher_exit = launcher.wait(timeout=300)
        except subprocess.TimeoutExpired:
            launcher.kill()
            raise TimeoutError("launch command did not finish within 300 seconds")
        deadline = time.monotonic() + 120
        while True:
            running = processes_running(self.exe)
            if running:
                break
            assert time.monotonic() < deadline, (
                f"launch command exited with {launcher_exit} but {self.exe} did not start within 120 seconds")
            time.sleep(0.5)
        assert len(running) == 1, f"expected one relaunched app, found {running}"
        pid, path = running[0]
        self.process = ExternalProcess(pid)
        return {"pid": pid, "home": str(self.home), "mode": "in-app",
                "relaunched_by_updater": True, "launcher": self.launch_command,
                "launcher_exit_code": launcher_exit, "executable": path}

    def drive(self):
        assert os.name == "nt", "in-app smoke requires Windows"
        assert self.launch_command or self.exe.is_file(), f"missing executable {self.exe}"
        self.demo.mkdir()
        copied = []
        for name in INPUTS:
            source = ROOT / "data/demo" / name
            if source.is_file():
                shutil.copy2(source, self.demo / name)
                copied.append(name)
        assert (self.demo / "FL,FR.wav").is_file()
        write_json(self.output / "demo-inputs.json", copied)
        self.recording.mkdir()
        self.ack_directory = self.output / "acks"
        self.ack_directory.mkdir()
        report = self.output / "report.ndjson"
        report.touch()
        params = {"demo": str(self.demo), "recovery": str(self.recovery),
                  "recording": str(self.recording), "hardware": self.hardware,
                  "en": self.en,
                  "labels": {code: load_catalog(code)["label_select_language"] for code in ("ko", "en")}}
        param_file = self.output / "params.json"
        write_json(param_file, params)
        env = os.environ.copy()
        for key in ("IMPULCIFER_APP_SMOKE_CDP_PORT", "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"):
            env.pop(key, None)
        env.update(USERPROFILE=str(self.home), HOME=str(self.home),
                   WEBVIEW2_USER_DATA_FOLDER=str(self.home / "webview-data"),
                   IMPULCIFER_APP_SMOKE_ACK_DIR=str(self.ack_directory),
                   IMPULCIFER_APP_SMOKE="1", IMPULCIFER_APP_SMOKE_REPORT=str(report),
                   IMPULCIFER_APP_SMOKE_PARAMS=str(param_file),
                   IMPULCIFER_APP_SMOKE_DRIVER=str(ROOT / "tests/app_smoke/driver.js"))
        launch_started = time.monotonic()
        details = self.start_app(env)
        self.steps.append({"name": "launch", "status": "ok",
                           "duration_seconds": round(time.monotonic() - launch_started, 3),
                           "details": details})
        deadline = time.monotonic() + 300
        pending = ""
        with report.open(encoding="utf-8") as stream:
            while time.monotonic() < deadline:
                assert self.process.poll() is None, f"app exited {self.process.returncode}"
                pending += stream.read()
                while "\n" in pending:
                    line, pending = pending.split("\n", 1)
                    record = json.loads(line)
                    if "checkpoint" in record:
                        self.checkpoint(record)
                    if record.get("result"):
                        self.steps.append({"name": record["step"], **record})
                        print(json.dumps({k: v for k, v in record.items() if k != "ui_log"}, ensure_ascii=False), flush=True)
                    if record["step"] == "done":
                        self.events = record["events"]
                        self.summary["done"] = True
                        self.summary["driver_steps"] = record["steps"]
                        return
                if self.hardware:
                    self.summary["native_confirmations"].extend(accept_confirmation(self.process.pid))
                    if getattr(self, "confirm_pending", False):
                        from win32_capture import windows
                        hwnd = next(h for h, title, _ in windows(self.process.pid) if title == "Impulcifer")
                        confirmation = subprocess.run(["powershell.exe", "-NoProfile", "-File",
                            str(ROOT / "tests/app_smoke/confirm.ps1"), "-WindowHandle", str(hwnd)],
                            capture_output=True, timeout=20)
                        (self.output / "confirmation.log").write_bytes(confirmation.stdout + confirmation.stderr)
                        assert confirmation.returncode == 0, confirmation.stderr
                        result = json.loads(confirmation.stdout.decode("utf-8-sig"))
                        if result["accepted"]:
                            self.summary["native_confirmations"].append(result)
                            self.confirm_pending = False
                time.sleep(0.05)
        raise TimeoutError("in-app driver did not report done within 300 seconds")

    def run(self):
        self.temp = tempfile.TemporaryDirectory(prefix="impulcifer-p15-")
        self.home = Path(self.temp.name)
        self.demo = self.home / "demo"
        self.recovery = self.home / "recovery"
        self.recording = self.home / "recording"
        self.en = load_catalog("en")
        self.summary.update(mode="in-app", checkpoints=[], native_confirmations=[])
        write_json(self.output / "frozen-before.json", self.before)
        try:
            with (self.output / "app.log").open("wb") as self.app_log:
                self.drive()
        except Exception:
            self.summary["harness_error"] = traceback.format_exc()
            print(self.summary["harness_error"], flush=True)
        finally:
            self.step("cleanup", self.cleanup)
            after = frozen_hashes()
            write_json(self.output / "frozen-after.json", after)
            write_json(self.output / "events.json", self.events)
            write_json(ROOT / "target/app-smoke/events.json", self.events)
            errors = [e for e in self.events if e["kind"] in ("pageerror", "resource-error") or
                      (e["kind"] == "console" and e.get("failed", e["level"] == "error"))]
            for step in self.steps:
                step["console_errors"] = [event for event in errors if event.get("step") == step["name"]]
                step["error_envelopes"] = [event for event in self.events
                    if event.get("step") == step["name"] and event["kind"] == "ipc-response"
                    and event["response"].get("ok") is False]
            required = {"launch", "bootstrap", "brir", "recovery", "settings", "updates"}
            if self.hardware:
                required.add("recording")
            passed = {s["name"] for s in self.steps if s["status"] == "ok"}
            self.summary.update(frozen_unchanged=self.before == after, console_errors=errors,
                                duration_seconds=round(time.monotonic() - self.started, 3))
            self.summary["ok"] = (self.summary.get("done", False) and not self.summary.get("harness_error")
                                  and not errors and self.before == after and required <= passed
                                  and all(s["status"] != "fail" for s in self.steps)
                                  and all(c["ok"] for c in self.summary["checkpoints"])
                                  and all(s["status"] != "fail" for s in self.summary.get("driver_steps", [])))
            for path in (self.output / "summary.json", ROOT / "target/app-smoke/summary.json"):
                write_json(path, self.summary)
            write_json(self.output.parent / "latest.json", {"summary": str(self.output / "summary.json")})
            print(f"SMOKE_SUMMARY {self.output / 'summary.json'}", flush=True)
            print(f"SMOKE_OK {self.summary['ok']}", flush=True)
        return 0 if self.summary["ok"] else 1
