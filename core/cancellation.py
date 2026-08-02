# -*- coding: utf-8 -*-
"""Cooperative cancellation for BRIR generation.

The cancellation event travels in a :class:`~contextvars.ContextVar` so nested
pipeline code can check it without threading the event through every call.
``impulcifer`` re-exports these names for backward compatibility — the GUI tabs
and the application service historically imported them from there.
"""

import contextlib
from contextvars import ContextVar

_CANCEL_EVENT = ContextVar("impulcifer_cancel_event", default=None)


class CancelledError(RuntimeError):
    """Raised when BRIR generation is cancelled cooperatively."""


@contextlib.contextmanager
def cancellation_scope(cancel_event):
    """Install a cancellation event for the current processing context."""
    token = _CANCEL_EVENT.set(cancel_event)
    try:
        yield
    finally:
        _CANCEL_EVENT.reset(token)


def check_cancelled():
    """Raise :class:`CancelledError` if the active cancellation event is set."""
    cancel_event = _CANCEL_EVENT.get()
    if cancel_event is not None and cancel_event.is_set():
        raise CancelledError("User cancelled BRIR generation.")
