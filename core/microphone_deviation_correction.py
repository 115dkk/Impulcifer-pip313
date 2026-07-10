# -*- coding: utf-8 -*-
"""마이크 착용 편차 보정 (v4.0) — 양이(interaural) 마이크 불일치 보정.

음향학적 근거 (코드베이스 + 문헌 검토 기반):

- 인이어/차폐 외이도(blocked-ear) 바이노럴 측정에서 좌우 채널의 "방향 무관"
  크기 차이는 주로 마이크 좌우 불일치(삽입 깊이·착용·감도)에서 비롯된다.
  반면 특정 스피커 방향에서 나타나는 좌우 차이는 실제 ILD(양이 레벨차)이며
  방향·주파수에 따라 비단조적으로 변한다(Cai/Rakerd/Hartmann 2015).
  따라서 "방향 평균(확산음장/CTF)" 또는 "정면(FC, 기대 ILD≈0)"을 기준으로
  방향 무관 성분만 추정해야 한다. 스피커별 기대 ILD 부호표는 부적절하다.

- 개인차 스펙트럼 특징은 ~3.7 kHz 이상 협대역에 몰려 있으므로(Denk 2021;
  Middlebrooks 1999) 6개 옥타브 점이 아니라 풀 FFT 크기를 분수옥타브로
  평활해 추정한다(Tylka/Boren/Choueiri 2017).

- HRTF는 "최소위상 + 단일 지연"으로 잘 근사되고 청자는 저역 ITD만 맞으면
  위상 디테일에 둔감하다(Kistler & Wightman 1992; Kulkarni/Isabelle/Colburn
  1999). 따라서 크기 전용(minimum-phase) 보정을 적용하되 ITD는 건드리지
  않는다. 좌우를 ±Δ/2로 대칭 분할해 모노 합 레벨을 보존한다.

- 깊은 노치를 역으로 부스트하면 고-Q 공진이 생겨 청감상 거슬리므로
  (Bücklein 1981; Gomez Bolaños/Mäkivirta/Pulkki 2016) 분수옥타브 평활 +
  최대 보정량 클램프로 규제화한다.

중요: 같은 마이크를 같은 위치에 둔 채 스피커와 헤드폰을 모두 측정하고
헤드폰 보상을 적용하면 마이크 전달함수가 귀별로 소거된다(Hammershøi &
Møller 2005). 그 경우 이 보정은 잉여이므로 파이프라인에서 헤드폰 보상이
켜져 있으면 건너뛴다(impulcifer.py 참조).
"""

import os
import warnings

import numpy as np
from scipy import signal
from scipy.fft import next_fast_len, rfft, rfftfreq

from autoeq.frequency_response import FrequencyResponse


# 정면(기대 ILD≈0) 앵커로 쓸 수 있는 스피커 이름들
_CENTER_SPEAKERS = ("FC", "TFC", "BC")
# 단일 스피커 보정 시 사용하는 가상 이름
_SINGLE_SPEAKER = "SINGLE"


