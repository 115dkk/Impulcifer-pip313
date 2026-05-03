#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Impulcifer (BRIR generation) tab for the modern GUI.

Hosts inputs/outputs, processing options, advanced options, virtual bass
controls, and the Generate BRIR button. Moved from ``gui/modern_gui.py``
without behavioural changes — the ``generate_brir`` argument-assembly
logic is preserved verbatim.
"""

from __future__ import annotations

import os
import shutil
import threading
from typing import TYPE_CHECKING

import customtkinter as ctk

import impulcifer
from gui.constants import (
    FILETYPES_AUDIO_WITH_PKL,
    FILETYPES_TEXT,
    FILETYPES_WAV,
    WIDGET_BUTTON_WIDTH_BROWSE,
    WIDGET_ENTRY_WIDTH_DEFAULT,
    WIDGET_ENTRY_WIDTH_NARROW,
    WIDGET_ENTRY_WIDTH_TINY,
    WIDGET_OPTION_WIDTH_DEFAULT,
    WIDGET_OPTION_WIDTH_NARROW,
)
from gui.dialogs import ProcessingDialog
from gui.utils import (
    browse_directory,
    browse_file,
    restore_tk_vars,
    safe_get_double,
    safe_get_int,
    safe_get_string,
    snapshot_tk_vars,
)
from infra.logger import get_logger, set_gui_callbacks

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class ImpulciferTab:
    """Build and handle the BRIR generation tab."""

    def __init__(self, app: ModernImpulciferGUI) -> None:
        """Create the BRIR generation tab.

        Args:
            app: Top-level GUI application.
        """
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.tabview = app.tabview
        self.root = app.root
        self._build()

    def _build(self) -> None:
        """Create Impulcifer tab with all processing features."""
        tab = self.tabview.tab(self.loc.get('tab_impulcifer'))
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        # Create scrollable frame
        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)

        row = 0

        # === Input Files Section ===
        input_frame = ctk.CTkFrame(scroll, corner_radius=0)
        input_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        input_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            input_frame,
            text=self.loc.get('section_input_files'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, columnspan=3, sticky="w", padx=15, pady=(15, 10))

        # Your recordings
        ctk.CTkLabel(input_frame, text=self.loc.get('label_your_recordings')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.dir_path_var = ctk.StringVar(value=os.path.join('data', 'my_hrir'))
        self.dir_path_entry = ctk.CTkEntry(input_frame, textvariable=self.dir_path_var)
        self.dir_path_entry.grid(row=1, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            input_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_directory(self.dir_path_var),
            width=WIDGET_BUTTON_WIDTH_BROWSE,
        ).grid(row=1, column=2, padx=15, pady=5)

        # Test signal
        ctk.CTkLabel(input_frame, text=self.loc.get('label_test_signal')).grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.test_signal_var = ctk.StringVar(value=os.path.join('data', 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.test_signal_entry = ctk.CTkEntry(input_frame, textvariable=self.test_signal_var)
        self.test_signal_entry.grid(row=2, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            input_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.test_signal_var, 'open', FILETYPES_AUDIO_WITH_PKL),
            width=WIDGET_BUTTON_WIDTH_BROWSE,
        ).grid(row=2, column=2, padx=(15, 15), pady=(5, 15))

        # === Processing Options Section ===
        processing_frame = ctk.CTkFrame(scroll, corner_radius=0)
        processing_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        processing_frame.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            processing_frame,
            text=self.loc.get('section_processing_options'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        proc_row = 1

        # Room Correction
        self.do_room_correction_var = ctk.BooleanVar(value=False)
        self.room_correction_check = ctk.CTkCheckBox(
            processing_frame,
            text=self.loc.get('section_room_correction'),
            variable=self.do_room_correction_var,
            command=self.toggle_room_correction
        )
        self.room_correction_check.grid(row=proc_row, column=0, sticky="w", padx=15, pady=5)
        proc_row += 1
        # Reserve next row for room options frame (revealed by toggle)
        self._room_options_row = proc_row
        proc_row += 1

        # Room correction options (initially hidden)
        self.room_options_frame = ctk.CTkFrame(processing_frame, fg_color="transparent")

        room_opt_row = 0
        # Specific Limit
        limits_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        limits_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        room_opt_row += 1

        ctk.CTkLabel(limits_frame, text=self.loc.get('label_specific_limit')).pack(side="left", padx=5)
        self.specific_limit_var = ctk.IntVar(value=20000)
        ctk.CTkEntry(limits_frame, textvariable=self.specific_limit_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).pack(side="left", padx=5)

        ctk.CTkLabel(limits_frame, text=self.loc.get('label_generic_limit')).pack(side="left", padx=(20, 5))
        self.generic_limit_var = ctk.IntVar(value=1000)
        ctk.CTkEntry(limits_frame, textvariable=self.generic_limit_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).pack(side="left", padx=5)

        # FR combination method
        fr_method_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        fr_method_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        room_opt_row += 1

        ctk.CTkLabel(fr_method_frame, text=self.loc.get('label_fr_combination')).pack(side="left", padx=5)
        self.fr_combination_var = ctk.StringVar(value="average")
        ctk.CTkOptionMenu(
            fr_method_frame,
            variable=self.fr_combination_var,
            values=["average", "conservative"],
            width=WIDGET_OPTION_WIDTH_DEFAULT,
        ).pack(side="left", padx=5)

        # Mic calibration
        mic_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        mic_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        mic_frame.grid_columnconfigure(1, weight=1)
        room_opt_row += 1

        ctk.CTkLabel(mic_frame, text=self.loc.get('label_mic_calibration')).grid(row=0, column=0, sticky="w", padx=5, pady=2)
        self.room_mic_calibration_var = ctk.StringVar()
        ctk.CTkEntry(mic_frame, textvariable=self.room_mic_calibration_var).grid(row=0, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            mic_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.room_mic_calibration_var, 'open', FILETYPES_TEXT),
            width=WIDGET_ENTRY_WIDTH_DEFAULT,
        ).grid(row=0, column=2, padx=5, pady=2)

        # Room target
        ctk.CTkLabel(mic_frame, text=self.loc.get('label_target_curve')).grid(row=1, column=0, sticky="w", padx=5, pady=2)
        self.room_target_var = ctk.StringVar()
        ctk.CTkEntry(mic_frame, textvariable=self.room_target_var).grid(row=1, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            mic_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.room_target_var, 'open', FILETYPES_TEXT),
            width=WIDGET_ENTRY_WIDTH_DEFAULT,
        ).grid(row=1, column=2, padx=5, pady=2)

        # Headphone Compensation
        self.do_headphone_compensation_var = ctk.BooleanVar(value=False)
        self.headphone_check = ctk.CTkCheckBox(
            processing_frame,
            text=self.loc.get('section_headphone_compensation'),
            variable=self.do_headphone_compensation_var,
            command=self.toggle_headphone_compensation
        )
        self.headphone_check.grid(row=proc_row, column=0, sticky="w", padx=15, pady=5)
        proc_row += 1
        # Reserve next row for headphone options frame (revealed by toggle)
        self._headphone_options_row = proc_row
        proc_row += 1

        # Headphone compensation options (initially hidden)
        self.headphone_options_frame = ctk.CTkFrame(processing_frame, fg_color="transparent")

        hp_frame = ctk.CTkFrame(self.headphone_options_frame, fg_color="transparent")
        hp_frame.grid(row=0, column=0, sticky="ew", padx=30, pady=5)
        hp_frame.grid_columnconfigure(1, weight=1)

        ctk.CTkLabel(hp_frame, text=self.loc.get('label_headphone_file')).grid(row=0, column=0, sticky="w", padx=5, pady=2)
        self.headphone_compensation_file_var = ctk.StringVar()
        ctk.CTkEntry(hp_frame, textvariable=self.headphone_compensation_file_var).grid(row=0, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            hp_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.headphone_compensation_file_var, 'open', FILETYPES_WAV),
            width=WIDGET_ENTRY_WIDTH_DEFAULT,
        ).grid(row=0, column=2, padx=5, pady=2)

        # Custom EQ
        self.do_equalization_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            processing_frame,
            text=self.loc.get('checkbox_custom_eq'),
            variable=self.do_equalization_var
        ).grid(row=proc_row, column=0, sticky="w", padx=15, pady=5)
        proc_row += 1

        # Plot results
        self.plot_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            processing_frame,
            text=self.loc.get('checkbox_plot_results'),
            variable=self.plot_var
        ).grid(row=proc_row, column=0, sticky="w", padx=15, pady=(5, 15))
        proc_row += 1

        # === Advanced Options Section ===
        advanced_frame = ctk.CTkFrame(scroll, corner_radius=0)
        advanced_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        advanced_frame.grid_columnconfigure(0, weight=1)
        row += 1

        self.show_advanced_var = ctk.BooleanVar(value=False)
        advanced_toggle = ctk.CTkCheckBox(
            advanced_frame,
            text=self.loc.get('section_advanced_options'),
            variable=self.show_advanced_var,
            command=self.toggle_advanced_options,
            font=self.fonts['heading']
        )
        advanced_toggle.grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        # Advanced options container (initially hidden)
        self.advanced_options_frame = ctk.CTkFrame(advanced_frame, fg_color="transparent")

        adv_row = 0

        # Resample
        resample_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        resample_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.fs_check_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(resample_frame, text=self.loc.get('checkbox_resample_to'), variable=self.fs_check_var).pack(side="left", padx=5)
        self.fs_var = ctk.IntVar(value=48000)
        ctk.CTkOptionMenu(
            resample_frame,
            variable=self.fs_var,
            values=["44100", "48000", "88200", "96000", "176400", "192000", "352000", "384000"],
            width=WIDGET_OPTION_WIDTH_NARROW,
        ).pack(side="left", padx=5)

        # Target level
        target_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        target_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(target_frame, text=self.loc.get('label_target_level')).pack(side="left", padx=5)
        self.target_level_var = ctk.StringVar()
        ctk.CTkEntry(target_frame, textvariable=self.target_level_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).pack(side="left", padx=5)

        # Bass boost
        bass_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        bass_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(bass_frame, text=self.loc.get('label_bass_boost')).pack(side="left", padx=5)
        ctk.CTkLabel(bass_frame, text=self.loc.get('label_gain_db')).pack(side="left", padx=(10, 2))
        self.bass_boost_gain_var = ctk.DoubleVar()
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_gain_var, width=WIDGET_ENTRY_WIDTH_NARROW).pack(side="left", padx=2)

        ctk.CTkLabel(bass_frame, text=self.loc.get('label_fc')).pack(side="left", padx=(10, 2))
        self.bass_boost_fc_var = ctk.IntVar(value=105)
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_fc_var, width=WIDGET_ENTRY_WIDTH_NARROW).pack(side="left", padx=2)

        ctk.CTkLabel(bass_frame, text=self.loc.get('label_q')).pack(side="left", padx=(10, 2))
        self.bass_boost_q_var = ctk.DoubleVar(value=0.76)
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_q_var, width=WIDGET_ENTRY_WIDTH_NARROW).pack(side="left", padx=2)

        # Tilt
        tilt_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        tilt_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(tilt_frame, text=self.loc.get('label_tilt')).pack(side="left", padx=5)
        self.tilt_var = ctk.DoubleVar()
        ctk.CTkEntry(tilt_frame, textvariable=self.tilt_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).pack(side="left", padx=5)

        # Channel Balance
        balance_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        balance_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(balance_frame, text=self.loc.get('label_balance')).pack(side="left", padx=5)
        self.channel_balance_var = ctk.StringVar(value="none")
        self.channel_balance_menu = ctk.CTkOptionMenu(
            balance_frame,
            variable=self.channel_balance_var,
            values=["none", "trend", "mids", "avg", "min", "left", "right", "number"],
            width=WIDGET_OPTION_WIDTH_NARROW,
            command=self.update_balance_entry
        )
        self.channel_balance_menu.pack(side="left", padx=5)

        ctk.CTkLabel(balance_frame, text=self.loc.get('label_balance_db')).pack(side="left", padx=(10, 2))
        self.channel_balance_db_var = ctk.IntVar(value=0)
        self.channel_balance_db_entry = ctk.CTkEntry(balance_frame, textvariable=self.channel_balance_db_var, width=WIDGET_ENTRY_WIDTH_NARROW, state="disabled")
        self.channel_balance_db_entry.pack(side="left", padx=2)

        # Decay
        decay_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        decay_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(decay_frame, text=self.loc.get('label_decay')).pack(side="left", padx=5)
        self.decay_var = ctk.StringVar()
        self.decay_entry = ctk.CTkEntry(decay_frame, textvariable=self.decay_var, width=WIDGET_ENTRY_WIDTH_DEFAULT)
        self.decay_entry.pack(side="left", padx=5)

        self.decay_per_channel_var = ctk.BooleanVar(value=False)
        self.decay_per_channel_check = ctk.CTkCheckBox(
            decay_frame,
            text=self.loc.get('checkbox_per_channel'),
            variable=self.decay_per_channel_var,
            command=self.toggle_decay_per_channel
        )
        self.decay_per_channel_check.pack(side="left", padx=10)

        # Reserve next row for per-channel decay frame (revealed by toggle)
        self._decay_channels_row = adv_row
        adv_row += 1

        # Per-channel decay (initially hidden)
        self.decay_channels_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")

        decay_ch_subframe = ctk.CTkFrame(self.decay_channels_frame, fg_color="transparent")
        decay_ch_subframe.grid(row=0, column=0, sticky="ew", padx=30, pady=5)

        self.decay_channel_vars = {}
        for i, ch in enumerate(['FL', 'FC', 'FR', 'SL', 'SR', 'BL', 'BR']):
            ctk.CTkLabel(decay_ch_subframe, text=f"{ch}:").pack(side="left", padx=2)
            var = ctk.StringVar()
            self.decay_channel_vars[ch] = var
            ctk.CTkEntry(decay_ch_subframe, textvariable=var, width=WIDGET_ENTRY_WIDTH_TINY).pack(side="left", padx=2)

        # Pre-response
        pre_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        pre_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(pre_frame, text=self.loc.get('label_pre_response')).pack(side="left", padx=5)
        self.pre_response_var = ctk.DoubleVar(value=1.0)
        ctk.CTkEntry(pre_frame, textvariable=self.pre_response_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).pack(side="left", padx=5)

        # Output options
        output_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        output_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.jamesdsp_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(output_frame, text=self.loc.get('checkbox_jamesdsp'), variable=self.jamesdsp_var).pack(side="left", padx=5)

        self.hangloose_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(output_frame, text=self.loc.get('checkbox_hangloose'), variable=self.hangloose_var).pack(side="left", padx=10)

        self.interactive_plots_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(output_frame, text=self.loc.get('checkbox_interactive_plots'), variable=self.interactive_plots_var).pack(side="left", padx=10)

        # Mic deviation correction
        mic_dev_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        mic_dev_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.microphone_deviation_correction_var = ctk.BooleanVar(value=False)
        self.mic_dev_check = ctk.CTkCheckBox(
            mic_dev_frame,
            text=self.loc.get('checkbox_enable_mic_deviation'),
            variable=self.microphone_deviation_correction_var,
            command=self.toggle_mic_deviation
        )
        self.mic_dev_check.pack(side="left", padx=5)

        ctk.CTkLabel(mic_dev_frame, text=self.loc.get('label_strength')).pack(side="left", padx=(10, 2))
        self.mic_deviation_strength_var = ctk.DoubleVar(value=0.7)
        self.mic_deviation_strength_entry = ctk.CTkEntry(mic_dev_frame, textvariable=self.mic_deviation_strength_var, width=WIDGET_ENTRY_WIDTH_NARROW, state="disabled")
        self.mic_deviation_strength_entry.pack(side="left", padx=2)

        # Mic deviation v3.0 options (debug plots only - phase/adaptive/anatomical removed in v3.0)
        self.mic_deviation_debug_plots_var = ctk.BooleanVar(value=False)
        self.mic_dev_debug_plots_check = ctk.CTkCheckBox(
            mic_dev_frame,
            text=self.loc.get('checkbox_mic_deviation_debug_plots'),
            variable=self.mic_deviation_debug_plots_var,
            state="disabled"
        )
        self.mic_dev_debug_plots_check.pack(side="left", padx=10)

        # TrueHD layouts
        truehd_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        truehd_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.output_truehd_layouts_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            truehd_frame,
            text=self.loc.get('checkbox_truehd_layouts'),
            variable=self.output_truehd_layouts_var
        ).pack(side="left", padx=5)

        # === Virtual Bass Section ===
        vbass_group = ctk.CTkFrame(scroll, corner_radius=0)
        vbass_group.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        vbass_group.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            vbass_group,
            text=self.loc.get('vbass_group_title'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        vbass_row = 1

        # Enable toggle
        vbass_enable_frame = ctk.CTkFrame(vbass_group, fg_color="transparent")
        vbass_enable_frame.grid(row=vbass_row, column=0, sticky="ew", padx=15, pady=5)
        vbass_row += 1

        self.vbass_enable_var = ctk.BooleanVar(value=False)
        self.vbass_enable_check = ctk.CTkCheckBox(
            vbass_enable_frame,
            text=self.loc.get('vbass_enable'),
            variable=self.vbass_enable_var,
            command=self.toggle_vbass
        )
        self.vbass_enable_check.pack(side="left", padx=5)

        # Virtual Bass options container
        self.vbass_options_frame = ctk.CTkFrame(vbass_group, fg_color="transparent")

        vbopt_row = 0

        # Crossover frequency
        xo_frame = ctk.CTkFrame(self.vbass_options_frame, fg_color="transparent")
        xo_frame.grid(row=vbopt_row, column=0, sticky="ew", padx=30, pady=5)
        vbopt_row += 1

        ctk.CTkLabel(xo_frame, text=self.loc.get('vbass_crossover_freq')).pack(side="left", padx=5)
        self.vbass_freq_var = ctk.IntVar(value=250)
        self.vbass_freq_spin = ctk.CTkEntry(xo_frame, textvariable=self.vbass_freq_var, width=WIDGET_ENTRY_WIDTH_DEFAULT)
        self.vbass_freq_spin.pack(side="left", padx=5)

        # Sub-bass high-pass
        hp_frame = ctk.CTkFrame(self.vbass_options_frame, fg_color="transparent")
        hp_frame.grid(row=vbopt_row, column=0, sticky="ew", padx=30, pady=5)
        vbopt_row += 1

        ctk.CTkLabel(hp_frame, text=self.loc.get('vbass_hp_freq')).pack(side="left", padx=5)
        self.vbass_hp_var = ctk.DoubleVar(value=15.0)
        self.vbass_hp_entry = ctk.CTkEntry(hp_frame, textvariable=self.vbass_hp_var, width=WIDGET_ENTRY_WIDTH_DEFAULT)
        self.vbass_hp_entry.pack(side="left", padx=5)

        # Polarity handling
        pol_frame = ctk.CTkFrame(self.vbass_options_frame, fg_color="transparent")
        pol_frame.grid(row=vbopt_row, column=0, sticky="ew", padx=30, pady=(5, 15))
        vbopt_row += 1

        ctk.CTkLabel(pol_frame, text=self.loc.get('vbass_polarity')).pack(side="left", padx=5)
        self.vbass_polarity_var = ctk.StringVar(value=self.loc.get('vbass_polarity_auto'))
        self.vbass_polarity_menu = ctk.CTkOptionMenu(
            pol_frame,
            variable=self.vbass_polarity_var,
            values=[
                self.loc.get('vbass_polarity_auto'),
                self.loc.get('vbass_polarity_normal'),
                self.loc.get('vbass_polarity_invert'),
            ],
            width=WIDGET_OPTION_WIDTH_DEFAULT,
        )
        self.vbass_polarity_menu.pack(side="left", padx=5)

        # === Generate Button ===
        self.generate_button = ctk.CTkButton(
            scroll,
            text=self.loc.get('button_generate_brir'),
            command=self.generate_brir,
            height=50,
            font=self.fonts['heading'],
            fg_color="#28a745",
            hover_color="#218838"
        )
        self.generate_button.grid(row=row, column=0, sticky="ew", padx=10, pady=20)

    def get_state(self) -> dict:
        """Return a snapshot of user-editable Tk variables."""
        return snapshot_tk_vars(self)

    def apply_state(self, state: dict) -> None:
        """Restore user-editable Tk variables after a UI rebuild."""
        restore_tk_vars(self, state)
        self.toggle_room_correction()
        self.toggle_headphone_compensation()
        self.toggle_advanced_options()
        self.update_balance_entry()
        self.toggle_decay_per_channel()
        self.toggle_vbass()
        self.toggle_mic_deviation()

    def toggle_room_correction(self) -> None:
        """Show or hide room correction options."""
        if self.do_room_correction_var.get():
            self.room_options_frame.grid(row=self._room_options_row, column=0, sticky="ew", padx=0, pady=(0, 10))
        else:
            self.room_options_frame.grid_forget()

    def toggle_headphone_compensation(self) -> None:
        """Show or hide headphone compensation options."""
        if self.do_headphone_compensation_var.get():
            self.headphone_options_frame.grid(row=self._headphone_options_row, column=0, sticky="ew", padx=0, pady=(0, 10))
        else:
            self.headphone_options_frame.grid_forget()

    def toggle_advanced_options(self) -> None:
        """Show or hide advanced options."""
        if self.show_advanced_var.get():
            self.advanced_options_frame.grid(row=1, column=0, sticky="ew", padx=0, pady=(0, 15))
        else:
            self.advanced_options_frame.grid_forget()

    def update_balance_entry(self, *args: object) -> None:
        """Enable or disable balance dB entry."""
        if self.channel_balance_var.get() == "number":
            self.channel_balance_db_entry.configure(state="normal")
        else:
            self.channel_balance_db_entry.configure(state="disabled")

    def toggle_decay_per_channel(self) -> None:
        """Show or hide per-channel decay entries."""
        if self.decay_per_channel_var.get():
            self.decay_entry.configure(state="disabled")
            self.decay_channels_frame.grid(row=self._decay_channels_row, column=0, sticky="ew", padx=0, pady=5)
        else:
            self.decay_entry.configure(state="normal")
            self.decay_channels_frame.grid_forget()

    def toggle_vbass(self) -> None:
        """Enable or disable virtual bass options."""
        enabled = self.vbass_enable_var.get()
        state = "normal" if enabled else "disabled"
        self.vbass_freq_spin.configure(state=state)
        self.vbass_hp_entry.configure(state=state)
        self.vbass_polarity_menu.configure(state=state)
        if enabled:
            self.vbass_options_frame.grid(row=3, column=0, sticky="ew", padx=0, pady=(0, 10))
        else:
            self.vbass_options_frame.grid_forget()

    def toggle_mic_deviation(self) -> None:
        """Enable or disable mic deviation strength entry and debug options."""
        if self.microphone_deviation_correction_var.get():
            self.mic_deviation_strength_entry.configure(state="normal")
            self.mic_dev_debug_plots_check.configure(state="normal")
        else:
            self.mic_deviation_strength_entry.configure(state="disabled")
            self.mic_dev_debug_plots_check.configure(state="disabled")

    def generate_brir(self) -> None:
        """Generate BRIR using Impulcifer with progress dialog."""
        # Build arguments
        args = {
            'dir_path': self.dir_path_var.get(),
            'test_signal': self.test_signal_var.get(),
            'plot': self.plot_var.get(),
            'do_room_correction': self.do_room_correction_var.get(),
            'do_headphone_compensation': self.do_headphone_compensation_var.get(),
            'do_equalization': self.do_equalization_var.get()
        }

        # Room correction options
        if self.do_room_correction_var.get():
            args['room_target'] = self.room_target_var.get() if self.room_target_var.get() else None
            args['room_mic_calibration'] = self.room_mic_calibration_var.get() if self.room_mic_calibration_var.get() else None
            args['specific_limit'] = safe_get_int(self.specific_limit_var, 20000)
            args['generic_limit'] = safe_get_int(self.generic_limit_var, 1000)
            args['fr_combination_method'] = self.fr_combination_var.get()

        # Headphone compensation file handling
        if self.do_headphone_compensation_var.get() and self.headphone_compensation_file_var.get():
            source_file = self.headphone_compensation_file_var.get()
            if not os.path.isabs(source_file):
                source_file = os.path.join(self.dir_path_var.get(), source_file)

            target_file = os.path.join(self.dir_path_var.get(), 'headphones.wav')

            if os.path.exists(source_file):
                try:
                    shutil.copy2(source_file, target_file)
                except Exception as e:
                    print(f"Error copying headphone file: {e}")

        # Advanced options
        if self.show_advanced_var.get():
            args['fs'] = safe_get_int(self.fs_var, 48000) if self.fs_check_var.get() else None

            # Target level - safely convert string to float
            target_level_str = safe_get_string(self.target_level_var, "")
            if target_level_str.strip():
                try:
                    args['target_level'] = float(target_level_str)
                except ValueError:
                    args['target_level'] = None
            else:
                args['target_level'] = None

            # Channel balance
            if self.channel_balance_var.get() == 'number':
                args['channel_balance'] = safe_get_int(self.channel_balance_db_var, 0)
            elif self.channel_balance_var.get() != 'none':
                args['channel_balance'] = self.channel_balance_var.get()

            # Bass boost - safely get DoubleVar/IntVar values
            bass_gain = safe_get_double(self.bass_boost_gain_var, 0.0)
            if bass_gain:
                args['bass_boost_gain'] = bass_gain
                args['bass_boost_fc'] = safe_get_int(self.bass_boost_fc_var, 105)
                args['bass_boost_q'] = safe_get_double(self.bass_boost_q_var, 0.76)

            # Tilt - safely get DoubleVar value
            tilt_val = safe_get_double(self.tilt_var, 0.0)
            if tilt_val:
                args['tilt'] = tilt_val

            # Decay - safely handle string to float conversion
            if self.decay_per_channel_var.get():
                decay_dict = {}
                for ch, var in self.decay_channel_vars.items():
                    val_str = safe_get_string(var, "")
                    if val_str.strip():
                        try:
                            decay_dict[ch] = float(val_str) / 1000
                        except ValueError:
                            pass  # Skip invalid values
                if decay_dict:
                    args['decay'] = decay_dict
            else:
                decay_str = safe_get_string(self.decay_var, "")
                if decay_str.strip():
                    try:
                        decay_val = float(decay_str) / 1000
                        decay_dict = {}
                        for ch in ['FL', 'FC', 'FR', 'SL', 'SR', 'BL', 'BR']:
                            decay_dict[ch] = decay_val
                        args['decay'] = decay_dict
                    except ValueError:
                        pass  # Skip if invalid

            args['head_ms'] = safe_get_double(self.pre_response_var, 1.0)
            args['jamesdsp'] = self.jamesdsp_var.get()
            args['hangloose'] = self.hangloose_var.get()
            args['interactive_plots'] = self.interactive_plots_var.get()
            args['microphone_deviation_correction'] = self.microphone_deviation_correction_var.get()
            args['mic_deviation_strength'] = safe_get_double(self.mic_deviation_strength_var, 0.7)
            # v3.0: phase/adaptive/anatomical options are deprecated and ignored, using defaults
            args['mic_deviation_phase_correction'] = True
            args['mic_deviation_adaptive_correction'] = True
            args['mic_deviation_anatomical_validation'] = True
            args['mic_deviation_debug_plots'] = self.mic_deviation_debug_plots_var.get()
            args['output_truehd_layouts'] = self.output_truehd_layouts_var.get()

        # Virtual bass options
        if self.vbass_enable_var.get():
            args['vbass'] = True
            args['vbass_freq'] = max(30, min(500, safe_get_int(self.vbass_freq_var, 250)))
            args['vbass_hp'] = safe_get_double(self.vbass_hp_var, 15.0)
            # Map localized polarity string back to CLI value
            polarity_text = self.vbass_polarity_var.get()
            if polarity_text == self.loc.get('vbass_polarity_normal'):
                args['vbass_polarity'] = 'normal'
            elif polarity_text == self.loc.get('vbass_polarity_invert'):
                args['vbass_polarity'] = 'invert'
            else:
                args['vbass_polarity'] = 'auto'

        # Disable button during processing
        self.generate_button.configure(state="disabled", text=self.loc.get('button_processing'))

        # Create processing dialog
        dialog = ProcessingDialog(self.root, self.loc, fonts=self.fonts)

        # Setup logger callbacks and localization
        logger = get_logger()
        logger.set_localization(self.loc)  # Enable translations
        set_gui_callbacks(
            log_callback=dialog.add_log,
            progress_callback=dialog.update_progress
        )

        # Run processing in separate thread
        def run_processing():
            try:
                with impulcifer.cancellation_scope(dialog.cancel_event):
                    impulcifer.main(**args)
                # Mark as complete
                dialog.mark_complete(success=True)
                # Re-enable button
                self.root.after(0, lambda: self.generate_button.configure(
                    state="normal",
                    text=self.loc.get('button_generate_brir')
                ))
            except impulcifer.CancelledError:
                logger.warning("message_processing_cancelled")
                dialog.mark_cancelled()
                self.root.after(0, lambda: self.generate_button.configure(
                    state="normal",
                    text=self.loc.get('button_generate_brir')
                ))
            except Exception as e:
                # Mark as failed
                logger.error(f"Processing failed: {str(e)}")
                dialog.mark_complete(success=False)
                # Re-enable button
                self.root.after(0, lambda: self.generate_button.configure(
                    state="normal",
                    text=self.loc.get('button_generate_brir')
                ))
            finally:
                # 로거 콜백 해제 (다이얼로그 → GUI 위젯 참조 체인 끊기)
                set_gui_callbacks(log_callback=None, progress_callback=None)
                # 메모리 회수: BRIR 반복 생성 시 메모리 누적 방지
                import gc
                gc.collect()
                # Windows: 프로세스 working set 트리밍 (물리 메모리 OS 반환)
                try:
                    import ctypes
                    kernel32 = ctypes.windll.kernel32
                    kernel32.SetProcessWorkingSetSize(
                        kernel32.GetCurrentProcess(), -1, -1
                    )
                except Exception:
                    pass

        # Start processing thread
        thread = threading.Thread(target=run_processing, daemon=True)
        thread.start()
