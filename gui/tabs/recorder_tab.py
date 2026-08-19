#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Recorder tab for the modern GUI.

Hosts the host-API/device pickers, file paths, recording options, and the
record button.
"""

from __future__ import annotations

import os
import platform
from typing import TYPE_CHECKING
from tkinter import messagebox

import customtkinter as ctk
import sounddevice

from core.recording_validation import resolve_recording_channels
from core.sweep_signal import SWEEP_TRACK_LAYOUTS
from gui.constants import (
    FILETYPES_AUDIO,
    WIDGET_BUTTON_WIDTH_BROWSE,
    WIDGET_ENTRY_WIDTH_DEFAULT,
)
from gui.dialogs import RecordingProgressDialog
from gui.recording_actions import RecordingActionsMixin
from gui.recording_status import RecordingStatusController
from gui.sweep_source import recorder_sweep_mode_labels
from gui.utils import (
    browse_directory,
    browse_file,
    install_smooth_scrolling,
    restore_tk_vars,
    safe_get_int,
    snapshot_tk_vars,
)

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class RecorderTab(RecordingActionsMixin):
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

        ctk.CTkLabel(devices_frame, text=self.loc.get('label_host_api')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.host_api_var = ctk.StringVar(value="Windows DirectSound" if platform.system() == "Windows" else "")
        self.host_api_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.host_api_var,
            values=["Windows DirectSound"],
            command=self.refresh_devices
        )
        self.host_api_menu.grid(row=1, column=1, sticky="ew", padx=15, pady=5)

        ctk.CTkLabel(devices_frame, text=self.loc.get('label_playback_device')).grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.output_device_var = ctk.StringVar()
        self.output_device_menu = ctk.CTkOptionMenu(
            devices_frame,
            variable=self.output_device_var,
            values=["Default"]
        )
        self.output_device_menu.grid(row=2, column=1, sticky="ew", padx=15, pady=5)

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

        # Sweep source — on-the-fly generation is the default; a play file
        # is only needed for unusual/custom recordings.
        self._sweep_mode_labels = recorder_sweep_mode_labels(self.loc)
        ctk.CTkLabel(files_frame, text=self.loc.get('label_sweep_source')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.sweep_source_var = ctk.StringVar(value=self._sweep_mode_labels['default'])
        ctk.CTkOptionMenu(
            files_frame,
            variable=self.sweep_source_var,
            values=list(self._sweep_mode_labels.values()),
            command=lambda *_: self._on_sweep_source_change(),
        ).grid(row=1, column=1, sticky="w", padx=15, pady=5)

        # Generated-sweep parameters. Speakers + layout apply to both
        # generate modes; fs + duration only to the custom mode.
        self.sweep_params_frame = ctk.CTkFrame(files_frame, fg_color="transparent")
        self.sweep_params_frame.grid(row=2, column=0, columnspan=3, sticky="ew", padx=15, pady=0)
        self.sweep_params_frame.grid_columnconfigure(1, weight=1)

        ctk.CTkLabel(self.sweep_params_frame, text=self.loc.get('label_sweep_speakers')).grid(row=0, column=0, sticky="w", padx=(0, 10), pady=3)
        self.sweep_speakers_var = ctk.StringVar(value='FL,FR')
        self.sweep_speakers_var.trace_add('write', lambda *_: self._refresh_resolved_record_path())
        ctk.CTkEntry(self.sweep_params_frame, textvariable=self.sweep_speakers_var).grid(row=0, column=1, sticky="ew", pady=3)

        ctk.CTkLabel(self.sweep_params_frame, text=self.loc.get('label_sweep_layout')).grid(row=1, column=0, sticky="w", padx=(0, 10), pady=3)
        self.sweep_layout_var = ctk.StringVar(value='stereo')
        self.sweep_layout_var.trace_add('write', lambda *_: self._refresh_resolved_record_path())
        ctk.CTkOptionMenu(
            self.sweep_params_frame,
            variable=self.sweep_layout_var,
            values=list(SWEEP_TRACK_LAYOUTS),
        ).grid(row=1, column=1, sticky="w", pady=3)

        self.sweep_custom_frame = ctk.CTkFrame(self.sweep_params_frame, fg_color="transparent")
        self.sweep_custom_frame.grid(row=2, column=0, columnspan=2, sticky="ew", pady=0)
        self.sweep_custom_frame.grid_columnconfigure(3, weight=1)
        ctk.CTkLabel(self.sweep_custom_frame, text=self.loc.get('label_sweep_fs')).grid(row=0, column=0, sticky="w", padx=(0, 10), pady=3)
        self.sweep_fs_var = ctk.StringVar(value='48000')
        ctk.CTkEntry(self.sweep_custom_frame, textvariable=self.sweep_fs_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).grid(row=0, column=1, sticky="w", pady=3)
        ctk.CTkLabel(self.sweep_custom_frame, text=self.loc.get('label_sweep_duration')).grid(row=0, column=2, sticky="w", padx=(15, 10), pady=3)
        self.sweep_duration_var = ctk.StringVar(value='5.0')
        ctk.CTkEntry(self.sweep_custom_frame, textvariable=self.sweep_duration_var, width=WIDGET_ENTRY_WIDTH_DEFAULT).grid(row=0, column=3, sticky="w", pady=3)

        self.play_file_frame = ctk.CTkFrame(files_frame, fg_color="transparent")
        self.play_file_frame.grid(row=3, column=0, columnspan=3, sticky="ew", padx=0, pady=0)
        self.play_file_frame.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(self.play_file_frame, text=self.loc.get('label_file_to_play')).grid(row=0, column=0, sticky="w", padx=15, pady=5)
        self.play_var = ctk.StringVar(value=os.path.join('data', 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.play_var.trace_add('write', lambda *_: self._refresh_resolved_record_path())
        self.play_entry = ctk.CTkEntry(self.play_file_frame, textvariable=self.play_var)
        self.play_entry.grid(row=0, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            self.play_file_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_file(self.play_var, 'open', FILETYPES_AUDIO),
            width=WIDGET_BUTTON_WIDTH_BROWSE,
        ).grid(row=0, column=2, padx=15, pady=5)

        # Recording folder. Impulcifer's BRIR pipeline scans a directory
        # for ``<speakers>.wav`` / ``headphones.wav`` files, so the
        # recorder writes inside this folder using the canonical
        # filename derived from the sweep speakers (or the play file).
        ctk.CTkLabel(files_frame, text=self.loc.get('label_record_to_folder')).grid(row=4, column=0, sticky="w", padx=15, pady=5)
        self.record_dir_var = ctk.StringVar(value=os.path.join('data', 'my_hrir'))
        self.record_dir_var.trace_add('write', lambda *_: self._refresh_resolved_record_path())
        self.record_dir_entry = ctk.CTkEntry(files_frame, textvariable=self.record_dir_var)
        self.record_dir_entry.grid(row=4, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text=self.loc.get('button_browse'),
            command=lambda: browse_directory(self.record_dir_var),
            width=WIDGET_BUTTON_WIDTH_BROWSE,
        ).grid(row=4, column=2, padx=(15, 15), pady=5)

        # Resolved file preview — read-only hint showing where the WAV
        # will be written so the user can double-check before recording.
        self.resolved_record_var = ctk.StringVar()
        ctk.CTkLabel(
            files_frame,
            textvariable=self.resolved_record_var,
            font=self.fonts['small'],
            text_color="gray",
            wraplength=800,
            anchor="w",
            justify="left",
        ).grid(row=5, column=1, columnspan=2, sticky="ew", padx=15, pady=(0, 5))

        # sweep set은 수십 MB라 온디맨드 생성 — generate_sweep_set docstring 참조
        ctk.CTkButton(
            files_frame,
            text=self.loc.get('button_generate_14ch_sweep_set'),
            command=self.generate_sweep_set,
        ).grid(row=6, column=1, columnspan=2, sticky="w", padx=15, pady=(0, 15))

        self._on_sweep_source_change()

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

        self.channel_guidance = ctk.CTkLabel(
            options_frame,
            text=self.loc.get('message_using_default_recording'),
            font=self.fonts['small'],
            text_color="gray",
            wraplength=800,
            justify="left"
        )
        self.channel_guidance.grid(row=2, column=0, sticky="w", padx=15, pady=5)

        self.append_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            options_frame,
            text=self.loc.get('checkbox_append_to_file'),
            variable=self.append_var
        ).grid(row=3, column=0, sticky="w", padx=15, pady=5)

        self.debug_plots_var = ctk.BooleanVar(value=False)
        ctk.CTkCheckBox(
            options_frame,
            text=self.loc.get('checkbox_debug_plots'),
            variable=self.debug_plots_var,
        ).grid(row=4, column=0, sticky="w", padx=15, pady=(5, 15))

        # === Recording Status Section ===
        status_frame = ctk.CTkFrame(scroll, corner_radius=0)
        status_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        status_frame.grid_columnconfigure(0, weight=1)
        row += 1

        ctk.CTkLabel(
            status_frame,
            text=self.loc.get('section_recording_status'),
            font=self.fonts['heading']
        ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 8))

        self.recording_progress = ctk.CTkProgressBar(status_frame)
        self.recording_progress.grid(row=1, column=0, sticky="ew", padx=15, pady=(0, 8))

        self.recording_status_text = ctk.StringVar()
        self.recording_status_label = ctk.CTkLabel(
            status_frame,
            textvariable=self.recording_status_text,
            font=self.fonts['small_bold'],
            anchor="w",
            justify="left",
        )
        self.recording_status_label.grid(row=2, column=0, sticky="ew", padx=15, pady=(0, 2))

        self.recording_detail_text = ctk.StringVar()
        self.recording_detail_label = ctk.CTkLabel(
            status_frame,
            textvariable=self.recording_detail_text,
            font=self.fonts['small'],
            text_color="gray",
            wraplength=800,
            anchor="w",
            justify="left",
        )
        self.recording_detail_label.grid(row=3, column=0, sticky="ew", padx=15, pady=(0, 15))

        self.recording_feedback = RecordingStatusController(
            root=self.root,
            loc=self.loc,
            set_status=self.recording_status_text.set,
            set_detail=self.recording_detail_text.set,
            set_progress=self.recording_progress.set,
        )
        self.recording_feedback.reset()

        # === Record Buttons ===
        # Two separate paths: the main speaker-side button (derives the
        # filename from the play file) and the dedicated headphone-comp
        # button (locks the filename to ``headphones.wav`` and gates the
        # play file to mono/stereo so the L/R drivers can actually be
        # measured one at a time).
        record_buttons_frame = ctk.CTkFrame(scroll, fg_color="transparent")
        record_buttons_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=20)
        record_buttons_frame.grid_columnconfigure(0, weight=1)

        self.record_button = ctk.CTkButton(
            record_buttons_frame,
            text=self.loc.get('button_start_recording'),
            command=self.start_recording,
            height=50,
            font=self.fonts['heading'],
            fg_color="#dc3545",
            hover_color="#c82333"
        )
        self.record_button.grid(row=0, column=0, sticky="ew")

        self.record_headphones_button = ctk.CTkButton(
            record_buttons_frame,
            text=self.loc.get('button_record_headphones'),
            command=self.start_recording_headphones,
            height=40,
            font=self.fonts['heading'],
        )
        self.record_headphones_button.grid(row=1, column=0, sticky="ew", pady=(8, 0))

        self.refresh_devices()
        self.update_channel_guidance()

    def get_state(self) -> dict:
        """Return a snapshot of user-editable Tk variables."""
        return snapshot_tk_vars(self)

    def apply_state(self, state: dict) -> None:
        """Restore user-editable Tk variables after a UI rebuild."""
        restore_tk_vars(self, state)
        self.update_channel_guidance()
        self._on_sweep_source_change()

    def refresh_devices(self, *args: object) -> None:
        """Refresh audio device lists."""
        host_apis = {}
        for i, host in enumerate(sounddevice.query_hostapis()):
            host_apis[i] = host['name']

        if host_apis:
            self.host_api_menu.configure(values=list(host_apis.values()))
            if not self.host_api_var.get() or self.host_api_var.get() not in host_apis.values():
                if "Windows DirectSound" in host_apis.values():
                    self.host_api_var.set("Windows DirectSound")
                else:
                    self.host_api_var.set(list(host_apis.values())[0])

        output_devices = []
        input_devices = []

        for device in sounddevice.query_devices():
            if host_apis.get(device['hostapi']) == self.host_api_var.get():
                if device['max_output_channels'] > 0:
                    output_devices.append(device['name'])
                if device['max_input_channels'] > 0:
                    input_devices.append(device['name'])

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

    def _on_sweep_source_change(self) -> None:
        """Show/hide the parameter and play-file rows per sweep source."""
        mode = self._sweep_mode()
        if mode == "file":
            self.sweep_params_frame.grid_remove()
            self.play_file_frame.grid()
        else:
            self.sweep_params_frame.grid()
            self.play_file_frame.grid_remove()
            if mode == "custom":
                self.sweep_custom_frame.grid()
            else:
                self.sweep_custom_frame.grid_remove()
        self._refresh_resolved_record_path()

    def _recording_play_display(self, play_file: str) -> str:
        """Use Stable's basename-only file-mode feedback."""
        return os.path.basename(play_file)

    def _resolve_recording_channels(self) -> tuple[int, bool]:
        """Read Stable's explicit force-channels controls."""
        return resolve_recording_channels(
            self.channels_check_var.get(), safe_get_int(self.channels_var, 14)
        )

    def _confirm_recording_start(
        self,
        play_display: str,
        record_file: str,
        selected_channels: int,
    ) -> bool:
        """Show Stable's recording-setup confirmation dialog."""
        info_msg = self.loc.get(
            'message_recording_setup_info',
            play_file=play_display,
            record_file=os.path.basename(record_file),
            input_device=self.input_device_var.get() or 'Default',
            output_device=self.output_device_var.get() or 'Default',
            channels=selected_channels,
            host_api=self.host_api_var.get() or 'Auto',
        )
        return messagebox.askyesno(
            self.loc.get('message_start_recording_title'),
            info_msg,
        )

    def _prepare_speaker_recording(self, play_display: str) -> dict[str, object]:
        """Snapshot controls and start Stable feedback in the original order."""
        input_device = self.input_device_var.get()
        output_device = self.output_device_var.get()
        host_api = self.host_api_var.get()
        append = self.append_var.get()
        debug_plots = self.debug_plots_var.get()

        progress_context = self._start_recording_feedback(play_display)
        return {
            "input_device": input_device,
            "output_device": output_device,
            "host_api": host_api,
            "append": append,
            "debug_plots": debug_plots,
            "progress_context": progress_context,
        }

    def _prepare_headphones_recording(self, play_display: str) -> dict[str, object]:
        """Snapshot headphone controls before starting Stable feedback."""
        input_device = self.input_device_var.get()
        output_device = self.output_device_var.get()
        host_api = self.host_api_var.get()
        debug_plots = self.debug_plots_var.get()

        progress_context = self._start_recording_feedback(play_display)
        return {
            "input_device": input_device,
            "output_device": output_device,
            "host_api": host_api,
            "debug_plots": debug_plots,
            "progress_context": progress_context,
        }

    def _start_recording_feedback(
        self,
        play_display: str,
    ) -> RecordingProgressDialog:
        self._set_recording_busy(True)
        self.recording_feedback.start(play_display)
        return RecordingProgressDialog(
            self.root,
            self.loc,
            fonts=self.fonts,
        )

    def _handle_recording_progress(
        self,
        event: object,
        context: RecordingProgressDialog,
    ) -> None:
        """Update Stable recorder progress surfaces from a core progress event."""
        self.recording_feedback.handle_event(event)
        context.handle_event(event)

    def _set_recording_busy(self, busy: bool) -> None:
        """Toggle both record buttons together while one capture is in flight."""
        state = "disabled" if busy else "normal"
        if busy:
            self.record_button.configure(
                state=state,
                text=self.loc.get('button_start_recording_active'),
            )
            self.record_headphones_button.configure(state=state)
        else:
            self.record_button.configure(
                state=state,
                text=self.loc.get('button_start_recording'),
            )
            self.record_headphones_button.configure(
                state=state,
                text=self.loc.get('button_record_headphones'),
            )

    def _finish_recording_success(
        self,
        record_file: str,
        summary: object,
        context: RecordingProgressDialog,
    ) -> None:
        """Re-enable record button and show success message on main thread."""
        self._set_recording_busy(False)
        summary_text = self.recording_feedback.complete(record_file, summary)
        context.mark_complete(summary_text)
        messagebox.showinfo(
            self.loc.get('message_recording_complete_title'),
            self.loc.get('message_recording_complete', file=record_file)
        )

    def _finish_recording_error(
        self,
        error_msg: str,
        context: RecordingProgressDialog,
    ) -> None:
        """Re-enable record button and show error message on main thread."""
        self._set_recording_busy(False)
        self.recording_feedback.error(error_msg)
        context.mark_error(error_msg)
        messagebox.showerror(
            self.loc.get('message_recording_error_title'),
            self.loc.get('message_recording_error', error=error_msg)
        )
