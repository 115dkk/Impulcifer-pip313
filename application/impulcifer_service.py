"""JSON-safe application service used by non-Tk frontends."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field, is_dataclass
import os
from pathlib import Path
import platform
import shutil
import subprocess
import threading
import time
from typing import Any, Callable, Optional
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
    "force_channels",
    "append",
    "debug_plots",
    "confirm_warnings",
    "sweep",
}
_SWEEP_REQUEST_FIELDS = {"mode", "fs", "duration", "speakers", "tracks"}
_SWEEP_MODES = ("default", "custom", "file")

# Custom EQ files are not ``impulcifer.main`` kwargs: like the CTk GUI's
# ``sync_custom_eq_files``, they are copied into the measurement directory
# under fixed filenames before the pipeline runs.
_BRIR_SIDECAR_EQ_FILES = {
    "eq_file": "eq.csv",
    "eq_left_file": "eq-left.csv",
    "eq_right_file": "eq-right.csv",
}
_BRIR_PATH_FIELDS = {
    "test_signal",
    "room_target",
    "room_mic_calibration",
    "headphone_compensation_file",
}
_BRIR_CHOICE_FIELDS = {
    "fr_combination_method": ("average", "conservative"),
    "vbass_polarity": ("auto", "normal", "invert"),
}
_CHANNEL_BALANCE_TOKENS = ("trend", "left", "right", "avg", "min", "mids")
_DECAY_CHANNELS = ("FL", "FC", "FR", "SL", "SR", "BL", "BR")
_THEME_CODES = ("dark", "light", "system")
# Mirrors gui.skins.SKIN_CHOICES without importing the gui package (this
# module must stay importable without tkinter).
_SKIN_CODES = ("stable", "studio")
_FRONTEND_CODES = ("webview", "ctk")

_dsp_prewarm_started = False


def _start_dsp_prewarm() -> None:
    """Import the heavy DSP stack once in the background.

    Bootstrap itself stays import-light (fast first paint); this daemon
    thread pays the multi-second scipy/matplotlib/bokeh import bill ahead
    of time so the user's first recording/BRIR job doesn't stall on it.
    Python's import lock makes the concurrent-import case safe.
    """
    global _dsp_prewarm_started
    if _dsp_prewarm_started:
        return
    _dsp_prewarm_started = True

    def _prewarm() -> None:
        try:
            import impulcifer  # noqa: F401
        except Exception:
            pass

    threading.Thread(target=_prewarm, name="dsp-prewarm", daemon=True).start()


_brir_field_kinds_cache: dict[str, str] | None = None


def _brir_field_kinds() -> dict[str, str]:
    """Map every ``ProcessingConfig`` field to a JSON validation kind.

    Derived from the dataclass so that new pipeline parameters become
    available to frontends without touching this module. ``dir_path`` is
    validated separately because it is required.
    """
    global _brir_field_kinds_cache
    if _brir_field_kinds_cache is None:
        from dataclasses import fields as dataclass_fields

        from core.pipeline import ProcessingConfig

        kinds: dict[str, str] = {}
        for config_field in dataclass_fields(ProcessingConfig):
            name = config_field.name
            if name == "dir_path":
                continue
            if name == "decay":
                kinds[name] = "decay"
            elif name == "channel_balance":
                kinds[name] = "balance"
            elif name == "fs":
                kinds[name] = "optional_int"
            elif name == "target_level":
                kinds[name] = "optional_float"
            elif name in _BRIR_PATH_FIELDS:
                kinds[name] = "path"
            elif name in _BRIR_CHOICE_FIELDS:
                kinds[name] = "choice"
            elif isinstance(config_field.default, bool):
                kinds[name] = "bool"
            elif isinstance(config_field.default, int):
                kinds[name] = "int"
            elif isinstance(config_field.default, float):
                kinds[name] = "float"
            elif isinstance(config_field.default, str):
                kinds[name] = "str"
            else:
                kinds[name] = "any"
        _brir_field_kinds_cache = kinds
    return _brir_field_kinds_cache


_brir_field_defaults_cache: dict[str, Any] | None = None


def _brir_field_defaults() -> dict[str, Any]:
    """Map every defaulted ``ProcessingConfig`` field to its canonical default.

    Shipped to frontends via ``bootstrap()`` so they never hardcode copies of
    pipeline defaults.
    """
    global _brir_field_defaults_cache
    if _brir_field_defaults_cache is None:
        from dataclasses import MISSING, fields as dataclass_fields

        from core.pipeline import ProcessingConfig

        defaults: dict[str, Any] = {}
        for config_field in dataclass_fields(ProcessingConfig):
            if config_field.name == "dir_path":
                continue
            if config_field.default is not MISSING:
                defaults[config_field.name] = config_field.default
            elif config_field.default_factory is not MISSING:
                defaults[config_field.name] = config_field.default_factory()
        _brir_field_defaults_cache = defaults
    return _brir_field_defaults_cache


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
        # Deferred final action of a finished update job (e.g. Velopack's
        # apply-and-restart); staged so the frontend can show the completion
        # message first and apply on user confirmation, mirroring the CTk
        # UpdateDialog messagebox → after_message sequence.
        self._pending_update_apply: Callable[[], Any] | None = None

    def bootstrap(self) -> dict[str, Any]:
        from i18n.localization import get_localization_manager

        # The DSP stack (impulcifer → core → scipy …) must never be able to
        # kill the UI shell: a packaging regression there should surface as a
        # readable job/system-info error, not as a dead bootstrap that leaves
        # raw i18n keys on screen. infra.version is deliberately DSP-free —
        # ``import impulcifer`` would pull scipy/matplotlib/bokeh into the
        # WebView's first paint and cost seconds.
        try:
            from infra.version import get_app_version

            version = get_app_version()
        except Exception:
            version = "unknown"

        # Same isolation rule as the version probe above: pipeline defaults
        # come from core.pipeline, and a broken DSP bundle must degrade to
        # missing defaults (frontends keep their literal fallbacks), not to a
        # dead bootstrap.
        try:
            brir_defaults = _brir_field_defaults()
        except Exception:
            brir_defaults = {}

        # core.constants is import-light on purpose; do NOT source these
        # from core.sweep_signal here (that would drag scipy into bootstrap).
        try:
            from core.constants import (
                DEFAULT_SWEEP_DURATION,
                DEFAULT_SWEEP_FS,
                SPEAKER_NAMES,
                SWEEP_TRACK_LAYOUTS,
            )

            sweep_info = {
                "layouts": list(SWEEP_TRACK_LAYOUTS),
                "default_fs": DEFAULT_SWEEP_FS,
                "default_duration": DEFAULT_SWEEP_DURATION,
                "speaker_names": list(SPEAKER_NAMES),
            }
        except Exception:
            sweep_info = {}

        with self._lock:
            active_job = self._jobs.get(self._active_job_id or "")
            active = active_job.snapshot() if active_job is not None else None

        _start_dsp_prewarm()

        return _ok(
            {
                "version": version,
                # infra.environment.normalized_platform()과 동일 어휘
                # (windows/darwin/linux) — JSON API 계약 표면이므로 유지.
                "platform": platform.system().lower(),
                "brir_defaults": brir_defaults,
                "sweep": sweep_info,
                "capabilities": {
                    "recording": True,
                    "brir": True,
                    "output_recovery": True,
                    "recording_cancel": False,
                    "brir_cancel": True,
                    "output_recovery_cancel": False,
                },
                "active_job": active,
                "ui": self._ui_settings_payload(get_localization_manager()),
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

            play_signal = None
            if params["sweep_spec"] is not None:
                from core.sweep_signal import build_sweep_playback

                play_signal = build_sweep_playback(params["sweep_spec"])
            try:
                recorder.play_and_record(
                    play=params["play_path"],
                    play_signal=play_signal,
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
                    # A generated headphone sweep is already a stereo L→R
                    # sequence; the mono broadcast only applies to files.
                    mono_to_stereo=params["mode"] == "headphones" and play_signal is None,
                )
            except recorder.DeviceNotFoundError as exc:
                raise _ServiceFailure("DEVICE_ERROR", str(exc), retryable=True) from exc
            if not os.path.isfile(params["record_path"]):
                raise _ServiceFailure(
                    "OUTPUT_MISSING",
                    "Recording finished without producing the expected file.",
                    details={"record_path": params["record_path"]},
                )
            sidecar_path = None
            if (
                play_signal is not None
                and params["mode"] == "speakers"
                and not play_signal.spec.is_default_signal()
            ):
                # Custom-parameter captures must stay self-describing: the
                # BRIR pipeline resolves <dir>/test.wav before any bundled
                # default, so later processing picks up this exact sweep.
                from core.sweep_signal import write_sidecar

                sidecar_path = write_sidecar(
                    os.path.dirname(params["record_path"]), play_signal.estimator
                )
            summary = None
            try:
                # Tk-free analysis helper (numpy/soundfile only) shared with
                # the CTk RecordingStatusController's completion summary.
                from gui.recording_status import analyze_recording

                analyzed = analyze_recording(params["record_path"])
                if analyzed is not None:
                    summary = asdict(analyzed)
            except Exception:
                summary = None
            return {
                "mode": params["mode"],
                "record_path": params["record_path"],
                "summary": summary,
                "sweep": play_signal.display_name if play_signal is not None else None,
                "sidecar_path": sidecar_path,
            }

        return self._start_job("recording", False, run)

    def start_brir(self, request: dict[str, Any]) -> dict[str, Any]:
        validation = self._validate_brir_request(request)
        if not validation["ok"]:
            return validation
        params = validation["data"]["params"]
        eq_sidecars = validation["data"]["eq_sidecars"]

        def run(job_id: str, cancel_event: threading.Event) -> dict[str, Any]:
            import impulcifer
            from core.cancellation import CancelledError, cancellation_scope
            from infra.logger import get_logger

            if params.get("do_equalization", True):
                for target_name, source in eq_sidecars.items():
                    shutil.copy2(source, os.path.join(params["dir_path"], target_name))

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
                with cancellation_scope(cancel_event):
                    impulcifer.main(**params)
            except CancelledError:
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

    def start_output_recovery(self, request: dict[str, Any]) -> dict[str, Any]:
        """Restore missing HRIR/HeSuVi/Hangloose outputs by channel reordering."""
        validation = self._validate_output_recovery_request(request)
        if not validation["ok"]:
            return validation
        params = validation["data"]["params"]

        def run(_job_id: str, _cancel_event: threading.Event) -> dict[str, Any]:
            from core.brir_recovery import BrirRecoveryError, recover_brir_outputs

            try:
                result = recover_brir_outputs(**params)
            except BrirRecoveryError as exc:
                raise _ServiceFailure(
                    exc.code,
                    str(exc),
                    details=exc.details,
                ) from None
            return asdict(result)

        return self._start_job("output_recovery", False, run)

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
                    f"{job.kind} jobs cannot be cancelled safely.",
                    details={"job_id": job_id},
                )
            job.status = "cancel_requested"
            job.cancel_event.set()
            self._append_event(job, "status", {"status": job.status})
            return _ok({"job": job.snapshot()})

    # ------------------------------------------------------------------
    # UI settings / environment (shared with the CTk GUI via
    # LocalizationManager's ~/.impulcifer/settings.json persistence)
    # ------------------------------------------------------------------

    def get_ui_settings(self) -> dict[str, Any]:
        from i18n.localization import get_localization_manager

        return _ok(self._ui_settings_payload(get_localization_manager()))

    def set_language(self, language_code: str) -> dict[str, Any]:
        from i18n.localization import SUPPORTED_LANGUAGES, get_localization_manager

        if language_code not in SUPPORTED_LANGUAGES:
            return _error(
                "INVALID_REQUEST",
                "Unsupported language code.",
                details={"language": language_code},
            )
        loc = get_localization_manager()
        loc.set_language(language_code)
        if hasattr(loc, "mark_language_selected"):
            loc.mark_language_selected()
        return _ok(self._ui_settings_payload(loc))

    def set_theme(self, theme: str) -> dict[str, Any]:
        from i18n.localization import get_localization_manager

        if theme not in _THEME_CODES:
            return _error(
                "INVALID_REQUEST",
                "Theme must be one of: " + ", ".join(_THEME_CODES) + ".",
                details={"theme": theme},
            )
        get_localization_manager().set_theme(theme)
        return _ok({"theme": theme})

    def set_skin(self, skin: str) -> dict[str, Any]:
        from i18n.localization import get_localization_manager

        if skin not in _SKIN_CODES:
            return _error(
                "INVALID_REQUEST",
                "Skin must be one of: " + ", ".join(_SKIN_CODES) + ".",
                details={"skin": skin},
            )
        get_localization_manager().set_skin(skin)
        return _ok({"skin": skin})

    def set_frontend(self, frontend: str) -> dict[str, Any]:
        from i18n.localization import get_localization_manager

        if frontend not in _FRONTEND_CODES:
            return _error(
                "INVALID_REQUEST",
                "Frontend must be one of: " + ", ".join(_FRONTEND_CODES) + ".",
                details={"frontend": frontend},
            )
        get_localization_manager().set_frontend(frontend)
        return _ok({"frontend": frontend})

    @staticmethod
    def _ui_settings_payload(loc: Any) -> dict[str, Any]:
        from i18n.localization import SUPPORTED_LANGUAGES

        return {
            "language": loc.current_language,
            "theme": loc.get_theme(),
            "skin": loc.get_skin(),
            # Older LocalizationManager instances (or test fakes) may predate
            # the frontend setting; default matches get_frontend().
            "frontend": loc.get_frontend() if hasattr(loc, "get_frontend") else "webview",
            # First run → the frontend shows a language picker (CTk parity).
            "first_run": bool(loc.is_first_run()) if hasattr(loc, "is_first_run") else False,
            "languages": [
                {"code": code, "name": name} for code, name in SUPPORTED_LANGUAGES.items()
            ],
            "strings": _load_strings(loc.locales_dir, loc.current_language),
        }

    def get_system_info(self) -> dict[str, Any]:
        try:
            from impulcifer import __version__ as version
        except Exception:
            version = "unknown"

        install_kind = "dev"
        try:
            from infra.environment import get_install_kind

            install_kind = get_install_kind()
        except Exception:
            pass

        try:
            from core.parallel_processing import get_python_threading_info

            threading_info = get_python_threading_info()
        except Exception:
            threading_info = {}
        return _ok(
            {
                "version": version,
                "install_kind": install_kind,
                "python_version": threading_info.get("python_version"),
                "os": f"{platform.system()} {platform.release()}",
                "cpu_count": threading_info.get("cpu_count"),
                "gil_enabled": threading_info.get("gil_enabled"),
                "optimal_workers": threading_info.get("optimal_workers"),
            }
        )

    # ------------------------------------------------------------------
    # Recorder helpers
    # ------------------------------------------------------------------

    def resolve_recording_paths(
        self,
        record_dir: Any,
        play_path: Any = None,
        mode: str = "speakers",
        sweep: Any = None,
    ) -> dict[str, Any]:
        """Preview the canonical recording output path for the given inputs."""
        from core.recording_naming import (
            resolve_headphones_record_path,
            resolve_record_path,
            resolve_record_path_for_speakers,
        )

        target_dir = _optional_string(record_dir)
        if target_dir is None:
            return _error("INVALID_REQUEST", "record_dir is required.")
        if mode == "headphones":
            return _ok({"record_path": resolve_headphones_record_path(target_dir)})
        sweep_validation = self._validate_sweep_request(sweep, mode)
        if not sweep_validation["ok"]:
            return sweep_validation
        spec = sweep_validation["data"]["spec"]
        if spec is not None:
            return _ok({"record_path": resolve_record_path_for_speakers(target_dir, spec.speakers)})
        play = _optional_string(play_path)
        if play is None:
            return _error("INVALID_REQUEST", "play_path is required for speaker recordings.")
        return _ok({"record_path": resolve_record_path(target_dir, play)})

    def detect_sweep(self, dir_path: Any) -> dict[str, Any]:
        """Estimate which sweep the folder's recordings were captured with."""
        target = _optional_string(dir_path)
        if target is None or not os.path.isdir(target):
            return _error(
                "FILE_NOT_FOUND",
                "Measurement directory does not exist.",
                details={"path": dir_path},
            )
        try:
            from core.sweep_detection import detect_sweep_parameters

            result = detect_sweep_parameters(target)
        except Exception as exc:
            return _error("INTERNAL_ERROR", str(exc) or exc.__class__.__name__)
        sidecar = os.path.isfile(os.path.join(target, "test.wav"))
        if result is None:
            return _ok({"found": False, "sidecar": sidecar})
        return _ok(
            {
                "found": True,
                "sidecar": sidecar,
                "fs": result.fs,
                "duration_seconds": round(result.duration_seconds, 4),
                "n_segments": result.n_segments,
                "speakers": list(result.speakers),
                "confidence": result.confidence,
                "is_default": result.is_default,
                "generate_spec": result.generate_spec(),
                "source_files": list(result.source_files),
            }
        )

    def generate_sweep_set(self, dir_path: Any) -> dict[str, Any]:
        target = _optional_string(dir_path)
        if target is None or not os.path.isdir(target):
            return _error(
                "FILE_NOT_FOUND",
                "Sweep set directory does not exist.",
                details={"path": dir_path},
            )
        try:
            from core.sweep_set_generator import generate_sweep_set

            files = generate_sweep_set(target)
        except Exception as exc:
            return _error("INTERNAL_ERROR", str(exc) or exc.__class__.__name__)
        return _ok({"files": files, "play_path": files[0] if files else None})

    def open_path(self, path: Any = None) -> dict[str, Any]:
        """Open a folder in the OS file explorer; defaults to the data folder."""
        if path is None:
            from infra.resource_helper import DATA_DIR

            path = DATA_DIR
        target = _optional_string(path)
        if target is None or not os.path.isdir(target):
            return _error("FILE_NOT_FOUND", "Folder does not exist.", details={"path": path})
        try:
            system = platform.system()
            if system == "Windows":
                os.startfile(target)  # noqa: S606
            elif system == "Darwin":
                subprocess.Popen(["open", target])
            else:
                subprocess.Popen(["xdg-open", target])
        except Exception as exc:
            return _error("INTERNAL_ERROR", str(exc) or exc.__class__.__name__)
        return _ok({"path": target})

    # ------------------------------------------------------------------
    # Auto-update (ports the CTk check_for_updates_background → UpdateDialog
    # → UpdateExecutor flow onto the JSON job model)
    # ------------------------------------------------------------------

    def check_for_updates(self) -> dict[str, Any]:
        """Query GitHub releases for a newer version (blocking, ~10s max)."""
        from impulcifer import __version__
        from updater.update_checker import UpdateChecker

        try:
            checker = UpdateChecker(__version__)
            available, latest_version, download_url = checker.check_for_updates()
        except Exception as exc:
            return _error(
                "UPDATE_CHECK_FAILED",
                str(exc) or exc.__class__.__name__,
                retryable=True,
            )
        # Same gate as the CTk background check: only prompt when there is
        # both a newer version and something to download.
        update_available = bool(available and download_url)
        return _ok(
            {
                "update_available": update_available,
                "current_version": __version__,
                "latest_version": latest_version,
                "download_url": download_url,
                "release_notes": checker.get_release_notes() if update_available else None,
                "release_url": checker.get_release_url(),
            }
        )

    def start_update(self, request: dict[str, Any]) -> dict[str, Any]:
        """Run the environment-appropriate update executor as a job."""
        if not isinstance(request, dict):
            return _error("INVALID_REQUEST", "Request must be an object.")
        download_url = _optional_string(request.get("download_url"))
        latest_version = _optional_string(request.get("latest_version"))
        if latest_version is None:
            return _error("INVALID_REQUEST", "latest_version is required.")

        def run(job_id: str, _cancel_event: threading.Event) -> dict[str, Any]:
            from updater.updater_core import UpdateExecutionError, create_update_executor

            executor = create_update_executor(download_url or "", latest_version)

            def progress(value: float, message: str = "") -> None:
                # ``message`` is either an i18n key ("update_downloading") or
                # preformatted text ("Downloading: 42%"); the frontend
                # resolves keys and falls back to the raw string.
                self._emit(
                    job_id,
                    "progress",
                    {
                        "progress": max(0.0, min(1.0, float(value))),
                        "message": message,
                    },
                )

            try:
                result = executor.execute(progress)
            except UpdateExecutionError as exc:
                raise _ServiceFailure("UPDATE_FAILED", str(exc), retryable=True) from exc
            with self._lock:
                self._pending_update_apply = result.after_message
            return {
                "status_key": result.status_key,
                "status_default": result.status_default,
                "title_key": result.title_key,
                "title_default": result.title_default,
                "message_key": result.message_key,
                "message_default": result.message_default,
                "progress": result.progress,
                "requires_restart": result.after_message is not None,
            }

        return self._start_job("update", False, run)

    def apply_pending_update(self) -> dict[str, Any]:
        """Run the staged post-update action (Velopack apply-and-restart)."""
        with self._lock:
            apply_fn = self._pending_update_apply
            self._pending_update_apply = None
        if apply_fn is None:
            return _error("INVALID_REQUEST", "No staged update to apply.")
        try:
            applied = apply_fn()
        except SystemExit:
            # VelopackUpdater.apply_and_restart hands over to Update.exe and
            # calls sys.exit(); on a bridge worker thread that only unwinds
            # this frame, so reaching here means the handover succeeded and
            # the frontend should close the window.
            return _ok({"restarting": True})
        except Exception as exc:
            return _error(
                "UPDATE_FAILED",
                str(exc) or exc.__class__.__name__,
                retryable=True,
            )
        if applied is False:
            return _error("UPDATE_FAILED", "Failed to apply update.", retryable=True)
        return _ok({"restarting": True})

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
        record_dir = str(request.get("record_dir", "")).strip()
        if mode not in {"speakers", "headphones"}:
            return _error("INVALID_REQUEST", "mode must be speakers or headphones.")
        if not record_dir:
            return _error("INVALID_REQUEST", "record_dir is required.")

        sweep_validation = self._validate_sweep_request(request.get("sweep"), mode)
        if not sweep_validation["ok"]:
            return sweep_validation
        sweep_spec = sweep_validation["data"]["spec"]

        play_path: str | None = str(request.get("play_path", "")).strip()
        if sweep_spec is not None:
            play_path = None
        elif not play_path or not os.path.isfile(play_path):
            return _error("FILE_NOT_FOUND", "Playback file does not exist.", details={"path": play_path})

        channels = request.get("channels", 2)
        if isinstance(channels, bool) or not isinstance(channels, int) or not 1 <= channels <= 64:
            return _error("INVALID_REQUEST", "channels must be an integer from 1 to 64.")
        force_channels = request.get("force_channels", False)
        if not isinstance(force_channels, bool):
            return _error("INVALID_REQUEST", "force_channels must be a boolean.")
        confirm = request.get("confirm_warnings", False)
        if not isinstance(confirm, bool):
            return _error("INVALID_REQUEST", "confirm_warnings must be a boolean.")

        from core.headphones_recording import inspect_headphones_playback
        from core.recording_naming import (
            resolve_headphones_record_path,
            resolve_record_path,
            resolve_record_path_for_speakers,
        )
        from core.recording_validation import validate_recording_setup

        if mode == "headphones":
            if sweep_spec is None:
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
            # CTk parity: the channel count is only honored — and the
            # speaker-count mismatch warning only raised — when the user
            # explicitly forces channels. The default path records 2ch
            # without a confirmation prompt.
            if not force_channels:
                channels = 2
            if sweep_spec is not None:
                record_path = resolve_record_path_for_speakers(record_dir, sweep_spec.speakers)
            else:
                record_path = resolve_record_path(record_dir, play_path)
            channel_validation = validate_recording_setup(record_path, channels, force_channels)
            if channel_validation and channel_validation.has_mismatch and not confirm:
                return _error(
                    "CONFIRMATION_REQUIRED",
                    "Selected recording channels do not match the sweep speaker count.",
                    details=asdict(channel_validation),
                )

        debug_plots = request.get("debug_plots", False)
        if not isinstance(debug_plots, bool):
            return _error("INVALID_REQUEST", "debug_plots must be a boolean.")
        # Internal-only payload — deliberately NOT `_ok()`-wrapped data
        # semantics: `_json_safe` would flatten the SweepSpec dataclass to a
        # dict before `run()` could use it.
        return {
            "ok": True,
            "data": {
                "mode": mode,
                "play_path": play_path,
                "sweep_spec": sweep_spec,
                "record_path": record_path,
                "input_device": _optional_string(request.get("input_device")),
                "output_device": _optional_string(request.get("output_device")),
                "host_api": _optional_string(request.get("host_api")),
                "channels": channels,
                "append": append,
                "debug_plots": debug_plots,
            },
        }

    @staticmethod
    def _validate_sweep_request(sweep: Any, mode: str) -> dict[str, Any]:
        """Validate the optional on-the-fly sweep sub-object.

        Returns ``spec=None`` for file mode (or an absent object) so the
        caller can fall through to the legacy play_path flow. The OK result
        is a raw dict (not ``_ok``) because the SweepSpec dataclass must
        survive for internal consumers.
        """
        if sweep is None:
            return {"ok": True, "data": {"spec": None}}
        if not isinstance(sweep, dict):
            return _error("INVALID_REQUEST", "sweep must be an object.")
        unknown = sorted(set(sweep) - _SWEEP_REQUEST_FIELDS)
        if unknown:
            return _error("INVALID_REQUEST", "Unknown sweep fields.", details={"fields": unknown})
        sweep_mode = sweep.get("mode", "default")
        if sweep_mode not in _SWEEP_MODES:
            return _error("INVALID_REQUEST", "sweep.mode must be default, custom or file.")
        if sweep_mode == "file":
            return {"ok": True, "data": {"spec": None}}

        from core.sweep_signal import (
            DEFAULT_SWEEP_DURATION,
            DEFAULT_SWEEP_FS,
            SweepSpec,
            validate_sweep_spec,
        )

        if mode == "headphones":
            # Headphone compensation always plays the L→R stereo sequence;
            # only the signal parameters (fs/duration) are selectable.
            speakers: tuple = ("FL", "FR")
            tracks = "stereo"
        else:
            speakers_value = sweep.get("speakers", ["FL", "FR"])
            if isinstance(speakers_value, str):
                speakers = tuple(part.strip() for part in speakers_value.split(",") if part.strip())
            elif isinstance(speakers_value, (list, tuple)):
                speakers = tuple(str(part) for part in speakers_value)
            else:
                return _error(
                    "INVALID_REQUEST",
                    "sweep.speakers must be a list or comma-separated string.",
                )
            tracks = sweep.get("tracks", "stereo")
            if not isinstance(tracks, str):
                return _error("INVALID_REQUEST", "sweep.tracks must be a string.")

        if sweep_mode == "custom":
            fs = sweep.get("fs", DEFAULT_SWEEP_FS)
            duration = sweep.get("duration", DEFAULT_SWEEP_DURATION)
            if (
                isinstance(fs, bool)
                or not isinstance(fs, (int, float))
                or (isinstance(fs, float) and not fs.is_integer())
            ):
                return _error("INVALID_REQUEST", "sweep.fs must be an integer.")
            if isinstance(duration, bool) or not isinstance(duration, (int, float)):
                return _error("INVALID_REQUEST", "sweep.duration must be a number.")
        else:
            fs = DEFAULT_SWEEP_FS
            duration = DEFAULT_SWEEP_DURATION

        try:
            spec = validate_sweep_spec(
                SweepSpec(
                    fs=int(fs),
                    duration=float(duration),
                    speakers=speakers,
                    tracks=tracks,
                )
            )
        except ValueError as exc:
            return _error("INVALID_REQUEST", str(exc))
        return {"ok": True, "data": {"spec": spec}}

    @staticmethod
    def _validate_output_recovery_request(request: dict[str, Any]) -> dict[str, Any]:
        if not isinstance(request, dict):
            return _error("INVALID_REQUEST", "Output recovery request must be an object.")
        unknown = sorted(set(request) - {"dir_path", "include_hangloose"})
        if unknown:
            return _error(
                "INVALID_REQUEST",
                "Unknown output recovery fields.",
                details={"fields": unknown},
            )
        dir_path = request.get("dir_path")
        if not isinstance(dir_path, str) or not dir_path.strip():
            return _error("INVALID_REQUEST", "dir_path must be a directory path.")
        dir_path = dir_path.strip()
        if not os.path.isdir(dir_path):
            return _error(
                "FILE_NOT_FOUND",
                "Recovery directory does not exist.",
                details={"path": dir_path},
            )
        include_hangloose = request.get("include_hangloose", False)
        if not isinstance(include_hangloose, bool):
            return _error("INVALID_REQUEST", "include_hangloose must be a boolean.")
        return _ok(
            {
                "params": {
                    "directory": dir_path,
                    "include_hangloose": include_hangloose,
                }
            }
        )

    @staticmethod
    def _validate_brir_request(request: dict[str, Any]) -> dict[str, Any]:
        if not isinstance(request, dict):
            return _error("INVALID_REQUEST", "BRIR request must be an object.")
        kinds = _brir_field_kinds()
        allowed = set(kinds) | set(_BRIR_SIDECAR_EQ_FILES) | {"dir_path"}
        unknown = sorted(set(request) - allowed)
        if unknown:
            return _error("INVALID_REQUEST", "Unknown BRIR fields.", details={"fields": unknown})
        dir_path = str(request.get("dir_path", "")).strip()
        if not dir_path or not os.path.isdir(dir_path):
            return _error("FILE_NOT_FOUND", "Measurement directory does not exist.", details={"path": dir_path})

        params: dict[str, Any] = {"dir_path": dir_path}
        for name, value in request.items():
            if name == "dir_path" or name in _BRIR_SIDECAR_EQ_FILES:
                continue
            kind = kinds[name]
            if kind == "bool":
                if not isinstance(value, bool):
                    return _error("INVALID_REQUEST", f"{name} must be a boolean.")
                params[name] = value
            elif kind in ("int", "optional_int"):
                if value is None and kind == "optional_int":
                    params[name] = None
                    continue
                if (
                    isinstance(value, bool)
                    or not isinstance(value, (int, float))
                    or (isinstance(value, float) and not value.is_integer())
                ):
                    return _error("INVALID_REQUEST", f"{name} must be an integer.")
                params[name] = int(value)
            elif kind in ("float", "optional_float"):
                if value is None and kind == "optional_float":
                    params[name] = None
                    continue
                if isinstance(value, bool) or not isinstance(value, (int, float)):
                    return _error("INVALID_REQUEST", f"{name} must be a number.")
                params[name] = float(value)
            elif kind == "path":
                if value is not None and not isinstance(value, str):
                    return _error("INVALID_REQUEST", f"{name} must be a string path.")
                params[name] = _optional_string(value)
            elif kind == "choice":
                choices = _BRIR_CHOICE_FIELDS[name]
                if value not in choices:
                    return _error(
                        "INVALID_REQUEST",
                        f"{name} must be one of: {', '.join(choices)}.",
                    )
                params[name] = value
            elif kind == "balance":
                if value is None:
                    continue
                if not isinstance(value, bool) and isinstance(value, (int, float)):
                    params[name] = value
                elif isinstance(value, str) and value in _CHANNEL_BALANCE_TOKENS:
                    params[name] = value
                else:
                    return _error(
                        "INVALID_REQUEST",
                        "channel_balance must be a dB number or one of: "
                        + ", ".join(_CHANNEL_BALANCE_TOKENS)
                        + ".",
                    )
            elif kind == "decay":
                if value is None:
                    continue
                decay = _validate_decay(value)
                if decay is None:
                    return _error(
                        "INVALID_REQUEST",
                        "decay must be a positive number of seconds or a"
                        " {channel: seconds} object for FL/FC/FR/SL/SR/BL/BR.",
                    )
                params[name] = decay
            elif kind == "str":
                if not isinstance(value, str):
                    return _error("INVALID_REQUEST", f"{name} must be a string.")
                params[name] = value
            else:
                params[name] = value

        if isinstance(params.get("vbass_freq"), int):
            params["vbass_freq"] = max(30, min(500, params["vbass_freq"]))

        sidecars: dict[str, str] = {}
        for field_name, target_name in _BRIR_SIDECAR_EQ_FILES.items():
            raw = request.get(field_name)
            if raw is not None and not isinstance(raw, str):
                return _error("INVALID_REQUEST", f"{field_name} must be a string path.")
            text = _optional_string(raw)
            if text is None or text == target_name:
                continue
            source = text if os.path.isabs(text) else os.path.join(dir_path, text)
            if os.path.abspath(source) == os.path.abspath(os.path.join(dir_path, target_name)):
                continue
            if not os.path.isfile(source):
                return _error(
                    "FILE_NOT_FOUND",
                    "Custom EQ file does not exist.",
                    details={"field": field_name, "path": text},
                )
            sidecars[target_name] = source
        return _ok({"params": params, "eq_sidecars": sidecars})


def _optional_string(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _validate_decay(value: Any) -> Optional[dict[str, float]]:
    """Normalize a decay request (seconds) to a per-channel dict, or None."""
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        if value <= 0:
            return None
        return {channel: float(value) for channel in _DECAY_CHANNELS}
    if isinstance(value, dict):
        result: dict[str, float] = {}
        for channel, item in value.items():
            if channel not in _DECAY_CHANNELS:
                return None
            if isinstance(item, bool) or not isinstance(item, (int, float)) or item <= 0:
                return None
            result[str(channel)] = float(item)
        return result or None
    return None


def _load_strings(locales_dir: Path, language_code: str) -> dict[str, str]:
    """Merge the English locale (fallback) with the requested language."""
    import json

    merged: dict[str, str] = {}
    codes = ["en"] if language_code == "en" else ["en", language_code]
    for code in codes:
        try:
            with open(locales_dir / f"{code}.json", encoding="utf-8") as handle:
                merged.update(json.load(handle))
        except (OSError, ValueError):
            continue
    return merged


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
