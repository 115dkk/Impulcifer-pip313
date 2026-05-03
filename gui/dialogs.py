#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Modal dialogs for the modern GUI.

Hosts ``ProcessingDialog`` (BRIR generation progress + log) and ``UpdateDialog``
(version-update prompt + download orchestration). Moved from
``gui/modern_gui.py`` without behavioural changes.
"""

import threading
from tkinter import messagebox

import customtkinter as ctk

from gui.utils import _build_fallback_fonts, setup_pretendard_font
from updater.updater_core import (
    GITHUB_RELEASES_URL,
    LegacyInstallerUpdater,
    VelopackUpdater,
    is_pip_environment,
    is_velopack_environment,
)


class ProcessingDialog(ctk.CTkToplevel):
    """Dialog to show processing progress and logs"""

    def __init__(self, parent, loc_manager, fonts: dict = None):
        super().__init__(parent)
        self.loc = loc_manager
        self.font_family = setup_pretendard_font(self.loc.current_language)
        self.fonts = fonts if fonts is not None else _build_fallback_fonts(self.font_family)
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
            font=self.fonts['heading']
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
            font=self.fonts['dialog_small']
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
        """Update progress bar and label - thread-safe (marshals to main thread)"""
        def _update():
            try:
                self.progress_bar.set(value / 100.0)
                self.progress_label.configure(
                    text=f"{value}% - {message}" if message else f"{value}%"
                )
            except Exception:
                pass
        try:
            self.after(0, _update)
        except Exception:
            pass

    def add_log(self, level: str, message: str):
        """Add log message to text box - thread-safe (marshals to main thread)"""
        def _add():
            try:
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
            except Exception:
                pass
        try:
            self.after(0, _add)
        except Exception:
            pass

    def mark_complete(self, success: bool = True):
        """Mark processing as complete - thread-safe (marshals to main thread)"""
        self.processing_complete = True
        self.processing_error = not success

        def _apply():
            try:
                self.close_button.configure(state="normal")
                if success:
                    self.progress_bar.set(1.0)
                    self.progress_label.configure(
                        text="100% - " + self.loc.get('message_processing_complete', default="Complete!")
                    )
                else:
                    self.progress_label.configure(
                        text=self.loc.get('message_processing_error', default="Error occurred")
                    )
            except Exception:
                pass
        try:
            self.after(0, _apply)
        except Exception:
            pass

    def on_close(self):
        """Close dialog"""
        self.grab_release()
        self.destroy()


class UpdateDialog(ctk.CTkToplevel):
    """Dialog to notify user about available updates"""

    def __init__(self, parent, loc_manager, current_version: str, latest_version: str, download_url: str, release_notes: str = "", fonts: dict = None):
        super().__init__(parent)
        self.loc = loc_manager
        self.font_family = setup_pretendard_font(self.loc.current_language)
        self.fonts = fonts if fonts is not None else _build_fallback_fonts(self.font_family)
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
            font=self.fonts['dialog_title']
        )
        title_label.grid(row=0, column=0, pady=(0, 10))

        version_text = self.loc.get('update_version_info', default="Current: {current} → New: {latest}").format(
            current=current_version,
            latest=latest_version
        )
        version_label = ctk.CTkLabel(
            title_frame,
            text=version_text,
            font=self.fonts['dialog_body']
        )
        version_label.grid(row=1, column=0)

        # Release notes
        notes_label = ctk.CTkLabel(
            self,
            text=self.loc.get('update_release_notes', default="Release Notes:"),
            font=self.fonts['small_bold']
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
            font=self.fonts['dialog_small']
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

        # Disable our action buttons via direct references — robust to layout changes.
        self.update_button.configure(state="disabled")
        self.remind_button.configure(state="disabled")
        self.skip_button.configure(state="disabled")

        self.progress_frame.grid(row=4, column=0, padx=20, pady=(0, 10), sticky="ew")

        if is_velopack_environment():
            self.progress_label.configure(
                text=self.loc.get('update_downloading', default="Downloading update...")
            )
            self.progress_bar.set(0.1)
            update_thread = threading.Thread(target=self.velopack_update, daemon=True)
            update_thread.start()
        elif is_pip_environment():
            self.progress_label.configure(
                text=self.loc.get('update_preparing', default="Preparing to update via pip...")
            )
            self.progress_bar.set(0.3)
            upgrade_thread = threading.Thread(target=self.pip_upgrade, daemon=True)
            upgrade_thread.start()
        else:
            # macOS/Linux legacy installer
            self.progress_label.configure(
                text=self.loc.get('update_downloading', default="Downloading installer...")
            )
            self.progress_bar.set(0.1)
            download_thread = threading.Thread(target=self.legacy_update, daemon=True)
            download_thread.start()

    def pip_upgrade(self):
        """Upgrade using pip — waits for the subprocess and surfaces failures."""
        process = None
        try:
            import sys
            import subprocess

            self.after(0, lambda: self.progress_label.configure(
                text=self.loc.get('update_installing', default="Installing update via pip...")
            ))
            self.after(0, lambda: self.progress_bar.set(0.5))

            python_exe = sys.executable
            upgrade_cmd = [
                python_exe,
                '-m',
                'pip',
                'install',
                '--upgrade',
                'impulcifer-py313'
            ]

            print(f"Upgrading with command: {' '.join(upgrade_cmd)}")

            self.after(0, lambda: self.progress_bar.set(0.7))

            # Capture output on every platform so we can detect failures.
            process = subprocess.Popen(
                upgrade_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE
            )

            # Wait for pip to actually finish (5 minute cap).
            stdout, stderr = process.communicate(timeout=300)

            if process.returncode == 0:
                self.after(0, lambda: self.progress_bar.set(1.0))
                self.after(0, lambda: self.progress_label.configure(
                    text=self.loc.get('update_success', default="Update started! Please restart the application.")
                ))
                self.after(1000, lambda: messagebox.showinfo(
                    self.loc.get('update_complete_title', default="Update Complete"),
                    self.loc.get('update_complete_message', default="The update has been started in the background.\nPlease restart the application to use the new version.")
                ))
                self.after(2000, lambda: self.destroy())
            else:
                error_detail = stderr.decode('utf-8', errors='replace')[:500] if stderr else ''
                code = process.returncode
                self.after(0, lambda: self.show_error(
                    f"pip upgrade failed (exit code {code}):\n{error_detail}"
                ))

        except subprocess.TimeoutExpired:
            if process is not None:
                process.kill()
            self.after(0, lambda: self.show_error("Update timed out after 5 minutes."))
        except Exception as e:
            error_msg = self.loc.get('update_error_general', default="Update error: {error}").format(error=str(e))
            self.after(0, lambda: self.show_error(error_msg))

    def velopack_update(self):
        """Update using Velopack (Windows standalone)"""
        try:
            updater = VelopackUpdater(GITHUB_RELEASES_URL, self.latest_version)

            self.after(0, lambda: self.progress_bar.set(0.3))
            self.after(0, lambda: self.progress_label.configure(
                text=self.loc.get('update_downloading', default="Downloading update...")
            ))

            success = updater.check_and_download()

            if success:
                self.after(0, lambda: self.progress_bar.set(0.8))
                self.after(0, lambda: self.progress_label.configure(
                    text=self.loc.get('update_installing', default="Applying update...")
                ))

                # Show message before restart
                self.after(0, lambda: messagebox.showinfo(
                    self.loc.get('update_complete_title', default="Update Ready"),
                    self.loc.get('update_restart_message',
                                 default="The application will close to apply the update.\n"
                                         "It will restart automatically in a few seconds.")
                ))

                updater.apply_and_restart()
                # If we get here, something went wrong
                self.after(0, lambda: self.show_error(
                    self.loc.get('update_error_apply', default="Failed to apply update")
                ))
            else:
                self.after(0, lambda: self.show_error(
                    self.loc.get('update_error_download', default="Failed to download update")
                ))

        except Exception as e:
            error_msg = self.loc.get('update_error_general', default="Update error: {error}").format(error=str(e))
            self.after(0, lambda: self.show_error(error_msg))

    def legacy_update(self):
        """Update using legacy installer download (macOS/Linux)"""
        try:
            if not self.download_url:
                self.after(0, lambda: self.show_error(
                    self.loc.get('update_error_no_installer',
                                default="No installer available. Please download manually from GitHub.")
                ))
                return

            updater = LegacyInstallerUpdater(self.download_url, self.latest_version)

            success = updater.download(progress_callback=self.update_progress)

            if success:
                self.after(0, lambda: self.progress_label.configure(
                    text=self.loc.get('update_installing', default="Opening installer...")
                ))
                self.after(0, lambda: self.progress_bar.set(0.9))

                updater.install()

                self.after(0, lambda: self.progress_bar.set(1.0))
                self.after(0, lambda: messagebox.showinfo(
                    self.loc.get('update_complete_title', default="Update Started"),
                    self.loc.get('update_manual_complete', default="Please follow the installer prompts to complete the update.")
                ))
                self.after(1000, self.destroy)
            else:
                self.after(0, lambda: self.show_error(
                    self.loc.get('update_error_download', default="Failed to download update")
                ))

        except Exception as e:
            error_msg = self.loc.get('update_error_general', default="Update error: {error}").format(error=str(e))
            self.after(0, lambda: self.show_error(error_msg))

    def update_progress(self, downloaded: int, total: int):
        """Update progress bar (for legacy download method)"""
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
