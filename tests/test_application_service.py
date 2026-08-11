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


def test_output_recovery_job_maps_request_and_returns_manifest(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import brir_recovery

    captured = {}

    def fake_recovery(directory, *, include_hangloose=False):
        captured["directory"] = directory
        captured["include_hangloose"] = include_hangloose
        return brir_recovery.BrirRecoveryResult(
            source_kind="hrir",
            source_path=str(tmp_path / "hrir.wav"),
            output_dir=str(tmp_path),
            sample_rate=48_000,
            sample_count=4096,
            speakers=("FL", "FR"),
            created_files=(str(tmp_path / "hesuvi.wav"),),
            existing_files=(str(tmp_path / "hrir.wav"),),
        )

    monkeypatch.setattr(brir_recovery, "recover_brir_outputs", fake_recovery)
    service = ImpulciferApplicationService()
    started = service.start_output_recovery(
        {"dir_path": str(tmp_path), "include_hangloose": True}
    )
    assert started["ok"], started

    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "succeeded"
    assert captured == {"directory": str(tmp_path), "include_hangloose": True}
    assert data["job"]["result"]["source_kind"] == "hrir"
    assert data["job"]["result"]["speakers"] == ["FL", "FR"]


def test_output_recovery_request_validation_and_structured_failure(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import brir_recovery

    service = ImpulciferApplicationService()
    for request in (
        [],
        {},
        {"dir_path": str(tmp_path), "include_hangloose": "yes"},
        {"dir_path": str(tmp_path), "unknown": True},
    ):
        response = service.start_output_recovery(request)
        assert not response["ok"], request
        assert response["error"]["code"] == "INVALID_REQUEST", request

    def fail_recovery(*_args, **_kwargs):
        raise brir_recovery.BrirRecoveryError(
            "SOURCE_MISMATCH",
            "The surviving outputs disagree.",
            details={"tracks": ["FL-left"]},
        )

    monkeypatch.setattr(brir_recovery, "recover_brir_outputs", fail_recovery)
    started = service.start_output_recovery({"dir_path": str(tmp_path)})
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])

    assert data["job"]["status"] == "failed"
    assert data["job"]["error"] == {
        "code": "SOURCE_MISMATCH",
        "message": "The surviving outputs disagree.",
        "details": {"tracks": ["FL-left"]},
        "retryable": False,
    }


class _FakeLocalization:
    def __init__(self, locales_dir: Path) -> None:
        self.current_language = "en"
        self.locales_dir = locales_dir
        self.theme = "dark"
        self.skin = "stable"
        self.frontend = "webview"
        self.marked = False

    def get_theme(self) -> str:
        return self.theme

    def set_theme(self, theme: str) -> None:
        self.theme = theme

    def get_skin(self) -> str:
        return self.skin

    def set_skin(self, skin: str) -> None:
        self.skin = skin

    def get_frontend(self) -> str:
        return self.frontend

    def set_frontend(self, frontend: str) -> None:
        self.frontend = frontend

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

    assert settings["data"]["frontend"] == "webview"
    assert service.set_frontend("ctk")["ok"]
    assert fake.frontend == "ctk"
    assert service.set_frontend("qt")["error"]["code"] == "INVALID_REQUEST"


def test_bootstrap_ships_processing_config_defaults() -> None:
    """bootstrap() must publish ProcessingConfig defaults so frontends never
    hardcode drifting copies (audit #138 F020)."""
    from dataclasses import MISSING, fields

    from core.pipeline import ProcessingConfig

    response = ImpulciferApplicationService().bootstrap()
    assert response["ok"]
    shipped = response["data"]["brir_defaults"]

    for config_field in fields(ProcessingConfig):
        if config_field.name == "dir_path" or config_field.default is MISSING:
            continue
        assert shipped[config_field.name] == config_field.default, config_field.name

    assert shipped["specific_limit"] == 400
    assert shipped["generic_limit"] == 300
    assert response["data"]["capabilities"]["output_recovery"] is True
    assert response["data"]["capabilities"]["output_recovery_cancel"] is False


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


def test_check_for_updates_reports_available(monkeypatch) -> None:
    import updater.update_checker as checker_module

    class _FakeChecker:
        def __init__(self, current_version):
            self.current_version = current_version

        def check_for_updates(self):
            return True, "99.0.0", "https://example.invalid/Impulcifer-win-Setup.exe"

        def get_release_notes(self):
            return "notes"

        def get_release_url(self):
            return "https://example.invalid/releases/latest"

    monkeypatch.setattr(checker_module, "UpdateChecker", _FakeChecker)
    response = ImpulciferApplicationService().check_for_updates()
    assert response["ok"]
    data = response["data"]
    assert data["update_available"] is True
    assert data["latest_version"] == "99.0.0"
    assert data["download_url"].endswith("Setup.exe")
    assert data["release_notes"] == "notes"
    assert data["release_url"].endswith("latest")


