#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Modern GUI for Impulcifer using CustomTkinter
Professional-grade interface with dark/light mode support
"""

import os
import sys
import re
import shutil
import platform
from tkinter import filedialog, messagebox
import customtkinter as ctk
import sounddevice
import recorder
import impulcifer
from constants import SPEAKER_LIST_PATTERN

# Set appearance mode and default color theme
ctk.set_appearance_mode("dark")  # Modes: "System" (default), "Dark", "Light"
ctk.set_default_color_theme("blue")  # Themes: "blue" (default), "green", "dark-blue"


class ModernImpulciferGUI:
    def __init__(self):
        self.root = ctk.CTk()
        self.root.title("Impulcifer - Modern HRIR Processing Suite")

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

        # Create Recorder tab
        self.create_recorder_tab()

        # Create Impulcifer tab
        self.create_impulcifer_tab()

    def create_header(self):
        """Create header with app title and theme toggle"""
        header = ctk.CTkFrame(self.root, corner_radius=0, height=60)
        header.grid(row=0, column=0, sticky="ew", padx=0, pady=0)
        header.grid_columnconfigure(1, weight=1)

        # App title
        title = ctk.CTkLabel(
            header,
            text="🎧 Impulcifer",
            font=ctk.CTkFont(size=24, weight="bold")
        )
        title.grid(row=0, column=0, padx=20, pady=15, sticky="w")

        # Subtitle
        subtitle = ctk.CTkLabel(
            header,
            text="HRIR Measurement & Binaural Processing System",
            font=ctk.CTkFont(size=12),
            text_color="gray"
        )
        subtitle.grid(row=0, column=1, padx=10, pady=15, sticky="w")

        # Theme toggle button
        self.theme_button = ctk.CTkButton(
            header,
            text="🌙 Dark Mode",
            command=self.toggle_theme,
            width=120,
            fg_color=("gray85", "gray20"),
            hover_color=("gray75", "gray30"),
            text_color=("gray10", "gray90"),
            border_width=1,
            border_color=("gray70", "gray40")
        )
        self.theme_button.grid(row=0, column=2, padx=20, pady=15, sticky="e")
        self.current_theme = "dark"

    def toggle_theme(self):
        """Toggle between dark and light themes"""
        if self.current_theme == "dark":
            ctk.set_appearance_mode("light")
            self.theme_button.configure(text="☀️ Light Mode")
            self.current_theme = "light"
        else:
            ctk.set_appearance_mode("dark")
            self.theme_button.configure(text="🌙 Dark Mode")
            self.current_theme = "dark"

    def create_tabs(self):
        """Create main tab view"""
        self.tabview = ctk.CTkTabview(self.root, corner_radius=10)
        self.tabview.grid(row=1, column=0, padx=20, pady=(0, 20), sticky="nsew")

        # Add tabs
        self.tabview.add("📼 Recorder")
        self.tabview.add("🎛️ Impulcifer")

        # Set default tab
        self.tabview.set("📼 Recorder")

    def create_recorder_tab(self):
        """Create Recorder tab with all recording features"""
        tab = self.tabview.tab("📼 Recorder")
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        # Create scrollable frame
        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)

        row = 0

        # === Audio Devices Section ===
        devices_frame = ctk.CTkFrame(scroll, corner_radius=10)
        devices_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        devices_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            devices_frame,
            text="Audio Devices",
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        # Host API
        ctk.CTkLabel(devices_frame, text="Host API:").grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.host_api_var = ctk.StringVar(value="Windows DirectSound" if platform.system() == "Windows" else "")
        self.host_api_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.host_api_var,
            values=["Windows DirectSound"],
            command=self.refresh_devices
        )
        self.host_api_menu.grid(row=1, column=1, sticky="ew", padx=15, pady=5)

        # Playback device
        ctk.CTkLabel(devices_frame, text="Playback Device:").grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.output_device_var = ctk.StringVar()
        self.output_device_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.output_device_var,
            values=["Default"]
        )
        self.output_device_menu.grid(row=2, column=1, sticky="ew", padx=15, pady=5)

        # Recording device
        ctk.CTkLabel(devices_frame, text="Recording Device:").grid(row=3, column=0, sticky="w", padx=15, pady=5)
        self.input_device_var = ctk.StringVar()
        self.input_device_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.input_device_var,
            values=["Default"]
        )
        self.input_device_menu.grid(row=3, column=1, sticky="ew", padx=15, pady=(5, 15))

        # === Files Section ===
        files_frame = ctk.CTkFrame(scroll, corner_radius=10)
        files_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        files_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            files_frame,
            text="Files",
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, columnspan=3, sticky="w", padx=15, pady=(15, 10))

        # File to play
        ctk.CTkLabel(files_frame, text="File to Play:").grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.play_var = ctk.StringVar(value=os.path.join('data', 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.play_entry = ctk.CTkEntry(files_frame, textvariable=self.play_var)
        self.play_entry.grid(row=1, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text="Browse...",
            command=lambda: self.browse_file(self.play_var, 'open', [
                ('Audio files', '*.wav *.mlp *.thd *.truehd'),
                ('WAV files', '*.wav'),
                ('TrueHD/MLP files', '*.mlp *.thd *.truehd'),
                ('All files', '*.*')
            ]),
            width=100
        ).grid(row=1, column=2, padx=15, pady=5)

        # Record to file
        ctk.CTkLabel(files_frame, text="Record to File:").grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.record_var = ctk.StringVar(value=os.path.join('data', 'my_hrir', 'FL,FR.wav'))
        self.record_entry = ctk.CTkEntry(files_frame, textvariable=self.record_var)
        self.record_entry.grid(row=2, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text="Browse...",
            command=lambda: self.browse_file(self.record_var, 'save'),
            width=100
        ).grid(row=2, column=2, padx=(15, 15), pady=(5, 15))

        # === Recording Options Section ===
        options_frame = ctk.CTkFrame(scroll, corner_radius=10)
        options_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        options_frame.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            options_frame,
            text="Recording Options",
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        # Channels checkbox and entry
        channels_subframe = ctk.CTkFrame(options_frame, fg_color="transparent")
        channels_subframe.grid(row=1, column=0, sticky="ew", padx=15, pady=5)
        channels_subframe.grid_columnconfigure(1, weight=1)

        self.channels_check_var = ctk.BooleanVar(value=False)
        self.channels_check = ctk.CTkCheckBox(
            channels_subframe,
            text="Force Channels:",
            variable=self.channels_check_var,
            command=self.update_channel_guidance
        )
        self.channels_check.grid(row=0, column=0, sticky="w", pady=5)

        self.channels_var = ctk.IntVar(value=14)
        self.channels_entry = ctk.CTkEntry(
            channels_subframe,
            textvariable=self.channels_var,
            width=80,
            state="disabled"
        )
        self.channels_entry.grid(row=0, column=1, sticky="w", padx=10, pady=5)

        # Channel guidance label
        self.channel_guidance = ctk.CTkLabel(
            options_frame,
            text="Using default 2-channel recording.",
            font=ctk.CTkFont(size=11),
            text_color="gray",
            wraplength=800,
            justify="left"
        )
        self.channel_guidance.grid(row=2, column=0, sticky="w", padx=15, pady=5)

        # Append checkbox
        self.append_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            options_frame,
            text="Append to existing file (silence will be added to equalize track lengths)",
            variable=self.append_var
        ).grid(row=3, column=0, sticky="w", padx=15, pady=(5, 15))

        # === Record Button ===
        self.record_button = ctk.CTkButton(
            scroll,
            text="🔴 START RECORDING",
            command=self.start_recording,
            height=50,
            font=ctk.CTkFont(size=16, weight="bold"),
            fg_color="#dc3545",
            hover_color="#c82333"
        )
        self.record_button.grid(row=row, column=0, sticky="ew", padx=10, pady=20)

        # Initialize devices
        self.refresh_devices()
        self.update_channel_guidance()

    def create_impulcifer_tab(self):
        """Create Impulcifer tab with all processing features"""
        tab = self.tabview.tab("🎛️ Impulcifer")
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        # Create scrollable frame
        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)

        row = 0

        # === Input Files Section ===
        input_frame = ctk.CTkFrame(scroll, corner_radius=10)
        input_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        input_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            input_frame,
            text="Input Files",
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, columnspan=3, sticky="w", padx=15, pady=(15, 10))

        # Your recordings
        ctk.CTkLabel(input_frame, text="Your Recordings:").grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.dir_path_var = ctk.StringVar(value=os.path.join('data', 'my_hrir'))
        self.dir_path_entry = ctk.CTkEntry(input_frame, textvariable=self.dir_path_var)
        self.dir_path_entry.grid(row=1, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            input_frame,
            text="Browse...",
            command=lambda: self.browse_directory(self.dir_path_var),
            width=100
        ).grid(row=1, column=2, padx=15, pady=5)

        # Test signal
        ctk.CTkLabel(input_frame, text="Test Signal:").grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.test_signal_var = ctk.StringVar(value=os.path.join('data', 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.test_signal_entry = ctk.CTkEntry(input_frame, textvariable=self.test_signal_var)
        self.test_signal_entry.grid(row=2, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            input_frame,
            text="Browse...",
            command=lambda: self.browse_file(self.test_signal_var, 'open', [
                ('Audio files', '*.wav *.pkl *.mlp *.thd *.truehd'),
                ('WAV files', '*.wav'),
                ('Pickle files', '*.pkl'),
                ('TrueHD/MLP files', '*.mlp *.thd *.truehd'),
                ('All files', '*.*')
            ]),
            width=100
        ).grid(row=2, column=2, padx=(15, 15), pady=(5, 15))

        # === Processing Options Section ===
        processing_frame = ctk.CTkFrame(scroll, corner_radius=10)
        processing_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        processing_frame.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            processing_frame,
            text="Processing Options",
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        proc_row = 1

        # Room Correction
        self.do_room_correction_var = ctk.BooleanVar(value=False)
        self.room_correction_check = ctk.CTkCheckBox(
            processing_frame,
            text="Room Correction",
            variable=self.do_room_correction_var,
            command=self.toggle_room_correction
        )
        self.room_correction_check.grid(row=proc_row, column=0, sticky="w", padx=15, pady=5)
        proc_row += 1

        # Room correction options (initially hidden)
        self.room_options_frame = ctk.CTkFrame(processing_frame, fg_color="transparent")

        room_opt_row = 0
        # Specific Limit
        limits_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        limits_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        room_opt_row += 1

        ctk.CTkLabel(limits_frame, text="Specific Limit (Hz):").pack(side="left", padx=5)
        self.specific_limit_var = ctk.IntVar(value=20000)
        ctk.CTkEntry(limits_frame, textvariable=self.specific_limit_var, width=80).pack(side="left", padx=5)

        ctk.CTkLabel(limits_frame, text="Generic Limit (Hz):").pack(side="left", padx=(20, 5))
        self.generic_limit_var = ctk.IntVar(value=1000)
        ctk.CTkEntry(limits_frame, textvariable=self.generic_limit_var, width=80).pack(side="left", padx=5)

        # FR combination method
        fr_method_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        fr_method_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        room_opt_row += 1

        ctk.CTkLabel(fr_method_frame, text="FR Combination:").pack(side="left", padx=5)
        self.fr_combination_var = ctk.StringVar(value="average")
        ctk.CTkOptionMenu(
            fr_method_frame,
            variable=self.fr_combination_var,
            values=["average", "conservative"],
            width=150
        ).pack(side="left", padx=5)

        # Mic calibration
        mic_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        mic_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        mic_frame.grid_columnconfigure(1, weight=1)
        room_opt_row += 1

        ctk.CTkLabel(mic_frame, text="Mic Calibration:").grid(row=0, column=0, sticky="w", padx=5, pady=2)
        self.room_mic_calibration_var = ctk.StringVar()
        ctk.CTkEntry(mic_frame, textvariable=self.room_mic_calibration_var).grid(row=0, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            mic_frame,
            text="Browse...",
            command=lambda: self.browse_file(self.room_mic_calibration_var, 'open', [
                ('Text files', '*.csv *.txt'),
                ('All files', '*.*')
            ]),
            width=80
        ).grid(row=0, column=2, padx=5, pady=2)

        # Room target
        ctk.CTkLabel(mic_frame, text="Target Curve:").grid(row=1, column=0, sticky="w", padx=5, pady=2)
        self.room_target_var = ctk.StringVar()
        ctk.CTkEntry(mic_frame, textvariable=self.room_target_var).grid(row=1, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            mic_frame,
            text="Browse...",
            command=lambda: self.browse_file(self.room_target_var, 'open', [
                ('Text files', '*.csv *.txt'),
                ('All files', '*.*')
            ]),
            width=80
        ).grid(row=1, column=2, padx=5, pady=2)

        # Headphone Compensation
        self.do_headphone_compensation_var = ctk.BooleanVar(value=False)
        self.headphone_check = ctk.CTkCheckBox(
            processing_frame,
            text="Headphone Compensation",
            variable=self.do_headphone_compensation_var,
            command=self.toggle_headphone_compensation
        )
        self.headphone_check.grid(row=proc_row, column=0, sticky="w", padx=15, pady=5)
        proc_row += 1

        # Headphone compensation options (initially hidden)
        self.headphone_options_frame = ctk.CTkFrame(processing_frame, fg_color="transparent")

        hp_frame = ctk.CTkFrame(self.headphone_options_frame, fg_color="transparent")
        hp_frame.grid(row=0, column=0, sticky="ew", padx=30, pady=5)
        hp_frame.grid_columnconfigure(1, weight=1)

        ctk.CTkLabel(hp_frame, text="Headphone File:").grid(row=0, column=0, sticky="w", padx=5, pady=2)
        self.headphone_compensation_file_var = ctk.StringVar()
        ctk.CTkEntry(hp_frame, textvariable=self.headphone_compensation_file_var).grid(row=0, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            hp_frame,
            text="Browse...",
            command=lambda: self.browse_file(self.headphone_compensation_file_var, 'open', [
                ('Audio files', '*.wav'),
                ('All files', '*.*')
            ]),
            width=80
        ).grid(row=0, column=2, padx=5, pady=2)

        # Custom EQ
        self.do_equalization_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            processing_frame,
            text="Custom EQ (from eq.csv / eq-left.csv / eq-right.csv)",
            variable=self.do_equalization_var
        ).grid(row=proc_row, column=0, sticky="w", padx=15, pady=5)
        proc_row += 1

        # Plot results
        self.plot_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            processing_frame,
            text="Plot Results (creates graphs, increases processing time)",
            variable=self.plot_var
        ).grid(row=proc_row, column=0, sticky="w", padx=15, pady=(5, 15))
        proc_row += 1

        # === Advanced Options Section ===
        advanced_frame = ctk.CTkFrame(scroll, corner_radius=10)
        advanced_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        advanced_frame.grid_columnconfigure(0, weight=1)
        row += 1

        self.show_advanced_var = ctk.BooleanVar(value=False)
        advanced_toggle = ctk.CTkCheckBox(
            advanced_frame,
            text="Advanced Options",
            variable=self.show_advanced_var,
            command=self.toggle_advanced_options,
            font=ctk.CTkFont(size=16, weight="bold")
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
        ctk.CTkCheckBox(resample_frame, text="Resample to (Hz):", variable=self.fs_check_var).pack(side="left", padx=5)
        self.fs_var = ctk.IntVar(value=48000)
        ctk.CTkOptionMenu(
            resample_frame,
            variable=self.fs_var,
            values=["44100", "48000", "88200", "96000", "176400", "192000", "352000", "384000"],
            width=120
        ).pack(side="left", padx=5)

        # Target level
        target_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        target_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(target_frame, text="Target Level (dB):").pack(side="left", padx=5)
        self.target_level_var = ctk.StringVar()
        ctk.CTkEntry(target_frame, textvariable=self.target_level_var, width=80).pack(side="left", padx=5)

        # Bass boost
        bass_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        bass_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(bass_frame, text="Bass Boost:").pack(side="left", padx=5)
        ctk.CTkLabel(bass_frame, text="Gain (dB):").pack(side="left", padx=(10, 2))
        self.bass_boost_gain_var = ctk.DoubleVar()
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_gain_var, width=60).pack(side="left", padx=2)

        ctk.CTkLabel(bass_frame, text="Fc:").pack(side="left", padx=(10, 2))
        self.bass_boost_fc_var = ctk.IntVar(value=105)
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_fc_var, width=60).pack(side="left", padx=2)

        ctk.CTkLabel(bass_frame, text="Q:").pack(side="left", padx=(10, 2))
        self.bass_boost_q_var = ctk.DoubleVar(value=0.76)
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_q_var, width=60).pack(side="left", padx=2)

        # Tilt
        tilt_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        tilt_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(tilt_frame, text="Tilt (dB/octave):").pack(side="left", padx=5)
        self.tilt_var = ctk.DoubleVar()
        ctk.CTkEntry(tilt_frame, textvariable=self.tilt_var, width=80).pack(side="left", padx=5)

        # Channel Balance
        balance_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        balance_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(balance_frame, text="Channel Balance:").pack(side="left", padx=5)
        self.channel_balance_var = ctk.StringVar(value="none")
        self.channel_balance_menu = ctk.CTkOptionMenu(
            balance_frame,
            variable=self.channel_balance_var,
            values=["none", "trend", "mids", "avg", "min", "left", "right", "number"],
            width=120,
            command=self.update_balance_entry
        )
        self.channel_balance_menu.pack(side="left", padx=5)

        ctk.CTkLabel(balance_frame, text="dB:").pack(side="left", padx=(10, 2))
        self.channel_balance_db_var = ctk.IntVar(value=0)
        self.channel_balance_db_entry = ctk.CTkEntry(balance_frame, textvariable=self.channel_balance_db_var, width=60, state="disabled")
        self.channel_balance_db_entry.pack(side="left", padx=2)

        # Decay
        decay_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        decay_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(decay_frame, text="Decay (ms):").pack(side="left", padx=5)
        self.decay_var = ctk.StringVar()
        self.decay_entry = ctk.CTkEntry(decay_frame, textvariable=self.decay_var, width=80)
        self.decay_entry.pack(side="left", padx=5)

        self.decay_per_channel_var = ctk.BooleanVar(value=False)
        self.decay_per_channel_check = ctk.CTkCheckBox(
            decay_frame,
            text="per channel",
            variable=self.decay_per_channel_var,
            command=self.toggle_decay_per_channel
        )
        self.decay_per_channel_check.pack(side="left", padx=10)

        # Per-channel decay (initially hidden)
        self.decay_channels_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")

        decay_ch_subframe = ctk.CTkFrame(self.decay_channels_frame, fg_color="transparent")
        decay_ch_subframe.grid(row=0, column=0, sticky="ew", padx=30, pady=5)

        self.decay_channel_vars = {}
        for i, ch in enumerate(['FL', 'FC', 'FR', 'SL', 'SR', 'BL', 'BR']):
            ctk.CTkLabel(decay_ch_subframe, text=f"{ch}:").pack(side="left", padx=2)
            var = ctk.StringVar()
            self.decay_channel_vars[ch] = var
            ctk.CTkEntry(decay_ch_subframe, textvariable=var, width=50).pack(side="left", padx=2)

        # Pre-response
        pre_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        pre_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(pre_frame, text="Pre-response (ms):").pack(side="left", padx=5)
        self.pre_response_var = ctk.DoubleVar(value=1.0)
        ctk.CTkEntry(pre_frame, textvariable=self.pre_response_var, width=80).pack(side="left", padx=5)

        # Output options
        output_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        output_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.jamesdsp_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(output_frame, text="JamesDSP output", variable=self.jamesdsp_var).pack(side="left", padx=5)

        self.hangloose_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(output_frame, text="Hangloose output", variable=self.hangloose_var).pack(side="left", padx=10)

        self.interactive_plots_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(output_frame, text="Interactive plots", variable=self.interactive_plots_var).pack(side="left", padx=10)

        # Mic deviation correction
        mic_dev_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        mic_dev_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.microphone_deviation_correction_var = ctk.BooleanVar(value=False)
        self.mic_dev_check = ctk.CTkCheckBox(
            mic_dev_frame,
            text="Mic Deviation Correction",
            variable=self.microphone_deviation_correction_var,
            command=self.toggle_mic_deviation
        )
        self.mic_dev_check.pack(side="left", padx=5)

        ctk.CTkLabel(mic_dev_frame, text="Strength:").pack(side="left", padx=(10, 2))
        self.mic_deviation_strength_var = ctk.DoubleVar(value=0.7)
        self.mic_deviation_strength_entry = ctk.CTkEntry(mic_dev_frame, textvariable=self.mic_deviation_strength_var, width=60, state="disabled")
        self.mic_deviation_strength_entry.pack(side="left", padx=2)

        # Mic deviation v2.0 advanced options
        mic_dev_v2_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        mic_dev_v2_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=2)
        adv_row += 1

        ctk.CTkLabel(mic_dev_v2_frame, text="  v2.0 Options:", font=ctk.CTkFont(size=11, weight="bold")).pack(side="left", padx=5)

        self.mic_deviation_phase_correction_var = ctk.BooleanVar(value=True)
        self.mic_dev_phase_check = ctk.CTkCheckBox(
            mic_dev_v2_frame,
            text="Phase Correction",
            variable=self.mic_deviation_phase_correction_var,
            state="disabled"
        )
        self.mic_dev_phase_check.pack(side="left", padx=5)

        self.mic_deviation_adaptive_correction_var = ctk.BooleanVar(value=True)
        self.mic_dev_adaptive_check = ctk.CTkCheckBox(
            mic_dev_v2_frame,
            text="Adaptive",
            variable=self.mic_deviation_adaptive_correction_var,
            state="disabled"
        )
        self.mic_dev_adaptive_check.pack(side="left", padx=5)

        self.mic_deviation_anatomical_validation_var = ctk.BooleanVar(value=True)
        self.mic_dev_anatomical_check = ctk.CTkCheckBox(
            mic_dev_v2_frame,
            text="Anatomical Validation",
            variable=self.mic_deviation_anatomical_validation_var,
            state="disabled"
        )
        self.mic_dev_anatomical_check.pack(side="left", padx=5)

        # TrueHD layouts
        truehd_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        truehd_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        self.output_truehd_layouts_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            truehd_frame,
            text="TrueHD layouts (11ch/13ch)",
            variable=self.output_truehd_layouts_var
        ).pack(side="left", padx=5)

        # === Generate Button ===
        self.generate_button = ctk.CTkButton(
            scroll,
            text="⚡ GENERATE BRIR",
            command=self.generate_brir,
            height=50,
            font=ctk.CTkFont(size=16, weight="bold"),
            fg_color="#28a745",
            hover_color="#218838"
        )
        self.generate_button.grid(row=row, column=0, sticky="ew", padx=10, pady=20)

    # === Helper Methods ===

    def refresh_devices(self, *args):
        """Refresh audio device lists"""
        # Get host APIs
        host_apis = {}
        for i, host in enumerate(sounddevice.query_hostapis()):
            host_apis[i] = host['name']

        # Update host API menu
        if host_apis:
            self.host_api_menu.configure(values=list(host_apis.values()))
            if not self.host_api_var.get() or self.host_api_var.get() not in host_apis.values():
                if "Windows DirectSound" in host_apis.values():
                    self.host_api_var.set("Windows DirectSound")
                else:
                    self.host_api_var.set(list(host_apis.values())[0])

        # Get devices for selected host API
        output_devices = []
        input_devices = []

        for device in sounddevice.query_devices():
            if host_apis.get(device['hostapi']) == self.host_api_var.get():
                if device['max_output_channels'] > 0:
                    output_devices.append(device['name'])
                if device['max_input_channels'] > 0:
                    input_devices.append(device['name'])

        # Update device menus
        if output_devices:
            self.output_device_menu.configure(values=output_devices)
            if not self.output_device_var.get() or self.output_device_var.get() not in output_devices:
                self.output_device_var.set(output_devices[0])

        if input_devices:
            self.input_device_menu.configure(values=input_devices)
            if not self.input_device_var.get() or self.input_device_var.get() not in input_devices:
                self.input_device_var.set(input_devices[0])

    def update_channel_guidance(self):
        """Update channel guidance text"""
        if self.channels_check_var.get():
            self.channels_entry.configure(state="normal")
            try:
                channel_count = self.channels_var.get()
                if channel_count == 14:
                    text = f"Recording with {channel_count} channels (7 speakers × 2 ears). Speakers: FL,FR,FC,BL,BR,SL,SR.wav"
                elif channel_count == 22:
                    text = f"Recording with {channel_count} channels (11 speakers × 2 ears, 7.0.4 Atmos). Speakers: FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR.wav"
                elif channel_count == 26:
                    text = f"Recording with {channel_count} channels (13 speakers × 2 ears, 7.0.6 Atmos). Speakers: FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR,TSL,TSR.wav"
                elif channel_count > 0:
                    speakers_count = channel_count // 2
                    text = f"Recording with {channel_count} channels ({speakers_count} speakers × 2 ears). Make sure your filename matches the speaker configuration."
                else:
                    text = "Enter valid channel count (recommended: 14, 22, or 26)"
            except:
                text = "Enter valid channel count (recommended: 14, 22, or 26)"
        else:
            self.channels_entry.configure(state="disabled")
            text = "Using default 2-channel recording."

        self.channel_guidance.configure(text=text)

    def toggle_room_correction(self):
        """Show/hide room correction options"""
        if self.do_room_correction_var.get():
            self.room_options_frame.grid(row=99, column=0, sticky="ew", padx=0, pady=(0, 10))
        else:
            self.room_options_frame.grid_forget()

    def toggle_headphone_compensation(self):
        """Show/hide headphone compensation options"""
        if self.do_headphone_compensation_var.get():
            self.headphone_options_frame.grid(row=100, column=0, sticky="ew", padx=0, pady=(0, 10))
        else:
            self.headphone_options_frame.grid_forget()

    def toggle_advanced_options(self):
        """Show/hide advanced options"""
        if self.show_advanced_var.get():
            self.advanced_options_frame.grid(row=1, column=0, sticky="ew", padx=0, pady=(0, 15))
        else:
            self.advanced_options_frame.grid_forget()

    def update_balance_entry(self, *args):
        """Enable/disable balance dB entry"""
        if self.channel_balance_var.get() == "number":
            self.channel_balance_db_entry.configure(state="normal")
        else:
            self.channel_balance_db_entry.configure(state="disabled")

    def toggle_decay_per_channel(self):
        """Show/hide per-channel decay entries"""
        if self.decay_per_channel_var.get():
            self.decay_entry.configure(state="disabled")
            self.decay_channels_frame.grid(row=999, column=0, sticky="ew", padx=0, pady=5)
        else:
            self.decay_entry.configure(state="normal")
            self.decay_channels_frame.grid_forget()

    def toggle_mic_deviation(self):
        """Enable/disable mic deviation strength entry and v2.0 options"""
        if self.microphone_deviation_correction_var.get():
            self.mic_deviation_strength_entry.configure(state="normal")
            self.mic_dev_phase_check.configure(state="normal")
            self.mic_dev_adaptive_check.configure(state="normal")
            self.mic_dev_anatomical_check.configure(state="normal")
        else:
            self.mic_deviation_strength_entry.configure(state="disabled")
            self.mic_dev_phase_check.configure(state="disabled")
            self.mic_dev_adaptive_check.configure(state="disabled")
            self.mic_dev_anatomical_check.configure(state="disabled")

    def browse_file(self, var, mode, filetypes=None):
        """Browse for file"""
        if filetypes is None:
            filetypes = [('All files', '*.*')]

        if mode == 'open':
            filename = filedialog.askopenfilename(
                initialdir=os.path.dirname(var.get()) if var.get() else os.getcwd(),
                initialfile=os.path.basename(var.get()) if var.get() else "",
                filetypes=filetypes
            )
        else:  # save
            filename = filedialog.asksaveasfilename(
                initialdir=os.path.dirname(var.get()) if var.get() else os.getcwd(),
                initialfile=os.path.basename(var.get()) if var.get() else "",
                defaultextension=".wav",
                filetypes=[('WAV file', '*.wav'), ('All files', '*.*')]
            )

        if filename:
            # Convert to relative path if possible
            try:
                filename = os.path.relpath(filename, os.getcwd())
            except:
                pass
            var.set(filename)

    def browse_directory(self, var):
        """Browse for directory"""
        dirname = filedialog.askdirectory(
            initialdir=var.get() if var.get() else os.getcwd()
        )

        if dirname:
            # Convert to relative path if possible
            try:
                dirname = os.path.relpath(dirname, os.getcwd())
            except:
                pass
            var.set(dirname)

    def start_recording(self):
        """Start recording process"""
        play_file = self.play_var.get()
        record_file = self.record_var.get()
        selected_channels = self.channels_var.get() if self.channels_check_var.get() else 2

        # Validate play file exists
        if not os.path.exists(play_file):
            messagebox.showerror('Error', f'Play file does not exist: {play_file}')
            return

        # Channel mismatch warning
        try:
            filename = os.path.basename(record_file)
            match = re.search(SPEAKER_LIST_PATTERN, filename)
            if match:
                speakers_str = match.group(1)
                expected_speakers = speakers_str.split(',')
                expected_channels = len(expected_speakers) * 2

                if self.channels_check_var.get() and selected_channels != expected_channels:
                    warning_msg = (
                        f"Channel count mismatch detected!\n\n"
                        f"Recording filename suggests {len(expected_speakers)} speakers ({', '.join(expected_speakers)}) "
                        f"which requires {expected_channels} channels (stereo pairs).\n\n"
                        f"But you have selected {selected_channels} input channels.\n\n"
                        f"Expected speakers: {', '.join(expected_speakers)}\n"
                        f"Expected channels: {expected_channels}\n"
                        f"Selected channels: {selected_channels}\n\n"
                        f"Continue anyway?"
                    )

                    if not messagebox.askyesno('Channel Mismatch Warning', warning_msg):
                        return
        except Exception as e:
            print(f"Warning: Could not parse filename for speaker validation: {e}")

        # Confirmation dialog
        info_msg = (
            f"Recording Setup:\n"
            f"Play file: {os.path.basename(play_file)}\n"
            f"Record file: {os.path.basename(record_file)}\n"
            f"Input device: {self.input_device_var.get() or 'Default'}\n"
            f"Output device: {self.output_device_var.get() or 'Default'}\n"
            f"Channels: {selected_channels}\n"
            f"Host API: {self.host_api_var.get() or 'Auto'}\n\n"
            f"Make sure:\n"
            f"- Your audio interface is properly connected\n"
            f"- Input/output devices are correctly selected\n"
            f"- Channel count matches your setup\n\n"
            f"Ready to start recording?"
        )

        if not messagebox.askyesno('Start Recording', info_msg):
            return

        # Start recording
        try:
            self.record_button.configure(state="disabled", text="🔴 RECORDING...")
            self.root.update()

            recorder.play_and_record(
                play=play_file,
                record=record_file,
                input_device=self.input_device_var.get(),
                output_device=self.output_device_var.get(),
                host_api=self.host_api_var.get(),
                channels=selected_channels,
                append=self.append_var.get()
            )

            self.record_button.configure(state="normal", text="🔴 START RECORDING")
            messagebox.showinfo('Recording Complete', f'Successfully recorded to {record_file}')
        except Exception as e:
            self.record_button.configure(state="normal", text="🔴 START RECORDING")
            messagebox.showerror('Recording Error', f'Recording failed: {str(e)}')

    def generate_brir(self):
        """Generate BRIR using Impulcifer"""
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
            args['specific_limit'] = self.specific_limit_var.get()
            args['generic_limit'] = self.generic_limit_var.get()
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
                    print(f"Copied {source_file} to {target_file}")
                except Exception as e:
                    print(f"Error copying headphone file: {e}")

        # Advanced options
        if self.show_advanced_var.get():
            args['fs'] = self.fs_var.get() if self.fs_check_var.get() else None
            args['target_level'] = float(self.target_level_var.get()) if self.target_level_var.get() else None

            # Channel balance
            if self.channel_balance_var.get() == 'number':
                args['channel_balance'] = self.channel_balance_db_var.get()
            elif self.channel_balance_var.get() != 'none':
                args['channel_balance'] = self.channel_balance_var.get()

            # Bass boost
            if self.bass_boost_gain_var.get():
                args['bass_boost_gain'] = self.bass_boost_gain_var.get()
                args['bass_boost_fc'] = self.bass_boost_fc_var.get()
                args['bass_boost_q'] = self.bass_boost_q_var.get()

            # Tilt
            if self.tilt_var.get():
                args['tilt'] = self.tilt_var.get()

            # Decay
            if self.decay_per_channel_var.get():
                decay_dict = {}
                for ch, var in self.decay_channel_vars.items():
                    if var.get():
                        decay_dict[ch] = float(var.get()) / 1000
                if decay_dict:
                    args['decay'] = decay_dict
            elif self.decay_var.get():
                decay_dict = {}
                decay_val = float(self.decay_var.get()) / 1000
                for ch in ['FL', 'FC', 'FR', 'SL', 'SR', 'BL', 'BR']:
                    decay_dict[ch] = decay_val
                args['decay'] = decay_dict

            args['head_ms'] = self.pre_response_var.get()
            args['jamesdsp'] = self.jamesdsp_var.get()
            args['hangloose'] = self.hangloose_var.get()
            args['interactive_plots'] = self.interactive_plots_var.get()
            args['microphone_deviation_correction'] = self.microphone_deviation_correction_var.get()
            args['mic_deviation_strength'] = self.mic_deviation_strength_var.get()
            args['mic_deviation_phase_correction'] = self.mic_deviation_phase_correction_var.get()
            args['mic_deviation_adaptive_correction'] = self.mic_deviation_adaptive_correction_var.get()
            args['mic_deviation_anatomical_validation'] = self.mic_deviation_anatomical_validation_var.get()
            args['output_truehd_layouts'] = self.output_truehd_layouts_var.get()

        print("Impulcifer arguments:", args)

        # Disable button during processing
        self.generate_button.configure(state="disabled", text="⚡ PROCESSING...")
        self.root.update()

        try:
            impulcifer.main(**args)
            self.generate_button.configure(state="normal", text="⚡ GENERATE BRIR")
            messagebox.showinfo('Done!', 'Generated files, check recordings folder.')
        except Exception as e:
            self.generate_button.configure(state="normal", text="⚡ GENERATE BRIR")
            messagebox.showerror('Error', f'Processing failed: {str(e)}')

    def run(self):
        """Start the GUI main loop"""
        self.root.mainloop()


def main_gui():
    """Entry point for modern GUI"""
    app = ModernImpulciferGUI()
    app.run()


if __name__ == "__main__":
    main_gui()
