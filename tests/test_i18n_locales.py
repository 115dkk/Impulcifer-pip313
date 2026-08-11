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


def test_mic_deviation_copy_tracks_current_v4() -> None:
    """Microphone deviation labels should match the v4 implementation."""
    _locale_dir, _english, locales = _load_locales()

    for locale_file, locale in locales:
        assert "v2.0" not in locale["label_v2_options"], locale_file.name
        assert "v3.0" not in locale["cli_correcting_deviation"], locale_file.name
        assert "v4.0" in locale["cli_correcting_deviation"], locale_file.name
        # 헤드폰 보상 시 건너뛴다는 안내 문구가 존재해야 함
        assert locale["cli_mic_deviation_skipped_hpcomp"].strip(), locale_file.name


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
        "sidebar_output_recovery",
        "studio_recovery_title",
        "studio_recovery_subtitle",
        "recovery_action",
        "recovery_running_action",
        "recovery_source_hint",
        "recovery_include_hangloose",
        "recovery_preserve_hint",
        "recovery_idle_detail",
        "recovery_running_detail",
        "recovery_created_label",
        "recovery_existing_label",
    )

    for locale_file, locale in locales:
        if locale_file.name == "en.json":
            continue
        for key in visible_keys:
            assert locale[key] != english[key], f"{locale_file.name}:{key} is English fallback"


def test_webview_html_i18n_keys_exist_in_english() -> None:
    """Every data-i18n key in the WebView HTML must exist in en.json.

    A typo'd key silently renders as the raw key name in the WebView UI,
    so pin the HTML → locale linkage here (audit #138 F042).
    """
    _locale_dir, english, _locales = _load_locales()
    index_html = (
        Path(__file__).parent.parent / "webview_ui" / "index.html"
    ).read_text(encoding="utf-8")

    html_keys = set(re.findall(r'data-i18n="([^"]+)"', index_html))
    assert html_keys, "no data-i18n keys found — extraction regex is broken"

    missing = sorted(html_keys - set(english))
    assert not missing, f"index.html data-i18n keys missing from en.json: {missing}"


def test_bcp47_alias_files_mirror_canonical() -> None:
    """zh-cn/zh-tw are byte-mirrors of zh_CN/zh_TW.

    They are kept as BCP-47-style filename aliases for external consumers
    (audit #138 F030/Q3 — maintainer: do not delete). This test turns the
    accidental duplication into an enforced mirror contract so the pairs can
    never drift apart silently.
    """
    locale_dir = Path(__file__).parent.parent / "i18n" / "locales"
    for alias, canonical in (("zh-cn.json", "zh_CN.json"), ("zh-tw.json", "zh_TW.json")):
        assert (locale_dir / alias).read_bytes() == (locale_dir / canonical).read_bytes(), (
            f"{alias} must stay a byte-mirror of {canonical}"
        )


def test_traditional_chinese_is_a_real_translation() -> None:
    """zh_TW must remain genuinely Traditional, not a zh_CN copy."""
    locale_dir = Path(__file__).parent.parent / "i18n" / "locales"
    cn = json.loads((locale_dir / "zh_CN.json").read_text(encoding="utf-8"))
    tw = json.loads((locale_dir / "zh_TW.json").read_text(encoding="utf-8"))

    differing = sum(1 for key in cn if cn[key] != tw.get(key))
    assert differing > len(cn) * 0.5, "zh_TW looks like a zh_CN copy"

    tw_text = "".join(value for value in tw.values() if isinstance(value, str))
    traditional_only = "設應體錯誤"
    hits = sum(1 for ch in traditional_only if ch in tw_text)
    assert hits >= 3, "Traditional-specific characters missing from zh_TW"
