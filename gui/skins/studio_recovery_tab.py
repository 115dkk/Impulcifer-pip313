"""Studio-skin output recovery tab."""

from __future__ import annotations

from typing import TYPE_CHECKING

import customtkinter as ctk

from gui.recovery_actions import RecoveryActionsMixin
from gui.skins.studio_widgets import (
    add_card_header,
    add_field_row,
    make_card,
    make_card_body,
    make_page_header,
)
from gui.theme import COLORS, get_mono_font_family
from gui.utils import install_smooth_scrolling

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioRecoveryTab(RecoveryActionsMixin):
    """Output recovery in the Studio card system."""

    def __init__(self, app: ModernImpulciferGUI, parent: ctk.CTkBaseClass) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.root = app.root
        self.parent = parent
        self.dir_path_var = ctk.StringVar(value="data/my_hrir")
        self.include_hangloose_var = ctk.BooleanVar(value=False)
        self._init_recovery_actions()
        self._build()

    def _build(self) -> None:
        self.parent.grid_columnconfigure(0, weight=1)
        self.parent.grid_rowconfigure(0, weight=1)
        scroll = ctk.CTkScrollableFrame(self.parent, fg_color=COLORS["bg-1"])
        scroll.grid(row=0, column=0, sticky="nsew", padx=24, pady=24)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        header = make_page_header(
            scroll,
            title=self.loc.get("studio_recovery_title"),
            subtitle=self.loc.get("studio_recovery_subtitle"),
            fonts=self.fonts,
            cta_label=self.loc.get("recovery_action"),
            cta_command=self.start_recovery,
        )
        header.grid(row=0, column=0, sticky="ew", pady=(0, 18))
        self.restore_button = next(
            child for child in header.winfo_children() if isinstance(child, ctk.CTkButton)
        )

        self._build_source_card(scroll, row=1)
        self._build_options_card(scroll, row=2)
        self._build_result_card(scroll, row=3)

    def _build_source_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(
            card,
            number="01",
            title=self.loc.get("recovery_card_source"),
            fonts=self.fonts,
        )
        body = make_card_body(card)
        add_field_row(
            body,
            row=0,
            label=self.loc.get("recovery_source_label"),
            value_var=self.dir_path_var,
            on_change=self.browse_recovery_directory,
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        ctk.CTkLabel(
            body,
            text=self.loc.get("recovery_source_hint"),
            font=self.fonts["small"],
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=720,
        ).grid(row=1, column=0, columnspan=2, sticky="w", pady=(6, 0))

    def _build_options_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(
            card,
            number="02",
            title=self.loc.get("recovery_card_options"),
            fonts=self.fonts,
        )
        body = make_card_body(card)
        ctk.CTkCheckBox(
            body,
            text=self.loc.get("recovery_include_hangloose"),
            variable=self.include_hangloose_var,
        ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(
            body,
            text=self.loc.get("recovery_include_hangloose_hint"),
            font=self.fonts["small"],
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=720,
        ).grid(row=1, column=0, sticky="w", pady=(6, 0))
        ctk.CTkLabel(
            body,
            text=self.loc.get("recovery_preserve_hint"),
            font=self.fonts["small"],
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=720,
        ).grid(row=2, column=0, sticky="w", pady=(4, 0))

    def _build_result_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(
            card,
            number="03",
            title=self.loc.get("recovery_card_result"),
            fonts=self.fonts,
        )
        body = make_card_body(card)
        self.status_label = ctk.CTkLabel(
            body,
            text=self.loc.get("webview_job_idle"),
            font=self.fonts["heading"],
            text_color=COLORS["fg-1"],
            anchor="w",
        )
        self.status_label.grid(row=0, column=0, sticky="w")
        self.summary_label = ctk.CTkLabel(
            body,
            text=self.loc.get("recovery_idle_detail"),
            font=self.fonts["label"],
            text_color=COLORS["fg-1"],
            anchor="w",
            justify="left",
            wraplength=720,
        )
        self.summary_label.grid(row=1, column=0, sticky="w", pady=(6, 0))
        self.files_label = ctk.CTkLabel(
            body,
            text="",
            font=ctk.CTkFont(family=get_mono_font_family(), size=11),
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=720,
        )
        self.files_label.grid(row=2, column=0, sticky="w", pady=(6, 0))
        self.open_button = ctk.CTkButton(
            body,
            text=self.loc.get("webview_open_output_folder"),
            command=self.open_recovery_output,
            width=180,
        )
        self.open_button.grid(row=3, column=0, sticky="w", pady=(10, 0))
        self.open_button.grid_remove()
