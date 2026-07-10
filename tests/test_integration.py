# -*- coding: utf-8 -*-

"""마이크 편차 보정 (v4.0) 통합 테스트.

HRIR 처리 경로(여러 스피커 수집 → 방향 무관 양이 불일치 추정 → 보정 적용)가
오류 없이 동작하고 좌우 데이터를 실제로 수정하는지 확인한다.
"""

import numpy as np

from core.hrir import HRIR
from core.impulse_response import ImpulseResponse


def _build_hrir_with_mic_offset(mic_offset_db=3.0, fs=48000, length=4800):
    """좌측 마이크가 일관되게 +mic_offset_db 큰 HRIR을 만든다."""

    class DummyEstimator:
        def __init__(self, fs):
            self.fs = fs
            self.test_signal = np.zeros(fs)

    hrir = HRIR(DummyEstimator(fs))
    rng = np.random.default_rng(0)
    left_scale = 10 ** (mic_offset_db / 20.0)

    # 대칭 레이아웃: 방향성 ILD는 좌우 대칭이라 평균에서 상쇄되고,
    # 일관된 마이크 오프셋(좌측 +offset)만 남도록 구성한다.
    speaker_ild = {"FL": +2.0, "FR": -2.0, "FC": 0.0}
    for speaker, ild_db in speaker_ild.items():
        base = np.zeros(length)
        base[1000] = 1.0
        base += rng.normal(0, 0.002, length)
        # 방향성 ILD (좌우 대칭)
        ild_scale = 10 ** (ild_db / 20.0)
        left = base * ild_scale * left_scale
        right = base / ild_scale
        hrir.irs[speaker] = {
            "left": ImpulseResponse(left.copy(), fs),
            "right": ImpulseResponse(right.copy(), fs),
        }
    return hrir


def test_microphone_deviation_integration():
    """보정이 오류 없이 적용되고 데이터가 변경되어야 한다."""
    hrir = _build_hrir_with_mic_offset(mic_offset_db=3.0)

    original = {
        spk: {"left": pair["left"].data.copy(), "right": pair["right"].data.copy()}
        for spk, pair in hrir.irs.items()
    }

    summary = hrir.correct_microphone_deviation(correction_strength=1.0)

    assert "error" not in summary
    assert summary["method"] == "interaural_v4"
    assert summary["speakers_processed"], "처리된 스피커가 없음"
    assert summary["max_error_db"] > 0.0

    for spk, pair in hrir.irs.items():
        assert not np.array_equal(original[spk]["left"], pair["left"].data)
        assert not np.array_equal(original[spk]["right"], pair["right"].data)


def test_anchor_uses_frontal_when_fc_present():
    """FC가 있으면 정면 앵커를 사용한다."""
    hrir = _build_hrir_with_mic_offset(mic_offset_db=2.0)
    summary = hrir.correct_microphone_deviation(correction_strength=0.7, anchor="auto")
    assert summary["anchor"] == "frontal"
