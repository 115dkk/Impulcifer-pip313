"""A-1 regression: worker-thread UI updates in gui/dialogs.py must log
swallowed exceptions instead of silently ``pass``-ing.

These exercise the logging helper directly so no Tk root is required (the
CI/headless environments have no usable Tk display).
"""

from __future__ import annotations

from tkinter import TclError
from unittest.mock import MagicMock

import gui.dialogs as dialogs


def test_run_ui_safe_runs_the_action() -> None:
    called = []
    dialogs._run_ui_safe("noop", lambda: called.append(True))
    assert called == [True]


def test_run_ui_safe_logs_tclerror_as_debug(monkeypatch) -> None:
    """A destroyed-widget TclError during teardown is expected → debug, swallowed."""
    fake_logger = MagicMock()
    monkeypatch.setattr(dialogs, "logger", fake_logger)

    def raise_tcl_error() -> None:
        raise TclError("widget has been destroyed")

    dialogs._run_ui_safe("update_progress", raise_tcl_error)  # must not raise

    assert fake_logger.debug.called
    assert not fake_logger.warning.called


def test_run_ui_safe_logs_unexpected_as_warning(monkeypatch) -> None:
    """Any other exception is a real bug and must surface as a warning."""
    fake_logger = MagicMock()
    monkeypatch.setattr(dialogs, "logger", fake_logger)

    def raise_value_error() -> None:
        raise ValueError("boom")

    dialogs._run_ui_safe("add_log", raise_value_error)  # must not raise

    assert fake_logger.warning.called
