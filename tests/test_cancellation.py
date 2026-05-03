"""Tests for Impulcifer cooperative cancellation helpers."""

from __future__ import annotations

import threading

import pytest

import impulcifer


def test_cancellation_scope_raises_when_event_is_set() -> None:
    """The active cancellation event raises the public CancelledError."""
    cancel_event = threading.Event()

    with impulcifer.cancellation_scope(cancel_event):
        cancel_event.set()
        with pytest.raises(impulcifer.CancelledError):
            impulcifer._check_cancelled()


def test_cancellation_scope_is_reset_after_exit() -> None:
    """Cancellation state does not leak after leaving the scope."""
    cancel_event = threading.Event()
    cancel_event.set()

    with pytest.raises(impulcifer.CancelledError):
        with impulcifer.cancellation_scope(cancel_event):
            impulcifer._check_cancelled()

    impulcifer._check_cancelled()
