"""JSON-safe application service used by non-Tk frontends."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field, is_dataclass
import os
from pathlib import Path
import platform
import threading
import time
from typing import Any, Callable
from uuid import uuid4


_TERMINAL_STATES = {"succeeded", "failed", "cancelled"}
_RECORDING_FIELDS = {
    "mode",
    "play_path",
    "record_dir",
    "input_device",
    "output_device",
    "host_api",
    "channels",
    "append",
    "debug_plots",
    "confirm_warnings",
}
_BRIR_FIELDS = {
    "dir_path",
    "test_signal",
    "plot",
    "do_room_correction",
    "do_headphone_compensation",
    "do_equalization",
}


def _json_safe(value: Any) -> Any:
    if is_dataclass(value):
        value = asdict(value)
    if isinstance(value, dict):
        return {str(key): _json_safe(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_json_safe(item) for item in value]
    if isinstance(value, Path):
        return str(value)
    if value is None or isinstance(value, (bool, int, float, str)):
        return value
    return str(value)


def _ok(data: dict[str, Any]) -> dict[str, Any]:
    return {"ok": True, "data": _json_safe(data)}


def _error(
    code: str,
    message: str,
    *,
    details: dict[str, Any] | None = None,
    retryable: bool = False,
) -> dict[str, Any]:
    return {
        "ok": False,
        "error": {
            "code": code,
            "message": message,
            "details": _json_safe(details or {}),
            "retryable": retryable,
        },
    }


@dataclass
class _Job:
    job_id: str
    kind: str
    cancellable: bool
    cancel_event: threading.Event
    status: str = "running"
    result: dict[str, Any] | None = None
    error: dict[str, Any] | None = None
    events: list[dict[str, Any]] = field(default_factory=list)
    next_seq: int = 0

    def snapshot(self) -> dict[str, Any]:
        return {
            "job_id": self.job_id,
            "kind": self.kind,
            "status": self.status,
            "cancellable": self.cancellable,
            "result": self.result,
            "error": self.error,
        }


class ImpulciferApplicationService:
    """Run one recorder or BRIR job at a time without a GUI dependency."""

    def __init__(self) -> None:
        self._lock = threading.RLock()
        self._jobs: dict[str, _Job] = {}
        self._active_job_id: str | None = None

    def bootstrap(self) -> dict[str, Any]:
        from impulcifer import __version__

        with self._lock:
            active_job = self._jobs.get(self._active_job_id or "")
            active = active_job.snapshot() if active_job is not None else None
        return _ok(
            {
                "version": __version__,
                "platform": platform.system().lower(),
                "capabilities": {
                    "recording": True,
                    "brir": True,
                    "recording_cancel": False,
                    "brir_cancel": True,
                },
                "active_job": active,
            }
        )

    def list_audio_devices(self, host_api: str | None = None) -> dict[str, Any]:
        try:
            import sounddevice

            host_apis = list(sounddevice.query_hostapis())
            devices = list(sounddevice.query_devices())
            names = {index: str(item["name"]) for index, item in enumerate(host_apis)}
            if host_api and host_api not in names.values():
                return _error(
                    "INVALID_REQUEST",
                    "Unknown host API.",
                    details={"host_api": host_api},
                )

            serialized = []
            for index, device in enumerate(devices):
                api_name = names.get(int(device["hostapi"]), "")
                if host_api and api_name != host_api:
                    continue
                serialized.append(
                    {
                        "index": index,
                        "name": str(device["name"]),
                        "host_api": api_name,
                        "max_input_channels": int(device["max_input_channels"]),
                        "max_output_channels": int(device["max_output_channels"]),
                    }
                )

            default_pair = list(sounddevice.default.device)
            return _ok(
                {
                    "host_apis": list(names.values()),
                    "devices": serialized,
                    "default_input_index": int(default_pair[0]),
                    "default_output_index": int(default_pair[1]),
                }
            )
        except Exception as exc:
            return _error("DEVICE_ERROR", str(exc), retryable=True)

    def start_recording(self, request: dict[str, Any]) -> dict[str, Any]:
        validation = self._validate_recording_request(request)
        if not validation["ok"]:
            return validation
        params = validation["data"]

        def run(job_id: str, _cancel_event: threading.Event) -> dict[str, Any]:
            from core import recorder

            try:
                recorder.play_and_record(
                    play=params["play_path"],
                    record=params["record_path"],
                    input_device=params["input_device"],
                    output_device=params["output_device"],
                    host_api=params["host_api"],
                    channels=params["channels"],
                    append=params["append"],
                    debug_plots=params["debug_plots"],
                    progress_callback=lambda event: self._emit(
                        job_id, "progress", _json_safe(event)
                    ),
                    mono_to_stereo=params["mode"] == "headphones",
                )
            except recorder.DeviceNotFoundError as exc:
                raise _ServiceFailure("DEVICE_ERROR", str(exc), retryable=True) from exc
            if not os.path.isfile(params["record_path"]):
                raise _ServiceFailure(
                    "OUTPUT_MISSING",
                    "Recording finished without producing the expected file.",
                    details={"record_path": params["record_path"]},
                )
            return {
                "mode": params["mode"],
                "record_path": params["record_path"],
            }

        return self._start_job("recording", False, run)

    def start_brir(self, request: dict[str, Any]) -> dict[str, Any]:
        validation = self._validate_brir_request(request)
        if not validation["ok"]:
            return validation
        params = validation["data"]

        def run(job_id: str, cancel_event: threading.Event) -> dict[str, Any]:
            import impulcifer
            from infra.logger import get_logger

            logger = get_logger()
            previous_gui_callback = logger.gui_callback
            previous_progress_callback = logger.progress_callback
            logger.set_gui_callback(
                lambda level, message: self._emit(
                    job_id, "log", {"level": level, "message": message}
                )
            )
            logger.set_progress_callback(
                lambda progress, message: self._emit(
                    job_id,
                    "progress",
                    {"progress": max(0.0, min(1.0, float(progress) / 100.0)), "message": message},
                )
            )
            try:
                with impulcifer.cancellation_scope(cancel_event):
                    impulcifer.main(**params)
            except impulcifer.CancelledError:
                raise _JobCancelled() from None
            finally:
                logger.set_gui_callback(previous_gui_callback)
                logger.set_progress_callback(previous_progress_callback)

            output_path = os.path.join(params["dir_path"], "hesuvi.wav")
            if not os.path.isfile(output_path):
                raise _ServiceFailure(
                    "OUTPUT_MISSING",
                    "BRIR processing finished without producing hesuvi.wav.",
                    details={"output_path": output_path},
                )
            return {"output_path": output_path}

        return self._start_job("brir", True, run)

    def poll_job(self, job_id: str, after_seq: int = 0) -> dict[str, Any]:
        if not isinstance(job_id, str) or not job_id:
            return _error("INVALID_REQUEST", "job_id is required.")
        if isinstance(after_seq, bool) or not isinstance(after_seq, int) or after_seq < 0:
            return _error("INVALID_REQUEST", "after_seq must be a non-negative integer.")
        with self._lock:
            job = self._jobs.get(job_id)
            if job is None:
                return _error("JOB_NOT_FOUND", "Job not found.", details={"job_id": job_id})
            events = [event.copy() for event in job.events if event["seq"] > after_seq]
            return _ok(
                {
                    "job": job.snapshot(),
                    "events": events,
                    "next_seq": job.next_seq,
                }
            )

    def cancel_job(self, job_id: str) -> dict[str, Any]:
        with self._lock:
            job = self._jobs.get(job_id)
            if job is None:
                return _error("JOB_NOT_FOUND", "Job not found.", details={"job_id": job_id})
            if job.status in _TERMINAL_STATES:
                return _ok({"job": job.snapshot()})
            if not job.cancellable:
                return _error(
                    "JOB_NOT_CANCELLABLE",
                    "Recording jobs cannot be cancelled safely.",
                    details={"job_id": job_id},
                )
            job.status = "cancel_requested"
            job.cancel_event.set()
            self._append_event(job, "status", {"status": job.status})
            return _ok({"job": job.snapshot()})

    def _start_job(
        self,
        kind: str,
        cancellable: bool,
        target: Callable[[str, threading.Event], dict[str, Any]],
    ) -> dict[str, Any]:
        with self._lock:
            active = self._jobs.get(self._active_job_id or "")
            if active is not None and active.status not in _TERMINAL_STATES:
                return _error(
                    "JOB_BUSY",
                    "Another job is already running.",
                    details={"job": active.snapshot()},
                    retryable=True,
                )
            job = _Job(str(uuid4()), kind, cancellable, threading.Event())
            self._jobs[job.job_id] = job
            self._active_job_id = job.job_id
            self._append_event(job, "status", {"status": "running"})
            thread = threading.Thread(
                target=self._run_job,
                args=(job.job_id, target),
                name=f"impulcifer-{kind}-{job.job_id[:8]}",
                daemon=False,
            )
            thread.start()
            return _ok({"job": job.snapshot()})

    def _run_job(
        self,
        job_id: str,
        target: Callable[[str, threading.Event], dict[str, Any]],
    ) -> None:
        with self._lock:
            job = self._jobs[job_id]
            cancel_event = job.cancel_event
        try:
            result = _json_safe(target(job_id, cancel_event))
        except _JobCancelled:
            self._finish_job(job_id, "cancelled")
        except _ServiceFailure as exc:
            self._finish_job(job_id, "failed", error=exc.as_dict())
        except Exception as exc:
            self._finish_job(
                job_id,
                "failed",
                error={
                    "code": "INTERNAL_ERROR",
                    "message": str(exc) or exc.__class__.__name__,
                    "details": {},
                    "retryable": False,
                },
            )
        else:
            self._finish_job(job_id, "succeeded", result=result)

    def _finish_job(
        self,
        job_id: str,
        status: str,
        *,
        result: dict[str, Any] | None = None,
        error: dict[str, Any] | None = None,
    ) -> None:
        with self._lock:
            job = self._jobs[job_id]
            job.status = status
            job.result = result
            job.error = error
            self._append_event(job, "status", {"status": status})
            if self._active_job_id == job_id:
                self._active_job_id = None

    def _emit(self, job_id: str, event_type: str, payload: dict[str, Any]) -> None:
        with self._lock:
            job = self._jobs.get(job_id)
            if job is not None:
                self._append_event(job, event_type, payload)

    @staticmethod
    def _append_event(job: _Job, event_type: str, payload: dict[str, Any]) -> None:
        job.next_seq += 1
        job.events.append(
            {
                "seq": job.next_seq,
                "timestamp_ms": int(time.time() * 1000),
                "type": event_type,
                "payload": _json_safe(payload),
            }
        )

    def _validate_recording_request(self, request: dict[str, Any]) -> dict[str, Any]:
        if not isinstance(request, dict):
            return _error("INVALID_REQUEST", "Recording request must be an object.")
        unknown = sorted(set(request) - _RECORDING_FIELDS)
        if unknown:
            return _error("INVALID_REQUEST", "Unknown recording fields.", details={"fields": unknown})

        mode = request.get("mode", "speakers")
        play_path = str(request.get("play_path", "")).strip()
        record_dir = str(request.get("record_dir", "")).strip()
        if mode not in {"speakers", "headphones"}:
            return _error("INVALID_REQUEST", "mode must be speakers or headphones.")
        if not play_path or not os.path.isfile(play_path):
            return _error("FILE_NOT_FOUND", "Playback file does not exist.", details={"path": play_path})
        if not record_dir:
            return _error("INVALID_REQUEST", "record_dir is required.")

        channels = request.get("channels", 2)
        if isinstance(channels, bool) or not isinstance(channels, int) or not 1 <= channels <= 64:
            return _error("INVALID_REQUEST", "channels must be an integer from 1 to 64.")
        confirm = request.get("confirm_warnings", False)
        if not isinstance(confirm, bool):
            return _error("INVALID_REQUEST", "confirm_warnings must be a boolean.")

        from core.headphones_recording import inspect_headphones_playback
        from core.recording_naming import resolve_headphones_record_path, resolve_record_path
        from core.recording_validation import validate_recording_setup

        if mode == "headphones":
            playback = inspect_headphones_playback(play_path)
            if not playback.is_valid:
                return _error(
                    "INVALID_REQUEST",
                    playback.reason_key,
                    details={"path": play_path, "channels": playback.channels},
                )
            if playback.is_mono and not confirm:
                return _error(
                    "CONFIRMATION_REQUIRED",
                    "Mono playback produces generic L=R headphone compensation.",
                    details={"warning": "headphones_mono"},
                )
            channels = 2
            append = False
            record_path = resolve_headphones_record_path(record_dir)
        else:
            append = request.get("append", False)
            if not isinstance(append, bool):
                return _error("INVALID_REQUEST", "append must be a boolean.")
            record_path = resolve_record_path(record_dir, play_path)
            channel_validation = validate_recording_setup(record_path, channels, True)
            if channel_validation and channel_validation.has_mismatch and not confirm:
                return _error(
                    "CONFIRMATION_REQUIRED",
                    "Selected recording channels do not match the sweep speaker count.",
                    details=asdict(channel_validation),
                )

        debug_plots = request.get("debug_plots", False)
        if not isinstance(debug_plots, bool):
            return _error("INVALID_REQUEST", "debug_plots must be a boolean.")
        return _ok(
            {
                "mode": mode,
                "play_path": play_path,
                "record_path": record_path,
                "input_device": _optional_string(request.get("input_device")),
                "output_device": _optional_string(request.get("output_device")),
                "host_api": _optional_string(request.get("host_api")),
                "channels": channels,
                "append": append,
                "debug_plots": debug_plots,
            }
        )

    @staticmethod
    def _validate_brir_request(request: dict[str, Any]) -> dict[str, Any]:
        if not isinstance(request, dict):
            return _error("INVALID_REQUEST", "BRIR request must be an object.")
        unknown = sorted(set(request) - _BRIR_FIELDS)
        if unknown:
            return _error("INVALID_REQUEST", "Unknown BRIR fields.", details={"fields": unknown})
        dir_path = str(request.get("dir_path", "")).strip()
        if not dir_path or not os.path.isdir(dir_path):
            return _error("FILE_NOT_FOUND", "Measurement directory does not exist.", details={"path": dir_path})

        params: dict[str, Any] = {
            "dir_path": dir_path,
            "test_signal": _optional_string(request.get("test_signal")),
        }
        for name, default in (
            ("plot", False),
            ("do_room_correction", True),
            ("do_headphone_compensation", True),
            ("do_equalization", True),
        ):
            value = request.get(name, default)
            if not isinstance(value, bool):
                return _error("INVALID_REQUEST", f"{name} must be a boolean.")
            params[name] = value
        return _ok(params)


def _optional_string(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


class _ServiceFailure(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        details: dict[str, Any] | None = None,
        retryable: bool = False,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.details = details or {}
        self.retryable = retryable

    def as_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "message": str(self),
            "details": _json_safe(self.details),
            "retryable": self.retryable,
        }


class _JobCancelled(RuntimeError):
    pass
