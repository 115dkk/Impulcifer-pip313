#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Shared utilities for the modern GUI.

Contains module-level helpers (font setup, environment detection, Tk var
accessors, file dialog wrappers) that were previously colocated in
``gui/modern_gui.py``.
"""

from __future__ import annotations

import os
import math
import platform
import re
import sys
from ctypes.util import find_library
from pathlib import Path
from tkinter import filedialog, TclError
from tkinter import font as tkfont
from typing import Any, Optional

import customtkinter as ctk

from gui.constants import FILETYPES_WAV_SAVE
from gui.theme import get_ico_path, get_png_path


def setup_app_icon(root: ctk.CTk) -> bool:
    """Apply the bundled pulse logo to the GUI window and Windows taskbar.

    Why this exists. The previous build shipped without an explicit window
    icon, so Tk fell back to the Tcl/Tk feather icon — what the user
    described as "이상한 로고" pinned to the title bar and taskbar. The
    redesign hands off ``logo/pulse.ico`` (multi-resolution 16/24/32/48/64/
    128/256) and ``logo/pulse-*.png``; this helper wires both paths so the
    Windows title bar, the Windows taskbar (including pinned shortcuts),
    and the X11 / macOS dock icons all render the Pulse mark.

    On Windows we also call ``SetCurrentProcessExplicitAppUserModelID`` so
    the taskbar groups by our AppUserModelID instead of ``python.exe``
    (matters for dev runs and unsigned standalone builds; signed Velopack
    installs already provide one via the shortcut).

    Returns True when the icon was applied (any path), False if nothing
    bundled could be found.
    """
    applied = False

    ico_path = get_ico_path()
    if ico_path is not None:
        try:
            root.iconbitmap(default=str(ico_path))
            applied = True
        except Exception as e:
            print(f"iconbitmap failed: {e}")

    try:
        from tkinter import PhotoImage as _PhotoImage

        photos = []
        for size in (256, 128, 64, 48, 32):
            png_path = get_png_path(size)
            if png_path is None:
                continue
            try:
                photos.append(_PhotoImage(file=str(png_path)))
            except Exception:
                continue
        if photos:
            root.iconphoto(True, *photos)
            applied = True
            root._pulse_icon_photos = photos
    except Exception as e:
        print(f"iconphoto failed: {e}")

    if platform.system() == "Windows":
        try:
            import ctypes

            ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(
                "Impulcifer.115dkk.HRIR.1"
            )
        except Exception:
            pass

    return applied


def open_data_folder() -> None:
    """Open the application's data folder in file explorer."""
    import subprocess
    from infra.resource_helper import DATA_DIR

    data_dir = Path(DATA_DIR)

    if not data_dir.exists():
        # Fallback: executable 디렉토리 기준
        if is_frozen_or_standalone():
            data_dir = Path(sys.executable).parent / "data"
        else:
            data_dir = Path(__file__).parent.parent / "data"

    if not data_dir.exists():
        data_dir = Path.home() / "Documents"  # 최종 폴백

    system = platform.system()

    try:
        if system == 'Windows':
            subprocess.Popen(['explorer', str(data_dir)])
        elif system == 'Darwin':
            subprocess.Popen(['open', str(data_dir)])
        else:  # Linux
            subprocess.Popen(['xdg-open', str(data_dir)])
    except Exception as e:
        print(f"Failed to open data folder: {e}")


def is_frozen_or_standalone() -> bool:
    """
    Check if the application is running as a Nuitka-compiled standalone executable.

    Returns:
        True if running as Nuitka standalone
        False if running as a normal Python script or pip-installed package
    """
    # 빌드 마커 우선 (가장 확실)
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "standalone"
    except ImportError:
        pass

    # 폴백: 기존 런타임 감지
    if getattr(sys, 'frozen', False):
        return True

    if '__nuitka__' in sys.modules:
        return True

    return False


