"""Bokeh 분석 플롯(ILD/IPD/IACC/ETC)의 음향학적 정합성 테스트.

플롯에 실리는 값이 물리적으로 의미 있는 값인지 공개 API
(``HRIR.generate_*_bokeh_layout``)를 통해 검증한다. 합성 HRIR은
지연/스케일 관계가 알려진 신호쌍이므로 각 지표의 정답을 닫힌 식으로
계산할 수 있다.
"""

import sys
import types

import numpy as np
import pytest

sys.path.insert(0, ".")

from core.hrir import HRIR
from core.impulse_response import ImpulseResponse

FS = 48000


def test_bokeh_generator_registry_uses_interactive_titles_and_stage_subset():
    from core.plotting.bokeh_registry import BOKEH_ANALYSIS_GENERATORS

    assert [config.title for config in BOKEH_ANALYSIS_GENERATORS] == [
        "Interaural Overlay",
        "ILD",
        "IPD",
        "IACC",
        "EDC",
        "Result Overview",
    ]
    assert [
        config.name
        for config in BOKEH_ANALYSIS_GENERATORS
        if config.save_individually
    ] == ["ild", "ipd", "iacc", "etc"]


def make_hrir(left_data, right_data, fs=FS, speaker="FL"):
    """합성 IR 한 쌍으로 HRIR 인스턴스를 구성한다."""
    hrir = HRIR(types.SimpleNamespace(fs=fs))
    hrir.irs[speaker] = {
        "left": ImpulseResponse(np.asarray(left_data, dtype=np.float64), fs),
        "right": ImpulseResponse(np.asarray(right_data, dtype=np.float64), fs),
    }
    return hrir


def reverberant_ir(n=FS, fs=FS, seed=42):
    """임펄스 + 지수감쇠 잔향으로 현실적인 IR을 만든다."""
    rng = np.random.default_rng(seed)
    t = np.arange(n) / fs
    ir = np.zeros(n)
    ir[100] = 1.0
    ir += rng.standard_normal(n) * np.exp(-t / 0.05) * 0.05
    return ir


def column_data(layout):
    """Bokeh 레이아웃에서 모든 ColumnDataSource의 data dict를 수집한다."""
    from bokeh.models import ColumnDataSource

    return [m.data for m in layout.select({"type": ColumnDataSource})]


class TestIACC:
    def test_identical_signals_yield_coefficient_one_at_zero_lag(self):
        """완전 상관 신호쌍의 IACC는 정확히 1.0, 피크는 0ms여야 한다."""
        ir = reverberant_ir()
        hrir = make_hrir(ir, ir.copy())

        layout = hrir.generate_iacc_bokeh_layout()
        sources = column_data(layout)
        assert sources, "IACC 플롯이 생성되지 않았다"

        corr = np.asarray(sources[0]["correlation"])
        lags = np.asarray(sources[0]["lags_ms"])

        # 정규화된 상관 계수는 [-1, 1] 범위여야 한다
        assert np.max(np.abs(corr)) <= 1.0 + 1e-9
        # 완전 상관이므로 최대값은 1.0
        assert np.max(corr) == pytest.approx(1.0, abs=1e-9)
        # 피크 위치는 지연 0ms
        assert lags[np.argmax(corr)] == pytest.approx(0.0, abs=1e-9)

    def test_delayed_attenuated_copy_keeps_full_correlation(self):
        """지연/감쇠는 상관 '구조'를 바꾸지 않으므로 IACC는 여전히 ~1.0.

        우측 = 좌측을 0.5ms 지연 + 0.5배. 레벨 차이는 정규화로 소거되고,
        피크는 -0.5ms(우측이 늦게 도달)에 나타나야 한다.
        """
        ir = reverberant_ir()
        delay = int(0.0005 * FS)  # 24 samples
        right = np.roll(ir, delay) * 0.5
        right[:delay] = 0.0
        hrir = make_hrir(ir, right)

        layout = hrir.generate_iacc_bokeh_layout()
        sources = column_data(layout)

        corr = np.asarray(sources[0]["correlation"])
        lags = np.asarray(sources[0]["lags_ms"])

        assert np.max(np.abs(corr)) <= 1.0 + 1e-9
        assert np.max(corr) == pytest.approx(1.0, abs=1e-3)
        assert lags[np.argmax(corr)] == pytest.approx(-0.5, abs=1e-6)

    def test_uncorrelated_noise_has_low_iacc(self):
        """서로 무관한 노이즈 쌍의 IACC는 1보다 훨씬 작아야 한다."""
        rng = np.random.default_rng(7)
        left = rng.standard_normal(FS)
        right = rng.standard_normal(FS)
        hrir = make_hrir(left, right)

        layout = hrir.generate_iacc_bokeh_layout()
        sources = column_data(layout)

        corr = np.asarray(sources[0]["correlation"])
        assert np.max(np.abs(corr)) < 0.1


