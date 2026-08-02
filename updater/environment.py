# -*- coding: utf-8 -*-
"""Backward-compat shim — runtime environment detection moved to
:mod:`infra.environment` (audit #138 C3/F012), so ``gui``/``application``
code no longer has to reach into the ``updater`` package for it.

Import from :mod:`infra.environment` in new code.
"""

from infra.environment import (  # noqa: F401  (re-export)
    get_velopack_update_exe,
    is_pip_environment,
    is_standalone_build,
    is_standalone_build as _is_standalone_build,
    is_velopack_environment,
)
