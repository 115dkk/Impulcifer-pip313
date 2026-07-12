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


def test_included_modules_keeps_only_non_static_imports():
    """After the post-Phase 5 cleanup, the explicit list should only contain
    modules Nuitka cannot find via static-import tracing.

    Project modules (``core.*`` except ``parallel_workers``, ``gui.*``,
    ``i18n.*``, ``updater.*``, ``impulcifer``) and statically-imported
    third-party packages (``nnresample``, ``tabulate``, ``autoeq``,
    ``soundfile``, ``sounddevice``, top-level ``scipy``, ``seaborn``) are
    intentionally absent — the tracer follows them automatically and listing
    them only inflates compile time.
    """
    mod = _flags_module()
    listed = set(mod.INCLUDED_MODULES)

    # Things that MUST stay: subprocess-loaded worker + build-time marker
    assert "core.parallel_workers" in listed, (
        "ProcessPoolExecutor child processes need this explicit include."
    )
    assert "infra._build_info" in listed, (
        "build_nuitka.py generates this file just-in-time; defensive include."
    )

    # Things that MUST be absent: project tree statically followed from
    # gui_main → gui.modern_gui → gui.tabs → impulcifer → core.* → ...
    forbidden = {
        "core",
        "core.constants",
        "core.utils",
        "core.impulse_response",
        "core.hrir",
        "core.pipeline",
        "core.cli_builder",
        "core.plotting",
        "core.plotting.hrir_plotter",
        "core.plotting.impulse_response_plotter",
        "gui",
        "gui.modern_gui",
        "gui.legacy_gui",
        "gui.tabs.recorder_tab",
        "gui.tabs.impulcifer_tab",
        "i18n.localization",
        "infra.logger",
        "updater.update_checker",
        "updater.updater_core",
        "impulcifer",
    }
    leaked = forbidden & listed
    assert not leaked, (
        f"INCLUDED_MODULES contains entries Nuitka follows automatically: {leaked}"
    )


def test_common_flags_enable_required_plugins():
    mod = _flags_module()
    flags = mod.COMMON_FLAGS
    assert "--enable-plugin=tk-inter" in flags
    assert "--enable-plugin=matplotlib" in flags
    # multiprocessing / pkg-resources / anti-bloat are auto-enabled by
    # Nuitka itself; explicitly listing them would be a noise.
    for auto in ("multiprocessing", "pkg-resources", "anti-bloat"):
        assert f"--enable-plugin={auto}" not in flags, (
            f"--enable-plugin={auto} is auto-enabled and shouldn't be listed."
        )


def test_data_dirs_route_locales_into_i18n_subfolder():
    """``i18n.localization._find_locales_dir`` checks ``i18n/locales`` first."""
    mod = _flags_module()
    pairs = dict(mod.INCLUDED_DATA_DIRS)
    assert pairs.get("i18n/locales") == "i18n/locales", (
        "Locale data dir destination must be 'i18n/locales' to match the runtime loader."
    )


def test_data_dirs_bundle_pulse_logo_and_theme():
    """Pulse redesign assets must reach the standalone bundle."""
    mod = _flags_module()
    pairs = dict(mod.INCLUDED_DATA_DIRS)
    assert pairs.get("logo") == "logo", (
        "logo/ must be bundled at the same path so iconbitmap()/iconphoto() resolve at runtime."
    )
    assert pairs.get("gui/theme") == "gui/theme", (
        "gui/theme must be bundled so set_default_color_theme(pulse.json) works in the .exe."
    )


