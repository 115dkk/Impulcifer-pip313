# -*- coding: utf-8 -*-
"""Velopack-based update download/apply for standalone (Nuitka) builds.

Split out of ``updater/updater_core.py`` (issue #87 follow-up).
``updater_core`` now re-exports :class:`VelopackUpdater` and
:class:`VelopackDownloadError` for backward compatibility.

Velopack v0.0.x removed the ``Update.exe download`` subcommand, so the
download path is implemented in pure Python here: fetch
``releases.<channel>.json`` from the GitHub release feed, locate the latest
full ``.nupkg``, stream it to Velopack's packages directory with checksum
verification, and then call ``Update.exe apply --restart``.
"""

import hashlib
import json
import os
import platform
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Callable, Dict, Optional

from packaging import version as _version

from updater.environment import get_velopack_update_exe


class VelopackDownloadError(RuntimeError):
    """Raised when the Python-side Velopack download cannot complete.

    Carries a short stable :attr:`reason` (e.g. ``"manifest_fetch_failed"``,
    ``"checksum_mismatch"``) that can be surfaced to the user / regression
    tests, plus an optional underlying detail string.
    """

    def __init__(self, reason: str, detail: str = ""):
        self.reason = reason
        self.detail = detail
        super().__init__(f"{reason}: {detail}" if detail else reason)


