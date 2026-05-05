# -*- coding: utf-8 -*-
"""Update executor framework — non-GUI execution of an update.

Split out of ``updater/updater_core.py`` (issue #87 follow-up).
``updater_core`` re-exports :class:`UpdateExecutionError`,
:class:`UpdateExecutionResult`, :class:`UpdateExecutor`, the three concrete
executors, and :func:`create_update_executor` / :func:`get_updater` factories
for backward compatibility.

Each :class:`UpdateExecutor` subclass wraps one of the three update backends
(:mod:`updater.velopack`, :mod:`updater.pip_updater`, :mod:`updater.legacy`)
and exposes a uniform :meth:`execute(progress_callback)` →
:class:`UpdateExecutionResult` interface so the GUI dialog code stays
oblivious to which backend is in play.
"""

import subprocess
import sys
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Callable, Optional

from updater.environment import is_pip_environment, is_velopack_environment
from updater.legacy import GITHUB_RELEASES_URL, LegacyInstallerUpdater
from updater.pip_updater import PipUpdater
from updater.velopack import VelopackUpdater


class UpdateExecutionError(RuntimeError):
    """Raised when an update executor cannot complete its work."""


@dataclass
class UpdateExecutionResult:
    """Result returned by a non-UI update executor.

    Attributes:
        status_key: Localization key for the final progress label.
        status_default: Fallback status text.
        title_key: Localization key for the completion dialog title.
        title_default: Fallback completion title.
        message_key: Localization key for the completion dialog body.
        message_default: Fallback completion message.
        progress: Final progress bar value, from 0.0 to 1.0.
        close_delay_ms: Delay before closing the dialog.
        after_message: Optional callable to run after the completion message.
    """

    status_key: str
    status_default: str
    title_key: str
    title_default: str
    message_key: str
    message_default: str
    progress: float = 1.0
    close_delay_ms: int = 1000
    after_message: Optional[Callable[[], bool]] = None


class UpdateExecutor(ABC):
    """Abstract update executor with no GUI dependencies."""

    @abstractmethod
    def execute(self, progress_callback: Callable[[float, str], None]) -> UpdateExecutionResult:
        """Execute an update.

        Args:
            progress_callback: Callback receiving progress from 0.0 to 1.0 and
                either a localization key or display text.

        Returns:
            A structured result for the GUI to present.
        """


