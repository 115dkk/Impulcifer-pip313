"""Canonical Nuitka build flag definitions (issue #87 Phase 4).

This module is the single source of truth for the arguments passed to
``python -m nuitka`` when packaging Impulcifer. Both
:mod:`build_scripts.build_nuitka` (used by the Windows release workflow) and
the GitHub Actions ``build-linux.yml`` / ``build-macos.yml`` workflows should
ultimately consume the values defined here.

The flag list is split into named groups so callers can compose them:

* :data:`COMMON_FLAGS` — platform-independent Nuitka switches
* :data:`INCLUDED_PACKAGES` / :data:`INCLUDED_MODULES` — `--include-package` /
  `--include-module` lists
* :data:`INCLUDED_DATA_DIRS` / :data:`INCLUDED_DATA_FILES` — data assets
* :data:`METADATA_FLAGS` — name/version metadata template (callers must
  substitute version)

Use :func:`build_nuitka_args` to assemble the full argument list for a given
platform and version.
"""

from __future__ import annotations

import os
from typing import Iterable, List, Optional, Sequence


# ---------------------------------------------------------------------------
# Common (platform-independent) flags
# ---------------------------------------------------------------------------
COMMON_FLAGS: tuple[str, ...] = (
    "--standalone",
    "--remove-output",
    "--jobs=4",
    "--lto=no",
    "--enable-plugin=tk-inter",
    "--prefer-source-code",
    "--assume-yes-for-downloads",
    "--show-progress",
    "--show-memory",
)


# Output directory mapping per target platform
PLATFORM_OUTPUT_DIRS: dict[str, str] = {
    "windows": "dist/Impulcifer_Distribution/ImpulciferGUI",
    "macos": "dist/macos",
    "linux": "dist/linux",
}

PLATFORM_OUTPUT_FILENAMES: dict[str, str] = {
    "windows": "ImpulciferGUI",
    "macos": "Impulcifer",
    "linux": "Impulcifer",
}


# ---------------------------------------------------------------------------
# Module / package inclusion lists
# ---------------------------------------------------------------------------
INCLUDED_PACKAGES: tuple[str, ...] = (
    "customtkinter",
)

INCLUDED_MODULES: tuple[str, ...] = (
    # Third-party deps that Nuitka can't always discover dynamically
    "sounddevice",
    "soundfile",
    "scipy",
    "scipy.signal",
    "scipy.optimize",
    "scipy.interpolate",
    "scipy.io",
    "scipy.fft",
    "nnresample",
    "tabulate",
    "seaborn",
    "bokeh",
    "autoeq",
    # Project packages and modules
    "core",
    "core.constants",
    "core.utils",
    "core.impulse_response",
    "core.impulse_response_estimator",
    "core.hrir",
    "core.room_correction",
    "core.microphone_deviation_correction",
    "core.virtual_bass",
    "core.channel_generation",
    "core.recorder",
    "core.recording_validation",
    "core.parallel_processing",
    "core.parallel_utils",
    "core.parallel_workers",
    "core.pipeline",
    "core.cli_builder",
    "core.plotting",
    "core.plotting.hrir_plotter",
    "core.plotting.impulse_response_plotter",
    "gui",
    "gui.modern_gui",
    "gui.legacy_gui",
    "gui.constants",
    "gui.utils",
    "gui.dialogs",
    "gui.event_bus",
    "gui.tabs",
    "gui.tabs.recorder_tab",
    "gui.tabs.impulcifer_tab",
    "gui.tabs.settings_tab",
    "gui.tabs.info_tab",
    "i18n",
    "i18n.localization",
    "infra",
    "infra.logger",
    "infra.resource_helper",
    "infra.get_version",
    "infra._build_info",
    "updater",
    "updater.update_checker",
    "updater.updater_core",
    "impulcifer",
)


# ---------------------------------------------------------------------------
# Data assets — paths relative to project root
# ---------------------------------------------------------------------------
# Each entry is (source, destination_inside_bundle)
INCLUDED_DATA_DIRS: tuple[tuple[str, str], ...] = (
    ("data", "data"),
    ("font", "font"),
    ("img", "img"),
    ("i18n/locales", "i18n/locales"),
)