def is_pip_available() -> bool:
    """
    Check if pip is available in the current environment.

    Returns:
        True if pip can be used for package management
    """
    # Method 1: Try importing pip directly (most reliable)
    try:
        import pip  # noqa: F401
        return True
    except ImportError:
        pass

    # Method 2: Try importing pip._internal
    try:
        import pip._internal  # noqa: F401
        return True
    except ImportError:
        pass

    # Method 3: Try subprocess check (fallback)
    try:
        import subprocess
        result = subprocess.run(
            [sys.executable, '-m', 'pip', '--version'],
            capture_output=True,
            timeout=10,
            encoding='utf-8',
            errors='replace'
        )
        if result.returncode == 0:
            return True
    except Exception as e:
        print(f"Subprocess pip check failed: {e}")

    # Method 4: Check if pip module exists in sys.modules or can be found
    try:
        import importlib.util
        spec = importlib.util.find_spec('pip')
        if spec is not None:
            return True
    except Exception:
        pass

    return False


def safe_get_double(var: Any, default: float = 0.0) -> float:
    """Safely read a DoubleVar.

    Args:
        var: Tk variable with a ``get`` method.
        default: Value returned when the variable is empty or invalid.

    Returns:
        The variable value or ``default``.
    """
    try:
        return var.get()
    except (TclError, ValueError):
        return default


def safe_get_int(var: Any, default: int = 0) -> int:
    """Safely read an IntVar.

    Args:
        var: Tk variable with a ``get`` method.
        default: Value returned when the variable is empty or invalid.

    Returns:
        The variable value or ``default``.
    """
    try:
        return var.get()
    except (TclError, ValueError):
        return default


def safe_get_string(var: Any, default: str = "") -> str:
    """Safely read a StringVar.

    Args:
        var: Tk variable with a ``get`` method.
        default: Value returned when the variable is empty or invalid.

    Returns:
        The variable value or ``default``.
    """
    try:
        return var.get()
    except (TclError, ValueError):
        return default


# Cache for setup_pretendard_font() — keyed by language code, holds resolved font family
# (or None when no Pretendard is available). Avoids repeated GDI calls on Windows and
# repeated tkfont.families() scans across dialog construction.
_font_cache: dict[str, Optional[str]] = {}


def _resolve_bundled_font_dir() -> Optional[Path]:
    """Return the bundled ``font/`` directory across runtime modes."""
    script_dir = Path(__file__).parent
    candidates: list[Path] = []
    try:
        from infra.resource_helper import get_resource_path

        candidates.append(Path(get_resource_path("font")))
    except Exception:
        pass
    candidates.extend(
        [
            script_dir / "font",
            script_dir / "fonts",
            script_dir.parent / "font",
            script_dir.parent / "fonts",
        ]
    )
    for c in candidates:
        if c.is_dir():
            return c
    return None


def _scan_bundled_fonts() -> list[Path]:
    """Enumerate every ``.otf`` / ``.ttf`` / ``.ttc`` in the bundled font dir."""
    font_dir = _resolve_bundled_font_dir()
    if font_dir is None:
        return []
    suffixes = {".otf", ".ttf", ".ttc"}
    return sorted(
        (p for p in font_dir.iterdir() if p.suffix.lower() in suffixes),
        key=lambda p: p.name.casefold(),
    )


def _find_pretendard_font_file() -> Optional[Path]:
    """Return the bundled Pretendard font path when it is available.

    Priority order:
      1. ``PretendardVariable*.ttf`` — gives Tk/GDI access to every fvar
         weight (Thin~Black) from a single file, so ``weight="bold"`` resolves
         to the real wght=700 instance instead of GDI synthetic-bold (which
         garbles Hangul glyphs and can fall through to system serifs under
         MacType-style font hooks).
      2. ``Pretendard*Regular*`` static cut.
      3. Any ``Pretendard*`` file as last resort.
    """
    fonts = _scan_bundled_fonts()
    for path in fonts:
        if "pretendard" in path.stem.lower() and "variable" in path.stem.lower():
            return path
    for path in fonts:
        stem = path.stem.lower()
        if "pretendard" in stem and "regular" in stem:
            return path
    for path in fonts:
        if "pretendard" in path.stem.lower():
            return path
    return None


def _font_family_from_file(font_path: Path) -> str:
    """Read the real family name from the font name table when possible."""
    try:
        from fontTools.ttLib import TTFont

        with TTFont(str(font_path), lazy=True) as font:
            names = font["name"].names
            for name_id in (1, 16):
                for record in names:
                    if record.nameID == name_id:
                        family = record.toUnicode().strip()
                        if family:
                            return family
    except Exception:
        pass
    return "Pretendard"


