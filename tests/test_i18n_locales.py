"""Tests for localization file integrity."""

from __future__ import annotations

import json
import re
from pathlib import Path


_PLACEHOLDER_RE = re.compile(r"\{[^{}]+\}")


def _load_locales() -> tuple[Path, dict[str, str], list[tuple[Path, dict[str, str]]]]:
    locale_dir = Path(__file__).parent.parent / "i18n" / "locales"
    english = json.loads((locale_dir / "en.json").read_text(encoding="utf-8"))
    locales = [
        (locale_file, json.loads(locale_file.read_text(encoding="utf-8")))
        for locale_file in sorted(locale_dir.glob("*.json"))
    ]
    return locale_dir, english, locales


def test_all_locale_keys_match_english() -> None:
    """All locale JSON files should have exactly the same keys as en.json."""
    _locale_dir, english, locales = _load_locales()
    reference_keys = set(english)

    for locale_file, locale in locales:
        keys = set(locale)
        assert keys == reference_keys, (
            f"{locale_file.name} key mismatch: "
            f"missing={sorted(reference_keys - keys)[:5]}, "
            f"extra={sorted(keys - reference_keys)[:5]}"
        )


def test_locale_placeholders_match_english() -> None:
    """Translated strings should keep the same interpolation placeholders."""
    _locale_dir, english, locales = _load_locales()

    for locale_file, locale in locales:
        for key, english_value in english.items():
            if not isinstance(english_value, str):
                continue
            assert set(_PLACEHOLDER_RE.findall(locale[key])) == set(
                _PLACEHOLDER_RE.findall(english_value)
            ), f"{locale_file.name}:{key} placeholder mismatch"


def test_update_completion_copy_describes_completion() -> None:
    """Completed updates should not reuse old 'update started' copy."""
    _locale_dir, english, locales = _load_locales()
    old_completion_values = {
        "Update started! Please restart the application.",
        "Update Started",
        "The update has been started in the background.\n"
        "Please restart the application to use the new version.",
        "업데이트가 시작되었습니다! 애플리케이션을 재시작해주세요.",
        "업데이트 시작됨",
        "업데이트가 백그라운드에서 시작되었습니다.\n"
        "새 버전을 사용하려면 애플리케이션을 재시작해주세요.",
    }

    assert english["update_complete_title"] == "Update Complete"
    for locale_file, locale in locales:
        for key in ("update_success", "update_complete_title", "update_complete_message"):
            assert locale[key] not in old_completion_values, f"{locale_file.name}:{key}"


def test_mic_deviation_copy_tracks_current_v3() -> None:
    """Microphone deviation labels should match the v3 implementation."""
    _locale_dir, _english, locales = _load_locales()

    for locale_file, locale in locales:
        assert "v2.0" not in locale["label_v2_options"], locale_file.name
        assert "v2.0" not in locale["cli_correcting_deviation"], locale_file.name
        assert "v3.0" in locale["cli_correcting_deviation"], locale_file.name


def test_visible_locale_strings_are_not_english_fallbacks() -> None:
    """Recently added visible UI strings should be localized outside English."""
    _locale_dir, english, locales = _load_locales()
    visible_keys = (
        "message_using_default_recording",
        "checkbox_append_to_file",
        "checkbox_debug_plots",
        "section_processing_options",
        "checkbox_plot_results",
        "message_channel_mismatch_warning_title",
        "message_start_recording_title",
        "message_recording_complete_title",
        "section_recording_status",
        "recording_status_ready",
        "recording_status_recording",
        "recording_status_complete",
        "dialog_recording_title",
        "message_done",
        "message_skin_changed",
        "studio_card_channel_status",
        "studio_record_start",
        "update_restart_done",
    )

    for locale_file, locale in locales:
        if locale_file.name == "en.json":
            continue
        for key in visible_keys:
            assert locale[key] != english[key], f"{locale_file.name}:{key} is English fallback"
