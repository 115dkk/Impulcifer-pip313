#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Recorder tab for the modern GUI.

Hosts the host-API/device pickers, file paths, recording options, and the
record button. Moved from ``gui/modern_gui.py`` without behavioural changes.
"""

from __future__ import annotations

import os
import platform
import threading
from typing import TYPE_CHECKING
from tkinter import messagebox

import customtkinter as ctk
import sounddevice

import core.recorder as recorder
from core.recording_validation import validate_recording_setup
from gui.constants import (
    FILETYPES_AUDIO,
    WIDGET_BUTTON_WIDTH_BROWSE,
    WIDGET_ENTRY_WIDTH_DEFAULT,
)
from gui.utils import (
    browse_file,
    install_smooth_scrolling,
    restore_tk_vars,
    safe_get_int,
    snapshot_tk_vars,
)

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class RecorderTab:
    """Build and handle the recording tab."""

    def __init__(self, app: ModernImpulciferGUI) -> None:
        """Create the recorder tab.

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
        """Create Recorder tab with all recording features."""
        tab = self.tabview.tab(self.loc.get('tab_recorder'))
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        # Create scrollable frame
        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)
        # Skip per-scroll-step bbox/scrollregion recompute — see install_smooth_scrolling.
        install_smooth_scrolling(scroll)

        row = 0

        # === Audio Devices Section ===
        devices_frame = ctk.CTkFrame(scroll, corner_radius=0)
        devices_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        devices_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            devices_frame,
            text=self.loc.get('section_audio_devices'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        # Host API
        ctk.CTkLabel(devices_frame, text=self.loc.get('label_host_api')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.host_api_var = ctk.StringVar(value="Windows DirectSound" if platform.system() == "Windows" else "")
        self.host_api_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.host_api_var,
            values=["Windows DirectSound"],
            command=self.refresh_devices
        )
        self.host_api_menu.grid(row=1, column=1, sticky="ew", padx=15, pady=5)

        # Playback device
        ctk.CTkLabel(devices_frame, text=self.loc.get('label_playback_device')).grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.output_device_var = ctk.StringVar()
        self.output_device_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.output_device_var,
            values=["Default"]
        )
        self.output_device_menu.grid(row=2, column=1, sticky="ew", padx=15, pady=5)

        # Recording device
        ctk.CTkLabel(devices_frame, text=self.loc.get('label_recording_device')).grid(row=3, column=0, sticky="w", padx=15, pady=5)
        self.input_device_var = ctk.StringVar()
        self.input_device_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.input_device_var,
            values=["Default"]
        )
        self.input_device_menu.grid(row=3, column=1, sticky="ew", padx=15, pady=(5, 15))

        # === Files Section ===
        files_frame = ctk.CTkFrame(scroll, corner_radius=0)
        files_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        files_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            files_frame,
            text=self.loc.get('section_files'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, columnspan=3, sticky="w", padx=15, pady=(15, 10))

        # File to play
        ctk.CTkLabel(files_frame, text=self.loc.get('label_file_to_play')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.play_var = ctk.StringVar(value=os.path.join('data', 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.play_entry = ctk.CTkEntry(files_frame, textvariable=self.play_var)
        self.play_entry.grid(row=1, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.play_var, 'open', FILETYPES_AUDIO),
            width=WIDGET_BUTTON_WIDTH_BROWSE,
        ).grid(row=1, column=2, padx=15, pady=5)

        # Record to file
        ctk.CTkLabel(files_frame, text=self.loc.get('label_record_to_file')).grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.record_var = ctk.StringVar(value=os.path.join('data', 'my_hrir', 'FL,FR.wav'))
        self.record_entry = ctk.CTkEntry(files_frame, textvariable=self.record_var)
        self.record_entry.grid(row=2, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.record_var, 'save'),
            width=WIDGET_BUTTON_WIDTH_BROWSE,
        ).grid(row=2, column=2, padx=(15, 15), pady=(5, 15))

        # === Recording Options Section ===
        options_frame = ctk.CTkFrame(scroll, corner_radius=0)
        options_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        options_frame.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            options_frame,
            text=self.loc.get('section_recording_options'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        # Channels checkbox and entry
        channels_subframe = ctk.CTkFrame(options_frame, fg_color="transparent")
        channels_subframe.grid(row=1, column=0, sticky="ew", padx=15, pady=5)
        channels_subframe.grid_columnconfigure(1, weight=1)

        self.channels_check_var = ctk.BooleanVar(value=False)
        self.channels_check = ctk.CTkCheckBox(
            channels_subframe,
            text=self.loc.get('label_force_channels'),
            variable=self.channels_check_var,
            command=self.update_channel_guidance
        )
        self.channels_check.grid(row=0, column=0, sticky="w", pady=5)

        self.channels_var = ctk.IntVar(value=14)
        self.channels_entry = ctk.CTkEntry(
            channels_subframe,
            textvariable=self.channels_var,
            width=WIDGET_ENTRY_WIDTH_DEFAULT,
            state="disabled"
        )
        self.channels_entry.grid(row=0, column=1, sticky="w", padx=10, pady=5)

        # Channel guidance label
        self.channel_guidance = ctk.CTkLabel(
            options_frame,
            text=self.loc.get('message_using_default_recording'),
            font=self.fonts['small'],
            text_color="gray",
            wraplength=800,
            justify="left"
        )
        self.channel_guidance.grid(row=2, column=0, sticky="w", padx=15, pady=5)

        # Append checkbox
        self.append_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            options_frame,
            text=self.loc.get('checkbox_append_to_file'),
            variable=self.append_var
        ).grid(row=3, column=0, sticky="w", padx=15, pady=(5, 15))

        # === Record Button ===
        self.record_button = ctk.CTkButton(
            scroll,
            text=self.loc.get('button_start_recording'),
            command=self.start_recording,
            height=50,
            font=self.fonts['heading'],
            fg_color="#dc3545",
            hover_color="#c82333"
        )
        self.record_button.grid(row=row, column=0, sticky="ew", padx=10, pady=20)

        # Initialize devices
        self.refresh_devices()
        self.update_channel_guidance()

    def get_state(self) -> dict:
        """Return a snapshot of user-editable Tk variables."""
        return snapshot_tk_vars(self)

    def apply_state(self, state: dict) -> None:
        """Restore user-editable Tk variables after a UI rebuild."""
        restore_tk_vars(self, state)
        self.update_channel_guidance()

    def refresh_devices(self, *args: object) -> None:
        """Refresh audio device lists."""
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

    def update_channel_guidance(self) -> None:
        """Update channel guidance text."""
        if self.channels_check_var.get():
            self.channels_entry.configure(state="normal")
            channel_count = safe_get_int(self.channels_var, 0)
            if channel_count == 14:
                text = self.loc.get(
                    'message_channel_guidance_standard',
                    channels=channel_count, speakers=7,
                    speaker_list="FL,FR,FC,BL,BR,SL,SR",
                )
            elif channel_count == 22:
                text = self.loc.get(
                    'message_channel_guidance_atmos_704',
                    channels=channel_count, speakers=11,
                    speaker_list="FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR",
                )
            elif channel_count == 26:
                text = self.loc.get(
                    'message_channel_guidance_atmos_706',
                    channels=channel_count, speakers=13,
                    speaker_list="FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR,TSL,TSR",
                )
            elif channel_count > 0:
                text = self.loc.get(
                    'message_channel_guidance_custom',
                    channels=channel_count,
                    speakers=channel_count // 2,
                )
            else:
                text = self.loc.get('message_channel_guidance_invalid')
        else:
            self.channels_entry.configure(state="disabled")
            text = self.loc.get('message_using_default_recording')

        self.channel_guidance.configure(text=text)

    def start_recording(self) -> None:
        """Start recording process."""
        play_file = self.play_var.get()
        record_file = self.record_var.get()
        selected_channels = safe_get_int(self.channels_var, 14) if self.channels_check_var.get() else 2

        # Validate play file exists
        if not os.path.exists(play_file):
            messagebox.showerror(self.loc.get('message_error'), self.loc.get('message_play_file_not_exist', file=play_file))
            return

        validation = validate_recording_setup(
            record_file,
            selected_channels,
            self.channels_check_var.get(),
        )
        if validation and validation.has_mismatch:
            warning_msg = self.loc.get(
                'message_channel_mismatch_body',
                expected_speakers=len(validation.expected_speakers),
                speaker_names=', '.join(validation.expected_speakers),
                expected_channels=validation.expected_channels,
                selected_channels=validation.selected_channels,
            )

            if not messagebox.askyesno(self.loc.get('message_channel_mismatch_warning_title'), warning_msg):
                return

        # Confirmation dialog
        info_msg = self.loc.get(
            'message_recording_setup_info',
            play_file=os.path.basename(play_file),
            record_file=os.path.basename(record_file),
            input_device=self.input_device_var.get() or 'Default',
            output_device=self.output_device_var.get() or 'Default',
            channels=selected_channels,
            host_api=self.host_api_var.get() or 'Auto',
        )

        if not messagebox.askyesno(self.loc.get('message_start_recording_title'), info_msg):
            return

        # Snapshot variables now (Tk vars must be read on main thread)
        input_device = self.input_device_var.get()
        output_device = self.output_device_var.get()
        host_api = self.host_api_var.get()
        append = self.append_var.get()

        self.record_button.configure(
            state="disabled",
            text=self.loc.get('button_start_recording_active')
        )

        def run_recording():
            try:
                recorder.play_and_record(
                    play=play_file,
                    record=record_file,
                    input_device=input_device,
                    output_device=output_device,
                    host_api=host_api,
                    channels=selected_channels,
                    append=append
                )
                self.root.after(0, lambda: self._on_recording_complete(record_file))
            except Exception as e:
                err = str(e)
                self.root.after(0, lambda: self._on_recording_error(err))

        thread = threading.Thread(target=run_recording, daemon=True)
        thread.start()

    def _on_recording_complete(self, record_file: str) -> None:
        """Re-enable record button and show success message on main thread."""
        self.record_button.configure(
            state="normal",
            text=self.loc.get('button_start_recording')
        )
        messagebox.showinfo(
            self.loc.get('message_recording_complete_title'),
            self.loc.get('message_recording_complete', file=record_file)
        )

    def _on_recording_error(self, error_msg: str) -> None:
        """Re-enable record button and show error message on main thread."""
        self.record_button.configure(
            state="normal",
            text=self.loc.get('button_start_recording')
        )
        messagebox.showerror(
            self.loc.get('message_recording_error_title'),
            self.loc.get('message_recording_error', error=error_msg)
        )
