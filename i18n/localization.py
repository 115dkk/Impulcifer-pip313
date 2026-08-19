#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Localization system for Impulcifer
Supports multiple languages with automatic system language detection
"""

import json
import locale
import logging
from pathlib import Path
from typing import Dict, Optional

# Supported languages with their locale codes
SUPPORTED_LANGUAGES = {
    'en': 'English',
    'ko': '한국어',
    'fr': 'Français',
    'de': 'Deutsch',
    'es': 'Español',
    'ja': '日本語',
    'zh_CN': '简体中文',
    'zh_TW': '繁體中文',
    'ru': 'Русский'
}

# Missing-key warnings are deduplicated process-wide so a key that renders on
# every GUI refresh does not flood the log.
_missing_key_log = logging.getLogger(__name__)
_MISSING_KEYS_WARNED: set = set()

# Mapping from system locale to our language codes
LOCALE_MAPPING = {
    'en': 'en', 'en_US': 'en', 'en_GB': 'en', 'en_CA': 'en', 'en_AU': 'en',
    'ko': 'ko', 'ko_KR': 'ko',
    'fr': 'fr', 'fr_FR': 'fr', 'fr_CA': 'fr', 'fr_BE': 'fr', 'fr_CH': 'fr',
    'de': 'de', 'de_DE': 'de', 'de_AT': 'de', 'de_CH': 'de',
    'es': 'es', 'es_ES': 'es', 'es_MX': 'es', 'es_AR': 'es', 'es_CO': 'es',
    'ja': 'ja', 'ja_JP': 'ja',
    'zh': 'zh_CN', 'zh_CN': 'zh_CN', 'zh_SG': 'zh_CN',
    'zh_TW': 'zh_TW', 'zh_HK': 'zh_TW',
    'ru': 'ru', 'ru_RU': 'ru'
}


def normalize_language_code(code: str) -> str:
    """Canonicalize a language code to the ``SUPPORTED_LANGUAGES`` form.

    Accepts BCP-47-style variants (``zh-CN``, ``zh-tw``, ``ko-KR``) from web
    or OS contexts and maps them to the underscore form (``zh_CN``). The
    hyphenated ``i18n/locales/zh-cn.json``/``zh-tw.json`` files are kept as
    byte-mirrors of the canonical files for external BCP-47-style consumers
    (audit #138 F030/Q3); the loader itself always reads the canonical file.
    """
    if not code:
        return code
    normalized = code.replace('-', '_')
    parts = normalized.split('_')
    if len(parts) == 2:
        normalized = parts[0].lower() + '_' + parts[1].upper()
    return normalized


class LocalizationManager:
    """Manages translations and user preferences"""

    @staticmethod
    def _is_valid_locales_dir(path: Path) -> bool:
        """A locales directory only counts if it actually holds the base
        translation (``en.json``).

        Pre-2.4.27 wheels shipped the JSON via a ``shared-data`` mapping that
        installed a copy to ``<sys.prefix>/impulcifer_py313/locales``. Upgrading
        removes those files but leaves the now-empty directory behind. The old
        resolver accepted any directory that merely ``.exists()``, so that stale
        empty folder won — and every GUI string rendered as its raw i18n key on
        pip-upgraded environments. Validating by content rejects the decoy.
        """
        try:
            return (path / 'en.json').is_file()
        except OSError:
            return False

    def _find_locales_dir(self) -> Path:
        """Find the locales directory, preferring the actually-imported
        ``i18n`` package's own data.

        ``importlib.resources.files('i18n')`` anchors to wherever the running
        ``i18n`` package physically lives — the exact same install as this
        module — so it cannot be fooled by a stale ``impulcifer_py313`` folder
        under ``sys.prefix``.
        """
        candidates = []

        # 1. The real package's bundled data — canonical, install-agnostic.
        try:
            import importlib.resources as importlib_resources
            if hasattr(importlib_resources, 'files'):  # Python 3.9+
                candidates.append(Path(str(importlib_resources.files('i18n'))) / 'locales')
        except (ImportError, ModuleNotFoundError, AttributeError, TypeError):
            pass

        # 2. File-relative — sibling of this module (source checkout & wheel).
        candidates.append(Path(__file__).resolve().parent / 'locales')

        # 3. Same root that infra.resource_helper uses for every other bundled
        #    resource, so locale resolution matches data/font/img in both pip
        #    and Nuitka standalone builds.
        try:
            from infra.resource_helper import get_resource_path
            candidates.append(Path(get_resource_path('i18n/locales')))
        except Exception:
            pass

        # 4. Project-root layout (running from a source tree).
        candidates.append(Path(__file__).resolve().parent.parent / 'i18n' / 'locales')

        # 5. Legacy pre-2.4.27 shared-data location. Honoured only if it still
        #    holds the JSON (an old install that was never upgraded); the empty
        #    leftover from an upgrade fails the content check above.
        import sys
        for base in {Path(sys.prefix), Path(getattr(sys, 'base_prefix', sys.prefix))}:
            candidates.append(base / 'impulcifer_py313' / 'locales')
            candidates.append(base / 'share' / 'impulcifer_py313' / 'locales')
            candidates.append(base / 'lib' / 'impulcifer_py313' / 'locales')

        seen = set()
        for path in candidates:
            key = str(path)
            if key in seen:
                continue
            seen.add(key)
            if self._is_valid_locales_dir(path):
                return path

        # Nothing valid found. Return the package-relative path (without
        # creating an empty decoy) so the diagnostic in load_translations
        # points at the real expected location.
        return Path(__file__).resolve().parent / 'locales'

    def __init__(self):
        self.current_language = 'en'
        self.translations: Dict[str, str] = {}
        self.settings_dir = Path.home() / '.impulcifer'
        self.settings_file = self.settings_dir / 'settings.json'

        self.locales_dir = self._find_locales_dir()

        self.settings_dir.mkdir(exist_ok=True)

        self.settings = self.load_settings()

        if 'language' in self.settings:
            self.set_language(self.settings['language'])
        else:
            detected_lang = self.detect_system_language()
            self.set_language(detected_lang)

    def detect_system_language(self) -> str:
        """Detect system language"""
        try:
            system_locale = locale.getdefaultlocale()[0]
            if system_locale:
                for loc_code, lang_code in LOCALE_MAPPING.items():
                    if system_locale.startswith(loc_code):
                        return lang_code
        except Exception:
            pass

        return 'en'

    def load_settings(self) -> dict:
        """Load user settings from file"""
        if self.settings_file.exists():
            try:
                with open(self.settings_file, 'r', encoding='utf-8') as f:
                    return json.load(f)
            except Exception:
                pass
        return {}

    def save_settings(self):
        """Save user settings to file"""
        try:
            with open(self.settings_file, 'w', encoding='utf-8') as f:
                json.dump(self.settings, f, ensure_ascii=False, indent=2)
        except Exception as e:
            print(f"Failed to save settings: {e}")

    def set_language(self, language_code: str):
        """Set current language and load translations"""
        language_code = normalize_language_code(language_code)
        if language_code not in SUPPORTED_LANGUAGES:
            language_code = 'en'

        self.current_language = language_code
        self.settings['language'] = language_code
        self.save_settings()

        self.load_translations(language_code)

    def load_translations(self, language_code: str):
        """Load English as a base and overlay the requested language."""
        english_file = self.locales_dir / 'en.json'
        try:
            with open(english_file, 'r', encoding='utf-8') as f:
                translations = json.load(f)
        except Exception as e:
            print(f"Failed to load translations for en: {e}")
            translations = {}

        if language_code == 'en':
            self.translations = translations
            return

        locale_file = self.locales_dir / f'{language_code}.json'
        if not locale_file.exists():
            print(f"Translation file not found: {locale_file}")
            print(f"Locales directory: {self.locales_dir}")
            print(f"Directory exists: {self.locales_dir.exists()}")
            if self.locales_dir.exists():
                print(f"Files in locales dir: {list(self.locales_dir.glob('*.json'))}")
            self.translations = translations
            return

        try:
            with open(locale_file, 'r', encoding='utf-8') as f:
                translations.update(json.load(f))
        except Exception as e:
            print(f"Failed to load translations for {language_code}: {e}")

        self.translations = translations

    def get(self, key: str, default: Optional[str] = None, **kwargs) -> str:
        """Get translated text for a key.

        Returns ``default`` when the key has no translation; without a
        default the key itself is returned (legacy behavior for call sites
        that pass none).
        """
        text = self.translations.get(key)
        if text is None:
            if key not in _MISSING_KEYS_WARNED:
                _MISSING_KEYS_WARNED.add(key)
                _missing_key_log.warning(
                    "Missing translation key '%s' (language '%s')",
                    key, self.current_language,
                )
            text = key if default is None else default

        if kwargs:
            try:
                text = text.format(**kwargs)
            except Exception:
                pass

        return text

    def get_language_name(self, language_code: str) -> str:
        """Get the display name of a language"""
        return SUPPORTED_LANGUAGES.get(language_code, language_code)

    def get_all_languages(self) -> Dict[str, str]:
        """Get all supported languages"""
        return SUPPORTED_LANGUAGES.copy()

    def set_theme(self, theme: str):
        """Set theme preference"""
        self.settings['theme'] = theme
        self.save_settings()

    def get_theme(self) -> str:
        """Get theme preference"""
        return self.settings.get('theme', 'dark')

    def set_skin(self, skin: str):
        """Persist the GUI skin choice (``stable`` or ``studio``).

        Stable is the compact tabview the app has always shipped (now
        wearing the Pulse palette). Studio is the sidebar + card-based
        layout introduced alongside the redesign. The choice is stored in
        the same settings file as language/theme so it survives across
        launches without an extra file.
        """
        if skin not in ('stable', 'studio'):
            skin = 'stable'
        self.settings['skin'] = skin
        self.save_settings()

    def get_skin(self) -> str:
        """Return the persisted skin choice (default ``stable``)."""
        return self.settings.get('skin', 'stable')

    def set_frontend(self, frontend: str):
        """Persist which GUI frontend gui_main launches by default.

        ``webview`` is the platform WebView frontend (the default since
        2.10); ``ctk`` keeps the CustomTkinter interface, which stays fully
        supported (maintenance and new features) for the rest of version 2
        and is kept as a frozen legacy interface from version 3 on (only the
        old legacy GUI is removed in version 3).
        """
        if frontend not in ('webview', 'ctk'):
            frontend = 'webview'
        self.settings['frontend'] = frontend
        self.save_settings()

    def get_frontend(self) -> str:
        """Return the persisted frontend choice (default ``webview``)."""
        return self.settings.get('frontend', 'webview')

    def is_first_run(self) -> bool:
        """Check if this is the first run (no language setting)"""
        return 'language' not in self.settings or not self.settings.get('language_selected', False)

    def mark_language_selected(self):
        """Mark that user has selected a language"""
        self.settings['language_selected'] = True
        self.save_settings()


# Global instance
_localization_manager: Optional[LocalizationManager] = None


def get_localization_manager() -> LocalizationManager:
    """Get global localization manager instance"""
    global _localization_manager
    if _localization_manager is None:
        _localization_manager = LocalizationManager()
    return _localization_manager


def t(key: str, default: Optional[str] = None, **kwargs) -> str:
    """Shorthand for getting translated text"""
    return get_localization_manager().get(key, default=default, **kwargs)
