"""Experimental pywebview frontend for the Impulcifer application service."""

from __future__ import annotations

from pathlib import Path
import platform
from typing import Any

from application import ImpulciferApplicationService
from infra.resource_helper import get_resource_path

# Native file dialog filters, mirroring gui/constants.py FILETYPES_*.
_FILE_DIALOG_FILTERS: dict[str, tuple[str, ...]] = {
    "audio": ("Audio files (*.wav;*.mlp;*.thd;*.truehd)", "All files (*.*)"),
    "audio_pkl": (
        "Audio / estimator files (*.wav;*.pkl;*.mlp;*.thd;*.truehd)",
        "All files (*.*)",
    ),
    "text": ("EQ / CSV files (*.csv;*.txt)", "All files (*.*)"),
    "wav": ("WAV files (*.wav)", "All files (*.*)"),
}

# open_url() is allowlist-only so the JS side can never navigate the host
# browser to an arbitrary address.
_PROJECT_URLS = {
    "original_repo": "https://github.com/jaakkopasanen/Impulcifer",
    "fork_repo": "https://github.com/115dkk/Impulcifer-pip313",
    "report_bug": "https://github.com/115dkk/Impulcifer-pip313/issues/new",
    "license": "https://github.com/115dkk/Impulcifer-pip313/blob/master/LICENSE",
}


def _ok(data: dict[str, Any]) -> dict[str, Any]:
    return {"ok": True, "data": data}


def _error(code: str, message: str) -> dict[str, Any]:
    return {
        "ok": False,
        "error": {"code": code, "message": message, "details": {}, "retryable": False},
    }


class WebviewBridge:
    """Expose only the application service's JSON-safe public methods."""

    def __init__(self, service: ImpulciferApplicationService | None = None) -> None:
        self._service = service or ImpulciferApplicationService()
        self._window: Any = None

    def attach_window(self, window: Any) -> None:
        """Give the bridge the window handle needed for native dialogs."""
        self._window = window

    def bootstrap(self) -> dict[str, Any]:
        return self._service.bootstrap()

    def list_audio_devices(self, host_api: str | None = None) -> dict[str, Any]:
        return self._service.list_audio_devices(host_api)

    def start_recording(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._service.start_recording(request)

    def start_brir(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._service.start_brir(request)

    def poll_job(self, job_id: str, after_seq: int = 0) -> dict[str, Any]:
        return self._service.poll_job(job_id, after_seq)

    def cancel_job(self, job_id: str) -> dict[str, Any]:
        return self._service.cancel_job(job_id)

    def get_ui_settings(self) -> dict[str, Any]:
        return self._service.get_ui_settings()

    def set_language(self, language_code: str) -> dict[str, Any]:
        return self._service.set_language(language_code)

    def set_theme(self, theme: str) -> dict[str, Any]:
        return self._service.set_theme(theme)

    def get_system_info(self) -> dict[str, Any]:
        return self._service.get_system_info()

    def resolve_recording_paths(
        self,
        record_dir: Any,
        play_path: Any = None,
        mode: str = "speakers",
    ) -> dict[str, Any]:
        return self._service.resolve_recording_paths(record_dir, play_path, mode)

    def generate_sweep_set(self, dir_path: Any) -> dict[str, Any]:
        return self._service.generate_sweep_set(dir_path)

    def open_path(self, path: Any = None) -> dict[str, Any]:
        return self._service.open_path(path)

    def select_file(self, kind: str = "audio") -> dict[str, Any]:
        return self._create_file_dialog("open", kind)

    def select_directory(self) -> dict[str, Any]:
        return self._create_file_dialog("folder", None)

    def open_url(self, name: str) -> dict[str, Any]:
        url = _PROJECT_URLS.get(name)
        if url is None:
            return _error("INVALID_REQUEST", "Unknown project link.")
        import webbrowser

        webbrowser.open(url)
        return _ok({"url": url})

    def _create_file_dialog(self, mode: str, kind: str | None) -> dict[str, Any]:
        if self._window is None:
            return _error("NO_WINDOW", "Native dialogs are unavailable before startup.")
        import webview

        dialog_enum = getattr(webview, "FileDialog", None)
        if mode == "folder":
            dialog_type = dialog_enum.FOLDER if dialog_enum else webview.FOLDER_DIALOG
            selection = self._window.create_file_dialog(dialog_type, allow_multiple=False)
        else:
            dialog_type = dialog_enum.OPEN if dialog_enum else webview.OPEN_DIALOG
            file_types = _FILE_DIALOG_FILTERS.get(kind or "", _FILE_DIALOG_FILTERS["audio"])
            selection = self._window.create_file_dialog(
                dialog_type,
                allow_multiple=False,
                file_types=file_types,
            )
        path = None
        if selection:
            path = selection[0] if isinstance(selection, (list, tuple)) else selection
        return _ok({"path": path})


def _index_uri() -> str:
    return Path(get_resource_path("webview_ui/index.html")).resolve().as_uri()


def main() -> None:
    """Start the Windows Edge WebView2 frontend."""
    if platform.system() != "Windows":
        raise SystemExit("The experimental WebView frontend currently supports Windows only.")

    try:
        import webview
    except ImportError as exc:
        raise SystemExit(
            "pywebview is not installed. Install the WebView extra with "
            "'pip install .[webview]'."
        ) from exc

    bridge = WebviewBridge()
    window = webview.create_window(
        "Impulcifer WebView Preview",
        _index_uri(),
        js_api=bridge,
        width=1280,
        height=860,
        min_size=(980, 640),
    )
    bridge.attach_window(window)
    webview.start(gui="edgechromium", debug=False)


if __name__ == "__main__":
    main()
