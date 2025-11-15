#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Localization system for Impulcifer
Supports multiple languages with automatic system language detection
"""

import json
import locale
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


class LocalizationManager:
    """Manages translations and user preferences"""

    def __init__(self):
        self.current_language = 'en'
        self.translations: Dict[str, str] = {}
        self.settings_dir = Path.home() / '.impulcifer'
        self.settings_file = self.settings_dir / 'settings.json'
        self.locales_dir = Path(__file__).parent / 'locales'

        # Ensure directories exist
        self.settings_dir.mkdir(exist_ok=True)
        self.locales_dir.mkdir(exist_ok=True)

        # Load settings
        self.settings = self.load_settings()

        # Set language (from settings or detect system)
        if 'language' in self.settings:
            self.set_language(self.settings['language'])
        else:
            detected_lang = self.detect_system_language()
            self.set_language(detected_lang)

    def detect_system_language(self) -> str:
        """Detect system language"""
        try:
            # Try to get system locale
            system_locale = locale.getdefaultlocale()[0]
            if system_locale:
                # Map to our language code
                for loc_code, lang_code in LOCALE_MAPPING.items():
                    if system_locale.startswith(loc_code):
                        return lang_code
        except Exception:
            pass

        # Default to English if detection fails
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
        if language_code not in SUPPORTED_LANGUAGES:
            language_code = 'en'

        self.current_language = language_code
        self.settings['language'] = language_code
        self.save_settings()

        # Load translation file
        self.load_translations(language_code)

    def load_translations(self, language_code: str):
        """Load translation file for specified language"""
        locale_file = self.locales_dir / f'{language_code}.json'

        if locale_file.exists():
            try:
                with open(locale_file, 'r', encoding='utf-8') as f:
                    self.translations = json.load(f)
                return
            except Exception as e:
                print(f"Failed to load translations for {language_code}: {e}")

        # Fallback to English
        if language_code != 'en':
            self.load_translations('en')
        else:
            self.translations = {}

    def get(self, key: str, **kwargs) -> str:
        """Get translated text for a key"""
        text = self.translations.get(key, key)

        # Format with kwargs if provided
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


def t(key: str, **kwargs) -> str:
    """Shorthand for getting translated text"""
    return get_localization_manager().get(key, **kwargs)
