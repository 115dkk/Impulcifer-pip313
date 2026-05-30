# -*- coding: utf-8 -*-

"""마이크 착용 편차 보정 (v4.0) 단위 테스트.

방향 무관 양이(interaural) 마이크 불일치를 시뮬레이션해 추정·보정이
올바르게 동작하는지 확인한다. (플롯/하드웨어 의존성 없음)
"""

import numpy as np

from core.microphone_deviation_correction import (
    MicrophoneDeviationCorrector,
    MicrophoneMatchingCorrector,
)


def _impulse_pair(left_gain=1.0, right_gain=1.0, length=4800, peak=1000):
    left = np.zeros(length)
    right = np.zeros(length)
    left[peak] = left_gain
    right[peak] = right_gain
    return left, right


def test_estimate_matches_known_offset():
    """좌측이 정확히 +3 dB일 때 추정 Δ가 대역에서 ~3 dB여야 한다."""
    fs = 48000
    corrector = MicrophoneMatchingCorrector(sample_rate=fs, correction_strength=1.0)
    gain = 10 ** (-3.0 / 20.0)  # 우측을 3 dB 낮춤
    left, right = _impulse_pair(1.0, gain)
    corrector.collect_speaker("FL", left, right, 1000, 1000)
    delta = corrector.estimate_interaural_mismatch()

    f = corrector.frequency
    mid = (f >= 800) & (f <= 1200)
    assert 2.0 < np.mean(delta[mid]) < 4.0


def test_correction_reduces_interaural_difference():
    """보정 후 좌우 직접음 레벨 차이가 줄어들어야 한다."""
    fs = 48000
    corrector = MicrophoneDeviationCorrector(sample_rate=fs, correction_strength=1.0)
    gain = 10 ** (-4.0 / 20.0)
    left, right = _impulse_pair(1.0, gain)

    cl, cr, analysis = corrector.correct_microphone_deviation(
        left, right, left_peak_index=1000, right_peak_index=1000
    )
    assert analysis["correction_applied"] is True

    def band_level_db(x):
        from scipy.fft import rfft
        spec = np.abs(rfft(x, n=8192))
        f = np.fft.rfftfreq(8192, 1 / fs)
        band = (f >= 800) & (f <= 1200)
        return 20 * np.log10(np.mean(spec[band]) + 1e-20)

    before = band_level_db(left) - band_level_db(right)
    after = band_level_db(cl) - band_level_db(cr)
    assert abs(after) < abs(before)


def test_matched_microphones_skip_correction():
    """좌우가 같으면 보정을 적용하지 않는다."""
    corrector = MicrophoneDeviationCorrector(sample_rate=48000)
    left, right = _impulse_pair(1.0, 1.0)
    cl, cr, analysis = corrector.correct_microphone_deviation(
        left, right, left_peak_index=1000, right_peak_index=1000
    )
    assert analysis["correction_applied"] is False
    assert cl.shape == left.shape and cr.shape == right.shape
