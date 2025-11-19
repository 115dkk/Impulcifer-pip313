"""
Nuitka를 사용한 Impulcifer GUI 빌드 스크립트
Python 3.13 호환 버전
크로스 플랫폼 지원: Windows, macOS, Linux
"""

print("build_nuitka.py: Module level - script parsing started.", flush=True)
import os  # noqa: E402
import sys  # noqa: E402
import subprocess  # noqa: E402
import shutil  # noqa: E402
import platform  # noqa: E402
from pathlib import Path  # noqa: E402
print("build_nuitka.py: Module level - imports done.", flush=True)

def get_project_version():
    print("build_nuitka.py: get_project_version() called", flush=True)
    """get_version.py를 실행하여 프로젝트 버전 가져오기"""
    try:
        # get_version.py가 프로젝트 루트에 있다고 가정
        result = subprocess.run([sys.executable, "get_version.py"], capture_output=True, text=True, check=True, encoding='utf-8')
        version = result.stdout.strip()
        if not version:
            print("경고: get_version.py에서 버전을 가져왔지만 비어있습니다. 기본 버전을 사용합니다.", flush=True)
            return "0.0.0" # 기본값 또는 오류 처리
        print(f"✓ get_version.py에서 프로젝트 버전({version})을 성공적으로 가져왔습니다.", flush=True)
        return version
    except FileNotFoundError:
        print("경고: get_version.py 파일을 찾을 수 없습니다. 기본 버전을 사용합니다.", flush=True)
        return "0.0.0"
    except subprocess.CalledProcessError as e:
        print(f"경고: get_version.py 실행 중 오류 발생: {e}. 기본 버전을 사용합니다.", flush=True)
        print(f"Stderr: {e.stderr}", flush=True)
        return "0.0.0"
    except Exception as e:
        print(f"경고: 버전 정보 로드 중 예기치 않은 오류 발생: {e}. 기본 버전을 사용합니다.", flush=True)
        return "0.0.0"

def get_platform():
    """현재 플랫폼 감지"""
    system = platform.system().lower()
    if system == "windows":
        return "windows"
    elif system == "darwin":
        return "macos"
    elif system == "linux":
        return "linux"
    else:
        return "unknown"

