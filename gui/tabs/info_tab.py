#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Info tab for the modern GUI.

Lays out the Pulse redesign's "about-hero" card (large logo + version pill
+ description) followed by Contributors, System Information (2-column KV
grid), and Project Links sections.
"""

from __future__ import annotations

import os
import platform
import sys
import webbrowser
from pathlib import Path
from typing import TYPE_CHECKING

import customtkinter as ctk

import impulcifer
from core.parallel_processing import get_python_threading_info
from gui.constants import WIDGET_BUTTON_WIDTH_MEDIUM, WIDGET_BUTTON_WIDTH_WIDE
from gui.theme import COLORS, get_png_path
from gui.utils import install_smooth_scrolling
from updater.updater_core import is_pip_environment, is_velopack_environment

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class InfoTab:
    """Build the read-only application information tab."""

    def __init__(self, app: ModernImpulciferGUI) -> None:
        """Create the info tab.

        Args:
            app: Top-level GUI application.
        """
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.tabview = app.tabview
        self.root = app.root
        self._build()

    def _build(self) -> None:
        """Render hero, contributor, system, and link sections."""
        tab = self.tabview.tab(self.loc.get('tab_info'))
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        section_row = 0
        label_font = self.fonts['label']
        value_font = self.fonts['value']
        heading_font = self.fonts['heading']
        small_font = self.fonts['small']

        # === About hero ===
        section_row = self._build_about_hero(scroll, section_row)

        # === Contributors ===
        contrib_frame = ctk.CTkFrame(scroll, corner_radius=8)
        contrib_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        contrib_frame.grid_columnconfigure(1, weight=1)
        section_row += 1

        ctk.CTkLabel(contrib_frame, text=self.loc.get('section_contributors'), font=heading_font
                     ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        contributors = [
            ("Jaakko Pasanen", self.loc.get('contributor_role_original')),
            ("115dkk", self.loc.get('contributor_role_py313')),
            ("LionLion123", self.loc.get('contributor_role_contributor')),
            ("SDC (DCinside)", self.loc.get('contributor_role_contributor')),
        ]
        for i, (name, role) in enumerate(contributors):
            ctk.CTkLabel(contrib_frame, text=name, font=value_font
                         ).grid(row=i + 1, column=0, sticky="w", padx=15, pady=3)
            ctk.CTkLabel(contrib_frame, text=role, font=label_font, text_color=COLORS['fg-2']
                         ).grid(row=i + 1, column=1, sticky="w", padx=5, pady=3)
        contrib_frame.grid_rowconfigure(len(contributors) + 1, minsize=10)

        # === System Information (KV grid, 2 columns) ===
        sys_frame = ctk.CTkFrame(scroll, corner_radius=8)
        sys_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        sys_frame.grid_columnconfigure((0, 1), weight=1, uniform="kv")
        section_row += 1

        ctk.CTkLabel(sys_frame, text=self.loc.get('section_system_info'), font=heading_font
                     ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        try:
            threading_info = get_python_threading_info()
        except Exception:
            threading_info = {}

        py_ver = threading_info.get('python_version', f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}")
        os_name = f"{platform.system()} {platform.release()}"
        cpu_cores = str(threading_info.get('cpu_count', os.cpu_count() or '?'))
        optimal_workers = str(threading_info.get('optimal_workers', '?'))

        gil_raw = threading_info.get('gil_enabled', 'unknown (pre-3.14)')
        if gil_raw is True:
            gil_text = self.loc.get('info_gil_enabled')
        elif gil_raw is False:
            gil_text = self.loc.get('info_gil_disabled')
        else:
            gil_text = self.loc.get('info_gil_unknown')

        sys_items = [
            (self.loc.get('label_python'), py_ver),
            (self.loc.get('label_os'), os_name),
            (self.loc.get('label_cpu_cores'), cpu_cores),
            (self.loc.get('label_gil_status'), gil_text),
            (self.loc.get('label_optimal_workers'), optimal_workers),
        ]
        for i, (label, value) in enumerate(sys_items):
            row = 1 + (i // 2)
            col = i % 2
            cell = ctk.CTkFrame(sys_frame, corner_radius=4, fg_color=COLORS['bg-2'])
            cell.grid(row=row, column=col, sticky="ew", padx=(15 if col == 0 else 6, 15 if col == 1 else 6), pady=4)
            cell.grid_columnconfigure(1, weight=1)
            ctk.CTkLabel(cell, text=label, font=small_font, text_color=COLORS['fg-2']
                         ).grid(row=0, column=0, sticky="w", padx=10, pady=6)
            ctk.CTkLabel(cell, text=value, font=value_font, anchor="e"
                         ).grid(row=0, column=1, sticky="e", padx=10, pady=6)
        sys_frame.grid_rowconfigure(1 + (len(sys_items) - 1) // 2 + 1, minsize=12)

        # === Project Links ===
        links_frame = ctk.CTkFrame(scroll, corner_radius=8)
        links_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        links_frame.grid_columnconfigure(0, weight=1)
        section_row += 1

        ctk.CTkLabel(links_frame, text=self.loc.get('section_project_links'), font=heading_font
                     ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        ctk.CTkButton(links_frame, text=self.loc.get('button_original_repo'),
                      command=lambda: webbrowser.open("https://github.com/jaakkopasanen/Impulcifer"),
                      width=WIDGET_BUTTON_WIDTH_WIDE
                      ).grid(row=1, column=0, sticky="w", padx=15, pady=(5, 5))

        ctk.CTkButton(links_frame, text=self.loc.get('button_fork_repo'),
                      command=lambda: webbrowser.open("https://github.com/115dkk/Impulcifer-pip313"),
                      width=WIDGET_BUTTON_WIDTH_WIDE
                      ).grid(row=2, column=0, sticky="w", padx=15, pady=(5, 15))

    # ------------------------------------------------------------------
    # About hero
    # ------------------------------------------------------------------
    def _build_about_hero(self, parent: ctk.CTkScrollableFrame, section_row: int) -> int:
        """Render the Pulse redesign about-hero card.

        Layout: 88px logo on the left, then a vertical stack with the
        product name, a ``VERSION 2.4.X · PYTHON 3.13.Y · MIT`` mono pill,
        the localized subtitle, and License + Bug-report buttons.
        """
        hero = ctk.CTkFrame(parent, corner_radius=10, fg_color=COLORS['bg-2'])
        hero.grid(row=section_row, column=0, sticky="ew", padx=10, pady=(10, 10))
        hero.grid_columnconfigure(1, weight=1)

        # Logo (88px) — graceful fallback if PIL or PNG missing.
        try:
            from PIL import Image

            png_path = get_png_path(128)
            if png_path is not None:
                self._hero_logo_pil = Image.open(str(png_path))
                self._hero_logo_image = ctk.CTkImage(
                    light_image=self._hero_logo_pil,
                    dark_image=self._hero_logo_pil,
                    size=(88, 88),
                )
                ctk.CTkLabel(hero, image=self._hero_logo_image, text=""
                             ).grid(row=0, column=0, rowspan=4, padx=(20, 16), pady=20, sticky="nw")
        except Exception as e:
            print(f"Hero logo not loaded: {e}")

        title_label = ctk.CTkLabel(
            hero,
            text="Impulcifer",
            font=ctk.CTkFont(family=self.app.font_family, size=24, weight="bold"),
            anchor="w",
        )
        title_label.grid(row=0, column=1, sticky="sw", padx=(0, 20), pady=(20, 0))

        if is_velopack_environment():
            install_text = self.loc.get('info_install_velopack')
        elif is_pip_environment():
            install_text = self.loc.get('info_install_pip')
        else:
            install_text = self.loc.get('info_install_dev')
        version_pill = (
            f"VERSION {impulcifer.__version__}  ·  PYTHON "
            f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}  "
            f"·  {install_text}"
        )
        ctk.CTkLabel(
            hero,
            text=version_pill,
            font=ctk.CTkFont(family="JetBrains Mono", size=11, weight="bold"),
            text_color=COLORS['accent'],
            anchor="w",
        ).grid(row=1, column=1, sticky="nw", padx=(0, 20), pady=(4, 0))

        ctk.CTkLabel(
            hero,
            text=self.loc.get('app_subtitle'),
            font=self.fonts['label'],
            text_color=COLORS['fg-1'],
            anchor="w",
            wraplength=560,
            justify="left",
        ).grid(row=2, column=1, sticky="nw", padx=(0, 20), pady=(8, 12))

        # Action row — License + Bug report
        action_frame = ctk.CTkFrame(hero, fg_color="transparent")
        action_frame.grid(row=3, column=1, sticky="w", padx=(0, 20), pady=(0, 20))

        ctk.CTkButton(
            action_frame,
            text=self.loc.get('button_view_license'),
            command=self._open_license,
            width=WIDGET_BUTTON_WIDTH_MEDIUM,
        ).grid(row=0, column=0, padx=(0, 8))

        ctk.CTkButton(
            action_frame,
            text=self.loc.get('button_report_bug'),
            command=lambda: webbrowser.open("https://github.com/115dkk/Impulcifer-pip313/issues/new"),
            width=WIDGET_BUTTON_WIDTH_MEDIUM,
        ).grid(row=0, column=1, padx=(0, 8))

        return section_row + 1

    @staticmethod
    def _open_license() -> None:
        """Open LICENSE in the system's default text viewer."""
        candidates = [
            Path(sys.executable).parent / 'License.txt',
            Path(__file__).parent.parent.parent / 'LICENSE',
            Path(__file__).parent.parent.parent / 'License.txt',
        ]
        license_path = None
        for p in candidates:
            if p.exists():
                license_path = p
                break

        if license_path:
            if platform.system() == 'Windows':
                os.startfile(license_path)
            elif platform.system() == 'Darwin':
                os.system(f'open "{license_path}"')
            else:
                os.system(f'xdg-open "{license_path}"')
        else:
            webbrowser.open("https://github.com/115dkk/Impulcifer-pip313/blob/main/LICENSE")
