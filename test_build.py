"""
Impulcifer 빌드 테스트 스크립트
빌드된 실행 파일이 정상적으로 작동하는지 확인
"""

import os
import subprocess
import time

def test_executable():
    """실행 파일 테스트"""
    print("=== Impulcifer 빌드 테스트 ===\n")
    
    # 가능한 실행 파일 위치들
    possible_paths = [
        "ImpulciferGUI.exe",
        "Impulcifer_Distribution/ImpulciferGUI.exe",
        "Impulcifer_Distribution/ImpulciferGUI/ImpulciferGUI.exe",
        "dist/gui_main.dist/ImpulciferGUI.exe",
        "gui_main.exe",
    ]
    
    exe_path = None
    for path in possible_paths:
        if os.path.exists(path):
            exe_path = path
            break
    
    if not exe_path:
        print("✗ 실행 파일을 찾을 수 없습니다.")
        print("  먼저 build_nuitka.py를 실행하여 빌드하세요.")
        return False
    
    print(f"✓ 실행 파일 발견: {exe_path}")
    
    # 파일 정보 출력
    file_size = os.path.getsize(exe_path) / (1024 * 1024)  # MB
    print(f"  파일 크기: {file_size:.1f} MB")
    
    # 실행 파일 테스트
    print("\n실행 파일을 테스트합니다...")
    print("(GUI가 표시되면 정상 작동하는 것입니다)")
    
    try:
        # 프로세스 시작
        process = subprocess.Popen([exe_path])
        
        print("\n테스트 중... (10초 대기)")
        print("GUI 창이 나타났는지 확인하세요.")
        
        # 10초 대기
        for i in range(10, 0, -1):
            print(f"\r{i}초 남음...", end="")
            time.sleep(1)
        
        print("\n\n테스트를 종료하시겠습니까? (Y/N): ", end="")
        choice = input().strip().upper()
        
        if choice == 'Y':
            process.terminate()
            print("프로세스를 종료했습니다.")
        else:
            print("프로그램이 계속 실행됩니다.")
            print("수동으로 종료하세요.")
        
        return True
        
    except Exception as e:
        print(f"\n✗ 실행 중 오류 발생: {e}")
        return False

def check_dependencies():
    """의존성 파일 확인"""
    print("\n의존성 파일 확인 중...")
    
    # data 폴더 확인
    data_files = [
        "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav",
        "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl",
    ]
    
    missing_files = []
    for file in data_files:
        if not os.path.exists(file):
            missing_files.append(file)
    
    if missing_files:
        print(f"✗ 누락된 파일: {', '.join(missing_files)}")
    else:
        print("✓ 모든 데이터 파일이 존재합니다.")
    
    # font 폴더 확인
    if os.path.exists("font/Pretendard-Regular.otf"):
        print("✓ 폰트 파일이 존재합니다.")
    else:
        print("✗ 폰트 파일이 없습니다.")

def main():
    """메인 테스트 함수"""
    # 의존성 확인
    check_dependencies()
    
    # 실행 파일 테스트
    print()
    if test_executable():
        print("\n✓ 테스트 완료!")
        print("\n다음 단계:")
        print("1. 다른 PC에서도 테스트해보세요")
        print("2. 모든 기능이 정상 작동하는지 확인하세요")
        print("3. 배포 준비가 완료되었습니다!")
    else:
        print("\n✗ 테스트 실패!")
        print("\n문제 해결:")
        print("1. 빌드가 성공적으로 완료되었는지 확인")
        print("2. Visual C++ Redistributable 설치 확인")
        print("3. 바이러스 백신 예외 설정")

if __name__ == "__main__":
    main() 