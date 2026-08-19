# -*- coding: utf-8 -*-
"""EqualizerAPO(-XT) 설정 파서(core/eqapo.py) 테스트.

바이쿼드 수식은 EqualizerAPO-XT의 filters/BiQuad.cpp(RBJ Audio EQ Cookbook)와
동일해야 하며, 폐형식 gainAt 포팅은 동일 계수의 scipy.signal.freqz 크기 응답과
일치해야 한다.
"""

import math

import numpy as np
import pytest

from core.eqapo import (
    ALL_PASS,
    BAND_PASS,
    HIGH_PASS,
    HIGH_SHELF,
    LOW_PASS,
    LOW_SHELF,
    NOTCH,
    PEAKING,
    REASON_CHANNEL_SCOPE,
    REASON_CONDITIONAL,
    REASON_CONVOLUTION_FS_MISMATCH,
    REASON_CONVOLUTION_NOT_FOUND,
    REASON_DISABLED,
    REASON_EXPRESSION,
    REASON_INCLUDE_NOT_FOUND,
    REASON_MALFORMED,
    REASON_SCOPING_IGNORED,
    REASON_UNSUPPORTED,
    _biquad_coefficients,
    _biquad_gain_db,
    _evaluate_biquad,
    _parse_biquad,
    _parse_freq,
    _parse_graphic_eq_nodes,
    _parse_iir,
    looks_like_eqapo_config,
    parse_eqapo_config,
)

FS = 48000
FREQS = np.logspace(np.log10(20), np.log10(20000), 200)


def freqz_gain_db(coeffs, freqs, fs):
    """동일 계수를 scipy.signal.freqz로 평가한 참조 크기 응답."""
    from scipy.signal import freqz

    b0, b1, b2, a1, a2 = coeffs
    _, h = freqz([b0, b1, b2], [1.0, a1, a2], worN=freqs, fs=fs)
    return 20.0 * np.log10(np.abs(h))


class TestBiquadMath:
    """폐형식 gainAt 포팅과 RBJ 계수의 정확성."""

    @pytest.mark.parametrize(
        "type_,gain,q_or_s,is_bw_or_s",
        [
            (PEAKING, -4.7, 0.70, False),
            (PEAKING, 6.0, 1.41, False),
            (PEAKING, 6.0, 1.0, True),  # BW Oct
            (LOW_PASS, 0.0, 1.0 / math.sqrt(2.0), False),
            (HIGH_PASS, 0.0, 1.0 / math.sqrt(2.0), False),
            (BAND_PASS, 0.0, 2.0, False),
            (NOTCH, 0.0, 30.0, False),
            (ALL_PASS, 0.0, 1.0, False),
            (LOW_SHELF, 4.0, 0.9, True),  # S
            (LOW_SHELF, 4.0, 0.5, False),  # Q
            (HIGH_SHELF, -3.0, 0.9, True),
            (HIGH_SHELF, -3.0, 0.71, False),
        ],
    )
    def test_closed_form_matches_freqz(self, type_, gain, q_or_s, is_bw_or_s):
        coeffs = _biquad_coefficients(type_, gain, 1000.0, FS, q_or_s, is_bw_or_s)
        mine = _biquad_gain_db(FREQS, coeffs, FS)
        ref = freqz_gain_db(coeffs, FREQS, FS)
        np.testing.assert_allclose(mine, ref, atol=1e-6)

    def test_peaking_gain_at_center(self):
        coeffs = _biquad_coefficients(PEAKING, -4.7, 1000.0, FS, 0.70, False)
        gain = _biquad_gain_db(np.array([1000.0]), coeffs, FS)
        assert abs(gain[0] - (-4.7)) < 1e-9

    def test_all_pass_is_flat(self):
        coeffs = _biquad_coefficients(ALL_PASS, 0.0, 1000.0, FS, 1.0, False)
        gain = _biquad_gain_db(FREQS, coeffs, FS)
        np.testing.assert_allclose(gain, 0.0, atol=1e-9)

    def test_band_pass_unity_peak(self):
        coeffs = _biquad_coefficients(BAND_PASS, 0.0, 1000.0, FS, 2.0, False)
        gain = _biquad_gain_db(np.array([1000.0]), coeffs, FS)
        assert abs(gain[0]) < 1e-9

    def test_low_shelf_reaches_gain_at_low_freq(self):
        coeffs = _biquad_coefficients(LOW_SHELF, 4.0, 1000.0, FS, 0.9, True)
        gain = _biquad_gain_db(np.array([1.0]), coeffs, FS)
        assert abs(gain[0] - 4.0) < 1e-3

    def test_notch_cuts_center(self):
        coeffs = _biquad_coefficients(NOTCH, 0.0, 1000.0, FS, 30.0, False)
        gain = _biquad_gain_db(np.array([1000.0 * 1.0001]), coeffs, FS)
        assert gain[0] < -20


