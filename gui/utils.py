#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Shared utilities for the modern GUI.

Contains module-level helpers (font setup, environment detection, Tk var
accessors, file dialog wrappers) that were previously colocated in
``gui/modern_gui.py``.
"""

import os
import platform
import sys
from pathlib import Path
from tkinter import filedialog, TclError
from tkinter import font as tkfont

import customtkinter as ctk

from gui.constants import FILETYPES_WAV_SAVE


def open_data_folder():
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


def safe_get_double(var, default=0.0):
    """Safely get value from DoubleVar, returning default if empty or invalid."""
    try:
        return var.get()
    except (TclError, ValueError):
        return default


def safe_get_int(var, default=0):
    """Safely get value from IntVar, returning default if empty or invalid."""
    try:
        return var.get()
    except (TclError, ValueError):
        return default


def safe_get_string(var, default=""):
    """Safely get value from StringVar, returning default if error."""
    try:
        return var.get()
    except (TclError, ValueError):
        return default


# Cache for setup_pretendard_font() — keyed by language code, holds resolved font family
# (or None when no Pretendard is available). Avoids repeated GDI calls on Windows and
# repeated tkfont.families() scans across dialog construction.
_font_cache: dict = {}


def setup_pretendard_font(current_language: str = 'en') -> str:
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

    def _cache_and_return(value):
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


def _build_fallback_fonts(font_family: str) -> dict:
    """Build a minimal fonts dict for dialogs instantiated without a parent fonts dict.

    Mirrors the keys created by build_fonts. Dialogs always receive a shared
    fonts dict in normal use; this only fires if a dialog is constructed
    standalone (e.g. from tests).
    """
    return {
        'heading':       ctk.CTkFont(family=font_family, size=16, weight="bold"),
        'title':         ctk.CTkFont(family=font_family, size=24, weight="bold"),
        'subtitle':      ctk.CTkFont(family=font_family, size=12),
        'label':         ctk.CTkFont(family=font_family, size=13),
        'value':         ctk.CTkFont(family=font_family, size=13, weight="bold"),
        'button_large':  ctk.CTkFont(family=font_family, size=16, weight="bold"),
        'small':         ctk.CTkFont(family=font_family, size=11),
        'small_bold':    ctk.CTkFont(family=font_family, size=12, weight="bold"),
        'dialog_title':  ctk.CTkFont(family=font_family, size=18, weight="bold"),
        'dialog_body':   ctk.CTkFont(family=font_family, size=14),
        'dialog_small':  ctk.CTkFont(family=font_family, size=12),
    }


def build_fonts(family: str) -> dict:
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


def browse_file(var, mode, filetypes=None):
    """Browse for file"""
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


def browse_directory(var):
    """Browse for directory"""
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
