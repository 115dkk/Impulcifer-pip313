# -*- coding: utf-8 -*-
"""Unit tests for the canonical Nuitka flag definitions (issue #87 Phase 4).

The build flags are spread across:

* ``build_scripts/build_nuitka.py``        (Windows release, called by CI)
* ``.github/workflows/build-linux.yml``    (Linux AppImage)
* ``.github/workflows/build-macos.yml``    (macOS app bundle)
* ``.github/workflows/release-cross-platform.yml`` (Windows + macOS)

``build_scripts/nuitka_flags.py`` is the single source of truth. These tests
guard the public API of that module so its contract is testable on every
commit, even though we don't actually invoke Nuitka here.
"""

import importlib

import pytest


def _flags_module():
    return importlib.import_module("build_scripts.nuitka_flags")


def test_module_imports_cleanly():
    mod = _flags_module()
    for name in (
        "COMMON_FLAGS",
        "PLATFORM_OUTPUT_DIRS",
        "PLATFORM_OUTPUT_FILENAMES",
        "INCLUDED_PACKAGES",
        "INCLUDED_MODULES",
        "INCLUDED_DATA_DIRS",
        "INCLUDED_DATA_FILES",
        "METADATA_TEMPLATE",
        "platform_specific_flags",
        "build_nuitka_args",
    ):
        assert hasattr(mod, name), f"nuitka_flags missing public symbol: {name}"


@pytest.mark.parametrize("plat", ["windows", "macos", "linux"])
def test_build_nuitka_args_contains_required_switches(plat):
    mod = _flags_module()
    args = mod.build_nuitka_args(target_platform=plat, version="9.9.9")
    # Must declare a standalone build, an entry point, and a versioned filename
    assert "--standalone" in args
    assert "gui_main.py" in args
    assert "--file-version=9.9.9" in args
    assert "--product-version=9.9.9" in args
    # Output dir is platform-specific
    assert any(a.startswith("--output-dir=") for a in args)


def test_platform_specific_flags_windows():
    mod = _flags_module()
    flags = mod.platform_specific_flags("windows")
    assert "--windows-console-mode=disable" in flags


def test_platform_specific_flags_macos_includes_app_bundle():
    mod = _flags_module()
    flags = mod.platform_specific_flags("macos")
    assert "--macos-create-app-bundle" in flags
    assert "--macos-app-name=Impulcifer" in flags


def test_included_modules_cover_phase1_and_phase2_packages():
    """The Phase 1 plotting subpackage and Phase 2 pipeline module must be
    explicitly included so Nuitka picks them up in the standalone bundle."""
    mod = _flags_module()
    required = {
        "core.plotting",
        "core.plotting.hrir_plotter",
        "core.plotting.impulse_response_plotter",
        "core.pipeline",
        "core.cli_builder",
    }
    missing = required - set(mod.INCLUDED_MODULES)
    assert not missing, f"INCLUDED_MODULES missing: {missing}"


def test_data_dirs_route_locales_into_i18n_subfolder():
    """``i18n.localization._find_locales_dir`` checks ``i18n/locales`` first."""
    mod = _flags_module()
    pairs = dict(mod.INCLUDED_DATA_DIRS)
    assert pairs.get("i18n/locales") == "i18n/locales", (
        "Locale data dir destination must be 'i18n/locales' to match the runtime loader."
    )


def test_cli_emits_one_flag_per_line(capsys):
    mod = _flags_module()
    rc = mod.main(["--platform", "linux", "--version", "1.2.3"])
    assert rc == 0
    out = capsys.readouterr().out
    lines = [ln for ln in out.splitlines() if ln.strip()]
    assert any(ln.startswith("--output-dir=") for ln in lines)
    assert any(ln == "--standalone" for ln in lines)
    assert any(ln.endswith(".py") for ln in lines), "Entry point line missing"