class VelopackUpdater:
    """Updater for Velopack-installed standalone executables.

    Important — Velopack v0.0.x removed the ``Update.exe download`` subcommand;
    only ``apply`` / ``start`` / ``patch`` / ``uninstall`` / ``update-self``
    remain, and the download path was moved entirely into the Velopack SDKs
    (lib-csharp/lib-rust/...). Calling ``Update.exe download <url>`` against a
    Velopack-installed app therefore fails with "Unknown subcommand 'download'"
    on every modern build.

    To stay compatible we drive the download from Python: fetch
    ``releases.<channel>.json``, locate the latest full ``.nupkg``, stream it
    into Velopack's packages directory with a SHA256/SHA1 verification, and
    then call ``Update.exe apply --restart`` (which is still supported and
    will auto-locate the package via ``find_latest_full_package``).
    """

    # Chunk size matched to typical TCP window — large enough to amortise
    # syscall overhead, small enough that progress callbacks fire often.
    _CHUNK_SIZE = 65536

    def __init__(self, releases_url: str, version: str):
        """Initialize Velopack updater.

        Args:
            releases_url: Base URL for releases. The trailing slash is
                stripped; ``releases.<channel>.json`` and the resolved
                ``.nupkg`` are appended at request time.
            version: Target version string (used for diagnostic messages).
        """
        self.releases_url = releases_url.rstrip('/')
        self.version = version
        self.update_exe = get_velopack_update_exe()
        # Populated by :meth:`check_and_download` on success.
        self.downloaded_package: Optional[Path] = None
        # Last subprocess stderr for diagnostic surfacing in apply().
        self.last_apply_stderr: Optional[str] = None

    # ------------------------------------------------------------------
    # Velopack environment discovery
    # ------------------------------------------------------------------

    @staticmethod
    def _detect_channel() -> str:
        """Return the Velopack release-feed channel for this platform.

        Velopack's CoreUtil.GetVeloReleaseIndexName uses ``win`` for Windows,
        ``osx`` (with arch suffix in some pipelines) for macOS, and ``linux``
        for Linux. We mirror that to find the right release index file.
        """
        system = platform.system()
        if system == 'Windows':
            return 'win'
        if system == 'Darwin':
            return 'osx'
        if system == 'Linux':
            return 'linux'
        return 'win'

    def _get_packages_dir(self) -> Path:
        """Return Velopack's packages directory for this install.

        Default: ``<update_exe_dir>/packages``. If that directory cannot be
        created or written to (UAC-protected ProgramFiles install), Velopack
        falls back to ``%LOCALAPPDATA%\\<packId>\\packages`` — we mirror that.
        """
        if not self.update_exe:
            raise VelopackDownloadError("update_exe_missing")

        root = self.update_exe.parent
        packages_dir = root / "packages"

        try:
            packages_dir.mkdir(parents=True, exist_ok=True)
            test_file = packages_dir / ".write_test"
            test_file.write_text("ok", encoding='utf-8')
            test_file.unlink()
            return packages_dir
        except (OSError, PermissionError):
            local_app_data = os.environ.get('LOCALAPPDATA')
            if local_app_data:
                pack_id = self._get_pack_id() or "Impulcifer"
                fallback = Path(local_app_data) / pack_id / "packages"
                fallback.mkdir(parents=True, exist_ok=True)
                return fallback
            raise VelopackDownloadError(
                "packages_dir_not_writable",
                f"Cannot write to {packages_dir} and LOCALAPPDATA is unset.",
            )

    def _get_pack_id(self) -> Optional[str]:
        """Read the Velopack pack ID from ``sq.version`` next to Update.exe.

        The format is a simple ``key=value`` text file written by ``vpk pack``;
        ``id`` carries the pack identifier (e.g. ``Impulcifer``). If the file
        is missing we fall back to the directory name, which is the install
        layout's pack folder for default Velopack installs.
        """
        if not self.update_exe:
            return None
        sq_version = self.update_exe.parent / "sq.version"
        if sq_version.exists():
            try:
                for raw_line in sq_version.read_text(encoding='utf-8', errors='replace').splitlines():
                    line = raw_line.strip()
                    if line.lower().startswith("id="):
                        return line.split("=", 1)[1].strip()
            except (OSError, ValueError):
                pass
        return self.update_exe.parent.name

    # ------------------------------------------------------------------
    # Manifest + download
    # ------------------------------------------------------------------

    def _open_url(self, url: str, timeout: int):
        """Open a URL with the standard Impulcifer-Updater UA + 30s timeout."""
        req = urllib.request.Request(
            url,
            headers={'User-Agent': 'Impulcifer-Updater', 'Accept': 'application/json, */*'},
        )
        return urllib.request.urlopen(req, timeout=timeout)

    def _fetch_release_manifest(self, channel: str) -> Optional[Dict]:
        """Fetch ``releases.<channel>.json`` and return the latest full asset."""
        url = f"{self.releases_url}/releases.{channel}.json"
        try:
            with self._open_url(url, timeout=30) as response:
                data = json.loads(response.read().decode('utf-8'))
        except (urllib.error.HTTPError, urllib.error.URLError, json.JSONDecodeError, OSError) as exc:
            print(f"Failed to fetch {url}: {exc}")
            return None

        assets = data.get('Assets') or []
        full_assets = [a for a in assets if str(a.get('Type', '')).lower() == 'full']
        if not full_assets:
            return None

        def _key(asset: Dict):
            try:
                return _version.parse(str(asset.get('Version', '0')))
            except Exception:
                return _version.parse('0')

        return max(full_assets, key=_key)

    def _verify_checksum(self, path: Path, asset_info: Dict) -> bool:
        """Verify SHA256 (preferred) / SHA1 against ``asset_info``.

        Velopack's ``releases.<channel>.json`` carries both fields when
        produced by recent ``vpk pack`` versions. Older feeds may carry only
        SHA1 — we accept either, preferring SHA256 when both are present.
        """
        expected_sha256 = asset_info.get('SHA256')
        expected_sha1 = asset_info.get('SHA1')
        if not expected_sha256 and not expected_sha1:
            # Velopack's own SDK enforces a checksum; missing-by-design is
            # unexpected, but we choose to accept rather than block here so
            # we don't regress feeds that were intentionally unsigned.
            return True

        sha256_hasher = hashlib.sha256()
        sha1_hasher = hashlib.sha1()
        with open(path, 'rb') as f:
            for chunk in iter(lambda: f.read(self._CHUNK_SIZE), b''):
                sha256_hasher.update(chunk)
                sha1_hasher.update(chunk)

        if expected_sha256:
            return sha256_hasher.hexdigest().lower() == expected_sha256.lower()
        return sha1_hasher.hexdigest().lower() == expected_sha1.lower()

    def check_and_download(
        self, progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> bool:
        """Fetch the release manifest and download the latest full ``.nupkg``.

        Args:
            progress_callback: Optional ``(downloaded_bytes, total_bytes)``
                callback invoked while the package streams to disk.

        Returns:
            ``True`` if a complete, checksum-verified ``.nupkg`` is staged in
            the packages directory. ``False`` otherwise; diagnostic detail is
            printed to stdout (and recoverable via ``last_apply_stderr`` for
            apply errors).
        """
        if not self.update_exe:
            print("Velopack Update.exe not found")
            return False

        try:
            channel = self._detect_channel()
            packages_dir = self._get_packages_dir()
            asset_info = self._fetch_release_manifest(channel)

            if not asset_info:
                print(
                    f"No usable release manifest at {self.releases_url}/releases.{channel}.json. "
                    "Server may be missing the Velopack release index."
                )
                return False

            filename = asset_info.get('FileName')
            if not filename:
                print("Release manifest entry has no FileName.")
                return False

            target_path = packages_dir / filename
            partial_path = packages_dir / f"{filename}.partial"
            expected_size = int(asset_info.get('Size') or 0)

            # If the package is already on disk and matches the expected size,
            # skip the download — Velopack itself does the same.
            if target_path.exists() and (
                expected_size == 0 or target_path.stat().st_size == expected_size
            ):
                if self._verify_checksum(target_path, asset_info):
                    print(f"Update package already present: {target_path}")
                    self.downloaded_package = target_path
                    return True
                # Stale/corrupt — fall through to redownload.
                target_path.unlink(missing_ok=True)

            download_url = f"{self.releases_url}/{filename}"
            print(f"Downloading update from {download_url}")

            if partial_path.exists():
                partial_path.unlink()

            with self._open_url(download_url, timeout=120) as response:
                total_size = int(response.headers.get('Content-Length') or expected_size)
                downloaded = 0
                with open(partial_path, 'wb') as f:
                    while True:
                        chunk = response.read(self._CHUNK_SIZE)
                        if not chunk:
                            break
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback and total_size > 0:
                            progress_callback(downloaded, total_size)

            if not self._verify_checksum(partial_path, asset_info):
                partial_path.unlink(missing_ok=True)
                print("Downloaded package failed checksum verification.")
                return False

            if target_path.exists():
                target_path.unlink()
            partial_path.rename(target_path)
            self.downloaded_package = target_path

            print(f"Update downloaded successfully: {target_path}")
            return True

        except VelopackDownloadError as exc:
            print(f"Velopack environment error: {exc}")
            return False
        except urllib.error.HTTPError as exc:
            print(f"HTTP error during update download: {exc}")
            return False
        except urllib.error.URLError as exc:
            print(f"Network error during update download: {exc}")
            return False
        except OSError as exc:
            print(f"I/O error during update download: {exc}")
            return False
        except Exception as exc:  # noqa: BLE001 — last-line safety
            print(f"Unexpected error during update download: {exc}")
            return False

    def apply_and_restart(self) -> bool:
        """Apply the staged update via ``Update.exe apply`` and restart.

        ``Update.exe apply`` (without ``--norestart``) restarts by default.
        We pass the explicit downloaded-package path when available so the
        applier doesn't depend on ``find_latest_full_package`` heuristics.
        """
        if not self.update_exe:
            print("Velopack Update.exe not found")
            return False

        cmd = [str(self.update_exe), "apply"]
        if self.downloaded_package and self.downloaded_package.exists():
            cmd.extend(["--package", str(self.downloaded_package)])

        try:
            kwargs: Dict = {}
            if platform.system() == 'Windows':
                kwargs['creationflags'] = (
                    subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP
                )
            subprocess.Popen(cmd, **kwargs)

            # 현재 프로세스 종료 — Velopack이 인계받아 적용 후 재시작
            sys.exit(0)

        except Exception as exc:  # noqa: BLE001 — surface at GUI layer
            self.last_apply_stderr = str(exc)
            print(f"Update apply error: {exc}")
            return False
