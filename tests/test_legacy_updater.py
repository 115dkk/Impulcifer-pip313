"""Tests for macOS/Linux standalone installer updates."""

from __future__ import annotations

import hashlib
import io
import urllib.error
from pathlib import Path

import pytest

from updater import legacy
from updater import update_checker
from updater.legacy import LegacyInstallerUpdater
from updater.update_checker import UpdateChecker


class FakeResponse:
    """Context-managed byte response for patched urlopen calls."""

    def __init__(self, data: bytes, content_length: bool = False) -> None:
        self._stream = io.BytesIO(data)
        self.headers = {'Content-Length': str(len(data))} if content_length else {}

    def __enter__(self) -> FakeResponse:
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self, size: int = -1) -> bytes:
        return self._stream.read(size)


def _prepare_download(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    payload: bytes,
    sums: bytes | Exception,
) -> tuple[LegacyInstallerUpdater, list[tuple[str, int]]]:
    filename = "Impulcifer-9.9.9-x86_64.AppImage"
    download_url = f"https://example.test/release/{filename}"
    calls: list[tuple[str, int]] = []

    def fake_urlopen(request, timeout):
        calls.append((request.full_url, timeout))
        if request.full_url == download_url:
            return FakeResponse(payload, content_length=True)
        if isinstance(sums, Exception):
            raise sums
        return FakeResponse(sums)

    monkeypatch.setattr(legacy.tempfile, "gettempdir", lambda: str(tmp_path))
    monkeypatch.setattr(legacy.urllib.request, "urlopen", fake_urlopen)
    return LegacyInstallerUpdater(download_url, "9.9.9"), calls


def test_download_passes_timeout_to_urlopen(monkeypatch, tmp_path):
    """Installer and checksum requests both use the bounded timeout."""
    payload = b"appimage"
    digest = hashlib.sha256(payload).hexdigest()
    updater, calls = _prepare_download(
        monkeypatch,
        tmp_path,
        payload,
        f"{digest}  Impulcifer-9.9.9-x86_64.AppImage\n".encode(),
    )

    assert updater.download() is True
    assert [timeout for _, timeout in calls] == [30, 30]


def test_download_accepts_matching_checksum(monkeypatch, tmp_path):
    """A matching SHA256SUMS entry accepts the downloaded installer."""
    payload = b"verified-appimage"
    digest = hashlib.sha256(payload).hexdigest()
    updater, _ = _prepare_download(
        monkeypatch,
        tmp_path,
        payload,
        f"{digest} *Impulcifer-9.9.9-x86_64.AppImage\n".encode(),
    )

    assert updater.download() is True
    assert updater.download_path is not None
    assert updater.download_path.read_bytes() == payload


def test_download_rejects_mismatched_checksum(monkeypatch, tmp_path):
    """A checksum mismatch fails and removes the downloaded installer."""
    updater, _ = _prepare_download(
        monkeypatch,
        tmp_path,
        b"tampered-appimage",
        f"{'0' * 64}  Impulcifer-9.9.9-x86_64.AppImage\n".encode(),
    )

    assert updater.download() is False
    assert updater.download_path is not None
    assert updater.download_path.exists() is False


def test_download_skips_checksum_only_on_http_404(monkeypatch, tmp_path):
    """Only a 404 (release predates SHA256SUMS publication) is fail-open."""
    updater, _ = _prepare_download(
        monkeypatch,
        tmp_path,
        b"unsigned-appimage",
        urllib.error.HTTPError("https://example.test/SHA256SUMS.txt", 404, "Not Found", None, None),
    )

    assert updater.download() is True
    assert updater.download_path is not None
    assert updater.download_path.exists()


def test_download_fails_closed_when_sums_fetch_errors(monkeypatch, tmp_path):
    """Non-404 checksum fetch failures refuse the unverified install."""
    updater, _ = _prepare_download(
        monkeypatch,
        tmp_path,
        b"unsigned-appimage",
        urllib.error.URLError("connection reset"),
    )

    assert updater.download() is False
    assert updater.download_path is not None
    assert updater.download_path.exists() is False


