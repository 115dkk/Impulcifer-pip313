"""Shared recording controller for the Stable and Studio recorder tabs."""

from __future__ import annotations

import os
import threading
from tkinter import messagebox
from typing import Any

import core.recorder as recorder
from core.headphones_recording import inspect_headphones_playback
from core.recording_naming import (
    resolve_headphones_record_path,
    resolve_record_path,
    resolve_record_path_for_speakers,
)
from core.recording_validation import validate_recording_setup
from core.sweep_set_generator import generate_sweep_set as generate_sweep_set_files
from gui.recording_status import analyze_recording
from gui.sweep_source import (
    headphones_sweep_selection,
    label_to_code,
    resolve_sweep_selection,
    sweep_summary_text,
)


class RecordingActionsMixin:
    """Orchestrate recording while each skin owns its feedback surfaces."""

    root: Any
    loc: Any
    play_var: Any
    record_dir_var: Any
    resolved_record_var: Any
    sweep_source_var: Any
    sweep_speakers_var: Any
    sweep_layout_var: Any
    sweep_fs_var: Any
    sweep_duration_var: Any
    _sweep_mode_labels: dict[str, str]
    input_device_var: Any
    output_device_var: Any
    host_api_var: Any
    append_var: Any
    debug_plots_var: Any

    def generate_sweep_set(self) -> None:
        """Materialize the surround sweep WAVs in a user-chosen folder.

        Writes per-group stereo files plus one combined 7.1 file. Picks
        the play-file's directory as the default target so the result
        lands beside the rest of Impulcifer's bundled sweeps (``data/``).
        Once the files are written, the play-file picker is auto-pointed
        at the universally playable ``FL,FR`` stereo sweep so the user can
        immediately start recording.
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
            paths = generate_sweep_set_files(target_dir)
        except Exception as exc:  # noqa: BLE001 - GUI boundary surfaces failures
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

    def _sweep_mode(self) -> str:
        """Current sweep source code ("default" / "custom" / "file")."""
        return label_to_code(self._sweep_mode_labels, self.sweep_source_var.get(), "default")

    def _resolve_sweep_spec(self):
        """Validated SweepSpec for the current selection (None = file mode).

        Raises ``ValueError`` for invalid custom parameters.
        """
        return resolve_sweep_selection(
            self._sweep_mode(),
            speakers_text=self.sweep_speakers_var.get(),
            tracks=self.sweep_layout_var.get(),
            fs_text=self.sweep_fs_var.get(),
            duration_text=self.sweep_duration_var.get(),
        )

    def _refresh_resolved_record_path(self) -> None:
        """Recompute the read-only ``<folder>/<derived>.wav`` hint label.

        Runs from a Tk variable trace so any edit to the sweep params /
        ``play_var`` / ``record_dir_var`` updates the preview without
        needing the user to click anything.
        """
        record_dir = self.record_dir_var.get().strip()
        if not record_dir:
            self.resolved_record_var.set("")
            return
        try:
            spec = self._resolve_sweep_spec()
            if spec is not None:
                resolved = resolve_record_path_for_speakers(record_dir, spec.speakers)
            else:
                play_file = self.play_var.get().strip()
                if not play_file:
                    self.resolved_record_var.set("")
                    return
                resolved = resolve_record_path(record_dir, play_file)
        except Exception:  # noqa: BLE001 - preview must tolerate transient Tk edits
            self.resolved_record_var.set("")
            return
        self.resolved_record_var.set(
            self.loc.get("label_record_resolved_path", path=resolved)
        )

    def start_recording(self) -> None:
        """Start recording process."""
        play_file = self.play_var.get()
        record_dir = self.record_dir_var.get().strip()
        if not record_dir:
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_record_folder_required"),
            )
            return
        try:
            sweep_spec = self._resolve_sweep_spec()
        except ValueError as exc:
            messagebox.showerror(self.loc.get("message_error"), str(exc))
            return
        if sweep_spec is not None:
            record_file = resolve_record_path_for_speakers(record_dir, sweep_spec.speakers)
            play_display = sweep_summary_text(self.loc, sweep_spec)
        else:
            record_file = resolve_record_path(record_dir, play_file)
            play_display = self._recording_play_display(play_file)
        selected_channels, force_channels = self._resolve_recording_channels()

        if sweep_spec is None and not os.path.exists(play_file):
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_play_file_not_exist", file=play_file),
            )
            return

        validation = validate_recording_setup(
            record_file,
            selected_channels,
            force_channels,
        )
        if validation and validation.has_mismatch:
            warning_msg = self.loc.get(
                "message_channel_mismatch_body",
                expected_speakers=len(validation.expected_speakers),
                speaker_names=", ".join(validation.expected_speakers),
                expected_channels=validation.expected_channels,
                selected_channels=validation.selected_channels,
            )

            if not messagebox.askyesno(
                self.loc.get("message_channel_mismatch_warning_title"),
                warning_msg,
            ):
                return

        if not self._confirm_recording_start(
            play_display,
            record_file,
            selected_channels,
        ):
            return

        recording_context = self._prepare_speaker_recording(play_display)
        input_device = recording_context["input_device"]
        output_device = recording_context["output_device"]
        host_api = recording_context["host_api"]
        append = recording_context["append"]
        debug_plots = recording_context["debug_plots"]
        progress_context = recording_context["progress_context"]

        def report_progress(event):
            self.root.after(
                0,
                lambda event=event: self._handle_recording_progress(
                    event,
                    progress_context,
                ),
            )

        def run_recording():
            try:
                play_signal = None
                if sweep_spec is not None:
                    from core.sweep_signal import build_sweep_playback

                    play_signal = build_sweep_playback(sweep_spec)
                recorder.play_and_record(
                    play=play_file if play_signal is None else None,
                    play_signal=play_signal,
                    record=record_file,
                    input_device=input_device,
                    output_device=output_device,
                    host_api=host_api,
                    channels=selected_channels,
                    append=append,
                    debug_plots=debug_plots,
                    progress_callback=report_progress,
                )
                if play_signal is not None and not play_signal.spec.is_default_signal():
                    # Custom-parameter captures stay self-describing: the
                    # pipeline resolves <dir>/test.wav before the bundled
                    # default sweep.
                    from core.sweep_signal import write_sidecar

                    write_sidecar(os.path.dirname(record_file), play_signal.estimator)
                summary = analyze_recording(record_file)
                self.root.after(
                    0,
                    lambda: self._finish_recording_success(
                        record_file,
                        summary,
                        progress_context,
                    ),
                )
            except Exception as exc:  # noqa: BLE001 - GUI boundary surfaces failures
                error = str(exc)
                self.root.after(
                    0,
                    lambda: self._finish_recording_error(error, progress_context),
                )

        thread = threading.Thread(target=run_recording, daemon=True)
        thread.start()

    def start_recording_headphones(self) -> None:
        """Run the dedicated headphone-compensation capture path.

        Diverges from :meth:`start_recording` in three ways:

        1. The output filename is locked to ``headphones.wav`` regardless
           of the play file's speaker list (no auto-derivation).
        2. The play file is gated to mono or stereo via
           :func:`core.headphones_recording.inspect_headphones_playback`
           — multi-channel surround sweeps can't drive a stereo headphone
           pair and are rejected outright.
        3. If the play file is true mono, the user gets a warning that
           this only produces a generic L=R compensation (both drivers
           receive the same signal simultaneously). They can still
           continue if that's what they want.
        """
        play_file = self.play_var.get()
        record_dir = self.record_dir_var.get().strip()
        if not record_dir:
            messagebox.showerror(
                self.loc.get("message_error"),
                self.loc.get("message_record_folder_required"),
            )
            return

        # Generated sweeps always play the L→R stereo sequence, so the
        # play-file gating and mono warning only apply in file mode.
        try:
            sweep_spec = headphones_sweep_selection(
                self._sweep_mode(),
                fs_text=self.sweep_fs_var.get(),
                duration_text=self.sweep_duration_var.get(),
            )
        except ValueError as exc:
            messagebox.showerror(self.loc.get("message_error"), str(exc))
            return

        if sweep_spec is None:
            playback = inspect_headphones_playback(play_file)
            if not playback.is_valid:
                messagebox.showerror(
                    self.loc.get("message_error"),
                    self.loc.get(
                        playback.reason_key,
                        file=play_file,
                        channels=playback.channels,
                    ),
                )
                return

            if playback.is_mono:
                if not messagebox.askyesno(
                    self.loc.get("message_headphones_mono_warning_title"),
                    self.loc.get("message_headphones_mono_warning"),
                ):
                    return
            play_display = os.path.basename(play_file)
        else:
            play_display = sweep_summary_text(self.loc, sweep_spec)

        record_file = resolve_headphones_record_path(record_dir)

        info_msg = self.loc.get(
            "message_record_headphones_confirm",
            play_file=play_display,
            record_file=os.path.basename(record_file),
            input_device=self.input_device_var.get() or "Default",
            output_device=self.output_device_var.get() or "Default",
            host_api=self.host_api_var.get() or "Auto",
        )
        if not messagebox.askyesno(
            self.loc.get("message_record_headphones_title"),
            info_msg,
        ):
            return

        recording_context = self._prepare_headphones_recording(play_display)
        input_device = recording_context["input_device"]
        output_device = recording_context["output_device"]
        host_api = recording_context["host_api"]
        debug_plots = recording_context["debug_plots"]
        progress_context = recording_context["progress_context"]

        def report_progress(event):
            self.root.after(
                0,
                lambda event=event: self._handle_recording_progress(
                    event,
                    progress_context,
                ),
            )

        def run_recording():
            try:
                play_signal = None
                if sweep_spec is not None:
                    from core.sweep_signal import build_sweep_playback

                    play_signal = build_sweep_playback(sweep_spec)
                # Always 2-channel recording (the two in-ear mics) —
                # speaker-side ``force channels`` is irrelevant here. File
                # mode only: a mono play file is broadcast onto both output
                # headphone drivers (L=R generic EQ, warned above);
                # generated sweeps are already stereo sequences.
                recorder.play_and_record(
                    play=play_file if play_signal is None else None,
                    play_signal=play_signal,
                    record=record_file,
                    input_device=input_device,
                    output_device=output_device,
                    host_api=host_api,
                    channels=2,
                    append=False,
                    debug_plots=debug_plots,
                    progress_callback=report_progress,
                    mono_to_stereo=play_signal is None,
                )
                summary = analyze_recording(record_file)
                self.root.after(
                    0,
                    lambda: self._finish_recording_success(
                        record_file,
                        summary,
                        progress_context,
                    ),
                )
            except Exception as exc:  # noqa: BLE001 - GUI boundary surfaces failures
                error = str(exc)
                self.root.after(
                    0,
                    lambda: self._finish_recording_error(error, progress_context),
                )

        thread = threading.Thread(target=run_recording, daemon=True)
        thread.start()

    def _prepare_speaker_recording(self, play_display: str) -> dict[str, Any]:
        """Prepare speaker recording in the skin's original operation order."""
        raise NotImplementedError

    def _prepare_headphones_recording(self, play_display: str) -> dict[str, Any]:
        """Prepare headphone recording in the skin's original operation order."""
        raise NotImplementedError

    def _recording_play_display(self, play_file: str) -> str:
        """Return the skin's file-mode recording feedback label."""
        raise NotImplementedError

    def _resolve_recording_channels(self) -> tuple[int, bool]:
        """Resolve the skin's channel controls to core recording arguments."""
        raise NotImplementedError

    def _confirm_recording_start(
        self,
        play_display: str,
        record_file: str,
        selected_channels: int,
    ) -> bool:
        """Confirm or immediately accept the speaker recording setup."""
        raise NotImplementedError

    def _handle_recording_progress(self, event: object, context: Any) -> None:
        """Render one core progress event on the UI thread."""
        raise NotImplementedError

    def _finish_recording_success(
        self,
        record_file: str,
        summary: object,
        context: Any,
    ) -> None:
        """Render successful completion on the UI thread."""
        raise NotImplementedError

    def _finish_recording_error(self, error_msg: str, context: Any) -> None:
        """Render failed completion on the UI thread."""
        raise NotImplementedError

    def _set_recording_busy(self, busy: bool) -> None:
        """Toggle the skin's recording controls."""
        raise NotImplementedError
