# -*- coding: utf-8 -*-

import os
import subprocess
import tempfile
import json
import numpy as np
import soundfile as sf
from scipy import signal
from PIL import Image
import matplotlib.ticker as ticker
import matplotlib.pyplot as plt
import matplotlib.font_manager as fm
import platform
import shutil
from pathlib import Path

plt.rcParams['axes.unicode_minus'] = False

try:
    ADAPTIVE_PALETTE = Image.Palette.ADAPTIVE
except AttributeError:
    ADAPTIVE_PALETTE = getattr(Image, 'ADAPTIVE')

_font_configured = False
# Result of the most recent set_matplotlib_font() call. Public read-only
# diagnostic surface so the smoke-test in gui_main.py can verify Pretendard
# was actually applied (not just silently fell back to a system font).
#
# Shape:
#     {
#         "source": "bundled" | "system" | "fallback" | None,
#         "family": str | None,        # the family name written to rcParams
#         "path":   Path | None,       # the file findfont() resolved to
#         "is_pretendard": bool,       # True iff the resolved path is a
#                                       # Pretendard variant (.otf/.ttf with
#                                       # "pretendard" in the file name)
#     }
font_setup_result: dict = {
    "source": None,
    "family": None,
    "path": None,
    "is_pretendard": False,
}


def _resolve_bundled_font_dir() -> "Path | None":
    """Return the bundled ``font/`` directory across all runtime modes.

    Routes through :func:`infra.resource_helper.get_resource_path` first
    (Nuitka standalone / pip-install / dev), with a last-ditch
    ``Path(__file__).parent.parent`` fallback for the rare case ``infra``
    itself can't be imported (e.g. ad-hoc scripts).
    """
    try:
        from infra.resource_helper import get_resource_path

        candidate = Path(get_resource_path("font"))
        if candidate.is_dir():
            return candidate
    except Exception:
        pass

    project_root = Path(__file__).parent.parent
    for legacy in (project_root / "font", project_root / "fonts"):
        if legacy.is_dir():
            return legacy
    return None


def _scan_bundled_fonts() -> "list[Path]":
    """List every ``.otf`` / ``.ttf`` / ``.ttc`` bundled in the ``font/`` dir.

    Returns the files in case-insensitive name order so the same dev /
    standalone tree always yields the same registration order — important
    for matplotlib's ``findfont`` scoring when multiple bundled fonts
    declare the same family.
    """
    font_dir = _resolve_bundled_font_dir()
    if font_dir is None:
        return []
    suffixes = {".otf", ".ttf", ".ttc"}
    return sorted(
        (p for p in font_dir.iterdir() if p.suffix.lower() in suffixes),
        key=lambda p: p.name.casefold(),
    )


def _resolve_bundled_pretendard_path() -> "Path | None":
    """Find the bundled Pretendard regular weight (legacy compat name).

    Kept as a thin wrapper over :func:`_scan_bundled_fonts` so existing tests
    and old callers keep working. New code should prefer the scan-all helper
    so user-dropped fonts (e.g. a Source Han Serif placed alongside
    Pretendard) are also picked up.
    """
    for path in _scan_bundled_fonts():
        if "pretendard-regular" in path.stem.lower():
            return path
    # Pretendard not present? Return whatever Pretendard-shaped file we have.
    for path in _scan_bundled_fonts():
        if "pretendard" in path.stem.lower():
            return path
    return None


def _register_bundled_fonts_with_matplotlib() -> "list[Path]":
    """addfont() every bundled font for matplotlib and return the registered list.

    matplotlib's ``fontManager.addfont`` is idempotent (it deduplicates by
    file path), so this is safe to call multiple times. Used by
    :func:`set_matplotlib_font` so that BOTH Pretendard AND any extra Korean
    serif (e.g. Source Han Serif) the user drops into ``font/`` are
    available to matplotlib code that may want to reference them by family
    name.
    """
    registered: list[Path] = []
    for path in _scan_bundled_fonts():
        try:
            fm.fontManager.addfont(str(path))
            registered.append(path)
        except Exception:
            continue
    return registered


