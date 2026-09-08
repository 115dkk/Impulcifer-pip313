"""Local Windows M3 check of the real release app, not a mock browser page.

Requires Python 3.14; --cdp additionally requires playwright (no browser download).
Every action goes through the frozen UI. IPC instrumentation is observation only.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
import traceback
import urllib.error
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
INPUTS = (
    "FL,FR.wav", "FC.wav", "BL,SL.wav", "SR,BR.wav", "headphones.wav", "room.wav",
    "room-BL,SL-left.wav", "room-BL,SL-right.wav", "room-FC-left.wav", "room-FC-right.wav",
    "room-FL,FR-left.wav", "room-FL,FR-right.wav", "room-SR,BR-left.wav",
    "room-SR,BR-right.wav", "room-target.csv", "room-mic-calibration.csv",
    "room-mic-calibration.txt", "eq.csv", "eq.txt", "eq-left.csv", "eq-left.txt",
    "eq-right.csv", "eq-right.txt",
)
UPDATE_ERROR = {
    "ok": False,
    "error": {"code": "INTERNAL_ERROR", "message": "check_for_updates not implemented",
              "details": {}, "retryable": False},
}


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def frozen_hashes():
    return {p.relative_to(ROOT).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for name in ("webview_ui", "i18n")
            for p in sorted((ROOT / name).rglob("*")) if p.is_file()}


def wav_info(path):
    """Read RIFF metadata including IEEE float / extensible WAV without decoding."""
    with path.open("rb") as stream:
        header = stream.read(12)
        assert header[:4] == b"RIFF" and header[8:] == b"WAVE", "not RIFF/WAVE"
        info = {}
        while chunk := stream.read(8):
            assert len(chunk) == 8, "truncated RIFF chunk"
            kind, size = struct.unpack("<4sI", chunk)
            start = stream.tell()
            assert start + size <= path.stat().st_size, "truncated RIFF data"
            if kind == b"fmt ":
                tag, channels, rate, _, align, bits = struct.unpack("<HHIIHH", stream.read(16))
                info.update(format_tag=tag, channels=channels, sample_rate=rate,
                            block_align=align, bits=bits)
            elif kind == b"data":
                info["data_bytes"] = size
            stream.seek(start + size + size % 2)
    assert info.get("block_align", 0) and info.get("data_bytes", 0), "empty WAV"
    info["frames"] = info["data_bytes"] // info["block_align"]
    info["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    return info


class Skipped(Exception):
    pass


class Smoke:
    def __init__(self, exe, hardware):
        self.exe = exe
        self.hardware = hardware
        # Each invocation is retained, and hardware/nonhardware never overwrite each other.
        mode = "hardware" if hardware else "nonhardware"
        self.output = ROOT / "target" / "app-smoke" / mode / time.strftime("%Y%m%d-%H%M%S")
        self.output = self.output.with_name(f"{self.output.name}-{os.getpid()}")
        self.output.mkdir(parents=True)
        self.page = None
        self.process = None
        self.steps = []
        self.events = []
        self.cdp_events = []
        self.observer_index = 0
        self.current_step = "launch"
        self.started = time.monotonic()
        self.before = frozen_hashes()
        self.summary = {"exe": str(exe), "hardware": hardware, "output": str(self.output),
                        "steps": self.steps, "started_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
                        "python": sys.version,
                        "exe_sha256": hashlib.sha256(exe.read_bytes()).hexdigest() if exe.is_file() else None}
        self.ready = False

    def collect(self):
        if self.page is None or self.page.is_closed():
            return
        batch = self.page.evaluate("""index => {
            const observer = window.__impulciferSmoke;
            return observer ? observer.events.slice(index) : null;
        }""", self.observer_index)
        if batch is None:
            raise AssertionError("startup observer absent: rebuild the P15 release app")
        self.observer_index += len(batch)
        for event in batch:
            event["observed_step"] = self.current_step
        self.events.extend(batch)

    def screenshot(self, name):
        path = self.output / f"{name}.png"
        self.page.screenshot(path=str(path), full_page=True)
        shutil.copy2(path, ROOT / "target" / "app-smoke" / path.name)
        return str(path)

    def step(self, name, action, reason=None):
        self.current_step = name
        start = time.monotonic()
        event_start = len(self.events)
        cdp_start = len(self.cdp_events)
        result = {"name": name, "status": "ok", "details": {}}
        try:
            if reason:
                raise Skipped(reason)
            result["details"] = action() or {}
        except Skipped as error:
            result.update(status="skipped", error=str(error))
        except Exception as error:
            result.update(status="fail", error=str(error), traceback=traceback.format_exc())
        finally:
            if self.page is not None and not self.page.is_closed():
                try:
                    self.collect()
                    result["screenshot"] = self.screenshot(name)
                    result["ui_log"] = self.page.locator("[data-log]").first.text_content()
                except Exception as error:
                    result.update(status="fail", evidence_error=str(error))
            errors = [e for e in self.events[event_start:]
                      if e["kind"] in ("pageerror", "resource-error") or
                      (e["kind"] == "console" and e["level"] in ("error", "assert"))]
            cdp_errors = [e for e in self.cdp_events[cdp_start:] if e["kind"] in
                          ("pageerror", "resource-error") or e.get("level") == "error"]
            result["console_errors"] = errors
            result["cdp_errors"] = cdp_errors
            result["error_envelopes"] = [e for e in self.events[event_start:]
                                         if e["kind"] == "ipc-response" and
                                         e["response"].get("ok") is False]
            if errors or cdp_errors:
                result["status"] = "fail"
            result["duration_seconds"] = round(time.monotonic() - start, 3)
            self.steps.append(result)
            write_json(self.output / "summary.json", self.summary)
            print(json.dumps(result, ensure_ascii=False), flush=True)
        return result["status"] == "ok"

    def navigate(self, view):
        self.page.locator(f'#nav [data-view="{view}"]').click()
        self.page.locator(f"#view-{view}.active").wait_for(state="visible")

    def observe_page(self, page):
        def event(kind, **detail):
            self.cdp_events.append({"kind": kind, "step": self.current_step,
                                    "time": time.time(), "url": page.url, **detail})
        page.on("console", lambda message: event("console", level=message.type,
                                                 text=message.text, location=message.location))
        page.on("pageerror", lambda error: event("pageerror", text=str(error)))
        page.on("requestfailed", lambda request: event("resource-error", text=request.failure,
                                                       resource=request.url))
        page.on("response", lambda response: event("resource-error", text=str(response.status),
                                                   resource=response.url)
                if response.status >= 400 else None)
        page.on("dialog", lambda dialog: (event("dialog", type=dialog.type, text=dialog.message),
                                          dialog.accept()))

    def launch(self):
        assert sys.platform == "win32", "WebView2 smoke requires Windows"
        assert self.exe.is_file(), f"release executable missing: {self.exe}"
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        env = os.environ.copy()
        for key in ("IMPULCIFER_APP_SMOKE_DRIVER", "IMPULCIFER_APP_SMOKE_PARAMS", "IMPULCIFER_APP_SMOKE_ACK_DIR"):
            env.pop(key, None)
        env.update(USERPROFILE=str(self.home), HOME=str(self.home),
                   WEBVIEW2_USER_DATA_FOLDER=str(self.home / "webview-data"),
                   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}",
                   # wry overrides the environment variable above with its own
                   # AdditionalBrowserArguments; the shell reads this one instead.
                   IMPULCIFER_APP_SMOKE_CDP_PORT=str(port),
                   IMPULCIFER_APP_SMOKE="1")
        self.process = subprocess.Popen([str(self.exe)], cwd=ROOT, env=env,
                                        stdout=self.app_log, stderr=subprocess.STDOUT)
        endpoint = f"http://127.0.0.1:{port}"
        deadline = time.monotonic() + 30
        while True:
            assert self.process.poll() is None, f"app exited: {self.process.returncode}"
            try:
                with urllib.request.urlopen(endpoint + "/json/version", timeout=1) as response:
                    self.summary["cdp_version"] = json.load(response)
                break
            except (urllib.error.URLError, TimeoutError, ConnectionError):
                if time.monotonic() >= deadline:
                    raise TimeoutError("WebView2 CDP did not start within 30 seconds")
                time.sleep(0.1)
        from playwright.sync_api import sync_playwright
        self.playwright = sync_playwright().start()
        browser = self.playwright.chromium.connect_over_cdp(endpoint, timeout=10000)
        self.browser = browser
        deadline = time.monotonic() + 30
        seen = set()
        for context in browser.contexts:
            context.on("page", self.observe_page)
        while time.monotonic() < deadline:
            for context in browser.contexts:
                for page in context.pages:
                    if page not in seen:
                        self.observe_page(page)
                        seen.add(page)
                    if page.url.startswith(("http://tauri.localhost", "https://tauri.localhost",
                                            "tauri://localhost")):
                        self.page = page
                        page.set_default_timeout(10000)
                        page.wait_for_function("!!window.__impulciferSmoke")
                        self.collect()
                        return {"pid": self.process.pid, "url": page.url, "port": port,
                                "home": str(self.home), "webview_data": str(self.home / "webview-data")}
            time.sleep(0.1)
        raise TimeoutError("actual Tauri app page was not found")

    def bootstrap(self):
        self.page.locator("#language-modal:not([hidden])").wait_for(state="visible")
        # Fresh settings must show the actual first-run UI, not seeded preferences.
        self.page.locator("#language-modal-list button").filter(has_text="English").click()
        self.page.locator("#language-modal").wait_for(state="hidden")
        self.navigate("settings")
        self.page.locator("#sf-skin").select_option("studio")
        self.page.wait_for_function("document.documentElement.dataset.skin === 'studio'")
        self.navigate("processing")
        self.page.wait_for_function("!document.querySelector('#btn-generate-brir').disabled")
        self.collect()
        boots = [e for e in self.events if e["kind"] == "ipc-response" and e["method"] == "bootstrap"]
        assert len(boots) == 1 and boots[0]["response"]["ok"], "bootstrap must succeed once"
        self.ready = True
        return {"runtime": self.page.locator("#runtime-status").inner_text(),
                "bootstrap_observed": True, "reloads": 0}

    def wait_job(self, view, kind, method, event_start):
        self.collect()
        deadline = time.monotonic() + 120
        terminal = {self.en[f"webview_status_{s}"]: s for s in ("succeeded", "failed", "cancelled")}
        expected_prefix = self.en[kind] + " · "
        while time.monotonic() < deadline:
            self.collect()
            rejected = [e for e in self.events[event_start:] if e["kind"] == "ipc-response"
                        and e["method"] == method and not e["response"]["ok"]]
            # The UI logs start validation errors instead of making a job.
            if rejected:
                text = self.page.locator("[data-log]").first.text_content()
                message = rejected[-1]["response"]["error"]["message"]
                if message in text:
                    raise AssertionError(f"UI rejected {method}: {rejected[-1]['response']}")
            status = self.page.locator(f"#view-{view} [data-job-state]").inner_text()
            if status.startswith(expected_prefix) and status[len(expected_prefix):] in terminal:
                outcome = terminal[status[len(expected_prefix):]]
                assert outcome == "succeeded", f"UI terminal state: {status}"
                return status
            self.page.wait_for_timeout(100)
        raise TimeoutError(f"{method} did not reach a terminal UI state within 120 seconds")

    def brir(self):
        self.demo.mkdir()
        copied = []
        for name in INPUTS:
            source = ROOT / "data" / "demo" / name
            if source.is_file():
                shutil.copy2(source, self.demo / name)
                copied.append(name)
        assert (self.demo / "FL,FR.wav").is_file(), "demo FL reference missing"
        write_json(self.output / "demo-inputs.json", copied)
        self.navigate("processing")
        self.page.locator("#bf-dir-path").fill(str(self.demo))
        self.page.locator("#bf-test-signal-source").select_option("auto")
        self.collect()
        event_start = len(self.events)
        self.page.locator("#btn-generate-brir").click()
        status = self.wait_job("processing", "sidebar_processing", "start_brir", event_start)
        assert self.page.locator("#brir-steps li.done").count() == 10
        outputs = {}
        for name in ("hesuvi.wav", "README.md"):
            path = self.demo / name
            assert path.is_file() and path.stat().st_size > 0, f"missing output: {name}"
            outputs[name] = {"bytes": path.stat().st_size,
                             "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
            shutil.copy2(path, self.output / name)
        return {"status": status, "outputs": outputs, "test_signal": "auto"}

    def recovery(self):
        source = self.demo / "hesuvi.wav"
        if not source.is_file():
            raise Skipped("BRIR did not produce hesuvi.wav; no substitute recovery fixture")
        target = self.home / "recovery"
        target.mkdir()
        shutil.copy2(source, target / "hesuvi.wav")
        assert [p.name for p in target.iterdir()] == ["hesuvi.wav"]
        self.navigate("recovery")
        self.page.locator("#recovery-dir-path").fill(str(target))
        self.page.locator("#btn-start-recovery").click()
        self.page.wait_for_function("""() => ['succeeded','failed','cancelled'].includes(
            document.querySelector('.recovery-result').dataset.status)""", timeout=120000)
        title = self.page.locator("#recovery-status-title").inner_text()
        assert title == self.en["webview_status_succeeded"], self.page.locator("#recovery-status-detail").inner_text()
        assert (target / "hrir.wav").is_file(), "recovery did not write hrir.wav"
        assert source.read_bytes() == (target / "hesuvi.wav").read_bytes(), "recovery modified source"
        info = wav_info(target / "hrir.wav")
        shutil.copy2(target / "hrir.wav", self.output / "recovered-hrir.wav")
        return {"title": title, "hrir": info}

    def language(self, code):
        self.page.locator("#sf-language").select_option(code)
        expected = json.loads((ROOT / "i18n" / "locales" / f"{code}.json").read_text(encoding="utf-8"))["label_select_language"]
        self.page.wait_for_function("""expected => document.querySelector(
            'label[for="sf-language"]').textContent === expected""", arg=expected)
        settings = json.loads((self.home / ".impulcifer" / "settings.json").read_text(encoding="utf-8"))
        assert settings["language"] == code, settings
        write_json(self.output / f"settings-{code}.json", settings)
        self.screenshot(f"settings-{code}")
        return {"language": code, "label": expected, "persisted": settings["language"]}

    def settings(self):
        self.navigate("settings")
        try:
            ko = self.language("ko")
        finally:
            en = self.language("en")
        return {"ko": ko, "en": en}

    def updates(self):
        self.navigate("info")
        self.collect()
        event_start = len(self.events)
        self.page.locator("#btn-check-updates").click()
        expected = "INTERNAL_ERROR: check_for_updates not implemented"
        self.page.wait_for_function("expected => document.querySelector('#update-check-status').textContent === expected", arg=expected)
        self.collect()
        responses = [e["response"] for e in self.events[event_start:] if e["kind"] == "ipc-response"
                     and e["method"] == "check_for_updates"]
        assert responses and all(r == UPDATE_ERROR for r in responses), responses
        return {"ui_text": expected, "expected_envelope": responses[-1]}

    def recording(self):
        if not self.hardware:
            raise Skipped("requires --hardware and the documented CABLE-A pair")
        self.navigate("recorder")
        chosen = {}
        for element, prefix in (("rf-output-device", "CABLE-A Input"),
                                ("rf-input-device", "CABLE-A Output")):
            options = self.page.locator(f"#{element} option").evaluate_all("nodes => nodes.map(n => n.value)")
            matches = [value for value in options if value.startswith(prefix) and "VB-Audio Cable A" in value]
            assert len(matches) == 1, f"documented device missing or ambiguous: {prefix}; {options}"
            self.page.locator(f"#{element}").select_option(matches[0])
            chosen[element] = matches[0]
        folder = self.home / "recording"
        folder.mkdir()
        self.page.locator("#rf-record-dir").fill(str(folder))
        self.page.locator("#rf-sweep-source").select_option("default")
        self.collect()
        event_start = len(self.events)
        self.page.locator("#btn-record-headphones").click()
        status = self.wait_job("recorder", "sidebar_recorder", "start_recording", event_start)
        info = wav_info(folder / "headphones.wav")
        assert info["channels"] == 2 and info["sample_rate"] == 48000, info
        shutil.copy2(folder / "headphones.wav", self.output / "headphones.wav")
        return {"status": status, "devices": chosen, "wav": info}

    def cleanup(self):
        # Only our PID's tree is terminated; no global WebView2/image-name kill.
        if self.process is not None and self.process.poll() is None:
            result = subprocess.run(["taskkill", "/PID", str(self.process.pid), "/T", "/F"],
                                    stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=30)
            (self.output / "cleanup.log").write_bytes(result.stdout)
            assert result.returncode == 0, f"taskkill failed: {result.stdout!r}"
            self.process.wait(timeout=15)
        if self.process is not None:
            self.summary["app_exit_code"] = self.process.returncode
        if hasattr(self, "playwright"):
            self.playwright.stop()
        # TemporaryDirectory cleanup retries are for WebView2 handle release only.
        for attempt in range(30):
            try:
                self.temp.cleanup()
                return {"owned_process_reaped": True, "temporary_home_removed": not self.home.exists()}
            except PermissionError:
                if attempt == 29:
                    raise
                time.sleep(0.1)

    def run(self):
        self.temp = tempfile.TemporaryDirectory(prefix="impulcifer-p15-")
        self.home = Path(self.temp.name)
        self.demo = self.home / "demo"
        self.en = json.loads((ROOT / "i18n/locales/en.json").read_text(encoding="utf-8"))
        write_json(self.output / "frozen-before.json", self.before)
        try:
            with (self.output / "app.log").open("wb") as self.app_log:
                self.step("launch", self.launch)
                # Captured startup errors fail launch but must not suppress independent UI checks.
                attached = self.page is not None
                self.step("bootstrap", self.bootstrap, None if attached else "app page unavailable")
                for name, action in (("brir", self.brir), ("recovery", self.recovery),
                                     ("settings", self.settings), ("updates", self.updates),
                                     ("recording", self.recording)):
                    self.step(name, action, None if self.ready else "bootstrap did not complete")
                if attached:
                    self.step("final_console_check", self.collect)
        except Exception:
            self.summary["harness_error"] = traceback.format_exc()
        finally:
            self.page = None
            self.step("cleanup", self.cleanup)
            after = frozen_hashes()
            write_json(self.output / "frozen-after.json", after)
            self.summary["frozen_unchanged"] = self.before == after
            write_json(self.output / "events.json", self.events)
            write_json(self.output / "cdp-events.json", self.cdp_events)
            self.summary["duration_seconds"] = round(time.monotonic() - self.started, 3)
            self.summary["ok"] = (not self.summary.get("harness_error") and self.before == after
                                  and all(s["status"] != "fail" for s in self.steps)
                                  and all(s["status"] == "ok" for s in self.steps
                                          if s["name"] in ("launch", "bootstrap", "brir", "recovery", "settings", "updates")))
            write_json(self.output / "summary.json", self.summary)
            write_json(ROOT / "target" / "app-smoke" / "summary.json", self.summary)
            write_json(self.output.parent / "latest.json", {"summary": str(self.output / "summary.json")})
            print(f"SMOKE_SUMMARY {self.output / 'summary.json'}", flush=True)
            print(f"SMOKE_OK {self.summary['ok']}", flush=True)
        return 0 if self.summary["ok"] else 1


def main():
    # The driver records carry the UI's own strings (ellipsis, Korean labels); a
    # cp1252 console on the Windows runners must not turn that into a harness error.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--hardware", action="store_true")
    parser.add_argument("--cdp", action="store_true", help="optional legacy Playwright CDP attach")
    parser.add_argument("--launch-command", nargs=argparse.REMAINDER,
                        help="run this command (e.g. Update.exe apply --package X) with the harness "
                             "environment instead of starting --exe directly, then attach to the "
                             "--exe process it starts (the updater restart path)")
    args = parser.parse_args()
    if args.cdp:
        assert not args.launch_command, "--launch-command needs the in-app harness"
        return Smoke(args.exe.resolve(), args.hardware).run()
    from in_app import InAppSmoke
    return InAppSmoke(args.exe.resolve(), args.hardware, launch_command=args.launch_command or None).run()


if __name__ == "__main__":
    sys.exit(main())
