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

# When invoked as `python build_scripts/build_nuitka.py`, sys.path[0] is the
# `build_scripts/` directory and `from build_scripts.nuitka_flags import …`
# fails. Insert the project root so the package-qualified import resolves
# regardless of how this script is launched.
_PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(_PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(_PROJECT_ROOT))

print("build_nuitka.py: Module level - imports done.", flush=True)


def get_project_version():
    print("build_nuitka.py: get_project_version() called", flush=True)
    """infra/get_version.py를 실행하여 프로젝트 버전 가져오기"""
    try:
        result = subprocess.run(
            [sys.executable, "infra/get_version.py"],
            capture_output=True, text=True, check=True, encoding='utf-8')
        version = result.stdout.strip()
        if not version:
            print("경고: get_version.py에서 버전을 가져왔지만 비어있습니다. 기본 버전을 사용합니다.", flush=True)
            return "0.0.0"  # 기본값 또는 오류 처리
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
        subprocess.run([sys.executable, "-m", "nuitka", "--version"],
                      capture_output=True, text=True, check=True,
                      encoding='utf-8', errors='replace')
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


def _generate_build_info(version: str):
    """Nuitka 빌드용 마커 파일 생성 — 런타임에서 버전/빌드 타입을 확실히 식별."""
    build_info_path = Path("infra/_build_info.py")
    content = f'''# -*- coding: utf-8 -*-
"""빌드 정보 마커 — build_nuitka.py에 의해 자동 생성됨."""
BUILD_TYPE = "standalone"
VERSION = "{version}"
'''
    build_info_path.write_text(content, encoding="utf-8")
    print(f"✓ 빌드 마커 생성: BUILD_TYPE=standalone, VERSION={version}", flush=True)


def build_impulcifer(project_version="0.0.0", output_base_dir="dist", target_platform=None):
    print(f"build_nuitka.py: build_impulcifer() called with version={project_version}", flush=True)
    """Nuitka로 Impulcifer GUI 빌드 (크로스 플랫폼 지원).

    Flag definitions live in :mod:`build_scripts.nuitka_flags` (Phase 4 of
    issue #87). This function only handles platform routing and the
    ``subprocess.run`` invocation.
    """
    from build_scripts.nuitka_flags import (
        PLATFORM_OUTPUT_DIRS,
        PLATFORM_OUTPUT_FILENAMES,
        build_nuitka_args,
    )

    # 플랫폼 감지
    if target_platform is None:
        target_platform = get_platform()

    print(f"타겟 플랫폼: {target_platform}", flush=True)

    if target_platform not in PLATFORM_OUTPUT_DIRS:
        print(f"✗ 지원하지 않는 플랫폼: {target_platform}", flush=True)
        return False

    # 플랫폼별 출력 디렉토리 — output_base_dir이 기본 'dist'와 다르면 그것을
    # 우선하고, 그 아래에 플랫폼별 서브디렉토리를 둔다.
    canonical_subdir = PLATFORM_OUTPUT_DIRS[target_platform]
    if output_base_dir != "dist":
        # canonical_subdir은 "dist/..." 로 시작하므로 prefix 치환
        relative_sub = canonical_subdir[len("dist/") :] if canonical_subdir.startswith("dist/") else canonical_subdir
        final_output_dir = Path(output_base_dir) / relative_sub
    else:
        final_output_dir = Path(canonical_subdir)
    output_filename = PLATFORM_OUTPUT_FILENAMES[target_platform]

    print(f"최종 빌드 결과물은 다음 폴더에 생성됩니다: {final_output_dir.resolve()}", flush=True)

    nuitka_args = build_nuitka_args(
        target_platform=target_platform,
        version=project_version,
        project_root=".",
        output_dir=str(final_output_dir),
        output_filename=output_filename,
    )
    nuitka_cmd = [sys.executable, "-m", "nuitka", *nuitka_args]

    print("\n빌드 명령어:", flush=True)
    print(" ".join(nuitka_cmd), flush=True)
    print(f"\n{target_platform} 플랫폼용 빌드를 시작합니다... (시간이 좀 걸릴 수 있습니다)", flush=True)

    try:
        result = subprocess.run(nuitka_cmd, check=True, text=True, capture_output=True,
                              encoding='utf-8', errors='replace')
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

    # clean_specific_build_folders()
    # --remove-output 옵션이 output-dir을 정리하므로, 추가 정리 불필요할 수 있음
    # 필요하다면 Nuitka가 생성하는 루트의 임시 파일/폴더만 정리

    current_version = get_project_version()
    print(f"빌드에 사용될 버전: {current_version}", flush=True)

    # 빌드 마커 파일 생성 (런타임 버전/빌드 타입 식별용)
    _generate_build_info(current_version)

    if not os.path.exists("gui_main.py"):
        print("\n엔트리 포인트 파일 생성 중...", flush=True)
        with open("gui_main.py", "w", encoding="utf-8") as f:
            f.write("""#!/usr/bin/env python
# -*- coding: utf-8 -*-
\"\"\"Impulcifer Modern GUI 엔트리 포인트\"\"\"
import sys

if __name__ == "__main__":
    # Velopack lifecycle hooks (https://docs.velopack.io/integrating/hooks)
    for arg in sys.argv[1:]:
        if arg.startswith('--veloapp-'):
            sys.exit(0)
    from gui.modern_gui import main_gui
    main_gui()
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