def test_platform_specific_flags_windows_includes_pulse_icon(tmp_path):
    """On Windows the .exe must adopt the bundled pulse.ico via Nuitka flag."""
    mod = _flags_module()
    # Stage a fake project root with the icon present.
    (tmp_path / "logo").mkdir()
    icon = tmp_path / "logo" / "pulse.ico"
    icon.write_bytes(b"")  # presence is all the helper checks
    flags = mod.platform_specific_flags("windows", project_root=str(tmp_path))
    assert any(f.startswith("--windows-icon-from-ico=") and "pulse.ico" in f for f in flags), (
        f"--windows-icon-from-ico=logo/pulse.ico missing from windows flags: {flags}"
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


def test_webview_frontend_is_bundled():
    """The WebView frontend (launcher default since 2.10) must ship in the
    standalone bundle — but WITHOUT explicit module includes.

    Any ``--include-module=webview`` / ``--include-package=webview`` entry
    feeds Nuitka's follow patterns for the whole subtree and conflicts with
    the auto-enabled 'pywebview' plugin, which excludes the other platforms'
    backend modules (observed FATAL: "Conflict between user and plugin
    decision for module 'webview.platforms.android'"). The static
    ``import webview`` chain in gui_main → impulcifer_webview is traced
    automatically instead.
    """
    mod = _flags_module()
    assert "webview" not in mod.INCLUDED_PACKAGES
    assert "webview" not in mod.INCLUDED_MODULES
    # Data-file flags do not feed follow patterns — safe belt-and-suspenders.
    assert "webview" in mod.INCLUDED_PACKAGE_DATA
    pairs = dict(mod.INCLUDED_DATA_DIRS)
    assert pairs.get("webview_ui") == "webview_ui", (
        "webview_ui must be bundled at the same relative path so "
        "get_resource_path('webview_ui/index.html') resolves at runtime."
    )


def test_no_explicit_pythonnet_flags():
    """pythonnet/clr_loader are followed from webview.platforms.winforms and
    their DLLs come from Nuitka's package config ('clr', 'clr_loader.ffi',
    'pythonnet' entries); explicit includes are unnecessary on Windows and
    would fail macOS/Linux builds where those wheels don't exist."""
    mod = _flags_module()
    for plat in ("windows", "macos", "linux"):
        flags = mod.platform_specific_flags(plat)
        assert not any(
            "pythonnet" in flag or "clr" in flag for flag in flags
        ), f"{plat} flags must not reference the pythonnet stack: {flags}"


def test_pywebview_plugin_patch_source():
    """The build-time hotfix inserts webview.platforms.win32 into Nuitka's
    pywebview plugin whitelist (idempotent, anchor-guarded)."""
    from build_scripts.patch_nuitka_pywebview import patch_source

    stale = (
        "                result = module_name in (\n"
        '                    "webview.platforms.winforms",\n'
        '                    "webview.platforms.edgechromium",\n'
        "                )\n"
    )
    patched, status = patch_source(stale)
    assert status == "patched"
    lines = patched.splitlines()
    idx = next(i for i, line in enumerate(lines) if "winforms" in line)
    assert lines[idx + 1].strip() == '"webview.platforms.win32",'
    assert lines[idx + 1].startswith("                    ")

    again, status2 = patch_source(patched)
    assert status2 == "already-ok"
    assert again == patched

    assert patch_source("unrelated file contents")[1] == "anchor-missing"


def test_third_party_license_notices_are_bundled():
    """OFL/MIT/BSD-3 재배포 조건: 고지 파일이 standalone 번들에 실려야 한다."""
    mod = _flags_module()
    files = dict(mod.INCLUDED_DATA_FILES)
    assert files.get("THIRD_PARTY_LICENSES.md") == "THIRD_PARTY_LICENSES.md"
    assert files.get("autoeq/LICENSE") == "autoeq/LICENSE"
    # font/OFL-*.txt는 font 데이터 디렉토리 전체 포함으로 함께 실린다.
    assert dict(mod.INCLUDED_DATA_DIRS).get("font") == "font"


def test_scipy_vendored_compat_is_included_conditionally():
    """scipy의 벤더링 array-api-compat(lazy import 서브모듈)은 존재하는
    경로만 --include-package 되어야 한다 — 2.10.0 standalone에서 모든
    scipy import를 죽인 회귀(scipy 1.18의 scipy._external 누락)의 방지책."""
    from build_scripts.nuitka_flags import scipy_vendored_compat_flags

    # scipy 1.18+ 환경
    flags = scipy_vendored_compat_flags(package_exists=lambda n: n == "scipy._external")
    assert flags == ["--include-package=scipy._external"]

    # scipy 1.12~1.17 환경
    flags = scipy_vendored_compat_flags(
        package_exists=lambda n: n == "scipy._lib.array_api_compat"
    )
    assert flags == ["--include-package=scipy._lib.array_api_compat"]

    # 어느 쪽도 없으면(미래 재배치) 존재하지 않는 include를 만들지 않는다.
    assert scipy_vendored_compat_flags(package_exists=lambda n: False) == []


def test_build_args_include_scipy_compat_in_this_environment():
    """실제 빌드 환경 기준으로 최소 한 경로는 포함되어야 한다(requirements의
    scipy>=1.12는 모두 array-api-compat을 벤더링한다)."""
    mod = _flags_module()
    args = mod.build_nuitka_args(target_platform="windows", version="9.9.9")
    assert any(
        a in ("--include-package=scipy._external", "--include-package=scipy._lib.array_api_compat")
        for a in args
    ), "scipy vendored array-api-compat include missing from build args"
