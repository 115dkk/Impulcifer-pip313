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
from pathlib import Path
from typing import Optional, Callable


def is_velopack_environment() -> bool:
    """
    Check if running in a Velopack-installed environment.
    Velopack creates Update.exe in the app's parent directory.
    """
    if not getattr(sys, 'frozen', False) and not hasattr(sys, '__compiled__'):
        return False

    app_dir = Path(sys.executable).parent
    # Velopack 구조: {packId}/current/app.exe, {packId}/Update.exe
    update_exe = app_dir.parent / "Update.exe"
    return update_exe.exists()


def get_velopack_update_exe() -> Optional[Path]:
    """Get path to Velopack's Update.exe if available."""
    if not getattr(sys, 'frozen', False) and not hasattr(sys, '__compiled__'):
        return None

    app_dir = Path(sys.executable).parent
    update_exe = app_dir.parent / "Update.exe"

    if update_exe.exists():
        return update_exe
    return None


def is_pip_environment() -> bool:
    """Check if pip is available for package management."""
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
