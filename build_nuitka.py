"""
Nuitka를 사용한 Impulcifer GUI 빌드 스크립트
Python 3.13 호환 버전
"""

import os
import sys
import subprocess
import shutil
from pathlib import Path

def get_project_version():
    """get_version.py를 실행하여 프로젝트 버전 가져오기"""
    try:
        # get_version.py가 프로젝트 루트에 있다고 가정
        result = subprocess.run([sys.executable, "get_version.py"], capture_output=True, text=True, check=True, encoding='utf-8')
        version = result.stdout.strip()
        if not version:
            print("경고: get_version.py에서 버전을 가져왔지만 비어있습니다. 기본 버전을 사용합니다.")
            return "0.0.0" # 기본값 또는 오류 처리
        print(f"✓ get_version.py에서 프로젝트 버전({version})을 성공적으로 가져왔습니다.")
        return version
    except FileNotFoundError:
        print("경고: get_version.py 파일을 찾을 수 없습니다. 기본 버전을 사용합니다.")
        return "0.0.0"
    except subprocess.CalledProcessError as e:
        print(f"경고: get_version.py 실행 중 오류 발생: {e}. 기본 버전을 사용합니다.")
        print(f"Stderr: {e.stderr}")
        return "0.0.0"
    except Exception as e:
        print(f"경고: 버전 정보 로드 중 예기치 않은 오류 발생: {e}. 기본 버전을 사용합니다.")
        return "0.0.0"

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
            print(f"이전 Nuitka 빌드 관련 폴더 삭제 중: {folder}")
            shutil.rmtree(folder)
    # 추가적으로, 이전 빌드의 실행 파일이 루트에 남아있을 수 있다면 정리
    if os.path.exists("ImpulciferGUI.exe") and not os.path.isdir("ImpulciferGUI.exe"):
        print("루트의 이전 빌드 실행 파일 삭제 중: ImpulciferGUI.exe")
        os.remove("ImpulciferGUI.exe")

def build_impulcifer(project_version="0.0.0", output_base_dir="dist"):
    """Nuitka로 Impulcifer GUI 빌드 (폴더 모드)"""
    
    final_output_dir = Path(output_base_dir) / "Impulcifer_Distribution" / "ImpulciferGUI"
    print(f"최종 빌드 결과물은 다음 폴더에 생성됩니다: {final_output_dir.resolve()}")

    nuitka_cmd_base = [sys.executable, "-m", "nuitka"]
    nuitka_cmd_args = [
        "--standalone",
        f"--output-dir={final_output_dir}",
        "--remove-output",
        "--windows-console-mode=disable",
        "--enable-plugin=tk-inter",
        "--enable-plugin=numpy",
        "--enable-plugin=matplotlib",
        "--include-module=sounddevice",
        "--include-module=soundfile",
        "--include-module=scipy",
        "--include-module=scipy.signal",
        "--include-module=scipy.optimize",
        "--include-module=scipy.interpolate",
        "--include-module=scipy.io",
        "--include-module=scipy.io.wavfile",
        "--include-module=scipy.fft",
        "--include-module=nnresample",
        "--include-module=tabulate",
        "--include-module=seaborn",
        "--include-module=bokeh",
        "--include-module=autoeq",
        "--include-module=recorder",
        "--include-module=impulcifer",
        "--include-module=hrir",
        "--include-module=impulse_response",
        "--include-module=impulse_response_estimator",
        "--include-module=room_correction",
        "--include-module=microphone_deviation_correction",
        "--include-module=utils",
        "--include-module=constants",
        "--include-data-dir=data=data",
        "--include-data-dir=font=font",
        "--include-data-dir=img=img",
    ]

    # LICENSE 파일을 License.txt로 포함
    license_file_path = "LICENSE"
    if os.path.exists(license_file_path):
        nuitka_cmd_args.append(f"--include-data-file={license_file_path}=License.txt")
        print(f"정보: {license_file_path} 파일을 License.txt로 빌드에 포함합니다.")
    else:
        print(f"경고: {license_file_path} 파일을 찾을 수 없습니다. Inno Setup에서 필요할 수 있습니다.")

    # 지정된 README.txt 파일을 빌드 결과물에 README.txt로 포함
    # 사용자가 제공한 경로: Impulcifer_Distribution/README.txt
    readme_source_path = "README.txt"
    if readme_source_path.exists():
        nuitka_cmd_args.append(f"--include-data-file={readme_source_path}=README.txt")
        print(f"정보: {readme_source_path} 파일을 README.txt로 빌드에 포함합니다.")
    else:
        print(f"경고: 소스 README 파일({readme_source_path})을 찾을 수 없습니다. Inno Setup에서 필요할 수 있습니다.")

    nuitka_cmd_args.extend([
        "--output-filename=ImpulciferGUI",
        "--company-name=115dkk",
        "--product-name=Impulcifer",
        f"--file-version={project_version}",
        f"--product-version={project_version}",
        "--file-description=HRIR 측정 및 헤드폰 바이노럴 헤드트래킹 HRTF 시스템",
        "--assume-yes-for-downloads",
        "--show-progress",
        "--show-memory",
        "gui_main.py"
    ])
    
    nuitka_cmd = nuitka_cmd_base + [cmd for cmd in nuitka_cmd_args if cmd]
    
    print("\n빌드 명령어:")
    print(" ".join(nuitka_cmd))
    print("\n빌드를 시작합니다... (시간이 좀 걸릴 수 있습니다)")
    
    try:
        result = subprocess.run(nuitka_cmd, check=True, text=True, capture_output=True, encoding='utf-8')
        print("Nuitka stdout:")
        print(result.stdout)
        if result.stderr:
            print("Nuitka stderr:")
            print(result.stderr)
        print("\n✓ 빌드가 성공적으로 완료되었습니다!")
        return True
    except subprocess.CalledProcessError as e:
        print(f"\n✗ 빌드 중 오류가 발생했습니다: {e}")
        print("Nuitka stdout:")
        print(e.stdout)
        print("Nuitka stderr:")
        print(e.stderr)
        return False

def main():
    """메인 빌드 프로세스"""
    print("=== Impulcifer Nuitka 빌드 스크립트 ===\n")
    
    if not check_nuitka():
       sys.exit(1)
    
    # clean_specific_build_folders() # --remove-output 옵션이 output-dir을 정리하므로, 추가 정리 불필요할 수 있음
                                     # 필요하다면 Nuitka가 생성하는 루트의 임시 파일/폴더만 정리
    
    current_version = get_project_version()
    print(f"빌드에 사용될 버전: {current_version}")

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
    
    if build_impulcifer(project_version=current_version, output_base_dir="dist"):
        print("\n빌드가 완료되었습니다!")
        final_path = Path('dist/Impulcifer_Distribution/ImpulciferGUI').resolve()
        print(f"빌드 결과는 {final_path} 폴더에서 찾을 수 있습니다.")
        if final_path.exists() and any(final_path.iterdir()):
            print("✓ 최종 출력 폴더가 존재하고 비어있지 않습니다.")
        else:
            print("✗ 최종 출력 폴더가 존재하지 않거나 비어있습니다. 빌드 과정 확인 필요.")
            sys.exit(1)
    else:
        print("\n빌드에 실패했습니다.")
        sys.exit(1)
        
if __name__ == "__main__":
    main()