def _get_tk_font_families() -> Optional[set[str]]:
    """Return Tk-visible font families, or None when Tk is not initialized."""
    try:
        return {str(name) for name in tkfont.families()}
    except (RuntimeError, TclError):
        return None


def _match_tk_family(families: Optional[set[str]], desired: str) -> Optional[str]:
    """Find the exact Tk-visible spelling for a desired family name."""
    if not families:
        return None
    desired_folded = desired.casefold()
    for family in families:
        if family.casefold() == desired_folded:
            return family
    return None


def _tk_renders_family(desired: str) -> Optional[str]:
    """Return the family Tk's RENDER layer resolves ``desired`` to, or None.

    This is the authoritative check, not :func:`tkfont.families`. After
    ``AddFontResourceExW(FR_PRIVATE)`` registers a font for the current
    process, Windows' GDI sees it immediately (Tk uses GDI for rendering),
    but ``tkfont.families()`` caches its first-call output and may not list
    the new font. Creating a ``tkfont.Font`` with ``family=desired`` and
    inspecting its ``actual('family')`` exercises the render path: when Tk
    can render with the requested family, ``actual`` returns that family
    verbatim; otherwise it returns whatever Tk fell back to (e.g. the
    system default like Malgun Gothic), which we treat as a miss.

    This was the bug behind issue #87 follow-up: ``setup_pretendard_font``
    bailed to ``None`` because ``families()`` lacked Pretendard even though
    GDI / Tk render layer would have used it. CTk widgets then defaulted
    to the system font (Malgun Gothic / 명조 fallback on Korean Windows).
    """
    try:
        probe = tkfont.Font(family=desired, size=10)
        actual = probe.actual("family")
        if actual and actual.casefold() == desired.casefold():
            return actual
    except (RuntimeError, TclError):
        pass
    return None


def _register_font_file_for_tk(font_path: Path) -> bool:
    """Register a font for the current GUI process when the platform supports it."""
    system = platform.system()

    try:
        if system == "Windows":
            import ctypes

            gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)
            result = gdi32.AddFontResourceExW(str(font_path), 0x10, 0)  # FR_PRIVATE
            if result > 0:
                try:
                    user32 = ctypes.WinDLL("user32", use_last_error=True)
                    user32.SendMessageTimeoutW(0xFFFF, 0x001D, 0, 0, 0x0002, 1000, None)
                except Exception:
                    pass
                return True
            return False

        if system == "Darwin":
            import ctypes

            core_text_path = find_library("CoreText")
            core_foundation_path = find_library("CoreFoundation")
            if not core_text_path or not core_foundation_path:
                return False

            core_text = ctypes.CDLL(core_text_path)
            core_foundation = ctypes.CDLL(core_foundation_path)
            path_bytes = os.fsencode(str(font_path))

            core_foundation.CFURLCreateFromFileSystemRepresentation.restype = ctypes.c_void_p
            core_foundation.CFURLCreateFromFileSystemRepresentation.argtypes = [
                ctypes.c_void_p,
                ctypes.c_char_p,
                ctypes.c_long,
                ctypes.c_bool,
            ]
            url = core_foundation.CFURLCreateFromFileSystemRepresentation(
                None, path_bytes, len(path_bytes), False
            )
            if not url:
                return False

            try:
                core_text.CTFontManagerRegisterFontsForURL.restype = ctypes.c_bool
                core_text.CTFontManagerRegisterFontsForURL.argtypes = [
                    ctypes.c_void_p,
                    ctypes.c_uint,
                    ctypes.c_void_p,
                ]
                return bool(core_text.CTFontManagerRegisterFontsForURL(url, 1, None))
            finally:
                core_foundation.CFRelease.argtypes = [ctypes.c_void_p]
                core_foundation.CFRelease(url)

        if system == "Linux":
            import ctypes

            fontconfig_path = find_library("fontconfig")
            if not fontconfig_path:
                return False
            fontconfig = ctypes.CDLL(fontconfig_path)
            fontconfig.FcConfigAppFontAddFile.restype = ctypes.c_int
            fontconfig.FcConfigAppFontAddFile.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
            result = fontconfig.FcConfigAppFontAddFile(None, os.fsencode(str(font_path)))
            if result:
                fontconfig.FcConfigBuildFonts(None)
            return bool(result)
    except Exception as e:
        print(f"Failed to register Pretendard font: {e}")

    return False