INCLUDED_DATA_FILES: tuple[tuple[str, str], ...] = (
    ("LICENSE", "License.txt"),
    ("README.txt", "README.txt"),
)


# ---------------------------------------------------------------------------
# Metadata flags (callers substitute version & description)
# ---------------------------------------------------------------------------
METADATA_TEMPLATE: tuple[str, ...] = (
    "--company-name=115dkk",
    "--product-name=Impulcifer",
    "--file-description=HRIR 측정 및 헤드폰 바이노럴 헤드트래킹 HRTF 시스템",
)


def platform_specific_flags(target_platform: str, project_root: str = ".") -> List[str]:
    """Return platform-conditional Nuitka flags.

    Includes window subsystem (Windows), app bundle (macOS), and icon
    inclusion when an icon file exists under ``project_root/img/``.
    """
    flags: List[str] = []
    if target_platform == "windows":
        flags.append("--windows-console-mode=disable")
    elif target_platform == "macos":
        flags.extend(("--macos-create-app-bundle", "--macos-app-name=Impulcifer"))
        icns = os.path.join(project_root, "img", "icon.icns")
        if os.path.exists(icns):
            flags.append(f"--macos-app-icon={icns}")
    elif target_platform == "linux":
        png = os.path.join(project_root, "img", "icon.png")
        if os.path.exists(png):
            flags.append(f"--linux-icon={png}")
    return flags


def build_nuitka_args(
    target_platform: str,
    version: str,
    project_root: str = ".",
    output_dir: Optional[str] = None,
    output_filename: Optional[str] = None,
    entry_point: str = "gui_main.py",
) -> List[str]:
    """Assemble the complete argument list for ``python -m nuitka``.

    Returns the args without the leading ``["python", "-m", "nuitka"]``.
    The caller can prepend that or pass them to :func:`subprocess.run`.

    ``project_root`` is used to test for optional assets (icons, README.txt,
    LICENSE) so they are only included when they exist.
    """
    if output_dir is None:
        output_dir = PLATFORM_OUTPUT_DIRS.get(target_platform, "dist")
    if output_filename is None:
        output_filename = PLATFORM_OUTPUT_FILENAMES.get(target_platform, "Impulcifer")

    args: List[str] = list(COMMON_FLAGS)
    args.append(f"--output-dir={output_dir}")
    args.extend(platform_specific_flags(target_platform, project_root))

    for pkg in INCLUDED_PACKAGES:
        args.append(f"--include-package={pkg}")
    for mod in INCLUDED_MODULES:
        args.append(f"--include-module={mod}")

    for src, dst in INCLUDED_DATA_DIRS:
        full_src = os.path.join(project_root, src)
        if os.path.exists(full_src):
            args.append(f"--include-data-dir={src}={dst}")

    for src, dst in INCLUDED_DATA_FILES:
        full_src = os.path.join(project_root, src)
        if os.path.exists(full_src):
            args.append(f"--include-data-file={src}={dst}")

    args.extend(METADATA_TEMPLATE)
    args.append(f"--output-filename={output_filename}")
    args.append(f"--file-version={version}")
    args.append(f"--product-version={version}")
    args.append(entry_point)
    return args


def main(argv: Optional[Sequence[str]] = None) -> int:
    """Tiny CLI: ``python -m build_scripts.nuitka_flags --platform linux --version X``.

    Prints one Nuitka flag per line so a workflow can ``mapfile`` the output
    or feed it into a Nuitka invocation. Exists primarily to keep workflows
    in sync with this Python module without parsing it.
    """
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--platform",
        choices=("windows", "macos", "linux"),
        required=True,
        help="Target build platform.",
    )
    parser.add_argument(
        "--version",
        required=True,
        help="Project version string substituted into --file-version / --product-version.",
    )
    parser.add_argument(
        "--project-root",
        default=".",
        help="Path to the project root (defaults to CWD).",
    )
    args = parser.parse_args(argv)

    flag_list = build_nuitka_args(
        target_platform=args.platform,
        version=args.version,
        project_root=args.project_root,
    )
    for flag in flag_list:
        print(flag)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
