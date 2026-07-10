"""
리소스 파일 경로 헬퍼
Nuitka 빌드와 개발 환경 모두에서 작동하도록 처리
"""

import os
import sys
from pathlib import Path

def get_resource_path(relative_path):
    """리소스 파일의 절대 경로를 반환

    개발 환경과 Nuitka 빌드 환경 모두에서 작동
    """
    # 빌드 마커 우선
    try:
        from infra._build_info import BUILD_TYPE
        if BUILD_TYPE == "standalone":
            base_path = os.path.dirname(sys.executable)
            return os.path.join(base_path, relative_path)
    except ImportError:
        pass

    # Nuitka 컴파일 폴백 (__compiled__는 모듈 전역 변수)
    if "__compiled__" in globals():
        base_path = os.path.dirname(sys.executable)
        return os.path.join(base_path, relative_path)

    # 개발 환경: infra/ 의 상위 디렉토리 = 프로젝트 루트
    base_path = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    return os.path.join(base_path, relative_path)

def get_data_path(filename):
    """data 폴더 내 파일의 경로를 반환"""
    return get_resource_path(os.path.join("data", filename))

def get_font_path(filename):
    """font 폴더 내 파일의 경로를 반환"""
    return get_resource_path(os.path.join("font", filename))

def resolve_bundled_font_dir(extra_candidates=()):
    """Return the bundled ``font/`` directory across all runtime modes.

    Tries :func:`get_resource_path` ``("font")`` first (covers Nuitka
    standalone, pip-install and dev layouts), then each caller-supplied
    fallback path in order. Returns ``None`` when no candidate is a directory.

    Single source of truth for bundled-font directory resolution shared by
    ``core.utils`` (matplotlib backend) and ``gui.utils`` (Tk backend); each
    passes its own ``extra_candidates`` so their fallback chains are preserved.
    """
    candidate = Path(get_resource_path("font"))
    if candidate.is_dir():
        return candidate
    for extra in extra_candidates:
        extra = Path(extra)
        if extra.is_dir():
            return extra
    return None


def scan_bundled_fonts(extra_candidates=()):
    """List bundled ``.otf``/``.ttf``/``.ttc`` fonts in case-insensitive name order.

    The deterministic casefold sort keeps matplotlib ``findfont`` scoring stable
    across dev / standalone trees when multiple bundled fonts declare the same
    family. Shared by ``core.utils`` and ``gui.utils`` (see
    :func:`resolve_bundled_font_dir`).
    """
    font_dir = resolve_bundled_font_dir(extra_candidates)
    if font_dir is None:
        return []
    suffixes = {".otf", ".ttf", ".ttc"}
    return sorted(
        (path for path in font_dir.iterdir() if path.suffix.lower() in suffixes),
        key=lambda path: path.name.casefold(),
    )


def iter_font_paths():
    """Return bundled font files in deterministic preference order."""
    return scan_bundled_fonts()

def find_pretendard_font_path():
    """Find the bundled Pretendard font, preferring the shipped variable font."""
    fonts = iter_font_paths()
    for path in fonts:
        stem = path.stem.lower()
        if "pretendard" in stem and "variable" in stem:
            return str(path)
    for path in fonts:
        if "pretendard-regular" in path.stem.lower():
            return str(path)
    for path in fonts:
        if "pretendard" in path.stem.lower():
            return str(path)
    return None

def get_img_path(filename):
    """img 폴더 내 파일의 경로를 반환"""
    return get_resource_path(os.path.join("img", filename))

DATA_DIR = get_resource_path("data")
FONT_DIR = get_resource_path("font")
IMG_DIR = get_resource_path("img")

DEFAULT_SWEEP_FILE = get_data_path("sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
DEFAULT_SWEEP_PICKLE = get_data_path("sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl")
DEFAULT_FONT_FILE = find_pretendard_font_path() or get_font_path("PretendardVariable.ttf")

def ensure_dir_exists(directory):
    """디렉토리가 존재하지 않으면 생성"""
    if not os.path.exists(directory):
        os.makedirs(directory)

def file_exists(filepath):
    """파일 존재 여부 확인"""
    return os.path.exists(filepath) 
