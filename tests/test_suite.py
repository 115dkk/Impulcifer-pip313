#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
Impulcifer 종합 유닛 테스트 스위트

pytest 기반의 포괄적인 테스트로, CI/CD 파이프라인에서 실행됩니다.
"""

import pytest
import numpy as np
import sys
from pathlib import Path

# 테스트 대상 모듈 임포트
try:
    from core.microphone_deviation_correction import MicrophoneDeviationCorrector
    from core.impulse_response import ImpulseResponse
except ImportError:
    # 패키지가 설치되지 않은 경우 프로젝트 루트를 경로에 추가
    sys.path.insert(0, str(Path(__file__).parent.parent))
    from core.microphone_deviation_correction import MicrophoneDeviationCorrector
    from core.impulse_response import ImpulseResponse


class TestMicrophoneDeviationCorrector:
    """마이크 편차 보정 v3.0 테스트"""

    @pytest.fixture
    def corrector(self):
        """기본 corrector 인스턴스"""
        return MicrophoneDeviationCorrector(
            sample_rate=48000,
            correction_strength=0.7
        )

    def test_corrector_initialization(self, corrector):
        """초기화 테스트"""
        assert corrector.fs == 48000
        assert corrector.correction_strength == 0.7
        assert len(corrector.octave_bands) > 0, "옥타브 밴드가 비어있음"
        assert hasattr(corrector, 'expected_ild_sign'), "기대 ILD 부호 딕셔너리가 없음"

    def test_expected_ild_sign(self, corrector):
        """스피커별 기대 ILD 부호 테스트"""
        # 왼쪽 스피커는 양수
        assert corrector.expected_ild_sign.get('FL', 0) > 0, "FL은 양수여야 함"
        assert corrector.expected_ild_sign.get('SL', 0) > 0, "SL은 양수여야 함"
        # 오른쪽 스피커는 음수
        assert corrector.expected_ild_sign.get('FR', 0) < 0, "FR은 음수여야 함"
        assert corrector.expected_ild_sign.get('SR', 0) < 0, "SR은 음수여야 함"
        # 중앙 스피커는 0
        assert corrector.expected_ild_sign.get('FC', 0) == 0, "FC는 0이어야 함"

    def test_gate_length_calculation(self, corrector):
        """게이트 길이 계산 테스트"""
        assert len(corrector.gate_lengths) == len(corrector.octave_bands)

        # 고주파일수록 짧은 게이트
        freqs = sorted(corrector.gate_lengths.keys())
        for i in range(len(freqs) - 1):
            # 주파수가 높아질수록 게이트 길이가 짧아지거나 같아야 함
            assert corrector.gate_lengths[freqs[i]] >= corrector.gate_lengths[freqs[i+1]], \
                   "게이트 길이가 주파수에 따라 올바르게 감소하지 않음"

    def test_collect_speaker_deviation(self, corrector):
        """스피커 편차 수집 테스트"""
        # 테스트 IR 생성
        length = 4800
        left_ir = np.zeros(length)
        right_ir = np.zeros(length)
        left_ir[1000] = 1.0
        right_ir[1000] = 0.8

        deviations = corrector.collect_speaker_deviation(
            'FL', left_ir, right_ir,
            left_peak_index=1000, right_peak_index=1000
        )

        assert isinstance(deviations, dict), "편차는 딕셔너리여야 함"
        assert len(deviations) > 0, "편차가 비어있음"
        assert 'FL' in corrector.all_speaker_deviations, "스피커 데이터가 저장되지 않음"

    def test_separate_microphone_error(self, corrector):
        """마이크 오차 분리 테스트"""
        # 여러 스피커 데이터 수집
        length = 4800
        for speaker, expected_sign in [('FL', 1.0), ('FR', -1.0), ('FC', 0.0)]:
            left_ir = np.zeros(length)
            right_ir = np.zeros(length)
            left_ir[1000] = 1.0
            # 기대 방향과 반대로 편차 추가 (마이크 오차 시뮬레이션)
            right_ir[1000] = 1.2 if expected_sign >= 0 else 0.8

            corrector.collect_speaker_deviation(
                speaker, left_ir, right_ir,
                left_peak_index=1000, right_peak_index=1000
            )

        mic_error = corrector.separate_microphone_error()
        assert isinstance(mic_error, dict), "마이크 오차는 딕셔너리여야 함"

    def test_deviation_correction_basic(self, corrector):
        """기본 편차 보정 테스트"""
        # 간단한 임펄스 응답 생성
        length = 4800
        left_ir = np.zeros(length)
        right_ir = np.zeros(length)

        # 피크 추가
        left_ir[1000] = 1.0
        right_ir[1000] = 0.8  # 우측이 20% 작음

        # 보정 수행
        corrected_left, corrected_right, analysis = corrector.correct_microphone_deviation(
            left_ir, right_ir,
            left_peak_index=1000,
            right_peak_index=1000
        )

        # 결과 검증
        assert corrected_left.shape == left_ir.shape
        assert corrected_right.shape == right_ir.shape
        assert 'deviation_results' in analysis
        assert 'v3_cross_validation' in analysis


class TestImpulseResponse:
    """ImpulseResponse 클래스 테스트"""

    @pytest.fixture
    def sample_ir(self):
        """샘플 임펄스 응답"""
        data = np.zeros(4800)
        data[1000] = 1.0
        data[1100] = 0.5
        data[1200] = 0.25
        return ImpulseResponse(data, fs=48000)

    def test_impulse_response_creation(self, sample_ir):
        """임펄스 응답 생성 테스트"""
        assert len(sample_ir.data) == 4800
        assert sample_ir.fs == 48000

    def test_peak_detection(self, sample_ir):
        """피크 검출 테스트"""
        peak_idx = sample_ir.peak_index()
        assert peak_idx == 1000, f"피크 인덱스가 잘못됨: {peak_idx}"

class TestModuleImports:
    """모듈 임포트 테스트"""

    def test_core_modules_importable(self):
        """핵심 모듈들이 임포트 가능한지 테스트"""
        modules_to_test = [
            'impulcifer',
            'impulse_response',
            'hrir',
            'microphone_deviation_correction',
        ]

        for module_name in modules_to_test:
            try:
                __import__(module_name)
            except ImportError as e:
                pytest.fail(f"모듈 {module_name} 임포트 실패: {e}")

    def test_recorder_module_importable(self):
        """recorder 모듈 임포트 테스트 (오디오 하드웨어 필요)"""
        try:
            import recorder  # noqa: F401
        except (ImportError, OSError) as e:
            # CI 환경에서는 PortAudio가 없을 수 있음
            pytest.skip(f"recorder 모듈 임포트 불가 (정상): {e}")

    def test_gui_modules_importable(self):
        """GUI 모듈 임포트 테스트 (선택적)"""
        try:
            import modern_gui  # noqa: F401
            import gui  # noqa: F401
        except (ImportError, OSError) as e:
            # CI 환경에서는 PortAudio가 없을 수 있음
            pytest.skip(f"GUI 모듈 임포트 불가 (정상): {e}")


class TestDataFiles:
    """데이터 파일 존재 확인 테스트"""

    def test_data_directory_exists(self):
        """data 디렉토리 존재 확인"""
        data_dir = Path(__file__).parent / 'data'
        assert data_dir.exists(), "data 디렉토리가 없음"

    def test_essential_data_files(self):
        """필수 데이터 파일 존재 확인"""
        essential_files = [
            'data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
            'data/harman-in-room-headphone-target.csv',
        ]

        for file_path in essential_files:
            full_path = Path(__file__).parent / file_path
            assert full_path.exists(), f"필수 파일 {file_path}가 없음"


class TestConfigurationFiles:
    """설정 파일 검증 테스트"""

    def test_pyproject_toml_exists(self):
        """pyproject.toml 존재 확인"""
        pyproject = Path(__file__).parent / 'pyproject.toml'
        assert pyproject.exists(), "pyproject.toml이 없음"

    def test_pyproject_toml_valid(self):
        """pyproject.toml 유효성 검사"""
        try:
            import tomllib
        except ImportError:
            import tomli as tomllib

        pyproject = Path(__file__).parent / 'pyproject.toml'
        try:
            with open(pyproject, 'rb') as f:
                config = tomllib.load(f)

            assert 'project' in config
            assert 'name' in config['project']
            assert config['project']['name'] == 'impulcifer-py313'
        except Exception as e:
            pytest.fail(f"pyproject.toml 파싱 실패: {e}")


class TestVersionConsistency:
    """버전 일관성 테스트"""

    def test_version_in_pyproject(self):
        """pyproject.toml의 버전 확인"""
        pyproject = Path(__file__).parent / 'pyproject.toml'

        with open(pyproject, 'r', encoding='utf-8') as f:
            content = f.read()
            assert 'version = ' in content, "버전 정보가 없음"

            # 버전 형식 확인 (semantic versioning)
            import re
            version_match = re.search(r'version\s*=\s*"(\d+\.\d+\.\d+)"', content)
            assert version_match, "올바른 버전 형식이 아님"

            version = version_match.group(1)
            parts = version.split('.')
            assert len(parts) == 3, "버전은 X.Y.Z 형식이어야 함"
            assert all(p.isdigit() for p in parts), "버전은 숫자로만 구성되어야 함"


@pytest.mark.slow
class TestIntegration:
    """통합 테스트 (느림)"""

    def test_end_to_end_microphone_correction(self):
        """마이크 보정 전체 플로우 테스트"""
        # 이 테스트는 시간이 오래 걸리므로 @pytest.mark.slow로 표시
        corrector = MicrophoneDeviationCorrector(
            sample_rate=48000,
            correction_strength=0.5
        )

        # 실제와 유사한 IR 생성
        length = 48000  # 1초
        left_ir = np.random.randn(length) * 0.01
        right_ir = np.random.randn(length) * 0.01

        # 주요 임펄스 추가
        left_ir[10000] = 1.0
        right_ir[10000] = 0.9

        # 보정 실행
        corrected_left, corrected_right, analysis = corrector.correct_microphone_deviation(
            left_ir, right_ir
        )

        # 기본 검증
        assert corrected_left.shape == left_ir.shape
        assert corrected_right.shape == right_ir.shape
        assert analysis['correction_applied'] in [True, False]


def run_tests(verbose=True, markers=None):
    """테스트 실행 헬퍼 함수"""
    args = [__file__]

    if verbose:
        args.append('-v')

    if markers:
        args.extend(['-m', markers])

    # 커버리지 보고서 생성 (pytest-cov가 설치된 경우)
    try:
        import pytest_cov  # noqa: F401
        args.extend(['--cov=.', '--cov-report=term-missing'])
    except ImportError:
        pass

    return pytest.main(args)


if __name__ == '__main__':
    # 직접 실행 시
    print("=" * 70)
    print("Impulcifer 유닛 테스트 스위트")
    print("=" * 70)
    print()

    # 빠른 테스트만 실행 (slow 제외)
    exit_code = run_tests(verbose=True, markers='not slow')

    print()
    print("=" * 70)
    if exit_code == 0:
        print("✅ 모든 테스트 통과!")
    else:
        print(f"❌ 테스트 실패 (exit code: {exit_code})")
    print("=" * 70)

    sys.exit(exit_code)