class TestIPD:
    def test_pure_delay_gives_band_center_phase(self):
        """순수 지연 Δt의 대역 IPD는 대역 중심 주파수의 위상차여야 한다.

        평탄 스펙트럼(델타 임펄스)에서 대역 [lo, hi]의 에너지 가중 평균
        위상차(원형 평균)는 위상 스팬이 360도 미만인 한 정확히
        360 * (lo+hi)/2 * Δt (wrap) 이다. 진폭 차이는 위상에 영향이 없다.
        """
        delay = 10  # samples -> 10/48000 s = 0.2083ms
        dt = delay / FS
        left = np.zeros(FS // 4)
        right = np.zeros(FS // 4)
        left[1000] = 1.0
        right[1000 + delay] = 0.8  # 감쇠는 IPD에 무관해야 한다
        hrir = make_hrir(left, right)

        layout = hrir.generate_ipd_bokeh_layout()
        sources = column_data(layout)
        assert sources, "IPD 플롯이 생성되지 않았다"
        ipds = np.asarray(sources[0]["ipds"], dtype=np.float64)

        # 기본 옥타브 대역 (fs=48k): 125..16000 중심, 위상 스팬 < 360도인
        # 앞 6개 대역만 닫힌 식으로 검증한다.
        centers = [125, 250, 500, 1000, 2000, 4000]
        for k, center in enumerate(centers):
            lo = center / np.sqrt(2)
            hi = center * np.sqrt(2)
            expected = 360.0 * ((lo + hi) / 2) * dt
            expected = (expected + 180.0) % 360.0 - 180.0
            assert ipds[k] == pytest.approx(expected, abs=2.0), (
                f"band {center}Hz: got {ipds[k]:.1f}, expected {expected:.1f}"
            )

    def test_values_are_wrapped_to_half_circle(self):
        """IPD는 항상 [-180, 180]도 범위로 랩되어야 한다."""
        ir = reverberant_ir()
        delay = int(0.0007 * FS)
        right = np.roll(ir, delay) * 0.7
        right[:delay] = 0.0
        hrir = make_hrir(ir, right)

        layout = hrir.generate_ipd_bokeh_layout()
        sources = column_data(layout)
        ipds = np.asarray(sources[0]["ipds"], dtype=np.float64)
        assert np.all(np.abs(ipds) <= 180.0)


class TestILD:
    def test_level_scaled_copy_gives_uniform_ild(self):
        """우측이 좌측의 0.5배이면 모든 대역 ILD는 +6.02dB여야 한다."""
        ir = reverberant_ir()
        hrir = make_hrir(ir, ir * 0.5)

        layout = hrir.generate_ild_bokeh_layout()
        sources = column_data(layout)
        assert sources, "ILD 플롯이 생성되지 않았다"

        ilds = np.asarray(sources[0]["ilds"], dtype=np.float64)
        expected = 10 * np.log10(4.0)  # +6.02 dB
        assert np.allclose(ilds, expected, atol=0.01)


class TestEnergyDecayCurve:
    def test_curve_is_schroeder_integral(self):
        """플롯되는 값은 0dB에서 시작해 단조감소하는 Schroeder 역적분이다."""
        ir = reverberant_ir()
        hrir = make_hrir(ir, ir * 0.8)

        layout = hrir.generate_etc_bokeh_layout()
        sources = column_data(layout)
        assert sources

        for d in sources:
            curve = np.asarray(d["etc"], dtype=np.float64)
            assert curve[0] == pytest.approx(0.0, abs=0.01)
            assert np.all(np.diff(curve) <= 1e-9)

    def test_labels_declare_energy_decay_not_etc(self):
        """계산이 EDC(Schroeder)이므로 라벨도 순간 에너지(ETC)가 아니라
        Energy Decay Curve를 명시해야 한다."""
        from bokeh.plotting import figure as bokeh_figure

        ir = reverberant_ir()
        hrir = make_hrir(ir, ir * 0.8)

        layout = hrir.generate_etc_bokeh_layout()
        figures = layout.select({"type": bokeh_figure().__class__})
        assert figures
        for fig in figures:
            assert "Energy Decay" in fig.title.text
            assert "decay" in fig.yaxis[0].axis_label.lower()
