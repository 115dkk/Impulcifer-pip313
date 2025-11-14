#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Unified logging system for Impulcifer
Supports both CLI output and GUI callbacks
"""

import sys
from typing import Callable, Optional
from enum import Enum


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
    Unified logger that can output to console and/or GUI

    Usage:
        logger = ImpulciferLogger()
        logger.info("Processing started")
        logger.progress(30, "Processing impulse responses...")
        logger.success("Done!")
    """

    def __init__(self):
        self.gui_callback: Optional[Callable] = None
        self.progress_callback: Optional[Callable] = None
        self.enabled = True
        self.total_steps = 100  # Default total steps for progress
        self.current_step = 0

    def set_gui_callback(self, callback: Callable):
        """Set callback for GUI log output"""
        self.gui_callback = callback

    def set_progress_callback(self, callback: Callable):
        """Set callback for GUI progress updates"""
        self.progress_callback = callback

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

    def _log(self, level: LogLevel, message: str, progress_value: Optional[int] = None):
        """Internal logging method"""
        if not self.enabled:
            return

        # Format message with level for console
        if level == LogLevel.PROGRESS:
            console_msg = f"[{progress_value}%] {message}"
        elif level == LogLevel.SUCCESS:
            console_msg = f"✓ {message}"
        elif level == LogLevel.ERROR:
            console_msg = f"✗ {message}"
        elif level == LogLevel.WARNING:
            console_msg = f"⚠ {message}"
        else:
            console_msg = message

        # Output to console
        print(console_msg)

        # Output to GUI if callback is set
        if self.gui_callback:
            try:
                self.gui_callback(level.value, message)
            except Exception as e:
                print(f"Error in GUI callback: {e}")

        # Update progress if callback is set
        if level == LogLevel.PROGRESS and self.progress_callback and progress_value is not None:
            try:
                self.progress_callback(progress_value, message)
            except Exception as e:
                print(f"Error in progress callback: {e}")

    def debug(self, message: str):
        """Log debug message"""
        self._log(LogLevel.DEBUG, message)

    def info(self, message: str):
        """Log info message"""
        self._log(LogLevel.INFO, message)

    def warning(self, message: str):
        """Log warning message"""
        self._log(LogLevel.WARNING, message)

    def error(self, message: str):
        """Log error message"""
        self._log(LogLevel.ERROR, message)

    def success(self, message: str):
        """Log success message"""
        self._log(LogLevel.SUCCESS, message)

    def progress(self, value: int, message: str = ""):
        """
        Update progress

        Args:
            value: Progress percentage (0-100)
            message: Optional message describing current operation
        """
        self._log(LogLevel.PROGRESS, message, value)

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
