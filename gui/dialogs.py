#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Modal dialogs for the modern GUI."""

from __future__ import annotations

import threading
from collections.abc import Callable
from tkinter import messagebox
from typing import Optional

import customtkinter as ctk

from gui.constants import (
    DIALOG_LANGUAGE_SIZE,
    DIALOG_PROCESSING_SIZE,
    DIALOG_UPDATE_SIZE,
    WIDGET_LOG_TEXTBOX_WIDTH,
    WIDGET_NOTES_TEXTBOX_WIDTH,
    WIDGET_PROGRESS_BAR_WIDTH,
)
from gui.utils import build_fonts, setup_pretendard_font
from i18n.localization import SUPPORTED_LANGUAGES
from updater.updater_core import (
    UpdateExecutionError,
    UpdateExecutionResult,
    create_update_executor,
)


class BaseDialog(ctk.CTkToplevel):
    """Base class for modal CustomTkinter dialogs."""

    def __init__(
        self,
        parent: object,
        loc_manager,
        fonts: Optional[dict[str, ctk.CTkFont]],
        title: str,
        size: tuple[int, int],
    ) -> None:
        """Initialize shared modal dialog window behavior.

        Args:
            parent: Parent Tk window.
            loc_manager: Localization manager used by the dialog.
            fonts: Shared font dictionary, or ``None`` to build a fallback set.
            title: Window title.
            size: Dialog size as ``(width, height)``.
        """
        super().__init__(parent)
        self.loc = loc_manager
        self.font_family = setup_pretendard_font(self.loc.current_language)
        self.fonts = fonts if fonts is not None else build_fonts(self.font_family)

        width, height = size
        self.title(title)
        self.geometry(f"{width}x{height}")
        self.transient(parent)
        self.grab_set()

        self.update_idletasks()
        x = (self.winfo_screenwidth() // 2) - (width // 2)
        y = (self.winfo_screenheight() // 2) - (height // 2)
        self.geometry(f"{width}x{height}+{x}+{y}")


class ProcessingDialog(BaseDialog):
    """Dialog showing BRIR generation progress, logs, and cancellation."""

    def __init__(
        self,
        parent: object,
        loc_manager,
        fonts: Optional[dict[str, ctk.CTkFont]] = None,
    ) -> None:
        """Create the processing dialog.

        Args:
            parent: Parent Tk window.
            loc_manager: Localization manager used by the dialog.
            fonts: Optional shared font dictionary.
        """
        super().__init__(
            parent,
            loc_manager,
            fonts,
            loc_manager.get('dialog_processing_title', default="Processing"),
            DIALOG_PROCESSING_SIZE,
        )
        self.cancel_event = threading.Event()
        self.processing_complete = False
        self.processing_error = False
        self.cancel_requested = False
        self.protocol("WM_DELETE_WINDOW", self.on_window_close)

        self.grid_rowconfigure(3, weight=1)
        self.grid_columnconfigure(0, weight=1)

        title_label = ctk.CTkLabel(
            self,
            text=self.loc.get('dialog_processing_message', default="Processing BRIR..."),
            font=self.fonts['heading'],
        )
        title_label.grid(row=0, column=0, padx=20, pady=(20, 10), sticky="w")

        self.progress_bar = ctk.CTkProgressBar(self, width=WIDGET_PROGRESS_BAR_WIDTH)
        self.progress_bar.set(0)
        self.progress_bar.grid(row=1, column=0, padx=20, pady=10, sticky="ew")

        self.progress_label = ctk.CTkLabel(
            self,
            text="0%",
            font=self.fonts['dialog_small'],
        )
        self.progress_label.grid(row=2, column=0, padx=20, pady=(0, 10), sticky="w")

        self.log_text = ctk.CTkTextbox(self, width=WIDGET_LOG_TEXTBOX_WIDTH, height=300)
        self.log_text.grid(row=3, column=0, padx=20, pady=10, sticky="nsew")

        button_frame = ctk.CTkFrame(self)
        button_frame.grid(row=4, column=0, padx=20, pady=10, sticky="ew")

        self.close_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('button_close', default="Close"),
            command=self.on_close,
            state="disabled",
        )
        self.close_button.pack(side="right", padx=5)

        self.cancel_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('button_cancel', default="Cancel"),
            command=self.on_cancel,
        )
        self.cancel_button.pack(side="right", padx=5)

    def update_progress(self, value: int, message: str = "") -> None:
        """Update progress controls from any thread."""

        def _update() -> None:
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

    def add_log(self, level: str, message: str) -> None:
        """Append a log message from any thread."""

        def _add() -> None:
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

    def mark_complete(self, success: bool = True) -> None:
        """Mark processing complete and enable closing."""
        self.processing_complete = True
        self.processing_error = not success

        def _apply() -> None:
            try:
                self.close_button.configure(state="normal")
                self.cancel_button.configure(state="disabled")
                if success:
                    self.progress_bar.set(1.0)
                    self.progress_label.configure(
                        text="100% - " + self.loc.get(
                            'message_processing_complete',
                            default="Complete!",
                        )
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

    def mark_cancelled(self) -> None:
        """Mark processing cancelled and enable closing."""
        self.processing_complete = True
        self.processing_error = True

        def _apply() -> None:
            try:
                self.close_button.configure(state="normal")
                self.cancel_button.configure(state="disabled")
                self.progress_label.configure(
                    text=self.loc.get(
                        'message_processing_cancelled',
                        default="Processing cancelled.",
                    )
                )
            except Exception:
                pass

        try:
            self.after(0, _apply)
        except Exception:
            pass

    def on_cancel(self) -> None:
        """Request cooperative cancellation."""
        if self.processing_complete:
            return
        self.cancel_requested = True
        self.cancel_event.set()
        try:
            self.cancel_button.configure(state="disabled")
            self.progress_label.configure(
                text=self.loc.get(
                    'message_processing_cancelling',
                    default="Cancelling after the current step...",
                )
            )
        except Exception:
            pass

    def on_window_close(self) -> None:
        """Treat close attempts as cancel until processing finishes."""
        if self.processing_complete:
            self.on_close()
        else:
            self.on_cancel()

    def on_close(self) -> None:
        """Close the dialog."""
        self.grab_release()
        self.destroy()


class LanguageSelectionDialog(BaseDialog):
    """First-run language selection dialog."""

    def __init__(
        self,
        parent: object,
        loc_manager,
        fonts: Optional[dict[str, ctk.CTkFont]],
        on_complete: Optional[Callable[[str], None]] = None,
    ) -> None:
        """Create a modal language selection dialog.

        Args:
            parent: Parent Tk window.
            loc_manager: Localization manager used by the dialog.
            fonts: Optional shared font dictionary.
            on_complete: Callback receiving the selected language code.
        """
        super().__init__(
            parent,
            loc_manager,
            fonts,
            loc_manager.get('dialog_select_language_title'),
            DIALOG_LANGUAGE_SIZE,
        )
        self.on_complete = on_complete
        self.selected_lang = ctk.StringVar(value=self.loc.current_language)
        self.protocol("WM_DELETE_WINDOW", self.on_ok)

        message = ctk.CTkLabel(
            self,
            text=self.loc.get('dialog_select_language_message'),
            font=self.fonts['dialog_body'],
            wraplength=350,
        )
        message.pack(pady=20, padx=20)

        lang_frame = ctk.CTkFrame(self)
        lang_frame.pack(pady=10, padx=20, fill="both", expand=True)

        for lang_code, lang_name in SUPPORTED_LANGUAGES.items():
            radio = ctk.CTkRadioButton(
                lang_frame,
                text=lang_name,
                variable=self.selected_lang,
                value=lang_code,
                font=self.fonts['subtitle'],
            )
            radio.pack(pady=5, padx=10, anchor="w")

        button_frame = ctk.CTkFrame(self)
        button_frame.pack(pady=10, padx=20, fill="x")

        ok_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('button_ok'),
            command=self.on_ok,
        )
        ok_button.pack(side="right", padx=5)

    def show(self) -> None:
        """Show the modal dialog and wait for the user to choose."""
        self.wait_window()

    def on_ok(self) -> None:
        """Complete language selection."""
        language_code = self.selected_lang.get()
        self.grab_release()
        self.destroy()
        if self.on_complete is not None:
            self.on_complete(language_code)