def set_matplotlib_font():
    """한글을 지원하는 폰트를 matplotlib에 설정한다.

    번들 Pretendard 우선 → 시스템 Pretendard → OS별 한글 폴백 순으로
    시도하며, 한 번만 실행되고 이후 호출은 캐시된 결과를 반환한다.

    이전 구현은 silent fallback이라 "Pretendard 적용에 실패해 Malgun으로
    떨어졌다"를 추적할 방법이 없었다. 이번 리팩토링은:

    1. 번들 경로 해석을 ``infra.resource_helper.get_font_path`` 로 일원화해
       Nuitka standalone 환경에서도 같은 경로 규칙을 따른다.
    2. 어떤 source가 채택됐는지(``bundled`` / ``system`` / ``fallback``)와
       findfont가 실제로 어떤 파일을 골랐는지를 ``font_setup_result`` 모듈
       전역에 기록한다 — smoke-test가 이를 보고 "Pretendard 보장" 검증을
       수행할 수 있다.

    Returns:
        ``font_setup_result`` 의 사본. ``is_pretendard`` 가 ``True`` 일 때
        만 호출자는 Pretendard 적용이 보장되었다고 간주해야 한다.
    """
    global _font_configured
    if _font_configured:
        return dict(font_setup_result)

    _font_configured = True

    system = platform.system()

    # Register EVERY bundled font (Pretendard + any user-dropped extras such
    # as Source Han Serif). matplotlib only renders text in the family set on
    # rcParams, but registering the others makes them addressable when code
    # explicitly opts-in via FontProperties(family=...).
    registered = _register_bundled_fonts_with_matplotlib()
    bundled_pretendard = next(
        (p for p in registered if "pretendard" in p.stem.lower()),
        None,
    )

    chosen_source = None
    chosen_family = None

    # 1) 번들 Pretendard (the default sans-serif body font)
    if bundled_pretendard is not None:
        try:
            prop = fm.FontProperties(fname=str(bundled_pretendard))
            family = prop.get_name()
            plt.rcParams["font.family"] = family
            chosen_source = "bundled"
            chosen_family = family
        except Exception:
            chosen_source = None  # fall through to system

    # 2) 시스템 설치 Pretendard
    if chosen_source is None:
        try:
            if any(f.name == "Pretendard" for f in fm.fontManager.ttflist):
                plt.rcParams["font.family"] = "Pretendard"
                chosen_source = "system"
                chosen_family = "Pretendard"
        except Exception:
            pass

    # 3) OS 한글 폴백
    if chosen_source is None:
        chosen_source = "fallback"
        if system == "Windows":
            win_font = "C:/Windows/Fonts/malgun.ttf"
            if os.path.exists(win_font):
                prop = fm.FontProperties(fname=win_font)
                family = prop.get_name()
                plt.rcParams["font.family"] = family
                chosen_family = family
            else:
                plt.rcParams["font.family"] = "Malgun Gothic"
                chosen_family = "Malgun Gothic"
        elif system == "Darwin":
            plt.rcParams["font.family"] = "AppleGothic"
            chosen_family = "AppleGothic"
        elif system == "Linux":
            try:
                if any(f.name == "NanumGothic" for f in fm.fontManager.ttflist):
                    plt.rcParams["font.family"] = "NanumGothic"
                    chosen_family = "NanumGothic"
            except Exception:
                pass

    # 4) findfont로 실제 결과 검증
    resolved_path = None
    is_pretendard = False
    try:
        resolved = fm.findfont(
            fm.FontProperties(family=chosen_family or "Pretendard"),
            fallback_to_default=True,
        )
        if resolved:
            resolved_path = Path(resolved)
            is_pretendard = "pretendard" in resolved_path.name.lower()
    except Exception:
        pass

    font_setup_result.update(
        {
            "source": chosen_source,
            "family": chosen_family,
            "path": resolved_path,
            "is_pretendard": is_pretendard,
        }
    )
    return dict(font_setup_result)