_bundled_fonts_registered_for_tk = False


def register_all_bundled_fonts_for_tk() -> list[Path]:
    """Register every ``font/*.otf|*.ttf|*.ttc`` for the current Tk process.

    Idempotent — subsequent calls are no-ops. Returns the list of paths that
    were successfully registered the first time. So a Source Han Serif (or
    any other Korean font) the user drops into ``font/`` becomes addressable
    by family name from CTkFont / Tk widgets without any further code
    change.
    """
    global _bundled_fonts_registered_for_tk
    if _bundled_fonts_registered_for_tk:
        return []
    _bundled_fonts_registered_for_tk = True

    registered: list[Path] = []
    for path in _scan_bundled_fonts():
        try:
            if _register_font_file_for_tk(path):
                registered.append(path)
        except Exception:
            continue
    return registered


def setup_pretendard_font(current_language: str = 'en') -> Optional[str]:
    """
    Setup Pretendard font for Korean and English languages.
    Returns font family name to use, or None for system default.

    Side effect: every other font the user has dropped into ``font/`` (e.g.
    a Source Han Serif "본명조" file) is also registered for Tk via
    :func:`register_all_bundled_fonts_for_tk`, so the GUI can switch to
    those families on demand without code changes.

    Args:
        current_language: Current language code (e.g., 'ko', 'en')

    Returns:
        Font family name to use, or None for system default
    """
    if current_language in _font_cache:
        return _font_cache[current_language]

    def _cache_and_return(value: Optional[str]) -> Optional[str]:
        _font_cache[current_language] = value
        return value

    # Only use Pretendard for Korean and English
    if current_language not in ['ko', 'en']:
        # Even when we don't pick Pretendard for the language, still register
        # any bundled font so other code paths (e.g. matplotlib, dialogs that
        # opt into a different family) can find them.
        register_all_bundled_fonts_for_tk()
        return _cache_and_return(None)

    try:
        # Register every bundled font once — Pretendard is the primary, but
        # any companion files (Source Han Serif, etc.) become addressable
        # too. This is the place that satisfies "잡도록 수정" for fonts
        # placed alongside Pretendard.
        register_all_bundled_fonts_for_tk()

        # PRIMARY check: ask Tk's render layer directly.
        # tkfont.families() caches at startup and AddFontResourceExW does NOT
        # always invalidate that cache, but Tk renders via GDI which DOES see
        # process-private fonts immediately. So we trust actual() resolution
        # over the families() snapshot.
        #
        # We try "Pretendard Variable" FIRST: the bundled file's family-name
        # (name table id 1) is "Pretendard Variable", not "Pretendard".
        # Hitting this directly avoids one render-probe miss + lets Win32 GDI
        # auto-map weight="bold" to the fvar wght=700 instance instead of
        # falling through to synthetic bold.
        for candidate in ("Pretendard Variable", "Pretendard"):
            rendered = _tk_renders_family(candidate)
            if rendered:
                print(f"Tk render layer resolves {candidate}: {rendered}")
                return _cache_and_return(rendered)

        # If the bundled file wasn't registered yet (e.g. the helper above
        # short-circuited), force-register it now to fix Tk's render path.
        font_path = _find_pretendard_font_file()
        if font_path:
            family_name = _font_family_from_file(font_path)
            registered = _register_font_file_for_tk(font_path)
            rendered = _tk_renders_family(family_name)
            if rendered:
                print(f"Registered + render-verified {family_name}: {font_path}")
                return _cache_and_return(rendered)

            # Last-chance fall-back: if registration succeeded and Tk still
            # can't render the named family (extremely rare — usually means
            # the font file's family-name metadata diverged from "Pretendard"),
            # fall back to families() inspection so we at least return a
            # related family rather than None.
            available_fonts = _get_tk_font_families()
            for font_name in (available_fonts or set()):
                if "Pretendard" in font_name:
                    rendered = _tk_renders_family(font_name)
                    if rendered:
                        print(f"Using nearby Pretendard variant: {font_name}")
                        return _cache_and_return(rendered)

            print(
                f"Pretendard font file was found but Tk cannot render it: {font_path} "
                f"(registered={registered}, family_name={family_name!r})"
            )

        print("Pretendard font not available, using system default")
        return _cache_and_return(None)

    except Exception as e:
        print(f"Error setting up Pretendard font: {e}")
        return _cache_and_return(None)


