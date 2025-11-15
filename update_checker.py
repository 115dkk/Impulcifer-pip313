#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Automatic update checker for Impulcifer
Checks GitHub releases for new versions
"""

import json
import urllib.request
import urllib.error
from typing import Optional, Dict, Tuple
from packaging import version
import platform

# GitHub repository information
GITHUB_REPO_OWNER = "115dkk"
GITHUB_REPO_NAME = "Impulcifer-pip313"
GITHUB_API_URL = f"https://api.github.com/repos/{GITHUB_REPO_OWNER}/{GITHUB_REPO_NAME}/releases/latest"


class UpdateChecker:
    """Check for updates from GitHub releases"""

    def __init__(self, current_version: str):
        """
        Initialize update checker

        Args:
            current_version: Current application version (e.g., "1.8.5")
        """
        self.current_version = current_version
        self.latest_release_info: Optional[Dict] = None

    def check_for_updates(self, timeout: int = 10) -> Tuple[bool, Optional[str], Optional[str]]:
        """
        Check if a new version is available

        Args:
            timeout: Request timeout in seconds

        Returns:
            Tuple of (update_available, latest_version, download_url)
        """
        try:
            # Fetch latest release info from GitHub
            req = urllib.request.Request(
                GITHUB_API_URL,
                headers={'User-Agent': 'Impulcifer-Update-Checker'}
            )

            with urllib.request.urlopen(req, timeout=timeout) as response:
                data = json.loads(response.read().decode('utf-8'))
                self.latest_release_info = data

            # Extract version from tag name (e.g., "v1.9.0" -> "1.9.0")
            tag_name = data.get('tag_name', '')
            latest_version = tag_name.lstrip('v')

            if not latest_version:
                return False, None, None

            # Compare versions
            if self._is_newer_version(latest_version):
                download_url = self._get_download_url(data)
                return True, latest_version, download_url

            return False, latest_version, None

        except urllib.error.HTTPError as e:
            # Rate limiting or other HTTP errors
            if e.code == 403:
                print(f"GitHub API rate limit exceeded: {e}")
            else:
                print(f"HTTP error checking for updates: {e}")
            return False, None, None

        except urllib.error.URLError as e:
            # Network error
            print(f"Network error checking for updates: {e}")
            return False, None, None

        except Exception as e:
            # Other errors
            print(f"Error checking for updates: {e}")
            return False, None, None

    def _is_newer_version(self, latest_version: str) -> bool:
        """
        Compare versions using semantic versioning

        Args:
            latest_version: Latest version string

        Returns:
            True if latest_version is newer than current_version
        """
        try:
            current = version.parse(self.current_version)
            latest = version.parse(latest_version)
            return latest > current
        except Exception as e:
            print(f"Error comparing versions: {e}")
            return False

    def _get_download_url(self, release_data: Dict) -> Optional[str]:
        """
        Get the appropriate download URL for the current platform

        Args:
            release_data: GitHub release data

        Returns:
            Download URL or None if not found
        """
        assets = release_data.get('assets', [])

        # Detect platform
        system = platform.system()

        # Look for installer file
        for asset in assets:
            name = asset.get('name', '').lower()
            download_url = asset.get('browser_download_url')

            if system == 'Windows':
                # Look for .exe installer
                if name.endswith('.exe') and 'setup' in name:
                    return download_url
            elif system == 'Darwin':  # macOS
                # Look for .dmg or .pkg
                if name.endswith(('.dmg', '.pkg')):
                    return download_url
            elif system == 'Linux':
                # Look for .deb, .rpm, or .AppImage
                if name.endswith(('.deb', '.rpm', '.appimage')):
                    return download_url

        # Fallback: return first asset
        if assets:
            return assets[0].get('browser_download_url')

        return None

    def get_release_notes(self) -> Optional[str]:
        """
        Get release notes for the latest version

        Returns:
            Release notes text or None
        """
        if self.latest_release_info:
            return self.latest_release_info.get('body')
        return None

    def get_release_url(self) -> Optional[str]:
        """
        Get the GitHub release page URL

        Returns:
            Release page URL or None
        """
        if self.latest_release_info:
            return self.latest_release_info.get('html_url')
        return None


def check_for_updates_simple(current_version: str) -> Tuple[bool, Optional[str], Optional[str]]:
    """
    Simple function to check for updates

    Args:
        current_version: Current application version

    Returns:
        Tuple of (update_available, latest_version, download_url)
    """
    checker = UpdateChecker(current_version)
    return checker.check_for_updates()


if __name__ == '__main__':
    # Test the update checker
    import sys

    # Get version from pyproject.toml or command line
    test_version = sys.argv[1] if len(sys.argv) > 1 else "1.8.5"

    print(f"Current version: {test_version}")
    print("Checking for updates...")

    checker = UpdateChecker(test_version)
    has_update, latest_ver, download_url = checker.check_for_updates()

    if has_update:
        print(f"✅ Update available: {latest_ver}")
        print(f"Download URL: {download_url}")

        release_notes = checker.get_release_notes()
        if release_notes:
            print("\nRelease notes:")
            print(release_notes[:500])  # First 500 chars
    else:
        print(f"✅ You are up to date (latest: {latest_ver})")