class TestBiquadParse:
    """BiQuadFilterFactory::parseCommand 파싱 규칙."""

    def test_full_peaking(self):
        spec, failure = _parse_biquad(" ON PK Fc 105 Hz Gain -4.7 dB Q 0.70")
        assert failure is None
        assert spec.type == PEAKING
        assert spec.freq == 105.0
        assert spec.db_gain == -4.7
        assert spec.bandwidth_or_q_or_s == 0.70
        assert not spec.is_bandwidth_or_s
        assert not spec.is_corner_freq

    def test_type_aliases(self):
        for name, expected in [("PEQ", PEAKING), ("Modal", PEAKING), ("LPQ", LOW_PASS), ("HPQ", HIGH_PASS)]:
            params = f" ON {name} Fc 100 Hz Gain 1 dB Q 1"
            spec, failure = _parse_biquad(params)
            assert failure is None, name
            assert spec.type == expected, name

    def test_lp_default_q(self):
        spec, failure = _parse_biquad(" ON LP Fc 1000 Hz")
        assert failure is None
        assert spec.bandwidth_or_q_or_s == pytest.approx(1.0 / math.sqrt(2.0))
        assert not spec.is_bandwidth_or_s

    def test_shelf_default_slope(self):
        spec, failure = _parse_biquad(" ON LS Fc 1000 Hz Gain 4 dB")
        assert failure is None
        assert spec.bandwidth_or_q_or_s == 0.9
        assert spec.is_bandwidth_or_s
        assert not spec.is_corner_freq  # 기본값 경로에서는 코너 변환 없음

    def test_notch_default_q(self):
        spec, failure = _parse_biquad(" ON NO Fc 1000 Hz")
        assert failure is None
        assert spec.bandwidth_or_q_or_s == 30.0

    def test_peaking_requires_q(self):
        spec, failure = _parse_biquad(" ON PK Fc 1000 Hz Gain 3 dB")
        assert spec is None and failure == REASON_MALFORMED

    def test_peaking_requires_gain(self):
        spec, failure = _parse_biquad(" ON PK Fc 1000 Hz Q 1")
        assert spec is None and failure == REASON_MALFORMED

    def test_requires_fc(self):
        spec, failure = _parse_biquad(" ON PK Gain 3 dB Q 1")
        assert spec is None and failure == REASON_MALFORMED

    def test_gain_ignored_for_lp(self):
        spec, failure = _parse_biquad(" ON LP Fc 1000 Hz Gain 55 dB")
        assert failure is None
        assert spec.db_gain == 0.0

    def test_off_and_none_are_disabled(self):
        spec, failure = _parse_biquad(" OFF PK Fc 1000 Hz Gain 3 dB Q 1")
        assert spec is None and failure == REASON_DISABLED
        spec, failure = _parse_biquad(" ON None")
        assert spec is None and failure == REASON_DISABLED

    def test_unknown_type_is_malformed(self):
        spec, failure = _parse_biquad(" ON XYZ Fc 1000 Hz")
        assert spec is None and failure == REASON_MALFORMED

    def test_slope_variant(self):
        spec, failure = _parse_biquad(" ON LS 6dB Fc 100 Hz Gain 4 dB")
        assert failure is None
        assert spec.bandwidth_or_q_or_s == pytest.approx(0.5)  # 6/12
        assert spec.is_bandwidth_or_s
        assert spec.is_corner_freq

    def test_shelf_with_q_uses_corner_freq(self):
        spec, failure = _parse_biquad(" ON HS Fc 1000 Hz Gain -3 dB Q 0.71")
        assert failure is None
        assert not spec.is_bandwidth_or_s
        assert spec.is_corner_freq

    def test_shelf_center_variant_no_corner(self):
        spec, failure = _parse_biquad(" ON HSC Fc 1000 Hz Gain -3 dB Q 0.71")
        assert failure is None
        assert not spec.is_corner_freq

    def test_bw_oct(self):
        spec, failure = _parse_biquad(" ON PK Fc 1000 Hz Gain 6 dB BW Oct 1")
        assert failure is None
        assert spec.bandwidth_or_q_or_s == 1.0
        assert spec.is_bandwidth_or_s

    def test_bw_ignored_for_shelf(self):
        spec, failure = _parse_biquad(" ON LS Fc 1000 Hz Gain 4 dB BW Oct 1")
        assert failure is None
        assert spec.bandwidth_or_q_or_s == 0.9  # BW는 무시되고 기본 S 적용
        assert spec.is_bandwidth_or_s

    def test_comma_decimal(self):
        spec, failure = _parse_biquad(" ON PK Fc 1000,5 Hz Gain -4,7 dB Q 0,70")
        assert failure is None
        assert spec.freq == 1000.5
        assert spec.db_gain == -4.7
        assert spec.bandwidth_or_q_or_s == 0.70


