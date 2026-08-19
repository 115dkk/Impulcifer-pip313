# -*- coding: utf-8 -*-
"""Current installer-based update path for macOS/Linux standalone builds."""

import hashlib
import os
import platform
import re
import shutil
import subprocess
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable, Optional


# GitHub Releases base URL — used by both the direct-download path and
# Velopack's release-feed lookup.
GITHUB_RELEASES_URL = "https://github.com/115dkk/Impulcifer-pip313/releases/latest/download"


class LegacyInstallerUpdater:
    """Current installer-based updater for macOS/Linux standalone builds."""

    _TIMEOUT = 30
    _CHUNK_SIZE = 8192

    def __init__(self, download_url: str, version: str):
        self.download_url = download_url
        self.version = version
        self.download_path: Optional[Path] = None

    def download(self, progress_callback: Optional[Callable[[int, int], None]] = None) -> bool:
        """Download the installer file and verify its SHA-256 checksum when available."""
        try:
            url_path = urllib.parse.urlsplit(self.download_url).path
            filename = Path(urllib.parse.unquote(url_path)).name
            if not filename:
                filename = f"impulcifer_update_{self.version}"

            temp_dir = Path(tempfile.gettempdir()) / "impulcifer_updates"
            temp_dir.mkdir(exist_ok=True)
            self.download_path = temp_dir / filename

            req = urllib.request.Request(
                self.download_url,
                headers={'User-Agent': 'Impulcifer-Updater'},
            )

            with urllib.request.urlopen(req, timeout=self._TIMEOUT) as response:
                total_size = int(response.headers.get('Content-Length', 0))
                downloaded = 0

                with open(self.download_path, 'wb') as f:
                    while True:
                        chunk = response.read(self._CHUNK_SIZE)
                        if not chunk:
                            break
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback and total_size > 0:
                            progress_callback(downloaded, total_size)

            return self._verify_download_checksum(filename)

        except Exception as e:
            print(f"Download error: {e}")
            return False

    def _verify_download_checksum(self, filename: str) -> bool:
        """Verify the downloaded file against SHA256SUMS.txt when it is available."""
        sums_url = self.download_url.rsplit('/', 1)[0] + '/SHA256SUMS.txt'
        req = urllib.request.Request(
            sums_url,
            headers={'User-Agent': 'Impulcifer-Updater'},
        )

        try:
            with urllib.request.urlopen(req, timeout=self._TIMEOUT) as response:
                sums_text = response.read().decode('utf-8')
        except (urllib.error.HTTPError, urllib.error.URLError, OSError, UnicodeError) as e:
            print(f"Warning: checksum file unavailable; skipping verification: {e}")
            return True

        expected_hash = None
        checksum_line = re.compile(r'^([0-9a-fA-F]{64}) (?: |\*)(.+)$')
        for line in sums_text.splitlines():
            match = checksum_line.match(line)
            if match and match.group(2) == filename:
                expected_hash = match.group(1).lower()
                break

        if expected_hash is None:
            print(f"Warning: no checksum found for {filename}; skipping verification")
            return True

        assert self.download_path is not None
        hasher = hashlib.sha256()
        with open(self.download_path, 'rb') as f:
            for chunk in iter(lambda: f.read(self._CHUNK_SIZE), b''):
                hasher.update(chunk)

        if hasher.hexdigest().lower() == expected_hash:
            return True

        try:
            self.download_path.unlink()
        except OSError as e:
            print(f"Warning: failed to remove checksum-mismatched download: {e}")
        print(f"Checksum verification failed for {filename}")
        return False

    def install(self) -> bool:
        """Open the downloaded installer or replace the running AppImage."""
        if not self.download_path or not self.download_path.exists():
            return False

        system = platform.system()

        try:
            if system == 'Darwin':
                subprocess.Popen(['open', str(self.download_path)])
                return True
            if system == 'Linux':
                path_str = str(self.download_path)
                if path_str.lower().endswith('.appimage'):
                    current_appimage = os.environ.get('APPIMAGE')
                    if current_appimage:
                        try:
                            replacement_path = Path(f"{current_appimage}.new")
                            shutil.copy2(self.download_path, replacement_path)
                            os.chmod(replacement_path, 0o755)
                            os.replace(replacement_path, current_appimage)
                            subprocess.Popen([current_appimage])
                            return True
                        except Exception as e:
                            print(f"AppImage replacement error; launching download instead: {e}")

                    os.chmod(self.download_path, 0o755)
                    subprocess.Popen([path_str])
                else:
                    subprocess.Popen(['xdg-open', path_str])
                return True
        except Exception as e:
            print(f"Install error: {e}")

        return False
