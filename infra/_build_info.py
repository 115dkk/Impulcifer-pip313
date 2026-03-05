# -*- coding: utf-8 -*-
"""빌드 정보 마커.

이 파일은 빌드 환경에 따라 자동으로 덮어쓰기됩니다:
- Nuitka 빌드: build_scripts/build_nuitka.py가 BUILD_TYPE="standalone"으로 덮어씀
- PyPI 빌드: hatch_build.py 훅이 BUILD_TYPE="pip"으로 덮어씀
- 개발 환경: 이 기본값("dev") 사용

주의: 이 파일을 수동으로 수정하지 마세요. 빌드 시 자동 생성됩니다.
"""

BUILD_TYPE = "dev"       # "standalone" | "pip" | "dev"
VERSION = None           # None이면 pyproject.toml에서 동적 읽기
