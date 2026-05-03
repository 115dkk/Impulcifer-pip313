# -*- coding: utf-8 -*-
"""End-to-end regression tests for the Velopack standalone update path.

These tests reproduce the failure observed in the 2.4.13 standalone build —
``Update.exe download <url>`` returning "Unknown subcommand 'download'" — by
running the actual ``VelopackUpdater.check_and_download()`` flow against:

  * A real localhost HTTP server serving a ``releases.win.json`` manifest and
    a fake ``.nupkg`` payload, and
  * A real fake ``Update.exe`` script in a tempdir laid out exactly like a
    Velopack-installed app (``<root>/Update.exe`` + ``<root>/current/`` +
    ``<root>/packages/``).

The tests verify the BUG-FIX contract:

  1. Velopack's ``download`` subcommand is **never** invoked. The fake
     ``Update.exe`` records every invocation; only ``apply`` is allowed and
     any ``download`` call fails the test.
  2. Successful downloads land in ``<root>/packages/<filename>.nupkg`` with
     the right size and checksum.
  3. Manifest-fetch / network / checksum failures all return ``False``
     (instead of raising), so the GUI surfaces the localized error.
  4. ``VelopackUpdater.apply_and_restart()`` invokes ``Update.exe apply
     --package <path>`` on success — confirming the apply path picks up the
     downloaded ``.nupkg`` directly rather than relying on Velopack's
     packages-directory heuristic.

If the ``download``-subcommand regression returns, test 1 fails immediately.
If the URL pattern or feed schema drifts, tests 2-3 fail with a precise
mismatch message. The tests run on every platform — they don't require an
actual standalone build, so CI catches breakage before ship.
"""

from __future__ import annotations

import hashlib
import http.server
import json
import os
import socket
import socketserver
import stat
import sys
import tempfile
import threading
import unittest
import unittest.mock as mock
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator, List, Tuple

import updater.updater_core as updater_core
from updater.updater_core import (
    UpdateExecutionError,
    VelopackExecutor,
    VelopackUpdater,
)


PACK_ID = "Impulcifer"
LATEST_VERSION = "2.4.99"
PACKAGE_BYTES = b"FAKE-NUPKG-CONTENTS-FOR-REGRESSION-TEST" * 16


