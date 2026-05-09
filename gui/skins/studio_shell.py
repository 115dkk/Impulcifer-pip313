#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Studio skin shell — sidebar navigation + content panel.

Replaces the Stable skin's CTkTabview with a sidebar-driven layout. Each
sidebar item swaps a tab implementation into the content panel. The four
Studio tab classes (StudioRecorderTab / StudioImpulciferTab /
StudioSettingsTab / StudioInfoTab) are responsible for filling the panel
they receive.
"""
from __future__ import annotations

from typing import TYPE_CHECKING

import customtkinter as ctk

from gui.theme import COLORS, get_mono_font_family, get_png_path

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioShell:
    """Sidebar + content panel orchestrator for the Studio skin."""

    NAV_ITEMS: tuple[tuple[str, str, str], ...] = (
        # (key, icon-glyph, i18n-key)
        ("recorder", "●", "sidebar_recorder"),
        ("impulcifer", "≡", "sidebar_processing"),
        ("settings", "⚙", "sidebar_settings"),
        ("info", "ⓘ", "sidebar_info"),
    )

    def __init__(self, app: ModernImpulciferGUI) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.root = app.root
        self.active_key = "recorder"
        self.nav_buttons: dict[str, ctk.CTkButton] = {}
        self.content_frames: dict[str, ctk.CTkFrame] = {}
        self.tabs: dict[str, object] = {}
        self._build()

    # ------------------------------------------------------------------
    # Shell layout
    # ------------------------------------------------------------------
    def _build(self) -> None:
        """Construct the sidebar + content area inside the root window."""
        # The root grid was configured by ModernImpulciferGUI for the
        # Stable layout (header row 0, content row 1). We reuse that:
        # the Studio shell takes the whole row 1 area and provides its
        # own internal split.
        shell = ctk.CTkFrame(self.root, corner_radius=0, fg_color=COLORS["bg-1"])
        shell.grid(row=1, column=0, sticky="nsew", padx=0, pady=0)
        shell.grid_columnconfigure(1, weight=1)
        shell.grid_rowconfigure(0, weight=1)
        self.shell = shell

        self._build_sidebar(shell)
        self._build_content(shell)
        self.select(self.active_key)

    def _build_sidebar(self, parent: ctk.CTkFrame) -> None:
        """Left sidebar: brand block + nav buttons."""
        sidebar = ctk.CTkFrame(
            parent,
            corner_radius=0,
            fg_color=COLORS["bg-0"],
            width=200,
            border_width=0,
        )
        sidebar.grid(row=0, column=0, sticky="nsw")
        sidebar.grid_propagate(False)
        sidebar.grid_columnconfigure(0, weight=1)
        sidebar.grid_rowconfigure(99, weight=1)

        # Brand block — small logo + "Impulcifer" + version. Mirrors the
        # design's `.st-brand` element.
        brand = ctk.CTkFrame(sidebar, fg_color="transparent")
        brand.grid(row=0, column=0, sticky="ew", padx=14, pady=(18, 12))
        brand.grid_columnconfigure(1, weight=1)

        try:
            from PIL import Image

            png_path = get_png_path(64)
            if png_path is not None:
                self._brand_logo_pil = Image.open(str(png_path))
                self._brand_logo = ctk.CTkImage(
                    light_image=self._brand_logo_pil,
                    dark_image=self._brand_logo_pil,
                    size=(28, 28),
                )
                ctk.CTkLabel(brand, image=self._brand_logo, text="").grid(
                    row=0, column=0, rowspan=2, padx=(0, 10), sticky="w"
                )
        except Exception as e:
            print(f"Studio brand logo not loaded: {e}")

        ctk.CTkLabel(
            brand,
            text="Impulcifer",
            font=ctk.CTkFont(family=self.app.font_family, size=15, weight="bold"),
            anchor="w",
        ).grid(row=0, column=1, sticky="sw")
        ctk.CTkLabel(
            brand,
            text=f"v{self._current_version()}",
            font=ctk.CTkFont(family=get_mono_font_family(), size=12),
            text_color=COLORS["fg-2"],
            anchor="w",
        ).grid(row=1, column=1, sticky="nw")

        # Hairline under brand
        rule = ctk.CTkFrame(sidebar, fg_color=COLORS["line-soft"], height=1, corner_radius=0)
        rule.grid(row=1, column=0, sticky="ew", padx=10, pady=(0, 8))

        # Nav buttons
        for idx, (key, glyph, label_key) in enumerate(self.NAV_ITEMS):
            btn = self._make_nav_button(sidebar, key, glyph, self.loc.get(label_key))
            btn.grid(row=2 + idx, column=0, sticky="ew", padx=8, pady=2)
            self.nav_buttons[key] = btn

    def _current_version(self) -> str:
        """Return ``self.app.get_current_version()`` defensively."""
        try:
            return self.app.get_current_version()
        except Exception:
            return "?"

    def _make_nav_button(
        self,
        parent: ctk.CTkFrame,
        key: str,
        glyph: str,
        label: str,
    ) -> ctk.CTkButton:
        """Create a single sidebar nav button (icon + label)."""

        def _on_click() -> None:
            self.select(key)

        btn = ctk.CTkButton(
            parent,
            text=f"  {glyph}    {label}",
            font=ctk.CTkFont(family=self.app.font_family, size=13, weight="bold"),
            command=_on_click,
            anchor="w",
            fg_color="transparent",
            hover_color=COLORS["bg-2"],
            text_color=COLORS["fg-1"],
            corner_radius=4,
            height=36,
        )
        return btn

    def _build_content(self, parent: ctk.CTkFrame) -> None:
        """Right content panel — one CTkFrame per tab, swapped via grid()."""
        content_host = ctk.CTkFrame(parent, fg_color=COLORS["bg-1"], corner_radius=0)
        content_host.grid(row=0, column=1, sticky="nsew")
        content_host.grid_columnconfigure(0, weight=1)
        content_host.grid_rowconfigure(0, weight=1)
        self.content_host = content_host

        # Lazy: each tab is built on first selection.
        for key, _, _ in self.NAV_ITEMS:
            placeholder = ctk.CTkFrame(content_host, fg_color="transparent")
            self.content_frames[key] = placeholder

    # ------------------------------------------------------------------
    # Activation
    # ------------------------------------------------------------------
    def select(self, key: str) -> None:
        """Activate the named tab. Builds the tab on first use."""
        if key not in self.content_frames:
            return
        # Update nav button visual states
        for k, btn in self.nav_buttons.items():
            if k == key:
                btn.configure(fg_color=COLORS["accent-soft"], text_color=COLORS["accent"])
            else:
                btn.configure(fg_color="transparent", text_color=COLORS["fg-1"])

        # Hide all, show selected
        for k, frame in self.content_frames.items():
            frame.grid_remove()
        target = self.content_frames[key]
        target.grid(row=0, column=0, sticky="nsew", padx=0, pady=0)

        # Lazy-build the tab on first activation
        if key not in self.tabs:
            self._build_tab(key, target)

        self.active_key = key

    def _build_tab(self, key: str, parent: ctk.CTkFrame) -> None:
        """Instantiate the appropriate Studio tab class."""
        # Local imports avoid a circular import at module load time
        if key == "recorder":
            from gui.skins.studio_recorder_tab import StudioRecorderTab

            self.tabs[key] = StudioRecorderTab(self.app, parent)
        elif key == "impulcifer":
            from gui.skins.studio_impulcifer_tab import StudioImpulciferTab

            self.tabs[key] = StudioImpulciferTab(self.app, parent)
        elif key == "settings":
            from gui.skins.studio_settings_tab import StudioSettingsTab

            self.tabs[key] = StudioSettingsTab(self.app, parent)
        elif key == "info":
            from gui.skins.studio_info_tab import StudioInfoTab

            self.tabs[key] = StudioInfoTab(self.app, parent)