class PipExecutor(UpdateExecutor):
    """Upgrade the installed package with pip."""

    def __init__(self, package_name: str = "impulcifer-py313", timeout: int = 300):
        """Initialize a pip executor."""
        self.package_name = package_name
        self.timeout = timeout

    def execute(self, progress_callback: Callable[[float, str], None]) -> UpdateExecutionResult:
        """Run ``pip install --upgrade`` and wait for completion."""
        process = None
        progress_callback(0.3, "update_preparing")
        try:
            upgrade_cmd = [
                sys.executable,
                '-m',
                'pip',
                'install',
                '--upgrade',
                self.package_name,
            ]
            print(f"Upgrading with command: {' '.join(upgrade_cmd)}")
            progress_callback(0.5, "update_installing")
            process = subprocess.Popen(
                upgrade_cmd,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            progress_callback(0.7, "update_installing")
            _stdout, stderr = process.communicate(timeout=self.timeout)

            if process.returncode != 0:
                detail = stderr.decode('utf-8', errors='replace')[:500] if stderr else ''
                raise UpdateExecutionError(
                    f"pip upgrade failed (exit code {process.returncode}):\n{detail}"
                )
        except subprocess.TimeoutExpired as exc:
            if process is not None:
                process.kill()
            raise UpdateExecutionError("Update timed out after 5 minutes.") from exc

        return UpdateExecutionResult(
            status_key="update_success",
            status_default="Update started! Please restart the application.",
            title_key="update_complete_title",
            title_default="Update Complete",
            message_key="update_complete_message",
            message_default=(
                "The update has been started in the background.\n"
                "Please restart the application to use the new version."
            ),
            close_delay_ms=2000,
        )


class VelopackExecutor(UpdateExecutor):
    """Download and apply a Velopack update."""

    def __init__(self, latest_version: str):
        """Initialize a Velopack executor."""
        self.latest_version = latest_version

    def execute(self, progress_callback: Callable[[float, str], None]) -> UpdateExecutionResult:
        """Download the Velopack update and prepare the apply action."""
        updater = VelopackUpdater(GITHUB_RELEASES_URL, self.latest_version)

        # Progress for the .nupkg download streams from 0.1 → 0.8 of the
        # overall bar; the remaining 0.8 → 1.0 covers checksum verify + apply.
        def _download_progress(downloaded: int, total: int) -> None:
            if total <= 0:
                return
            percent = int((downloaded / total) * 100)
            progress_callback(0.1 + (downloaded / total) * 0.7, f"Downloading: {percent}%")

        progress_callback(0.1, "update_downloading")
        if not updater.check_and_download(progress_callback=_download_progress):
            raise UpdateExecutionError(
                "Failed to download update. The release feed may be unreachable, "
                "the package may be missing from the release, or the file failed "
                "checksum verification. Please check your internet connection and "
                "try again, or download the latest installer manually from GitHub."
            )

        progress_callback(0.8, "update_installing")
        return UpdateExecutionResult(
            status_key="update_installing",
            status_default="Applying update...",
            title_key="update_complete_title",
            title_default="Update Ready",
            message_key="update_restart_message",
            message_default=(
                "The application will close to apply the update.\n"
                "It will restart automatically in a few seconds."
            ),
            progress=0.9,
            close_delay_ms=0,
            after_message=updater.apply_and_restart,
        )


class LegacyExecutor(UpdateExecutor):
    """Download and open the legacy installer for macOS/Linux."""

    def __init__(self, download_url: str, latest_version: str):
        """Initialize a legacy installer executor."""
        self.download_url = download_url
        self.latest_version = latest_version

    def execute(self, progress_callback: Callable[[float, str], None]) -> UpdateExecutionResult:
        """Download the installer and open it for the user."""
        if not self.download_url:
            raise UpdateExecutionError(
                "No installer available. Please download manually from GitHub."
            )

        updater = LegacyInstallerUpdater(self.download_url, self.latest_version)

        def download_progress(downloaded: int, total: int) -> None:
            if total <= 0:
                return
            percent = int((downloaded / total) * 100)
            progress_callback(downloaded / total, f"Downloading: {percent}%")

        progress_callback(0.1, "update_downloading")
        if not updater.download(progress_callback=download_progress):
            raise UpdateExecutionError("Failed to download update")

        progress_callback(0.9, "update_installing")
        if not updater.install():
            raise UpdateExecutionError("Failed to open installer")

        return UpdateExecutionResult(
            status_key="update_installing",
            status_default="Opening installer...",
            title_key="update_complete_title",
            title_default="Update Started",
            message_key="update_manual_complete",
            message_default="Please follow the installer prompts to complete the update.",
            close_delay_ms=1000,
        )


def create_update_executor(download_url: str, version: str) -> UpdateExecutor:
    """Create an update executor for the current runtime environment."""
    if is_velopack_environment():
        return VelopackExecutor(version)
    if is_pip_environment():
        return PipExecutor()
    return LegacyExecutor(download_url, version)


def get_updater(download_url: str, version: str):
    """
    Factory function to get the appropriate updater for the current environment.

    Args:
        download_url: URL to download update from
        version: Target version

    Returns:
        Tuple of (updater_instance, updater_type_string)
    """
    if is_velopack_environment():
        return VelopackUpdater(GITHUB_RELEASES_URL, version), "velopack"
    elif is_pip_environment():
        return PipUpdater(), "pip"
    else:
        return LegacyInstallerUpdater(download_url, version), "legacy"
