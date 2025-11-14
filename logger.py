#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Unified logging system for Impulcifer
Supports both CLI output and GUI callbacks with localization
"""

from typing import Callable, Optional, TYPE_CHECKING
from enum import Enum

if TYPE_CHECKING:
    from localization import LocalizationManager


class LogLevel(Enum):
    """Log message severity levels"""
    DEBUG = "DEBUG"
    INFO = "INFO"
    WARNING = "WARNING"
    ERROR = "ERROR"
    SUCCESS = "SUCCESS"
    PROGRESS = "PROGRESS"  # Special level for progress updates


class ImpulciferLogger:
    """
    Unified logger that can output to console and/or GUI with localization support

    Usage:
        logger = ImpulciferLogger()
        logger.set_localization(loc_manager)  # Enable translations
        logger.info("cli_creating_estimator")  # Translation key
        logger.progress(30, "cli_processing")
        logger.success("cli_success_complete")
    """

    def __init__(self):
        self.gui_callback: Optional[Callable] = None
        self.progress_callback: Optional[Callable] = None
        self.localization: Optional['LocalizationManager'] = None
        self.enabled = True
        self.total_steps = 100  # Default total steps for progress
        self.current_step = 0

    def set_localization(self, loc_manager: 'LocalizationManager'):
        """Set localization manager for translating messages"""
        self.localization = loc_manager

    def set_gui_callback(self, callback: Callable):
        """Set callback for GUI log output"""
        self.gui_callback = callback

    def set_progress_callback(self, callback: Callable):
        """Set callback for GUI progress updates"""
        self.progress_callback = callback

    def _translate(self, message: str, **kwargs) -> str:
        """
        Translate message if it's a translation key, otherwise return as-is

        Args:
            message: Message string or translation key
            **kwargs: Format parameters for translation

        Returns:
            Translated and formatted message
        """
        if not self.localization:
            return message

        # If message starts with common prefixes, treat as translation key
        if message.startswith(('cli_', 'message_', 'error_', 'warning_', 'success_', 'info_')):
            return self.localization.get(message, **kwargs)

        # Otherwise return as-is (allows mixing translated and non-translated messages)
        return message

    def set_total_steps(self, total: int):
        """Set total number of steps for automatic progress calculation"""
        self.total_steps = total
        self.current_step = 0

    def step(self, message: str = ""):
        """Increment step counter and update progress"""
        self.current_step += 1
        progress = int((self.current_step / self.total_steps) * 100)
        if message:
            self.progress(progress, message)
        else:
            self.progress(progress, f"Step {self.current_step}/{self.total_steps}")

    def disable(self):
        """Disable all logging output"""
        self.enabled = False

    def enable(self):
        """Enable logging output"""
        self.enabled = True

    def _log(self, level: LogLevel, message: str, progress_value: Optional[int] = None, **kwargs):
        """
        Internal logging method with translation support

        Args:
            level: Log level
            message: Message string or translation key
            progress_value: Optional progress percentage (0-100)
            **kwargs: Format parameters for translation
        """
        if not self.enabled:
            return

        # Translate message if it's a key
        translated_msg = self._translate(message, **kwargs)

        # Format message with level for console
        if level == LogLevel.PROGRESS:
            console_msg = f"[{progress_value}%] {translated_msg}"
        elif level == LogLevel.SUCCESS:
            console_msg = f"✓ {translated_msg}"
        elif level == LogLevel.ERROR:
            console_msg = f"✗ {translated_msg}"
        elif level == LogLevel.WARNING:
            console_msg = f"⚠ {translated_msg}"
        else:
            console_msg = translated_msg

        # Output to console
        print(console_msg)

        # Output to GUI if callback is set
        if self.gui_callback:
            try:
                self.gui_callback(level.value, translated_msg)
            except Exception as e:
                print(f"Error in GUI callback: {e}")

        # Update progress if callback is set
        if level == LogLevel.PROGRESS and self.progress_callback and progress_value is not None:
            try:
                self.progress_callback(progress_value, translated_msg)
            except Exception as e:
                print(f"Error in progress callback: {e}")

    def debug(self, message: str, **kwargs):
        """Log debug message (supports translation keys)"""
        self._log(LogLevel.DEBUG, message, **kwargs)

    def info(self, message: str, **kwargs):
        """Log info message (supports translation keys)"""
        self._log(LogLevel.INFO, message, **kwargs)

    def warning(self, message: str, **kwargs):
        """Log warning message (supports translation keys)"""
        self._log(LogLevel.WARNING, message, **kwargs)

    def error(self, message: str, **kwargs):
        """Log error message (supports translation keys)"""
        self._log(LogLevel.ERROR, message, **kwargs)

    def success(self, message: str, **kwargs):
        """Log success message (supports translation keys)"""
        self._log(LogLevel.SUCCESS, message, **kwargs)

    def progress(self, value: int, message: str = "", **kwargs):
        """
        Update progress (supports translation keys)

        Args:
            value: Progress percentage (0-100)
            message: Optional message describing current operation (can be translation key)
            **kwargs: Format parameters for translation
        """
        self._log(LogLevel.PROGRESS, message, value, **kwargs)

    def separator(self):
        """Print a separator line"""
        self.info("-" * 60)


# Global logger instance
_logger: Optional[ImpulciferLogger] = None


def get_logger() -> ImpulciferLogger:
    """Get or create global logger instance"""
    global _logger
    if _logger is None:
        _logger = ImpulciferLogger()
    return _logger


def set_gui_callbacks(log_callback: Optional[Callable] = None,
                      progress_callback: Optional[Callable] = None):
    """
    Convenience function to set GUI callbacks on global logger

    Args:
        log_callback: Function(level: str, message: str) to display log messages
        progress_callback: Function(progress: int, message: str) to update progress
    """
    logger = get_logger()
    if log_callback:
        logger.set_gui_callback(log_callback)
    if progress_callback:
        logger.set_progress_callback(progress_callback)
