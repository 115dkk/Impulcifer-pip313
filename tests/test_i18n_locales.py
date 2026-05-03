"""Tests for localization file integrity."""

from __future__ import annotations

import json
from pathlib import Path


def test_all_locale_keys_match_english() -> None:
    """All locale JSON files should have exactly the same keys as en.json."""
    locale_dir = Path(__file__).parent.parent / "i18n" / "locales"
    reference_keys = set(json.loads((locale_dir / "en.json").read_text(encoding="utf-8")))

    for locale_file in sorted(locale_dir.glob("*.json")):
        keys = set(json.loads(locale_file.read_text(encoding="utf-8")))
        assert keys == reference_keys, (
            f"{locale_file.name} key mismatch: "
            f"missing={sorted(reference_keys - keys)[:5]}, "
            f"extra={sorted(keys - reference_keys)[:5]}"
        )
