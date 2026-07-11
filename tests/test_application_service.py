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
            "force_channels": True,
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


def test_unforced_default_recording_skips_channel_confirmation(
    monkeypatch,
    tmp_path: Path,
) -> None:
    """Codex review P2: the default 2ch path must not warn about the sweep's
    speaker count — only an explicitly forced channel count is validated."""
    from core import recorder

    segmented = tmp_path / "sweep-seg-FL,FR-stereo-6.15s-test.wav"
    segmented.write_bytes(b"sweep")
    captured = {}

    def fake_play_and_record(**kwargs):
        captured.update(kwargs)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", fake_play_and_record)
    service = ImpulciferApplicationService()
    started = service.start_recording(
        {
            "mode": "speakers",
            "play_path": str(segmented),
            "record_dir": str(tmp_path),
            "channels": 14,
        }
    )
    assert started["ok"], started
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "succeeded"
    # Without force_channels the requested count is ignored, like the CTk GUI.
    assert captured["channels"] == 2


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


def test_brir_accepts_full_processing_config_surface(monkeypatch, tmp_path: Path) -> None:
    import impulcifer

    captured = {}

    def fake_main(**kwargs):
        captured.update(kwargs)
        Path(kwargs["dir_path"], "hesuvi.wav").write_bytes(b"brir")

    monkeypatch.setattr(impulcifer, "main", fake_main)
    service = ImpulciferApplicationService()
    started = service.start_brir(
        {
            "dir_path": str(tmp_path),
            "fs": 44100,
            "target_level": -12.5,
            "channel_balance": "trend",
            "decay": {"FL": 0.3, "FR": 0.25},
            "head_ms": 2.0,
            "jamesdsp": True,
            "fr_combination_method": "conservative",
            "vbass": True,
            "vbass_freq": 800,
            "vbass_hp": 12.5,
            "vbass_polarity": "invert",
        }
    )
    assert started["ok"], started
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "succeeded"
    assert captured["fs"] == 44100
    assert captured["target_level"] == -12.5
    assert captured["channel_balance"] == "trend"
    assert captured["decay"] == {"FL": 0.3, "FR": 0.25}
    assert captured["head_ms"] == 2.0
    assert captured["jamesdsp"] is True
    assert captured["fr_combination_method"] == "conservative"
    # The service clamps vbass_freq to [30, 500] like the CTk GUI.
    assert captured["vbass_freq"] == 500
    assert captured["vbass_polarity"] == "invert"


def test_brir_numeric_decay_fans_out_to_all_channels(tmp_path: Path) -> None:
    validation = ImpulciferApplicationService._validate_brir_request(
        {"dir_path": str(tmp_path), "decay": 0.3}
    )
    assert validation["ok"]
    assert validation["data"]["params"]["decay"] == {
        channel: 0.3 for channel in ("FL", "FC", "FR", "SL", "SR", "BL", "BR")
    }


def test_brir_rejects_unknown_and_invalid_fields(tmp_path: Path) -> None:
    validate = ImpulciferApplicationService._validate_brir_request
    base = {"dir_path": str(tmp_path)}

    for bad in (
        {"definitely_not_a_field": 1},
        {"vbass": "yes"},
        {"vbass_polarity": "sideways"},
        {"fr_combination_method": "median"},
        {"channel_balance": "loud"},
        {"decay": {"XX": 0.3}},
        {"decay": -1},
        {"fs": 44100.5},
        {"specific_limit": "wide"},
        {"test_signal": 42},
    ):
        response = validate({**base, **bad})
        assert not response["ok"], bad
        assert response["error"]["code"] == "INVALID_REQUEST", bad


def test_brir_copies_custom_eq_sidecars(monkeypatch, tmp_path: Path) -> None:
    import impulcifer

    source = tmp_path / "my-eq.csv"
    source.write_text("20 0.0\n", encoding="utf-8")

    def fake_main(**kwargs):
        Path(kwargs["dir_path"], "hesuvi.wav").write_bytes(b"brir")

    monkeypatch.setattr(impulcifer, "main", fake_main)
    service = ImpulciferApplicationService()
    started = service.start_brir(
        {
            "dir_path": str(tmp_path),
            "do_equalization": True,
            "eq_file": str(source),
        }
    )
    assert started["ok"], started
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "succeeded"
    assert (tmp_path / "eq.csv").read_text(encoding="utf-8") == "20 0.0\n"


def test_brir_missing_eq_sidecar_fails_fast(tmp_path: Path) -> None:
    service = ImpulciferApplicationService()
    response = service.start_brir({"dir_path": str(tmp_path), "eq_file": "missing.csv"})
    assert response["error"]["code"] == "FILE_NOT_FOUND"


