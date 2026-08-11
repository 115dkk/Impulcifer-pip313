"""Stable-skin output recovery tab."""

from __future__ import annotations

from typing import TYPE_CHECKING

import customtkinter as ctk

from gui.recovery_actions import RecoveryActionsMixin
from gui.theme import COLORS, get_mono_font_family
from gui.utils import install_smooth_scrolling

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class RecoveryTab(RecoveryActionsMixin):
    """Restore missing hrir.wav, hesuvi.wav, and optional Hangloose files."""

    def __init__(self, app: ModernImpulciferGUI) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.root = app.root
        self.tabview = app.tabview
        self.dir_path_var = ctk.StringVar(value="data/my_hrir")
        self.include_hangloose_var = ctk.BooleanVar(value=False)
        self._init_recovery_actions()
        self._build()

    def _build(self) -> None:
        tab = self.tabview.tab(self.loc.get("tab_output_recovery"))
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        header = ctk.CTkFrame(scroll, fg_color="transparent")
        header.grid(row=0, column=0, sticky="ew", padx=10, pady=(10, 14))
        header.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(
            header,
            text=self.loc.get("studio_recovery_title"),
            font=self.fonts["title"],
            anchor="w",
        ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(
            header,
            text=self.loc.get("studio_recovery_subtitle"),
            font=self.fonts["small"],
            text_color=COLORS["fg-2"],
            anchor="w",
        ).grid(row=1, column=0, sticky="w", pady=(4, 0))
        self.restore_button = ctk.CTkButton(
            header,
            text=self.loc.get("recovery_action"),
            command=self.start_recovery,
            width=160,
        )
        self.restore_button.grid(row=0, column=1, rowspan=2, sticky="e", padx=(12, 0))

        self._build_source(scroll, row=1)
        self._build_options(scroll, row=2)
        self._build_result(scroll, row=3)

    def _section(self, parent: ctk.CTkBaseClass, row: int, title: str) -> ctk.CTkFrame:
        frame = ctk.CTkFrame(parent, corner_radius=8)
        frame.grid(row=row, column=0, sticky="ew", padx=10, pady=(0, 12))
        frame.grid_columnconfigure(0, weight=1)
        ctk.CTkLabel(
            frame,
            text=title,
            font=self.fonts["heading"],
            anchor="w",
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))
        return frame

    def _build_source(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        frame = self._section(parent, row, self.loc.get("recovery_card_source"))
        field = ctk.CTkFrame(frame, fg_color="transparent")
        field.grid(row=1, column=0, sticky="ew", padx=15)
        field.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(
            field,
            text=self.loc.get("recovery_source_label"),
            text_color=COLORS["fg-1"],
            anchor="w",
        ).grid(row=0, column=0, sticky="w", padx=(0, 12))
        ctk.CTkEntry(
            field,
            textvariable=self.dir_path_var,
            font=ctk.CTkFont(family=get_mono_font_family(), size=12),
        ).grid(row=0, column=1, sticky="ew")
        ctk.CTkButton(
            field,
            text=self.loc.get("button_browse"),
            command=self.browse_recovery_directory,
            width=90,
        ).grid(row=0, column=2, padx=(10, 0))
        ctk.CTkLabel(
            frame,
            text=self.loc.get("recovery_source_hint"),
            font=self.fonts["small"],
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=760,
        ).grid(row=2, column=0, sticky="w", padx=15, pady=(8, 15))

    def _build_options(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        frame = self._section(parent, row, self.loc.get("recovery_card_options"))
        ctk.CTkCheckBox(
            frame,
            text=self.loc.get("recovery_include_hangloose"),
            variable=self.include_hangloose_var,
        ).grid(row=1, column=0, sticky="w", padx=15)
        ctk.CTkLabel(
            frame,
            text=(
                f"{self.loc.get('recovery_include_hangloose_hint')}\n"
                f"{self.loc.get('recovery_preserve_hint')}"
            ),
            font=self.fonts["small"],
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=760,
        ).grid(row=2, column=0, sticky="w", padx=15, pady=(8, 15))

    def _build_result(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        frame = self._section(parent, row, self.loc.get("recovery_card_result"))
        self.status_label = ctk.CTkLabel(
            frame,
            text=self.loc.get("webview_job_idle"),
            font=self.fonts["heading"],
            text_color=COLORS["fg-1"],
            anchor="w",
        )
        self.status_label.grid(row=1, column=0, sticky="w", padx=15)
        self.summary_label = ctk.CTkLabel(
            frame,
            text=self.loc.get("recovery_idle_detail"),
            font=self.fonts["label"],
            text_color=COLORS["fg-1"],
            anchor="w",
            justify="left",
            wraplength=760,
        )
        self.summary_label.grid(row=2, column=0, sticky="w", padx=15, pady=(6, 0))
        self.files_label = ctk.CTkLabel(
            frame,
            text="",
            font=ctk.CTkFont(family=get_mono_font_family(), size=11),
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=760,
        )
        self.files_label.grid(row=3, column=0, sticky="w", padx=15, pady=(6, 0))
        self.open_button = ctk.CTkButton(
            frame,
            text=self.loc.get("webview_open_output_folder"),
            command=self.open_recovery_output,
            width=180,
        )
        self.open_button.grid(row=4, column=0, sticky="w", padx=15, pady=(10, 15))
        self.open_button.grid_remove()
