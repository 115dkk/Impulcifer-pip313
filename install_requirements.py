"""
Impulcifer 빌드를 위한 필수 패키지 설치 스크립트
"""

import subprocess
import sys
import os

def install_package(package):
    """패키지 설치"""
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", package])
        print(f"✓ {package} 설치 완료")
        return True
    except subprocess.CalledProcessError:
        print(f"✗ {package} 설치 실패")
        return False

def main():
    print("=== Impulcifer 빌드 환경 설정 ===\n")
    
    # pip 업그레이드
    print("pip 업그레이드 중...")
    subprocess.check_call([sys.executable, "-m", "pip", "install", "--upgrade", "pip"])
    
    # 필수 패키지 목록
    packages = [
        "nuitka",  # Nuitka 컴파일러
        "ordered-set",  # Nuitka 의존성
        "zstandard",  # Nuitka 의존성
        "matplotlib>=3.8.0",
        "numpy>=1.26.0",
        "scipy>=1.12.0",
        "soundfile>=0.12.1",
        "sounddevice>=0.4.6",
        "nnresample>=0.2.4",
        "tabulate>=0.9.0",
        "autoeq-py313>=1.2.0",
        "seaborn",
        "bokeh>=3.0.0",
    ]
    
    print("\n필수 패키지 설치 중...\n")
    
    failed_packages = []
    for package in packages:
        if not install_package(package):
            failed_packages.append(package)
    
    print("\n" + "="*50)
    
    if failed_packages:
        print(f"\n설치 실패한 패키지: {', '.join(failed_packages)}")
        print("문제 해결 방법:")
        print("1. 관리자 권한으로 실행해보세요")
        print("2. 가상 환경을 사용해보세요")
        print("3. 수동으로 설치해보세요: pip install <package_name>")
    else:
        print("\n✓ 모든 패키지가 성공적으로 설치되었습니다!")
        print("\n이제 build_nuitka.py를 실행하여 빌드할 수 있습니다.")
    
    # Windows에서 Visual C++ 확인
    if sys.platform == "win32":
        print("\n주의: Windows에서 Nuitka를 사용하려면 Visual Studio 또는")
        print("Visual C++ Build Tools가 설치되어 있어야 합니다.")
        print("다운로드: https://visualstudio.microsoft.com/downloads/")

if __name__ == "__main__":
    main()