# -*- coding: utf-8 -*-
"""EqualizerAPO-XT 엔진 골든 출력과의 교차 검증.

`tests/fixtures/eqapo/`의 레퍼런스 `.raw`는 실제 EqualizerAPO-XT
FilterEngine(C++)이 결정적 입력(임펄스/DC)을 처리한 float32 인터리브드
스테레오 출력이다(픽스처 출처는 fixtures/eqapo/README.md 참고). 임펄스 입력의
출력은 필터 체인의 임펄스 응답 그 자체이므로, FFT 크기 스펙트럼이 EqualizerAPO
필터의 실제 크기 응답이 된다. 이 테스트는 core/eqapo.py가 합성한 크기 응답이
그 실측 응답과 일치하는지 검증한다. Impulcifer가 바이패스하는 명령(Copy,
Delay, LoudnessCorrection)은 코퍼스에서 제외했다.
"""

from pathlib import Path

import numpy as np

from core.eqapo import _evaluate_graphic_eq, _parse_graphic_eq_nodes, parse_eqapo_config

FIXTURE_DIR = Path(__file__).parent / "fixtures" / "eqapo"
FS = 48000


def _load_reference(name, frames, channels=2):
    """레퍼런스 raw(little-endian float32, 인터리브드)를 (frames, ch)로 읽는다."""
    data = np.fromfile(FIXTURE_DIR / f"{name}.raw", dtype="<f4")
    assert data.size == frames * channels, f"{name}.raw size mismatch"
    return data.reshape(frames, channels).astype(np.float64)


def _parse_fixture_config(name, frequency):
    text = (FIXTURE_DIR / f"{name}.txt").read_text(encoding="utf-8")
    return parse_eqapo_config(text, FS, frequency, base_dir=str(FIXTURE_DIR))


def _magnitude_case(name, frames):
    """임펄스 입력 케이스: 출력 FFT 크기와 합성 곡선의 선형 크기를 비교한다."""
    reference = _load_reference(name, frames)
    bin_freqs = np.fft.rfftfreq(frames, 1.0 / FS)
    result = _parse_fixture_config(name, bin_freqs)
    ref_left = np.abs(np.fft.rfft(reference[:, 0]))
    ref_right = np.abs(np.fft.rfft(reference[:, 1]))
    ours_left = 10.0 ** (result.left_db / 20.0)
    ours_right = 10.0 ** (result.right_db / 20.0)
    return ours_left, ours_right, ref_left, ref_right


