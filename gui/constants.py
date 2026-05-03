#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Shared file dialog filter constants for the modern GUI.

Defined once at module level so changes propagate uniformly and identical
filters never drift between call sites.
"""

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
