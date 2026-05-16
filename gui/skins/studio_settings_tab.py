#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Studio Settings tab — skin selector + language + theme + paths."""

from __future__ import annotations

from tkinter import messagebox
from typing import TYPE_CHECKING

import customtkinter as ctk

from gui.skins import SKIN_STABLE, SKIN_STUDIO
from gui.skins.studio_widgets import add_card_header, make_card, make_card_body, make_page_header
from gui.theme import COLORS
from gui.utils import install_smooth_scrolling, open_data_folder
from i18n.localization import SUPPORTED_LANGUAGES

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioSettingsTab:
    """Studio Settings — appearance, language, paths in card layout."""

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
            title=self.loc.get("studio_settings_title"),
            subtitle=self.loc.get("studio_settings_subtitle"),
            fonts=self.fonts,
        )
        page_header.grid(row=0, column=0, sticky="ew", pady=(0, 18))

        self._build_appearance_card(scroll, row=1)
        self._build_paths_card(scroll, row=2)

    # ------------------------------------------------------------------
    # Appearance
    # ------------------------------------------------------------------
    def _build_appearance_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(
            card, number="UI", title=self.loc.get("studio_card_appearance"), fonts=self.fonts
        )
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        # Skin selector — segmented button
        self._build_setting_row(
            body,
            row=0,
            label=self.loc.get("section_skin"),
            description=self._current_skin_description(),
            make_control=self._make_skin_segment,
        )

        # Language selector
        self._build_setting_row(
            body,
            row=1,
            label=self.loc.get("section_language"),
            description=self.loc.get("label_select_language"),
            make_control=self._make_language_dropdown,
        )

        # Theme selector
        self._build_setting_row(
            body,
            row=2,
            label=self.loc.get("section_theme"),
            description=self.loc.get("label_select_theme"),
            make_control=self._make_theme_dropdown,
        )

    def _build_setting_row(
        self,
        parent: ctk.CTkBaseClass,
        *,
        row: int,
        label: str,
        description: str,
        make_control,
    ) -> None:
        """Render a settings row.

        ``make_control`` is a factory that takes the row's right-hand
        cell as parent and creates the actual control there. Passing the
        parent live (instead of pre-built control) avoids the cross-
        parent grid bug where the control's geometry parent didn't match
        its widget parent and ended up gridding into the wrong cell.
        """
        row_frame = ctk.CTkFrame(parent, fg_color="transparent")
        row_frame.grid(row=row * 2, column=0, sticky="ew", pady=(8, 4))
        row_frame.grid_columnconfigure(0, weight=1)

        text_col = ctk.CTkFrame(row_frame, fg_color="transparent")
        text_col.grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(
            text_col,
            text=label,
            font=ctk.CTkFont(family=self.app.font_family, size=13, weight="bold"),
            anchor="w",
        ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(
            text_col,
            text=description,
            font=ctk.CTkFont(family=self.app.font_family, size=12),
            text_color=COLORS["fg-2"],
            anchor="w",
        ).grid(row=1, column=0, sticky="w", pady=(2, 0))

        # Right-side control cell — the make_control callable creates the
        # widget AS A CHILD of this cell so its geometry parent matches.
        ctrl_cell = ctk.CTkFrame(row_frame, fg_color="transparent")
        ctrl_cell.grid(row=0, column=1, sticky="e", padx=(20, 0))
        control = make_control(ctrl_cell)
        if control is not None:
            control.grid(row=0, column=0, sticky="e")

        # Hairline below row
        rule = ctk.CTkFrame(parent, fg_color=COLORS["line-soft"], height=1, corner_radius=0)
        rule.grid(row=row * 2 + 1, column=0, sticky="sew", padx=4)

    def _current_skin_description(self) -> str:
        if self.loc.get_skin() == SKIN_STUDIO:
            return self.loc.get("tooltip_skin_studio")
        return self.loc.get("tooltip_skin_stable")

    def _make_skin_segment(self, parent: ctk.CTkBaseClass) -> ctk.CTkSegmentedButton:
        current = self.loc.get_skin()
        skin_label_map = {
            self.loc.get("option_skin_stable"): SKIN_STABLE,
            self.loc.get("option_skin_studio"): SKIN_STUDIO,
        }
        skin_value_map = {v: k for k, v in skin_label_map.items()}

        def _on_change(value: str) -> None:
            new_skin = skin_label_map.get(value, SKIN_STABLE)
            if new_skin == self.loc.get_skin():
                return
            self.app.bus.emit("skin_changed", code=new_skin)

        seg = ctk.CTkSegmentedButton(
            parent,
            values=list(skin_label_map.keys()),
            command=_on_change,
            font=ctk.CTkFont(family=self.app.font_family, size=12, weight="bold"),
            width=200,
        )
        seg.set(skin_value_map.get(current, self.loc.get("option_skin_stable")))
        return seg

    def _make_language_dropdown(self, parent: ctk.CTkBaseClass) -> ctk.CTkOptionMenu:
        var = ctk.StringVar(value=self.loc.get_language_name(self.loc.current_language))

        def _on_change(name: str) -> None:
            for code, lname in SUPPORTED_LANGUAGES.items():
                if lname == name:
                    self.app.bus.emit(
                        "language_changed", code=code, selected_tab_key="settings"
                    )
                    messagebox.showinfo(
                        self.loc.get("message_info"),
                        self.loc.get("message_language_changed", language=name),
                    )
                    return

        return ctk.CTkOptionMenu(
            parent,
            variable=var,
            values=list(SUPPORTED_LANGUAGES.values()),
            command=_on_change,
            width=180,
        )

    def _make_theme_dropdown(self, parent: ctk.CTkBaseClass) -> ctk.CTkOptionMenu:
        current = self.loc.get_theme()
        theme_display = {
            "dark": self.loc.get("option_theme_dark"),
            "light": self.loc.get("option_theme_light"),
            "system": self.loc.get("option_theme_system"),
        }
        var = ctk.StringVar(value=theme_display.get(current, theme_display["dark"]))
        display_to_code = {v: k for k, v in theme_display.items()}

        def _on_change(name: str) -> None:
            code = display_to_code.get(name, "dark")
            self.app.bus.emit("theme_changed", code=code)

        return ctk.CTkOptionMenu(
            parent,
            variable=var,
            values=list(theme_display.values()),
            command=_on_change,
            width=180,
        )

    # ------------------------------------------------------------------
    # Paths card
    # ------------------------------------------------------------------
    def _build_paths_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(
            card, number="dir", title=self.loc.get("studio_card_paths"), fonts=self.fonts
        )
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        ctk.CTkLabel(
            body,
            text=self.loc.get("label_data_folder_description",
                              default="Access reference files, test signals, and recordings"),
            font=ctk.CTkFont(family=self.app.font_family, size=12),
            text_color=COLORS["fg-2"],
            anchor="w",
        ).grid(row=0, column=0, sticky="w", pady=(0, 10))

        ctk.CTkButton(
            body,
            text=self.loc.get("button_open_data_folder", default="Open Data Folder"),
            command=open_data_folder,
            width=200,
        ).grid(row=1, column=0, sticky="w")
