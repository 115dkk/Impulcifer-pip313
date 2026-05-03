"""Unit tests for updater executors separated from GUI dialogs."""

from __future__ import annotations

import subprocess
from typing import Optional

import pytest

from updater import updater_core
from updater.updater_core import PipExecutor, UpdateExecutionError


class FakeProcess:
    """Minimal subprocess object used by PipExecutor tests."""

    def __init__(self, returncode: int, stderr: bytes = b"") -> None:
        """Store process results for communicate()."""
        self.returncode = returncode
        self.stderr = stderr
        self.killed = False

    def communicate(self, timeout: Optional[int] = None) -> tuple[bytes, bytes]:
        """Return captured stdout/stderr."""
        return b"", self.stderr

    def kill(self) -> None:
        """Record that kill was requested."""
        self.killed = True


def test_pip_executor_success(monkeypatch: pytest.MonkeyPatch) -> None:
    """PipExecutor returns a structured success result."""
    monkeypatch.setattr(
        updater_core.subprocess,
        "Popen",
        lambda *args, **kwargs: FakeProcess(returncode=0),
    )
    progress: list[tuple[float, str]] = []

    result = PipExecutor(timeout=1).execute(lambda value, message: progress.append((value, message)))

    assert result.status_key == "update_success"
    assert result.message_key == "update_complete_message"
    assert progress[0] == (0.3, "update_preparing")
    assert progress[-1] == (0.7, "update_installing")


def test_pip_executor_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    """PipExecutor raises a typed error when pip exits nonzero."""
    monkeypatch.setattr(
        updater_core.subprocess,
        "Popen",
        lambda *args, **kwargs: FakeProcess(returncode=2, stderr=b"boom"),
    )

    with pytest.raises(UpdateExecutionError, match="exit code 2"):
        PipExecutor(timeout=1).execute(lambda value, message: None)


def test_pip_executor_timeout(monkeypatch: pytest.MonkeyPatch) -> None:
    """PipExecutor kills the child process when communicate times out."""
    process = FakeProcess(returncode=1)

    def raise_timeout(timeout: Optional[int] = None) -> tuple[bytes, bytes]:
        raise subprocess.TimeoutExpired("pip", timeout)

    process.communicate = raise_timeout  # type: ignore[method-assign]
    monkeypatch.setattr(updater_core.subprocess, "Popen", lambda *args, **kwargs: process)

    with pytest.raises(UpdateExecutionError, match="timed out"):
        PipExecutor(timeout=1).execute(lambda value, message: None)

    assert process.killed is True