# -- Backward-compat re-exports (issue #87 Phase 5) --
# FFmpeg / TrueHD helpers were split into ``core.ffmpeg_utils``. We import
# them here so existing callers (including tests and GUI code) keep working
# without a churn of import statement updates.
from core.ffmpeg_utils import (  # noqa: E402,F401  (intentional re-export)
    MIN_FFMPEG_VERSION,
    get_ffmpeg_version,
    find_ffmpeg_in_common_paths,
    install_ffmpeg,
    setup_ffmpeg,
    ensure_ffmpeg_available,
    is_truehd_file,
    convert_truehd_to_wav,
    get_truehd_channel_info,
    read_audio,
    check_ffmpeg_available,
    get_supported_audio_formats,
)




def to_db(x):
    """Convert amplitude to dB

    Args:
        x: Amplitude value

    Returns:
        Value in dB
    """
    return 20 * np.log10(np.abs(x) + 1e-10)


def db_to_gain(x):
    """Convert dB to amplitude gain

    Args:
        x: Value in dB

    Returns:
        Amplitude gain
    """
    return 10 ** (x / 20)


def convolve(x, y):
    """Convolve two signals

    Args:
        x: First signal
        y: Second signal

    Returns:
        Convolved signal
    """
    return signal.convolve(x, y, mode='full')


def dB_unweight(x):
    """Remove dB weighting from a signal

    Args:
        x: Signal with dB weighting

    Returns:
        Signal without dB weighting
    """
    return 10 ** (x / 20)


def read_wav(file_path, expand=False):
    """Reads WAV file (backward compatibility wrapper)

    Args:
        file_path: Path to WAV file as string
        expand: Expand dimensions of a single track recording to produce 2-D array?

    Returns:
        - sampling frequency as integer
        - wav data as numpy array with one row per track, samples in range -1..1
    """
    fs, data, _ = read_audio(file_path, expand=expand)
    return fs, data


def write_wav(file_path, fs, data, bit_depth=32):
    """Writes WAV file."""
    # Ensure the directory exists before saving
    os.makedirs(os.path.dirname(file_path), exist_ok=True)
    
    if bit_depth == 16:
        subtype = "PCM_16"
    elif bit_depth == 24:
        subtype = "PCM_24"
    elif bit_depth == 32:
        subtype = "PCM_32"
    else:
        raise ValueError('Invalid bit depth. Accepted values are 16, 24 and 32.')
    if len(data.shape) > 1 and data.shape[1] > data.shape[0]:
        # We have tracks on rows, soundfile want's them on columns
        data = np.transpose(data)
    sf.write(file_path, data, samplerate=fs, subtype=subtype)


def magnitude_response(x, fs):
    """Calculates frequency magnitude response.

    Returns the same first ``ceil(N/2)`` bins as Lion's full-FFT implementation
    while only computing the one-sided real spectrum (``rfft`` is ~2x faster on
    real input). For real ``x`` we have ``fft(x)[k] == rfft(x)[k]`` for
    ``0 <= k <= N/2``, so the slice we expose here is bit-identical to Lion.
    """
    nfft = len(x)
    half = int(np.ceil(nfft / 2))
    X = np.fft.rfft(x)
    X_mag = 20 * np.log10(np.abs(X[:half]))
    f = np.arange(half) * (fs / nfft)
    return f, X_mag


def sync_axes(axes, sync_x=True, sync_y=True):
    """Synchronizes X and Y limits for axes

    Args:
        axes: List Axis objects
        sync_x: Flag depicting whether to sync X-axis
        sync_y: Flag depicting whether to sync Y-axis

    Returns:

    """
    x_min = []
    x_max = []
    y_min = []
    y_max = []
    for ax in axes:
        x_min.append(ax.get_xlim()[0])
        x_max.append(ax.get_xlim()[1])
        y_min.append(ax.get_ylim()[0])
        y_max.append(ax.get_ylim()[1])
    xlim = [np.min(x_min), np.max(x_max)]
    ylim = [np.min(y_min), np.max(y_max)]
    for ax in axes:
        if sync_x:
            ax.set_xlim(xlim)
        if sync_y:
            ax.set_ylim(ylim)


