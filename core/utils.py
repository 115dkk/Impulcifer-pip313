# -*- coding: utf-8 -*-
"""Backward-compatible re-export shim; new code should import from the
focused modules below directly.

core/utils.py was a three-domain grab-bag; it is now split into focused modules
and re-exports their public surface so the 25+ existing importers keep working:
  - core.audio_io      : WAV I/O + DSP (read_wav, write_wav, magnitude_response, ...)
  - core.font_setup    : matplotlib Korean font setup (set_matplotlib_font, ...)
  - core.plotting_utils  : plot/PNG helpers (sync_axes, save_fig_as_png, ...)
  - core.ffmpeg_discovery: FFmpeg discovery/install and lazy setup
  - core.audio_truehd    : TrueHD/MLP decode and TrueHD-aware audio reading
"""

from core.audio_io import (  # noqa: F401  (intentional re-export)
    to_db,
    db_to_gain,
    convolve,
    dB_unweight,
    read_wav,
    write_wav,
    magnitude_response,
    running_mean,
)
from core.font_setup import (  # noqa: F401  (intentional re-export)
    set_matplotlib_font,
    font_setup_result,
)
from core.plotting_utils import (  # noqa: F401  (intentional re-export)
    ADAPTIVE_PALETTE,
    sync_axes,
    get_ylim,
    versus_distance,
    optimize_png_size,
    save_fig_as_png,
    config_fr_axis,
)
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
