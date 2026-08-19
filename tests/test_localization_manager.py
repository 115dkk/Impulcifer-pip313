"""Tests for the ``LocalizationManager.get()`` fallback contract (audit #138 F031/F055).

Managers are built via ``__new__`` to skip ``__init__``'s filesystem side
effects (settings dir creation and ``settings.json`` write).
"""

import json
import logging

from i18n import localization
from i18n.localization import LocalizationManager


def _manager_with(translations):
    mgr = LocalizationManager.__new__(LocalizationManager)
    mgr.translations = dict(translations)
    mgr.current_language = 'en'
    return mgr


def test_get_returns_translation_when_key_exists():
    mgr = _manager_with({'button_close': 'Close'})
    assert mgr.get('button_close', default='ignored') == 'Close'


def test_get_returns_default_on_missing_key():
    mgr = _manager_with({})
    assert mgr.get('dialog_recording_title', default='Recording') == 'Recording'


def test_get_returns_key_on_missing_key_without_default():
    mgr = _manager_with({})
    assert mgr.get('some_missing_key') == 'some_missing_key'


def test_get_formats_kwargs_into_default():
    mgr = _manager_with({})
    assert mgr.get('missing_fmt', default='Hello {name}', name='World') == 'Hello World'


def test_missing_key_warns_once(caplog):
    localization._MISSING_KEYS_WARNED.discard('warn_once_key')
    mgr = _manager_with({})
    with caplog.at_level(logging.WARNING, logger='i18n.localization'):
        mgr.get('warn_once_key')
        mgr.get('warn_once_key')
    assert sum('warn_once_key' in record.getMessage() for record in caplog.records) == 1


def test_existing_locale_missing_key_falls_back_to_english(tmp_path):
    (tmp_path / 'en.json').write_text(
        json.dumps({'shared': 'English shared', 'english_only': 'English fallback'}),
        encoding='utf-8',
    )
    (tmp_path / 'ko.json').write_text(
        json.dumps({'shared': '한국어 공통'}),
        encoding='utf-8',
    )
    mgr = _manager_with({})
    mgr.locales_dir = tmp_path

    mgr.load_translations('ko')

    assert mgr.get('shared') == '한국어 공통'
    assert mgr.get('english_only') == 'English fallback'


def test_normalize_language_code_accepts_bcp47_variants():
    from i18n.localization import normalize_language_code

    assert normalize_language_code('zh-CN') == 'zh_CN'
    assert normalize_language_code('zh-tw') == 'zh_TW'
    assert normalize_language_code('ko-KR') == 'ko_KR'
    assert normalize_language_code('zh_CN') == 'zh_CN'
    assert normalize_language_code('en') == 'en'
    assert normalize_language_code('') == ''
