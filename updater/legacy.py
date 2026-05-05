# -*- coding: utf-8 -*-
"""Legacy installer-based updater (DMG/AppImage download + open).

Split out of ``updater/updater_core.py`` (issue #87 follow-up).
``updater_core`` re-exports :class:`LegacyInstallerUpdater`, the legacy-compat
:class:`Updater` shim, and :data:`GITHUB_RELEASES_URL` for backward
compatibility.
"""

import os
import platform
import subprocess
import tempfile
import urllib.request
from pathlib import Path
from typing import Callable, Optional


# GitHub Releases base URL — used by both the legacy direct-download path and
# Velopack's release-feed lookup. Kept in this module because it's most often
# touched alongside LegacyInstallerUpdater.
GITHUB_RELEASES_URL = "https://github.com/115dkk/Impulcifer-pip313/releases/latest/download"


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