class TestFreqParsing:
    """BiQuadFilterFactory::getFreq의 REW 천단위 구분자 처리."""

    def test_plain(self):
        assert _parse_freq("105") == 105.0
        assert _parse_freq("102.5") == 102.5

    def test_rew_thousands_period(self):
        assert _parse_freq("1.250") == 1250.0

    def test_nbsp_thousands(self):
        assert _parse_freq("1 250") == 1250.0

    def test_scientific_not_thousands(self):
        assert _parse_freq("1.2e3") == 1200.0

    def test_invalid(self):
        assert _parse_freq("abc") == -1.0


class TestShelfCornerFrequency:
    """BiQuadFilter::initialize의 코너 → 중심 주파수 변환."""

    def test_ls_slope_conversion(self):
        # LS 6dB: S=0.5, factor = 10^(|4|/80/0.5) = 10^0.1
        spec, _ = _parse_biquad(" ON LS 6dB Fc 100 Hz Gain 4 dB")
        expected_center = 100.0 * 10.0 ** (4.0 / 80.0 / 0.5)
        coeffs = _biquad_coefficients(LOW_SHELF, 4.0, expected_center, FS, 0.5, True)
        expected = _biquad_gain_db(FREQS, coeffs, FS)
        actual = _evaluate_biquad(spec, FREQS, FS)
        np.testing.assert_allclose(actual, expected, atol=1e-12)

    def test_hs_q_conversion(self):
        spec, _ = _parse_biquad(" ON HS Fc 1000 Hz Gain -3 dB Q 0.71")
        q = 0.71
        a = 10.0 ** (-3.0 / 40.0)
        s = 1.0 / ((1.0 / (q * q) - 2.0) / (a + 1.0 / a) + 1.0)
        expected_center = 1000.0 / (10.0 ** (3.0 / 80.0 / s))
        coeffs = _biquad_coefficients(HIGH_SHELF, -3.0, expected_center, FS, q, False)
        expected = _biquad_gain_db(FREQS, coeffs, FS)
        actual = _evaluate_biquad(spec, FREQS, FS)
        np.testing.assert_allclose(actual, expected, atol=1e-12)

    def test_center_variant_differs_from_corner(self):
        corner, _ = _parse_biquad(" ON LS Fc 100 Hz Gain 4 dB Q 0.5")
        center, _ = _parse_biquad(" ON LSC Fc 100 Hz Gain 4 dB Q 0.5")
        r_corner = _evaluate_biquad(corner, FREQS, FS)
        r_center = _evaluate_biquad(center, FREQS, FS)
        assert np.max(np.abs(r_corner - r_center)) > 0.1


