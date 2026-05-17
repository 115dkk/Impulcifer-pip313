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
from gui.skins import SKIN_STABLE, SKIN_STUDIO
from gui.tabs.impulcifer_tab import ImpulciferTab
from gui.tabs.info_tab import InfoTab
from gui.tabs.recorder_tab import RecorderTab
from gui.tabs.settings_tab import SettingsTab
from gui.theme import get_ctk_theme_json_path
from gui.utils import build_fonts, setup_app_icon, setup_pretendard_font
from i18n.localization import get_localization_manager
from updater.update_checker import UpdateChecker

# Apply the Pulse audio-equipment palette when the bundled theme JSON is
# present (set_default_color_theme silently falls back to "blue" if the
# file is missing, e.g. when running from an unusual layout).
_pulse_theme_path = get_ctk_theme_json_path()
if _pulse_theme_path is not None:
    ctk.set_default_color_theme(str(_pulse_theme_path))
else:
    ctk.set_default_color_theme("blue")


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
        self.bus.on('skin_changed', self._handle_skin_changed)

        # Active skin (Stable = compact tabview, Studio = sidebar + cards).
        # Persisted via LocalizationManager so it survives across launches.
        self.skin = self.loc.get_skin()
        self.studio_shell = None
        self.font_family = None

        # Apply saved theme
        if self.current_theme == 'system':
            ctk.set_appearance_mode("system")
        else:
            ctk.set_appearance_mode(self.current_theme)

        self.root = ctk.CTk()
        self.root.title(self.loc.get('app_title') + " - " + self.loc.get('app_subtitle'))

        # Apply the Pulse logo to title bar + Windows taskbar before any
        # child widget gets a chance to resolve its own icon. Failures are
        # non-fatal (logged) — the GUI works without the icon, just less
        # branded.
        setup_app_icon(self.root)

        # Setup font after Tk exists so bundled font registration can be
        # verified against Tk-visible font families.
        self.font_family = setup_pretendard_font(self.loc.current_language)
        self._sync_ctk_font_default(self.font_family)

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

        # Build the body in the active skin's layout.
        self._build_body()

    def create_header(self) -> None:
        """Create the localized app header.

        Layout follows the Pulse redesign's ``cv-brand`` pattern: 32px Pulse
        mark on the left, then a vertical stack with the app name (22px
        bold) over the subtitle (12px secondary). The previous header put
        title and subtitle side-by-side which read like two unrelated
        labels.
        """
        header = ctk.CTkFrame(self.root, corner_radius=0, height=72)
        header.grid(row=0, column=0, sticky="ew", padx=0, pady=0)
        header.grid_columnconfigure(2, weight=1)

        # Pulse logo (32px) — best effort. CTkImage uses tk.PhotoImage which
        # only reads PNG/GIF natively; we feed it the pre-rendered 64px PNG
        # and let CTk size it down for HiDPI sharpness.
        try:
            from PIL import Image
            from gui.theme import get_png_path

            png_path = get_png_path(64)
            if png_path is not None:
                self._header_logo_pil = Image.open(str(png_path))
                self._header_logo_image = ctk.CTkImage(
                    light_image=self._header_logo_pil,
                    dark_image=self._header_logo_pil,
                    size=(32, 32),
                )
                logo_label = ctk.CTkLabel(header, image=self._header_logo_image, text="")
                logo_label.grid(row=0, column=0, rowspan=2, padx=(20, 12), pady=14, sticky="w")
        except Exception as e:
            print(f"Header logo not loaded: {e}")

        # App title (22px bold)
        self.title_label = ctk.CTkLabel(
            header,
            text=self.loc.get('app_title'),
            font=self.fonts['title'],
            anchor="w",
        )
        self.title_label.grid(row=0, column=1, padx=(0, 12), pady=(14, 0), sticky="sw")

        # Subtitle (12px secondary)
        self.subtitle_label = ctk.CTkLabel(
            header,
            text=self.loc.get('app_subtitle'),
            font=self.fonts['subtitle'],
            text_color=("gray35", "gray60"),
            anchor="w",
        )
        self.subtitle_label.grid(row=1, column=1, padx=(0, 12), pady=(0, 14), sticky="nw")

    def toggle_theme(self) -> None:
        """Toggle between dark and light themes (legacy method - kept for compatibility)"""
        if self.current_theme == "dark":
            self.bus.emit('theme_changed', code="light")
        else:
            self.bus.emit('theme_changed', code="dark")

    def _build_body(self) -> None:
        """Construct the body region in the active skin's layout.

        - Stable: existing CTkTabview + four tab classes.
        - Studio: sidebar + content panel via :class:`StudioShell`.

        Each call assumes the previous body (if any) has already been torn
        down by the caller. ``_handle_skin_changed`` does that before
        invoking us again.
        """
        if self.skin == SKIN_STUDIO:
            from gui.skins.studio_shell import StudioShell

            self.studio_shell = StudioShell(self)
        else:
            self.create_tabs()
            self.recorder_tab = RecorderTab(self)
            self.impulcifer_tab = ImpulciferTab(self)
            self.settings_tab = SettingsTab(self)
            self.info_tab = InfoTab(self)

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

    @staticmethod
    def _sync_ctk_font_default(family: str | None) -> None:
        """Patch ``ctk.ThemeManager.theme["CTkFont"]`` to the live family.

        Why this exists. Widgets that don't pass an explicit ``font=``
        (``CTkOptionMenu``, ``CTkComboBox``, dropdown menus, internal
        labels) construct their own ``CTkFont()`` from the theme JSON.
        ``pulse.json`` ships ``"Pretendard"`` as a static fallback name,
        but the bundled variable file's family-name (name table id 1) is
        ``Pretendard Variable`` — on Korean Windows Tk silently falls
        through ``Pretendard`` to Gulim (굴림). The probe at
        ``CTkFont().cget('family')='Pretendard' -> tkfont.actual='굴림'``
        confirmed the regression. Patching the theme dict *before* any
        widget is built makes every subsequent ``CTkFont()`` instance
        adopt the resolved family.

        Idempotent and safe when ``family`` is None — leaves the JSON
        defaults in place so widgets fall back to system defaults
        cleanly.
        """
        if not family:
            return
        try:
            font_section = ctk.ThemeManager.theme.get("CTkFont", {})
            font_section["family"] = family
            for plat_key in ("Windows", "macOS", "Linux"):
                plat_entry = font_section.get(plat_key)
                if isinstance(plat_entry, dict):
                    plat_entry["family"] = family
        except Exception as e:
            print(f"CTkFont theme patch failed: {e}")

    def _handle_skin_changed(self, code: str) -> None:
        """Persist the skin choice and rebuild the body in the new layout.

        Mirrors ``refresh_localized_ui`` but only tears down the body
        region (header stays so the rebuild is fast and the user keeps
        the same window position). Recorder and Impulcifer input state is
        transferred when the two skins expose matching Tk variables.
        """
        if code not in (SKIN_STABLE, SKIN_STUDIO):
            return
        if code == self.skin:
            return
        state = self._collect_input_state()
        self.loc.set_skin(code)
        self.skin = code
        # Tear down current body — header is at row=0, body lives at row=1
        for child in self.root.grid_slaves(row=1, column=0):
            child.destroy()
        self.studio_shell = None
        self._build_body()
        self._restore_input_state(state)

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
        """Rebuild localized widgets while preserving tab input state.

        Recorder + Impulcifer tabs in both skins implement ``get_state`` /
        ``apply_state`` so language changes can rebuild labels without
        dropping user input.
        """
        state = self._collect_input_state()
        for child in self.root.winfo_children():
            child.destroy()

        self.root.title(self.loc.get('app_title') + " - " + self.loc.get('app_subtitle'))
        self.font_family = setup_pretendard_font(self.loc.current_language)
        self._sync_ctk_font_default(self.font_family)
        self.fonts = build_fonts(self.font_family)

        self.create_header()
        self.studio_shell = None
        self._build_body()
        self._restore_input_state(state, selected_tab_key=selected_tab_key)

    def _collect_input_state(self) -> dict:
        """Collect input snapshots from the active skin."""
        if self.skin == SKIN_STUDIO and self.studio_shell is not None:
            return self.studio_shell.get_state()

        tabs_state: dict[str, dict] = {}
        for key, name in (('recorder', 'recorder_tab'), ('impulcifer', 'impulcifer_tab')):
            tab = getattr(self, name, None)
            if tab is not None and hasattr(tab, 'get_state'):
                tabs_state[key] = tab.get_state()
        return {
            "active_key": self._current_stable_tab_key(),
            "tabs": tabs_state,
        }

    def _restore_input_state(self, state: dict, selected_tab_key: str | None = None) -> None:
        """Restore input snapshots after rebuilding the active skin."""
        tabs_state = state.get("tabs", {})
        active_key = selected_tab_key or state.get("active_key")

        if self.skin == SKIN_STUDIO:
            if self.studio_shell is not None:
                self.studio_shell.apply_state({
                    "active_key": active_key or "recorder",
                    "tabs": tabs_state,
                })
            return

        for key, name in (('recorder', 'recorder_tab'), ('impulcifer', 'impulcifer_tab')):
            tab_state = tabs_state.get(key)
            tab = getattr(self, name, None)
            if tab_state is not None and tab is not None and hasattr(tab, 'apply_state'):
                tab.apply_state(tab_state)
        if active_key in self.tab_keys:
            self.select_tab(active_key)

    def _current_stable_tab_key(self) -> str:
        """Return the Stable tab key currently selected in the CTkTabview."""
        tabview = getattr(self, "tabview", None)
        if tabview is None:
            return "recorder"
        try:
            selected_label = tabview.get()
        except Exception:
            return "recorder"
        for key, loc_key in self.tab_keys.items():
            if selected_label == self.loc.get(loc_key):
                return key
        return "recorder"

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
