# -*- coding: utf-8 -*-
"""FFmpeg detection / installation and TrueHD-MLP audio routing.

Phase 5 of issue #87 split this section out of ``core/utils.py`` (~440 lines).
``core.utils`` now re-exports the public functions for backward compatibility,
so existing callers continue to import from ``core.utils`` without changes.

Public surface:
    - ``MIN_FFMPEG_VERSION`` (constant)
    - ``get_ffmpeg_version(path)``
    - ``find_ffmpeg_in_common_paths()``
    - ``install_ffmpeg()``
    - ``setup_ffmpeg(auto_install=True)``
    - ``ensure_ffmpeg_available(auto_install=True)``
    - ``check_ffmpeg_available(auto_install=False)``
    - ``is_truehd_file(file_path)``
    - ``convert_truehd_to_wav(file_path, output_path=None)``
    - ``get_truehd_channel_info(file_path)``
    - ``read_audio(file_path, expand=False)`` (TrueHD-aware audio reader)
    - ``get_supported_audio_formats()``

Module globals ``FFMPEG_PATH`` / ``FFPROBE_PATH`` / ``_FFMPEG_SETUP_DONE`` are
intentionally module-private so the lazy-init pattern stays scoped to one
file. ``ensure_ffmpeg_available`` is the only entry point that mutates them.
"""

import json
import os
import platform
import shutil
import subprocess
import tempfile
from pathlib import Path

import numpy as np
import soundfile as sf

# FFmpeg 최소 요구 버전 (major.minor 형태)
MIN_FFMPEG_VERSION = (4, 0)

def get_ffmpeg_version(ffmpeg_path):
    """FFmpeg 버전을 확인합니다."""
    try:
        result = subprocess.run([ffmpeg_path, '-version'],
                              capture_output=True, text=True, timeout=10,
                              encoding='utf-8', errors='replace')
        if result.returncode == 0:
            # 첫 번째 줄에서 버전 정보 추출
            first_line = result.stdout.split('\n')[0]
            # 'ffmpeg version X.Y.Z' 형태에서 버전 추출
            if 'version' in first_line:
                version_part = first_line.split('version')[1].strip().split()[0]
                
                # Git 빌드 버전 처리 (N-xxxxx-gxxxxxx 형태)
                if version_part.startswith('N-'):
                    # Git 빌드의 경우 빌드 번호를 확인하여 대략적인 버전 추정
                    try:
                        build_num = int(version_part.split('-')[1])
                        # 대략적인 매핑: N-55702는 2013년경 버전 (1.x 대)
                        if build_num < 60000:  # 대략 2014년 이전
                            return (1, 0)  # 구버전으로 분류
                        elif build_num < 80000:  # 대략 2016년 이전
                            return (3, 0)
                        elif build_num < 100000:  # 대략 2019년 이전
                            return (4, 0)
                        else:  # 최신 빌드
                            return (6, 0)
                    except (ValueError, IndexError):
                        return (1, 0)  # 파싱 실패시 구버전으로 간주
                
                # 숫자로 시작하는 일반 버전 추출
                version_nums = []
                for part in version_part.split('.'):
                    try:
                        # 숫자가 아닌 문자가 나오면 중단
                        clean_part = ''
                        for char in part:
                            if char.isdigit():
                                clean_part += char
                            else:
                                break
                        if clean_part:
                            version_nums.append(int(clean_part))
                        else:
                            break
                    except ValueError:
                        break
                
                if len(version_nums) >= 2:
                    return tuple(version_nums[:2])
        return None
    except (subprocess.TimeoutExpired, FileNotFoundError, Exception):
        return None

