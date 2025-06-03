"""
Nuitka를 사용한 Impulcifer GUI 빌드 스크립트
Python 3.13 호환 버전
"""

import os
import sys
import subprocess
import shutil
from pathlib import Path

def check_nuitka():
    """Nuitka가 설치되어 있는지 확인"""
    try:
        subprocess.run([sys.executable, "-m", "nuitka", "--version"], capture_output=True, text=True, check=True)
        print("✓ Nuitka가 설치되어 있습니다.")
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("✗ Nuitka가 설치되어 있지 않습니다.")
        print("  다음 명령어로 설치하세요: pip install nuitka")
        return False

def clean_build_folders():
    """이전 빌드 폴더 정리"""
    folders_to_clean = ['dist', 'build', 'impulcifer_gui.build', 'impulcifer_gui.dist', 'impulcifer_gui.onefile-build']
    for folder in folders_to_clean:
        if os.path.exists(folder):
            print(f"이전 빌드 폴더 삭제 중: {folder}")
            shutil.rmtree(folder)

def build_impulcifer():
    """Nuitka로 Impulcifer GUI 빌드"""
    
    # Nuitka 명령어 구성
    nuitka_cmd_base = [sys.executable, "-m", "nuitka"]
    nuitka_cmd_args = [
        "--standalone",  # 독립 실행 가능한 폴더 생성
        "--onefile",     # 단일 실행 파일로 생성
        "--windows-console-mode=disable",  # Windows에서 콘솔 창 숨기기
        "--enable-plugin=tk-inter",  # Tkinter 플러그인 활성화
        "--enable-plugin=numpy",     # NumPy 플러그인 활성화
        "--enable-plugin=matplotlib",  # Matplotlib 플러그인 활성화
        "--include-module=sounddevice",  # sounddevice 모듈 포함
        "--include-module=soundfile",    # soundfile 모듈 포함
        "--include-module=scipy",        # scipy 모듈 포함
        "--include-module=scipy.signal", # scipy.signal 서브모듈 명시적 포함
        "--include-module=scipy.optimize",  # scipy.optimize 서브모듈 포함
        "--include-module=scipy.interpolate",  # scipy.interpolate 서브모듈 포함
        "--include-module=scipy.io",  # scipy.io 서브모듈 포함
        "--include-module=scipy.io.wavfile",  # wavfile 서브모듈 포함
        "--include-module=scipy.fft",  # scipy.fft 서브모듈 포함
        "--include-module=nnresample",   # nnresample 모듈 포함
        "--include-module=tabulate",     # tabulate 모듈 포함
        "--include-module=seaborn",      # seaborn 모듈 포함
        "--include-module=bokeh",        # bokeh 모듈 포함
        "--include-module=autoeq",       # autoeq 모듈 포함
        "--include-module=recorder",     # 로컬 모듈들
        "--include-module=impulcifer",
        "--include-module=hrir",
        "--include-module=impulse_response",
        "--include-module=impulse_response_estimator",
        "--include-module=room_correction",
        "--include-module=microphone_deviation_correction",
        "--include-module=utils",
        "--include-module=constants",
        "--include-data-dir=data=data",       # data 디렉토리 전체 포함
        "--include-data-dir=font=font",       # font 디렉토리 전체 포함 
        "--include-data-dir=img=img",        # img 디렉토리 전체 포함
        "--output-dir=dist",             # 출력 디렉토리
        "--company-name=115dkk",
        "--product-name=Impulcifer",
        "--file-version=1.4.1",
        "--product-version=1.4.1",
        "--file-description=HRIR 측정 및 헤드폰 바이노럴 헤드트래킹 HRTF 시스템",
        "--windows-icon-from-ico=icon.ico" if os.path.exists("icon.ico") else "",  # 아이콘이 있으면 사용
        "--assume-yes-for-downloads",  # 필요한 파일 자동 다운로드
        "--show-progress",  # 진행 상황 표시
        "--show-memory",   # 메모리 사용량 표시
        "--output-filename=ImpulciferGUI",  # 출력 파일명
        "gui_main.py"      # 엔트리 포인트 파일
    ]
    nuitka_cmd = nuitka_cmd_base + nuitka_cmd_args
    
    # 빈 문자열 제거 (아이콘이 없는 경우)
    nuitka_cmd = [cmd for cmd in nuitka_cmd if cmd]
    
    print("\n빌드 명령어:")
    print(" ".join(nuitka_cmd))
    print("\n빌드를 시작합니다... (시간이 좀 걸릴 수 있습니다)")
    
    try:
        result = subprocess.run(nuitka_cmd, check=True)
        print("\n✓ 빌드가 성공적으로 완료되었습니다!")
        return True
    except subprocess.CalledProcessError as e:
        print(f"\n✗ 빌드 중 오류가 발생했습니다: {e}")
        return False

