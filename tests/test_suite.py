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

try:
    from core.microphone_deviation_correction import MicrophoneDeviationCorrector
    from core.impulse_response import ImpulseResponse
except ImportError:
    # 패키지가 설치되지 않은 경우 프로젝트 루트를 경로에 추가
    sys.path.insert(0, str(Path(__file__).parent.parent))
    from core.microphone_deviation_correction import MicrophoneDeviationCorrector
    from core.impulse_response import ImpulseResponse


class TestMicrophoneDeviationCorrector:
    """마이크 편차 보정 v4.0 테스트 (방향 무관 양이 불일치)"""

    @pytest.fixture
    def corrector(self):
        """기본 corrector 인스턴스"""
        return MicrophoneDeviationCorrector(
            sample_rate=48000,
            correction_strength=0.7
        )

    @staticmethod
    def _impulse_pair(left_gain=1.0, right_gain=1.0, length=4800, peak=1000):
        left = np.zeros(length)
        right = np.zeros(length)
        left[peak] = left_gain
        right[peak] = right_gain
        return left, right

    def test_corrector_initialization(self, corrector):
        """초기화 테스트"""
        assert corrector.fs == 48000
        assert corrector.correction_strength == 0.7
        assert len(corrector.frequency) > 0, "주파수 그리드가 비어있음"
        assert corrector.f_min < corrector.f_max
        assert corrector.anchor == "auto"

    def test_collect_speaker(self, corrector):
        """스피커 직접음 파워 수집 테스트"""
        left, right = self._impulse_pair(1.0, 0.8)
        data = corrector.collect_speaker('FL', left, right, 1000, 1000)
        assert 'left' in data and 'right' in data
        assert len(data['left']) == len(corrector.frequency)
        assert 'FL' in corrector.speaker_power

    def test_estimate_mismatch_sign(self, corrector):
        """왼쪽이 ~3dB 큰 경우 Δ는 대역에서 양수여야 함"""
        gain = 10 ** (-3.0 / 20.0)
        left, right = self._impulse_pair(1.0, gain)
        corrector.collect_speaker('FL', left, right, 1000, 1000)
        delta = corrector.estimate_interaural_mismatch()
        assert len(delta) == len(corrector.frequency)
        f = corrector.frequency
        mid = (f >= 800) & (f <= 1200)
        assert np.mean(delta[mid]) > 1.0, "마이크 불일치 방향(양수)이 아님"

    def test_band_limit_taper(self, corrector):
        """보정 대역 밖은 0으로 테이퍼되어야 함"""
        gain = 10 ** (-3.0 / 20.0)
        left, right = self._impulse_pair(1.0, gain)
        corrector.collect_speaker('FL', left, right, 1000, 1000)
        delta = corrector.estimate_interaural_mismatch()
        f = corrector.frequency
        low_idx = int(np.argmin(np.abs(f - 30.0)))
        assert abs(delta[low_idx]) < 0.5, "저역이 테이퍼되지 않음"
        very_high = f >= corrector.fs / 2 * 0.99
        if np.any(very_high):
            assert np.all(np.abs(delta[very_high]) < 0.5), "초고역이 테이퍼되지 않음"

    def test_frontal_anchor_preferred(self):
        """FC가 있으면 정면(frontal) 앵커를 사용해야 함"""
        c = MicrophoneDeviationCorrector(sample_rate=48000, anchor='auto')
        gain = 10 ** (-3.0 / 20.0)
        # FC: 왼쪽이 큼 / FL: 오른쪽이 큼(반대 방향)
        l1, r1 = self._impulse_pair(1.0, gain)
        l2, r2 = self._impulse_pair(gain, 1.0)
        c.collect_speaker('FC', l1, r1, 1000, 1000)
        c.collect_speaker('FL', l2, r2, 1000, 1000)
        delta = c.estimate_interaural_mismatch()
        assert c.anchor_used == 'frontal'
        f = c.frequency
        mid = (f >= 800) & (f <= 1200)
        assert np.mean(delta[mid]) > 1.0, "FC(양수)를 따르지 않음"

    def test_design_correction_filters(self, corrector):
        """불일치가 있으면 좌/우 FIR이 생성되어야 함"""
        gain = 10 ** (-3.0 / 20.0)
        left, right = self._impulse_pair(1.0, gain)
        corrector.collect_speaker('FL', left, right, 1000, 1000)
        corrector.estimate_interaural_mismatch()
        left_fir, right_fir = corrector.design_correction_filters()
        assert len(left_fir) > 1 and len(right_fir) > 1

    def test_single_pair_correction(self, corrector):
        """단일 스피커 쌍 보정 (호환 API), 길이 보존"""
        gain = 10 ** (-3.0 / 20.0)
        left, right = self._impulse_pair(1.0, gain)
        cl, cr, analysis = corrector.correct_microphone_deviation(
            left, right, left_peak_index=1000, right_peak_index=1000
        )
        assert cl.shape == left.shape
        assert cr.shape == right.shape
        assert analysis['correction_applied'] is True
        assert analysis['method'] == 'interaural_v4'

    def test_no_correction_when_matched(self, corrector):
        """좌우가 동일하면 보정을 건너뛴다"""
        left, right = self._impulse_pair(1.0, 1.0)
        _cl, _cr, analysis = corrector.correct_microphone_deviation(
            left, right, left_peak_index=1000, right_peak_index=1000
        )
        assert analysis['correction_applied'] is False


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
            'core.impulse_response',
            'core.hrir',
            'core.microphone_deviation_correction',
        ]

        for module_name in modules_to_test:
            try:
                __import__(module_name)
            except ImportError as e:
                pytest.fail(f"모듈 {module_name} 임포트 실패: {e}")

    def test_recorder_module_importable(self):
        """recorder 모듈 임포트 테스트 (오디오 하드웨어 필요)"""
        try:
            import core.recorder  # noqa: F401
        except (ImportError, OSError) as e:
            # CI 환경에서는 PortAudio가 없을 수 있음
            pytest.skip(f"recorder 모듈 임포트 불가 (정상): {e}")

    def test_gui_modules_importable(self):
        """GUI 모듈 임포트 테스트 (선택적)"""
        try:
            from gui import modern_gui  # noqa: F401
            from gui import legacy_gui  # noqa: F401
        except (ImportError, OSError) as e:
            # CI 환경에서는 PortAudio가 없을 수 있음
            pytest.skip(f"GUI 모듈 임포트 불가 (정상): {e}")


