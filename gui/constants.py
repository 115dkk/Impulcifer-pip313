#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Shared constants for the modern GUI.

Defined once at module level so changes propagate uniformly and repeated
dialog, widget, and file filter values never drift between call sites.
"""

# Window sizes (width, height)
WINDOW_MAIN_SIZE = (1000, 700)
DIALOG_PROCESSING_SIZE = (700, 500)
DIALOG_UPDATE_SIZE = (600, 500)
DIALOG_LANGUAGE_SIZE = (400, 550)

# Widget widths
WIDGET_BUTTON_WIDTH_BROWSE = 100
WIDGET_BUTTON_WIDTH_MEDIUM = 200
WIDGET_BUTTON_WIDTH_WIDE = 280
WIDGET_ENTRY_WIDTH_DEFAULT = 80
WIDGET_ENTRY_WIDTH_NARROW = 60
WIDGET_ENTRY_WIDTH_TINY = 50
WIDGET_OPTION_WIDTH_DEFAULT = 150
WIDGET_OPTION_WIDTH_NARROW = 120
WIDGET_PROGRESS_BAR_WIDTH = 660
WIDGET_LOG_TEXTBOX_WIDTH = 660
WIDGET_NOTES_TEXTBOX_WIDTH = 560

FILETYPES_AUDIO = [
    ('Audio files', '*.wav *.mlp *.thd *.truehd'),
    ('WAV files', '*.wav'),
    ('TrueHD/MLP files', '*.mlp *.thd *.truehd'),
    ('All files', '*.*'),
]

FILETYPES_AUDIO_WITH_PKL = [
    ('Audio files', '*.wav *.pkl *.mlp *.thd *.truehd'),
    ('WAV files', '*.wav'),
    ('Pickle files', '*.pkl'),
    ('TrueHD/MLP files', '*.mlp *.thd *.truehd'),
    ('All files', '*.*'),
]

FILETYPES_TEXT = [
    ('Text files', '*.csv *.txt'),
    ('All files', '*.*'),
]

FILETYPES_WAV = [
    ('Audio files', '*.wav'),
    ('All files', '*.*'),
]

FILETYPES_WAV_SAVE = [
    ('WAV file', '*.wav'),
    ('All files', '*.*'),
]