class TestGraphicEQ:
    def test_parse_and_sort(self):
        nodes = _parse_graphic_eq_nodes(" 1000 3; 20 -5; 100 0")
        assert nodes == [(20.0, -5.0), (100.0, 0.0), (1000.0, 3.0)]

    def test_odd_trailing_number_dropped(self):
        nodes = _parse_graphic_eq_nodes(" 20 -5; 100")
        assert nodes == [(20.0, -5.0)]

    def test_comma_decimal_when_no_period(self):
        nodes = _parse_graphic_eq_nodes(" 20 -5,5; 100 0")
        assert nodes == [(20.0, -5.5), (100.0, 0.0)]

    def test_interpolation_log_axis(self):
        text = "GraphicEQ: 20 -5; 100 0; 1000 3"
        freq = np.array([10.0, 20.0, math.sqrt(20.0 * 100.0), 100.0, 1000.0, 20000.0])
        res = parse_eqapo_config(text, FS, freq)
        np.testing.assert_allclose(
            res.left_db, [-5.0, -5.0, -2.5, 0.0, 3.0, 3.0], atol=1e-9
        )


class TestIIR:
    def test_parse_and_flat_gain(self):
        text = "Filter: ON IIR Order 2 Coefficients 0.5 0 0 1 0 0"
        freq = np.array([100.0, 1000.0, 10000.0])
        res = parse_eqapo_config(text, FS, freq)
        assert len(res.applied) == 1
        np.testing.assert_allclose(res.left_db, 20.0 * math.log10(0.5), atol=1e-6)

    def test_wrong_coefficient_count_is_malformed(self):
        assert _parse_iir(" ON IIR Order 2 Coefficients 1 0 0 1 0") is None
        text = "Filter: ON IIR Order 2 Coefficients 1 0 0 1 0"
        res = parse_eqapo_config(text, FS, np.array([1000.0]))
        assert len(res.applied) == 0
        assert res.bypassed[0].reason == REASON_MALFORMED

    def test_order_zero_rejected(self):
        assert _parse_iir(" ON IIR Order 0 Coefficients 1 1") is None


class TestPreamp:
    def test_preamp_applies_to_both(self):
        res = parse_eqapo_config("Preamp: -6.4 dB", FS, np.array([100.0, 1000.0]))
        np.testing.assert_allclose(res.left_db, -6.4)
        np.testing.assert_allclose(res.right_db, -6.4)
        assert res.preamp_left == -6.4 and res.preamp_right == -6.4

    def test_preamps_sum(self):
        res = parse_eqapo_config("Preamp: -3 dB\nPreamp: -2,5 dB", FS, np.array([100.0]))
        assert res.preamp_left == pytest.approx(-5.5)

    def test_malformed_preamp_bypassed(self):
        res = parse_eqapo_config("Preamp: loud dB", FS, np.array([100.0]))
        assert res.bypassed[0].reason == REASON_MALFORMED
        np.testing.assert_allclose(res.left_db, 0.0)


class TestChannelScoping:
    def test_left_only(self):
        text = "Channel: L\nPreamp: -3 dB"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert res.channel_split
        np.testing.assert_allclose(res.left_db, -3.0)
        np.testing.assert_allclose(res.right_db, 0.0)

    def test_numeric_and_all(self):
        text = "Channel: 2\nPreamp: -3 dB\nChannel: all\nPreamp: -1 dB"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -1.0)
        np.testing.assert_allclose(res.right_db, -4.0)

    def test_non_headphone_channel_bypasses(self):
        text = "Channel: C\nFilter: ON PK Fc 100 Hz Gain 3 dB Q 1"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert len(res.applied) == 0
        assert res.bypassed[0].reason == REASON_CHANNEL_SCOPE
        assert not res.channel_split