def test_check_for_updates_handles_constructor_failure(monkeypatch) -> None:
    import updater.update_checker as checker_module

    class _Boom:
        def __init__(self, current_version):
            raise OSError("offline")

    monkeypatch.setattr(checker_module, "UpdateChecker", _Boom)
    response = ImpulciferApplicationService().check_for_updates()
    assert not response["ok"]
    assert response["error"]["code"] == "UPDATE_CHECK_FAILED"
    assert response["error"]["retryable"] is True


def test_start_update_requires_latest_version() -> None:
    response = ImpulciferApplicationService().start_update({})
    assert not response["ok"]
    assert response["error"]["code"] == "INVALID_REQUEST"


def test_start_update_job_stages_restart_action(monkeypatch) -> None:
    import updater.updater_core as updater_core
    from updater.updater_core import UpdateExecutionResult

    applied = []

    class _FakeExecutor:
        def execute(self, progress_callback):
            progress_callback(0.5, "update_downloading")
            return UpdateExecutionResult(
                status_key="update_installing",
                status_default="Applying update...",
                title_key="update_ready_title",
                title_default="Update Ready",
                message_key="update_restart_message",
                message_default="The application will close to apply the update.",
                progress=0.9,
                close_delay_ms=0,
                after_message=lambda: applied.append(True) or True,
            )

    monkeypatch.setattr(
        updater_core, "create_update_executor", lambda url, version: _FakeExecutor()
    )
    service = ImpulciferApplicationService()
    started = service.start_update(
        {"latest_version": "99.0.0", "download_url": "https://example.invalid/pkg"}
    )
    assert started["ok"]
    assert started["data"]["job"]["kind"] == "update"

    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])
    job = data["job"]
    assert job["status"] == "succeeded"
    assert job["result"]["requires_restart"] is True
    assert job["result"]["message_key"] == "update_restart_message"
    progress_events = [event for event in data["events"] if event["type"] == "progress"]
    assert progress_events
    assert progress_events[0]["payload"]["message"] == "update_downloading"

    response = service.apply_pending_update()
    assert response["ok"]
    assert response["data"]["restarting"] is True
    assert applied == [True]

    # The staged action is single-use: a second apply has nothing to run.
    again = service.apply_pending_update()
    assert not again["ok"]
    assert again["error"]["code"] == "INVALID_REQUEST"


def test_start_update_failure_is_structured(monkeypatch) -> None:
    import updater.updater_core as updater_core
    from updater.updater_core import UpdateExecutionError

    class _FailingExecutor:
        def execute(self, progress_callback):
            raise UpdateExecutionError("Failed to download update")

    monkeypatch.setattr(
        updater_core, "create_update_executor", lambda url, version: _FailingExecutor()
    )
    service = ImpulciferApplicationService()
    started = service.start_update({"latest_version": "99.0.0"})
    assert started["ok"]
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])
    assert data["job"]["status"] == "failed"
    assert data["job"]["error"]["code"] == "UPDATE_FAILED"
    assert data["job"]["error"]["retryable"] is True


def test_apply_pending_update_treats_sys_exit_as_success() -> None:
    # VelopackUpdater.apply_and_restart exits the process after handing over
    # to Update.exe; on a bridge worker thread that must read as success.
    service = ImpulciferApplicationService()
    service._pending_update_apply = lambda: (_ for _ in ()).throw(SystemExit(0))
    response = service.apply_pending_update()
    assert response["ok"]
    assert response["data"]["restarting"] is True


def test_apply_pending_update_reports_false_return() -> None:
    service = ImpulciferApplicationService()
    service._pending_update_apply = lambda: False
    response = service.apply_pending_update()
    assert not response["ok"]
    assert response["error"]["code"] == "UPDATE_FAILED"


# ------------------------------------------------------------------
# On-the-fly sweep (2.11): request validation, playback and sidecars
# ------------------------------------------------------------------

def test_recording_with_custom_generated_sweep_writes_sidecar(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import recorder

    captured = {}

    def fake_play_and_record(**kwargs):
        captured.update(kwargs)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", fake_play_and_record)
    service = ImpulciferApplicationService()
    started = service.start_recording(
        {
            "mode": "speakers",
            "record_dir": str(tmp_path),
            "sweep": {
                "mode": "custom",
                "fs": 8000,
                "duration": 1.0,
                "speakers": "FL,FR",
                "tracks": "stereo",
            },
        }
    )
    assert started["ok"]
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])
    assert data["job"]["status"] == "succeeded"
    assert captured["play"] is None
    assert captured["play_signal"] is not None
    assert captured["play_signal"].fs == 8000
    assert captured["play_signal"].record_filename == "FL,FR.wav"
    assert Path(captured["record"]).name == "FL,FR.wav"
    # Custom parameters must leave a self-describing test.wav behind.
    assert (tmp_path / "test.wav").is_file()
    json.dumps(data)