class TestDataFiles:
    """데이터 파일 존재 확인 테스트"""

    @staticmethod
    def _project_root():
        return Path(__file__).parent.parent

    def test_data_directory_exists(self):
        """data 디렉토리 존재 확인"""
        data_dir = self._project_root() / 'data'
        assert data_dir.exists(), "data 디렉토리가 없음"

    def test_essential_data_files(self):
        """필수 데이터 파일 존재 확인"""
        essential_files = [
            'data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
            'data/harman-in-room-headphone-target.csv',
        ]

        for file_path in essential_files:
            full_path = self._project_root() / file_path
            assert full_path.exists(), f"필수 파일 {file_path}가 없음"


class TestConfigurationFiles:
    """설정 파일 검증 테스트"""

    @staticmethod
    def _project_root():
        return Path(__file__).parent.parent

    def test_pyproject_toml_exists(self):
        """pyproject.toml 존재 확인"""
        pyproject = self._project_root() / 'pyproject.toml'
        assert pyproject.exists(), "pyproject.toml이 없음"

    def test_pyproject_toml_valid(self):
        """pyproject.toml 유효성 검사"""
        try:
            import tomllib
        except ImportError:
            import tomli as tomllib

        pyproject = self._project_root() / 'pyproject.toml'
        try:
            with open(pyproject, 'rb') as f:
                config = tomllib.load(f)

            assert 'project' in config
            assert 'name' in config['project']
            assert config['project']['name'] == 'impulcifer-py313'
        except Exception as e:
            pytest.fail(f"pyproject.toml 파싱 실패: {e}")

    def test_processing_config_matches_main_room_limits(self):
        """ProcessingConfig is the single source of truth for room-EQ limits.

        ``core.cli_builder`` derives the argparse CLI defaults directly from
        these dataclass field defaults, and ``impulcifer.main`` now forwards
        ``**kwargs`` straight into :class:`ProcessingConfig` (there are no
        per-parameter signature defaults left to drift). Pinning the canonical
        room-correction limits here guards against silently changing the CLI
        default behavior and the BRIR hash.
        """
        from core.pipeline import ProcessingConfig

        config = ProcessingConfig()
        assert config.specific_limit == 400
        assert config.generic_limit == 300


class TestVersionConsistency:
    """버전 일관성 테스트"""

    def test_version_in_pyproject(self):
        """pyproject.toml의 버전 확인"""
        pyproject = Path(__file__).parent.parent / 'pyproject.toml'

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
        corrector = MicrophoneDeviationCorrector(
            sample_rate=48000,
            correction_strength=0.5
        )

        length = 48000  # 1초
        left_ir = np.random.randn(length) * 0.01
        right_ir = np.random.randn(length) * 0.01

        left_ir[10000] = 1.0
        right_ir[10000] = 0.9

        corrected_left, corrected_right, analysis = corrector.correct_microphone_deviation(
            left_ir, right_ir
        )

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
