#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Modern GUI for Impulcifer using CustomTkinter
Professional-grade interface with dark/light mode support
"""

import os
import re
import shutil
import platform
import threading
from tkinter import filedialog, messagebox
import customtkinter as ctk
import sounddevice
import recorder
import impulcifer
from constants import SPEAKER_LIST_PATTERN
from localization import get_localization_manager, SUPPORTED_LANGUAGES
from logger import get_logger, set_gui_callbacks
from update_checker import UpdateChecker
from updater import Updater

# Default theme setting (will be overridden by user preference)
ctk.set_default_color_theme("blue")  # Themes: "blue" (default), "green", "dark-blue"


class ProcessingDialog(ctk.CTkToplevel):
    """Dialog to show processing progress and logs"""

    def __init__(self, parent, loc_manager):
        super().__init__(parent)
        self.loc = loc_manager
        self.title(self.loc.get('dialog_processing_title', default="Processing"))
        self.geometry("700x500")
        self.transient(parent)
        self.grab_set()

        # Center the dialog
        self.update_idletasks()
        x = (self.winfo_screenwidth() // 2) - (700 // 2)
        y = (self.winfo_screenheight() // 2) - (500 // 2)
        self.geometry(f"700x500+{x}+{y}")

        # Configure grid
        self.grid_rowconfigure(1, weight=1)
        self.grid_columnconfigure(0, weight=1)

        # Title label
        title_label = ctk.CTkLabel(
            self,
            text=self.loc.get('dialog_processing_message', default="Processing BRIR..."),
            font=ctk.CTkFont(size=16, weight="bold")
        )
        title_label.grid(row=0, column=0, padx=20, pady=(20, 10), sticky="w")

        # Progress bar
        self.progress_bar = ctk.CTkProgressBar(self, width=660)
        self.progress_bar.set(0)
        self.progress_bar.grid(row=1, column=0, padx=20, pady=10, sticky="ew")

        # Progress label
        self.progress_label = ctk.CTkLabel(
            self,
            text="0%",
            font=ctk.CTkFont(size=12)
        )
        self.progress_label.grid(row=2, column=0, padx=20, pady=(0, 10), sticky="w")

        # Log text box
        self.log_text = ctk.CTkTextbox(self, width=660, height=300)
        self.log_text.grid(row=3, column=0, padx=20, pady=10, sticky="nsew")

        # Button frame
        button_frame = ctk.CTkFrame(self)
        button_frame.grid(row=4, column=0, padx=20, pady=10, sticky="ew")

        # Close button (initially disabled)
        self.close_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('button_close', default="Close"),
            command=self.on_close,
            state="disabled"
        )
        self.close_button.pack(side="right", padx=5)

        self.processing_complete = False
        self.processing_error = False

    def update_progress(self, value: int, message: str = ""):
        """Update progress bar and label"""
        try:
            self.progress_bar.set(value / 100.0)
            self.progress_label.configure(text=f"{value}% - {message}" if message else f"{value}%")
            self.update()
        except Exception:
            pass

    def add_log(self, level: str, message: str):
        """Add log message to text box"""
        try:
            # Add timestamp and level prefix
            if level == "ERROR":
                prefix = "✗ "
            elif level == "SUCCESS":
                prefix = "✓ "
            elif level == "WARNING":
                prefix = "⚠ "
            else:
                prefix = ""

            self.log_text.insert("end", f"{prefix}{message}\n")
            self.log_text.see("end")
            self.update()
        except Exception:
            pass

    def mark_complete(self, success: bool = True):
        """Mark processing as complete"""
        self.processing_complete = True
        self.processing_error = not success
        self.close_button.configure(state="normal")

        if success:
            self.progress_bar.set(1.0)
            self.progress_label.configure(text="100% - " + self.loc.get('message_processing_complete', default="Complete!"))
        else:
            self.progress_label.configure(text=self.loc.get('message_processing_error', default="Error occurred"))

    def on_close(self):
        """Close dialog"""
        self.grab_release()
        self.destroy()


class UpdateDialog(ctk.CTkToplevel):
    """Dialog to notify user about available updates"""

    def __init__(self, parent, loc_manager, current_version: str, latest_version: str, download_url: str, release_notes: str = ""):
        super().__init__(parent)
        self.loc = loc_manager
        self.current_version = current_version
        self.latest_version = latest_version
        self.download_url = download_url
        self.release_notes = release_notes
        self.user_choice = None  # Will be 'update', 'skip', or 'remind_later'

        self.title(self.loc.get('update_available_title', default="Update Available"))
        self.geometry("600x500")
        self.transient(parent)
        self.grab_set()

        # Center the dialog
        self.update_idletasks()
        x = (self.winfo_screenwidth() // 2) - (600 // 2)
        y = (self.winfo_screenheight() // 2) - (500 // 2)
        self.geometry(f"600x500+{x}+{y}")

        # Configure grid
        self.grid_rowconfigure(1, weight=1)
        self.grid_columnconfigure(0, weight=1)

        # Title and version info
        title_frame = ctk.CTkFrame(self)
        title_frame.grid(row=0, column=0, padx=20, pady=20, sticky="ew")
        title_frame.grid_columnconfigure(0, weight=1)

        title_label = ctk.CTkLabel(
            title_frame,
            text=self.loc.get('update_available_message', default="A new version is available!"),
            font=ctk.CTkFont(size=18, weight="bold")
        )
        title_label.grid(row=0, column=0, pady=(0, 10))

        version_text = self.loc.get('update_version_info', default="Current: {current} → New: {latest}").format(
            current=current_version,
            latest=latest_version
        )
        version_label = ctk.CTkLabel(
            title_frame,
            text=version_text,
            font=ctk.CTkFont(size=14)
        )
        version_label.grid(row=1, column=0)

        # Release notes
        notes_label = ctk.CTkLabel(
            self,
            text=self.loc.get('update_release_notes', default="Release Notes:"),
            font=ctk.CTkFont(size=12, weight="bold")
        )
        notes_label.grid(row=1, column=0, padx=20, pady=(0, 5), sticky="w")

        self.notes_text = ctk.CTkTextbox(self, width=560, height=250)
        self.notes_text.grid(row=2, column=0, padx=20, pady=(0, 20), sticky="nsew")
        self.grid_rowconfigure(2, weight=1)

        if release_notes:
            self.notes_text.insert("1.0", release_notes)
        else:
            self.notes_text.insert("1.0", self.loc.get('update_no_notes', default="No release notes available."))

        self.notes_text.configure(state="disabled")

        # Progress frame (hidden initially)
        self.progress_frame = ctk.CTkFrame(self)
        self.progress_label = ctk.CTkLabel(
            self.progress_frame,
            text=self.loc.get('update_downloading', default="Downloading update..."),
            font=ctk.CTkFont(size=12)
        )
        self.progress_label.pack(pady=(10, 5))

        self.progress_bar = ctk.CTkProgressBar(self.progress_frame, width=560)
        self.progress_bar.set(0)
        self.progress_bar.pack(pady=(0, 10))

        # Buttons
        button_frame = ctk.CTkFrame(self)
        button_frame.grid(row=3, column=0, padx=20, pady=20, sticky="ew")
        button_frame.grid_columnconfigure((0, 1, 2), weight=1)

        self.update_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('update_button_update', default="Update Now"),
            command=self.on_update,
            fg_color="green",
            hover_color="darkgreen"
        )
        self.update_button.grid(row=0, column=0, padx=5, sticky="ew")

        self.remind_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('update_button_remind', default="Remind Me Later"),
            command=self.on_remind_later
        )
        self.remind_button.grid(row=0, column=1, padx=5, sticky="ew")

        self.skip_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('update_button_skip', default="Skip This Version"),
            command=self.on_skip,
            fg_color="gray",
            hover_color="darkgray"
        )
        self.skip_button.grid(row=0, column=2, padx=5, sticky="ew")

    def on_update(self):
        """User chose to update now"""
        self.user_choice = 'update'

        # Hide buttons, show progress
        for widget in self.winfo_children():
            if isinstance(widget, ctk.CTkFrame) and widget != self.progress_frame:
                for button in widget.winfo_children():
                    if isinstance(button, ctk.CTkButton):
                        button.configure(state="disabled")

        self.progress_frame.grid(row=4, column=0, padx=20, pady=(0, 10), sticky="ew")

        # Start download in separate thread
        download_thread = threading.Thread(target=self.download_and_install, daemon=True)
        download_thread.start()

    def download_and_install(self):
        """Download and install the update"""
        try:
            updater = Updater(self.download_url, self.latest_version)

            # Download with progress callback
            success = updater.download(progress_callback=self.update_progress)

            if success:
                # Update UI on main thread
                self.after(0, lambda: self.progress_label.configure(
                    text=self.loc.get('update_installing', default="Installing update...")
                ))

                # Install and restart
                updater.install_and_restart()

                # If we get here, installation failed to start
                self.after(0, lambda: self.show_error(
                    self.loc.get('update_error_install', default="Failed to start installation")
                ))
            else:
                self.after(0, lambda: self.show_error(
                    self.loc.get('update_error_download', default="Failed to download update")
                ))

        except Exception as e:
            error_msg = self.loc.get('update_error_general', default="Update error: {error}").format(error=str(e))
            self.after(0, lambda: self.show_error(error_msg))

    def update_progress(self, downloaded: int, total: int):
        """Update progress bar"""
        if total > 0:
            progress = downloaded / total
            percentage = int(progress * 100)

            self.after(0, lambda: self.progress_bar.set(progress))
            self.after(0, lambda: self.progress_label.configure(
                text=self.loc.get('update_downloading_progress', default="Downloading: {percent}%").format(
                    percent=percentage
                )
            ))

    def show_error(self, message: str):
        """Show error message"""
        messagebox.showerror(
            self.loc.get('error_title', default="Error"),
            message
        )
        self.grab_release()
        self.destroy()

    def on_remind_later(self):
        """User chose to be reminded later"""
        self.user_choice = 'remind_later'
        self.grab_release()
        self.destroy()

    def on_skip(self):
        """User chose to skip this version"""
        self.user_choice = 'skip'
        self.grab_release()
        self.destroy()


class ModernImpulciferGUI:
    def __init__(self):
        # Initialize localization manager
        self.loc = get_localization_manager()

        # Apply saved theme
        saved_theme = self.loc.get_theme()
        if saved_theme == 'system':
            ctk.set_appearance_mode("system")
        else:
            ctk.set_appearance_mode(saved_theme)

        self.root = ctk.CTk()
        self.root.title(self.loc.get('app_title') + " - " + self.loc.get('app_subtitle'))

        # Show language selection dialog on first run
        if self.loc.is_first_run():
            self.root.after(500, self.show_language_selection_dialog)

        # Check for updates in background (after 2 seconds)
        self.root.after(2000, self.check_for_updates_background)

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

        # Create UI Settings tab
        self.create_ui_settings_tab()

    def create_header(self):
        """Create header with app title and theme toggle"""
        header = ctk.CTkFrame(self.root, corner_radius=0, height=60)
        header.grid(row=0, column=0, sticky="ew", padx=0, pady=0)
        header.grid_columnconfigure(1, weight=1)

        # App title
        self.title_label = ctk.CTkLabel(
            header,
            text=self.loc.get('app_title'),
            font=ctk.CTkFont(size=24, weight="bold")
        )
        self.title_label.grid(row=0, column=0, padx=20, pady=15, sticky="w")

        # Subtitle
        self.subtitle_label = ctk.CTkLabel(
            header,
            text=self.loc.get('app_subtitle'),
            font=ctk.CTkFont(size=12),
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

        # Set default tab
        self.tabview.set(self.loc.get('tab_recorder'))

    def create_recorder_tab(self):
        """Create Recorder tab with all recording features"""
        tab = self.tabview.tab(self.loc.get('tab_recorder'))
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
            text=self.loc.get('section_audio_devices'),
            font=ctk.CTkFont(size=16, weight="bold")
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
        files_frame = ctk.CTkFrame(scroll, corner_radius=10)
        files_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        files_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            files_frame,
            text=self.loc.get('section_files'),
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, columnspan=3, sticky="w", padx=15, pady=(15, 10))

        # File to play
        ctk.CTkLabel(files_frame, text=self.loc.get('label_file_to_play')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.play_var = ctk.StringVar(value=os.path.join('data', 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.play_entry = ctk.CTkEntry(files_frame, textvariable=self.play_var)
        self.play_entry.grid(row=1, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text=self.loc.get('button_browse'),
            command=lambda: self.browse_file(self.play_var, 'open', [
                ('Audio files', '*.wav *.mlp *.thd *.truehd'),
                ('WAV files', '*.wav'),
                ('TrueHD/MLP files', '*.mlp *.thd *.truehd'),
                ('All files', '*.*')
            ]),
            width=100
        ).grid(row=1, column=2, padx=15, pady=5)

        # Record to file
        ctk.CTkLabel(files_frame, text=self.loc.get('label_record_to_file')).grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.record_var = ctk.StringVar(value=os.path.join('data', 'my_hrir', 'FL,FR.wav'))
        self.record_entry = ctk.CTkEntry(files_frame, textvariable=self.record_var)
        self.record_entry.grid(row=2, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            files_frame,
            text=self.loc.get('button_browse'),
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
            text=self.loc.get('section_recording_options'),
            font=ctk.CTkFont(size=16, weight="bold")
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
            width=80,
            state="disabled"
        )
        self.channels_entry.grid(row=0, column=1, sticky="w", padx=10, pady=5)

        # Channel guidance label
        self.channel_guidance = ctk.CTkLabel(
            options_frame,
            text=self.loc.get('message_using_default_recording'),
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
            text=self.loc.get('checkbox_append_to_file'),
            variable=self.append_var
        ).grid(row=3, column=0, sticky="w", padx=15, pady=(5, 15))

        # === Record Button ===
        self.record_button = ctk.CTkButton(
            scroll,
            text=self.loc.get('button_start_recording'),
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
        tab = self.tabview.tab(self.loc.get('tab_impulcifer'))
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
            text=self.loc.get('section_input_files'),
            font=ctk.CTkFont(size=16, weight="bold")
        ).grid(row=0, column=0, columnspan=3, sticky="w", padx=15, pady=(15, 10))

        # Your recordings
        ctk.CTkLabel(input_frame, text=self.loc.get('label_your_recordings')).grid(row=1, column=0, sticky="w", padx=15, pady=5)
        self.dir_path_var = ctk.StringVar(value=os.path.join('data', 'my_hrir'))
        self.dir_path_entry = ctk.CTkEntry(input_frame, textvariable=self.dir_path_var)
        self.dir_path_entry.grid(row=1, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            input_frame,
            text=self.loc.get('button_browse'),
            command=lambda: self.browse_directory(self.dir_path_var),
            width=100
        ).grid(row=1, column=2, padx=15, pady=5)

        # Test signal
        ctk.CTkLabel(input_frame, text=self.loc.get('label_test_signal')).grid(row=2, column=0, sticky="w", padx=15, pady=5)
        self.test_signal_var = ctk.StringVar(value=os.path.join('data', 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        self.test_signal_entry = ctk.CTkEntry(input_frame, textvariable=self.test_signal_var)
        self.test_signal_entry.grid(row=2, column=1, sticky="ew", padx=15, pady=5)
        ctk.CTkButton(
            input_frame,
            text=self.loc.get('button_browse'),
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
            text=self.loc.get('section_processing_options'),
            font=ctk.CTkFont(size=16, weight="bold")
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

        # Room correction options (initially hidden)
        self.room_options_frame = ctk.CTkFrame(processing_frame, fg_color="transparent")

        room_opt_row = 0
        # Specific Limit
        limits_frame = ctk.CTkFrame(self.room_options_frame, fg_color="transparent")
        limits_frame.grid(row=room_opt_row, column=0, sticky="ew", padx=30, pady=5)
        room_opt_row += 1

        ctk.CTkLabel(limits_frame, text=self.loc.get('label_specific_limit')).pack(side="left", padx=5)
        self.specific_limit_var = ctk.IntVar(value=20000)
        ctk.CTkEntry(limits_frame, textvariable=self.specific_limit_var, width=80).pack(side="left", padx=5)

        ctk.CTkLabel(limits_frame, text=self.loc.get('label_generic_limit')).pack(side="left", padx=(20, 5))
        self.generic_limit_var = ctk.IntVar(value=1000)
        ctk.CTkEntry(limits_frame, textvariable=self.generic_limit_var, width=80).pack(side="left", padx=5)

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
            width=150
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
            command=lambda: self.browse_file(self.room_mic_calibration_var, 'open', [
                ('Text files', '*.csv *.txt'),
                ('All files', '*.*')
            ]),
            width=80
        ).grid(row=0, column=2, padx=5, pady=2)

        # Room target
        ctk.CTkLabel(mic_frame, text=self.loc.get('label_target_curve')).grid(row=1, column=0, sticky="w", padx=5, pady=2)
        self.room_target_var = ctk.StringVar()
        ctk.CTkEntry(mic_frame, textvariable=self.room_target_var).grid(row=1, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            mic_frame,
            text=self.loc.get('button_browse'),
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
            text=self.loc.get('section_headphone_compensation'),
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

        ctk.CTkLabel(hp_frame, text=self.loc.get('label_headphone_file')).grid(row=0, column=0, sticky="w", padx=5, pady=2)
        self.headphone_compensation_file_var = ctk.StringVar()
        ctk.CTkEntry(hp_frame, textvariable=self.headphone_compensation_file_var).grid(row=0, column=1, sticky="ew", padx=5, pady=2)
        ctk.CTkButton(
            hp_frame,
            text=self.loc.get('button_browse'),
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
        advanced_frame = ctk.CTkFrame(scroll, corner_radius=10)
        advanced_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        advanced_frame.grid_columnconfigure(0, weight=1)
        row += 1

        self.show_advanced_var = ctk.BooleanVar(value=False)
        advanced_toggle = ctk.CTkCheckBox(
            advanced_frame,
            text=self.loc.get('section_advanced_options'),
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
        ctk.CTkCheckBox(resample_frame, text=self.loc.get('checkbox_resample_to'), variable=self.fs_check_var).pack(side="left", padx=5)
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

        ctk.CTkLabel(target_frame, text=self.loc.get('label_target_level')).pack(side="left", padx=5)
        self.target_level_var = ctk.StringVar()
        ctk.CTkEntry(target_frame, textvariable=self.target_level_var, width=80).pack(side="left", padx=5)

        # Bass boost
        bass_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        bass_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(bass_frame, text=self.loc.get('label_bass_boost')).pack(side="left", padx=5)
        ctk.CTkLabel(bass_frame, text=self.loc.get('label_gain_db')).pack(side="left", padx=(10, 2))
        self.bass_boost_gain_var = ctk.DoubleVar()
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_gain_var, width=60).pack(side="left", padx=2)

        ctk.CTkLabel(bass_frame, text=self.loc.get('label_fc')).pack(side="left", padx=(10, 2))
        self.bass_boost_fc_var = ctk.IntVar(value=105)
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_fc_var, width=60).pack(side="left", padx=2)

        ctk.CTkLabel(bass_frame, text=self.loc.get('label_q')).pack(side="left", padx=(10, 2))
        self.bass_boost_q_var = ctk.DoubleVar(value=0.76)
        ctk.CTkEntry(bass_frame, textvariable=self.bass_boost_q_var, width=60).pack(side="left", padx=2)

        # Tilt
        tilt_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        tilt_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(tilt_frame, text=self.loc.get('label_tilt')).pack(side="left", padx=5)
        self.tilt_var = ctk.DoubleVar()
        ctk.CTkEntry(tilt_frame, textvariable=self.tilt_var, width=80).pack(side="left", padx=5)

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
            width=120,
            command=self.update_balance_entry
        )
        self.channel_balance_menu.pack(side="left", padx=5)

        ctk.CTkLabel(balance_frame, text=self.loc.get('label_balance_db')).pack(side="left", padx=(10, 2))
        self.channel_balance_db_var = ctk.IntVar(value=0)
        self.channel_balance_db_entry = ctk.CTkEntry(balance_frame, textvariable=self.channel_balance_db_var, width=60, state="disabled")
        self.channel_balance_db_entry.pack(side="left", padx=2)

        # Decay
        decay_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        decay_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=5)
        adv_row += 1

        ctk.CTkLabel(decay_frame, text=self.loc.get('label_decay')).pack(side="left", padx=5)
        self.decay_var = ctk.StringVar()
        self.decay_entry = ctk.CTkEntry(decay_frame, textvariable=self.decay_var, width=80)
        self.decay_entry.pack(side="left", padx=5)

        self.decay_per_channel_var = ctk.BooleanVar(value=False)
        self.decay_per_channel_check = ctk.CTkCheckBox(
            decay_frame,
            text=self.loc.get('checkbox_per_channel'),
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

        ctk.CTkLabel(pre_frame, text=self.loc.get('label_pre_response')).pack(side="left", padx=5)
        self.pre_response_var = ctk.DoubleVar(value=1.0)
        ctk.CTkEntry(pre_frame, textvariable=self.pre_response_var, width=80).pack(side="left", padx=5)

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
        self.mic_deviation_strength_entry = ctk.CTkEntry(mic_dev_frame, textvariable=self.mic_deviation_strength_var, width=60, state="disabled")
        self.mic_deviation_strength_entry.pack(side="left", padx=2)

        # Mic deviation v2.0 advanced options
        mic_dev_v2_frame = ctk.CTkFrame(self.advanced_options_frame, fg_color="transparent")
        mic_dev_v2_frame.grid(row=adv_row, column=0, sticky="ew", padx=15, pady=2)
        adv_row += 1

        ctk.CTkLabel(mic_dev_v2_frame, text=self.loc.get('label_v2_options'), font=ctk.CTkFont(size=11, weight="bold")).pack(side="left", padx=5)

        self.mic_deviation_phase_correction_var = ctk.BooleanVar(value=True)
        self.mic_dev_phase_check = ctk.CTkCheckBox(
            mic_dev_v2_frame,
            text=self.loc.get('checkbox_phase_correction'),
            variable=self.mic_deviation_phase_correction_var,
            state="disabled"
        )
        self.mic_dev_phase_check.pack(side="left", padx=5)

        self.mic_deviation_adaptive_correction_var = ctk.BooleanVar(value=True)
        self.mic_dev_adaptive_check = ctk.CTkCheckBox(
            mic_dev_v2_frame,
            text=self.loc.get('checkbox_adaptive_correction'),
            variable=self.mic_deviation_adaptive_correction_var,
            state="disabled"
        )
        self.mic_dev_adaptive_check.pack(side="left", padx=5)

        self.mic_deviation_anatomical_validation_var = ctk.BooleanVar(value=True)
        self.mic_dev_anatomical_check = ctk.CTkCheckBox(
            mic_dev_v2_frame,
            text=self.loc.get('checkbox_anatomical_validation'),
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
            text=self.loc.get('checkbox_truehd_layouts'),
            variable=self.output_truehd_layouts_var
        ).pack(side="left", padx=5)

        # === Generate Button ===
        self.generate_button = ctk.CTkButton(
            scroll,
            text=self.loc.get('button_generate_brir'),
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
            except Exception:
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
            except Exception:
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
            except Exception:
                pass
            var.set(dirname)

    def start_recording(self):
        """Start recording process"""
        play_file = self.play_var.get()
        record_file = self.record_var.get()
        selected_channels = self.channels_var.get() if self.channels_check_var.get() else 2

        # Validate play file exists
        if not os.path.exists(play_file):
            messagebox.showerror(self.loc.get('message_error'), self.loc.get('message_play_file_not_exist', file=play_file))
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

                    if not messagebox.askyesno(self.loc.get('message_channel_mismatch_warning_title'), warning_msg):
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

        if not messagebox.askyesno(self.loc.get('message_start_recording_title'), info_msg):
            return

        # Start recording
        try:
            self.record_button.configure(state="disabled", text=self.loc.get('button_start_recording_active'))
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

            self.record_button.configure(state="normal", text=self.loc.get('button_start_recording'))
            messagebox.showinfo(self.loc.get('message_recording_complete_title'), self.loc.get('message_recording_complete', file=record_file))
        except Exception as e:
            self.record_button.configure(state="normal", text=self.loc.get('button_start_recording'))
            messagebox.showerror(self.loc.get('message_recording_error_title'), self.loc.get('message_recording_error', error=str(e)))

    def generate_brir(self):
        """Generate BRIR using Impulcifer with progress dialog"""
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

        # Disable button during processing
        self.generate_button.configure(state="disabled", text=self.loc.get('button_processing'))

        # Create processing dialog
        dialog = ProcessingDialog(self.root, self.loc)

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
                impulcifer.main(**args)
                # Mark as complete
                dialog.mark_complete(success=True)
                # Re-enable button
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

        # Start processing thread
        thread = threading.Thread(target=run_processing, daemon=True)
        thread.start()

    def get_current_version(self) -> str:
        """Get current application version from pyproject.toml"""
        try:
            import tomllib
        except ImportError:
            try:
                import tomli as tomllib
            except ImportError:
                return "1.8.5"  # Fallback version

        try:
            from pathlib import Path
            pyproject_path = Path(__file__).parent / 'pyproject.toml'
            if pyproject_path.exists():
                with open(pyproject_path, 'rb') as f:
                    data = tomllib.load(f)
                    return data.get('project', {}).get('version', '1.8.5')
        except Exception:
            pass

        return "1.8.5"  # Fallback

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
        UpdateDialog(self.root, self.loc, current_version, latest_version, download_url, release_notes)

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
            font=ctk.CTkFont(size=14),
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
                font=ctk.CTkFont(size=12)
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

    def create_ui_settings_tab(self):
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
        lang_frame = ctk.CTkFrame(scroll, corner_radius=10)
        lang_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        lang_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            lang_frame,
            text=self.loc.get('section_language'),
            font=ctk.CTkFont(size=16, weight="bold")
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
        theme_frame = ctk.CTkFrame(scroll, corner_radius=10)
        theme_frame.grid(row=row, column=0, sticky="ew", padx=10, pady=10)
        theme_frame.grid_columnconfigure(1, weight=1)
        row += 1

        ctk.CTkLabel(
            theme_frame,
            text=self.loc.get('section_theme'),
            font=ctk.CTkFont(size=16, weight="bold")
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
        self.current_theme = theme_code

        messagebox.showinfo(
            self.loc.get('message_success'),
            self.loc.get('message_theme_changed')
        )

    def run(self):
        """Start the GUI main loop"""
        self.root.mainloop()


def main_gui():
    """Entry point for modern GUI"""
    app = ModernImpulciferGUI()
    app.run()


if __name__ == "__main__":
    main_gui()
