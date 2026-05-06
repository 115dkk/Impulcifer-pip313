"""Local fast-iteration Nuitka build.

Same flag set as :mod:`build_scripts.build_nuitka` but cranks ``--jobs`` up
to (CPU count - 2) so multi-minute C compilation collapses to roughly a
quarter of the CI build time on developer machines. CI keeps the canonical
``--jobs=4`` because GitHub runners are not guaranteed to be wide.

Output is written to ``dist/local/`` so it never collides with CI's release
output directory and can be wiped without affecting other builds.

Usage:
    python build_scripts/build_local.py
    PYTHONIOENCODING=utf-8 python build_scripts/build_local.py
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

# When invoked as a script, add project root to sys.path so the package import
# below resolves regardless of the current working directory.
_PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(_PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(_PROJECT_ROOT))

from build_scripts.nuitka_flags import (  # noqa: E402
    PLATFORM_OUTPUT_FILENAMES,
    build_nuitka_args,
)


def _detect_platform() -> str:
    """Return canonical platform string used in nuitka_flags."""
    import platform

    system = platform.system().lower()
    if system == "windows":
        return "windows"
    if system == "darwin":
        return "macos"
    if system == "linux":
        return "linux"
    return "unknown"


def _read_project_version() -> str:
    """Run ``infra/get_version.py`` to fetch the current project version."""
    try:
        result = subprocess.run(
            [sys.executable, "infra/get_version.py"],
            capture_output=True,
            text=True,
            check=True,
            encoding="utf-8",
        )
        version = (result.stdout or "").strip()
        return version or "0.0.0-local"
    except Exception as exc:  # noqa: BLE001 - best-effort fallback
        print(f"  warning: could not read version ({exc}); using 0.0.0-local")
        return "0.0.0-local"


def _generate_build_marker(version: str) -> None:
    """Write infra/_build_info.py exactly like the canonical build script."""
    target = _PROJECT_ROOT / "infra" / "_build_info.py"
    target.write_text(
        '# -*- coding: utf-8 -*-\n'
        '"""빌드 정보 마커 — build_local.py에 의해 자동 생성됨."""\n'
        'BUILD_TYPE = "standalone"\n'
        f'VERSION = "{version}"\n',
        encoding="utf-8",
    )


def main() -> int:
    target_platform = _detect_platform()
    if target_platform == "unknown":
        print(f"  ERROR: unsupported platform {sys.platform!r}", flush=True)
        return 2

    version = _read_project_version()
    print(f"=== build_local.py ===", flush=True)
    print(f"  platform: {target_platform}", flush=True)
    print(f"  version:  {version}", flush=True)

    cpu_count = os.cpu_count() or 4
    # Leave 2 cores for the OS / IDE so the machine remains usable. Cap at 14
    # which empirically gives diminishing returns past that on Windows MSVC.
    jobs = max(2, min(cpu_count - 2, 14))
    print(f"  jobs:     {jobs} (cpu_count={cpu_count})", flush=True)

    output_dir = "dist/local"
    output_filename = PLATFORM_OUTPUT_FILENAMES.get(target_platform, "Impulcifer")
    print(f"  output:   {output_dir}/gui_main.dist/{output_filename}.exe", flush=True)

    _generate_build_marker(version)

    args = build_nuitka_args(
        target_platform=target_platform,
        version=version,
        project_root=str(_PROJECT_ROOT),
        output_dir=output_dir,
        output_filename=output_filename,
        jobs=jobs,
    )
    cmd = [sys.executable, "-m", "nuitka", *args]
    print(f"\nlaunching: {' '.join(cmd[:5])} ... ({len(cmd)} args total)", flush=True)

    try:
        # Stream Nuitka's own progress (capture_output=True hides --show-progress).
        completed = subprocess.run(cmd, check=False)
    except KeyboardInterrupt:
        print("\n  build_local.py: interrupted by user.", flush=True)
        return 130

    if completed.returncode != 0:
        print(
            f"\n  ERROR: nuitka exited with code {completed.returncode}",
            flush=True,
        )
        return completed.returncode

    binary = Path(output_dir) / "gui_main.dist" / f"{output_filename}.exe"
    if target_platform != "windows":
        binary = Path(output_dir) / "gui_main.dist" / output_filename
    if binary.exists():
        size_mb = binary.stat().st_size // (1024 * 1024)
        print(f"\n  OK: built {binary} ({size_mb} MB)", flush=True)
    else:
        print(f"\n  WARNING: expected binary not found at {binary}", flush=True)

    print(
        "\n  Hint: run\n"
        f"    {binary} --smoke-test\n"
        "  to verify the bundled binary's import chain + Pretendard guarantee.",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
