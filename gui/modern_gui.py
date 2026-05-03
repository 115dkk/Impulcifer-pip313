#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Modern GUI for Impulcifer using CustomTkinter
Professional-grade interface with dark/light mode support
"""

import os
import threading
from tkinter import messagebox

import customtkinter as ctk

from gui.dialogs import UpdateDialog
from gui.tabs.impulcifer_tab import ImpulciferTab
from gui.tabs.info_tab import InfoTab
from gui.tabs.recorder_tab import RecorderTab
from gui.tabs.settings_tab import SettingsTab
from gui.utils import build_fonts, setup_pretendard_font
from i18n.localization import SUPPORTED_LANGUAGES, get_localization_manager
from updater.update_checker import UpdateChecker

# Default theme setting (will be overridden by user preference)
ctk.set_default_color_theme("blue")  # Themes: "blue" (default), "green", "dark-blue"


class ModernImpulciferGUI:
    def __init__(self):
        # Initialize localization manager
        self.loc = get_localization_manager()

        # Setup font based on language
        self.font_family = setup_pretendard_font(self.loc.current_language)

        # Apply saved theme
        saved_theme = self.loc.get_theme()
        if saved_theme == 'system':
            ctk.set_appearance_mode("system")
        else:
            ctk.set_appearance_mode(saved_theme)

        self.root = ctk.CTk()
        self.root.title(self.loc.get('app_title') + " - " + self.loc.get('app_subtitle'))

        # Shared CTkFont instances — reused across all tabs and dialogs to avoid
        # rebuilding identical font specs for every label/button (was a major
        # cost at startup since most widgets requested size=16 bold).
        self.fonts = build_fonts(self.font_family)

        # Show language selection dialog on first run
        if self.loc.is_first_run():
            self.root.after(500, self.show_language_selection_dialog)

        # Check for updates in background (after 2 seconds)
        self.root.after(2000, self.check_for_updates_background)

        # Velopack 업데이트 후 재시작 감지 — 사용자에게 완료 알림
        if os.environ.get('VELOPACK_RESTART'):
            self.root.after(1000, lambda: messagebox.showinfo(
                self.loc.get('update_complete_title', default="Update Complete"),
                self.loc.get('update_restart_done', default="The new version has been installed successfully.")
            ))

        # Set window size and position
        window_width = 1000
        window_height = 700
        screen_width = self.root.winfo_screenwidth()
        screen_height = self.root.winfo_screenheight()
        x = (screen_width - window_width) // 2
        y = (screen_height - window_height) // 2
        self.root.geometry(f"{window_width}x{window_height}+{x}+{y}")

        # Configure grid weight
        self.root.grid_rowconfigure(1, weight=1)
        self.root.grid_columnconfigure(0, weight=1)

        # Create header frame
        self.create_header()

        # Create main tab view
        self.create_tabs()

        # Instantiate tab classes (each builds its own widgets in __init__)
        self.recorder_tab = RecorderTab(self)
        self.impulcifer_tab = ImpulciferTab(self)
        self.settings_tab = SettingsTab(self)
        self.info_tab = InfoTab(self)

    def create_header(self):
        """Create header with app title and theme toggle"""
        header = ctk.CTkFrame(self.root, corner_radius=0, height=60)
        header.grid(row=0, column=0, sticky="ew", padx=0, pady=0)
        header.grid_columnconfigure(1, weight=1)

        # App title
        self.title_label = ctk.CTkLabel(
            header,
            text=self.loc.get('app_title'),
            font=self.fonts['title']
        )
        self.title_label.grid(row=0, column=0, padx=20, pady=15, sticky="w")

        # Subtitle
        self.subtitle_label = ctk.CTkLabel(
            header,
            text=self.loc.get('app_subtitle'),
            font=self.fonts['subtitle'],
            text_color="gray"
        )
        self.subtitle_label.grid(row=0, column=1, padx=10, pady=15, sticky="w")

        # Theme toggle button (removed - moved to UI Settings tab)
        # Now header is cleaner
        self.current_theme = self.loc.get_theme()

    def toggle_theme(self):
        """Toggle between dark and light themes (legacy method - kept for compatibility)"""
        if self.current_theme == "dark":
            ctk.set_appearance_mode("light")
            self.current_theme = "light"
        else:
            ctk.set_appearance_mode("dark")
            self.current_theme = "dark"
        self.loc.set_theme(self.current_theme)

    def create_tabs(self):
        """Create main tab view"""
        self.tabview = ctk.CTkTabview(self.root, corner_radius=10)
        self.tabview.grid(row=1, column=0, padx=20, pady=(0, 20), sticky="nsew")

        # Add tabs (using localized text)
        self.tabview.add(self.loc.get('tab_recorder'))
        self.tabview.add(self.loc.get('tab_impulcifer'))
        self.tabview.add(self.loc.get('tab_ui_settings'))
        self.tabview.add(self.loc.get('tab_info'))

        # Set default tab
        self.tabview.set(self.loc.get('tab_recorder'))

    def get_current_version(self) -> str:
        """Get current application version from build marker, pyproject.toml, or metadata."""
        # Method 0: 빌드 마커 (Nuitka/pip 빌드에서 가장 확실)
        try:
            from infra._build_info import VERSION as build_version
            if build_version is not None:
                return build_version
        except ImportError:
            pass

        # Method 1: impulcifer.__version__ (이미 빌드 마커 → pyproject.toml → metadata 순으로 시도)
        try:
            import impulcifer
            if hasattr(impulcifer, '__version__'):
                return impulcifer.__version__
        except Exception:
            pass

        # Fallback
        return "2.4.11"

    def check_for_updates_background(self):
        """Check for updates in background thread"""
        def check_updates():
            try:
                current_version = self.get_current_version()
                checker = UpdateChecker(current_version)
                has_update, latest_version, download_url = checker.check_for_updates()

                if has_update and download_url:
                    # Show update dialog on main thread
                    release_notes = checker.get_release_notes() or ""
                    self.root.after(0, lambda: self.show_update_dialog(
                        current_version, latest_version, download_url, release_notes
                    ))
            except Exception as e:
                # Silently fail - don't disturb user if update check fails
                print(f"Update check failed: {e}")

        # Run in background thread
        update_thread = threading.Thread(target=check_updates, daemon=True)
        update_thread.start()

    def show_update_dialog(self, current_version: str, latest_version: str, download_url: str, release_notes: str):
        """Show update notification dialog"""
        UpdateDialog(self.root, self.loc, current_version, latest_version, download_url, release_notes, fonts=self.fonts)

    def show_language_selection_dialog(self):
        """Show language selection dialog on first run"""
        dialog = ctk.CTkToplevel(self.root)
        dialog.title(self.loc.get('dialog_select_language_title'))
        dialog.geometry("400x550")
        dialog.transient(self.root)
        dialog.grab_set()

        # Center the dialog
        dialog.update_idletasks()
        x = (dialog.winfo_screenwidth() // 2) - (400 // 2)
        y = (dialog.winfo_screenheight() // 2) - (550 // 2)
        dialog.geometry(f"400x550+{x}+{y}")

        # Message
        message = ctk.CTkLabel(
            dialog,
            text=self.loc.get('dialog_select_language_message'),
            font=self.fonts['dialog_body'],
            wraplength=350
        )
        message.pack(pady=20, padx=20)

        # Language list
        lang_frame = ctk.CTkFrame(dialog)
        lang_frame.pack(pady=10, padx=20, fill="both", expand=True)

        selected_lang = ctk.StringVar(value=self.loc.current_language)

        for lang_code, lang_name in SUPPORTED_LANGUAGES.items():
            rb = ctk.CTkRadioButton(
                lang_frame,
                text=lang_name,
                variable=selected_lang,
                value=lang_code,
                font=self.fonts['subtitle']
            )
            rb.pack(pady=5, padx=10, anchor="w")

        # Buttons
        button_frame = ctk.CTkFrame(dialog)
        button_frame.pack(pady=10, padx=20, fill="x")

        def on_ok():
            self.loc.set_language(selected_lang.get())
            self.loc.mark_language_selected()
            dialog.destroy()
            # Show message about restart
            messagebox.showinfo(
                self.loc.get('message_info'),
                self.loc.get('message_language_changed', language=self.loc.get_language_name(selected_lang.get()))
            )

        ok_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('button_ok'),
            command=on_ok
        )
        ok_button.pack(side="right", padx=5)

        dialog.protocol("WM_DELETE_WINDOW", on_ok)

    def run(self):
        """Start the GUI main loop"""
        self.root.mainloop()


def main_gui():
    """Entry point for modern GUI"""
    app = ModernImpulciferGUI()
    app.run()


if __name__ == "__main__":
    main_gui()