def build_fonts(family: Optional[str]) -> dict[str, ctk.CTkFont]:
    """Build shared CTkFont instances keyed by semantic role.

    Must be called after a Tk default root exists (i.e. ``ctk.CTk()`` has
    been instantiated) because ``CTkFont`` needs one.
    """
    return {
        'heading':       ctk.CTkFont(family=family, size=16, weight="bold"),
        'title':         ctk.CTkFont(family=family, size=24, weight="bold"),
        'subtitle':      ctk.CTkFont(family=family, size=12),
        'label':         ctk.CTkFont(family=family, size=13),
        'value':         ctk.CTkFont(family=family, size=13, weight="bold"),
        'button_large':  ctk.CTkFont(family=family, size=16, weight="bold"),
        'small':         ctk.CTkFont(family=family, size=12),
        'small_bold':    ctk.CTkFont(family=family, size=12, weight="bold"),
        'dialog_title':  ctk.CTkFont(family=family, size=18, weight="bold"),
        'dialog_body':   ctk.CTkFont(family=family, size=14),
        'dialog_small':  ctk.CTkFont(family=family, size=12),
    }


def snapshot_tk_vars(owner: Any) -> dict[str, Any]:
    """Snapshot Tk variable attributes from an object.

    Args:
        owner: Object whose ``*_var`` attributes should be captured.

    Returns:
        A dictionary of attribute names to plain Python values. Nested
        dictionaries of Tk variables are preserved under the same attribute
        name.
    """
    state: dict[str, Any] = {}
    for name, value in vars(owner).items():
        if name.endswith("_var") and hasattr(value, "get"):
            try:
                state[name] = value.get()
            except (TclError, ValueError):
                continue
        elif name.endswith("_vars") and isinstance(value, dict):
            nested: dict[str, Any] = {}
            for key, var in value.items():
                if hasattr(var, "get"):
                    try:
                        nested[key] = var.get()
                    except (TclError, ValueError):
                        continue
            if nested:
                state[name] = nested
    return state


def restore_tk_vars(owner: Any, state: dict[str, Any]) -> None:
    """Restore Tk variable attributes captured by :func:`snapshot_tk_vars`.

    Args:
        owner: Object whose Tk variables should be restored.
        state: Snapshot returned by :func:`snapshot_tk_vars`.
    """
    for name, saved_value in state.items():
        current = getattr(owner, name, None)
        if hasattr(current, "set"):
            try:
                current.set(saved_value)
            except (TclError, ValueError):
                continue
        elif isinstance(current, dict) and isinstance(saved_value, dict):
            for key, nested_value in saved_value.items():
                var = current.get(key)
                if hasattr(var, "set"):
                    try:
                        var.set(nested_value)
                    except (TclError, ValueError):
                        continue


def browse_file(var: Any, mode: str, filetypes: Optional[list[tuple[str, str]]] = None) -> None:
    """Open a file chooser and store the selected path in a Tk variable."""
    if filetypes is None:
        filetypes = [('All files', '*.*')]

    if mode == 'open':
        filename = filedialog.askopenfilename(
            initialdir=os.path.dirname(var.get()) if var.get() else os.getcwd(),
            initialfile=os.path.basename(var.get()) if var.get() else "",
            filetypes=filetypes
        )
    else:  # save
        filename = filedialog.asksaveasfilename(
            initialdir=os.path.dirname(var.get()) if var.get() else os.getcwd(),
            initialfile=os.path.basename(var.get()) if var.get() else "",
            defaultextension=".wav",
            filetypes=FILETYPES_WAV_SAVE
        )

    if filename:
        # Convert to relative path if possible
        try:
            filename = os.path.relpath(filename, os.getcwd())
        except Exception:
            pass
        var.set(filename)