def test_download_fails_closed_when_entry_missing(monkeypatch, tmp_path):
    """A published SHA256SUMS without our asset's entry refuses the install."""
    updater, _ = _prepare_download(
        monkeypatch,
        tmp_path,
        b"unsigned-appimage",
        ("0" * 64 + "  some-other-asset.dmg\n").encode(),
    )

    assert updater.download() is False
    assert updater.download_path is not None
    assert updater.download_path.exists() is False


def test_install_recognizes_capitalized_appimage(monkeypatch, tmp_path):
    """Capitalized AppImage extensions execute directly instead of via xdg-open."""
    appimage = tmp_path / "X.AppImage"
    appimage.write_bytes(b"appimage")
    updater = LegacyInstallerUpdater("https://example.test/X.AppImage", "9.9.9")
    updater.download_path = appimage
    chmod_calls: list[tuple[object, int]] = []
    popen_calls: list[list[str]] = []

    monkeypatch.setattr(legacy.platform, "system", lambda: "Linux")
    monkeypatch.delenv("APPIMAGE", raising=False)
    monkeypatch.setattr(legacy.os, "chmod", lambda path, mode: chmod_calls.append((path, mode)))
    monkeypatch.setattr(legacy.subprocess, "Popen", lambda args: popen_calls.append(args))

    assert updater.install() is True
    assert chmod_calls == [(appimage, 0o755)]
    assert popen_calls == [[str(appimage)]]


def test_install_replaces_running_appimage(monkeypatch, tmp_path):
    """An AppImage runtime update atomically replaces and restarts its installed file."""
    downloaded = tmp_path / "X.AppImage"
    downloaded.write_bytes(b"new-appimage")
    current = tmp_path / "Impulcifer.AppImage"
    current.write_bytes(b"old-appimage")
    updater = LegacyInstallerUpdater("https://example.test/X.AppImage", "9.9.9")
    updater.download_path = downloaded
    replace_calls: list[tuple[object, object]] = []
    popen_calls: list[list[str]] = []

    monkeypatch.setattr(legacy.platform, "system", lambda: "Linux")
    monkeypatch.setenv("APPIMAGE", str(current))
    monkeypatch.setattr(
        legacy.os,
        "replace",
        lambda source, target: replace_calls.append((source, target)),
    )
    monkeypatch.setattr(legacy.subprocess, "Popen", lambda args: popen_calls.append(args))

    assert updater.install() is True
    assert len(replace_calls) == 1
    tmp_source, replace_target = replace_calls[0]
    assert replace_target == str(current)
    assert Path(tmp_source).parent == current.parent
    assert Path(tmp_source).name.startswith(current.name + ".")
    assert Path(tmp_source).name.endswith(".new")
    assert popen_calls == [[str(current)]]


def test_get_download_url_prefers_linux_appimage(monkeypatch):
    """Linux selects an AppImage before deb/rpm assets regardless of list order."""
    monkeypatch.setattr(update_checker.platform, "system", lambda: "Linux")
    release = {
        'assets': [
            {'name': 'Impulcifer.deb', 'browser_download_url': 'https://example.test/linux.deb'},
            {
                'name': 'Impulcifer-9.9.9-x86_64.AppImage',
                'browser_download_url': 'https://example.test/Impulcifer.AppImage',
            },
        ]
    }

    assert UpdateChecker("1.0.0")._get_download_url(release) == (
        'https://example.test/Impulcifer.AppImage'
    )


def test_get_download_url_returns_none_without_match(monkeypatch):
    """An unrelated first release asset is never used as an installer fallback."""
    monkeypatch.setattr(update_checker.platform, "system", lambda: "Linux")
    release = {
        'assets': [
            {'name': 'assets.win.json', 'browser_download_url': 'https://example.test/assets.win.json'}
        ]
    }

    assert UpdateChecker("1.0.0")._get_download_url(release) is None


def test_get_download_url_selects_macos_dmg(monkeypatch):
    """macOS selects its DMG installer asset."""
    monkeypatch.setattr(update_checker.platform, "system", lambda: "Darwin")
    release = {
        'assets': [
            {'name': 'SHA256SUMS.txt', 'browser_download_url': 'https://example.test/sums'},
            {
                'name': 'Impulcifer-9.9.9-macOS.dmg',
                'browser_download_url': 'https://example.test/Impulcifer.dmg',
            },
        ]
    }

    assert UpdateChecker("1.0.0")._get_download_url(release) == (
        'https://example.test/Impulcifer.dmg'
    )
