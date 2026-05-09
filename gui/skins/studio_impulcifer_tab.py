#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Studio Impulcifer tab — card-based BRIR generator.

Mirrors the focused option set from the Pulse redesign mockup:

    Card 01  Input files          (recordings dir + test signal)
    Card 02  Processing options   (room / headphone / custom EQ / plot)
    Card 03  Virtual bass         (crossover / sub HP / polarity)

Users who need the full advanced options (resample, target_level, bass
boost, tilt, channel balance, decay, etc.) should switch back to the
Stable skin. By design the Studio variant is the focused experience.
"""
from __future__ import annotations

import os
import shutil
import threading
from typing import TYPE_CHECKING

import customtkinter as ctk

import impulcifer
from gui.constants import FILETYPES_AUDIO_WITH_PKL, FILETYPES_TEXT, FILETYPES_WAV
from gui.dialogs import ProcessingDialog
from gui.skins.studio_widgets import (
    add_card_header,
    add_disclosure,
    add_field_row,
    add_inline_dropdown,
    add_inline_metric,
    make_card,
    make_card_body,
    make_page_header,
)
from gui.theme import COLORS
from gui.utils import (
    browse_directory,
    browse_file,
    install_smooth_scrolling,
    safe_get_double,
    safe_get_int,
    safe_get_string,
)
from infra.logger import get_logger, set_gui_callbacks

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioImpulciferTab:
    """Card-based Impulcifer tab in Studio skin."""

    def __init__(self, app: ModernImpulciferGUI, parent: ctk.CTkBaseClass) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.root = app.root
        self.parent = parent

        # Tk variables — Studio's own, independent of Stable's
        self.dir_path_var = ctk.StringVar(value=os.path.join("data", "my_hrir"))
        self.test_signal_var = ctk.StringVar(
            value=os.path.join("data", "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
        )

        self.do_room_correction_var = ctk.BooleanVar(value=False)
        self.room_target_var = ctk.StringVar()
        self.room_mic_calibration_var = ctk.StringVar()
        self.specific_limit_var = ctk.IntVar(value=20000)
        self.generic_limit_var = ctk.IntVar(value=1000)
        self.fr_combination_var = ctk.StringVar(value="average")

        self.do_headphone_compensation_var = ctk.BooleanVar(value=False)
        self.headphone_compensation_file_var = ctk.StringVar()

        self.do_equalization_var = ctk.BooleanVar(value=False)
        self.eq_file_var = ctk.StringVar(value="eq.csv")
        self.eq_left_file_var = ctk.StringVar(value="eq-left.csv")
        self.eq_right_file_var = ctk.StringVar(value="eq-right.csv")

        self.plot_var = ctk.BooleanVar(value=False)

        self.show_advanced_var = ctk.BooleanVar(value=False)
        self.fs_check_var = ctk.BooleanVar(value=False)
        self.fs_var = ctk.IntVar(value=48000)
        self.target_level_var = ctk.StringVar()
        self.bass_boost_gain_var = ctk.DoubleVar(value=0.0)
        self.bass_boost_fc_var = ctk.IntVar(value=105)
        self.bass_boost_q_var = ctk.DoubleVar(value=0.76)
        self.tilt_var = ctk.DoubleVar(value=0.0)
        self.channel_balance_var = ctk.StringVar(value="none")
        self.channel_balance_db_var = ctk.IntVar(value=0)
        self.decay_var = ctk.StringVar()
        self.decay_per_channel_var = ctk.BooleanVar(value=False)
        self.decay_channel_vars = {
            ch: ctk.StringVar() for ch in ("FL", "FC", "FR", "SL", "SR", "BL", "BR")
        }
        self.pre_response_var = ctk.DoubleVar(value=1.0)
        self.jamesdsp_var = ctk.BooleanVar(value=False)
        self.hangloose_var = ctk.BooleanVar(value=False)
        self.interactive_plots_var = ctk.BooleanVar(value=False)
        self.microphone_deviation_correction_var = ctk.BooleanVar(value=False)
        self.mic_deviation_strength_var = ctk.DoubleVar(value=0.7)
        self.mic_deviation_debug_plots_var = ctk.BooleanVar(value=False)
        self.output_truehd_layouts_var = ctk.BooleanVar(value=False)

        self.vbass_enable_var = ctk.BooleanVar(value=False)
        self.vbass_freq_var = ctk.IntVar(value=250)
        self.vbass_hp_var = ctk.DoubleVar(value=15.0)
        self.vbass_polarity_var = ctk.StringVar(value="auto")

        self.channel_balance_db_entry: ctk.CTkEntry | None = None
        self.decay_entry: ctk.CTkEntry | None = None
        self.decay_channels_frame: ctk.CTkFrame | None = None
        self.mic_deviation_strength_entry: ctk.CTkEntry | None = None
        self.mic_dev_debug_plots_check: ctk.CTkCheckBox | None = None
        self.generate_button: ctk.CTkButton | None = None

        self._build()

    def _build(self) -> None:
        self.parent.grid_columnconfigure(0, weight=1)
        self.parent.grid_rowconfigure(0, weight=1)

        scroll = ctk.CTkScrollableFrame(self.parent, fg_color="transparent")
        scroll.grid(row=0, column=0, sticky="nsew", padx=24, pady=24)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        page_header = make_page_header(
            scroll,
            title=self.loc.get("studio_processing_title"),
            subtitle=self.loc.get("studio_processing_subtitle"),
            fonts=self.fonts,
            cta_label=self.loc.get("studio_brir_generate"),
            cta_command=self.generate_brir,
        )
        page_header.grid(row=0, column=0, sticky="ew", pady=(0, 18))
        # Snapshot the CTA so we can disable/restore during processing
        for child in page_header.winfo_children():
            if isinstance(child, ctk.CTkButton):
                self.generate_button = child
                break

        self._build_input_card(scroll, row=1)
        self._build_options_card(scroll, row=2)
        self._build_advanced_card(scroll, row=3)
        self._build_vbass_card(scroll, row=4)

    # ------------------------------------------------------------------
    # Card 01 — Input files
    # ------------------------------------------------------------------
    def _build_input_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="01", title=self.loc.get("studio_card_input_files"), fonts=self.fonts)
        body = make_card_body(card)

        add_field_row(
            body,
            row=0,
            label=self.loc.get("label_your_recordings"),
            value_var=self.dir_path_var,
            on_change=lambda: browse_directory(self.dir_path_var),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            body,
            row=1,
            label=self.loc.get("label_test_signal"),
            value_var=self.test_signal_var,
            on_change=lambda: browse_file(self.test_signal_var, "open", FILETYPES_AUDIO_WITH_PKL),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )

    # ------------------------------------------------------------------
    # Card 02 — Processing options (disclosures)
    # ------------------------------------------------------------------
    def _build_options_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="02", title=self.loc.get("studio_card_processing_options"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        # 1. Room correction
        _, rc_body = add_disclosure(
            body,
            row=0,
            label=self.loc.get("section_room_correction"),
            desc=self.loc.get("studio_disclosure_room_correction_desc"),
            state_var=self.do_room_correction_var,
            fonts=self.fonts,
        )
        add_field_row(
            rc_body, row=0,
            label=self.loc.get("label_mic_calibration"),
            value_var=self.room_mic_calibration_var,
            on_change=lambda: browse_file(self.room_mic_calibration_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            rc_body, row=1,
            label=self.loc.get("label_target_curve"),
            value_var=self.room_target_var,
            on_change=lambda: browse_file(self.room_target_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        rc_inline = ctk.CTkFrame(rc_body, fg_color="transparent")
        rc_inline.grid(row=2, column=0, columnspan=2, sticky="ew", pady=(6, 0))
        rc_inline.grid_columnconfigure((0, 1, 2), weight=1, uniform="rcm")
        add_inline_metric(
            rc_inline,
            row=0,
            column=0,
            label="Specific limit",
            value_var=self.specific_limit_var,
            unit="Hz",
        )
        add_inline_metric(
            rc_inline,
            row=0,
            column=1,
            label="Generic limit",
            value_var=self.generic_limit_var,
            unit="Hz",
        )
        add_inline_dropdown(
            rc_inline, row=0, column=2,
            label="FR combination",
            value_var=self.fr_combination_var,
            values=("average", "conservative"),
        )

        # 2. Headphone compensation
        _, hp_body = add_disclosure(
            body,
            row=1,
            label=self.loc.get("section_headphone_compensation"),
            desc=self.loc.get("studio_disclosure_headphone_comp_desc"),
            state_var=self.do_headphone_compensation_var,
            fonts=self.fonts,
        )
        add_field_row(
            hp_body, row=0,
            label=self.loc.get("label_headphone_file"),
            value_var=self.headphone_compensation_file_var,
            on_change=lambda: browse_file(self.headphone_compensation_file_var, "open", FILETYPES_WAV),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )

        # 3. Custom EQ
        _, eq_body = add_disclosure(
            body,
            row=2,
            label=self.loc.get("section_equalization"),
            desc=self.loc.get("studio_disclosure_custom_eq_desc"),
            state_var=self.do_equalization_var,
            fonts=self.fonts,
        )
        add_field_row(
            eq_body, row=0,
            label="eq.csv",
            value_var=self.eq_file_var,
            on_change=lambda: browse_file(self.eq_file_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            eq_body, row=1,
            label="eq-left.csv",
            value_var=self.eq_left_file_var,
            on_change=lambda: browse_file(self.eq_left_file_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            eq_body, row=2,
            label="eq-right.csv",
            value_var=self.eq_right_file_var,
            on_change=lambda: browse_file(self.eq_right_file_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )

        # 4. Plot toggle (no disclosure body)
        plot_box = ctk.CTkFrame(
            body,
            corner_radius=4,
            fg_color=COLORS["bg-1"],
            border_width=1,
            border_color=COLORS["line-soft"],
        )
        plot_box.grid(row=3, column=0, sticky="ew", pady=4)
        plot_box.grid_columnconfigure(1, weight=1)
        ctk.CTkSwitch(
            plot_box,
            text="",
            variable=self.plot_var,
            width=40,
            switch_width=30,
            switch_height=18,
        ).grid(row=0, column=0, sticky="w", padx=12, pady=10)
        plot_text = ctk.CTkFrame(plot_box, fg_color="transparent")
        plot_text.grid(row=0, column=1, sticky="w", padx=(10, 0))
        ctk.CTkLabel(
            plot_text,
            text=self.loc.get("checkbox_plot_results"),
            font=ctk.CTkFont(size=12, weight="bold"),
            anchor="w",
        ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(
            plot_text,
            text=self.loc.get("studio_toggle_plot_desc"),
            font=ctk.CTkFont(size=12),
            text_color=COLORS["fg-2"],
            anchor="w",
        ).grid(row=1, column=0, sticky="w")

    # ------------------------------------------------------------------
    # Card 03 — Advanced options
    # ------------------------------------------------------------------
    def _build_advanced_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="03", title=self.loc.get("section_advanced_options"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        _, adv_body = add_disclosure(
            body,
            row=0,
            label=self.loc.get("section_advanced_options"),
            desc=self.loc.get("studio_disclosure_advanced_desc"),
            state_var=self.show_advanced_var,
            fonts=self.fonts,
        )
        adv_body.grid_columnconfigure(0, weight=1)

        # Resampling
        fs_row = ctk.CTkFrame(adv_body, fg_color="transparent")
        fs_row.grid(row=0, column=0, sticky="ew", pady=(0, 8))
        fs_row.grid_columnconfigure(1, weight=1)
        ctk.CTkCheckBox(
            fs_row,
            text=self.loc.get("checkbox_resample_to"),
            variable=self.fs_check_var,
            font=ctk.CTkFont(size=13, weight="bold"),
        ).grid(row=0, column=0, sticky="w", padx=(0, 12), pady=4)
        ctk.CTkOptionMenu(
            fs_row,
            variable=self.fs_var,
            values=["44100", "48000", "88200", "96000", "176400", "192000", "352000", "384000"],
            width=160,
        ).grid(row=0, column=1, sticky="w", pady=4)

        tonal_grid = ctk.CTkFrame(adv_body, fg_color="transparent")
        tonal_grid.grid(row=1, column=0, sticky="ew", pady=(0, 4))
        tonal_grid.grid_columnconfigure((0, 1, 2), weight=1, uniform="adv-tonal")
        add_inline_metric(
            tonal_grid,
            row=0,
            column=0,
            label=self.loc.get("label_target_level"),
            value_var=self.target_level_var,
            unit="dB",
        )
        add_inline_metric(
            tonal_grid,
            row=0,
            column=1,
            label=self.loc.get("label_tilt"),
            value_var=self.tilt_var,
        )
        add_inline_metric(
            tonal_grid,
            row=0,
            column=2,
            label=self.loc.get("label_pre_response"),
            value_var=self.pre_response_var,
            unit="ms",
        )

        bass_grid = ctk.CTkFrame(adv_body, fg_color="transparent")
        bass_grid.grid(row=2, column=0, sticky="ew", pady=(0, 4))
        bass_grid.grid_columnconfigure((0, 1, 2), weight=1, uniform="adv-bass")
        add_inline_metric(
            bass_grid,
            row=0,
            column=0,
            label=f"{self.loc.get('label_bass_boost')} {self.loc.get('label_gain_db')}",
            value_var=self.bass_boost_gain_var,
            unit="dB",
        )
        add_inline_metric(
            bass_grid,
            row=0,
            column=1,
            label=self.loc.get("label_fc"),
            value_var=self.bass_boost_fc_var,
            unit="Hz",
        )
        add_inline_metric(
            bass_grid,
            row=0,
            column=2,
            label=self.loc.get("label_q"),
            value_var=self.bass_boost_q_var,
        )

        balance_row = self._make_advanced_line(adv_body, row=3, label=self.loc.get("label_balance"))
        ctk.CTkOptionMenu(
            balance_row,
            variable=self.channel_balance_var,
            values=["none", "trend", "mids", "avg", "min", "left", "right", "number"],
            command=lambda _: self.update_balance_entry(),
            width=170,
        ).grid(row=0, column=1, sticky="w", pady=4)
        ctk.CTkLabel(
            balance_row,
            text=self.loc.get("label_balance_db"),
            font=ctk.CTkFont(size=12),
            text_color=COLORS["fg-2"],
        ).grid(row=0, column=2, sticky="e", padx=(16, 4), pady=4)
        self.channel_balance_db_entry = ctk.CTkEntry(
            balance_row,
            textvariable=self.channel_balance_db_var,
            width=80,
            state="disabled",
        )
        self.channel_balance_db_entry.grid(row=0, column=3, sticky="w", pady=4)

        decay_row = self._make_advanced_line(adv_body, row=4, label=self.loc.get("label_decay"))
        self.decay_entry = ctk.CTkEntry(decay_row, textvariable=self.decay_var, width=140)
        self.decay_entry.grid(row=0, column=1, sticky="w", pady=4)
        ctk.CTkCheckBox(
            decay_row,
            text=self.loc.get("checkbox_per_channel"),
            variable=self.decay_per_channel_var,
            command=self.toggle_decay_per_channel,
        ).grid(row=0, column=2, columnspan=2, sticky="w", padx=(16, 0), pady=4)

        self.decay_channels_frame = ctk.CTkFrame(adv_body, fg_color="transparent")
        self.decay_channels_frame.grid(row=5, column=0, sticky="ew", padx=(18, 0), pady=(0, 8))
        for idx, (ch, var) in enumerate(self.decay_channel_vars.items()):
            ctk.CTkLabel(
                self.decay_channels_frame,
                text=f"{ch}:",
                font=ctk.CTkFont(size=12, weight="bold"),
                text_color=COLORS["fg-1"],
            ).grid(row=0, column=idx * 2, sticky="w", padx=(0, 4), pady=4)
            ctk.CTkEntry(
                self.decay_channels_frame,
                textvariable=var,
                width=54,
            ).grid(row=0, column=idx * 2 + 1, sticky="w", padx=(0, 8), pady=4)
        self.decay_channels_frame.grid_remove()

        output_row = ctk.CTkFrame(adv_body, fg_color="transparent")
        output_row.grid(row=6, column=0, sticky="ew", pady=4)
        for idx, (label, var) in enumerate(
            (
                (self.loc.get("checkbox_jamesdsp"), self.jamesdsp_var),
                (self.loc.get("checkbox_hangloose"), self.hangloose_var),
                (self.loc.get("checkbox_interactive_plots"), self.interactive_plots_var),
                (self.loc.get("checkbox_truehd_layouts"), self.output_truehd_layouts_var),
            )
        ):
            ctk.CTkCheckBox(output_row, text=label, variable=var).grid(
                row=0,
                column=idx,
                sticky="w",
                padx=(0, 14),
                pady=4,
            )

        mic_row = self._make_advanced_line(
            adv_body,
            row=7,
            label=self.loc.get("checkbox_enable_mic_deviation"),
        )
        ctk.CTkCheckBox(
            mic_row,
            text="",
            variable=self.microphone_deviation_correction_var,
            command=self.toggle_mic_deviation,
            width=28,
        ).grid(row=0, column=1, sticky="w", pady=4)
        ctk.CTkLabel(
            mic_row,
            text=self.loc.get("label_strength"),
            font=ctk.CTkFont(size=12),
            text_color=COLORS["fg-2"],
        ).grid(row=0, column=2, sticky="e", padx=(16, 4), pady=4)
        self.mic_deviation_strength_entry = ctk.CTkEntry(
            mic_row,
            textvariable=self.mic_deviation_strength_var,
            width=80,
            state="disabled",
        )
        self.mic_deviation_strength_entry.grid(row=0, column=3, sticky="w", pady=4)
        self.mic_dev_debug_plots_check = ctk.CTkCheckBox(
            mic_row,
            text=self.loc.get("checkbox_mic_deviation_debug_plots"),
            variable=self.mic_deviation_debug_plots_var,
            state="disabled",
        )
        self.mic_dev_debug_plots_check.grid(row=0, column=4, sticky="w", padx=(16, 0), pady=4)

    def _make_advanced_line(
        self,
        parent: ctk.CTkBaseClass,
        *,
        row: int,
        label: str,
    ) -> ctk.CTkFrame:
        line = ctk.CTkFrame(parent, fg_color="transparent")
        line.grid(row=row, column=0, sticky="ew", pady=4)
        line.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(
            line,
            text=label,
            font=ctk.CTkFont(size=13, weight="bold"),
            text_color=COLORS["fg-1"],
            anchor="w",
            width=140,
        ).grid(row=0, column=0, sticky="w", padx=(0, 16), pady=4)
        return line

    def update_balance_entry(self) -> None:
        if self.channel_balance_db_entry is None:
            return
        state = "normal" if self.channel_balance_var.get() == "number" else "disabled"
        self.channel_balance_db_entry.configure(state=state)

    def toggle_decay_per_channel(self) -> None:
        if self.decay_entry is None or self.decay_channels_frame is None:
            return
        if self.decay_per_channel_var.get():
            self.decay_entry.configure(state="disabled")
            self.decay_channels_frame.grid()
        else:
            self.decay_entry.configure(state="normal")
            self.decay_channels_frame.grid_remove()

    def toggle_mic_deviation(self) -> None:
        if self.mic_deviation_strength_entry is None or self.mic_dev_debug_plots_check is None:
            return
        state = "normal" if self.microphone_deviation_correction_var.get() else "disabled"
        self.mic_deviation_strength_entry.configure(state=state)
        self.mic_dev_debug_plots_check.configure(state=state)

    # ------------------------------------------------------------------
    # Card 04 — Virtual bass
    # ------------------------------------------------------------------
    def _build_vbass_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="04", title=self.loc.get("studio_card_virtual_bass"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        _, vb_body = add_disclosure(
            body,
            row=0,
            label=self.loc.get("vbass_enable"),
            desc=self.loc.get("studio_disclosure_virtual_bass_desc"),
            state_var=self.vbass_enable_var,
            fonts=self.fonts,
        )
        vb_inline = ctk.CTkFrame(vb_body, fg_color="transparent")
        vb_inline.grid(row=0, column=0, sticky="ew")
        vb_inline.grid_columnconfigure((0, 1, 2), weight=1, uniform="vbm")
        add_inline_metric(vb_inline, row=0, column=0, label="Crossover", value_var=self.vbass_freq_var, unit="Hz")
        add_inline_metric(vb_inline, row=0, column=1, label="Sub HP", value_var=self.vbass_hp_var, unit="Hz")
        add_inline_dropdown(
            vb_inline, row=0, column=2,
            label="Polarity",
            value_var=self.vbass_polarity_var,
            values=("auto", "normal", "invert"),
        )

    # ------------------------------------------------------------------
    # Generate
    # ------------------------------------------------------------------
    def generate_brir(self) -> None:
        """Run :func:`impulcifer.main` in a worker thread with progress dialog."""
        args = {
            "dir_path": self.dir_path_var.get(),
            "test_signal": self.test_signal_var.get(),
            "plot": self.plot_var.get(),
            "do_room_correction": self.do_room_correction_var.get(),
            "do_headphone_compensation": self.do_headphone_compensation_var.get(),
            "do_equalization": self.do_equalization_var.get(),
        }

        if self.do_room_correction_var.get():
            args["room_target"] = self.room_target_var.get() or None
            args["room_mic_calibration"] = self.room_mic_calibration_var.get() or None
            args["specific_limit"] = safe_get_int(self.specific_limit_var, 20000)
            args["generic_limit"] = safe_get_int(self.generic_limit_var, 1000)
            args["fr_combination_method"] = self.fr_combination_var.get()

        if self.do_headphone_compensation_var.get() and self.headphone_compensation_file_var.get():
            source_file = self.headphone_compensation_file_var.get()
            if not os.path.isabs(source_file):
                source_file = os.path.join(self.dir_path_var.get(), source_file)
            target_file = os.path.join(self.dir_path_var.get(), "headphones.wav")
            if os.path.exists(source_file):
                try:
                    shutil.copy2(source_file, target_file)
                except Exception as e:
                    print(f"Error copying headphone file: {e}")

        if self.show_advanced_var.get():
            args["fs"] = safe_get_int(self.fs_var, 48000) if self.fs_check_var.get() else None

            target_level_str = safe_get_string(self.target_level_var, "")
            if target_level_str.strip():
                try:
                    args["target_level"] = float(target_level_str)
                except ValueError:
                    args["target_level"] = None
            else:
                args["target_level"] = None

            if self.channel_balance_var.get() == "number":
                args["channel_balance"] = safe_get_int(self.channel_balance_db_var, 0)
            elif self.channel_balance_var.get() != "none":
                args["channel_balance"] = self.channel_balance_var.get()

            bass_gain = safe_get_double(self.bass_boost_gain_var, 0.0)
            if bass_gain:
                args["bass_boost_gain"] = bass_gain
                args["bass_boost_fc"] = safe_get_int(self.bass_boost_fc_var, 105)
                args["bass_boost_q"] = safe_get_double(self.bass_boost_q_var, 0.76)

            tilt_val = safe_get_double(self.tilt_var, 0.0)
            if tilt_val:
                args["tilt"] = tilt_val

            if self.decay_per_channel_var.get():
                decay_dict = {}
                for ch, var in self.decay_channel_vars.items():
                    val_str = safe_get_string(var, "")
                    if val_str.strip():
                        try:
                            decay_dict[ch] = float(val_str) / 1000
                        except ValueError:
                            pass
                if decay_dict:
                    args["decay"] = decay_dict
            else:
                decay_str = safe_get_string(self.decay_var, "")
                if decay_str.strip():
                    try:
                        decay_val = float(decay_str) / 1000
                        args["decay"] = {
                            ch: decay_val for ch in ("FL", "FC", "FR", "SL", "SR", "BL", "BR")
                        }
                    except ValueError:
                        pass

            args["head_ms"] = safe_get_double(self.pre_response_var, 1.0)
            args["jamesdsp"] = self.jamesdsp_var.get()
            args["hangloose"] = self.hangloose_var.get()
            args["interactive_plots"] = self.interactive_plots_var.get()
            args["microphone_deviation_correction"] = self.microphone_deviation_correction_var.get()
            args["mic_deviation_strength"] = safe_get_double(self.mic_deviation_strength_var, 0.7)
            args["mic_deviation_phase_correction"] = True
            args["mic_deviation_adaptive_correction"] = True
            args["mic_deviation_anatomical_validation"] = True
            args["mic_deviation_debug_plots"] = self.mic_deviation_debug_plots_var.get()
            args["output_truehd_layouts"] = self.output_truehd_layouts_var.get()

        if self.vbass_enable_var.get():
            args["vbass"] = True
            args["vbass_freq"] = max(30, min(500, safe_get_int(self.vbass_freq_var, 250)))
            args["vbass_hp"] = safe_get_double(self.vbass_hp_var, 15.0)
            args["vbass_polarity"] = self.vbass_polarity_var.get() or "auto"

        # Disable CTA during processing
        if self.generate_button:
            self.generate_button.configure(state="disabled", text=self.loc.get("button_processing"))

        dialog = ProcessingDialog(self.root, self.loc, fonts=self.fonts)
        logger = get_logger()
        logger.set_localization(self.loc)
        set_gui_callbacks(log_callback=dialog.add_log, progress_callback=dialog.update_progress)

        def _run() -> None:
            try:
                with impulcifer.cancellation_scope(dialog.cancel_event):
                    impulcifer.main(**args)
                dialog.mark_complete(success=True)
            except impulcifer.CancelledError:
                logger.warning("message_processing_cancelled")
                dialog.mark_cancelled()
            except Exception as e:
                logger.error(f"Processing failed: {e}")
                dialog.mark_complete(success=False)
            finally:
                set_gui_callbacks(log_callback=None, progress_callback=None)
                self.root.after(0, self._restore_cta)

        threading.Thread(target=_run, daemon=True).start()

    def _restore_cta(self) -> None:
        if self.generate_button:
            self.generate_button.configure(
                state="normal",
                text=self.loc.get("studio_brir_generate"),
            )