def get_ylim(x, padding=0.1):
    lower = np.min(x)
    upper = np.max(x)
    diff = upper - lower
    lower -= padding * diff
    upper += padding * diff
    return lower, upper


def versus_distance(angle=30, distance=3, breadth=0.148, ear='primary', sound_field='reverberant', sound_velocity=343):
    """Calculates speaker-ear distance delta, dealy delta and SPL delta

    Speaker-ear distance delta is the difference between distance from speaker to middle of the head and distance from
    speaker to ear.

    Dealy delta is the time it takes for sound to travel speaker-ear distance delta.

    SPL delta is the sound pressure level change in dB for a distance delta.

    Sound pressure attenuates by 3 dB for each distance doubling in reverberant room
    (http://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.10.1442&rep=rep1&type=pdf).

    Sound pressure attenuates by 6 dB for each distance doubling in free field and does not attenuate in diffuse field.

    Args:
        angle: Angle between center and the speaker in degrees
        distance: Distance from speaker to the middle of the head in meters
        breadth: Head breadth in meters
        ear: Which ear? "primary" for same side ear as the speaker or "secondary" for the opposite side
        sound_field: Sound field determines the attenuation over distance. 3 dB for "reverberant", 6 dB for "free"
                     and 0 dB for "diffuse"
        sound_velocity: The speed of sound in meters per second

    Returns:
        - Distance delta in meters
        - Delay delta in seconds
        - SPL delta in dB
    """
    if ear == 'primary':
        aa = (90 - angle) / 180 * np.pi
    elif ear == 'secondary':
        aa = (90 + angle) / 180 * np.pi
    else:
        raise ValueError('Ear must be "primary" or "secondary".')
    b = np.sqrt(distance ** 2 + (breadth / 2) ** 2 - 2 * distance * (breadth / 2) * np.cos(aa))
    d = b - distance
    delay = d / sound_velocity
    spl = np.log(b / distance) / np.log(2)
    if sound_field == 'reverberant':
        spl *= -3
    elif sound_field == 'free':
        spl *= -6
    elif sound_field == 'diffuse':
        spl *= -0
    else:
        raise ValueError('Sound field must be "reverberant", "free" or "diffuse".')
    return d, delay, spl


def optimize_png_size(file_path, n_colors=60):
    """Optimizes PNG file size in place.

    Args:
        file_path: Path to image
        n_colors: Number of colors in the PNG image

    Returns:
        None
    """
    im = Image.open(file_path)
    im = im.convert('P', palette=ADAPTIVE_PALETTE, colors=n_colors)
    im.save(file_path, optimize=True)


def save_fig_as_png(file_path, fig, n_colors=60):
    """Saves figure and optimizes file size."""
    # Ensure the directory exists before saving
    os.makedirs(os.path.dirname(file_path), exist_ok=True)
    
    fig.savefig(file_path, bbox_inches='tight')
    optimize_png_size(file_path, n_colors=n_colors)


def config_fr_axis(ax):
    """Configures given axis instance for frequency response plots."""
    ax.set_xlabel('Frequency (Hz)')
    ax.semilogx()
    ax.set_xlim([20, 20e3])
    ax.set_ylabel('Amplitude (dB)')
    ax.grid(True, which='major')
    ax.grid(True, which='minor')
    ax.xaxis.set_major_formatter(ticker.StrMethodFormatter('{x:.0f}'))


def running_mean(x, N):
    cumsum = np.cumsum(np.insert(x, 0, 0))
    return (cumsum[N:] - cumsum[:-N]) / float(N)