def find_ffmpeg_in_common_paths():
    """일반적인 경로에서 FFmpeg를 찾습니다."""
    system = platform.system().lower()
    
    common_paths = []
    
    if system == 'windows':
        common_paths = [
            r'C:\ProgramData\chocolatey\bin\ffmpeg.exe',
            r'C:\Program Files\ffmpeg\bin\ffmpeg.exe',
            r'C:\Program Files (x86)\ffmpeg\bin\ffmpeg.exe',
            r'C:\ffmpeg\bin\ffmpeg.exe',
            r'C:\tools\ffmpeg\bin\ffmpeg.exe',
            Path.home() / 'AppData' / 'Local' / 'Microsoft' / 'WinGet' / 'Packages' / 'Gyan.FFmpeg_*' / 'ffmpeg-*' / 'bin' / 'ffmpeg.exe'
        ]
    elif system == 'darwin':  # macOS
        common_paths = [
            '/usr/local/bin/ffmpeg',
            '/opt/homebrew/bin/ffmpeg',
            '/usr/bin/ffmpeg',
            Path.home() / '.local' / 'bin' / 'ffmpeg'
        ]
    else:  # Linux
        common_paths = [
            '/usr/bin/ffmpeg',
            '/usr/local/bin/ffmpeg',
            '/snap/bin/ffmpeg',
            '/opt/ffmpeg/bin/ffmpeg',
            Path.home() / '.local' / 'bin' / 'ffmpeg'
        ]
    
    for path in common_paths:
        if isinstance(path, Path):
            # WinGet 패턴 처리
            if '*' in str(path):
                parent = path.parent.parent
                if parent.exists():
                    for subdir in parent.glob('*'):
                        ffmpeg_dirs = list(subdir.glob('ffmpeg-*'))
                        for ffmpeg_dir in ffmpeg_dirs:
                            ffmpeg_path = ffmpeg_dir / 'bin' / 'ffmpeg.exe'
                            if ffmpeg_path.exists():
                                version = get_ffmpeg_version(str(ffmpeg_path))
                                if version and version >= MIN_FFMPEG_VERSION:
                                    return str(ffmpeg_path), str(ffmpeg_path).replace('ffmpeg.exe', 'ffprobe.exe')
            else:
                path = str(path)
        
        if os.path.isfile(path):
            version = get_ffmpeg_version(path)
            if version and version >= MIN_FFMPEG_VERSION:
                probe_path = path.replace('ffmpeg', 'ffprobe')
                if system == 'windows' and not probe_path.endswith('.exe'):
                    probe_path += '.exe'
                return path, probe_path
    
    return None, None

def install_ffmpeg():
    """운영체제에 맞는 방법으로 FFmpeg를 설치합니다."""
    system = platform.system().lower()
    
    print("FFmpeg가 감지되지 않았거나 버전이 너무 오래되었습니다. 자동 설치를 시도합니다...")
    
    try:
        if system == 'windows':
            # Windows: Chocolatey 또는 winget 사용
            
            # 먼저 chocolatey 시도
            try:
                subprocess.run(['choco', '--version'], capture_output=True, check=True, timeout=10,
                             encoding='utf-8', errors='replace')
                print("Chocolatey를 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['choco', 'install', 'ffmpeg', '-y'],
                                      capture_output=True, text=True, timeout=300,
                                      encoding='utf-8', errors='replace')
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass

            # winget 시도
            try:
                subprocess.run(['winget', '--version'], capture_output=True, check=True, timeout=10,
                             encoding='utf-8', errors='replace')
                print("WinGet을 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['winget', 'install', 'Gyan.FFmpeg'],
                                      capture_output=True, text=True, timeout=300,
                                      encoding='utf-8', errors='replace')
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
                
        elif system == 'darwin':  # macOS
            # Homebrew 사용
            try:
                subprocess.run(['brew', '--version'], capture_output=True, check=True, timeout=10,
                             encoding='utf-8', errors='replace')
                print("Homebrew를 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['brew', 'install', 'ffmpeg'],
                                      capture_output=True, text=True, timeout=600,
                                      encoding='utf-8', errors='replace')
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass

        else:  # Linux
            # apt (Ubuntu/Debian) 시도
            try:
                subprocess.run(['apt', '--version'], capture_output=True, check=True, timeout=10,
                             encoding='utf-8', errors='replace')
                print("APT를 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['sudo', 'apt', 'update'],
                                      capture_output=True, text=True, timeout=120,
                                      encoding='utf-8', errors='replace')
                if result.returncode == 0:
                    result = subprocess.run(['sudo', 'apt', 'install', '-y', 'ffmpeg'],
                                          capture_output=True, text=True, timeout=300,
                                          encoding='utf-8', errors='replace')
                    if result.returncode == 0:
                        return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass

            # yum (CentOS/RHEL) 시도
            try:
                subprocess.run(['yum', '--version'], capture_output=True, check=True, timeout=10,
                             encoding='utf-8', errors='replace')
                print("YUM을 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['sudo', 'yum', 'install', '-y', 'ffmpeg'],
                                      capture_output=True, text=True, timeout=300,
                                      encoding='utf-8', errors='replace')
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
        
        print("자동 설치에 실패했습니다. 수동으로 FFmpeg를 설치해주세요.")
        return None, None
        
    except Exception as e:
        print(f"FFmpeg 설치 중 오류 발생: {e}")
        return None, None

