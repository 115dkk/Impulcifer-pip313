"""Experimental pywebview frontend for the Impulcifer application service."""

from __future__ import annotations

from pathlib import Path
import platform
from typing import Any

from application import ImpulciferApplicationService
from infra.resource_helper import get_resource_path


class WebviewBridge:
    """Expose only the application service's JSON-safe public methods."""

    def __init__(self, service: ImpulciferApplicationService | None = None) -> None:
        self._service = service or ImpulciferApplicationService()

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


def _index_uri() -> str:
    return Path(get_resource_path("webview_ui/index.html")).resolve().as_uri()


def main() -> None:
    """Start the Windows Edge WebView2 proof-of-concept frontend."""
    if platform.system() != "Windows":
        raise SystemExit("The experimental WebView frontend currently supports Windows only.")

    try:
        import webview
    except ImportError as exc:
        raise SystemExit(
            "pywebview is not installed. Install the WebView extra with "
            "'pip install .[webview]'."
        ) from exc

    webview.create_window(
        "Impulcifer WebView Preview",
        _index_uri(),
        js_api=WebviewBridge(),
        width=1180,
        height=820,
        min_size=(900, 640),
    )
    webview.start(gui="edgechromium", debug=False)


if __name__ == "__main__":
    main()
