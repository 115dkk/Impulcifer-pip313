# -*- coding: utf-8 -*-
"""TrueHD/MLP detection, conversion and the TrueHD-aware audio reader."""

import json
import os
import subprocess
import tempfile

import numpy as np
import soundfile as sf

from core.ffmpeg_discovery import check_ffmpeg_available, get_ffmpeg_paths


def is_truehd_file(file_path):
    """Check if file is TrueHD/MLP format"""
    # TrueHD/MLP 처리 경로 — 자동 설치를 허용한다.
    ffmpeg_paths = get_ffmpeg_paths(auto_install=True)
    if ffmpeg_paths is None:
        return False
    _, ffprobe_path = ffmpeg_paths

    try:
        result = subprocess.run(
            [ffprobe_path, '-v', 'error', '-select_streams', 'a:0',
             '-show_entries', 'stream=codec_name', '-of', 'default=noprint_wrappers=1:nokey=1',
             file_path],
            capture_output=True, text=True, timeout=10,
            encoding='utf-8', errors='replace'
        )
        if result.returncode != 0:
            return False

        codec = result.stdout.strip().lower()
        return codec in ['truehd', 'mlp']
    except Exception:
        return False


def convert_truehd_to_wav(truehd_path, output_path=None):
    """Convert TrueHD/MLP file to WAV format"""
    ffmpeg_paths = get_ffmpeg_paths(auto_install=True)
    if ffmpeg_paths is None:
        raise RuntimeError("FFmpeg is not available for TrueHD conversion")
    ffmpeg_path, _ = ffmpeg_paths

    if output_path is None:
        fd, output_path = tempfile.mkstemp(suffix='.wav')
        os.close(fd)

    channel_info = get_truehd_channel_info(truehd_path)

    cmd = [
        ffmpeg_path, '-i', truehd_path,
        '-acodec', 'pcm_f32le',  # 32-bit float PCM
        '-ar', '48000',
        output_path, '-y'
    ]

    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60,
                          encoding='utf-8', errors='replace')
    if result.returncode != 0:
        raise RuntimeError(f"FFmpeg conversion failed: {result.stderr}")

    return output_path, channel_info


def get_truehd_channel_info(file_path):
    """Get channel layout information from TrueHD file"""
    ffmpeg_paths = get_ffmpeg_paths(auto_install=True)
    if ffmpeg_paths is None:
        return None
    _, ffprobe_path = ffmpeg_paths

    try:
        result = subprocess.run(
            [ffprobe_path, '-v', 'error', '-select_streams', 'a:0',
             '-show_entries', 'stream=channel_layout,channels',
             '-of', 'json', file_path],
            capture_output=True, text=True, timeout=10,
            encoding='utf-8', errors='replace'
        )

        if result.returncode != 0:
            return None

        info = json.loads(result.stdout)
        stream = info['streams'][0]

        channels = stream.get('channels', 0)
        stream.get('channel_layout', '')

        from core.constants import CHANNEL_LAYOUT_MAP

        if channels in CHANNEL_LAYOUT_MAP:
            return CHANNEL_LAYOUT_MAP[channels]
        else:
            return None
    except Exception:
        return None


def get_truehd_profile(file_path):
    """Return the TrueHD/MLP codec profile string, or ``None``.

    Used to tell ordinary TrueHD layouts apart from Atmos-object
    masters. ffprobe reports ``profile`` as e.g. ``"Dolby TrueHD"`` for
    a plain 5.1/7.1 stream, or ``"Dolby TrueHD + Dolby Atmos"`` for an
    Atmos master whose height/wide objects FFmpeg cannot decode into
    discrete channels (it only emits the 7.1 bed). Callers reject the
    Atmos case while letting ordinary TrueHD play with its decoded PCM.
    """
    ffmpeg_paths = get_ffmpeg_paths(auto_install=True)
    if ffmpeg_paths is None:
        return None
    _, ffprobe_path = ffmpeg_paths

    try:
        result = subprocess.run(
            [ffprobe_path, '-v', 'error', '-select_streams', 'a:0',
             '-show_entries', 'stream=profile',
             '-of', 'default=noprint_wrappers=1:nokey=1', file_path],
            capture_output=True, text=True, timeout=10,
            encoding='utf-8', errors='replace'
        )
        if result.returncode != 0:
            return None
        profile = result.stdout.strip()
        return profile or None
    except Exception:
        return None


def is_truehd_atmos_object_master(file_path):
    """``True`` when the file carries Atmos object content over a bed.

    These files (the bundled ``11cmaster.mlp`` / ``13cmaster.mlp``)
    decode to only the 7.1 bed under FFmpeg — the extra height/wide
    objects the file name promises are lost, so they're unsuitable as
    a discrete-channel sweep source. Plain ``Dolby TrueHD`` 5.1/7.1
    streams return ``False`` and remain playable.
    """
    profile = get_truehd_profile(file_path)
    if not profile:
        return False
    return 'atmos' in profile.lower()


# TrueHD/MLP 확장자 집합 — read_audio()의 fast-path 분기에 사용한다.
_TRUEHD_EXTENSIONS = frozenset({'.mlp', '.thd', '.truehd'})


def read_audio(file_path, expand=False):
    """Read audio file (WAV or TrueHD/MLP)

    Returns:
        - Sample rate
        - Audio data (channels x samples)
        - Channel info (for TrueHD) or None
    """
    # 일반 WAV 등 비-TrueHD 확장자는 FFmpeg setup 자체를 건너뛴다. 확장자
    # 기반 빠른 분기로 모듈 import / 일반 처리 경로의 ffmpeg 탐색 비용을 제거.
    ext = os.path.splitext(file_path)[1].lower()
    if ext in _TRUEHD_EXTENSIONS and is_truehd_file(file_path):
        temp_wav, channel_info = convert_truehd_to_wav(file_path)
        try:
            data, fs = sf.read(temp_wav)
            if len(data.shape) > 1:
                # Soundfile has tracks on columns, we want them on rows
                data = np.transpose(data)
            elif expand:
                data = np.expand_dims(data, axis=0)

            return fs, data, channel_info
        finally:
            if os.path.exists(temp_wav):
                os.remove(temp_wav)
    else:
        data, fs = sf.read(file_path)
        if len(data.shape) > 1:
            # Soundfile has tracks on columns, we want them on rows
            data = np.transpose(data)
        elif expand:
            data = np.expand_dims(data, axis=0)
        return fs, data, None


def get_supported_audio_formats():
    """Get list of supported audio formats"""
    formats = {
        'wav': 'WAV Audio',
        'mlp': 'TrueHD/MLP Audio',
        'thd': 'TrueHD Audio',
        'truehd': 'Dolby TrueHD'
    }

    # 정보 조회용이므로 자동 설치는 트리거하지 않는다.
    if not check_ffmpeg_available(auto_install=False):
        # If FFmpeg not available, only support WAV
        return {'wav': 'WAV Audio'}

    return formats
