#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Shared utilities for the modern GUI.

Contains module-level helpers (font setup, environment detection, Tk var
accessors, file dialog wrappers) that were previously colocated in
``gui/modern_gui.py``.
"""

from __future__ import annotations

import os
import platform
import sys
from ctypes.util import find_library
from pathlib import Path
from tkinter import filedialog, TclError
from tkinter import font as tkfont
from typing import Any, Optional

import customtkinter as ctk

from gui.constants import FILETYPES_WAV_SAVE


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


def _find_pretendard_font_file() -> Optional[Path]:
    """Return the bundled Pretendard font path when it is available."""
    script_dir = Path(__file__).parent
    candidates: list[Path] = []

    try:
        from infra.resource_helper import get_font_path

        candidates.append(Path(get_font_path("Pretendard-Regular.otf")))
    except Exception:
        pass

    candidates.extend([
        script_dir / "font" / "Pretendard-Regular.otf",
        script_dir / "fonts" / "Pretendard-Regular.otf",
        script_dir.parent / "font" / "Pretendard-Regular.otf",
        script_dir.parent / "fonts" / "Pretendard-Regular.otf",
    ])

    for path in candidates:
        if path.exists():
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


def setup_pretendard_font(current_language: str = 'en') -> Optional[str]:
    """
    Setup Pretendard font for Korean and English languages.
    Returns font family name to use, or None for system default.

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
        return _cache_and_return(None)

    try:
        available_fonts = _get_tk_font_families()
        system_family = _match_tk_family(available_fonts, "Pretendard")
        if system_family:
            print(f"Using system-installed Pretendard font: {system_family}")
            return _cache_and_return(system_family)

        font_path = _find_pretendard_font_file()
        if font_path:
            family_name = _font_family_from_file(font_path)
            registered = _register_font_file_for_tk(font_path)
            available_fonts = _get_tk_font_families()
            tk_family = _match_tk_family(available_fonts, family_name)

            if tk_family:
                print(f"Successfully registered Pretendard font: {font_path}")
                return _cache_and_return(tk_family)

            if registered and available_fonts is None:
                print(f"Registered Pretendard font before Tk font validation: {font_path}")
                return family_name

            print(f"Pretendard font file was found but is not visible to Tk: {font_path}")

        available_fonts = _get_tk_font_families()
        if available_fonts:
            for font_name in available_fonts:
                if "Pretendard" in font_name:
                    print(f"Using system-installed Pretendard font: {font_name}")
                    return _cache_and_return(font_name)

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
        'small':         ctk.CTkFont(family=family, size=11),
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
    canvas content but the per-step cost goes from O(N) to O(1).

    The fix is reversible: the size-change branch still calls the same
    ``bbox + configure(scrollregion)`` so layout-driven sizing still works
    exactly like the upstream CustomTkinter behavior.
    """
    canvas = scroll_frame._parent_canvas  # type: ignore[attr-defined]

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