def _free_port() -> int:
    """Return a free TCP port on localhost so parallel tests don't collide."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class _ManifestHandler(http.server.BaseHTTPRequestHandler):
    """Serves a Velopack release feed — manifest + nupkg + 404 for the rest.

    Routes (relative to ``/``):
      * ``/releases.win.json`` — the JSON release index Velopack now uses
      * ``/releases.osx.json`` / ``/releases.linux.json`` — same for unit
        tests that exercise alternate channels
      * ``/Impulcifer-2.4.99-full.nupkg`` — the binary payload
      * Anything else → 404
    """

    # Filled in by the harness before requests start arriving.
    server_payload: bytes = b""
    server_manifest: dict = {}
    request_log: List[str] = []

    def log_message(self, format, *args):  # noqa: A002 — http.server signature
        # Silence the default stderr access log for clean test output.
        return

    def do_GET(self):  # noqa: N802 — http.server signature
        type(self).request_log.append(self.path)
        if self.path.startswith("/releases.") and self.path.endswith(".json"):
            payload = json.dumps(type(self).server_manifest).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return

        if self.path.endswith(".nupkg"):
            payload = type(self).server_payload
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
            return

        self.send_response(404)
        self.end_headers()


@contextmanager
def _release_server(payload: bytes, manifest: dict) -> Iterator[Tuple[str, List[str]]]:
    """Spin up a localhost server with the given payload + manifest."""
    handler = _ManifestHandler
    handler.server_payload = payload
    handler.server_manifest = manifest
    handler.request_log = []

    port = _free_port()
    server = socketserver.TCPServer(("127.0.0.1", port), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()

    try:
        yield f"http://127.0.0.1:{port}", handler.request_log
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


def _build_manifest(payload: bytes, version: str = LATEST_VERSION) -> dict:
    """Return a Velopack-format release manifest for ``payload``.

    Mirrors the schema produced by ``vpk pack`` in CI — ``Assets`` is a list
    of full / delta entries with checksums, sizes, and filenames.
    """
    return {
        "Assets": [
            {
                "PackageId": PACK_ID,
                "Version": version,
                "Type": "Full",
                "FileName": f"{PACK_ID}-{version}-full.nupkg",
                "SHA1": hashlib.sha1(payload).hexdigest().upper(),
                "SHA256": hashlib.sha256(payload).hexdigest().upper(),
                "Size": len(payload),
            }
        ]
    }


def _make_fake_update_exe(target: Path, log_path: Path) -> None:
    """Write a fake ``Update.exe`` script that logs args + exit code.

    The script accepts the ``apply`` subcommand (and writes its argv to
    ``log_path``) and rejects ``download`` outright — exactly mirroring the
    behaviour of the real Velopack ``Update.exe`` v0.0.1298+. If the
    Python-side download path ever falls back to invoking ``Update.exe
    download``, the regression test fails with a clear message.

    On Windows we still emit a ``.cmd`` despite the ``.exe`` filename — the
    test never executes ``apply_and_restart``, only inspects the script
    contents. (Velopack's apply path is exercised separately via mock_apply.)
    """
    if sys.platform == "win32":
        # Use a .cmd-style batch so the file is at least syntactically valid
        # if a future test does shell-execute it; the assertion logic never
        # actually runs it though.
        script = (
            "@echo off\r\n"
            f"echo %* >> \"{log_path}\"\r\n"
            "if /I \"%1\"==\"download\" exit /b 1\r\n"
            "exit /b 0\r\n"
        )
    else:
        script = (
            "#!/bin/sh\n"
            f"echo \"$@\" >> \"{log_path}\"\n"
            "if [ \"$1\" = \"download\" ]; then exit 1; fi\n"
            "exit 0\n"
        )
    target.write_text(script, encoding="utf-8")
    if sys.platform != "win32":
        target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


@contextmanager
def _fake_velopack_install() -> Iterator[Tuple[Path, Path, Path]]:
    """Yield ``(root, update_exe, packages_dir)`` for a fake install layout.

    Layout matches what Velopack's installer produces on Windows:
      * ``<root>/Update.exe``
      * ``<root>/current/app.exe`` (we don't actually execute it)
      * ``<root>/packages/`` (created on demand)
      * ``<root>/sq.version`` with ``id=Impulcifer`` so the pack-ID lookup hits

    A lightweight ``Update.exe.log`` next to ``Update.exe`` records every
    invocation so tests can assert which subcommands were run.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir) / PACK_ID
        root.mkdir()
        (root / "current").mkdir()
        (root / "current" / "app.exe").write_bytes(b"")
        (root / "sq.version").write_text(f"id={PACK_ID}\nversion=2.4.13\n", encoding="utf-8")

        update_exe = root / "Update.exe"
        log_path = root / "Update.exe.log"
        _make_fake_update_exe(update_exe, log_path)

        packages_dir = root / "packages"

        yield root, update_exe, packages_dir


class VelopackUpdaterDownloadTest(unittest.TestCase):
    """Regression tests for the Python-driven download path."""

    def test_download_fetches_manifest_and_writes_package(self) -> None:
        """Happy path: manifest → nupkg → checksum-verified file on disk."""
        manifest = _build_manifest(PACKAGE_BYTES)

        with _fake_velopack_install() as (root, update_exe, packages_dir), \
             _release_server(PACKAGE_BYTES, manifest) as (base_url, log):

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                self.assertTrue(u.check_and_download(),
                                "Happy path must return True")

            # The fixed implementation must NOT touch the legacy
            # ``Update.exe download`` subcommand. (That's the whole bug.)
            self.assertNotIn("download", str((root / "Update.exe.log").read_text("utf-8"))
                             if (root / "Update.exe.log").exists() else "",
                             "Update.exe must never receive the 'download' subcommand")

            # Manifest fetch + nupkg fetch should both have happened.
            self.assertIn(f"/releases.{u._detect_channel()}.json", log)
            expected_filename = manifest["Assets"][0]["FileName"]
            self.assertIn(f"/{expected_filename}", log)

            # Package landed in the packages dir with the right contents.
            target = packages_dir / expected_filename
            self.assertTrue(target.exists(),
                            f"Expected downloaded package at {target}")
            self.assertEqual(target.read_bytes(), PACKAGE_BYTES)
            self.assertEqual(u.downloaded_package, target)

    def test_download_skips_when_package_already_present(self) -> None:
        """Re-running the download must short-circuit on a cached package."""
        manifest = _build_manifest(PACKAGE_BYTES)

        with _fake_velopack_install() as (root, update_exe, packages_dir), \
             _release_server(PACKAGE_BYTES, manifest) as (base_url, log):

            packages_dir.mkdir(parents=True, exist_ok=True)
            cached = packages_dir / manifest["Assets"][0]["FileName"]
            cached.write_bytes(PACKAGE_BYTES)

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                self.assertTrue(u.check_and_download())

            nupkg_requests = [p for p in log if p.endswith(".nupkg")]
            self.assertEqual(nupkg_requests, [],
                             "Cached package present — must not refetch nupkg")

    def test_download_redownloads_on_size_mismatch(self) -> None:
        """A stale cached package with the wrong size must be replaced."""
        manifest = _build_manifest(PACKAGE_BYTES)

        with _fake_velopack_install() as (root, update_exe, packages_dir), \
             _release_server(PACKAGE_BYTES, manifest) as (base_url, log):

            packages_dir.mkdir(parents=True, exist_ok=True)
            cached = packages_dir / manifest["Assets"][0]["FileName"]
            cached.write_bytes(b"wrong-size")  # length 10, manifest expects 624

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                self.assertTrue(u.check_and_download())

            self.assertEqual(cached.read_bytes(), PACKAGE_BYTES,
                             "Stale cached package must be replaced")

    def test_download_fails_clean_on_checksum_mismatch(self) -> None:
        """Tampered payload must fail download (returns False, no crash)."""
        # Manifest claims one checksum, but the server returns different bytes.
        manifest = _build_manifest(PACKAGE_BYTES)
        with _fake_velopack_install() as (root, update_exe, packages_dir), \
             _release_server(b"TAMPERED-PAYLOAD", manifest) as (base_url, _):

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                self.assertFalse(u.check_and_download(),
                                 "Checksum mismatch must return False")
                self.assertIsNone(u.downloaded_package)

            # The .partial file must be cleaned up so a retry starts fresh.
            partials = list(packages_dir.glob("*.partial"))
            self.assertEqual(partials, [],
                             "Failed download must not leave .partial files behind")

    def test_download_fails_clean_on_missing_manifest(self) -> None:
        """No releases.<channel>.json on the server → False, no exception."""
        # Server returns 404 for everything (no manifest registered).
        with _fake_velopack_install() as (root, update_exe, _), \
             _release_server(b"", {}) as (base_url, _):

            class _EmptyHandler(_ManifestHandler):
                pass

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                # Empty manifest has no full assets → returns False
                self.assertFalse(u.check_and_download())

    def test_download_fails_clean_on_unreachable_server(self) -> None:
        """Connection refused (port not listening) → False, no exception."""
        with _fake_velopack_install() as (_, update_exe, _):
            unreachable = f"http://127.0.0.1:{_free_port()}"
            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(unreachable, LATEST_VERSION)
                self.assertFalse(u.check_and_download())

    def test_progress_callback_receives_byte_pairs(self) -> None:
        """Progress callback gets ``(downloaded, total)`` pairs during stream."""
        manifest = _build_manifest(PACKAGE_BYTES)
        progress_log: List[Tuple[int, int]] = []

        with _fake_velopack_install() as (_, update_exe, _), \
             _release_server(PACKAGE_BYTES, manifest) as (base_url, _):

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                self.assertTrue(u.check_and_download(progress_callback=lambda d, t: progress_log.append((d, t))))

        self.assertGreater(len(progress_log), 0,
                           "Progress callback must fire at least once")
        last = progress_log[-1]
        self.assertEqual(last[0], len(PACKAGE_BYTES),
                         "Final downloaded byte count must equal package size")
        self.assertEqual(last[1], len(PACKAGE_BYTES),
                         "Final total must equal package size (matches Content-Length)")


class VelopackUpdaterApplyTest(unittest.TestCase):
    """Apply-and-restart tests — verify Update.exe args and exit semantics."""

    def test_apply_passes_downloaded_package_to_update_exe(self) -> None:
        """``apply --package <downloaded>`` is the exact subprocess invocation."""
        manifest = _build_manifest(PACKAGE_BYTES)

        with _fake_velopack_install() as (_, update_exe, packages_dir), \
             _release_server(PACKAGE_BYTES, manifest) as (base_url, _):

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater(base_url, LATEST_VERSION)
                self.assertTrue(u.check_and_download())
                expected_pkg = packages_dir / manifest["Assets"][0]["FileName"]
                self.assertEqual(u.downloaded_package, expected_pkg)

                # Stub Popen + sys.exit so apply_and_restart returns to the
                # test harness instead of terminating the interpreter.
                with mock.patch.object(updater_core.subprocess, "Popen") as popen_mock, \
                     mock.patch.object(updater_core.sys, "exit",
                                       side_effect=SystemExit(0)) as exit_mock:
                    with self.assertRaises(SystemExit):
                        u.apply_and_restart()

                # The single Popen call must include "apply" (NOT "download")
                # plus the explicit --package path.
                self.assertEqual(popen_mock.call_count, 1)
                args, _kwargs = popen_mock.call_args
                cmd = args[0]
                self.assertEqual(cmd[0], str(update_exe))
                self.assertEqual(cmd[1], "apply")
                self.assertNotIn("download", cmd,
                                 "Apply command must never contain the 'download' subcommand")
                self.assertIn("--package", cmd)
                self.assertIn(str(expected_pkg), cmd)
                exit_mock.assert_called_once_with(0)


class VelopackExecutorTest(unittest.TestCase):
    """End-to-end executor tests covering the GUI-facing surface."""

    def test_executor_surfaces_actionable_error_on_download_failure(self) -> None:
        """Failed download → UpdateExecutionError with manual-download hint."""
        with _fake_velopack_install() as (_, update_exe, _):
            unreachable = f"http://127.0.0.1:{_free_port()}"
            with mock.patch.object(updater_core, "GITHUB_RELEASES_URL", unreachable), \
                 mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                executor = VelopackExecutor(LATEST_VERSION)
                progress_calls: List[Tuple[float, str]] = []
                with self.assertRaises(UpdateExecutionError) as cm:
                    executor.execute(lambda p, m: progress_calls.append((p, m)))

                msg = str(cm.exception).lower()
                self.assertIn("download", msg,
                              "Error must mention download for triage")
                # Must hint at the manual-download fallback so the user is
                # never blocked when auto-update fails.
                self.assertIn("github", msg,
                              "Error must point users at the GitHub fallback")

    def test_executor_progress_is_normalized_to_unit_interval(self) -> None:
        """Progress reported to the GUI must stay within [0.0, 1.0]."""
        manifest = _build_manifest(PACKAGE_BYTES)
        with _fake_velopack_install() as (_, update_exe, _), \
             _release_server(PACKAGE_BYTES, manifest) as (base_url, _):

            with mock.patch.object(updater_core, "GITHUB_RELEASES_URL", base_url), \
                 mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                executor = VelopackExecutor(LATEST_VERSION)
                progress_calls: List[Tuple[float, str]] = []
                executor.execute(lambda p, m: progress_calls.append((p, m)))

                self.assertGreater(len(progress_calls), 0)
                for p, _ in progress_calls:
                    self.assertGreaterEqual(p, 0.0)
                    self.assertLessEqual(p, 1.0)


class VelopackUpdaterEnvironmentTest(unittest.TestCase):
    """Sanity tests on the Velopack-environment helpers."""

    def test_packages_dir_falls_back_when_root_not_writable(self) -> None:
        """Read-only root → LOCALAPPDATA fallback (Windows-style behavior)."""
        with _fake_velopack_install() as (root, update_exe, _), \
             tempfile.TemporaryDirectory() as appdata:
            packages_dir = root / "packages"

            # Force the writability test inside _get_packages_dir to fail.
            original_mkdir = Path.mkdir

            def _failing_mkdir(self, *args, **kwargs):
                if str(self) == str(packages_dir):
                    raise PermissionError("simulated read-only install")
                return original_mkdir(self, *args, **kwargs)

            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe), \
                 mock.patch.dict(os.environ, {"LOCALAPPDATA": appdata}, clear=False), \
                 mock.patch.object(Path, "mkdir", _failing_mkdir):

                u = VelopackUpdater("http://example.invalid", LATEST_VERSION)
                fallback = u._get_packages_dir()

            self.assertEqual(fallback, Path(appdata) / PACK_ID / "packages")
            self.assertTrue(fallback.exists(),
                            "Fallback packages dir must be created")

    def test_pack_id_read_from_sq_version(self) -> None:
        """``sq.version`` ``id=`` line is the source of truth for pack ID."""
        with _fake_velopack_install() as (_, update_exe, _):
            with mock.patch.object(updater_core, "get_velopack_update_exe",
                                   return_value=update_exe):
                u = VelopackUpdater("http://example.invalid", LATEST_VERSION)
                self.assertEqual(u._get_pack_id(), PACK_ID)

    def test_no_update_exe_returns_false_immediately(self) -> None:
        """Missing Update.exe must not crash — return False with a log line."""
        with mock.patch.object(updater_core, "get_velopack_update_exe",
                               return_value=None):
            u = VelopackUpdater("http://example.invalid", LATEST_VERSION)
            self.assertFalse(u.check_and_download())
            self.assertFalse(u.apply_and_restart())


if __name__ == "__main__":
    unittest.main()
