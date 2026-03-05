"""
리소스 파일 경로 헬퍼
Nuitka 빌드와 개발 환경 모두에서 작동하도록 처리
"""

import os
import sys

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

def get_img_path(filename):
    """img 폴더 내 파일의 경로를 반환"""
    return get_resource_path(os.path.join("img", filename))

# 기본 경로들을 상수로 정의
DATA_DIR = get_resource_path("data")
FONT_DIR = get_resource_path("font")
IMG_DIR = get_resource_path("img")

# 자주 사용되는 파일들
DEFAULT_SWEEP_FILE = get_data_path("sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
DEFAULT_SWEEP_PICKLE = get_data_path("sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl")
DEFAULT_FONT_FILE = get_font_path("Pretendard-Regular.otf")

def ensure_dir_exists(directory):
    """디렉토리가 존재하지 않으면 생성"""
    if not os.path.exists(directory):
        os.makedirs(directory)

def file_exists(filepath):
    """파일 존재 여부 확인"""
    return os.path.exists(filepath) 