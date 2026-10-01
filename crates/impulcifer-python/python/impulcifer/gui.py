"""``impulcifer_gui``: start the 3.x app from a pip install.

The wheel carries neither the app nor an audio backend (manylinux forbids
libasound), so this command fetches the app of the same version from GitHub
Releases once, checks it against that release's SHA256SUMS.txt, keeps it in the
user cache and starts it. It is the same program as the standalone release:
recording included.
"""
import argparse
import ctypes.util
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile

RELEASES = "https://github.com/115dkk/Impulcifer-pip313/releases"
# Tests and mirrors point these elsewhere; the defaults are the real release and cache.
BASE_URL_ENV = "IMPULCIFER_RELEASE_BASE_URL"
CACHE_ENV = "IMPULCIFER_APP_CACHE"
_MARKER = ".complete"


class LauncherError(RuntimeError):
    """A failure with a message written for the person at the terminal."""


class Target:
    """The release asset for one platform and the program inside it."""

    def __init__(self, asset, kind, entry):
        self.asset = asset  # file name in the release; "{version}" is filled in
        self.kind = kind  # "zip", "tar" or "appimage"
        self.entry = entry  # path of the program inside the unpacked asset

    def asset_name(self, version):
        return self.asset.format(version=version)


TARGETS = {
    ("Windows", "x86_64"): Target("Impulcifer-win-Portable.zip", "zip", "current/impulcifer-app.exe"),
    ("Darwin", "arm64"): Target("Impulcifer-{version}-aarch64.app.tar.gz", "tar", "Impulcifer.app"),
    ("Linux", "x86_64"): Target("Impulcifer-{version}-x86_64.AppImage", "appimage", "Impulcifer.AppImage"),
}
_MACHINES = {"amd64": "x86_64", "x86_64": "x86_64", "x64": "x86_64", "arm64": "arm64", "aarch64": "arm64"}


def target(system=None, machine=None):
    """The app build for this OS and CPU, or None where no app is released."""
    system = system or platform.system()
    machine = _MACHINES.get((machine or platform.machine()).lower(), machine)
    return TARGETS.get((system, machine))


def version():
    """The release tag's version (SemVer, e.g. 3.1.0-alpha.1), not pip's normalised form."""
    from . import impulcifer_native

    return impulcifer_native.version()


def cache_root():
    override = os.environ.get(CACHE_ENV)
    if override:
        return Path(override)
    if sys.platform == "win32":
        base = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local")
    elif sys.platform == "darwin":
        base = Path.home() / "Library" / "Caches"
    else:
        base = Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache")
    return base / "impulcifer-py313" / "app"


def base_url(tag_version):
    override = os.environ.get(BASE_URL_ENV)
    if override:
        return override.rstrip("/")
    return f"{RELEASES}/download/v{tag_version}"


def _fetch(url, destination, log):
    """Stream url to destination and return its SHA-256."""
    digest = hashlib.sha256()
    try:
        with urllib.request.urlopen(url, timeout=60) as response, open(destination, "wb") as out:
            total = int(response.headers.get("Content-Length") or 0)
            done = 0
            last = -1
            while True:
                block = response.read(1 << 20)
                if not block:
                    break
                out.write(block)
                digest.update(block)
                done += len(block)
                if total and log:
                    percent = done * 100 // total
                    if percent // 10 != last // 10:
                        log(f"  {percent}% of {total / 1048576:.0f} MiB")
                        last = percent
    except (urllib.error.URLError, OSError) as error:
        raise LauncherError(
            f"Could not download {url} ({getattr(error, 'reason', error)}). "
            f"Check the connection and run impulcifer_gui again, or download the app from {RELEASES}."
        ) from error
    return digest.hexdigest()


def _checksums(url):
    try:
        with urllib.request.urlopen(url, timeout=60) as response:
            text = response.read().decode("utf-8")
    except (urllib.error.URLError, OSError) as error:
        raise LauncherError(
            f"Could not download {url} ({getattr(error, 'reason', error)}). "
            f"Check the connection and run impulcifer_gui again, or download the app from {RELEASES}."
        ) from error
    sums = {}
    for line in text.splitlines():
        parts = line.split(maxsplit=1)
        if len(parts) == 2:
            sums[parts[1].strip().lstrip("*")] = parts[0].lower()
    return sums