def browse_directory(var: Any) -> None:
    """Open a directory chooser and store the selected path in a Tk variable."""
    dirname = filedialog.askdirectory(
        initialdir=var.get() if var.get() else os.getcwd()
    )

    if dirname:
        # Convert to relative path if possible
        try:
            dirname = os.path.relpath(dirname, os.getcwd())
        except Exception:
            pass
        var.set(dirname)


_DEFAULT_SCROLL_REFRESH_RATE_HZ = 60.0
_MIN_SCROLL_REFRESH_RATE_HZ = 30.0
_MAX_SCROLL_REFRESH_RATE_HZ = 1000.0
_SCROLL_REFRESH_ENV_VAR = "IMPULCIFER_SCROLL_REFRESH_HZ"


def _refresh_rate_to_frame_interval_ms(refresh_rate_hz: float) -> int:
    """Convert a display refresh rate to a conservative frame interval."""
    if refresh_rate_hz < _MIN_SCROLL_REFRESH_RATE_HZ:
        refresh_rate_hz = _DEFAULT_SCROLL_REFRESH_RATE_HZ
    return max(1, int(math.ceil(1000.0 / refresh_rate_hz)))


def _valid_refresh_rate(value: Any) -> Optional[float]:
    try:
        refresh_rate = float(value)
    except (TypeError, ValueError):
        return None
    if _MIN_SCROLL_REFRESH_RATE_HZ <= refresh_rate <= _MAX_SCROLL_REFRESH_RATE_HZ:
        return refresh_rate
    return None


def _get_windows_refresh_rate_hz(canvas: Any | None = None) -> Optional[float]:
    """Return the nearest Windows monitor refresh rate when available."""
    try:
        import ctypes
    except Exception:
        return None

    try:
        user32 = ctypes.WinDLL("user32", use_last_error=True)
    except Exception:
        return None

    class PointL(ctypes.Structure):
        _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]

    class Rect(ctypes.Structure):
        _fields_ = [
            ("left", ctypes.c_long),
            ("top", ctypes.c_long),
            ("right", ctypes.c_long),
            ("bottom", ctypes.c_long),
        ]

    class MonitorInfoExW(ctypes.Structure):
        _fields_ = [
            ("cbSize", ctypes.c_ulong),
            ("rcMonitor", Rect),
            ("rcWork", Rect),
            ("dwFlags", ctypes.c_ulong),
            ("szDevice", ctypes.c_wchar * 32),
        ]

    class DevModeW(ctypes.Structure):
        _fields_ = [
            ("dmDeviceName", ctypes.c_wchar * 32),
            ("dmSpecVersion", ctypes.c_ushort),
            ("dmDriverVersion", ctypes.c_ushort),
            ("dmSize", ctypes.c_ushort),
            ("dmDriverExtra", ctypes.c_ushort),
            ("dmFields", ctypes.c_ulong),
            ("dmPosition", PointL),
            ("dmDisplayOrientation", ctypes.c_ulong),
            ("dmDisplayFixedOutput", ctypes.c_ulong),
            ("dmColor", ctypes.c_short),
            ("dmDuplex", ctypes.c_short),
            ("dmYResolution", ctypes.c_short),
            ("dmTTOption", ctypes.c_short),
            ("dmCollate", ctypes.c_short),
            ("dmFormName", ctypes.c_wchar * 32),
            ("dmLogPixels", ctypes.c_ushort),
            ("dmBitsPerPel", ctypes.c_ulong),
            ("dmPelsWidth", ctypes.c_ulong),
            ("dmPelsHeight", ctypes.c_ulong),
            ("dmDisplayFlags", ctypes.c_ulong),
            ("dmDisplayFrequency", ctypes.c_ulong),
            ("dmICMMethod", ctypes.c_ulong),
            ("dmICMIntent", ctypes.c_ulong),
            ("dmMediaType", ctypes.c_ulong),
            ("dmDitherType", ctypes.c_ulong),
            ("dmReserved1", ctypes.c_ulong),
            ("dmReserved2", ctypes.c_ulong),
            ("dmPanningWidth", ctypes.c_ulong),
            ("dmPanningHeight", ctypes.c_ulong),
        ]

    device_name = None
    if canvas is not None:
        try:
            hwnd = int(canvas.winfo_toplevel().winfo_id())
            monitor = user32.MonitorFromWindow(ctypes.c_void_p(hwnd), 2)
            if monitor:
                monitor_info = MonitorInfoExW()
                monitor_info.cbSize = ctypes.sizeof(MonitorInfoExW)
                if user32.GetMonitorInfoW(monitor, ctypes.byref(monitor_info)):
                    device_name = monitor_info.szDevice
        except Exception:
            device_name = None

    dev_mode = DevModeW()
    dev_mode.dmSize = ctypes.sizeof(DevModeW)
    try:
        ok = user32.EnumDisplaySettingsW(device_name, -1, ctypes.byref(dev_mode))
    except Exception:
        return None
    if not ok:
        return None
    return _valid_refresh_rate(dev_mode.dmDisplayFrequency)


