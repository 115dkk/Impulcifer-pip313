"""Platform WebView frontend for the Impulcifer application service."""

from __future__ import annotations

from pathlib import Path
import platform
import time
from typing import Any

from application import ImpulciferApplicationService
from infra.resource_helper import get_resource_path

# Native file dialog filters, mirroring gui/constants.py FILETYPES_*.
_FILE_DIALOG_FILTERS: dict[str, tuple[str, ...]] = {
    "audio": ("Audio files (*.wav;*.mlp;*.thd;*.truehd)", "All files (*.*)"),
    "text": ("EQ / CSV files (*.csv;*.txt)", "All files (*.*)"),
    "wav": ("WAV files (*.wav)", "All files (*.*)"),
}

# platform.system() → forced pywebview GUI backend. Forcing keeps startup
# deterministic (pywebview would otherwise prefer Qt inside a KDE session) and
# pins exactly the three engines that webview-backend-validation.yml verifies.
_GUI_BACKENDS: dict[str, str] = {
    "Windows": "edgechromium",  # Microsoft Edge WebView2
    "Darwin": "cocoa",  # WKWebView
    "Linux": "gtk",  # WebKit2GTK
}


def select_gui_backend() -> str:
    """Return the pywebview ``gui=`` value for the current platform."""
    backend = _GUI_BACKENDS.get(platform.system())
    if backend is None:
        raise SystemExit(
            "The WebView frontend does not support this platform: "
            f"{platform.system() or 'unknown'}."
        )
    return backend


# Pulse --bg-0 tokens (webview_ui/styles.css). Passed as the window's own
# background so the pre-load flash matches the UI instead of blinding white.
_WINDOW_BACKGROUNDS = {"dark": "#101214", "light": "#f3f5f7"}


def resolve_effective_theme() -> str:
    """Resolve the persisted theme to ``dark``/``light`` (``system`` → OS)."""
    theme = "dark"
    try:
        from i18n.localization import get_localization_manager

        theme = get_localization_manager().get_theme()
    except Exception:
        pass
    if theme == "system":
        if platform.system() == "Windows":
            try:
                import winreg

                with winreg.OpenKey(
                    winreg.HKEY_CURRENT_USER,
                    r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                ) as key:
                    light, _ = winreg.QueryValueEx(key, "AppsUseLightTheme")
                return "light" if light else "dark"
            except Exception:
                return "dark"
        return "dark"
    return theme if theme in ("dark", "light") else "dark"