def _inside(root, member):
    resolved = (root / member).resolve()
    return resolved == root or root in resolved.parents


def _unpack(archive, kind, destination, entry):
    root = destination.resolve()
    if kind == "zip":
        with zipfile.ZipFile(archive) as zf:
            for name in zf.namelist():
                if not _inside(root, name):
                    raise LauncherError(f"The app archive has an unsafe path ({name}); nothing was installed.")
            zf.extractall(destination)
    elif kind == "tar":
        with tarfile.open(archive) as tf:
            for member in tf.getmembers():
                link = member.linkname if (member.issym() or member.islnk()) else None
                if not _inside(root, member.name) or (
                    link and not _inside(root, Path(member.name).parent / link)
                ):
                    raise LauncherError(f"The app archive has an unsafe path ({member.name}); nothing was installed.")
            if hasattr(tarfile, "data_filter"):
                # Python 3.12+ and recent 3.9-3.11 patches: also strips unsafe modes.
                tf.extractall(destination, filter="data")
            else:
                tf.extractall(destination)
    else:
        program = destination / entry
        shutil.move(str(archive), program)
        program.chmod(0o755)


def install(tag_version, chosen, root=None, log=None):
    """Return the path of the app for tag_version, downloading it on first use."""
    root = Path(root or cache_root())
    installed = root / tag_version
    program = installed / chosen.entry
    if (installed / _MARKER).is_file() and program.exists():
        return program
    asset = chosen.asset_name(tag_version)
    url = base_url(tag_version)
    expected = _checksums(f"{url}/SHA256SUMS.txt").get(asset)
    if expected is None:
        raise LauncherError(
            f"Release v{tag_version} has no {asset}. "
            f"Download the app from {RELEASES} or install a released version of impulcifer-py313."
        )
    root.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{tag_version}-", dir=root))
    try:
        if log:
            log(f"Downloading the Impulcifer {tag_version} app to {installed}")
        archive = staging / asset
        actual = _fetch(f"{url}/{asset}", archive, log)
        if actual != expected:
            raise LauncherError(
                f"The downloaded {asset} does not match SHA256SUMS.txt of v{tag_version}; "
                "nothing was installed. Run impulcifer_gui again."
            )
        unpacked = staging / "app"
        unpacked.mkdir()
        _unpack(archive, chosen.kind, unpacked, chosen.entry)
        if not (unpacked / chosen.entry).exists():
            raise LauncherError(f"{asset} does not contain {chosen.entry}; nothing was installed.")
        (unpacked / _MARKER).write_text(asset + "\n", encoding="utf-8")
        if installed.exists():
            shutil.rmtree(installed)
        os.replace(unpacked, installed)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
    return program


def command(program, kind, system=None, environ=None):
    """argv and environment that start the unpacked app and wait until it closes."""
    system = system or platform.system()
    env = dict(os.environ if environ is None else environ)
    if system == "Darwin" and kind == "tar":
        return ["open", "-W", "-n", str(program)], env
    if kind == "appimage" and not ctypes.util.find_library("fuse"):
        # The AppImage runtime needs FUSE 2; without it, it unpacks itself to a temp dir.
        env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    return [str(program)], env


def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="impulcifer_gui",
        description="Start the Impulcifer app (recording and processing screens). "
        "The first run downloads the app of this version from GitHub Releases.",
    )
    parser.add_argument(
        "--download-only",
        action="store_true",
        help="Download and check the app, print where it is, and do not start it.",
    )
    args = parser.parse_args(argv)
    chosen = target()
    if chosen is None:
        print(
            f"No Impulcifer app is released for {platform.system()} {platform.machine()}. "
            f"Use the impulcifer command, or build the app from source: {RELEASES.rsplit('/', 1)[0]}#readme",
            file=sys.stderr,
        )
        return 2
    try:
        program = install(version(), chosen, log=lambda line: print(line, file=sys.stderr))
    except LauncherError as error:
        print(error, file=sys.stderr)
        return 1
    if args.download_only:
        print(program)
        return 0
    argv_, env = command(program, chosen.kind)
    try:
        return subprocess.call(argv_, env=env)
    except OSError as error:
        print(f"Could not start {program} ({error}). Delete {program.parent} and run impulcifer_gui again.", file=sys.stderr)
        return 1


def cli():
    sys.exit(main())
