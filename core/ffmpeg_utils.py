# -*- coding: utf-8 -*-
"""Backward-compatible re-export shim; new code should import from
core.ffmpeg_discovery / core.audio_truehd directly.

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


def __getattr__(name):
    """Delegate attribute reads (lazy FFmpeg state) to ``core.ffmpeg_discovery``.

    ``core.ffmpeg_discovery`` owns the lazy-init
    module globals ``FFMPEG_PATH`` / ``FFPROBE_PATH`` / ``_FFMPEG_SETUP_DONE``
    (and the ``_FFMPEG_DETECTION_DONE`` / ``_FFMPEG_AUTO_INSTALL_ATTEMPTED``
    flags). ``ensure_ffmpeg_available()`` reassigns them, so a plain
    ``from core.ffmpeg_discovery import FFMPEG_PATH`` re-export here would bind
    the import-time value (``None``) forever and never reflect that mutation —
    leaving direct importers that read ``core.ffmpeg_utils.FFMPEG_PATH`` after
    setup with a stale ``None``. PEP 562 module ``__getattr__`` resolves the
    name against the discovery module on every access, so reads stay live and
    this shim keeps the backward-compatible surface it advertises.
    """
    import core.ffmpeg_discovery as _discovery

    try:
        return getattr(_discovery, name)
    except AttributeError:
        raise AttributeError(
            f"module {__name__!r} has no attribute {name!r}"
        ) from None
