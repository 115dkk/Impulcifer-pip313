# -*- coding: utf-8 -*-
"""Hatchling 커스텀 빌드 훅 — PyPI 빌드 시 infra/_build_info.py 생성.

pip install . 또는 pip install impulcifer-py313 시 자동으로 실행되어
빌드 마커 파일에 BUILD_TYPE="pip"과 정확한 버전을 기록합니다.
"""
from hatchling.builders.hooks.plugin.interface import BuildHookInterface
from pathlib import Path


class CustomBuildHook(BuildHookInterface):
    PLUGIN_NAME = "impulcifer-build-info"

    def initialize(self, version, build_data):
        # ``version`` 파라미터는 hatchling의 build target 종류("standard"/"editable")이지
        # 패키지 버전이 아니다. 패키지 버전은 self.metadata.version 으로 읽는다.
        # (이전 회귀: VERSION = "standard"가 박혀 정보 탭 표시와 UpdateChecker가 깨졌다.)
        pkg_version = self.metadata.version
        build_info_path = Path(self.root) / "infra" / "_build_info.py"
        content = f'''# -*- coding: utf-8 -*-
"""빌드 정보 마커 — hatch_build.py에 의해 자동 생성됨."""
BUILD_TYPE = "pip"
VERSION = "{pkg_version}"
'''
        build_info_path.write_text(content, encoding="utf-8")