def _get_macos_refresh_rate_hz() -> Optional[float]:
    """Return the macOS main display refresh rate when CoreGraphics reports it."""
    try:
        import ctypes
    except Exception:
        return None

    core_graphics_path = find_library("CoreGraphics")
    if not core_graphics_path:
        return None

    try:
        core_graphics = ctypes.CDLL(core_graphics_path)
        core_graphics.CGMainDisplayID.restype = ctypes.c_uint32
        display_id = core_graphics.CGMainDisplayID()
        core_graphics.CGDisplayCopyDisplayMode.restype = ctypes.c_void_p
        core_graphics.CGDisplayCopyDisplayMode.argtypes = [ctypes.c_uint32]
        mode = core_graphics.CGDisplayCopyDisplayMode(display_id)
        if not mode:
            return None
        try:
            core_graphics.CGDisplayModeGetRefreshRate.restype = ctypes.c_double
            core_graphics.CGDisplayModeGetRefreshRate.argtypes = [ctypes.c_void_p]
            return _valid_refresh_rate(core_graphics.CGDisplayModeGetRefreshRate(mode))
        finally:
            core_graphics.CGDisplayModeRelease.argtypes = [ctypes.c_void_p]
            core_graphics.CGDisplayModeRelease(mode)
    except Exception:
        return None