class TestBypassReporting:
    def test_unsupported_commands(self):
        text = (
            "Copy: L=R\n"
            "Delay: 10 ms\n"
            "MultiConvolution: L br.wav\n"
            "Preamp: -1 dB\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert [r.command for r in res.bypassed] == ["Copy", "Delay", "MultiConvolution"]
        assert all(r.reason == REASON_UNSUPPORTED for r in res.bypassed)
        np.testing.assert_allclose(res.left_db, -1.0)  # Preamp은 정상 적용

    def test_device_and_stage_ignored_but_processing_continues(self):
        text = "Device: Headphones\nStage: post-mix\nPreamp: -2 dB"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert all(r.reason == REASON_SCOPING_IGNORED for r in res.bypassed)
        assert len(res.bypassed) == 2
        np.testing.assert_allclose(res.left_db, -2.0)

    def test_unevaluable_if_block_is_skipped(self):
        text = (
            'If: deviceName == "Speakers"\n'
            "Preamp: -10 dB\n"
            "Else:\n"
            "Preamp: -20 dB\n"
            "EndIf:\n"
            "Preamp: -1 dB\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -1.0)
        reasons = {r.reason for r in res.bypassed}
        assert reasons == {REASON_CONDITIONAL}

    def test_nested_if_blocks(self):
        text = (
            "If: a\n"
            "If: b\n"
            "Preamp: -10 dB\n"
            "EndIf:\n"
            "Preamp: -20 dB\n"
            "EndIf:\n"
            "Preamp: -1 dB\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -1.0)

    def test_inline_expression_bypassed(self):
        text = "Filter: ON PK Fc `sampleRate/4` Hz Gain 3 dB Q 1"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert res.bypassed[0].reason == REASON_EXPRESSION

    def test_disabled_filters_skipped_not_bypassed(self):
        text = "Filter 1: ON None\nFilter 2: OFF PK Fc 100 Hz Gain 3 dB Q 1"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert len(res.bypassed) == 0
        assert len(res.skipped) == 2
        assert all(r.reason == REASON_DISABLED for r in res.skipped)
        np.testing.assert_allclose(res.left_db, 0.0)

    def test_comments_ignored_silently(self):
        text = "# Filter 1: ON PK Fc 100 Hz Gain 3 dB Q 1\nPreamp: -1 dB"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        assert len(res.bypassed) == 0
        np.testing.assert_allclose(res.left_db, -1.0)


class TestIfEvaluation:
    """단순 sampleRate 조건의 If/ElseIf/Else 분기 평가."""

    def test_true_branch_applies(self):
        text = (
            "If: sampleRate == 48000\n"
            "Preamp: -10 dB\n"
            "Else:\n"
            "Preamp: -20 dB\n"
            "EndIf:\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -10.0)
        assert len(res.bypassed) == 0

    def test_false_condition_takes_else(self):
        text = (
            "If: sampleRate == 44100\n"
            "Preamp: -10 dB\n"
            "Else:\n"
            "Preamp: -20 dB\n"
            "EndIf:\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -20.0)
        # 평가된 분기의 미선택 라인은 EqualizerAPO처럼 조용히 건너뛴다.
        assert len(res.bypassed) == 0

    def test_elseif_chain(self):
        text = (
            "If: sampleRate == 44100\n"
            "Preamp: -10 dB\n"
            "ElseIf: sampleRate == 48000\n"
            "Preamp: -20 dB\n"
            "Else:\n"
            "Preamp: -30 dB\n"
            "EndIf:\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -20.0)
        assert len(res.bypassed) == 0

    @pytest.mark.parametrize(
        "condition,expected",
        [
            ("sampleRate == 48000", True),
            ("sampleRate != 48000", False),
            ("sampleRate <= 48000", True),
            ("sampleRate >= 96000", False),
            ("sampleRate < 96000", True),
            ("sampleRate > 44100", True),
            ("(sampleRate == 48000)", True),
        ],
    )
    def test_comparison_operators(self, condition, expected):
        text = f"If: {condition}\nPreamp: -3 dB\nEndIf:\n"
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -3.0 if expected else 0.0)

    def test_unevaluable_elseif_degrades_to_bypass(self):
        text = (
            "If: sampleRate == 44100\n"
            "Preamp: -10 dB\n"
            'ElseIf: deviceName == "X"\n'
            "Preamp: -20 dB\n"
            "EndIf:\n"
            "Preamp: -1 dB\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -1.0)
        assert any(r.reason == REASON_CONDITIONAL for r in res.bypassed)

    def test_nested_evaluated_conditions(self):
        text = (
            "If: sampleRate >= 48000\n"
            "If: sampleRate == 44100\n"
            "Preamp: -10 dB\n"
            "Else:\n"
            "Preamp: -20 dB\n"
            "EndIf:\n"
            "EndIf:\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -20.0)
        assert len(res.bypassed) == 0

    def test_inactive_branch_nested_if_is_silent(self):
        text = (
            "If: sampleRate == 44100\n"
            "If: sampleRate == 48000\n"
            "Preamp: -10 dB\n"
            "EndIf:\n"
            "EndIf:\n"
            "Preamp: -1 dB\n"
        )
        res = parse_eqapo_config(text, FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -1.0)
        assert len(res.bypassed) == 0

    def test_stray_endif_reported(self):
        res = parse_eqapo_config("EndIf:\nPreamp: -1 dB\n", FS, np.array([100.0]))
        np.testing.assert_allclose(res.left_db, -1.0)
        assert res.bypassed[0].command == "EndIf"


class TestConvolution:
    """Convolution 명령의 크기 응답 합성."""

    @staticmethod
    def _write_wav(path, data, fs=FS):
        import soundfile as sf

        sf.write(str(path), data, fs, subtype="FLOAT")

    def test_flat_gain_ir(self, tmp_path):
        self._write_wav(tmp_path / "ir.wav", np.array([0.5, 0.0, 0.0, 0.0]))
        res = parse_eqapo_config(
            "Convolution: ir.wav", FS, np.array([100.0, 1000.0, 10000.0]),
            base_dir=str(tmp_path),
        )
        assert len(res.applied) == 1
        np.testing.assert_allclose(res.left_db, 20.0 * math.log10(0.5), atol=1e-6)
        assert not res.channel_split

    def test_ir_magnitude_matches_dft(self, tmp_path):
        # ir_short.wav와 동일한 구성: h[0]=1.0, h[20]=0.5, h[40]=0.25
        ir = np.zeros(64)
        ir[0], ir[20], ir[40] = 1.0, 0.5, 0.25
        self._write_wav(tmp_path / "ir.wav", ir)
        freq = np.linspace(20.0, 20000.0, 50)
        res = parse_eqapo_config(
            "Convolution: ir.wav", FS, freq, base_dir=str(tmp_path)
        )
        omega = -2j * np.pi * np.outer(freq, np.arange(64)) / FS
        expected = 20.0 * np.log10(np.abs(np.exp(omega) @ ir))
        np.testing.assert_allclose(res.left_db, expected, atol=1e-6)

    def test_stereo_ir_maps_per_channel(self, tmp_path):
        # IrCache.cpp: 채널 i는 IR의 i % ch 채널을 사용한다.
        stereo = np.zeros((4, 2))
        stereo[0, 0] = 1.0
        stereo[0, 1] = 0.5
        self._write_wav(tmp_path / "ir.wav", stereo)
        res = parse_eqapo_config(
            "Convolution: ir.wav", FS, np.array([1000.0]), base_dir=str(tmp_path)
        )
        assert res.channel_split
        np.testing.assert_allclose(res.left_db, 0.0, atol=1e-9)
        np.testing.assert_allclose(res.right_db, 20.0 * math.log10(0.5), atol=1e-9)

    def test_quoted_path(self, tmp_path):
        self._write_wav(tmp_path / "my ir.wav", np.array([1.0]))
        res = parse_eqapo_config(
            'Convolution: "my ir.wav"', FS, np.array([1000.0]), base_dir=str(tmp_path)
        )
        assert len(res.applied) == 1

    def test_fs_mismatch_bypassed(self, tmp_path):
        # IrCache.cpp: 1 Hz 넘게 다르면 필터를 만들지 않는다.
        self._write_wav(tmp_path / "ir.wav", np.array([1.0]), fs=44100)
        res = parse_eqapo_config(
            "Convolution: ir.wav", FS, np.array([1000.0]), base_dir=str(tmp_path)
        )
        assert len(res.applied) == 0
        assert res.bypassed[0].reason == REASON_CONVOLUTION_FS_MISMATCH

    def test_missing_file_bypassed(self, tmp_path):
        res = parse_eqapo_config(
            "Convolution: nope.wav", FS, np.array([1000.0]), base_dir=str(tmp_path)
        )
        assert res.bypassed[0].reason == REASON_CONVOLUTION_NOT_FOUND

    def test_no_base_dir_bypassed(self):
        res = parse_eqapo_config("Convolution: ir.wav", FS, np.array([1000.0]))
        assert res.bypassed[0].reason == REASON_CONVOLUTION_NOT_FOUND

    def test_channel_scoped_convolution(self, tmp_path):
        self._write_wav(tmp_path / "ir.wav", np.array([0.5]))
        text = "Channel: L\nConvolution: ir.wav"
        res = parse_eqapo_config(text, FS, np.array([1000.0]), base_dir=str(tmp_path))
        assert res.channel_split
        np.testing.assert_allclose(res.left_db, 20.0 * math.log10(0.5), atol=1e-9)
        np.testing.assert_allclose(res.right_db, 0.0, atol=1e-9)


class TestInclude:
    def test_include_resolved(self, tmp_path):
        child = tmp_path / "child.txt"
        child.write_text("Preamp: -3 dB\n", encoding="utf-8")
        text = "Include: child.txt\nPreamp: -1 dB"
        res = parse_eqapo_config(text, FS, np.array([100.0]), base_dir=str(tmp_path))
        np.testing.assert_allclose(res.left_db, -4.0)
        assert len(res.bypassed) == 0

    def test_include_missing_bypassed(self, tmp_path):
        text = "Include: nope.txt"
        res = parse_eqapo_config(text, FS, np.array([100.0]), base_dir=str(tmp_path))
        assert res.bypassed[0].reason == REASON_INCLUDE_NOT_FOUND

    def test_include_without_base_dir_bypassed(self):
        res = parse_eqapo_config("Include: child.txt", FS, np.array([100.0]))
        assert res.bypassed[0].reason == REASON_INCLUDE_NOT_FOUND

    def test_include_cycle_guard(self, tmp_path):
        a = tmp_path / "a.txt"
        a.write_text("Preamp: -1 dB\nInclude: a.txt\n", encoding="utf-8")
        text = "Include: a.txt"
        res = parse_eqapo_config(text, FS, np.array([100.0]), base_dir=str(tmp_path))
        np.testing.assert_allclose(res.left_db, -1.0)
        assert any(r.reason == REASON_INCLUDE_NOT_FOUND for r in res.bypassed)

    def test_channel_scope_restored_after_include(self, tmp_path):
        child = tmp_path / "child.txt"
        child.write_text("Channel: L\nPreamp: -3 dB\n", encoding="utf-8")
        text = "Include: child.txt\nPreamp: -1 dB"
        res = parse_eqapo_config(text, FS, np.array([100.0]), base_dir=str(tmp_path))
        np.testing.assert_allclose(res.left_db, -4.0)
        np.testing.assert_allclose(res.right_db, -1.0)  # 포함 파일의 Channel: L은 복원됨


class TestSniffer:
    def test_detects_parametric_eq(self):
        text = "Preamp: -6.4 dB\nFilter 1: ON PK Fc 105 Hz Gain -4.7 dB Q 0.70\n"
        assert looks_like_eqapo_config(text)

    def test_detects_graphic_eq(self):
        assert looks_like_eqapo_config("GraphicEQ: 20 -1.1; 100 0.5\n")

    def test_rejects_autoeq_csv(self):
        text = "frequency,raw,error\n20,0.0,0.1\n40,1.0,0.2\n"
        assert not looks_like_eqapo_config(text)

    def test_rejects_two_column_data(self):
        assert not looks_like_eqapo_config("20 0.0\n40 1.0\n")

    def test_rejects_commented_commands(self):
        assert not looks_like_eqapo_config("# Preamp: -6 dB\n")


class TestEqualizationIntegration:
    """pipeline_stages.equalization()의 EqualizerAPO 파일 지원."""

    @staticmethod
    def _estimator():
        from types import SimpleNamespace

        return SimpleNamespace(fs=FS)

    def test_eqapo_txt_in_eq_csv(self, tmp_path):
        from core.pipeline_stages import equalization

        (tmp_path / "eq.csv").write_text(
            "Preamp: -6.4 dB\nFilter 1: ON PK Fc 105 Hz Gain -4.7 dB Q 0.70\n",
            encoding="utf-8",
        )
        left, right = equalization(self._estimator(), str(tmp_path))
        assert left is not None
        assert right is left  # 채널 분리가 없으면 동일 객체 재사용
        # error = -raw 규약: 파이프라인이 EQ 곡선을 그대로 적용하게 한다
        np.testing.assert_allclose(left.error, -left.raw, atol=1e-9)
        # 105 Hz 부근에서 PK 이득 + 프리앰프
        idx = int(np.argmin(np.abs(left.frequency - 105.0)))
        assert abs(left.raw[idx] - (-4.7 - 6.4)) < 0.05

    def test_eq_txt_filename_fallback(self, tmp_path):
        from core.pipeline_stages import equalization

        (tmp_path / "eq.txt").write_text("Preamp: -3 dB\n", encoding="utf-8")
        left, right = equalization(self._estimator(), str(tmp_path))
        assert left is not None
        np.testing.assert_allclose(left.raw, -3.0, atol=1e-9)

    def test_channel_split_produces_two_curves(self, tmp_path):
        from core.pipeline_stages import equalization

        (tmp_path / "eq.csv").write_text(
            "Channel: L\nPreamp: -3 dB\nChannel: R\nPreamp: -1 dB\n",
            encoding="utf-8",
        )
        left, right = equalization(self._estimator(), str(tmp_path))
        assert left is not None and right is not None and right is not left
        np.testing.assert_allclose(left.raw, -3.0, atol=1e-9)
        np.testing.assert_allclose(right.raw, -1.0, atol=1e-9)

    def test_autoeq_csv_still_works(self, tmp_path):
        from core.pipeline_stages import equalization

        rows = ["frequency,raw,error"]
        f = 20.0
        while f <= 20000.0:
            rows.append(f"{f:.2f},1.00,0.50")
            f *= 1.1
        (tmp_path / "eq.csv").write_text("\n".join(rows) + "\n", encoding="utf-8")
        left, right = equalization(self._estimator(), str(tmp_path))
        assert left is not None
        assert right is left
        np.testing.assert_allclose(left.error, 0.5, atol=1e-6)

    def test_no_eq_files(self, tmp_path):
        from core.pipeline_stages import equalization

        left, right = equalization(self._estimator(), str(tmp_path))
        assert left is None and right is None

    def test_two_column_plain_file(self, tmp_path):
        """error 열이 없는 평문 gain 곡선 파일은 error = -raw로 적용된다."""
        from core.pipeline_stages import equalization

        rows = []
        f = 20.0
        while f <= 20000.0:
            rows.append(f"{f:.2f} -3.00")
            f *= 1.5
        (tmp_path / "eq.csv").write_text("\n".join(rows) + "\n", encoding="utf-8")
        left, right = equalization(self._estimator(), str(tmp_path))
        assert left is not None
        assert len(left.error) == len(left.raw) > 0
        np.testing.assert_allclose(left.error, -left.raw, atol=1e-9)
        np.testing.assert_allclose(left.raw, -3.0, atol=1e-6)


class TestRealWorldConfigs:
    """실제 도구가 내보내는 형태의 설정 파일."""

    def test_autoeq_parametric_export(self):
        text = (
            "Preamp: -6.8 dB\n"
            "Filter 1: ON LSC Fc 105 Hz Gain 6.7 dB Q 0.70\n"
            "Filter 2: ON PK Fc 208 Hz Gain -3.1 dB Q 0.61\n"
            "Filter 3: ON PK Fc 1364 Hz Gain 2.5 dB Q 1.78\n"
            "Filter 4: ON PK Fc 5793 Hz Gain -4.9 dB Q 3.00\n"
            "Filter 5: ON HSC Fc 10000 Hz Gain -4.5 dB Q 0.70\n"
        )
        res = parse_eqapo_config(text, FS, FREQS)
        assert len(res.applied) == 5
        assert len(res.bypassed) == 0
        assert res.preamp_left == pytest.approx(-6.8)
        assert not res.channel_split

    def test_eqapo_config_with_mixed_commands(self):
        text = (
            "# EqualizerAPO-XT export\n"
            "Device: Speakers\n"
            "Preamp: -2 dB\n"
            "Channel: L R\n"
            "Filter 1: ON PK Fc 100 Hz Gain 2 dB Q 1.0\n"
            "GraphicEQ: 20 0; 20000 -3\n"
            "Convolution: room.wav\n"
            "Delay: 5 samples\n"
        )
        res = parse_eqapo_config(text, FS, FREQS)
        assert len(res.applied) == 2  # PK + GraphicEQ
        commands = [r.command for r in res.bypassed]
        assert "Device" in commands
        assert "Convolution" in commands
        assert "Delay" in commands