class UpdateDialog(BaseDialog):
    """Dialog notifying the user about available updates."""

    def __init__(
        self,
        parent: object,
        loc_manager,
        current_version: str,
        latest_version: str,
        download_url: str,
        release_notes: str = "",
        fonts: Optional[dict[str, ctk.CTkFont]] = None,
    ) -> None:
        """Create an update prompt dialog."""
        super().__init__(
            parent,
            loc_manager,
            fonts,
            loc_manager.get('update_available_title', default="Update Available"),
            DIALOG_UPDATE_SIZE,
        )
        self.current_version = current_version
        self.latest_version = latest_version
        self.download_url = download_url
        self.release_notes = release_notes
        self.user_choice = None

        self.grid_rowconfigure(2, weight=1)
        self.grid_columnconfigure(0, weight=1)

        title_frame = ctk.CTkFrame(self)
        title_frame.grid(row=0, column=0, padx=20, pady=20, sticky="ew")
        title_frame.grid_columnconfigure(0, weight=1)

        title_label = ctk.CTkLabel(
            title_frame,
            text=self.loc.get('update_available_message', default="A new version is available!"),
            font=self.fonts['dialog_title'],
        )
        title_label.grid(row=0, column=0, pady=(0, 10))

        version_text = self.loc.get(
            'update_version_info',
            default="Current: {current} → New: {latest}",
        ).format(current=current_version, latest=latest_version)
        version_label = ctk.CTkLabel(
            title_frame,
            text=version_text,
            font=self.fonts['dialog_body'],
        )
        version_label.grid(row=1, column=0)

        notes_label = ctk.CTkLabel(
            self,
            text=self.loc.get('update_release_notes', default="Release Notes:"),
            font=self.fonts['small_bold'],
        )
        notes_label.grid(row=1, column=0, padx=20, pady=(0, 5), sticky="w")

        self.notes_text = ctk.CTkTextbox(self, width=WIDGET_NOTES_TEXTBOX_WIDTH, height=250)
        self.notes_text.grid(row=2, column=0, padx=20, pady=(0, 20), sticky="nsew")
        self.notes_text.insert(
            "1.0",
            release_notes or self.loc.get('update_no_notes', default="No release notes available."),
        )
        self.notes_text.configure(state="disabled")

        self.progress_frame = ctk.CTkFrame(self)
        self.progress_label = ctk.CTkLabel(
            self.progress_frame,
            text=self.loc.get('update_downloading', default="Downloading update..."),
            font=self.fonts['dialog_small'],
        )
        self.progress_label.pack(pady=(10, 5))

        self.progress_bar = ctk.CTkProgressBar(
            self.progress_frame,
            width=WIDGET_NOTES_TEXTBOX_WIDTH,
        )
        self.progress_bar.set(0)
        self.progress_bar.pack(pady=(0, 10))

        button_frame = ctk.CTkFrame(self)
        button_frame.grid(row=3, column=0, padx=20, pady=20, sticky="ew")
        button_frame.grid_columnconfigure((0, 1, 2), weight=1)

        self.update_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('update_button_update', default="Update Now"),
            command=self.on_update,
            fg_color="green",
            hover_color="darkgreen",
        )
        self.update_button.grid(row=0, column=0, padx=5, sticky="ew")

        self.remind_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('update_button_remind', default="Remind Me Later"),
            command=self.on_remind_later,
        )
        self.remind_button.grid(row=0, column=1, padx=5, sticky="ew")

        self.skip_button = ctk.CTkButton(
            button_frame,
            text=self.loc.get('update_button_skip', default="Skip This Version"),
            command=self.on_skip,
            fg_color="gray",
            hover_color="darkgray",
        )
        self.skip_button.grid(row=0, column=2, padx=5, sticky="ew")

    def on_update(self) -> None:
        """Start the selected update executor in a worker thread."""
        self.user_choice = 'update'
        self.update_button.configure(state="disabled")
        self.remind_button.configure(state="disabled")
        self.skip_button.configure(state="disabled")
        self.progress_frame.grid(row=4, column=0, padx=20, pady=(0, 10), sticky="ew")

        executor = create_update_executor(self.download_url, self.latest_version)
        update_thread = threading.Thread(
            target=self._run_update_executor,
            args=(executor,),
            daemon=True,
        )
        update_thread.start()

    def _run_update_executor(self, executor) -> None:
        """Run an update executor and marshal the result to the UI thread."""
        try:
            result = executor.execute(self._executor_progress)
        except UpdateExecutionError as exc:
            error_text = str(exc)
            self.after(0, lambda error_text=error_text: self.show_error(error_text))
            return
        except Exception as exc:
            error_msg = self.loc.get(
                'update_error_general',
                default="Update error: {error}",
            ).format(error=str(exc))
            self.after(0, lambda error_msg=error_msg: self.show_error(error_msg))
            return

        self.after(0, lambda: self._handle_update_result(result))

    def _executor_progress(self, progress: float, message: str = "") -> None:
        """Update progress controls from an executor thread."""

        def _apply() -> None:
            self.progress_bar.set(max(0.0, min(1.0, progress)))
            if message:
                self.progress_label.configure(text=self.loc.get(message, default=message))

        self.after(0, _apply)

    def _handle_update_result(self, result: UpdateExecutionResult) -> None:
        """Display executor completion and run any deferred final action."""
        self.progress_bar.set(result.progress)
        self.progress_label.configure(
            text=self.loc.get(result.status_key, default=result.status_default)
        )
        messagebox.showinfo(
            self.loc.get(result.title_key, default=result.title_default),
            self.loc.get(result.message_key, default=result.message_default),
        )

        if result.after_message is not None:
            try:
                if result.after_message() is False:
                    self.show_error(
                        self.loc.get('update_error_apply', default="Failed to apply update")
                    )
                    return
            except SystemExit:
                raise
            except Exception as exc:
                self.show_error(str(exc))
                return

        self.after(result.close_delay_ms, self.destroy)

    def show_error(self, message: str) -> None:
        """Show an update error and close the dialog."""
        messagebox.showerror(self.loc.get('error_title', default="Error"), message)
        self.grab_release()
        self.destroy()

    def on_remind_later(self) -> None:
        """Close the dialog and remember the remind-later choice."""
        self.user_choice = 'remind_later'
        self.grab_release()
        self.destroy()

    def on_skip(self) -> None:
        """Close the dialog and remember the skip choice."""
        self.user_choice = 'skip'
        self.grab_release()
        self.destroy()
