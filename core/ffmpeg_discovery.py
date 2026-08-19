# -*- coding: utf-8 -*-
"""FFmpeg binary discovery, version check and (lazy) auto-install.

This module owns the FFmpeg/ffprobe lookup policy and the lazy-init module
globals
``FFMPEG_PATH`` / ``FFPROBE_PATH``. ``ensure_ffmpeg_available`` is the only
entry point that mutates them; the TrueHD decode layer (``core.audio_truehd``)
reads them through this module so it always sees the current values.
"""

import os
import platform
import shutil
import subprocess
import threading
from pathlib import Path



# FFmpeg 최소 요구 버전 (major.minor 형태)
MIN_FFMPEG_VERSION = (4, 0)


def get_ffmpeg_version(ffmpeg_path):
    """FFmpeg 버전을 확인합니다."""
    try:
        result = subprocess.run([ffmpeg_path, '-version'],
                              capture_output=True, text=True, timeout=10,
                              encoding='utf-8', errors='replace')
        if result.returncode == 0:
            # 'ffmpeg version X.Y.Z' 형태의 첫 줄에서 버전 추출
            first_line = result.stdout.split('\n')[0]
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
                
                version_nums = []
                for part in version_part.split('.'):
                    try:
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
    """FFmpeg를 설치합니다. Linux에서는 수동 설치 방법만 안내합니다."""
    system = platform.system().lower()

    if system not in ('windows', 'darwin'):
        print("Linux에서는 FFmpeg 자동 설치를 시도하지 않습니다.")
        print(
            "FFmpeg를 수동으로 설치해주세요. 예: "
            "sudo apt install ffmpeg 또는 sudo dnf install ffmpeg"
        )
        return None, None

    print("FFmpeg가 감지되지 않았거나 버전이 너무 오래되었습니다. 자동 설치를 시도합니다...")

    try:
        if system == 'windows':
            # Chocolatey → winget 순으로 시도
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
                
        elif system == 'darwin':
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

        print("자동 설치에 실패했습니다. 수동으로 FFmpeg를 설치해주세요.")
        return None, None
        
    except Exception as e:
        print(f"FFmpeg 설치 중 오류 발생: {e}")
        return None, None


def setup_ffmpeg(auto_install=True):
    """FFmpeg를 설정하고 경로를 반환합니다.

    Args:
        auto_install: 시스템 PATH 및 일반 경로 검색에 실패한 경우 Windows와
            macOS에서 자동 설치를 시도할지 여부. Linux에서는 ``True``여도
            자동 설치하지 않고 수동 설치 방법을 안내한다.
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
# 탐색과 subprocess 호출이 발생해 시작 비용이 누적된다. 조회용 탐색 실패와
# 실제 TrueHD/MLP 사용 시 자동 설치 시도는 별도 캐시로 관리한다.
FFMPEG_PATH = None
FFPROBE_PATH = None
_FFMPEG_DETECTION_DONE = False
_FFMPEG_AUTO_INSTALL_ATTEMPTED = False
_FFMPEG_UNAVAILABLE_REASON = None
# Backward-compatible state name for older tests/importers. It mirrors whether
# any detection/install path has already been attempted.
_FFMPEG_SETUP_DONE = False
# The lazy init is reachable from the audio read path on multiple threads
# (free-threaded builds especially); without the lock two threads could both
# pass the detection check and launch concurrent auto-installs.
_FFMPEG_LOCK = threading.Lock()


def get_ffmpeg_unavailable_reason():
    """마지막 FFmpeg 탐색 또는 설치 실패 이유를 반환합니다."""
    return _FFMPEG_UNAVAILABLE_REASON


def ensure_ffmpeg_available(auto_install=True):
    """Lazy 초기화하고 실패 이유를 출력한다.

    Windows와 macOS에서는 필요 시 FFmpeg 자동 설치를 시도하지만 Linux에서는
    수동 설치 방법만 안내한다.

    Args:
        auto_install: 자동 설치 시도 여부. TrueHD/MLP 실제 사용 경로에서는
            ``True``를 유지하고, 단순 정보 조회(`get_supported_audio_formats`
            등)에서는 ``False``로 호출해 불필요한 install attempt를 막는다.

    Returns:
        FFmpeg/ffprobe 경로가 모두 설정되어 있으면 ``True``, 아니면 ``False``.
    """
    global FFMPEG_PATH, FFPROBE_PATH, _FFMPEG_UNAVAILABLE_REASON
    global _FFMPEG_DETECTION_DONE, _FFMPEG_AUTO_INSTALL_ATTEMPTED, _FFMPEG_SETUP_DONE

    with _FFMPEG_LOCK:
        if FFMPEG_PATH is not None and FFPROBE_PATH is not None:
            _FFMPEG_UNAVAILABLE_REASON = None
            return True

        detection_was_done = _FFMPEG_DETECTION_DONE
        if not _FFMPEG_DETECTION_DONE:
            _FFMPEG_DETECTION_DONE = True
            _FFMPEG_SETUP_DONE = True
            FFMPEG_PATH, FFPROBE_PATH = setup_ffmpeg(auto_install=False)
            if FFMPEG_PATH is not None and FFPROBE_PATH is not None:
                _FFMPEG_UNAVAILABLE_REASON = None
                return True
            _FFMPEG_UNAVAILABLE_REASON = "FFmpeg를 찾을 수 없습니다."

        if auto_install and not _FFMPEG_AUTO_INSTALL_ATTEMPTED:
            _FFMPEG_AUTO_INSTALL_ATTEMPTED = True
            _FFMPEG_SETUP_DONE = True
            FFMPEG_PATH, FFPROBE_PATH = setup_ffmpeg(auto_install=True)
            if FFMPEG_PATH is not None and FFPROBE_PATH is not None:
                _FFMPEG_UNAVAILABLE_REASON = None
                return True
            _FFMPEG_UNAVAILABLE_REASON = (
                "FFmpeg를 찾거나 설치할 수 없습니다. "
                "TrueHD/MLP 지원이 비활성화됩니다."
            )
            print(_FFMPEG_UNAVAILABLE_REASON)
            return False

        if detection_was_done and _FFMPEG_UNAVAILABLE_REASON:
            print(_FFMPEG_UNAVAILABLE_REASON)
        return False


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