class TestEngineMagnitudeParity:
    """비-바이패스 필터의 합성 응답 == 실제 엔진 응답."""

    def test_biquad_peaking_1khz(self):
        # float32 처리 잡음 수준(≈4e-8)에서 일치해야 한다.
        ours_l, ours_r, ref_l, ref_r = _magnitude_case("biquad_peaking_1khz", 8192)
        np.testing.assert_allclose(ours_l, ref_l, atol=1e-6)
        np.testing.assert_allclose(ours_r, ref_r, atol=1e-6)

    def test_iir_order2_lowpass(self):
        ours_l, ours_r, ref_l, ref_r = _magnitude_case("iir_order2_lowpass", 256)
        np.testing.assert_allclose(ours_l, ref_l, atol=1e-9)
        np.testing.assert_allclose(ours_r, ref_r, atol=1e-9)

    def test_convolution_short(self):
        ours_l, ours_r, ref_l, ref_r = _magnitude_case("convolution_short", 4096)
        np.testing.assert_allclose(ours_l, ref_l, atol=1e-9)
        np.testing.assert_allclose(ours_r, ref_r, atol=1e-9)

    def test_preamp_minus6_dc(self):
        # DC 입력: 정상 상태 출력 = 선형 게인.
        reference = _load_reference("preamp_minus6", 4800)
        result = _parse_fixture_config("preamp_minus6", np.array([100.0]))
        expected = 10.0 ** (result.left_db[0] / 20.0)
        assert abs(reference[-1, 0] - expected) < 1e-6
        assert abs(reference[-1, 1] - expected) < 1e-6

    def test_channel_left_only_dc(self):
        # Channel: L 스코핑 — 좌측만 -6 dB, 우측은 그대로.
        reference = _load_reference("channel_left_only", 256)
        result = _parse_fixture_config("channel_left_only", np.array([100.0]))
        assert result.channel_split
        left_gain = 10.0 ** (result.left_db[0] / 20.0)
        right_gain = 10.0 ** (result.right_db[0] / 20.0)
        assert abs(reference[-1, 0] - left_gain) < 1e-6
        assert abs(reference[-1, 1] - right_gain) < 1e-6

    def test_graphiceq_15band_ideal_curve(self):
        """GainIterator 이상 곡선 vs 엔진의 FIR 실현 응답.

        엔진은 16384탭 최소위상 FIR + 반코사인 윈도우로 곡선을 실현하므로
        저주파에서 이상 곡선과 벌어진다(실측 최대 ≈0.55 dB @ 20-100 Hz).
        이 편차는 EqualizerAPO 자체의 실현 오차이며, 대역별 허용치는 그
        실측값에 여유를 둔 것이다. 샘플 단위 정합은 아래 재구성 테스트가
        담당한다.
        """
        ours_l, _, ref_l, _ = _magnitude_case("graphiceq_15band", 8192)
        bin_freqs = np.fft.rfftfreq(8192, 1.0 / FS)
        mask = ref_l > 1e-6
        error_db = np.abs(20.0 * np.log10(ours_l[mask] / ref_l[mask]))
        freqs = bin_freqs[mask]
        for f_lo, f_hi, tol_db in [(0, 100, 0.7), (100, 1000, 0.15), (1000, 24000, 0.02)]:
            band = (freqs >= f_lo) & (freqs < f_hi)
            assert error_db[band].max() < tol_db, (
                f"{f_lo}-{f_hi} Hz: {error_db[band].max():.4f} dB"
            )

    def test_graphiceq_15band_fir_reconstruction(self):
        """GraphicEQFilter의 FIR 합성 체인을 재현해 샘플 단위로 비교한다.

        우리의 GainIterator 포팅이 EqualizerAPO와 빈 단위로 동일한 게인을
        내면, 엔진과 같은 최소위상 재구성(mps) + 반코사인 윈도우를 거친
        FIR은 레퍼런스 출력(임펄스 응답)과 float32 잡음 수준에서 일치해야
        한다. (GraphicEQFilter.cpp / mps() 알고리즘의 NumPy 재현.)
        """
        n = 16384  # GraphicEQFilterFactory가 사용하는 filterLength
        n2 = 2 * n
        config_text = (FIXTURE_DIR / "graphiceq_15band.txt").read_text(encoding="utf-8")
        nodes = _parse_graphic_eq_nodes(config_text.split(":", 1)[1])

        # 엔진과 동일한 빈 주파수에서 게인 샘플링 (우리의 GainIterator 포팅)
        bin_freqs = np.arange(n) * FS / n2
        gain = 10.0 ** (_evaluate_graphic_eq(nodes, bin_freqs) / 20.0)
        spectrum = np.concatenate([gain, gain[::-1]])

        # mps(): 켑스트럼 기반 최소위상 재구성
        threshold = 10.0 ** (-100.0 / 20.0)
        log_mag = np.where(spectrum < threshold, np.log(threshold), np.log(spectrum))
        time_data = np.fft.ifft(log_mag.astype(complex))
        folded = time_data.copy()
        folded[1:n] = (
            time_data[1:n].real + time_data[n2 - 1:n:-1].real
        ) + 1j * (time_data[1:n].imag - time_data[n2 - 1:n:-1].imag)
        folded[n + 1:] = 0
        folded[n] = np.conj(folded[n])
        log_spectrum = np.fft.fft(folded)
        min_phase = np.exp(log_spectrum.real) * (
            np.cos(log_spectrum.imag) + 1j * np.sin(log_spectrum.imag)
        )

        # 시간 영역 IR + 반코사인 윈도우
        ir_full = np.fft.ifft(min_phase)
        window = 0.5 * (1.0 + np.cos(2.0 * np.pi * np.arange(n) / n2))
        ir = (ir_full[:n].real) * window

        # 임펄스 입력이므로 레퍼런스 출력 == FIR 앞 8192탭
        reference = _load_reference("graphiceq_15band", 8192)
        np.testing.assert_allclose(ir[:8192], reference[:, 0], atol=1e-6)
        np.testing.assert_allclose(ir[:8192], reference[:, 1], atol=1e-6)
