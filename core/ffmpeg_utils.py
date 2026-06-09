# -*- coding: utf-8 -*-
"""Backward-compatible re-export shim (audit #115 finding 9).

FFmpeg discovery/install moved to :mod:`core.ffmpeg_discovery`; TrueHD/MLP
decode and the TrueHD-aware ``read_audio`` moved to :mod:`core.audio_truehd`.
Existing importers (``core.utils``, the ``gui_main`` smoke test) keep importing
``core.ffmpeg_utils`` unchanged.
"""

from core.ffmpeg_discovery import (  # noqa: F401  (intentional re-export)
    MIN_FFMPEG_VERSION,
    get_ffmpeg_version,
    find_ffmpeg_in_common_paths,
    install_ffmpeg,
    setup_ffmpeg,
    ensure_ffmpeg_available,
    check_ffmpeg_available,
)
from core.audio_truehd import (  # noqa: F401  (intentional re-export)
    is_truehd_file,
    convert_truehd_to_wav,
    get_truehd_channel_info,
    get_truehd_profile,
    is_truehd_atmos_object_master,
    read_audio,
    get_supported_audio_formats,
)