class MicrophoneMatchingCorrector:
    """방향 무관 양이(interaural) 마이크 불일치 보정기 (v4.0).

    여러 스피커의 직접음 크기응답을 모아, 방향 무관한 좌우 크기 차이 Δ(f)를
    추정한다. Δ는 확산음장(CTF) 파워 평균 또는 정면(FC) 측정으로 구한다.
    Δ를 분수옥타브로 평활하고 대역 제한·클램프한 뒤, 좌우를 ±Δ/2 최소위상
    FIR로 보정한다(ITD 불변).
    """

    def __init__(self, sample_rate,
                 correction_strength=0.7,
                 max_correction_db=6.0,
                 smoothing_octave=1.0 / 6.0,
                 f_min=200.0,
                 f_max=16000.0,
                 window_ms=5.0,
                 pre_ms=0.5,
                 anchor="auto"):
        """
        Args:
            sample_rate (int): 샘플링 레이트 (Hz)
            correction_strength (float): 보정 강도 (0.0~1.0)
            max_correction_db (float): 한쪽 귀 최대 보정량 절댓값 (dB)
            smoothing_octave (float): 분수옥타브 평활 폭 (옥타브 단위, 예: 1/6)
            f_min (float): 보정 하한 주파수 (Hz). 이하로 테이퍼링되어 0이 됨
            f_max (float): 보정 상한 주파수 (Hz). 이상으로 테이퍼링되어 0이 됨
            window_ms (float): 직접음 분석 창 길이 (ms, 피크 이후)
            pre_ms (float): 피크 이전 포함 길이 (ms)
            anchor (str): 'auto' | 'diffuse' | 'frontal'
                - 'auto': 정면(FC) 측정이 있으면 사용, 없으면 확산음장 평균
                - 'diffuse': 항상 확산음장(CTF) 파워 평균
                - 'frontal': 정면(FC) 측정만 사용(없으면 확산음장으로 폴백)
        """
        self.fs = int(sample_rate)
        self.correction_strength = float(np.clip(correction_strength, 0.0, 1.0))
        self.max_correction_db = float(max_correction_db)
        self.smoothing_octave = float(smoothing_octave)
        nyq = self.fs / 2.0
        self.f_min = float(np.clip(f_min, 1.0, nyq * 0.5))
        self.f_max = float(np.clip(f_max, self.f_min * 2.0, nyq * 0.98))
        self.window_ms = float(window_ms)
        self.pre_ms = float(pre_ms)
        self.anchor = anchor

        self.win_samples = max(int(round(self.window_ms * self.fs / 1000.0)), 32)
        self.pre_samples = max(int(round(self.pre_ms * self.fs / 1000.0)), 0)

        # 공통 로그 주파수 그리드 (autoeq와 동일 규약)
        self.frequency = FrequencyResponse.generate_frequencies(
            f_step=1.01, f_min=20.0, f_max=nyq
        )

        # 수집 데이터: speaker -> {'left': power[], 'right': power[]}
        self.speaker_power = {}
        # 추정 결과 (공통 그리드의 dB 곡선)
        self.mismatch_db = None
        self.anchor_used = None

    def _windowed_power(self, ir, peak_index):
        """피크 주변 직접음을 창으로 잘라 공통 그리드의 파워 스펙트럼을 반환."""
        ir = np.asarray(ir, dtype=float)
        n = len(ir)
        if n == 0:
            return np.zeros_like(self.frequency)

        if peak_index is None:
            peak_index = int(np.argmax(np.abs(ir)))
        peak_index = int(np.clip(peak_index, 0, n - 1))

        start = max(peak_index - self.pre_samples, 0)
        end = min(peak_index + self.win_samples, n)
        seg = ir[start:end]
        if len(seg) < 8:
            return np.zeros_like(self.frequency)

        # 양 끝 테이퍼 (Tukey 유사): 앞 pre, 뒤 1/4 페이드
        w = np.ones(len(seg))
        fade_in = min(self.pre_samples, len(seg) // 4)
        if fade_in > 1:
            w[:fade_in] = np.hanning(2 * fade_in)[:fade_in]
        fade_out = max(len(seg) // 4, 1)
        if fade_out > 1:
            w[-fade_out:] = np.hanning(2 * fade_out)[fade_out:]
        seg = seg * w

        nfft = next_fast_len(max(len(seg), 8192))
        spec = rfft(seg, n=nfft)
        fft_freq = rfftfreq(nfft, 1.0 / self.fs)
        mag = np.abs(spec)

        # 공통 로그 그리드로 보간 (선형 주파수 → 로그 그리드)
        mag_grid = np.interp(self.frequency, fft_freq, mag, left=mag[0], right=mag[-1])
        return mag_grid ** 2

    def collect_speaker(self, speaker_name, left_ir, right_ir,
                        left_peak_index=None, right_peak_index=None):
        """단일 스피커의 좌우 직접음 파워 스펙트럼을 수집."""
        left_power = self._windowed_power(left_ir, left_peak_index)
        right_power = self._windowed_power(right_ir, right_peak_index)
        self.speaker_power[speaker_name] = {
            "left": left_power,
            "right": right_power,
        }
        return self.speaker_power[speaker_name]

    # 하위 호환 별칭 (v3.0 명칭)
    def collect_speaker_deviation(self, speaker_name, left_ir, right_ir,
                                  left_peak_index=None, right_peak_index=None):
        self.collect_speaker(speaker_name, left_ir, right_ir,
                             left_peak_index, right_peak_index)
        # v3.0 호환: 대표 대역의 dB 편차 dict 반환
        p = self.speaker_power[speaker_name]
        eps = 1e-20
        delta = 10.0 * np.log10((p["left"] + eps) / (p["right"] + eps))
        bands = [250, 500, 1000, 2000, 4000, 8000]
        out = {}
        for b in bands:
            if b < self.fs / 2:
                out[b] = float(np.interp(b, self.frequency, delta))
        return out

    def _band_weight(self):
        """[f_min, f_max] 안은 1, 밖은 로그축 raised-cosine으로 0이 되는 가중치."""
        f = self.frequency
        w = np.ones_like(f)
        logf = np.log10(f)

        lo2, lo1 = np.log10(self.f_min), np.log10(max(self.f_min / 2.0, 1.0))
        hi1 = np.log10(self.f_max)
        hi2 = np.log10(min(self.f_max * 2.0, self.fs / 2.0 * 0.999))

        # 저역 테이퍼
        low_band = (logf >= lo1) & (logf < lo2)
        w[logf < lo1] = 0.0
        if np.any(low_band):
            x = (logf[low_band] - lo1) / max(lo2 - lo1, 1e-9)
            w[low_band] = 0.5 - 0.5 * np.cos(np.pi * x)

        # 고역 테이퍼
        high_band = (logf > hi1) & (logf <= hi2)
        w[logf > hi2] = 0.0
        if np.any(high_band):
            x = (logf[high_band] - hi1) / max(hi2 - hi1, 1e-9)
            w[high_band] = 0.5 + 0.5 * np.cos(np.pi * x)

        return w

    def estimate_interaural_mismatch(self):
        """방향 무관 양이 크기 불일치 Δ(f) [dB, 양수=왼쪽이 큼]를 추정."""
        if not self.speaker_power:
            warnings.warn("수집된 스피커 데이터가 없습니다. collect_speaker를 먼저 호출하세요.")
            self.mismatch_db = np.zeros_like(self.frequency)
            self.anchor_used = "none"
            return self.mismatch_db

        eps = 1e-20
        anchor = self.anchor
        center = [s for s in self.speaker_power if s in _CENTER_SPEAKERS]

        use_frontal = (anchor == "frontal" and center) or (anchor == "auto" and center)

        if use_frontal:
            # 정면 스피커들의 좌우 파워 평균 → Δ
            left = np.mean([self.speaker_power[s]["left"] for s in center], axis=0)
            right = np.mean([self.speaker_power[s]["right"] for s in center], axis=0)
            self.anchor_used = "frontal"
        else:
            # 확산음장(CTF): 모든 방향의 파워 평균 → 좌우 비교
            left = np.mean([p["left"] for p in self.speaker_power.values()], axis=0)
            right = np.mean([p["right"] for p in self.speaker_power.values()], axis=0)
            self.anchor_used = "diffuse"

        raw_delta = 10.0 * np.log10((left + eps) / (right + eps))

        # 분수옥타브 평활
        fr = FrequencyResponse(name="interaural_mismatch",
                               frequency=self.frequency.copy(),
                               raw=raw_delta)
        try:
            fr.smoothen(window_size=self.smoothing_octave,
                        treble_window_size=self.smoothing_octave)
            smoothed = fr.smoothed if len(fr.smoothed) else raw_delta
        except Exception:
            smoothed = raw_delta

        # 대역 제한 테이퍼 + 클램프
        smoothed = smoothed * self._band_weight()
        smoothed = np.clip(smoothed, -2.0 * self.max_correction_db, 2.0 * self.max_correction_db)

        self.mismatch_db = smoothed
        return self.mismatch_db

    # v3.0 호환 별칭
    def separate_microphone_error(self):
        delta = self.estimate_interaural_mismatch()
        bands = [250, 500, 1000, 2000, 4000, 8000]
        return {b: float(np.interp(b, self.frequency, delta))
                for b in bands if b < self.fs / 2}

    def design_correction_filters(self):
        """좌/우 최소위상 보정 FIR을 생성. (좌 -Δ/2, 우 +Δ/2)"""
        if self.mismatch_db is None:
            self.estimate_interaural_mismatch()

        # 보정 강도 적용 후 한쪽 귀 보정량을 ±max_correction_db로 제한
        delta = self.mismatch_db * self.correction_strength
        half = delta / 2.0
        half = np.clip(half, -self.max_correction_db, self.max_correction_db)

        left_corr = -half
        right_corr = half

        left_fir = self._fir_from_curve(left_corr, "left_mic_correction")
        right_fir = self._fir_from_curve(right_corr, "right_mic_correction")
        return left_fir, right_fir

    def _fir_from_curve(self, curve_db, name):
        fr = FrequencyResponse(name=name, frequency=self.frequency.copy(),
                               raw=curve_db.copy())
        fr.equalization = curve_db.copy()
        try:
            fir = fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
        except Exception as exc:  # pragma: no cover - 방어적
            warnings.warn(f"FIR 생성 실패: {exc}. 단위 임펄스 반환.")
            return np.array([1.0])
        max_len = min(2048, self.fs // 10)
        if len(fir) > max_len:
            fir = fir[:max_len]
        return fir

    def get_analysis_summary(self):
        if self.mismatch_db is None:
            return {"error": "분석 미완료"}
        # 보정 강도가 반영된 실제 한쪽 귀 보정량
        applied = np.clip((self.mismatch_db * self.correction_strength) / 2.0,
                          -self.max_correction_db, self.max_correction_db)
        nz = np.abs(applied[self._band_weight() > 0])
        return {
            "method": "interaural_v4",
            "anchor": self.anchor_used,
            "avg_error_db": float(np.mean(nz)) if len(nz) else 0.0,
            "max_error_db": float(np.max(nz)) if len(nz) else 0.0,
            "speakers_analyzed": list(self.speaker_power.keys()),
            "correction_strength": self.correction_strength,
        }


class MicrophoneDeviationCorrector(MicrophoneMatchingCorrector):
    """기존 임포트/호출 호환용 래퍼.

    v2.0/v3.0의 사용되지 않는 파라미터(octave_bands, gate_cycles,
    enable_* 등)는 무시된다.
    """

    def __init__(self, sample_rate,
                 correction_strength=0.7,
                 max_correction_db=6.0,
                 smoothing_octave=1.0 / 6.0,
                 f_min=200.0,
                 f_max=16000.0,
                 window_ms=5.0,
                 anchor="auto",
                 **legacy_kwargs):
        super().__init__(
            sample_rate=sample_rate,
            correction_strength=correction_strength,
            max_correction_db=max_correction_db,
            smoothing_octave=smoothing_octave,
            f_min=f_min,
            f_max=f_max,
            window_ms=window_ms,
            anchor=anchor,
        )
        if legacy_kwargs.get("enable_phase_correction"):
            warnings.warn(
                "enable_phase_correction은 제거되었습니다. v4.0은 크기 전용 "
                "최소위상 보정만 수행하며 ITD는 보존합니다.",
                DeprecationWarning,
            )

    def correct_microphone_deviation(self, left_ir, right_ir,
                                     left_peak_index=None, right_peak_index=None,
                                     plot_analysis=False, plot_dir=None):
        """단일 스피커 쌍 보정 (진단/호환용). 길이를 보존해 반환한다."""
        left_ir = np.asarray(left_ir, dtype=float)
        right_ir = np.asarray(right_ir, dtype=float)
        if len(left_ir) != len(right_ir):
            min_len = min(len(left_ir), len(right_ir))
            left_ir = left_ir[:min_len]
            right_ir = right_ir[:min_len]

        self.speaker_power.clear()
        self.collect_speaker(_SINGLE_SPEAKER, left_ir, right_ir,
                             left_peak_index, right_peak_index)
        # 단일 쌍은 확산음장과 동일(스피커 1개) → 그대로 Δ 추정
        self.anchor = "diffuse"
        self.estimate_interaural_mismatch()

        applied = np.clip((self.mismatch_db * self.correction_strength) / 2.0,
                          -self.max_correction_db, self.max_correction_db)
        significant = float(np.max(np.abs(applied))) if len(applied) else 0.0

        analysis = {
            "method": "interaural_v4",
            "anchor": self.anchor_used,
            "mismatch_db": self.mismatch_db,
            "frequency": self.frequency,
        }

        if significant < 0.05:
            analysis["correction_applied"] = False
            return left_ir.copy(), right_ir.copy(), analysis

        left_fir, right_fir = self.design_correction_filters()
        try:
            corrected_left = signal.convolve(left_ir, left_fir, mode="same")
            corrected_right = signal.convolve(right_ir, right_fir, mode="same")
        except Exception as exc:
            warnings.warn(f"보정 적용 실패: {exc}. 원본 반환.")
            corrected_left, corrected_right = left_ir.copy(), right_ir.copy()

        analysis.update({
            "correction_applied": True,
            "correction_filters": {"left_fir": left_fir, "right_fir": right_fir},
            "avg_error_db": self.get_analysis_summary()["avg_error_db"],
            "max_error_db": self.get_analysis_summary()["max_error_db"],
        })

        if plot_analysis and plot_dir:
            try:
                _plot_single_pair(self, left_ir, right_ir,
                                  corrected_left, corrected_right, plot_dir)
            except Exception as exc:  # pragma: no cover
                warnings.warn(f"플롯 생성 실패: {exc}")

        return corrected_left, corrected_right, analysis


def apply_microphone_deviation_correction_to_hrir(hrir,
                                                  correction_strength=0.7,
                                                  anchor="auto",
                                                  plot_analysis=False,
                                                  plot_dir=None):
    """HRIR 객체에 방향 무관 양이 마이크 불일치 보정 적용 (v4.0).

    모든 스피커의 직접음을 모아 방향 무관 좌우 크기 차이를 추정하고,
    좌우를 ±Δ/2 최소위상 FIR로 보정한다. ITD는 보존된다.

    Returns:
        dict: 분석 요약
    """
    corrector = MicrophoneMatchingCorrector(
        sample_rate=hrir.fs,
        correction_strength=correction_strength,
        anchor=anchor,
    )

    print("\n🎧 마이크 편차 보정 v4.0 시작 (방향 무관 양이 불일치)")
    print(f"  - 보정 강도: {correction_strength}")
    print(f"  - 분석 대상 스피커: {len(hrir.irs)}개")

    speaker_data = {}
    for speaker, pair in hrir.irs.items():
        left_ir = pair["left"]
        right_ir = pair["right"]
        left_peak = left_ir.peak_index()
        right_peak = right_ir.peak_index()
        if left_peak is None or right_peak is None:
            print(f"  ⚠️ {speaker}: 피크를 찾을 수 없어 건너뜁니다.")
            continue
        corrector.collect_speaker(speaker, left_ir.data, right_ir.data,
                                  left_peak, right_peak)
        speaker_data[speaker] = {"left_ir": left_ir, "right_ir": right_ir}

    if not speaker_data:
        print("  ⚠️ 보정에 사용할 스피커가 없습니다.")
        return {"error": "스피커 데이터 없음"}

    corrector.estimate_interaural_mismatch()
    summary = corrector.get_analysis_summary()
    print(f"  - 추정 기준(anchor): {summary['anchor']}")
    print(f"  - 평균 보정량: {summary['avg_error_db']:.2f} dB, "
          f"최대 보정량: {summary['max_error_db']:.2f} dB")

    if summary["max_error_db"] < 0.05:
        print("  ℹ️ 유의미한 좌우 불일치가 없어 보정을 건너뜁니다.")
        summary["speakers_processed"] = []
        return summary

    left_fir, right_fir = corrector.design_correction_filters()
    for speaker, data in speaker_data.items():
        try:
            data["left_ir"].equalize(left_fir)
            data["right_ir"].equalize(right_fir)
        except Exception as exc:
            print(f"  ⚠️ {speaker}: 보정 적용 실패 ({exc})")

    if plot_analysis and plot_dir:
        try:
            _plot_mismatch(corrector, plot_dir)
        except Exception as exc:  # pragma: no cover
            warnings.warn(f"플롯 생성 실패: {exc}")

    summary["speakers_processed"] = list(speaker_data.keys())
    print("✅ 마이크 편차 보정 v4.0 완료")
    return summary


def _plot_mismatch(corrector, plot_dir):
    import matplotlib.pyplot as plt
    from core.utils import set_matplotlib_font

    set_matplotlib_font()
    os.makedirs(plot_dir, exist_ok=True)

    fig, ax = plt.subplots(figsize=(12, 6))
    ax.semilogx(corrector.frequency, corrector.mismatch_db, "k-", linewidth=2,
                label="추정 좌우 불일치 Δ (양수=왼쪽이 큼)")
    applied = np.clip((corrector.mismatch_db * corrector.correction_strength) / 2.0,
                      -corrector.max_correction_db, corrector.max_correction_db)
    ax.semilogx(corrector.frequency, applied, "C0--", label="좌측 보정량 (-Δ/2)")
    ax.semilogx(corrector.frequency, -applied, "C3--", label="우측 보정량 (+Δ/2)")
    ax.axhline(y=0, color="gray", linestyle=":", alpha=0.5)
    ax.axvline(x=corrector.f_min, color="gray", linestyle=":", alpha=0.3)
    ax.axvline(x=corrector.f_max, color="gray", linestyle=":", alpha=0.3)
    ax.set_xlim([20, corrector.fs / 2])
    ax.set_xlabel("주파수 (Hz)")
    ax.set_ylabel("크기 (dB)")
    ax.set_title(f"마이크 편차 보정 v4.0 (anchor={corrector.anchor_used})")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=9)
    plt.tight_layout()
    plt.savefig(os.path.join(plot_dir, "microphone_deviation_v4.png"),
                dpi=150, bbox_inches="tight")
    plt.close()
    print(f"✅ 마이크 편차 보정 플롯이 {plot_dir}에 저장되었습니다.")


def _plot_single_pair(corrector, original_left, original_right,
                      corrected_left, corrected_right, plot_dir):
    import matplotlib.pyplot as plt
    from core.utils import set_matplotlib_font

    set_matplotlib_font()
    os.makedirs(plot_dir, exist_ok=True)
    fig, ax = plt.subplots(figsize=(12, 6))
    ax.semilogx(corrector.frequency, corrector.mismatch_db, "k-",
                label="추정 좌우 불일치 Δ")
    ax.axhline(y=0, color="gray", linestyle=":", alpha=0.5)
    ax.set_xlim([20, corrector.fs / 2])
    ax.set_xlabel("주파수 (Hz)")
    ax.set_ylabel("좌우 차이 (dB)")
    ax.set_title("마이크 편차 보정 v4.0 (단일 스피커)")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=9)
    plt.tight_layout()
    plt.savefig(os.path.join(plot_dir, "microphone_deviation_v4_single.png"),
                dpi=150, bbox_inches="tight")
    plt.close()