def apply_windows_titlebar_theme(window: Any, dark: bool) -> bool:
    """Match the OS title bar to the app theme via DWM, like CustomTkinter.

    Without this the WebView window wears the default white Windows title
    bar over the dark Pulse UI. Returns False while the native handle or
    frame is not ready (caller may retry); True otherwise (done or no-op).
    """
    if platform.system() != "Windows":
        return True
    try:
        import ctypes
        from ctypes import wintypes

        handle = getattr(getattr(window, "native", None), "Handle", None)
        if handle is None:
            return False
        hwnd = int(handle.ToInt64()) if hasattr(handle, "ToInt64") else int(str(handle))
        set_attribute = ctypes.windll.dwmapi.DwmSetWindowAttribute
        set_attribute.argtypes = [wintypes.HWND, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD]
        set_attribute.restype = ctypes.c_long
        value = ctypes.c_int(1 if dark else 0)
        # 20 = DWMWA_USE_IMMERSIVE_DARK_MODE (19 on pre-20H1 Windows 10).
        for attribute in (20, 19):
            if set_attribute(hwnd, attribute, ctypes.byref(value), ctypes.sizeof(value)) == 0:
                break
        set_window_pos = ctypes.windll.user32.SetWindowPos
        set_window_pos.argtypes = [
            wintypes.HWND,
            wintypes.HWND,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
            wintypes.UINT,
        ]
        set_window_pos.restype = wintypes.BOOL
        # SWP_NOSIZE|NOMOVE|NOZORDER|FRAMECHANGED recalculates the frame.
        frame_changed = set_window_pos(hwnd, 0, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0004 | 0x0020)

        redraw_window = ctypes.windll.user32.RedrawWindow
        redraw_window.argtypes = [
            wintypes.HWND,
            ctypes.c_void_p,
            ctypes.c_void_p,
            wintypes.UINT,
        ]
        redraw_window.restype = wintypes.BOOL
        # RDW_INVALIDATE|UPDATENOW|FRAME explicitly paints the non-client
        # area; otherwise Windows may wait for an activation change.
        frame_redrawn = redraw_window(hwnd, None, None, 0x0001 | 0x0100 | 0x0400)
        return bool(frame_changed and frame_redrawn)
    except Exception:
        return False


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
        payload = self._service.bootstrap()
        if payload.get("ok"):
            payload["data"]["webview_backend"] = _GUI_BACKENDS.get(platform.system())
        return payload

    def list_audio_devices(self, host_api: str | None = None) -> dict[str, Any]:
        return self._service.list_audio_devices(host_api)

    def start_recording(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._service.start_recording(request)

    def start_brir(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._service.start_brir(request)

    def start_output_recovery(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._service.start_output_recovery(request)

    def poll_job(self, job_id: str, after_seq: int = 0) -> dict[str, Any]:
        return self._service.poll_job(job_id, after_seq)

    def cancel_job(self, job_id: str) -> dict[str, Any]:
        return self._service.cancel_job(job_id)

    def get_ui_settings(self) -> dict[str, Any]:
        return self._service.get_ui_settings()

    def set_language(self, language_code: str) -> dict[str, Any]:
        return self._service.set_language(language_code)

    def set_theme(self, theme: str) -> dict[str, Any]:
        response = self._service.set_theme(theme)
        if response.get("ok"):
            self.apply_titlebar_theme()
        return response

    def apply_titlebar_theme(self) -> None:
        """Sync the native title bar with the current app theme (Windows).

        ``before_show`` normally runs with the native handle ready. Keep the
        short retry for runtime theme changes and unusually slow handle setup.
        """
        window = self._window
        if window is None:
            return
        dark = resolve_effective_theme() == "dark"
        for _ in range(25):
            if apply_windows_titlebar_theme(window, dark):
                return
            time.sleep(0.2)

    def set_skin(self, skin: str) -> dict[str, Any]:
        return self._service.set_skin(skin)

    def set_frontend(self, frontend: str) -> dict[str, Any]:
        return self._service.set_frontend(frontend)

    def get_system_info(self) -> dict[str, Any]:
        return self._service.get_system_info()

    def resolve_recording_paths(
        self,
        record_dir: Any,
        play_path: Any = None,
        mode: str = "speakers",
        sweep: Any = None,
    ) -> dict[str, Any]:
        return self._service.resolve_recording_paths(record_dir, play_path, mode, sweep)

    def generate_sweep_set(self, dir_path: Any) -> dict[str, Any]:
        return self._service.generate_sweep_set(dir_path)

    def detect_sweep(self, dir_path: Any) -> dict[str, Any]:
        return self._service.detect_sweep(dir_path)

    def open_path(self, path: Any = None) -> dict[str, Any]:
        return self._service.open_path(path)

    def check_for_updates(self) -> dict[str, Any]:
        return self._service.check_for_updates()

    def start_update(self, request: dict[str, Any]) -> dict[str, Any]:
        return self._service.start_update(request)

    def apply_pending_update(self) -> dict[str, Any]:
        response = self._service.apply_pending_update()
        restarting = bool(response.get("ok") and response["data"].get("restarting"))
        if restarting and self._window is not None:
            # Update.exe has taken over; give the JS caller a moment to
            # receive this response, then close the window so main() returns
            # and the old process exits for the restart.
            import threading

            threading.Timer(0.8, self._window.destroy).start()
        return response

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


def create_app_window(webview_module: Any, bridge: "WebviewBridge") -> Any:
    """Create the main application window and wire the bridge to it."""
    window = webview_module.create_window(
        "Impulcifer",
        _index_uri(),
        js_api=bridge,
        width=1280,
        height=860,
        min_size=(980, 640),
        background_color=_WINDOW_BACKGROUNDS[resolve_effective_theme()],
    )
    bridge.attach_window(window)
    # pywebview applies the Windows system theme inside BrowserForm.__init__.
    # Run after that assignment but before BrowserForm.Show() so the app theme
    # is the final value and the first visible frame already has the right
    # caption. A webview.start callback begins before BrowserForm is created
    # and can therefore be overwritten by pywebview during construction.
    window.events.before_show += bridge.apply_titlebar_theme
    return window


def main() -> None:
    """Start the platform WebView frontend (WebView2 / WKWebView / WebKit2GTK)."""
    backend = select_gui_backend()

    try:
        import webview
    except ImportError as exc:
        raise SystemExit(
            "pywebview is not installed. Install the WebView extra with "
            "'pip install .[webview]'."
        ) from exc

    bridge = WebviewBridge()
    create_app_window(webview, bridge)
    webview.start(gui=backend, debug=False)


if __name__ == "__main__":
    main()
