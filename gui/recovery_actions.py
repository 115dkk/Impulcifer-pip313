"""Shared controller for the Stable and Studio output-recovery tabs."""

from __future__ import annotations

import os
import platform
import subprocess
import threading
from pathlib import Path
from typing import Any

from core.brir_recovery import (
    BrirRecoveryError,
    BrirRecoveryResult,
    recover_brir_outputs,
)
from gui.theme import COLORS


class RecoveryActionsMixin:
    """Run recovery directly from CTk and render its terminal state.

    The pywebview frontend intentionally goes through the application service;
    CTk remains a native adapter over the core operation, matching the rest of
    the legacy/native GUI boundary.
    """

    app: Any
    root: Any
    loc: Any
    dir_path_var: Any
    include_hangloose_var: Any
    restore_button: Any
    status_label: Any
    summary_label: Any
    files_label: Any
    open_button: Any

    def _init_recovery_actions(self) -> None:
        self._recovery_running = False
        self._last_output_dir: str | None = None

    def browse_recovery_directory(self) -> None:
        """Select the output root or Hangloose directory."""
        from gui.utils import browse_directory

        browse_directory(self.dir_path_var)

    def start_recovery(self) -> None:
        """Validate and rebuild missing outputs on a worker thread."""
        if self._recovery_running:
            return
        self._recovery_running = True
        self._last_output_dir = None
        self._render_running()

        directory = self.dir_path_var.get().strip()
        include_hangloose = bool(self.include_hangloose_var.get())

        def _run() -> None:
            try:
                result = recover_brir_outputs(
                    directory,
                    include_hangloose=include_hangloose,
                )
            except BrirRecoveryError as exc:
                code, message = exc.code, str(exc)
                self.root.after(
                    0,
                    lambda code=code, message=message: self._finish_error(code, message),
                )
            except Exception as exc:  # noqa: BLE001 - GUI boundary must surface unexpected failures
                message = str(exc) or exc.__class__.__name__
                self.root.after(
                    0,
                    lambda message=message: self._finish_error("INTERNAL_ERROR", message),
                )
            else:
                self.root.after(0, lambda: self._finish_success(result))

        threading.Thread(target=_run, daemon=True).start()

    def _render_running(self) -> None:
        self.restore_button.configure(
            state="disabled",
            text=self.loc.get("recovery_running_action"),
        )
        self.status_label.configure(
            text=self.loc.get("webview_status_running"),
            text_color=COLORS["accent"],
        )
        self.summary_label.configure(text=self.loc.get("recovery_running_detail"))
        self.files_label.configure(text="")
        self.open_button.grid_remove()

    def _finish_success(self, result: BrirRecoveryResult) -> None:
        self._recovery_running = False
        self._last_output_dir = result.output_dir
        self._restore_button()
        self.status_label.configure(
            text=self.loc.get("webview_status_succeeded"),
            text_color=COLORS["ok"],
        )
        self.summary_label.configure(
            text=self.loc.get(
                "recovery_success_summary",
                source=_source_label(result.source_kind),
                sample_rate=result.sample_rate,
                samples=result.sample_count,
                created=len(result.created_files),
                existing=len(result.existing_files),
            )
        )
        created = _relative_names(result.created_files, result.output_dir)
        existing = _relative_names(result.existing_files, result.output_dir)
        self.files_label.configure(
            text=(
                f"{self.loc.get('recovery_created_label')}: "
                f"{created or self.loc.get('recovery_none')}\n"
                f"{self.loc.get('recovery_existing_label')}: "
                f"{existing or self.loc.get('recovery_none')}"
            )
        )
        self.open_button.grid()

    def _finish_error(self, code: str, message: str) -> None:
        self._recovery_running = False
        self._restore_button()
        self.status_label.configure(
            text=self.loc.get("webview_status_failed"),
            text_color=COLORS["err"],
        )
        self.summary_label.configure(
            text=self.loc.get("recovery_failed_summary", code=code, message=message)
        )
        self.files_label.configure(text="")
        self.open_button.grid_remove()

    def _restore_button(self) -> None:
        self.restore_button.configure(
            state="normal",
            text=self.loc.get("recovery_action"),
        )

    def open_recovery_output(self) -> None:
        """Open the most recently resolved output directory."""
        if not self._last_output_dir or not Path(self._last_output_dir).is_dir():
            return
        try:
            if platform.system() == "Windows":
                os.startfile(self._last_output_dir)
            elif platform.system() == "Darwin":
                subprocess.Popen(["open", self._last_output_dir])
            else:
                subprocess.Popen(["xdg-open", self._last_output_dir])
        except (OSError, subprocess.SubprocessError):
            return

    def get_state(self) -> dict[str, object]:
        """Preserve inputs across language and skin rebuilds."""
        return {
            "dir_path": self.dir_path_var.get(),
            "include_hangloose": bool(self.include_hangloose_var.get()),
        }

    def apply_state(self, state: dict[str, object]) -> None:
        """Restore inputs captured by :meth:`get_state`."""
        if isinstance(state.get("dir_path"), str):
            self.dir_path_var.set(state["dir_path"])
        if isinstance(state.get("include_hangloose"), bool):
            self.include_hangloose_var.set(state["include_hangloose"])


def _source_label(source_kind: str) -> str:
    return {
        "hangloose": "Hangloose",
        "hrir": "hrir.wav",
        "hesuvi": "hesuvi.wav",
        "hrir+hesuvi": "hrir.wav + hesuvi.wav",
    }.get(source_kind, source_kind)


def _relative_names(paths: tuple[str, ...], output_dir: str) -> str:
    root = Path(output_dir)
    names: list[str] = []
    for raw_path in paths:
        path = Path(raw_path)
        try:
            names.append(str(path.relative_to(root)))
        except ValueError:
            names.append(path.name)
    return ", ".join(names)
