#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Automatic updater for Impulcifer
Handles downloading and installing updates
"""

import os
import sys
import subprocess
import urllib.request
import tempfile
import platform
from pathlib import Path
from typing import Optional, Callable


class Updater:
    """Download and install updates"""

    def __init__(self, download_url: str, version: str):
        """
        Initialize updater

        Args:
            download_url: URL to download the installer
            version: Version being installed
        """
        self.download_url = download_url
        self.version = version
        self.download_path: Optional[Path] = None

    def download(self, progress_callback: Optional[Callable[[int, int], None]] = None) -> bool:
        """
        Download the installer

        Args:
            progress_callback: Callback function(downloaded, total) for progress updates

        Returns:
            True if download successful, False otherwise
        """
        try:
            # Determine file extension from URL
            url_parts = self.download_url.split('/')
            filename = url_parts[-1] if url_parts else f"impulcifer_setup_{self.version}.exe"

            # Create temp directory for download
            temp_dir = Path(tempfile.gettempdir()) / "impulcifer_updates"
            temp_dir.mkdir(exist_ok=True)

            self.download_path = temp_dir / filename

            # Download with progress
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
            print(f"Error downloading update: {e}")
            return False

    def install_and_restart(self) -> bool:
        """
        Install the update and restart the application

        Returns:
            True if installation started successfully
        """
        if not self.download_path or not self.download_path.exists():
            print("No installer file found")
            return False

        try:
            system = platform.system()

            if system == 'Windows':
                return self._install_windows()
            elif system == 'Darwin':  # macOS
                return self._install_macos()
            elif system == 'Linux':
                return self._install_linux()
            else:
                print(f"Unsupported platform: {system}")
                return False

        except Exception as e:
            print(f"Error installing update: {e}")
            return False

    def _install_windows(self) -> bool:
        """Install on Windows"""
        try:
            # Run installer silently and exit current program
            # /VERYSILENT /NORESTART /SP- are common Inno Setup flags
            installer_args = [
                str(self.download_path),
                '/VERYSILENT',
                '/NORESTART',
                '/SP-',
            ]

            # Start installer
            subprocess.Popen(
                installer_args,
                creationflags=subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
            )

            # Exit current application to allow installation
            # The installer should handle restarting the new version
            sys.exit(0)

        except Exception as e:
            print(f"Windows installation error: {e}")
            return False

    def _install_macos(self) -> bool:
        """Install on macOS"""
        try:
            # For .dmg: mount and copy
            # For .pkg: run installer
            if str(self.download_path).endswith('.pkg'):
                subprocess.Popen(['open', str(self.download_path)])
            elif str(self.download_path).endswith('.dmg'):
                # Mount DMG and open Finder
                subprocess.Popen(['open', str(self.download_path)])

            # User will need to manually install
            print("Please follow the installation prompts")
            return True

        except Exception as e:
            print(f"macOS installation error: {e}")
            return False

    def _install_linux(self) -> bool:
        """Install on Linux"""
        try:
            path_str = str(self.download_path)

            if path_str.endswith('.deb'):
                # Debian/Ubuntu
                subprocess.Popen(['xdg-open', path_str])
            elif path_str.endswith('.rpm'):
                # RedHat/Fedora
                subprocess.Popen(['xdg-open', path_str])
            elif path_str.endswith('.appimage'):
                # Make executable and run
                os.chmod(self.download_path, 0o755)
                subprocess.Popen([path_str])

            return True

        except Exception as e:
            print(f"Linux installation error: {e}")
            return False

    def cleanup(self):
        """Clean up downloaded files"""
        if self.download_path and self.download_path.exists():
            try:
                self.download_path.unlink()
            except Exception as e:
                print(f"Error cleaning up download: {e}")


if __name__ == '__main__':
    # Test the updater
    print("This module should be imported, not run directly")
    print("Use update_checker.py to check for updates first")
