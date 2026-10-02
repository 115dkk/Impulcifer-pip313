"""impulcifer_gui downloads the release app once, checks it and starts it."""
import configparser
import functools
import hashlib
import http.server
import io
import os
import tarfile
import threading
import zipfile

import pytest

ASSETS = {
    "zip": ("Impulcifer-win-Portable.zip", "current/impulcifer-app.exe"),
    "tar": ("Impulcifer-{version}-aarch64.app.tar.gz", "Impulcifer.app"),
    "appimage": ("Impulcifer-{version}-x86_64.AppImage", "Impulcifer.AppImage"),
}


@pytest.fixture
def gui(package):
    from impulcifer import gui

    return gui


class Release:
    """A local stand-in for github.com/.../releases/download/v<version>/."""

    def __init__(self, root):
        self.root = root
        self.requests = []
        release = self

        class Handler(http.server.SimpleHTTPRequestHandler):
            def do_GET(self):
                release.requests.append(self.path)
                super().do_GET()

            def log_message(self, *args):
                pass

        self.server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(Handler, directory=str(root))
        )
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f"http://127.0.0.1:{self.server.server_address[1]}"

    def publish(self, name, data, checksum=None):
        (self.root / name).write_bytes(data)
        sums = self.root / "SHA256SUMS.txt"
        line = f"{checksum or hashlib.sha256(data).hexdigest()}  {name}\n"
        sums.write_text((sums.read_text() if sums.exists() else "") + line)

    def close(self):
        self.server.shutdown()
        self.server.server_close()


@pytest.fixture
def release(tmp_path, monkeypatch, gui):
    served = tmp_path / "release"
    served.mkdir()
    r = Release(served)
    monkeypatch.setenv(gui.BASE_URL_ENV, r.url)
    monkeypatch.setenv(gui.CACHE_ENV, str(tmp_path / "cache"))
    yield r
    r.close()


def archive(kind, entry, extra=None):
    buffer = io.BytesIO()
    if kind == "zip":
        with zipfile.ZipFile(buffer, "w") as zf:
            zf.writestr(entry, b"MZ app")
            zf.writestr("Update.exe", b"MZ updater")
    elif kind == "tar":
        with tarfile.open(fileobj=buffer, mode="w:gz") as tf:
            program = f"{entry}/Contents/MacOS/impulcifer-app"
            for name, data in [(program, b"\xcf\xfa\xed\xfe app"), *(extra or [])]:
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = 0o755
                tf.addfile(info, io.BytesIO(data))
    else:
        return b"\x7fELF AppImage"
    return buffer.getvalue()


def test_targets_are_the_released_app_builds(gui):
    assert gui.target("Windows", "AMD64").asset_name("3.1.0") == "Impulcifer-win-Portable.zip"
    assert gui.target("Darwin", "arm64").asset_name("3.1.0") == "Impulcifer-3.1.0-aarch64.app.tar.gz"
    assert gui.target("Linux", "x86_64").asset_name("3.1.0") == "Impulcifer-3.1.0-x86_64.AppImage"
    # No app is built for Intel Macs or ARM Linux; those installs keep the CLI only.
    assert gui.target("Darwin", "x86_64") is None
    assert gui.target("Linux", "aarch64") is None


@pytest.mark.parametrize("kind", sorted(ASSETS))
def test_first_run_downloads_checks_and_unpacks_then_reuses(gui, release, kind):
    asset, entry = ASSETS[kind]
    name = asset.format(version="9.8.7")
    release.publish(name, archive(kind, entry))
    chosen = gui.Target(asset, kind, entry)
    program = gui.install("9.8.7", chosen)
    assert program.exists()
    assert program == gui.cache_root() / "9.8.7" / entry
    if os.name != "nt":  # Windows has no execute bits to check
        if kind == "appimage":
            assert program.stat().st_mode & 0o111
        if kind == "tar":
            assert (program / "Contents/MacOS/impulcifer-app").stat().st_mode & 0o100
    assert release.requests == ["/SHA256SUMS.txt", f"/{name}"]
    assert gui.install("9.8.7", chosen) == program
    assert len(release.requests) == 2, "a completed install is reused without downloading"
    assert not [p for p in gui.cache_root().iterdir() if p.name.startswith(".")], "staging is removed"


def test_checksum_mismatch_installs_nothing(gui, release):
    asset, entry = ASSETS["appimage"]
    name = asset.format(version="9.8.7")
    release.publish(name, archive("appimage", entry), checksum="0" * 64)
    with pytest.raises(gui.LauncherError, match="does not match SHA256SUMS.txt"):
        gui.install("9.8.7", gui.Target(asset, "appimage", entry))
    assert not (gui.cache_root() / "9.8.7").exists()
    assert not any(gui.cache_root().iterdir())