def setup_ffmpeg(auto_install=True):
    """FFmpeg를 설정하고 경로를 반환합니다.

    Args:
        auto_install: 시스템 PATH 및 일반 경로 검색에 실패한 경우 자동 설치
            를 시도할지 여부. TrueHD/MLP 처리 경로에서는 기본값 ``True``를
            유지해 사용자 편의 기능을 보존한다.
    """
    # 1. 시스템 PATH에서 ffmpeg 확인
    ffmpeg_path = shutil.which('ffmpeg')
    ffprobe_path = shutil.which('ffprobe')

    if ffmpeg_path and ffprobe_path:
        version = get_ffmpeg_version(ffmpeg_path)
        if version and version >= MIN_FFMPEG_VERSION:
            print(f"시스템 PATH에서 FFmpeg {version[0]}.{version[1]} 감지됨")
            return ffmpeg_path, ffprobe_path
        else:
            print(f"시스템 PATH의 FFmpeg 버전이 너무 오래됨: {version}")

    # 2. 일반적인 경로에서 검색
    ffmpeg_path, ffprobe_path = find_ffmpeg_in_common_paths()
    if ffmpeg_path and ffprobe_path:
        version = get_ffmpeg_version(ffmpeg_path)
        print(f"로컬 경로에서 FFmpeg {version[0]}.{version[1]} 감지됨: {ffmpeg_path}")
        return ffmpeg_path, ffprobe_path

    # 3. 자동 설치 시도 (TrueHD/MLP 사용 경로에서만 트리거)
    if auto_install:
        ffmpeg_path, ffprobe_path = install_ffmpeg()
        if ffmpeg_path and ffprobe_path:
            version = get_ffmpeg_version(ffmpeg_path)
            print(f"FFmpeg {version[0]}.{version[1]} 설치 완료: {ffmpeg_path}")
            return ffmpeg_path, ffprobe_path
        # install_ffmpeg() 내부에서 실패 메시지를 이미 출력함
        return None, None

    # 4. auto_install=False — 단순 검출만 수행, 사용자에게 알리지 않음
    return None, None


# FFmpeg 경로는 lazy하게 초기화한다. 모듈 import 시점에 setup_ffmpeg()를 실행
# 하면 일반 WAV 처리, ProcessPool 워커 import, unit test에서도 ffmpeg/ffprobe
# 탐색과 subprocess 호출이 발생해 시작 비용이 누적된다. 실제로 TrueHD/MLP
# 기능을 사용하는 경로에서만 ensure_ffmpeg_available()를 통해 1회 초기화된다.
FFMPEG_PATH = None
FFPROBE_PATH = None
_FFMPEG_SETUP_DONE = False


