#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""UI Settings tab for the modern GUI.

Handles language selection, theme selection, and a quick "Open Data Folder"
shortcut. Moved from ``gui/modern_gui.py`` without behavioural changes.
"""

from tkinter import messagebox

import customtkinter as ctk

from gui.utils import open_data_folder
from i18n.localization import SUPPORTED_LANGUAGES


class SettingsTab:
    def __init__(self, app):
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.tabview = app.tabview
        self.root = app.root
        self._build()

    def _build(self):
        """Create UI Settings tab for language and theme"""
        tab = self.tabview.tab(self.loc.get('tab_ui_settings'))
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        # Create scrollable frame
        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)

        row = 0

        # === Language Section ===
        lang_frame = ctk.CTkFrame(scroll, corner_radius=0)
        lang_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        lang_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            lang_frame,
            text=self.loc.get('section_language'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        ctk.CTkLabel(
            lang_frame,
            text=self.loc.get('label_select_language')
        ).grid(row=1, column=0, sticky="w", padx=15, pady=5)

        self.language_var = ctk.StringVar(value=self.loc.get_language_name(self.loc.current_language))
        language_menu = ctk.CTkOptionMenu(
            lang_frame,
            variable=self.language_var,
            values=list(SUPPORTED_LANGUAGES.values()),
            command=self.change_language
        )
        language_menu.grid(row=1, column=1, sticky="ew", padx=15, pady=5)

        # === Theme Section ===
        theme_frame = ctk.CTkFrame(scroll, corner_radius=0)
        theme_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        theme_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            theme_frame,
            text=self.loc.get('section_theme'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        ctk.CTkLabel(
            theme_frame,
            text=self.loc.get('label_select_theme')
        ).grid(row=1, column=0, sticky="w", padx=15, pady=5)

        current_theme = self.loc.get_theme()
        theme_display = {
            'dark': self.loc.get('option_theme_dark'),
            'light': self.loc.get('option_theme_light'),
            'system': self.loc.get('option_theme_system')
        }

        self.theme_var = ctk.StringVar(value=theme_display.get(current_theme, theme_display['dark']))
        theme_menu = ctk.CTkOptionMenu(
            theme_frame,
            variable=self.theme_var,
            values=[
                self.loc.get('option_theme_dark'),
                self.loc.get('option_theme_light'),
                self.loc.get('option_theme_system')
            ],
            command=self.change_theme
        )
        theme_menu.grid(row=1, column=1, sticky="ew", padx=15, pady=5)

        # === Data Access Section ===
        data_frame = ctk.CTkFrame(scroll, corner_radius=0)
        data_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        data_frame.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            data_frame,
            text=self.loc.get('section_data_access', default="Data Access"),
            font=self.fonts['heading']
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        # Description label
        ctk.CTkLabel(
            data_frame,
            text=self.loc.get('label_data_folder_description',
                             default="Access reference files, test signals, and recordings"),
            font=self.fonts['subtitle'],
            text_color="gray"
        ).grid(row=1, column=0, sticky="w", padx=15, pady=(0, 10))

        # Open data folder button
        open_folder_btn = ctk.CTkButton(
            data_frame,
            text=self.loc.get('button_open_data_folder', default="Open Data Folder"),
            command=open_data_folder,
            width=200
        )
        open_folder_btn.grid(row=2, column=0, sticky="w", padx=15, pady=(0, 15))

    def change_language(self, language_name):
        """Change application language"""
        # Find language code from name
        lang_code = None
        for code, name in SUPPORTED_LANGUAGES.items():
            if name == language_name:
                lang_code = code
                break

        if lang_code:
            self.loc.set_language(lang_code)
            messagebox.showinfo(
                self.loc.get('message_info'),
                self.loc.get('message_language_changed', language=language_name)
            )

    def change_theme(self, theme_name):
        """Change application theme"""
        # Map display name to theme code
        theme_map = {
            self.loc.get('option_theme_dark'): 'dark',
            self.loc.get('option_theme_light'): 'light',
            self.loc.get('option_theme_system'): 'system'
        }

        theme_code = theme_map.get(theme_name, 'dark')

        if theme_code == 'system':
            ctk.set_appearance_mode("system")
        else:
            ctk.set_appearance_mode(theme_code)

        self.loc.set_theme(theme_code)
        self.app.current_theme = theme_code

        messagebox.showinfo(
            self.loc.get('message_success'),
            self.loc.get('message_theme_changed')
        )