def test_release_without_the_asset_is_reported(gui, release):
    release.publish("Impulcifer-win-Portable.zip", archive("zip", ASSETS["zip"][1]))
    asset, entry = ASSETS["appimage"]
    with pytest.raises(gui.LauncherError, match="has no Impulcifer-9.8.7-x86_64.AppImage"):
        gui.install("9.8.7", gui.Target(asset, "appimage", entry))


def test_archive_paths_outside_the_cache_are_refused(gui, release):
    asset, entry = ASSETS["tar"]
    name = asset.format(version="9.8.7")
    release.publish(name, archive("tar", entry, extra=[("../escaped", b"x")]))
    with pytest.raises(gui.LauncherError, match="unsafe path"):
        gui.install("9.8.7", gui.Target(asset, "tar", entry))
    assert not (gui.cache_root().parent / "escaped").exists()
    assert not (gui.cache_root() / "9.8.7").exists()


def test_unpublished_release_says_to_wait(gui, release):
    asset, entry = ASSETS["zip"]
    with pytest.raises(gui.LauncherError, match="not published yet"):
        gui.install("9.8.7", gui.Target(asset, "zip", entry))


def test_unreachable_release_names_the_url(gui, release, monkeypatch):
    monkeypatch.setenv(gui.BASE_URL_ENV, "http://127.0.0.1:9")
    asset, entry = ASSETS["zip"]
    with pytest.raises(gui.LauncherError, match="Could not download http://127.0.0.1:9/SHA256SUMS.txt"):
        gui.install("9.8.7", gui.Target(asset, "zip", entry))


def test_commands_wait_for_the_app(gui, monkeypatch, tmp_path):
    app = tmp_path / "Impulcifer.app"
    assert gui.command(app, "tar", system="Darwin", environ={})[0] == ["open", "-W", "-n", str(app)]
    exe = tmp_path / "impulcifer-app.exe"
    assert gui.command(exe, "zip", system="Windows", environ={}) == ([str(exe)], {})
    image = tmp_path / "Impulcifer.AppImage"
    monkeypatch.setattr(gui.ctypes.util, "find_library", lambda name: None)
    argv, env = gui.command(image, "appimage", system="Linux", environ={})
    assert argv == [str(image)]
    assert env["APPIMAGE_EXTRACT_AND_RUN"] == "1", "without FUSE 2 the AppImage unpacks itself"
    monkeypatch.setattr(gui.ctypes.util, "find_library", lambda name: "libfuse.so.2")
    assert "APPIMAGE_EXTRACT_AND_RUN" not in gui.command(image, "appimage", system="Linux", environ={})[1]


def test_download_only_prints_the_program(gui, release, monkeypatch, capsys):
    asset, entry = ASSETS["appimage"]
    release.publish(asset.format(version="9.8.7"), archive("appimage", entry))
    monkeypatch.setattr(gui, "target", lambda: gui.Target(asset, "appimage", entry))
    monkeypatch.setattr(gui, "version", lambda: "9.8.7")
    assert gui.main(["--download-only"]) == 0
    out = capsys.readouterr()
    assert out.out.strip() == str(gui.cache_root() / "9.8.7" / entry)
    assert "Downloading the Impulcifer 9.8.7 app" in out.err


def test_unsupported_platform_says_what_still_works(gui, monkeypatch, capsys):
    monkeypatch.setattr(gui, "target", lambda: None)
    assert gui.main([]) == 2
    assert "Use the impulcifer command" in capsys.readouterr().err


def test_release_url_is_the_version_tag(gui, monkeypatch):
    monkeypatch.delenv(gui.BASE_URL_ENV, raising=False)
    assert gui.base_url("3.1.0-alpha.1") == (
        "https://github.com/115dkk/Impulcifer-pip313/releases/download/v3.1.0-alpha.1"
    )


def test_wheel_declares_the_console_script(wheel):
    with zipfile.ZipFile(wheel) as zf:
        (name,) = [n for n in zf.namelist() if n.endswith(".dist-info/entry_points.txt")]
        parser = configparser.ConfigParser()
        parser.read_string(zf.read(name).decode())
    scripts = dict(parser["console_scripts"])
    assert scripts["impulcifer_gui"] == "impulcifer.gui:cli"
    assert scripts["impulcifer"] == "impulcifer:cli"
