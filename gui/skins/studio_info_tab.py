#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Studio Info tab — about-hero card + environment KV grid + links."""

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
from gui.skins.studio_widgets import add_card_header, make_card, make_card_body, make_page_header
from gui.theme import COLORS, get_mono_font_family, get_png_path
from gui.utils import install_smooth_scrolling
from updater.updater_core import is_pip_environment, is_velopack_environment

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioInfoTab:
    """Read-only Info tab in Studio's card layout."""

    def __init__(self, app: ModernImpulciferGUI, parent: ctk.CTkBaseClass) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.parent = parent
        self._build()

    def _build(self) -> None:
        self.parent.grid_columnconfigure(0, weight=1)
        self.parent.grid_rowconfigure(0, weight=1)

        # Opaque bg-1 backing — a transparent scrollable frame leaves the
        # inner tk.Canvas without a solid background, causing Win32 scroll
        # ghosting ("잔상") of the embedded widgets. Stable's tabs use the
        # theme's opaque default; mirror that here.
        scroll = ctk.CTkScrollableFrame(self.parent, fg_color=COLORS["bg-1"])
        scroll.grid(row=0, column=0, sticky="nsew", padx=24, pady=24)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        page_header = make_page_header(
            scroll,
            title=self.loc.get("studio_info_title"),
            subtitle=self.loc.get("studio_info_subtitle"),
            fonts=self.fonts,
        )
        page_header.grid(row=0, column=0, sticky="ew", pady=(0, 18))

        self._build_hero(scroll, row=1)
        self._build_environment_card(scroll, row=2)
        self._build_links_card(scroll, row=3)

    # ------------------------------------------------------------------
    # Hero
    # ------------------------------------------------------------------
    def _build_hero(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        hero = ctk.CTkFrame(
            parent,
            corner_radius=10,
            fg_color=COLORS["bg-2"],
            border_width=1,
            border_color=COLORS["line-soft"],
        )
        hero.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        hero.grid_columnconfigure(1, weight=1)

        # Logo (88px)
        try:
            from PIL import Image

            png = get_png_path(128)
            if png is not None:
                self._hero_logo_pil = Image.open(str(png))
                self._hero_logo = ctk.CTkImage(
                    light_image=self._hero_logo_pil,
                    dark_image=self._hero_logo_pil,
                    size=(88, 88),
                )
                ctk.CTkLabel(hero, image=self._hero_logo, text="").grid(
                    row=0, column=0, rowspan=4, padx=(20, 16), pady=20, sticky="nw"
                )
        except Exception as e:
            print(f"Studio hero logo not loaded: {e}")

        ctk.CTkLabel(
            hero,
            text="Impulcifer",
            font=ctk.CTkFont(family=self.app.font_family, size=24, weight="bold"),
            anchor="w",
        ).grid(row=0, column=1, sticky="sw", padx=(0, 20), pady=(20, 0))

        if is_velopack_environment():
            install_text = self.loc.get("info_install_velopack")
        elif is_pip_environment():
            install_text = self.loc.get("info_install_pip")
        else:
            install_text = self.loc.get("info_install_dev")

        version_pill = (
            f"VERSION {impulcifer.__version__}  ·  PYTHON "
            f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}  "
            f"·  {install_text}"
        )
        ctk.CTkLabel(
            hero,
            text=version_pill,
            font=ctk.CTkFont(family=get_mono_font_family(), size=11, weight="bold"),
            text_color=COLORS["accent"],
            anchor="w",
        ).grid(row=1, column=1, sticky="nw", padx=(0, 20), pady=(4, 0))

        ctk.CTkLabel(
            hero,
            text=self.loc.get("app_subtitle"),
            font=self.fonts["label"],
            text_color=COLORS["fg-1"],
            anchor="w",
            wraplength=520,
            justify="left",
        ).grid(row=2, column=1, sticky="nw", padx=(0, 20), pady=(8, 12))

        # Action buttons
        actions = ctk.CTkFrame(hero, fg_color="transparent")
        actions.grid(row=3, column=1, sticky="w", padx=(0, 20), pady=(0, 20))
        ctk.CTkButton(
            actions,
            text=self.loc.get("button_view_license"),
            command=self._open_license,
            width=140,
        ).grid(row=0, column=0, padx=(0, 8))
        ctk.CTkButton(
            actions,
            text=self.loc.get("button_report_bug"),
            command=lambda: webbrowser.open(
                "https://github.com/115dkk/Impulcifer-pip313/issues/new"
            ),
            width=140,
        ).grid(row=0, column=1)

    # ------------------------------------------------------------------
    # Environment card
    # ------------------------------------------------------------------
    def _build_environment_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="env", title=self.loc.get("studio_card_environment"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure((0, 1), weight=1, uniform="kv")

        try:
            threading_info = get_python_threading_info()
        except Exception:
            threading_info = {}

        py_ver = threading_info.get(
            "python_version",
            f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
        )
        os_name = f"{platform.system()} {platform.release()}"
        cpu_cores = str(threading_info.get("cpu_count", os.cpu_count() or "?"))
        optimal_workers = str(threading_info.get("optimal_workers", "?"))
        gil_raw = threading_info.get("gil_enabled", "unknown")
        gil_text = (
            self.loc.get("info_gil_enabled")
            if gil_raw is True
            else self.loc.get("info_gil_disabled")
            if gil_raw is False
            else self.loc.get("info_gil_unknown")
        )

        items = [
            (self.loc.get("label_python"), py_ver),
            (self.loc.get("label_os"), os_name),
            (self.loc.get("label_cpu_cores"), cpu_cores),
            (self.loc.get("label_gil_status"), gil_text),
            (self.loc.get("label_optimal_workers"), optimal_workers),
            (self.loc.get("label_version").rstrip(":：").strip(), impulcifer.__version__),
        ]
        for i, (label, value) in enumerate(items):
            cell = ctk.CTkFrame(
                body,
                corner_radius=4,
                fg_color=COLORS["bg-3"],
                border_width=1,
                border_color=COLORS["line-soft"],
            )
            cell.grid(row=i // 2, column=i % 2, sticky="ew", padx=4, pady=4)
            cell.grid_columnconfigure(1, weight=1)
            ctk.CTkLabel(
                cell, text=label, font=self.fonts["small"], text_color=COLORS["fg-2"], anchor="w"
            ).grid(row=0, column=0, padx=10, pady=8, sticky="w")
            ctk.CTkLabel(
                cell,
                text=value,
                font=ctk.CTkFont(family=get_mono_font_family(), size=12),
                text_color=COLORS["fg-0"],
                anchor="e",
            ).grid(row=0, column=1, padx=10, pady=8, sticky="e")

    # ------------------------------------------------------------------
    # Links card
    # ------------------------------------------------------------------
    def _build_links_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="link", title=self.loc.get("studio_card_links"), fonts=self.fonts)
        body = make_card_body(card, padx=0, pady=0)
        body.grid_columnconfigure(0, weight=1)

        links = (
            (self.loc.get("button_original_repo"), "github.com/jaakkopasanen/Impulcifer",
             "https://github.com/jaakkopasanen/Impulcifer"),
            (self.loc.get("button_fork_repo"), "github.com/115dkk/Impulcifer-pip313",
             "https://github.com/115dkk/Impulcifer-pip313"),
            ("CHANGELOG", "CHANGELOG.md",
             "https://github.com/115dkk/Impulcifer-pip313/blob/master/CHANGELOG.md"),
        )
        for i, (title, sub, url) in enumerate(links):
            row_frame = ctk.CTkFrame(body, fg_color="transparent", corner_radius=0)
            row_frame.grid(row=i, column=0, sticky="ew")
            row_frame.grid_columnconfigure(0, weight=1)

            text_col = ctk.CTkFrame(row_frame, fg_color="transparent")
            text_col.grid(row=0, column=0, sticky="w", padx=18, pady=12)
            ctk.CTkLabel(
                text_col, text=title, font=self.fonts["label"], anchor="w"
            ).grid(row=0, column=0, sticky="w")
            ctk.CTkLabel(
                text_col,
                text=sub,
                font=ctk.CTkFont(family=get_mono_font_family(), size=12),
                text_color=COLORS["fg-2"],
                anchor="w",
            ).grid(row=1, column=0, sticky="w", pady=(2, 0))

            ctk.CTkButton(
                row_frame,
                text="↗",
                command=lambda u=url: webbrowser.open(u),
                width=36,
                height=28,
                corner_radius=4,
                fg_color="transparent",
                hover_color=COLORS["accent-soft"],
                text_color=COLORS["fg-2"],
            ).grid(row=0, column=1, padx=18, pady=8, sticky="e")

            if i < len(links) - 1:
                rule = ctk.CTkFrame(body, fg_color=COLORS["line-soft"], height=1, corner_radius=0)
                rule.grid(row=i, column=0, sticky="sew", padx=18)

    @staticmethod
    def _open_license() -> None:
        """Open LICENSE in the default text viewer."""
        candidates = [
            Path(sys.executable).parent / "License.txt",
            Path(__file__).parent.parent.parent / "LICENSE",
            Path(__file__).parent.parent.parent / "License.txt",
        ]
        for p in candidates:
            if p.exists():
                if platform.system() == "Windows":
                    os.startfile(p)
                elif platform.system() == "Darwin":
                    os.system(f'open "{p}"')
                else:
                    os.system(f'xdg-open "{p}"')
                return
        webbrowser.open("https://github.com/115dkk/Impulcifer-pip313/blob/main/LICENSE")
