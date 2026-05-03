#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Automatic updater for Impulcifer
Supports both Velopack (standalone) and pip (development/pip install) environments
"""

import hashlib
import json
import os
import sys
import subprocess
import urllib.error
import urllib.request
import tempfile
import platform
from abc import ABC, abstractmethod
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, Optional

from packaging import version as _version


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
