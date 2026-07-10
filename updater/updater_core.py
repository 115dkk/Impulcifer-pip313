#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Automatic updater for Impulcifer
Supports both Velopack (standalone) and pip (development/pip install)
environments.

The implementation is split into focused modules; this file is a thin
re-export shim so existing callers keep working unchanged.

New module layout:

    updater/environment.py       — runtime probes (Velopack/Pip/standalone)
    updater/velopack.py          — VelopackUpdater + VelopackDownloadError
    updater/pip_updater.py       — PipUpdater
    updater/legacy.py            — LegacyInstallerUpdater + Updater + URL
    updater/executors.py         — UpdateExecutor + 3 subclasses + factory

Prefer importing from the focused modules in new code; this shim is kept for
backward compatibility only.
"""

from updater.environment import (  # noqa: F401  (re-export)
    _is_standalone_build,
    get_velopack_update_exe,
    is_pip_environment,
    is_velopack_environment,
)
from updater.executors import (  # noqa: F401  (re-export)
    LegacyExecutor,
    PipExecutor,
    UpdateExecutionError,
    UpdateExecutionResult,
    UpdateExecutor,
    VelopackExecutor,
    create_update_executor,
    get_updater,
)
from updater.legacy import (  # noqa: F401  (re-export)
    GITHUB_RELEASES_URL,
    LegacyInstallerUpdater,
    Updater,
)
from updater.pip_updater import PipUpdater  # noqa: F401  (re-export)
from updater.velopack import (  # noqa: F401  (re-export)
    VelopackDownloadError,
    VelopackUpdater,
)


if __name__ == '__main__':
    print("Updater environment detection:")
    print(f"  Velopack environment: {is_velopack_environment()}")
    print(f"  Pip environment: {is_pip_environment()}")
    print(f"  Update.exe path: {get_velopack_update_exe()}")
