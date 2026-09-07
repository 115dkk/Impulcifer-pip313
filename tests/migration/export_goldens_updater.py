"""P20: deterministic, offline observations of the unmodified 2.x updater."""
import contextlib
import io
import json
from pathlib import Path
import socket
import sys
import tempfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from application.impulcifer_service import ImpulciferApplicationService
from infra import environment
from updater.executors import LegacyExecutor, VelopackExecutor, UpdateExecutionResult
from updater.update_checker import UpdateChecker


def export():
    cases = []
    versions = [
        ("2.3.1", "v2.3.2"), ("v2.3.1", "v2.3.1-beta"),
        ("2.3.1", "v2.3.1.1"), ("2.3.1.0", "2.3.1"),
        ("2.3.1", "vv2.4.0-20241129123456"), ("2.3.1", "2.3.1.post1"),
        ("3.0.0-alpha.0", "v3.0.0-rc1"), ("2.3.1", "v2.3.0"),
        ("2.3.1", ""), ("2.3.1", "not-a-version"),
        ("garbage", "4.0.0"), ("2.3.1", "V2.4.0"),
        ("1", "2"), ("1.0", "1.0.0"), ("1.0001", "1.2"),
        ("2.0", "2.0.0.1"), ("2.0", "3.0.dev0"),
        ("2.0", "3.0+local"), ("2.0", "3.0."),
        ("2.0", "3..1"), ("2.0", "v"), ("2.0", " 3.0 "),
        ("2.0", "2!1.0"), ("2.0", "v3.0-rc2"),
        ("2.0", "v3.0a1"), ("2.0", "v3.0.0.0.1"),
        ("2.0", "v99999999999999999999.0"), ("2.0", "v0.1"),
        ("2.0", "v02.001"), ("2.0", "v3.0"),
    ]
    asset_sets = [
        ["notes.txt", "Impulcifer-Setup.EXE", "Impulcifer.dmg", "impulcifer.deb", "impulcifer.AppImage"],
        ["portable.exe", "first.pkg", "second.dmg", "impulcifer.rpm"],
        ["releases.win.json", "full.nupkg", "latest.json"],
        ["x.rpm", "x.deb", "x.appimage", "OtherSetup.exe", "SecondSetup.exe"],
        [],
        ["SETUP.EXE", "archive.zip", "portable.exe", "app.PKG"],
    ]
    for index, (current, tag) in enumerate(versions):
        assets = [{"name": name, "browser_download_url": f"https://example.invalid/{index}/{name}"}
                  for name in asset_sets[index % len(asset_sets)]]
        if index == 29:
            assets = [{"name": "Setup.exe"}, {"name": "second-setup.exe", "browser_download_url": "https://example.invalid/second"}, {}]
        release = {"tag_name": tag, "assets": assets, "body": f"Release notes {index}", "html_url": f"https://example.invalid/releases/{index}"}
        checker = UpdateChecker(current)
        selected = {}
        for platform in ("Windows", "Darwin", "Linux", "FreeBSD"):
            with patch("updater.update_checker.platform.system", return_value=platform):
                selected[platform.lower()] = checker._get_download_url(release)
        with contextlib.redirect_stdout(io.StringIO()):
            newer = checker._is_newer_version(tag)
        cases.append(dict(current=current, release=release, normalized_current=checker._normalize_version(current),
                          normalized_latest=checker._normalize_version(tag), newer=newer, selected=selected))

    layouts = []
    with tempfile.TemporaryDirectory(prefix="impulcifer-p20-oracle-") as directory:
        root = Path(directory)
        for present in (False, True):
            app = root / str(present) / "current" / "app.exe"
            app.parent.mkdir(parents=True)
            app.touch()
            if present:
                (app.parent.parent / "Update.exe").touch()
            with patch.object(environment, "is_standalone_build", return_value=True), \
                 patch.object(environment, "is_pip_environment", return_value=False), \
                 patch.object(environment.sys, "executable", str(app)):
                layouts.append(dict(update_exe=present, expected=environment.get_install_kind()))

    results = {}
    for kind, executor in [("legacy", LegacyExecutor("https://example.invalid/Setup.exe", "3.1.0")),
                           ("restart", VelopackExecutor("3.1.0"))]:
        progress = []
        with patch("updater.executors.LegacyInstallerUpdater") as legacy, patch("updater.executors.VelopackUpdater") as velo:
            legacy.return_value.download.return_value = True
            legacy.return_value.install.return_value = True
            velo.return_value.check_and_download.return_value = True
            result = executor.execute(lambda value, message: progress.append(dict(progress=value, message=message)))
        results[kind] = {key: getattr(result, key) for key in
                         ("status_key", "status_default", "title_key", "title_default", "message_key", "message_default", "progress")}
        results[kind]["requires_restart"] = result.after_message is not None
        results[kind + "_progress"] = progress

    validations = []
    requests = [None, [], "bad", 5, False, {}, {"latest_version": None}, {"latest_version": "  "},
                {"latest_version": " 3.1.0 "}, {"latest_version": 42, "download_url": False},
                {"latest_version": True, "download_url": 17}, {"latest_version": [], "download_url": {}},
                {"latest_version": "3.1.0", "download_url": "  https://example.invalid/Setup.exe  ", "extra": 1}]
    for request in requests:
        service = ImpulciferApplicationService()
        captured = {}
        def factory(download_url, latest_version):
            captured.update(download_url=download_url, latest_version=latest_version)
            class Executor:
                def execute(self, progress):
                    return UpdateExecutionResult("status", "status", "title", "title", "message", "message")
            return Executor()
        def start_job(kind, cancellable, run):
            run("offline-job", None)
            return {"ok": True, "data": {"kind": kind, "cancellable": cancellable}}
        with patch.object(service, "_start_job", side_effect=start_job), \
             patch("updater.updater_core.create_update_executor", side_effect=factory):
            envelope = service.start_update(request)
        validations.append(dict(request=request, envelope=envelope, normalized=captured))
    output = ROOT / "tests/migration/goldens/p20_updater.json"
    output.write_text(json.dumps(dict(releases=cases, layouts=layouts, validations=validations, results=results),
                                 ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(f"P20 offline goldens: {len(cases)} releases, {len(layouts)} layouts, {len(validations)} requests, 2 executor results")
    print(f"Wrote {output.as_posix()}")


if __name__ == "__main__":
    # Fail immediately if an oracle refactor ever tries to access the network.
    with patch.object(socket.socket, "connect", side_effect=AssertionError("offline oracle attempted network")):
        export()
