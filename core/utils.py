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
import importlib.resources
from pathlib import Path

plt.rcParams['axes.unicode_minus'] = False

try:
    ADAPTIVE_PALETTE = Image.Palette.ADAPTIVE
except AttributeError:
    ADAPTIVE_PALETTE = getattr(Image, 'ADAPTIVE')

_font_configured = False


def set_matplotlib_font():
    """한글을 지원하는 폰트를 matplotlib에 설정합니다.

    Pretendard 번들 폰트를 우선 사용하고, 없으면 OS별 시스템 폰트로 대체합니다.
    한 번만 실행되며 이후 호출은 무시됩니다.
    """
    global _font_configured
    if _font_configured:
        return
    _font_configured = True

    system = platform.system()
    font_loaded = False

    # 1. 번들 Pretendard 폰트 시도
    font_search_paths = []

    try:
        if hasattr(importlib.resources, "files"):
            try:
                font_resource = (
                    importlib.resources.files("impulcifer_py313")
                    .joinpath("font")
                    .joinpath("Pretendard-Regular.otf")
                )
                font_search_paths.append(("bundled_files", font_resource))
            except (FileNotFoundError, ModuleNotFoundError):
                pass
    except Exception:
        pass

    # 2. 로컬 개발 환경 폰트 경로
    project_root = Path(__file__).parent.parent
    local_candidates = [
        project_root / "font" / "Pretendard-Regular.otf",
        project_root / "fonts" / "Pretendard-Regular.otf",
    ]
    for local_path in local_candidates:
        if local_path.exists():
            font_search_paths.append(("local", str(local_path)))
            break

    # 3. 시스템 설치 Pretendard
    try:
        if any(f.name == "Pretendard" for f in fm.fontManager.ttflist):
            font_search_paths.append(("system", "Pretendard"))
    except Exception:
        pass

    # 로딩 시도
    for source_type, font_path in font_search_paths:
        try:
            if source_type == "bundled_files":
                with importlib.resources.as_file(font_path) as fp:
                    fm.fontManager.addfont(str(fp))
                    prop = fm.FontProperties(fname=str(fp))
                    plt.rcParams["font.family"] = prop.get_name()
                    font_loaded = True
                    break
            elif source_type == "local":
                fm.fontManager.addfont(font_path)
                prop = fm.FontProperties(fname=font_path)
                plt.rcParams["font.family"] = prop.get_name()
                font_loaded = True
                break
            elif source_type == "system":
                plt.rcParams["font.family"] = "Pretendard"
                font_loaded = True
                break
        except Exception:
            continue

    # 4. OS 기본 한글 폰트 대체
    if not font_loaded:
        if system == "Windows":
            win_font = "C:/Windows/Fonts/malgun.ttf"
            if os.path.exists(win_font):
                prop = fm.FontProperties(fname=win_font)
                plt.rcParams["font.family"] = prop.get_name()
            else:
                plt.rcParams["font.family"] = "Malgun Gothic"
        elif system == "Darwin":
            plt.rcParams["font.family"] = "AppleGothic"
        elif system == "Linux":
            # NanumGothic 시도, 없으면 sans-serif 유지
            try:
                if any(f.name == "NanumGothic" for f in fm.fontManager.ttflist):
                    plt.rcParams["font.family"] = "NanumGothic"
            except Exception:
                pass

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