def test_recording_with_default_generated_sweep_writes_no_sidecar(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import recorder

    captured = {}

    def fake_play_and_record(**kwargs):
        captured.update(kwargs)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", fake_play_and_record)
    service = ImpulciferApplicationService()
    started = service.start_recording(
        {
            "mode": "speakers",
            "record_dir": str(tmp_path),
            "sweep": {"mode": "default", "speakers": ["SL", "SR"], "tracks": "stereo"},
        }
    )
    assert started["ok"]
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"], timeout=15.0)
    assert data["job"]["status"] == "succeeded"
    assert Path(captured["record"]).name == "SL,SR.wav"
    # Default parameters == bundled sweep — no sidecar needed.
    assert not (tmp_path / "test.wav").exists()


def test_headphones_recording_with_generated_sweep(
    monkeypatch,
    tmp_path: Path,
) -> None:
    from core import recorder

    captured = {}

    def fake_play_and_record(**kwargs):
        captured.update(kwargs)
        Path(kwargs["record"]).write_bytes(b"recorded")

    monkeypatch.setattr(recorder, "play_and_record", fake_play_and_record)
    service = ImpulciferApplicationService()
    started = service.start_recording(
        {
            "mode": "headphones",
            "record_dir": str(tmp_path),
            "sweep": {"mode": "custom", "fs": 8000, "duration": 1.0},
        }
    )
    assert started["ok"]
    data = _wait_for_terminal(service, started["data"]["job"]["job_id"])
    assert data["job"]["status"] == "succeeded"
    assert captured["record"].endswith("headphones.wav")
    assert captured["channels"] == 2
    # Generated headphone sweeps are stereo L→R sequences already.
    assert captured["mono_to_stereo"] is False
    assert tuple(s.speaker for s in captured["play_signal"].segments) == ("FL", "FR")
    # Headphone captures never write the speaker-side sidecar.
    assert not (tmp_path / "test.wav").exists()


def test_sweep_request_validation_rejections(tmp_path: Path) -> None:
    service = ImpulciferApplicationService()
    base = {"mode": "speakers", "record_dir": str(tmp_path)}
    assert not service.start_recording({**base, "sweep": {"mode": "bogus"}})["ok"]
    assert not service.start_recording({**base, "sweep": {"unknown_field": 1}})["ok"]
    assert not service.start_recording(
        {**base, "sweep": {"mode": "custom", "speakers": ["TFL"], "tracks": "7.1"}}
    )["ok"]
    assert not service.start_recording(
        {**base, "sweep": {"mode": "custom", "fs": 999, "speakers": "FL", "tracks": "stereo"}}
    )["ok"]
    # File mode still demands an existing play file.
    missing = service.start_recording({**base, "sweep": {"mode": "file"}, "play_path": "nope.wav"})
    assert missing["error"]["code"] == "FILE_NOT_FOUND"


def test_resolve_recording_paths_with_sweep(tmp_path: Path) -> None:
    service = ImpulciferApplicationService()
    preview = service.resolve_recording_paths(
        str(tmp_path), sweep={"mode": "default", "speakers": "BL,BR"}
    )
    assert preview["ok"]
    assert preview["data"]["record_path"].endswith("BL,BR.wav")
    invalid = service.resolve_recording_paths(
        str(tmp_path), sweep={"mode": "custom", "speakers": ["TFL"], "tracks": "7.1"}
    )
    assert not invalid["ok"]


def test_detect_sweep_endpoint(tmp_path: Path) -> None:
    import numpy as np

    from core.sweep_signal import SweepSpec, build_sweep_playback
    from core.utils import write_wav

    playback = build_sweep_playback(SweepSpec(fs=8000, duration=1.0, speakers=("FL", "FR")))
    mix = np.sum(playback.data, axis=0)
    write_wav(str(tmp_path / "FL,FR.wav"), playback.fs, np.vstack([mix, mix]), bit_depth=32)

    service = ImpulciferApplicationService()
    response = service.detect_sweep(str(tmp_path))
    assert response["ok"]
    assert response["data"]["found"] is True
    assert response["data"]["fs"] == 8000
    assert response["data"]["confidence"] == "high"
    assert response["data"]["is_default"] is False
    assert response["data"]["generate_spec"].startswith("generate:")
    json.dumps(response)

    missing = service.detect_sweep(str(tmp_path / "nope"))
    assert missing["error"]["code"] == "FILE_NOT_FOUND"


def test_bootstrap_exposes_sweep_presets() -> None:
    service = ImpulciferApplicationService()
    boot = service.bootstrap()
    assert boot["ok"]
    sweep = boot["data"]["sweep"]
    assert "7.1.6" in sweep["layouts"]
    assert sweep["default_fs"] == 48000
    assert sweep["default_duration"] == 5.0
    json.dumps(boot)
