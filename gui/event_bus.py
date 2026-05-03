#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Small publish/subscribe event bus for modern GUI coordination."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any


class EventBus:
    """Dispatch named GUI events to registered callbacks.

    Callbacks are intentionally simple Python callables. The returned
    unsubscribe function should be retained by short-lived subscribers that may
    be destroyed before the application exits.
    """

    def __init__(self) -> None:
        """Create an empty listener registry."""
        self._listeners: dict[str, list[Callable[..., None]]] = {}

    def on(self, event: str, callback: Callable[..., None]) -> Callable[[], None]:
        """Register a callback for an event.

        Args:
            event: Event name.
            callback: Callable invoked with keyword arguments passed to
                :meth:`emit`.

        Returns:
            A function that removes the callback when called.
        """
        listeners = self._listeners.setdefault(event, [])
        listeners.append(callback)

        def unsubscribe() -> None:
            self.off(event, callback)

        return unsubscribe

    def off(self, event: str, callback: Callable[..., None]) -> None:
        """Remove a callback from an event if it is currently registered."""
        listeners = self._listeners.get(event)
        if not listeners:
            return
        try:
            listeners.remove(callback)
        except ValueError:
            return
        if not listeners:
            del self._listeners[event]

    def emit(self, event: str, **kwargs: Any) -> None:
        """Notify all listeners registered for an event."""
        for callback in list(self._listeners.get(event, [])):
            callback(**kwargs)
