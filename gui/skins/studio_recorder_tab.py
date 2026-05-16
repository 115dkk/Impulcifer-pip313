#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Studio Recorder tab — card-based device + recording UI."""

from __future__ import annotations

import os
import threading
from tkinter import messagebox
from typing import TYPE_CHECKING

import customtkinter as ctk
import sounddevice

from core import recorder
from core.headphones_recording import inspect_headphones_playback
from core.recording_naming import resolve_headphones_record_path, resolve_record_path
from core.recording_validation import validate_recording_setup
from core.sweep_set_generator import generate_sweep_set
from gui.constants import FILETYPES_AUDIO
from gui.recording_status import RecordingStatusController, analyze_recording
from gui.skins.studio_widgets import (
    add_card_header,
    add_field_row,
    make_card,
    make_card_body,
    make_page_header,
)
from gui.theme import COLORS, get_mono_font_family
from gui.utils import (
    browse_directory,
    browse_file,
    install_smooth_scrolling,
    safe_get_int,
)


# Channel layouts the project knows how to record. The label reads to
# the user; the int is what gets passed to ``recorder.play_and_record``.
# Keep this in sync with ``RecorderTab.update_channel_guidance`` over in
# the Stable skin — the two skins should accept the same set of presets.
CHANNEL_PRESETS: tuple[tuple[str, int], ...] = (
    ("2 (Stereo)", 2),
    ("14 (7ch)", 14),
    ("22 (7.0.4 Atmos)", 22),
    ("26 (7.0.6 Atmos)", 26),
)
_CUSTOM_CHANNEL_KEY = "기타…"

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioRecorderTab:
    """Studio recorder tab — focused subset of the Stable recorder UI."""

    def __init__(self, app: ModernImpulciferGUI, parent: ctk.CTkBaseClass) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.root = app.root
        self.parent = parent

        self.host_api_var = ctk.StringVar(value="Windows DirectSound")
        self.output_device_var = ctk.StringVar()
        self.input_device_var = ctk.StringVar()
        self.play_var = ctk.StringVar(
            value=os.path.join("data", "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
        )
        # Studio writes into a folder and derives the canonical filename
        # from the play file (e.g. ``sweep-seg-FL,FR-…wav`` → ``FL,FR.wav``).
        self.record_dir_var = ctk.StringVar(value=os.path.join("data", "my_hrir"))
        self.resolved_record_var = ctk.StringVar()
        self.channels_var = ctk.IntVar(value=2)
        self.debug_plots_var = ctk.BooleanVar(value=False)
        # Channel selector state — preset dropdown vs free-form custom entry.
        # ``channels_preset_var`` carries the dropdown label; when it's
        # ``_CUSTOM_CHANNEL_KEY`` the custom row appears and writes into
        # ``channels_var``.
        self.channels_preset_var = ctk.StringVar(value=CHANNEL_PRESETS[0][0])
        self.channels_custom_entry: ctk.CTkEntry | None = None
        self.channels_custom_row: ctk.CTkFrame | None = None

        self.host_api_menu: ctk.CTkOptionMenu | None = None
        self.input_device_menu: ctk.CTkOptionMenu | None = None
        self.output_device_menu: ctk.CTkOptionMenu | None = None
        self.record_button: ctk.CTkButton | None = None
        self.record_headphones_button: ctk.CTkButton | None = None
        self.recording_progress: ctk.CTkProgressBar | None = None
        self.recording_status_text = ctk.StringVar()
        self.recording_detail_text = ctk.StringVar()
        self.recording_feedback: RecordingStatusController | None = None
        self.segment_chip_frame: ctk.CTkFrame | None = None
        self.segment_chips: dict[str, ctk.CTkLabel] = {}
        self.segment_speakers: tuple[str, ...] = ()

        self._build()
        self._refresh_devices()

    def _build(self) -> None:
        self.parent.grid_columnconfigure(0, weight=1)
        self.parent.grid_rowconfigure(0, weight=1)

        # Opaque background (matches the Studio content host's bg-1) — a
        # ``transparent`` scrollable frame leaves the inner tk.Canvas with
        # no solid backing, so on Win32 the embedded child windows smear
        # ("잔상"/ghosting) during scroll because nothing repaints the
        # vacated region. Stable avoids this by using the theme's opaque
        # default fg_color; mirror that here.
        scroll = ctk.CTkScrollableFrame(self.parent, fg_color=COLORS["bg-1"])
        scroll.grid(row=0, column=0, sticky="nsew", padx=24, pady=24)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        page_header = make_page_header(
            scroll,
            title=self.loc.get("studio_recorder_title"),
            subtitle=self.loc.get("studio_recorder_subtitle"),
            fonts=self.fonts,
            cta_label=self.loc.get("studio_record_start"),
            cta_command=self.start_recording,
            cta_color="#dc2626",
        )
        page_header.grid(row=0, column=0, sticky="ew", pady=(0, 18))
        for child in page_header.winfo_children():
            if isinstance(child, ctk.CTkButton):
                self.record_button = child
                break

        self._build_devices_card(scroll, row=1)
        self._build_files_card(scroll, row=2)
        self._build_capture_status_card(scroll, row=3)

    # ------------------------------------------------------------------
    # Card 01 — Devices
    # ------------------------------------------------------------------
    def _build_devices_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="01", title=self.loc.get("studio_card_io_devices"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        # Host API
        api_row = self._labelled_row(body, row=0, label=self.loc.get("label_host_api"))
        self.host_api_menu = ctk.CTkOptionMenu(
            api_row,
            variable=self.host_api_var,
            values=["—"],
            command=lambda _: self._refresh_devices(),
        )
        self.host_api_menu.grid(row=0, column=1, sticky="ew", padx=0, pady=4)

        # Output
        out_row = self._labelled_row(body, row=1, label=self.loc.get("label_playback_device"))
        self.output_device_menu = ctk.CTkOptionMenu(
            out_row, variable=self.output_device_var, values=["—"]
        )
        self.output_device_menu.grid(row=0, column=1, sticky="ew", padx=0, pady=4)

        # Input
        in_row = self._labelled_row(body, row=2, label=self.loc.get("label_recording_device"))
        self.input_device_menu = ctk.CTkOptionMenu(
            in_row, variable=self.input_device_var, values=["—"]
        )
        self.input_device_menu.grid(row=0, column=1, sticky="ew", padx=0, pady=4)

        # Channels selector — preset dropdown + optional custom entry.
        ch_row = self._labelled_row(body, row=3, label=self.loc.get("label_force_channels"))
        ch_values = [label for label, _ in CHANNEL_PRESETS] + [_CUSTOM_CHANNEL_KEY]
        ctk.CTkOptionMenu(
            ch_row,
            variable=self.channels_preset_var,
            values=ch_values,
            command=self._on_channel_preset,
        ).grid(row=0, column=1, sticky="ew", pady=4)

        # Custom row appears below when the user picks 기타…
        custom_row = ctk.CTkFrame(body, fg_color="transparent")
        custom_row.grid(row=4, column=0, sticky="ew", pady=4)
        custom_row.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(
            custom_row,
            text=self.loc.get("label_force_channels_custom"),
            font=ctk.CTkFont(size=13),
            text_color=COLORS["fg-1"],
            anchor="w",
            width=140,
        ).grid(row=0, column=0, sticky="w", padx=(0, 16))
        self.channels_custom_entry = ctk.CTkEntry(
            custom_row,
            textvariable=self.channels_var,
            font=ctk.CTkFont(family=get_mono_font_family(), size=13),
            width=120,
        )
        self.channels_custom_entry.grid(row=0, column=1, sticky="w")
        self.channels_custom_row = custom_row
        custom_row.grid_remove()  # hidden until 기타… is chosen

        ctk.CTkCheckBox(
            body,
            text=self.loc.get("checkbox_debug_plots"),
            variable=self.debug_plots_var,
        ).grid(row=5, column=0, sticky="w", padx=0, pady=(8, 2))

    def _labelled_row(self, parent: ctk.CTkBaseClass, *, row: int, label: str) -> ctk.CTkFrame:
        frame = ctk.CTkFrame(parent, fg_color="transparent")
        frame.grid(row=row, column=0, sticky="ew", pady=4)
        frame.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(
            frame,
            text=label,
            font=ctk.CTkFont(size=13),
            text_color=COLORS["fg-1"],
            anchor="w",
            width=140,
        ).grid(row=0, column=0, sticky="w", padx=(0, 16))
        return frame

    def _on_channel_preset(self, label: str) -> None:
        """React to the channels preset dropdown.

        Mapping label → int is straight; the special ``기타…`` entry
        instead reveals the custom IntVar entry so power users can type
        any number that ``recorder.play_and_record`` is willing to
        accept (some surround interfaces report unusual layouts).
        """
        for preset_label, value in CHANNEL_PRESETS:
            if preset_label == label:
                self.channels_var.set(value)
                if self.channels_custom_row:
                    self.channels_custom_row.grid_remove()
                return
        # Custom path
        if self.channels_custom_row:
            self.channels_custom_row.grid()
        if self.channels_custom_entry:
            self.channels_custom_entry.focus_set()

    # ------------------------------------------------------------------
    # Card 02 — Files
    # ------------------------------------------------------------------
    def _build_files_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="02", title=self.loc.get("section_files"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        add_field_row(
            body, row=0,
            label=self.loc.get("label_file_to_play"),
            value_var=self.play_var,
            on_change=lambda: browse_file(self.play_var, "open", FILETYPES_AUDIO),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        self.play_var.trace_add('write', lambda *_: self._refresh_resolved_record_path())

        add_field_row(
            body, row=1,
            label=self.loc.get("label_record_to_folder"),
            value_var=self.record_dir_var,
            on_change=lambda: browse_directory(self.record_dir_var),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        self.record_dir_var.trace_add('write', lambda *_: self._refresh_resolved_record_path())

        # Resolved file preview — derived ``<folder>/<speakers>.wav``
        # path so the user can confirm before pressing record.
        ctk.CTkLabel(
            body,
            textvariable=self.resolved_record_var,
            font=ctk.CTkFont(size=12),
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=720,
        ).grid(row=2, column=0, sticky="ew", pady=(2, 6))
        self._refresh_resolved_record_path()

        # 14-channel surround sweep set generator. Drops four
        # ``sweep-seg-…-7.1.6-…wav`` files into a folder of the user's
        # choice (defaults near the existing play file). Materialized
        # on demand because each file is ~47 MB at PCM_32.
        ctk.CTkButton(
            body,
            text=self.loc.get("button_generate_14ch_sweep_set"),
            command=self._generate_sweep_set,
        ).grid(row=3, column=0, sticky="w", pady=(2, 0))

    # ------------------------------------------------------------------
    # Card 03 — Capture status
    # ------------------------------------------------------------------
    def _build_capture_status_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="03", title=self.loc.get("studio_card_capture_session"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        self.recording_progress = ctk.CTkProgressBar(body)
        self.recording_progress.grid(row=0, column=0, sticky="ew", pady=(2, 10))

        ctk.CTkLabel(
            body,
            textvariable=self.recording_status_text,
            font=ctk.CTkFont(size=13, weight="bold"),
            text_color=COLORS["fg-0"],
            anchor="w",
            justify="left",
        ).grid(row=1, column=0, sticky="ew", pady=(0, 4))

        ctk.CTkLabel(
            body,
            textvariable=self.recording_detail_text,
            font=ctk.CTkFont(size=12),
            text_color=COLORS["fg-2"],
            anchor="w",
            justify="left",
            wraplength=720,
        ).grid(row=2, column=0, sticky="ew")

        self.segment_chip_frame = ctk.CTkFrame(body, fg_color="transparent")
        self.segment_chip_frame.grid(row=3, column=0, sticky="ew", pady=(12, 0))

        # Secondary "Record headphones" affordance — distinct code path
        # from the page-header CTA so it can lock the filename to
        # ``headphones.wav`` and gate the play file to mono/stereo.
        self.record_headphones_button = ctk.CTkButton(
            body,
            text=self.loc.get("button_record_headphones"),
            command=self.start_recording_headphones,
        )
        self.record_headphones_button.grid(row=4, column=0, sticky="w", pady=(12, 0))

        self.recording_feedback = RecordingStatusController(
            root=self.root,
            loc=self.loc,
            set_status=self.recording_status_text.set,
            set_detail=self.recording_detail_text.set,
            set_progress=self.recording_progress.set,
        )
        self.recording_feedback.reset()

    # ------------------------------------------------------------------
    # Devices
    # ------------------------------------------------------------------
    def _refresh_devices(self) -> None:
        host_apis: dict[int, str] = {}
        for i, host in enumerate(sounddevice.query_hostapis()):
            host_apis[i] = host["name"]

        if self.host_api_menu and host_apis:
            values = list(host_apis.values())
            self.host_api_menu.configure(values=values)
            if not self.host_api_var.get() or self.host_api_var.get() not in values:
                self.host_api_var.set("Windows DirectSound" if "Windows DirectSound" in values else values[0])

        output_devices: list[str] = []
        input_devices: list[str] = []
        for device in sounddevice.query_devices():
            if host_apis.get(device["hostapi"]) == self.host_api_var.get():
                if device["max_output_channels"] > 0:
                    output_devices.append(device["name"])
                if device["max_input_channels"] > 0:
                    input_devices.append(device["name"])

        if self.output_device_menu and output_devices:
            self.output_device_menu.configure(values=output_devices)
            if not self.output_device_var.get() or self.output_device_var.get() not in output_devices:
                self.output_device_var.set(output_devices[0])

        if self.input_device_menu and input_devices:
            self.input_device_menu.configure(values=input_devices)
            if not self.input_device_var.get() or self.input_device_var.get() not in input_devices:
                self.input_device_var.set(input_devices[0])

    # ------------------------------------------------------------------
    # Recording
    # ------------------------------------------------------------------
    def _generate_sweep_set(self) -> None:
        """Generate the bundled 14-channel surround sweep set on demand.

        Defaults the target folder to the play file's parent so the new
        WAVs land next to the existing bundled sweeps. After writing,
        the play picker is auto-pointed at the FL,FR group.
        """
        from tkinter.filedialog import askdirectory

        play_file = self.play_var.get().strip()
        initial_dir = os.path.dirname(play_file) if play_file else os.getcwd()
        if not os.path.isdir(initial_dir):
            initial_dir = os.getcwd()

        target_dir = askdirectory(
            initialdir=initial_dir,
            title=self.loc.get("dialog_choose_sweep_set_folder"),
        )
        if not target_dir:
            return

        try:
            paths = generate_sweep_set(target_dir)
        except Exception as exc:
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_sweep_set_error", error=str(exc)),
            )
            return

        if paths:
            try:
                self.play_var.set(os.path.relpath(paths[0]))
            except ValueError:
                self.play_var.set(paths[0])

        messagebox.showinfo(
            self.loc.get("message_sweep_set_complete_title"),
            self.loc.get(
                "message_sweep_set_complete",
                count=len(paths),
                folder=target_dir,
            ),
        )

    def _refresh_resolved_record_path(self) -> None:
        """Recompute the read-only ``<folder>/<derived>.wav`` hint label."""
        record_dir = self.record_dir_var.get().strip()
        play_file = self.play_var.get().strip()
        if not record_dir or not play_file:
            self.resolved_record_var.set("")
            return
        try:
            resolved = resolve_record_path(record_dir, play_file)
        except Exception:
            self.resolved_record_var.set("")
            return
        self.resolved_record_var.set(
            self.loc.get("label_record_resolved_path", path=resolved)
        )

    def start_recording(self) -> None:
        play_file = self.play_var.get()
        record_dir = self.record_dir_var.get().strip()
        if not record_dir:
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_record_folder_required"),
            )
            return
        record_file = resolve_record_path(record_dir, play_file)
        channels = max(2, safe_get_int(self.channels_var, 2))

        if not os.path.exists(play_file):
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_play_file_not_exist", file=play_file),
            )
            return

        # Stable's force-channels checkbox is replaced here by the channel
        # preset itself: keep the auto-stereo (channels == 2) path silent,
        # and only warn when the user explicitly picks a different channel
        # count that disagrees with the speaker-list filename.
        validation = validate_recording_setup(record_file, channels, channels != 2)
        if validation and validation.has_mismatch:
            warning_msg = self.loc.get(
                "message_channel_mismatch_body",
                expected_speakers=len(validation.expected_speakers),
                speaker_names=", ".join(validation.expected_speakers),
                expected_channels=validation.expected_channels,
                selected_channels=validation.selected_channels,
            )
            if not messagebox.askyesno(self.loc.get("message_channel_mismatch_warning_title"), warning_msg):
                return

        self._set_recording_busy(True)
        if self.recording_feedback:
            self.recording_feedback.start(play_file)

        def report_progress(event):
            self.root.after(0, lambda event=event: self._on_progress_event(event))

        input_device = self.input_device_var.get()
        output_device = self.output_device_var.get()
        host_api = self.host_api_var.get()
        debug_plots = self.debug_plots_var.get()

        def _run() -> None:
            try:
                recorder.play_and_record(
                    play=play_file,
                    record=record_file,
                    input_device=input_device,
                    output_device=output_device,
                    host_api=host_api,
                    channels=channels,
                    append=False,
                    debug_plots=debug_plots,
                    progress_callback=report_progress,
                )
                summary = analyze_recording(record_file)
                self.root.after(0, lambda: self._on_complete(record_file, summary))
            except Exception as e:
                err = str(e)
                self.root.after(0, lambda: self._on_error(err))

        threading.Thread(target=_run, daemon=True).start()

    def _on_progress_event(self, event: object) -> None:
        if self.recording_feedback:
            self.recording_feedback.handle_event(event)
        speakers = tuple(getattr(event, "speakers", ()) or ())
        self._render_segment_chips(speakers)
        self._highlight_segment_chip(getattr(event, "speaker", None))

    def _render_segment_chips(self, speakers: tuple[str, ...]) -> None:
        if self.segment_chip_frame is None or speakers == self.segment_speakers:
            return

        for child in self.segment_chip_frame.winfo_children():
            child.destroy()
        self.segment_chips.clear()
        self.segment_speakers = speakers

        for column, speaker in enumerate(speakers):
            chip = ctk.CTkLabel(
                self.segment_chip_frame,
                text=speaker,
                font=ctk.CTkFont(family=get_mono_font_family(), size=12, weight="bold"),
                fg_color=COLORS["bg-3"],
                text_color=COLORS["fg-1"],
                corner_radius=4,
                padx=10,
                height=26,
            )
            chip.grid(row=0, column=column, sticky="w", padx=(0, 8), pady=2)
            self.segment_chips[speaker] = chip

    def _highlight_segment_chip(self, active_speaker: str | None) -> None:
        for speaker, chip in self.segment_chips.items():
            if speaker == active_speaker:
                chip.configure(fg_color=COLORS["accent"], text_color="#ffffff")
            else:
                chip.configure(fg_color=COLORS["bg-3"], text_color=COLORS["fg-1"])

    def _set_recording_busy(self, busy: bool) -> None:
        """Toggle both record buttons together while one capture is in flight."""
        state = "disabled" if busy else "normal"
        if self.record_button:
            self.record_button.configure(
                state=state,
                text=self.loc.get(
                    "button_start_recording_active" if busy else "studio_record_start"
                ),
            )
        if self.record_headphones_button:
            if busy:
                self.record_headphones_button.configure(state=state)
            else:
                self.record_headphones_button.configure(
                    state=state,
                    text=self.loc.get("button_record_headphones"),
                )

    def _on_complete(self, record_file: str, summary: object) -> None:
        self._set_recording_busy(False)
        if self.recording_feedback:
            self.recording_feedback.complete(record_file, summary)
        self._highlight_segment_chip(None)
        messagebox.showinfo(
            self.loc.get("message_recording_complete_title"),
            self.loc.get("message_recording_complete", file=record_file),
        )

    def _on_error(self, error_msg: str) -> None:
        self._set_recording_busy(False)
        if self.recording_feedback:
            self.recording_feedback.error(error_msg)
        self._highlight_segment_chip(None)
        messagebox.showerror(self.loc.get("message_error"), error_msg)

    def start_recording_headphones(self) -> None:
        """Dedicated headphone-compensation capture path (Studio).

        Mirrors :class:`gui.tabs.recorder_tab.RecorderTab.start_recording_headphones`:
        locks the output to ``headphones.wav``, gates the play file to
        mono or stereo, and warns when a true mono playback is used
        (L=R simultaneous = generic EQ, not per-driver).
        """
        play_file = self.play_var.get()
        record_dir = self.record_dir_var.get().strip()
        if not record_dir:
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_record_folder_required"),
            )
            return

        playback = inspect_headphones_playback(play_file)
        if not playback.is_valid:
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get(playback.reason_key, file=play_file, channels=playback.channels),
            )
            return

        if playback.is_mono:
            if not messagebox.askyesno(
                self.loc.get("message_headphones_mono_warning_title"),
                self.loc.get("message_headphones_mono_warning"),
            ):
                return

        record_file = resolve_headphones_record_path(record_dir)

        info_msg = self.loc.get(
            "message_record_headphones_confirm",
            play_file=os.path.basename(play_file),
            record_file=os.path.basename(record_file),
            input_device=self.input_device_var.get() or "Default",
            output_device=self.output_device_var.get() or "Default",
            host_api=self.host_api_var.get() or "Auto",
        )
        if not messagebox.askyesno(self.loc.get("message_record_headphones_title"), info_msg):
            return

        input_device = self.input_device_var.get()
        output_device = self.output_device_var.get()
        host_api = self.host_api_var.get()
        debug_plots = self.debug_plots_var.get()

        self._set_recording_busy(True)
        if self.recording_feedback:
            self.recording_feedback.start(play_file)

        def report_progress(event):
            self.root.after(0, lambda event=event: self._on_progress_event(event))

        def _run() -> None:
            try:
                # ``mono_to_stereo=True`` only matters when the play file
                # is mono: it duplicates the sweep onto both headphone
                # drivers so the user gets an L=R generic EQ. The user
                # has already been warned about this trade-off above.
                recorder.play_and_record(
                    play=play_file,
                    record=record_file,
                    input_device=input_device,
                    output_device=output_device,
                    host_api=host_api,
                    channels=2,
                    append=False,
                    debug_plots=debug_plots,
                    progress_callback=report_progress,
                    mono_to_stereo=True,
                )
                summary = analyze_recording(record_file)
                self.root.after(0, lambda: self._on_complete(record_file, summary))
            except Exception as e:
                err = str(e)
                self.root.after(0, lambda: self._on_error(err))

        threading.Thread(target=_run, daemon=True).start()
