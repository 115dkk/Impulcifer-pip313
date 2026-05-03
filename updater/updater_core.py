#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Automatic updater for Impulcifer
Supports both Velopack (standalone) and pip (development/pip install) environments
"""

import os
import sys
import subprocess
import urllib.request
import tempfile
import platform
from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Callable


def _is_standalone_build() -> bool:
    """빌드 마커 기반 스탠드얼론(Nuitka) 빌드 감지."""
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "standalone"
    except ImportError:
        pass
    # 폴백: 기존 런타임 감지 (마커 없는 구 빌드 호환)
    if getattr(sys, 'frozen', False):
        return True
    if '__nuitka__' in sys.modules:
        return True
    return False


def is_velopack_environment() -> bool:
    """
    Check if running in a Velopack-installed environment.
    Velopack creates Update.exe in the app's parent directory.
    """
    if not _is_standalone_build():
        return False

    app_dir = Path(sys.executable).parent
    # Velopack 구조: {packId}/current/app.exe, {packId}/Update.exe
    update_exe = app_dir.parent / "Update.exe"
    return update_exe.exists()


def get_velopack_update_exe() -> Optional[Path]:
    """Get path to Velopack's Update.exe if available."""
    if not _is_standalone_build():
        return None

    app_dir = Path(sys.executable).parent
    update_exe = app_dir.parent / "Update.exe"

    if update_exe.exists():
        return update_exe
    return None


def is_pip_environment() -> bool:
    """Check if running as a pip-installed package."""
    # 빌드 마커 우선
    try:
        from infra._build_info import BUILD_TYPE
        return BUILD_TYPE == "pip"
    except ImportError:
        pass
    # 스탠드얼론 빌드에 번들된 pip은 무시
    if _is_standalone_build():
        return False
    # 폴백: 기존 pip 확인 로직
    try:
        import pip  # noqa: F401
        return True
    except ImportError:
        pass

    try:
        result = subprocess.run(
            [sys.executable, '-m', 'pip', '--version'],
            capture_output=True,
            timeout=10
        )
        return result.returncode == 0
    except Exception:
        return False


class VelopackUpdater:
    """Updater for Velopack-installed standalone executables."""

    def __init__(self, releases_url: str, version: str):
        """
        Initialize Velopack updater.

        Args:
            releases_url: Base URL for releases (e.g., GitHub releases URL)
            version: Target version string
        """
        self.releases_url = releases_url
        self.version = version
        self.update_exe = get_velopack_update_exe()

    def check_and_download(self, progress_callback: Optional[Callable[[int, int], None]] = None) -> bool:
        """
        Check for updates and download if available.

        Args:
            progress_callback: Optional callback for progress updates

        Returns:
            True if update is downloaded and ready to apply
        """
        if not self.update_exe:
            print("Velopack Update.exe not found")
            return False

        try:
            # Velopack CLI로 업데이트 확인 및 다운로드
            result = subprocess.run(
                [str(self.update_exe), "download", self.releases_url],
                capture_output=True,
                text=True,
                timeout=120
            )

            if result.returncode == 0:
                print(f"Update downloaded successfully: {result.stdout}")
                return True
            else:
                print(f"Update download failed: {result.stderr}")
                return False

        except subprocess.TimeoutExpired:
            print("Update download timed out")
            return False
        except Exception as e:
            print(f"Update download error: {e}")
            return False

    def apply_and_restart(self) -> bool:
        """
        Apply downloaded update and restart the application.
        This method does not return if successful.

        Returns:
            False if application failed (method returns only on failure)
        """
        if not self.update_exe:
            print("Velopack Update.exe not found")
            return False

        try:
            # Velopack이 앱을 종료하고 업데이트 적용 후 재시작함
            subprocess.Popen(
                [str(self.update_exe), "apply", "--restart"],
                creationflags=subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
                if platform.system() == 'Windows' else 0
            )

            # 현재 프로세스 종료
            sys.exit(0)

        except Exception as e:
            print(f"Update apply error: {e}")
            return False