def _get_xrandr_refresh_rate_hz() -> Optional[float]:
    """Return the active X11 refresh rate from xrandr output when available."""
    try:
        import subprocess

        result = subprocess.run(
            ["xrandr", "--current"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=1,
            check=False,
        )
    except Exception:
        return None
    if result.returncode != 0:
        return None

    for line in result.stdout.splitlines():
        if "*" not in line:
            continue
        match = re.search(r"(\d+(?:\.\d+)?)\*", line)
        refresh_rate = _valid_refresh_rate(match.group(1) if match else None)
        if refresh_rate is not None:
            return refresh_rate
    return None


def _get_display_refresh_rate_hz(canvas: Any | None = None) -> float:
    """Best-effort monitor refresh-rate detection with a safe 60 Hz fallback."""
    refresh_rate = _valid_refresh_rate(os.environ.get(_SCROLL_REFRESH_ENV_VAR))
    if refresh_rate is not None:
        return refresh_rate

    system = platform.system()
    if system == "Windows":
        refresh_rate = _get_windows_refresh_rate_hz(canvas)
    elif system == "Darwin":
        refresh_rate = _get_macos_refresh_rate_hz()
    else:
        refresh_rate = _get_xrandr_refresh_rate_hz()

    return refresh_rate or _DEFAULT_SCROLL_REFRESH_RATE_HZ


def _get_scroll_frame_interval_ms(canvas: Any | None = None) -> int:
    """Return the scroll frame interval for the display containing the canvas."""
    return _refresh_rate_to_frame_interval_ms(_get_display_refresh_rate_hz(canvas))


def _install_frame_limited_canvas_scroll(
    canvas: Any,
    frame_interval_ms: int | None = None,
) -> None:
    """Coalesce wheel-driven canvas scroll calls to one update per frame."""
    if getattr(canvas, "_impulcifer_frame_limited_scroll", False):
        return
    if frame_interval_ms is None:
        frame_interval_ms = _get_scroll_frame_interval_ms(canvas)

    original_xview = canvas.xview
    original_yview = canvas.yview
    pending_units = {"x": 0, "y": 0}
    after_ids: dict[str, Any] = {"x": None, "y": None}

    def _flush(axis: str, original_view: Any) -> None:
        after_ids[axis] = None
        amount = pending_units[axis]
        pending_units[axis] = 0
        if amount == 0:
            return
        try:
            original_view("scroll", amount, "units")
        except TclError:
            pass

    def _schedule(axis: str, original_view: Any) -> None:
        if after_ids[axis] is not None:
            return
        try:
            after_ids[axis] = canvas.after(
                frame_interval_ms,
                lambda: _flush(axis, original_view),
            )
        except TclError:
            _flush(axis, original_view)

    def _cancel_pending(axis: str) -> None:
        after_id = after_ids[axis]
        if after_id is not None:
            try:
                canvas.after_cancel(after_id)
            except (TclError, ValueError):
                pass
        after_ids[axis] = None
        pending_units[axis] = 0

    def _make_frame_limited_view(axis: str, original_view: Any):
        def _frame_limited_view(*args):
            if len(args) == 3 and args[0] == "scroll" and args[2] == "units":
                try:
                    amount = int(args[1])
                except (TypeError, ValueError):
                    return original_view(*args)
                if amount != 0:
                    pending_units[axis] += amount
                    _schedule(axis, original_view)
                return None

            if args:
                _cancel_pending(axis)
            return original_view(*args)

        return _frame_limited_view

    canvas.xview = _make_frame_limited_view("x", original_xview)
    canvas.yview = _make_frame_limited_view("y", original_yview)
    canvas._impulcifer_frame_limited_scroll = True


def install_smooth_scrolling(scroll_frame: ctk.CTkScrollableFrame) -> None:
    """Patch a ``CTkScrollableFrame`` to skip redundant scrollregion updates.

    **Why this exists.** ``CTkScrollableFrame.__init__`` binds the inner Frame's
    ``<Configure>`` event to a lambda that calls ``canvas.bbox('all')`` and
    ``canvas.configure(scrollregion=...)`` on every event. On Win32 Tk fires
    ``<Configure>`` for *position* changes too — so every scroll step (each
    ``yview_moveto`` call) triggers a fresh ``bbox('all')`` walk over the
    canvas item tree plus a scrollregion reconfigure. With ~100 visible CTk
    widgets each containing their own ``tk.Canvas``, this is O(N) work per
    scroll step. Combined with DWM compositor traffic from moving all those
    child windows, it pushes the GPU to ~30% during scroll.

    **What this does.** Replaces the inner Frame's ``<Configure>`` binding
    with a size-change-only variant: it remembers the last seen ``(width,
    height)`` and only recomputes scrollregion when either dimension actually
    changes (children added / removed / resized — e.g. on language change or
    advanced-options toggle). Position-only Configure events from scrolling
    are no-ops, so the scrollregion bookkeeping stays in sync with the
    canvas content but the per-step cost goes from O(N) to O(1). Wheel-driven
    ``xview/yview("scroll", ..., "units")`` calls are also coalesced to the
    active monitor's refresh interval, which keeps scroll paint cadence above
    the 50 Hz target without hard-coding a 60 Hz display.

    The fix is reversible: the size-change branch still calls the same
    ``bbox + configure(scrollregion)`` so layout-driven sizing still works
    exactly like the upstream CustomTkinter behavior.
    """
    canvas = scroll_frame._parent_canvas  # type: ignore[attr-defined]
    _install_frame_limited_canvas_scroll(canvas)

    # Drop the lambda installed by CTkScrollableFrame.__init__. ``unbind``
    # without a binding ID removes ALL bindings for the sequence on this
    # widget — that's what we want here, since the only `<Configure>` binding
    # CTkScrollableFrame installs is the unconditional one.
    scroll_frame.unbind("<Configure>")

    # Use a list so the closure can mutate without `nonlocal`.
    last_size: list[Optional[int]] = [None, None]

    def _on_configure(event):
        """Recompute scrollregion only on actual size changes."""
        if event.width == last_size[0] and event.height == last_size[1]:
            return
        last_size[0] = event.width
        last_size[1] = event.height
        try:
            canvas.configure(scrollregion=canvas.bbox("all"))
        except TclError:
            # Widget being destroyed — bbox/configure raise during teardown.
            pass

    scroll_frame.bind("<Configure>", _on_configure, add="+")