def test_brir_runtime_failure_reports_structured_error(monkeypatch, tmp_path: Path) -> None:
    import impulcifer

    def failing_main(**_kwargs):
        raise ValueError("Impulse response peak not found for FL-left")

    monkeypatch.setattr(impulcifer, "main", failing_main)
    service = ImpulciferApplicationService()
    started = service.start_brir({"dir_path": str(tmp_path)})
    assert started["ok"], started
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    job = data["job"]
    assert job["status"] == "failed"
    assert job["error"]["code"] == "INTERNAL_ERROR"
    assert "peak not found" in job["error"]["message"]
    statuses = [
        event["payload"]["status"] for event in data["events"] if event["type"] == "status"
    ]
    assert statuses[-1] == "failed"


class _FakeLocalization:
    def __init__(self, locales_dir: Path) -> None:
        self.current_language = "en"
        self.locales_dir = locales_dir
        self.theme = "dark"
        self.skin = "stable"
        self.marked = False

    def get_theme(self) -> str:
        return self.theme

    def set_theme(self, theme: str) -> None:
        self.theme = theme

    def get_skin(self) -> str:
        return self.skin

    def set_skin(self, skin: str) -> None:
        self.skin = skin

    def set_language(self, code: str) -> None:
        self.current_language = code

    def mark_language_selected(self) -> None:
        self.marked = True


def test_ui_settings_language_and_theme_roundtrip(monkeypatch) -> None:
    import i18n.localization as localization

    fake = _FakeLocalization(Path(__file__).resolve().parents[1] / "i18n" / "locales")
    monkeypatch.setattr(localization, "get_localization_manager", lambda: fake)
    service = ImpulciferApplicationService()

    settings = service.get_ui_settings()
    assert settings["ok"]
    assert settings["data"]["language"] == "en"
    assert settings["data"]["strings"]["tab_recorder"]
    assert any(entry["code"] == "ko" for entry in settings["data"]["languages"])

    switched = service.set_language("ko")
    assert switched["ok"]
    assert fake.current_language == "ko"
    assert fake.marked is True
    assert switched["data"]["strings"] != settings["data"]["strings"]

    assert service.set_language("xx")["error"]["code"] == "INVALID_REQUEST"

    assert service.set_theme("light")["ok"]
    assert fake.theme == "light"
    assert service.set_theme("neon")["error"]["code"] == "INVALID_REQUEST"

    assert settings["data"]["skin"] == "stable"
    assert service.set_skin("studio")["ok"]
    assert fake.skin == "studio"
    assert service.set_skin("neon")["error"]["code"] == "INVALID_REQUEST"


def test_system_info_reports_environment() -> None:
    import impulcifer

    response = ImpulciferApplicationService().get_system_info()
    assert response["ok"]
    data = response["data"]
    assert data["version"] == impulcifer.__version__
    assert data["install_kind"] in {"velopack", "pip", "dev"}
    for key in ("python_version", "os", "cpu_count", "gil_enabled", "optimal_workers"):
        assert key in data


def test_resolve_recording_paths() -> None:
    service = ImpulciferApplicationService()
    speakers = service.resolve_recording_paths("out", "sweep-seg-FL,FR-stereo-test.wav")
    assert speakers["ok"]
    assert speakers["data"]["record_path"].endswith("FL,FR.wav")

    headphones = service.resolve_recording_paths("out", None, "headphones")
    assert headphones["data"]["record_path"].endswith("headphones.wav")

    assert service.resolve_recording_paths("")["error"]["code"] == "INVALID_REQUEST"
    assert service.resolve_recording_paths("out")["error"]["code"] == "INVALID_REQUEST"


def test_generate_sweep_set_delegates(monkeypatch, tmp_path: Path) -> None:
    import core.sweep_set_generator as generator_module

    files = [str(tmp_path / "sweep-seg-FL,FR-stereo.wav"), str(tmp_path / "sweep-7.1.wav")]
    monkeypatch.setattr(generator_module, "generate_sweep_set", lambda target: files)
    service = ImpulciferApplicationService()

    response = service.generate_sweep_set(str(tmp_path))
    assert response["ok"]
    assert response["data"]["files"] == files
    assert response["data"]["play_path"] == files[0]

    missing = service.generate_sweep_set(str(tmp_path / "nope"))
    assert missing["error"]["code"] == "FILE_NOT_FOUND"


def test_open_path_rejects_missing_folder(tmp_path: Path) -> None:
    response = ImpulciferApplicationService().open_path(str(tmp_path / "nope"))
    assert response["error"]["code"] == "FILE_NOT_FOUND"


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