def check_nuitka():
    print("build_nuitka.py: check_nuitka() called", flush=True)
    """Nuitka가 설치되어 있는지 확인"""
    try:
        subprocess.run([sys.executable, "-m", "nuitka", "--version"], capture_output=True, text=True, check=True)
        print("✓ Nuitka가 설치되어 있습니다.", flush=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        print("✗ Nuitka가 설치되어 있지 않습니다.", flush=True)
        print("  다음 명령어로 설치하세요: pip install nuitka", flush=True)
        return False

def clean_specific_build_folders():
    """Nuitka 관련 이전 빌드 폴더 정리 (dist 제외)"""
    # Nuitka가 생성하는 기본 패턴의 폴더들 및 파일
    # --output-dir을 사용하면 대부분 해당 디렉토리 내에서 관리됨
    # --remove-output 옵션을 사용하면 Nuitka가 빌드 시작 시 output-dir을 정리해줌
    folders_to_clean = ['build', 'impulcifer_gui.build', 'impulcifer_gui.dist', 'ImpulciferGUI.onefile-build']
    # .spec 파일 등도 생성될 수 있으나, 여기서는 주요 폴더만 대상으로 함
    # 실행 파일 자체 (ImpulciferGUI.exe)는 --output-dir 내에 생성되므로 여기서 직접 삭제 안함

    for folder in folders_to_clean:
        if os.path.exists(folder):
            print(f"이전 Nuitka 빌드 관련 폴더 삭제 중: {folder}", flush=True)
            shutil.rmtree(folder)
    # 추가적으로, 이전 빌드의 실행 파일이 루트에 남아있을 수 있다면 정리
    if os.path.exists("ImpulciferGUI.exe") and not os.path.isdir("ImpulciferGUI.exe"):
        print("루트의 이전 빌드 실행 파일 삭제 중: ImpulciferGUI.exe", flush=True)
        os.remove("ImpulciferGUI.exe")

def build_impulcifer(project_version="0.0.0", output_base_dir="dist", target_platform=None):
    print(f"build_nuitka.py: build_impulcifer() called with version={project_version}", flush=True)
    """Nuitka로 Impulcifer GUI 빌드 (크로스 플랫폼 지원)"""

    # 플랫폼 감지
    if target_platform is None:
        target_platform = get_platform()

    print(f"타겟 플랫폼: {target_platform}", flush=True)

    # 플랫폼별 출력 디렉토리 설정
    if target_platform == "windows":
        final_output_dir = Path(output_base_dir) / "Impulcifer_Distribution" / "ImpulciferGUI"
        output_filename = "ImpulciferGUI"
    elif target_platform == "macos":
        final_output_dir = Path(output_base_dir) / "macos"
        output_filename = "Impulcifer"
    elif target_platform == "linux":
        final_output_dir = Path(output_base_dir) / "linux"
        output_filename = "Impulcifer"
    else:
        print(f"✗ 지원하지 않는 플랫폼: {target_platform}", flush=True)
        return False

    print(f"최종 빌드 결과물은 다음 폴더에 생성됩니다: {final_output_dir.resolve()}", flush=True)

    nuitka_cmd_base = [sys.executable, "-m", "nuitka"]

    # 공통 옵션
    nuitka_cmd_args = [
        "--standalone",
        f"--output-dir={final_output_dir}",
        "--remove-output",
        "--jobs=4",
        "--lto=no",
        "--enable-plugin=tk-inter",
        "--enable-plugin=numpy",
        "--enable-plugin=matplotlib",
        "--include-package=customtkinter",
    ]

    # 플랫폼별 옵션
    if target_platform == "windows":
        nuitka_cmd_args.append("--windows-console-mode=disable")
    elif target_platform == "macos":
        nuitka_cmd_args.extend([
            "--macos-create-app-bundle",
            "--macos-app-name=Impulcifer",
        ])
        # macOS 아이콘이 있다면 추가
        if os.path.exists("img/icon.icns"):
            nuitka_cmd_args.append("--macos-app-icon=img/icon.icns")
    elif target_platform == "linux":
        nuitka_cmd_args.append("--onefile")
        # Linux 아이콘이 있다면 추가
        if os.path.exists("img/icon.png"):
            nuitka_cmd_args.append("--linux-icon=img/icon.png")

    # 공통 모듈 포함
    common_modules = [
        "sounddevice", "soundfile", "scipy", "scipy.signal", "scipy.optimize",
        "scipy.interpolate", "scipy.io", "scipy.fft", "nnresample", "tabulate",
        "seaborn", "bokeh", "autoeq", "modern_gui", "gui", "localization",
        "logger", "channel_generation", "update_checker", "updater", "recorder",
        "impulcifer", "hrir", "impulse_response", "impulse_response_estimator",
        "room_correction", "microphone_deviation_correction", "utils", "constants",
    ]

    for module in common_modules:
        nuitka_cmd_args.append(f"--include-module={module}")

    # 데이터 디렉토리 포함
    data_dirs = ["data", "font", "img", "locales"]
    for data_dir in data_dirs:
        if os.path.exists(data_dir):
            nuitka_cmd_args.append(f"--include-data-dir={data_dir}={data_dir}")

    # LICENSE 파일 포함
    if os.path.exists("LICENSE"):
        nuitka_cmd_args.append("--include-data-file=LICENSE=License.txt")
        print("정보: LICENSE 파일을 License.txt로 빌드에 포함합니다.", flush=True)

    # README.txt 파일 포함
    if os.path.exists("README.txt"):
        nuitka_cmd_args.append("--include-data-file=README.txt=README.txt")
        print("정보: README.txt 파일을 빌드에 포함합니다.", flush=True)

    # 메타데이터
    nuitka_cmd_args.extend([
        f"--output-filename={output_filename}",
        "--company-name=115dkk",
        "--product-name=Impulcifer",
        f"--file-version={project_version}",
        f"--product-version={project_version}",
        "--file-description=HRIR 측정 및 헤드폰 바이노럴 헤드트래킹 HRTF 시스템",
        "--prefer-source-code",
        "--assume-yes-for-downloads",
        "--show-progress",
        "--show-memory",
        "gui_main.py"
    ])

    nuitka_cmd = nuitka_cmd_base + [cmd for cmd in nuitka_cmd_args if cmd]

    print("\n빌드 명령어:", flush=True)
    print(" ".join(nuitka_cmd), flush=True)
    print(f"\n{target_platform} 플랫폼용 빌드를 시작합니다... (시간이 좀 걸릴 수 있습니다)", flush=True)

    try:
        result = subprocess.run(nuitka_cmd, check=True, text=True, capture_output=True, encoding='utf-8')
        print("Nuitka stdout:", flush=True)
        print(result.stdout, flush=True)
        if result.stderr:
            print("Nuitka stderr:", flush=True)
            print(result.stderr, flush=True)
        print(f"\n✓ {target_platform} 플랫폼용 빌드가 성공적으로 완료되었습니다!", flush=True)
        return True
    except subprocess.CalledProcessError as e:
        print(f"\n✗ 빌드 중 오류가 발생했습니다: {e}", flush=True)
        print("Nuitka stdout:", flush=True)
        print(e.stdout, flush=True)
        print("Nuitka stderr:", flush=True)
        print(e.stderr, flush=True)
        return False

def main():
    print("build_nuitka.py: main() function - entry point.", flush=True)
    """메인 빌드 프로세스"""
    print("=== Impulcifer Nuitka 크로스 플랫폼 빌드 스크립트 ===\n", flush=True)

    # 플랫폼 감지 및 표시
    current_platform = get_platform()
    print(f"감지된 플랫폼: {current_platform}", flush=True)

    if not check_nuitka():
       sys.exit(1)

    # clean_specific_build_folders() # --remove-output 옵션이 output-dir을 정리하므로, 추가 정리 불필요할 수 있음
                                     # 필요하다면 Nuitka가 생성하는 루트의 임시 파일/폴더만 정리

    current_version = get_project_version()
    print(f"빌드에 사용될 버전: {current_version}", flush=True)

    if not os.path.exists("gui_main.py"):
        print("\n엔트리 포인트 파일 생성 중...", flush=True)
        with open("gui_main.py", "w", encoding="utf-8") as f:
            f.write("""#!/usr/bin/env python
# -*- coding: utf-8 -*-
\"\"\"Impulcifer Modern GUI 엔트리 포인트\"\"\"

if __name__ == "__main__":
    import modern_gui
    modern_gui.main_gui()
""")

    if build_impulcifer(project_version=current_version, output_base_dir="dist"):
        print("\n빌드가 완료되었습니다!", flush=True)

        # 플랫폼별 출력 경로 확인
        if current_platform == "windows":
            final_path = Path('dist/Impulcifer_Distribution/ImpulciferGUI').resolve()
        elif current_platform == "macos":
            final_path = Path('dist/macos').resolve()
        elif current_platform == "linux":
            final_path = Path('dist/linux').resolve()
        else:
            final_path = Path('dist').resolve()

        print(f"빌드 결과는 {final_path} 폴더에서 찾을 수 있습니다.", flush=True)
        if final_path.exists() and any(final_path.iterdir()):
            print("✓ 최종 출력 폴더가 존재하고 비어있지 않습니다.", flush=True)
        else:
            print("✗ 최종 출력 폴더가 존재하지 않거나 비어있습니다. 빌드 과정 확인 필요.", flush=True)
            sys.exit(1)
    else:
        print("\n빌드에 실패했습니다.", flush=True)
        sys.exit(1)
        
if __name__ == "__main__":
    print("build_nuitka.py: Script is run directly (before main() call).", flush=True)
    main()