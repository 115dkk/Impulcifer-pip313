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
        # Try to find Pretendard font file
        font_path = None
        script_dir = Path(__file__).parent

        # Check common font locations
        possible_paths = [
            script_dir / "font" / "Pretendard-Regular.otf",
            script_dir / "fonts" / "Pretendard-Regular.otf",
            script_dir.parent / "font" / "Pretendard-Regular.otf",
            script_dir.parent / "fonts" / "Pretendard-Regular.otf",
        ]

        for path in possible_paths:
            if path.exists():
                font_path = path
                break

        if font_path and font_path.exists():
            # Try to register font with system (Windows)
            try:
                if platform.system() == "Windows":
                    import ctypes

                    # Register font temporarily for this session
                    gdi32 = ctypes.WinDLL('gdi32', use_last_error=True)
                    FR_PRIVATE = 0x10

                    # Add font resource
                    result = gdi32.AddFontResourceExW(
                        str(font_path),
                        FR_PRIVATE,
                        0
                    )

                    if result > 0:
                        print(f"Successfully registered Pretendard font: {font_path}")
                        return _cache_and_return("Pretendard")
                    else:
                        print(f"Failed to register Pretendard font (result={result})")
                else:
                    # For Linux/Mac, just try using the font name
                    # The font should be installed system-wide
                    print(f"Found Pretendard font at: {font_path}")
                    print("Note: On Linux/Mac, please install Pretendard font system-wide for best results")
                    # Try to use Pretendard anyway
                    return _cache_and_return("Pretendard")

            except Exception as e:
                print(f"Failed to register Pretendard font: {e}")

        # Fallback: Check if Pretendard is already installed in system
        try:
            available_fonts = tkfont.families()
            for font_name in available_fonts:
                if "Pretendard" in font_name:
                    print(f"Using system-installed Pretendard font: {font_name}")
                    return _cache_and_return(font_name)
        except Exception as e:
            print(f"Error checking system fonts: {e}")

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