class PipUpdater:
    """Updater for pip-installed packages."""

    def __init__(self, package_name: str = "impulcifer-py313"):
        """
        Initialize pip updater.

        Args:
            package_name: Name of the package on PyPI
        """
        self.package_name = package_name

    def upgrade(self) -> bool:
        """
        Upgrade the package using pip.

        Returns:
            True if upgrade process started successfully
        """
        try:
            upgrade_cmd = [
                sys.executable,
                '-m', 'pip', 'install', '--upgrade',
                self.package_name
            ]

            print(f"Upgrading with command: {' '.join(upgrade_cmd)}")

            if platform.system() == 'Windows':
                subprocess.Popen(
                    upgrade_cmd,
                    creationflags=subprocess.CREATE_NEW_CONSOLE
                )
            else:
                subprocess.Popen(
                    upgrade_cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE
                )

            return True

        except Exception as e:
            print(f"Pip upgrade error: {e}")
            return False


class LegacyInstallerUpdater:
    """Legacy updater for downloading and running installer files (macOS/Linux)."""

    def __init__(self, download_url: str, version: str):
        self.download_url = download_url
        self.version = version
        self.download_path: Optional[Path] = None

    def download(self, progress_callback: Optional[Callable[[int, int], None]] = None) -> bool:
        """Download the installer file."""
        try:
            url_parts = self.download_url.split('/')
            filename = url_parts[-1] if url_parts else f"impulcifer_update_{self.version}"

            temp_dir = Path(tempfile.gettempdir()) / "impulcifer_updates"
            temp_dir.mkdir(exist_ok=True)
            self.download_path = temp_dir / filename

            req = urllib.request.Request(
                self.download_url,
                headers={'User-Agent': 'Impulcifer-Updater'}
            )

            with urllib.request.urlopen(req) as response:
                total_size = int(response.headers.get('Content-Length', 0))
                downloaded = 0
                chunk_size = 8192

                with open(self.download_path, 'wb') as f:
                    while True:
                        chunk = response.read(chunk_size)
                        if not chunk:
                            break
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback and total_size > 0:
                            progress_callback(downloaded, total_size)

            return True

        except Exception as e:
            print(f"Download error: {e}")
            return False

    def install(self) -> bool:
        """Run the downloaded installer."""
        if not self.download_path or not self.download_path.exists():
            return False

        system = platform.system()

        try:
            if system == 'Darwin':  # macOS
                subprocess.Popen(['open', str(self.download_path)])
                return True
            elif system == 'Linux':
                path_str = str(self.download_path)
                if path_str.endswith('.appimage'):
                    os.chmod(self.download_path, 0o755)
                    subprocess.Popen([path_str])
                else:
                    subprocess.Popen(['xdg-open', path_str])
                return True
        except Exception as e:
            print(f"Install error: {e}")

        return False


# GitHub Releases URL 상수
GITHUB_RELEASES_URL = "https://github.com/115dkk/Impulcifer-pip313/releases/latest/download"


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
        progress_callback(0.3, "update_downloading")
        if not updater.check_and_download():
            raise UpdateExecutionError("Failed to download update")

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


# Legacy compatibility: Keep Updater class for backward compatibility
class Updater:
    """Legacy Updater class for backward compatibility. Use get_updater() instead."""

    def __init__(self, download_url: str, version: str):
        self.download_url = download_url
        self.version = version
        self._legacy = LegacyInstallerUpdater(download_url, version)

    def download(self, progress_callback: Optional[Callable[[int, int], None]] = None) -> bool:
        return self._legacy.download(progress_callback)

    def install_and_restart_legacy(self) -> bool:
        return self._legacy.install()


if __name__ == '__main__':
    print("Updater environment detection:")
    print(f"  Velopack environment: {is_velopack_environment()}")
    print(f"  Pip environment: {is_pip_environment()}")
    print(f"  Update.exe path: {get_velopack_update_exe()}")