def ensure_ffmpeg_available(auto_install=True):
    """Lazy 초기화. 최초 호출 시 ``setup_ffmpeg()``를 실행하고 결과 캐시.

    Args:
        auto_install: 자동 설치 시도 여부. TrueHD/MLP 실제 사용 경로에서는
            ``True``를 유지하고, 단순 정보 조회(`get_supported_audio_formats`
            등)에서는 ``False``로 호출해 불필요한 install attempt를 막는다.

    Returns:
        FFmpeg/ffprobe 경로가 모두 설정되어 있으면 ``True``, 아니면 ``False``.
    """
    global FFMPEG_PATH, FFPROBE_PATH, _FFMPEG_SETUP_DONE

    if _FFMPEG_SETUP_DONE:
        return FFMPEG_PATH is not None and FFPROBE_PATH is not None

    _FFMPEG_SETUP_DONE = True
    FFMPEG_PATH, FFPROBE_PATH = setup_ffmpeg(auto_install=auto_install)

    if FFMPEG_PATH is None or FFPROBE_PATH is None:
        if auto_install:
            print("FFmpeg를 찾거나 설치할 수 없습니다. TrueHD/MLP 지원이 비활성화됩니다.")
        return False
    return True


def is_truehd_file(file_path):
    """Check if file is TrueHD/MLP format"""
    # TrueHD/MLP 처리 경로 — 자동 설치를 허용한다.
    if not ensure_ffmpeg_available(auto_install=True):
        return False

    try:
        result = subprocess.run(
            [FFPROBE_PATH, '-v', 'error', '-select_streams', 'a:0',
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
    if not ensure_ffmpeg_available(auto_install=True):
        raise RuntimeError("FFmpeg is not available for TrueHD conversion")

    if output_path is None:
        fd, output_path = tempfile.mkstemp(suffix='.wav')
        os.close(fd)

    # Get channel layout info first
    channel_info = get_truehd_channel_info(truehd_path)

    # Convert to WAV with proper channel mapping
    cmd = [
        FFMPEG_PATH, '-i', truehd_path,
        '-acodec', 'pcm_f32le',  # 32-bit float PCM
        '-ar', '48000',  # Sample rate
        output_path, '-y'
    ]

    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60,
                          encoding='utf-8', errors='replace')
    if result.returncode != 0:
        raise RuntimeError(f"FFmpeg conversion failed: {result.stderr}")

    return output_path, channel_info

def get_truehd_channel_info(file_path):
    """Get channel layout information from TrueHD file"""
    if not ensure_ffmpeg_available(auto_install=True):
        return None

    try:
        result = subprocess.run(
            [FFPROBE_PATH, '-v', 'error', '-select_streams', 'a:0',
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

        # Map channel layouts to speaker names
        from core.constants import CHANNEL_LAYOUT_MAP

        if channels in CHANNEL_LAYOUT_MAP:
            return CHANNEL_LAYOUT_MAP[channels]
        else:
            # Unknown layout, return None
            return None
    except Exception:
        return None

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
        # Convert TrueHD to temporary WAV
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
            # Clean up temp file
            if os.path.exists(temp_wav):
                os.remove(temp_wav)
    else:
        # Original WAV reading logic
        data, fs = sf.read(file_path)
        if len(data.shape) > 1:
            # Soundfile has tracks on columns, we want them on rows
            data = np.transpose(data)
        elif expand:
            data = np.expand_dims(data, axis=0)
        return fs, data, None

def check_ffmpeg_available(auto_install=False):
    """Check if FFmpeg is available.

    Args:
        auto_install: ``True``로 호출하면 lazy setup 시 자동 설치를 시도한다.
            TrueHD/MLP 처리 진입점(예: ``open_impulse_response_estimator``)에서
            이 옵션을 켜 사용자가 .mlp/.thd/.truehd 파일을 열 때 자동 설치
            UX가 유지되도록 한다.
    """
    if not ensure_ffmpeg_available(auto_install=auto_install):
        return False

    # 실제 파일 존재 및 실행 가능 여부 확인
    try:
        result = subprocess.run([FFMPEG_PATH, '-version'],
                              capture_output=True, text=True, timeout=10,
                              encoding='utf-8', errors='replace')
        return result.returncode == 0
    except Exception:
        return False

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
