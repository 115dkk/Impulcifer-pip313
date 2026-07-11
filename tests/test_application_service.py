"""Contract tests for the frontend-neutral application service."""

from __future__ import annotations

import importlib
import json
from pathlib import Path
import sys
import threading
import time
from types import SimpleNamespace

from application.impulcifer_service import ImpulciferApplicationService
from core.recording_progress import RecorderProgressEvent


def _wait_for_terminal(
    service: ImpulciferApplicationService,
    job_id: str,
    timeout: float = 3.0,
) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        response = service.poll_job(job_id)
        assert response["ok"]
        job = response["data"]["job"]
        if job["status"] in {"succeeded", "failed", "cancelled"}:
            return response["data"]
        time.sleep(0.01)
    raise AssertionError("job did not reach a terminal state")


def test_application_import_does_not_load_frontend_modules(monkeypatch) -> None:
    for name in ("tkinter", "customtkinter", "webview"):
        monkeypatch.delitem(sys.modules, name, raising=False)
    module = importlib.reload(importlib.import_module("application.impulcifer_service"))
    assert module.ImpulciferApplicationService
    assert "tkinter" not in sys.modules
    assert "customtkinter" not in sys.modules
    assert "webview" not in sys.modules


def test_recording_job_maps_arguments_and_emits_json_safe_events(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import recorder

    play_path = tmp_path / "custom-sweep.wav"
    play_path.write_bytes(b"sweep")
    captured = {}

    def fake_play_and_record(**kwargs):
        captured.update(kwargs)
        kwargs["progress_callback"](
            RecorderProgressEvent(
                phase="recording",
                progress=0.5,
                speakers=("FL", "FR"),
            )
        )
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", fake_play_and_record)
    service = ImpulciferApplicationService()
    started = service.start_recording(
        {
            "mode": "speakers",
            "play_path": str(play_path),
            "record_dir": str(tmp_path),
            "input_device": "Input",
            "output_device": "Output",
            "host_api": "API",
            "channels": 2,
            "append": True,
            "debug_plots": False,
        }
    )
    assert started["ok"]
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "succeeded"
    assert captured["play"] == str(play_path)
    assert captured["channels"] == 2
    assert captured["append"] is True
    assert captured["mono_to_stereo"] is False
    progress_events = [event for event in data["events"] if event["type"] == "progress"]
    assert progress_events[0]["payload"]["speakers"] == ["FL", "FR"]
    json.dumps(data)


def test_recording_confirmation_and_headphone_forced_values(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import recorder
    import application.impulcifer_service as service_module

    segmented = tmp_path / "sweep-seg-FL,FR-stereo-6.15s-test.wav"
    segmented.write_bytes(b"sweep")
    service = ImpulciferApplicationService()
    warning = service.start_recording(
        {
            "mode": "speakers",
            "play_path": str(segmented),
            "record_dir": str(tmp_path),
            "channels": 2,
        }
    )
    assert warning["error"]["code"] == "CONFIRMATION_REQUIRED"

    monkeypatch.setattr(
        service_module,
        "_optional_string",
        service_module._optional_string,
    )
    monkeypatch.setattr(
        "core.headphones_recording.inspect_headphones_playback",
        lambda _path: SimpleNamespace(is_valid=True, is_mono=False, channels=2, reason_key=""),
    )
    captured = {}

    def fake_play_and_record(**kwargs):
        captured.update(kwargs)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", fake_play_and_record)
    started = service.start_recording(
        {
            "mode": "headphones",
            "play_path": str(segmented),
            "record_dir": str(tmp_path),
            "channels": 14,
            "append": True,
        }
    )
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])
    assert data["job"]["status"] == "succeeded"
    assert captured["channels"] == 2
    assert captured["append"] is False
    assert captured["mono_to_stereo"] is True
    assert captured["record"].endswith("headphones.wav")


def test_service_allows_only_one_active_job(monkeypatch, tmp_path: Path) -> None:
    from core import recorder

    play_path = tmp_path / "custom.wav"
    play_path.write_bytes(b"sweep")
    release = threading.Event()

    def blocking_record(**kwargs):
        assert release.wait(timeout=3.0)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", blocking_record)
    service = ImpulciferApplicationService()
    request = {
        "mode": "speakers",
        "play_path": str(play_path),
        "record_dir": str(tmp_path),
        "channels": 2,
    }
    first = service.start_recording(request)
    second = service.start_recording(request)
    assert first["ok"]
    assert second["error"]["code"] == "JOB_BUSY"
    release.set()
    _wait_for_terminal(service, first["data"]["job"]["job_id"])


def test_brir_job_reports_progress_and_supports_cancellation(
    monkeypatch,
    tmp_path: Path,
) -> None:
    import impulcifer

    entered = threading.Event()

    def cancellable_main(**_kwargs):
        entered.set()
        while True:
            impulcifer._check_cancelled()
            time.sleep(0.005)

    monkeypatch.setattr(impulcifer, "main", cancellable_main)
    service = ImpulciferApplicationService()
    started = service.start_brir({"dir_path": str(tmp_path)})
    job_id = started["data"]["job"]["job_id"]
    assert entered.wait(timeout=1.0)
    cancelled = service.cancel_job(job_id)
    assert cancelled["ok"]
    assert cancelled["data"]["job"]["status"] == "cancel_requested"
    data = _wait_for_terminal(service, job_id)
    assert data["job"]["status"] == "cancelled"


def test_successful_brir_restores_logger_callbacks(monkeypatch, tmp_path: Path) -> None:
    import impulcifer
    from infra.logger import get_logger

    logger = get_logger()
    def old_log(*_args):
        return None

    def old_progress(*_args):
        return None
    logger.set_gui_callback(old_log)
    logger.set_progress_callback(old_progress)

    def fake_main(**kwargs):
        logger.progress(50, "halfway")
        Path(kwargs["dir_path"], "hesuvi.wav").write_bytes(b"brir")

    monkeypatch.setattr(impulcifer, "main", fake_main)
    service = ImpulciferApplicationService()
    started = service.start_brir({"dir_path": str(tmp_path)})
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "succeeded"
    assert any(event["type"] == "progress" for event in data["events"])
    assert logger.gui_callback is old_log
    assert logger.progress_callback is old_progress


def test_recording_jobs_reject_cancellation(monkeypatch, tmp_path: Path) -> None:
    from core import recorder

    play_path = tmp_path / "custom.wav"
    play_path.write_bytes(b"sweep")
    release = threading.Event()

    def blocking_record(**kwargs):
        assert release.wait(timeout=3.0)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", blocking_record)
    service = ImpulciferApplicationService()
    started = service.start_recording(
        {
            "play_path": str(play_path),
            "record_dir": str(tmp_path),
            "channels": 2,
        }
    )
    job_id = started["data"]["job"]["job_id"]
    response = service.cancel_job(job_id)
    assert response["error"]["code"] == "JOB_NOT_CANCELLABLE"
    release.set()
    _wait_for_terminal(service, job_id)