def create_distribution():
    """배포용 폴더 생성"""
    dist_folder = "Impulcifer_Distribution"
    
    if os.path.exists(dist_folder):
        shutil.rmtree(dist_folder)
    
    os.makedirs(dist_folder)
    
    # 실행 파일 복사
    exe_files = ["ImpulciferGUI.exe", "impulcifer_gui.exe"]
    exe_copied = False
    
    for exe_file in exe_files:
        if os.path.exists(exe_file):
            shutil.copy(exe_file, dist_folder)
            exe_copied = True
            print(f"✓ {exe_file}를 배포 폴더로 복사했습니다.")
            break
    
    if not exe_copied:
        print("✗ 실행 파일을 찾을 수 없습니다.")
        return False
    
    # README 파일 생성
    readme_content = """# Impulcifer GUI

HRIR 측정 및 헤드폰 바이노럴 헤드트래킹 HRTF 시스템

## 실행 방법
ImpulciferGUI.exe를 더블클릭하여 실행하세요.

## 주의사항
- Windows Defender나 백신 프로그램에서 경고가 나올 수 있습니다.
  이는 Nuitka로 빌드된 프로그램의 일반적인 현상입니다.
- 첫 실행 시 시간이 좀 걸릴 수 있습니다.

## 문제 해결
프로그램이 실행되지 않는 경우:
1. Windows 10/11 64비트인지 확인하세요
2. Visual C++ Redistributable이 설치되어 있는지 확인하세요
3. 바이러스 백신 프로그램에서 예외 처리를 해주세요

원본 프로젝트: https://github.com/jaakkopasanen/impulcifer
Python 3.13 호환 버전: https://github.com/115dkk/Impulcifer-pip313
"""
    
    with open(os.path.join(dist_folder, "README.txt"), "w", encoding="utf-8") as f:
        f.write(readme_content)
    
    print(f"\n✓ 배포 폴더가 생성되었습니다: {dist_folder}")
    return True

def main():
    """메인 빌드 프로세스"""
    print("=== Impulcifer Nuitka 빌드 스크립트 ===\n")
    
    # Nuitka 확인
    if not check_nuitka():
        sys.exit(1) # Nuitka 미설치 시 오류 종료
    
    # 이전 빌드 정리
    clean_build_folders()
    
    # 엔트리 포인트 파일이 없으면 생성
    if not os.path.exists("gui_main.py"):
        print("\n엔트리 포인트 파일 생성 중...")
        with open("gui_main.py", "w", encoding="utf-8") as f:
            f.write("""#!/usr/bin/env python
# -*- coding: utf-8 -*-
\"\"\"Impulcifer GUI 엔트리 포인트\"\"\"

if __name__ == "__main__":
    import gui
    gui.main_gui()
""")
    
    # 빌드 실행
    if build_impulcifer():
        # 배포 폴더 생성
        if not create_distribution(): # 배포 폴더 생성 실패 시
            print("\n✗ 배포 폴더 생성에 실패했습니다.")
            sys.exit(1)
        print("\n빌드가 완료되었습니다!")
        print("Impulcifer_Distribution 폴더에서 실행 파일을 찾을 수 있습니다.")
    else:
        print("\n빌드에 실패했습니다.")
        sys.exit(1) # 빌드 실패 시 오류 종료

if __name__ == "__main__":
    main()