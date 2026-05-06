# -*- coding: utf-8 -*-
"""Pip-based updater for development / pip-install environments.

Split out of ``updater/updater_core.py`` (issue #87 follow-up).
``updater_core`` re-exports :class:`PipUpdater` for backward compatibility.
"""

import platform
import subprocess
import sys


class PipUpdater:
    """Updater for pip-installed packages."""

    def __init__(self, package_name: str = "impulcifer-py313"):
        """
        Initialize pip updater.

        Args:
            package_name: Name of the package on PyPI
        """
        self.package_name = package_name

    def upgrade(self) -> bool:
        """
        Upgrade the package using pip.

        Returns:
            True if upgrade process started successfully
        """
        try:
            upgrade_cmd = [
                sys.executable,
                '-m', 'pip', 'install', '--upgrade',
                self.package_name
            ]

            print(f"Upgrading with command: {' '.join(upgrade_cmd)}")

            if platform.system() == 'Windows':
                subprocess.Popen(
                    upgrade_cmd,
                    creationflags=subprocess.CREATE_NEW_CONSOLE
                )
            else:
                subprocess.Popen(
                    upgrade_cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE
                )

            return True

        except Exception as e:
            print(f"Pip upgrade error: {e}")
            return False
