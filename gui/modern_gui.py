#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Modern GUI for Impulcifer using CustomTkinter."""

from __future__ import annotations

import os
import threading
from tkinter import messagebox

import customtkinter as ctk

from gui.constants import WINDOW_MAIN_SIZE
from gui.dialogs import LanguageSelectionDialog, UpdateDialog
from gui.event_bus import EventBus
from gui.tabs.impulcifer_tab import ImpulciferTab
from gui.tabs.info_tab import InfoTab
from gui.tabs.recorder_tab import RecorderTab
from gui.tabs.settings_tab import SettingsTab
from gui.utils import build_fonts, setup_pretendard_font
from i18n.localization import get_localization_manager
from updater.update_checker import UpdateChecker

# Default theme setting (will be overridden by user preference)
ctk.set_default_color_theme("blue")  # Themes: "blue" (default), "green", "dark-blue"


class ModernImpulciferGUI:
    """Top-level orchestrator for the modern GUI."""

    def __init__(self) -> None:
        """Initialize localization, theme, root window, and tabs."""
        # Initialize localization manager
        self.loc = get_localization_manager()
        self.current_theme = self.loc.get_theme()
        self.bus = EventBus()
        self.bus.on('language_changed', self._handle_language_changed)
        self.bus.on('theme_changed', self._handle_theme_changed)

        self.font_family = None

        # Apply saved theme
        if self.current_theme == 'system':
            ctk.set_appearance_mode("system")
        else:
            ctk.set_appearance_mode(self.current_theme)

        self.root = ctk.CTk()
        self.root.title(self.loc.get('app_title') + " - " + self.loc.get('app_subtitle'))

        # Setup font after Tk exists so bundled font registration can be
        # verified against Tk-visible font families.
        self.font_family = setup_pretendard_font(self.loc.current_language)

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
        window_width, window_height = WINDOW_MAIN_SIZE
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

    def create_header(self) -> None:
        """Create the localized app header."""
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

    def toggle_theme(self) -> None:
        """Toggle between dark and light themes (legacy method - kept for compatibility)"""
        if self.current_theme == "dark":
            self.bus.emit('theme_changed', code="light")
        else:
            self.bus.emit('theme_changed', code="dark")

    def create_tabs(self) -> None:
        """Create the localized tab view."""
        self.tabview = ctk.CTkTabview(self.root, corner_radius=10)
        self.tabview.grid(row=1, column=0, padx=20, pady=(0, 20), sticky="nsew")

        self.tab_keys = {
            'recorder': 'tab_recorder',
            'impulcifer': 'tab_impulcifer',
            'settings': 'tab_ui_settings',
            'info': 'tab_info',
        }

        for tab_key in self.tab_keys.values():
            self.tabview.add(self.loc.get(tab_key))

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
        return "2.4.15"

    def check_for_updates_background(self) -> None:
        """Check for updates in a background thread."""
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

    def show_update_dialog(self, current_version: str, latest_version: str, download_url: str, release_notes: str) -> None:
        """Show update notification dialog."""
        UpdateDialog(self.root, self.loc, current_version, latest_version, download_url, release_notes, fonts=self.fonts)

    def show_language_selection_dialog(self) -> None:
        """Show the first-run language selection dialog."""
        LanguageSelectionDialog(
            self.root,
            self.loc,
            self.fonts,
            on_complete=self._complete_first_run_language_selection,
        ).show()

    def _complete_first_run_language_selection(self, language_code: str) -> None:
        """Apply the first-run language selection."""
        self.bus.emit(
            'language_changed',
            code=language_code,
            selected_tab_key='recorder',
            mark_selected=True,
        )
        messagebox.showinfo(
            self.loc.get('message_info'),
            self.loc.get(
                'message_language_changed',
                language=self.loc.get_language_name(language_code),
            ),
        )

    def _handle_theme_changed(self, code: str) -> None:
        """Apply and persist a theme change event."""
        if code == 'system':
            ctk.set_appearance_mode("system")
        else:
            ctk.set_appearance_mode(code)
        self.loc.set_theme(code)
        self.current_theme = code

    def _handle_language_changed(
        self,
        code: str,
        selected_tab_key: str = 'settings',
        mark_selected: bool = False,
    ) -> None:
        """Apply a language change and rebuild localized widgets."""
        self.loc.set_language(code)
        if mark_selected:
            self.loc.mark_language_selected()
        self.refresh_localized_ui(selected_tab_key=selected_tab_key)

    def refresh_localized_ui(self, selected_tab_key: str = 'settings') -> None:
        """Rebuild localized widgets while preserving tab input state."""
        state = self._collect_tab_state()
        for child in self.root.winfo_children():
            child.destroy()

        self.root.title(self.loc.get('app_title') + " - " + self.loc.get('app_subtitle'))
        self.font_family = setup_pretendard_font(self.loc.current_language)
        self.fonts = build_fonts(self.font_family)

        self.create_header()
        self.create_tabs()
        self.recorder_tab = RecorderTab(self)
        self.impulcifer_tab = ImpulciferTab(self)
        self.settings_tab = SettingsTab(self)
        self.info_tab = InfoTab(self)
        self._restore_tab_state(state)
        self.select_tab(selected_tab_key)

    def _collect_tab_state(self) -> dict[str, dict]:
        """Collect state snapshots from tabs that support it."""
        state: dict[str, dict] = {}
        for name in ('recorder_tab', 'impulcifer_tab'):
            tab = getattr(self, name, None)
            if tab is not None and hasattr(tab, 'get_state'):
                state[name] = tab.get_state()
        return state

    def _restore_tab_state(self, state: dict[str, dict]) -> None:
        """Restore state snapshots after rebuilding tabs."""
        for name, tab_state in state.items():
            tab = getattr(self, name, None)
            if tab is not None and hasattr(tab, 'apply_state'):
                tab.apply_state(tab_state)

    def select_tab(self, tab_key: str) -> None:
        """Select a tab by stable internal key."""
        loc_key = self.tab_keys.get(tab_key)
        if loc_key is not None:
            self.tabview.set(self.loc.get(loc_key))

    def run(self) -> None:
        """Start the GUI main loop."""
        self.root.mainloop()


def main_gui() -> None:
    """Start the modern GUI application."""
    app = ModernImpulciferGUI()
    app.run()


if __name__ == "__main__":
    main_gui()
