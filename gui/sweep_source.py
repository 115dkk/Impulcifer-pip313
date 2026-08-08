#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Shared skin-agnostic logic for the sweep-source and test-signal controls.

Both CTk skins (Stable tabs and Studio cards) present the same three
recorder sweep sources — generate with default parameters, generate with
custom parameters, play a file — and the same four BRIR test-signal
sources — auto-detect, bundled default, manual parameters, file. The
widgets stay native to each skin; the label↔code mapping, SweepSpec
assembly and argument composition live here so the two skins cannot
drift (same contract as core.recording_naming / recording_validation).
"""

from __future__ import annotations

from typing import Any

# Recorder sweep source codes, in UI order.
RECORDER_SWEEP_MODES = ("default", "custom", "file")
# BRIR test signal source codes, in UI order ("auto" is the default).
TEST_SIGNAL_SOURCES = ("auto", "default", "manual", "file")


def recorder_sweep_mode_labels(loc: Any) -> dict:
    """Localized label per recorder sweep mode code."""
    return {
        "default": loc.get("option_sweep_default"),
        "custom": loc.get("option_sweep_custom"),
        "file": loc.get("option_sweep_file"),
    }


def test_signal_source_labels(loc: Any) -> dict:
    """Localized label per BRIR test-signal source code."""
    return {
        "auto": loc.get("option_test_signal_auto"),
        "default": loc.get("option_test_signal_default"),
        "manual": loc.get("option_test_signal_manual"),
        "file": loc.get("option_test_signal_file"),
    }


def label_to_code(labels: dict, label: str, fallback: str) -> str:
    """Map a displayed OptionMenu label back to its mode code."""
    for code, text in labels.items():
        if text == label:
            return code
    return fallback


def resolve_sweep_selection(
    mode: str,
    *,
    speakers_text: str,
    tracks: str,
    fs_text: str = "",
    duration_text: str = "",
):
    """Build the validated SweepSpec for a recorder sweep selection.

    Returns ``None`` for file mode. Raises ``ValueError`` (with a
    user-presentable message) for invalid custom parameters or speaker
    lists — the same validation the WebView service applies.
    """
    if mode == "file":
        return None

    from core.sweep_signal import (
        DEFAULT_SWEEP_DURATION,
        DEFAULT_SWEEP_FS,
        SweepSpec,
        validate_sweep_spec,
    )

    speakers = tuple(part.strip() for part in str(speakers_text).split(",") if part.strip())
    if mode == "custom":
        fs = int(str(fs_text).strip())
        duration = float(str(duration_text).strip())
    else:
        fs = DEFAULT_SWEEP_FS
        duration = DEFAULT_SWEEP_DURATION
    return validate_sweep_spec(
        SweepSpec(fs=fs, duration=duration, speakers=speakers, tracks=tracks)
    )


def headphones_sweep_selection(mode: str, *, fs_text: str = "", duration_text: str = ""):
    """Sweep selection for the dedicated headphone capture path.

    Headphone compensation always plays the L→R stereo sequence; only the
    signal parameters follow the selected mode. Returns ``None`` for file
    mode (legacy play-file gating applies there).
    """
    return resolve_sweep_selection(
        mode,
        speakers_text="FL,FR",
        tracks="stereo",
        fs_text=fs_text,
        duration_text=duration_text,
    )


def sweep_summary_text(loc: Any, spec: Any) -> str:
    """One-line description of a generated sweep for dialogs/status."""
    return loc.get(
        "label_sweep_generated_summary",
        speakers=",".join(spec.speakers),
        tracks=spec.tracks,
        fs=spec.fs,
        duration=f"{spec.duration:g}",
    )


def resolve_test_signal_arg(tab: Any, loc: Any) -> str:
    """Compose the ``test_signal`` pipeline argument from a BRIR tab.

    Tabs without the source selector (none after 2.11) fall back to the
    raw path entry. Raises ``ValueError`` when manual parameters do not
    parse — callers surface that as a validation dialog.
    """
    source_var = getattr(tab, "test_signal_source_var", None)
    if source_var is None:
        return tab.test_signal_var.get()
    code = label_to_code(test_signal_source_labels(loc), source_var.get(), "auto")
    if code == "auto":
        return "auto"
    if code == "default":
        return "default"
    if code == "manual":
        try:
            duration = float(str(tab.test_signal_duration_var.get()).strip())
            fs = int(str(tab.test_signal_fs_var.get()).strip())
        except (TypeError, ValueError) as exc:
            raise ValueError(loc.get("error_test_signal_manual_invalid")) from exc
        return f"generate:{duration:g}s@{fs}"
    return tab.test_signal_var.get()


def run_detect_preview(tab: Any) -> None:
    """Threaded recordings-folder detection preview shared by both skins.

    Expects the tab to expose ``loc``, ``root``, ``dir_path_var``,
    ``test_signal_duration_var`` and ``test_signal_fs_var``. Shows the
    summary in a messagebox and pre-fills the manual parameter fields so
    the user can switch to manual mode and tweak the detected values.
    """
    import os
    import threading
    from tkinter import messagebox

    loc = tab.loc
    dir_path = tab.dir_path_var.get().strip()
    if not dir_path or not os.path.isdir(dir_path):
        messagebox.showerror(
            loc.get("message_error"),
            loc.get("error_file_not_found", file=dir_path),
        )
        return

    def _detect() -> None:
        from core.sweep_detection import detect_sweep_parameters

        try:
            detection = detect_sweep_parameters(dir_path)
        except Exception as exc:  # pragma: no cover - defensive UI guard
            error = str(exc)
            tab.root.after(0, lambda: messagebox.showerror(loc.get("message_error"), error))
            return

        def _show() -> None:
            summary = detect_summary_text(loc, detection)
            if detection is None:
                messagebox.showwarning(loc.get("dialog_detect_sweep_title"), summary)
                return
            tab.test_signal_duration_var.set(f"{detection.duration_seconds:.2f}")
            tab.test_signal_fs_var.set(str(detection.fs))
            messagebox.showinfo(loc.get("dialog_detect_sweep_title"), summary)

        tab.root.after(0, _show)

    threading.Thread(target=_detect, daemon=True).start()


def detect_summary_text(loc: Any, detection: Any) -> str:
    """User-facing summary of a sweep detection result."""
    if detection is None:
        return loc.get("message_sweep_detect_failed")
    confidence_key = (
        "message_sweep_detect_confidence_high"
        if detection.confidence == "high"
        else "message_sweep_detect_confidence_low"
    )
    return loc.get(
        "message_sweep_detected",
        fs=detection.fs,
        duration=f"{detection.duration_seconds:.2f}",
        segments=detection.n_segments,
        files=", ".join(detection.source_files),
        confidence=loc.get(confidence_key),
    )
