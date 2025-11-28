# -*- coding: utf-8 -*-
"""
교차검증 기반 마이크 착용 편차 보정 (v3.0)

핵심 원리:
- 마이크 오차: 모든 스피커 방향에서 일관되게 나타나는 좌우 차이
- HRTF 비대칭: 스피커 방향에 따라 체계적으로 변하는 좌우 차이

이 두 성분을 통계적으로 분리하여 마이크 오차만 보정합니다.

v3.0 변경사항:
- 교차검증 로직 도입: 모든 스피커 데이터를 종합하여 마이크 오차 추정
- 위상 보정 제거: minimum_phase + 위상 보정의 구조적 모순 해결
- 해부학적 선험 지식 활용: 스피커 방향별 기대 ILD 부호 사용
- 일관성 검증: 추정된 마이크 오차의 물리적 타당성 검증
"""

import os
import numpy as np
import matplotlib.pyplot as plt
from scipy import signal
from scipy.fft import fft, fftfreq
from scipy.interpolate import interp1d
from autoeq.frequency_response import FrequencyResponse
import warnings


class CrossValidatedMicrophoneCorrector:
    """
    다중 스피커 교차검증 기반 마이크 편차 보정 (v3.0)

    핵심 원리:
    - 마이크 오차: 모든 스피커 방향에서 일관되게 나타나는 좌우 차이
    - HRTF 비대칭: 스피커 방향에 따라 체계적으로 변하는 좌우 차이

    이 두 성분을 통계적으로 분리하여 마이크 오차만 보정합니다.
    """

    def __init__(self, sample_rate,
                 correction_strength=0.7,
                 octave_bands=None,
                 max_correction_db=6.0,
                 min_gate_cycles=2,
                 max_gate_cycles=8):
        """
        Args:
            sample_rate (int): 샘플링 레이트 (Hz)
            correction_strength (float): 보정 강도 (0.0~1.0)
            octave_bands (list): 분석할 옥타브 밴드 중심 주파수들 (Hz)
            max_correction_db (float): 최대 보정량 (dB)
            min_gate_cycles (float): 최소 게이트 길이 (사이클 수)
            max_gate_cycles (float): 최대 게이트 길이 (사이클 수)
        """
        self.fs = sample_rate
        self.correction_strength = np.clip(correction_strength, 0.0, 1.0)
        self.max_correction_db = max_correction_db
        self.min_gate_cycles = min_gate_cycles
        self.max_gate_cycles = max_gate_cycles

        # 기본 옥타브 밴드 설정 (250Hz ~ 8kHz - 마이크 편차가 주로 나타나는 대역)
        if octave_bands is None:
            self.octave_bands = [250, 500, 1000, 2000, 4000, 8000]
        else:
            self.octave_bands = octave_bands

        # 나이퀴스트 주파수 이하로 제한
        self.octave_bands = [f for f in self.octave_bands if f < self.fs / 2]

        # 스피커별 기대 ILD 부호 (왼쪽 스피커는 양수, 오른쪽은 음수)
        # 양수: 왼쪽 귀가 더 큰 신호를 받음 (정상)
        # 음수: 오른쪽 귀가 더 큰 신호를 받음 (정상)
        # 0: 좌우 대칭에 가까움
        self.expected_ild_sign = {
            # 기본 스피커
            'FL': +1.0, 'FC': 0.0, 'FR': -1.0,
            'SL': +1.0, 'SR': -1.0,
            'BL': +0.8, 'BC': 0.0, 'BR': -0.8,
            # 천장 스피커 (좌우 대칭에 가까움)
            'TFL': +0.5, 'TFC': 0.0, 'TFR': -0.5,
            'TBL': +0.5, 'TBC': 0.0, 'TBR': -0.5,
            'TSL': +0.5, 'TSR': -0.5,
            # 서브우퍼 (좌우 무관)
            'LFE': 0.0, 'SW': 0.0,
        }

        # 각 밴드별 게이트 길이 계산
        self._calculate_gate_lengths()

        # 수집된 편차 데이터 저장
        self.all_speaker_deviations = {}
        self.mic_error_estimate = {}
        self.validation_result = {}

    def _calculate_gate_lengths(self):
        """각 주파수 밴드별 최적 게이트 길이 계산"""
        self.gate_lengths = {}

        for center_freq in self.octave_bands:
            # 주파수가 높을수록 짧은 게이트 사용
            if len(self.octave_bands) > 1:
                log_freq_ratio = np.log10(center_freq / self.octave_bands[0]) / \
                                np.log10(self.octave_bands[-1] / self.octave_bands[0])
            else:
                log_freq_ratio = 0.5
            cycles = self.max_gate_cycles - (self.max_gate_cycles - self.min_gate_cycles) * log_freq_ratio

            # 사이클 수를 샘플 수로 변환
            samples_per_cycle = self.fs / center_freq
            gate_samples = int(cycles * samples_per_cycle)

            # 최소 16샘플, 최대 fs/10 샘플로 제한
            gate_samples = np.clip(gate_samples, 16, self.fs // 10)

            self.gate_lengths[center_freq] = gate_samples

    def _apply_frequency_gate(self, ir_data, center_freq, peak_index):
        """특정 주파수 밴드에 대해 시간 게이팅 적용"""
        gate_length = self.gate_lengths[center_freq]

        start_idx = peak_index
        end_idx = min(start_idx + gate_length, len(ir_data))

        if end_idx <= start_idx:
            return np.zeros(gate_length)

        gated_segment = ir_data[start_idx:end_idx]

        if len(gated_segment) < gate_length:
            gated_segment = np.pad(gated_segment, (0, gate_length - len(gated_segment)), 'constant')

        # 테이퍼 윈도우 적용
        window = np.ones(gate_length)
        fade_length = min(gate_length // 4, 32)
        if fade_length > 0:
            window[-fade_length:] = np.linspace(1, 0, fade_length)

        return gated_segment * window

    def _measure_band_level(self, ir_data, center_freq, peak_index):
        """특정 주파수 밴드의 레벨(dB) 측정"""
        # 밴드패스 필터 설계 (1/3 옥타브)
        lower_freq = center_freq / (2**(1/6))
        upper_freq = center_freq * (2**(1/6))
        upper_freq = min(upper_freq, self.fs / 2 * 0.95)

        if lower_freq >= upper_freq:
            return -100.0  # 유효하지 않은 대역

        try:
            sos = signal.butter(4, [lower_freq, upper_freq], btype='band', fs=self.fs, output='sos')
            filtered_ir = signal.sosfilt(sos, ir_data)
        except ValueError:
            filtered_ir = ir_data

        # 게이팅 적용
        gated_ir = self._apply_frequency_gate(filtered_ir, center_freq, peak_index)

        # FFT로 레벨 계산
        fft_length = max(len(gated_ir) * 2, 512)
        fft_result = fft(gated_ir, n=fft_length)
        freqs = fftfreq(fft_length, 1/self.fs)

        # 중심 주파수에 가장 가까운 빈 찾기
        center_bin = np.argmin(np.abs(freqs - center_freq))
        magnitude = np.abs(fft_result[center_bin])

        if magnitude > 0:
            return 20 * np.log10(magnitude)
        else:
            return -100.0

    def collect_speaker_deviation(self, speaker_name, left_ir, right_ir,
                                  left_peak_index=None, right_peak_index=None):
        """
        단일 스피커의 좌우 편차를 수집

        Args:
            speaker_name (str): 스피커 이름 (예: 'FL', 'FR', 'FC')
            left_ir (np.array): 좌측 귀 임펄스 응답
            right_ir (np.array): 우측 귀 임펄스 응답
            left_peak_index (int): 좌측 피크 인덱스
            right_peak_index (int): 우측 피크 인덱스
        """
        if left_peak_index is None:
            left_peak_index = np.argmax(np.abs(left_ir))
        if right_peak_index is None:
            right_peak_index = np.argmax(np.abs(right_ir))

        speaker_deviations = {}

        for freq in self.octave_bands:
            left_level = self._measure_band_level(left_ir, freq, left_peak_index)
            right_level = self._measure_band_level(right_ir, freq, right_peak_index)

            # 편차: 양수면 왼쪽이 더 큼
            deviation_db = left_level - right_level
            speaker_deviations[freq] = deviation_db

        self.all_speaker_deviations[speaker_name] = speaker_deviations
        return speaker_deviations

    def separate_microphone_error(self):
        """
        마이크 오차와 HRTF 비대칭을 분리

        마이크 오차 추정: 기대 ILD 부호를 고려한 분석
        - FL에서 +3dB, FR에서 +1dB가 나왔다면,
          FL은 원래 양수가 기대되므로 일부는 HRTF
          FR은 원래 음수가 기대되므로 +1dB 전체가 이상함
        - 이런 "기대와 반대 방향" 편차들의 평균이 마이크 오차

        Returns:
            dict: 주파수별 추정 마이크 오차 (dB)
        """
        if not self.all_speaker_deviations:
            warnings.warn("수집된 스피커 데이터가 없습니다. 먼저 collect_speaker_deviation을 호출하세요.")
            return {}

        mic_error_estimate = {}

        for freq in self.octave_bands:
            # 기대 방향과 반대되는 편차들 수집
            anomalous_deviations = []
            neutral_deviations = []  # 중앙 스피커 편차

            for speaker, deviations in self.all_speaker_deviations.items():
                if freq not in deviations:
                    continue

                deviation = deviations[freq]
                expected_sign = self.expected_ild_sign.get(speaker, 0)

                if expected_sign > 0.5 and deviation < 0:
                    # 왼쪽 스피커인데 오른쪽이 더 큼 -> 이상
                    anomalous_deviations.append(deviation)
                elif expected_sign < -0.5 and deviation > 0:
                    # 오른쪽 스피커인데 왼쪽이 더 큼 -> 이상
                    anomalous_deviations.append(deviation)
                elif abs(expected_sign) <= 0.5:
                    # 중앙/천장 스피커는 원래 0에 가까워야 함
                    neutral_deviations.append(deviation)

            # 마이크 오차 추정
            if anomalous_deviations:
                # 이상 편차들의 중앙값 = 마이크 오차 추정
                mic_error_estimate[freq] = np.median(anomalous_deviations)
            elif neutral_deviations:
                # 중앙 스피커 편차의 중앙값 사용
                mic_error_estimate[freq] = np.median(neutral_deviations)
            else:
                # 모든 편차가 기대 방향이면 전체 중앙값의 일부를 마이크 오차로 추정
                all_devs = [d[freq] for d in self.all_speaker_deviations.values() if freq in d]
                if all_devs:
                    # 전체 중앙값의 30%만 마이크 오차로 간주 (보수적 추정)
                    mic_error_estimate[freq] = np.median(all_devs) * 0.3
                else:
                    mic_error_estimate[freq] = 0.0

        self.mic_error_estimate = mic_error_estimate
        return mic_error_estimate

    def validate_consistency(self):
        """
        추정된 마이크 오차의 일관성 검증

        마이크 오차를 빼고 나면 남은 편차가 물리적으로 타당해야 함:
        - FL, SL, BL에서는 양수 (왼쪽 귀가 가까움)
        - FR, SR, BR에서는 음수 (오른쪽 귀가 가까움)
        - FC에서는 0에 가까움

        Returns:
            dict: 검증 결과
        """
        if not self.mic_error_estimate or not self.all_speaker_deviations:
            return {'valid': False, 'reason': '데이터 부족'}

        validation_scores = []
        details = []

        for freq in self.octave_bands:
            if freq not in self.mic_error_estimate:
                continue

            mic_error = self.mic_error_estimate[freq]

            for speaker, deviations in self.all_speaker_deviations.items():
                if freq not in deviations:
                    continue

                raw_deviation = deviations[freq]
                corrected_deviation = raw_deviation - mic_error
                expected_sign = self.expected_ild_sign.get(speaker, 0)

                # 보정 후 편차가 기대 방향과 일치하는지 확인
                if abs(expected_sign) > 0.3:
                    # 부호 일치: +1, 불일치: -1
                    if corrected_deviation * expected_sign > 0:
                        sign_match = 1.0
                    elif abs(corrected_deviation) < 1.0:  # 1dB 미만은 중립
                        sign_match = 0.5
                    else:
                        sign_match = 0.0

                    validation_scores.append(sign_match)
                    details.append({
                        'speaker': speaker,
                        'freq': freq,
                        'raw': raw_deviation,
                        'corrected': corrected_deviation,
                        'expected_sign': expected_sign,
                        'match': sign_match
                    })

        # 평균 점수 계산
        if validation_scores:
            consistency = np.mean(validation_scores)
        else:
            consistency = 0.5  # 데이터 부족 시 중립

        self.validation_result = {
            'consistency_score': consistency,
            'is_valid': consistency > 0.4,  # 40% 이상 일치하면 유효
            'confidence': 'high' if consistency > 0.7 else 'medium' if consistency > 0.5 else 'low',
            'details': details
        }

        return self.validation_result

    def design_correction_filters(self):
        """
        마이크 오차 보정 필터 설계

        크기 보정만 수행, 위상은 건드리지 않음
        (위상 보정 + minimum_phase 모순 회피)

        Returns:
            tuple: (left_fir, right_fir) 보정 필터들
        """
        if not self.mic_error_estimate:
            warnings.warn("마이크 오차 추정이 필요합니다. 먼저 separate_microphone_error를 호출하세요.")
            return np.array([1.0]), np.array([1.0])

        frequencies = FrequencyResponse.generate_frequencies(
            f_step=1.01, f_min=20, f_max=self.fs/2
        )

        # 옥타브 밴드의 마이크 오차를 연속 곡선으로 보간
        band_freqs = np.array(sorted(self.mic_error_estimate.keys()))
        band_errors = np.array([self.mic_error_estimate[f] for f in band_freqs])

        if len(band_freqs) < 2:
            # 데이터 부족 시 단일 값으로 보정
            correction_curve = np.full(len(frequencies), band_errors[0] if len(band_errors) > 0 else 0.0)
        else:
            # 로그 주파수 공간에서 선형 보간
            interpolator = interp1d(
                np.log10(band_freqs),
                band_errors,
                kind='linear',
                bounds_error=False,
                fill_value=(band_errors[0], band_errors[-1])
            )
            correction_curve = interpolator(np.log10(frequencies))

        # 보정 강도 적용
        correction_curve *= self.correction_strength

        # 최대 보정량 제한
        correction_curve = np.clip(correction_curve, -self.max_correction_db, self.max_correction_db)

        # FrequencyResponse 객체로 FIR 생성
        # 왼쪽에는 -correction/2, 오른쪽에는 +correction/2 적용
        # (총 correction만큼 상대적 차이 보정)
        left_fr = FrequencyResponse(
            name='left_mic_correction',
            frequency=frequencies.copy(),
            raw=-correction_curve / 2
        )
        right_fr = FrequencyResponse(
            name='right_mic_correction',
            frequency=frequencies.copy(),
            raw=correction_curve / 2
        )

        # 최소 위상 FIR 생성 (크기만 보정하므로 minimum_phase 사용이 적절함)
        try:
            left_fir = left_fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            right_fir = right_fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)

            # FIR 길이 제한
            max_fir_length = min(1024, self.fs // 10)
            if len(left_fir) > max_fir_length:
                left_fir = left_fir[:max_fir_length]
            if len(right_fir) > max_fir_length:
                right_fir = right_fir[:max_fir_length]

        except Exception as e:
            warnings.warn(f"FIR 필터 생성 실패: {e}. 단위 임펄스 반환.")
            left_fir = np.array([1.0])
            right_fir = np.array([1.0])

        return left_fir, right_fir

    def get_analysis_summary(self):
        """분석 결과 요약 반환"""
        if not self.mic_error_estimate:
            return {'error': '분석 미완료'}

        # 평균/최대 마이크 오차
        errors = list(self.mic_error_estimate.values())
        avg_error = np.mean(np.abs(errors)) if errors else 0.0
        max_error = np.max(np.abs(errors)) if errors else 0.0

        return {
            'mic_error_estimate': self.mic_error_estimate.copy(),
            'avg_error_db': avg_error,
            'max_error_db': max_error,
            'speakers_analyzed': list(self.all_speaker_deviations.keys()),
            'validation': self.validation_result.copy() if self.validation_result else {},
            'correction_strength': self.correction_strength
        }


# 기존 API 호환성을 위한 래퍼 클래스
class MicrophoneDeviationCorrector(CrossValidatedMicrophoneCorrector):
    """
    기존 API 호환성을 위한 래퍼 클래스

    v2.0 API를 v3.0 교차검증 기반 구현으로 매핑합니다.
    enable_phase_correction, enable_adaptive_correction 등의 파라미터는
    더 이상 사용되지 않으며 무시됩니다.
    """

    def __init__(self, sample_rate,
                 octave_bands=None,
                 min_gate_cycles=2,
                 max_gate_cycles=8,
                 correction_strength=0.7,
                 smoothing_window=1/3,
                 max_correction_db=6.0,
                 enable_phase_correction=True,  # 무시됨 (v3.0에서 제거)
                 enable_adaptive_correction=True,  # 무시됨 (v3.0에서 제거)
                 enable_anatomical_validation=True,  # 무시됨 (v3.0에서 통합)
                 itd_range_ms=(-0.7, 0.7),  # 무시됨
                 head_radius_cm=8.75):  # 무시됨
        """
        Args:
            sample_rate (int): 샘플링 레이트 (Hz)
            octave_bands (list): 분석할 옥타브 밴드 중심 주파수들 (Hz)
            min_gate_cycles (float): 최소 게이트 길이 (사이클 수)
            max_gate_cycles (float): 최대 게이트 길이 (사이클 수)
            correction_strength (float): 보정 강도 (0.0~1.0)
            smoothing_window (float): 사용되지 않음 (v3.0에서 제거)
            max_correction_db (float): 최대 보정량 (dB)
            enable_phase_correction (bool): 사용되지 않음 (v3.0에서 제거됨)
            enable_adaptive_correction (bool): 사용되지 않음 (v3.0에서 제거됨)
            enable_anatomical_validation (bool): 사용되지 않음 (v3.0에서 통합)
            itd_range_ms (tuple): 사용되지 않음
            head_radius_cm (float): 사용되지 않음
        """
        # v3.0 부모 클래스 초기화
        super().__init__(
            sample_rate=sample_rate,
            correction_strength=correction_strength,
            octave_bands=octave_bands,
            max_correction_db=max_correction_db,
            min_gate_cycles=min_gate_cycles,
            max_gate_cycles=max_gate_cycles
        )

        # 사용되지 않는 파라미터 경고
        if enable_phase_correction:
            warnings.warn(
                "enable_phase_correction은 v3.0에서 제거되었습니다. "
                "위상 보정은 minimum_phase와 구조적으로 모순되어 "
                "크기 보정만 수행됩니다.",
                DeprecationWarning
            )

    def correct_microphone_deviation(self, left_ir, right_ir,
                                     left_peak_index=None, right_peak_index=None,
                                     plot_analysis=False, plot_dir=None):
        """
        단일 스피커에 대한 마이크 착용 편차 보정 (기존 API 호환)

        주의: 이 메서드는 단일 스피커만 분석하므로 교차검증이 제한됩니다.
        더 정확한 보정을 위해서는 apply_microphone_deviation_correction_to_hrir를
        사용하세요.
        """
        # 입력 검증
        if len(left_ir) != len(right_ir):
            min_len = min(len(left_ir), len(right_ir))
            left_ir = left_ir[:min_len]
            right_ir = right_ir[:min_len]

        if left_peak_index is None:
            left_peak_index = np.argmax(np.abs(left_ir))
        if right_peak_index is None:
            right_peak_index = np.argmax(np.abs(right_ir))

        # 단일 스피커 데이터 수집 (교차검증 없이)
        self.collect_speaker_deviation('SINGLE', left_ir, right_ir,
                                       left_peak_index, right_peak_index)

        # 마이크 오차 추정 (단일 스피커의 경우 그대로 사용)
        # 교차검증 없이 단순 편차를 마이크 오차로 간주
        self.mic_error_estimate = self.all_speaker_deviations.get('SINGLE', {})

        # 유의미한 편차 확인
        significant_deviations = [abs(d) for d in self.mic_error_estimate.values() if abs(d) > 0.5]

        if not significant_deviations:
            print("유의미한 마이크 편차가 감지되지 않았습니다. 보정을 건너뜁니다.")
            analysis_results = {
                'deviation_results': {'frequency_deviations': self.mic_error_estimate},
                'correction_filters': {'left_fir': np.array([1.0]), 'right_fir': np.array([1.0])},
                'correction_applied': False,
                'v3_cross_validation': False
            }
            return left_ir.copy(), right_ir.copy(), analysis_results

        # 보정 필터 생성
        left_fir, right_fir = self.design_correction_filters()

        # 보정 적용
        try:
            if len(left_fir) > 1 and len(right_fir) > 1:
                corrected_left = signal.convolve(left_ir, left_fir, mode='same')
                corrected_right = signal.convolve(right_ir, right_fir, mode='same')
            else:
                corrected_left = left_ir.copy()
                corrected_right = right_ir.copy()
        except Exception as e:
            print(f"보정 필터 적용 실패: {e}. 원본 반환.")
            corrected_left = left_ir.copy()
            corrected_right = right_ir.copy()

        analysis_results = {
            'deviation_results': {'frequency_deviations': self.mic_error_estimate},
            'correction_filters': {'left_fir': left_fir, 'right_fir': right_fir},
            'correction_applied': True,
            'avg_deviation_db': np.mean(significant_deviations),
            'max_deviation_db': np.max(significant_deviations),
            'v3_cross_validation': False
        }

        if plot_analysis and plot_dir:
            self._plot_analysis_results(left_ir, right_ir, corrected_left, corrected_right,
                                        analysis_results, plot_dir)

        return corrected_left, corrected_right, analysis_results

    def _plot_analysis_results(self, original_left, original_right,
                               corrected_left, corrected_right,
                               analysis_results, plot_dir):
        """분석 결과 플롯 생성"""
        os.makedirs(plot_dir, exist_ok=True)

        # 1. 편차 분석 결과 플롯
        fig, ax = plt.subplots(figsize=(12, 6))

        deviations = analysis_results['deviation_results']['frequency_deviations']
        freqs = sorted(deviations.keys())
        values = [deviations[f] for f in freqs]

        ax.semilogx(freqs, values, 'o-', linewidth=2, markersize=8, label='측정된 편차 (L-R)')
        ax.axhline(y=0, color='gray', linestyle='--', alpha=0.5)
        ax.set_xlabel('주파수 (Hz)', fontsize=11)
        ax.set_ylabel('편차 (dB)', fontsize=11)
        ax.set_title('마이크 착용 편차 분석 (v3.0 - 크기만 보정)', fontsize=13)
        ax.grid(True, alpha=0.3)
        ax.legend(fontsize=10)

        plt.tight_layout()
        plt.savefig(os.path.join(plot_dir, 'microphone_deviation_analysis_v3.png'),
                    dpi=150, bbox_inches='tight')
        plt.close()

        # 2. 보정 전후 비교 플롯
        fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(14, 8))

        fft_len = max(len(original_left) * 2, 8192)
        freqs_fft = np.fft.fftfreq(fft_len, 1/self.fs)[:fft_len//2]

        orig_left_fft = np.fft.fft(original_left, n=fft_len)[:fft_len//2]
        orig_right_fft = np.fft.fft(original_right, n=fft_len)[:fft_len//2]
        corr_left_fft = np.fft.fft(corrected_left, n=fft_len)[:fft_len//2]
        corr_right_fft = np.fft.fft(corrected_right, n=fft_len)[:fft_len//2]

        orig_left_db = 20 * np.log10(np.abs(orig_left_fft) + 1e-12)
        orig_right_db = 20 * np.log10(np.abs(orig_right_fft) + 1e-12)
        corr_left_db = 20 * np.log10(np.abs(corr_left_fft) + 1e-12)
        corr_right_db = 20 * np.log10(np.abs(corr_right_fft) + 1e-12)

        ax1.semilogx(freqs_fft, orig_left_db, alpha=0.6, label='원본 좌측', color='blue')
        ax1.semilogx(freqs_fft, orig_right_db, alpha=0.6, label='원본 우측', color='red')
        ax1.semilogx(freqs_fft, corr_left_db, '--', label='보정 좌측', color='darkblue')
        ax1.semilogx(freqs_fft, corr_right_db, '--', label='보정 우측', color='darkred')
        ax1.set_ylabel('크기 (dB)', fontsize=11)
        ax1.set_title('마이크 편차 보정 전후 비교 (v3.0)', fontsize=13)
        ax1.set_xlim([20, self.fs/2])
        ax1.grid(True, alpha=0.3)
        ax1.legend(fontsize=10)

        orig_diff = orig_left_db - orig_right_db
        corr_diff = corr_left_db - corr_right_db

        ax2.semilogx(freqs_fft, orig_diff, alpha=0.7, label='원본 L-R 차이', color='purple')
        ax2.semilogx(freqs_fft, corr_diff, '--', label='보정 후 L-R 차이', color='green')
        ax2.axhline(y=0, color='gray', linestyle='--', alpha=0.5)
        ax2.set_xlabel('주파수 (Hz)', fontsize=11)
        ax2.set_ylabel('좌우 차이 (dB)', fontsize=11)
        ax2.set_xlim([20, self.fs/2])
        ax2.grid(True, alpha=0.3)
        ax2.legend(fontsize=10)

        plt.tight_layout()
        plt.savefig(os.path.join(plot_dir, 'microphone_deviation_correction_comparison_v2.png'),
                    dpi=150, bbox_inches='tight')
        plt.close()

        print(f"✅ 마이크 편차 보정 분석 플롯이 {plot_dir}에 저장되었습니다.")


def apply_microphone_deviation_correction_to_hrir(hrir,
                                                  correction_strength=0.7,
                                                  enable_phase_correction=True,  # 무시됨
                                                  enable_adaptive_correction=True,  # 무시됨
                                                  enable_anatomical_validation=True,  # 무시됨
                                                  plot_analysis=False,
                                                  plot_dir=None):
    """
    HRIR 객체에 교차검증 기반 마이크 착용 편차 보정 적용 (v3.0)

    이 함수는 모든 스피커의 데이터를 먼저 수집한 후,
    교차검증을 통해 마이크 오차와 HRTF 비대칭을 분리합니다.

    Args:
        hrir (HRIR): HRIR 객체
        correction_strength (float): 보정 강도 (0.0~1.0)
        enable_phase_correction (bool): 무시됨 (v3.0에서 제거)
        enable_adaptive_correction (bool): 무시됨 (v3.0에서 제거)
        enable_anatomical_validation (bool): 무시됨 (v3.0에서 통합)
        plot_analysis (bool): 분석 결과 플롯 생성 여부
        plot_dir (str): 플롯 저장 디렉토리

    Returns:
        dict: 분석 결과
    """
    corrector = CrossValidatedMicrophoneCorrector(
        sample_rate=hrir.fs,
        correction_strength=correction_strength
    )

    print("\n🎧 마이크 편차 보정 v3.0 시작 (교차검증 기반)")
    print(f"  - 보정 강도: {correction_strength}")
    print(f"  - 분석 대상 스피커: {len(hrir.irs)}개")
    print()

    # 1단계: 모든 스피커에서 좌우 편차 수집
    print("📊 1단계: 모든 스피커에서 편차 수집 중...")

    speaker_data = {}  # IR 데이터 저장 (나중에 보정 적용용)

    for speaker, pair in hrir.irs.items():
        left_ir = pair['left']
        right_ir = pair['right']

        left_peak = left_ir.peak_index()
        right_peak = right_ir.peak_index()

        if left_peak is None or right_peak is None:
            print(f"  ⚠️ {speaker}: 피크를 찾을 수 없어 건너뜁니다.")
            continue

        # 편차 수집
        corrector.collect_speaker_deviation(
            speaker, left_ir.data, right_ir.data, left_peak, right_peak
        )

        # IR 데이터 저장
        speaker_data[speaker] = {
            'left_ir': left_ir,
            'right_ir': right_ir,
            'left_peak': left_peak,
            'right_peak': right_peak
        }

        print(f"  ✓ {speaker}: 편차 수집 완료")

    if len(corrector.all_speaker_deviations) < 2:
        print("\n⚠️ 교차검증에 충분한 스피커 데이터가 없습니다 (최소 2개 필요).")
        print("   단일 스피커 보정 모드로 전환합니다.")
        # 단일 스피커 모드로 폴백
        return _apply_single_speaker_fallback(hrir, corrector, speaker_data, plot_analysis, plot_dir)

    # 2단계: 마이크 오차와 HRTF 비대칭 분리
    print("\n📊 2단계: 교차검증으로 마이크 오차 분리 중...")
    mic_error = corrector.separate_microphone_error()

    if not mic_error:
        print("  ⚠️ 마이크 오차를 추정할 수 없습니다.")
        return {'error': '마이크 오차 추정 실패'}

    # 추정된 마이크 오차 출력
    print("\n  📈 추정된 마이크 오차 (양수 = 왼쪽이 더 큼):")
    for freq in sorted(mic_error.keys()):
        print(f"     {freq:5d} Hz: {mic_error[freq]:+.2f} dB")

    # 3단계: 일관성 검증
    print("\n📊 3단계: 일관성 검증 중...")
    validation = corrector.validate_consistency()

    print(f"  일관성 점수: {validation['consistency_score']:.2f}")
    print(f"  신뢰도: {validation['confidence']}")

    if not validation['is_valid']:
        print("  ⚠️ 일관성 검증 실패. 보정을 약하게 적용합니다.")
        # 신뢰도가 낮으면 보정 강도를 줄임
        corrector.correction_strength *= 0.5

    # 4단계: 보정 필터 생성 및 적용
    print("\n📊 4단계: 보정 필터 생성 및 적용 중...")
    left_fir, right_fir = corrector.design_correction_filters()

    # 각 스피커에 보정 적용
    for speaker, data in speaker_data.items():
        try:
            if len(left_fir) > 1 and len(right_fir) > 1:
                corrected_left = signal.convolve(data['left_ir'].data, left_fir, mode='same')
                corrected_right = signal.convolve(data['right_ir'].data, right_fir, mode='same')

                data['left_ir'].data = corrected_left
                data['right_ir'].data = corrected_right

                print(f"  ✓ {speaker}: 보정 적용 완료")
            else:
                print(f"  ℹ️ {speaker}: 보정 필터 없음, 원본 유지")
        except Exception as e:
            print(f"  ⚠️ {speaker}: 보정 적용 실패 ({e})")

    # 플롯 생성
    if plot_analysis and plot_dir:
        _plot_cross_validation_results(corrector, plot_dir)

    # 분석 결과 반환
    summary = corrector.get_analysis_summary()
    summary['v3_cross_validation'] = True
    summary['speakers_processed'] = list(speaker_data.keys())

    print(f"\n✅ 마이크 편차 보정 v3.0 완료")
    print(f"   평균 보정량: {summary['avg_error_db']:.2f} dB")
    print(f"   최대 보정량: {summary['max_error_db']:.2f} dB")

    return summary


def _apply_single_speaker_fallback(hrir, corrector, speaker_data, plot_analysis, plot_dir):
    """단일 스피커 데이터만 있을 때 폴백 처리"""
    print("   (단일 스피커 모드: 교차검증 없이 직접 보정)")

    # 수집된 편차를 그대로 마이크 오차로 간주
    if corrector.all_speaker_deviations:
        speaker_name = list(corrector.all_speaker_deviations.keys())[0]
        corrector.mic_error_estimate = corrector.all_speaker_deviations[speaker_name]

    # 보정 적용
    left_fir, right_fir = corrector.design_correction_filters()

    for speaker, data in speaker_data.items():
        try:
            if len(left_fir) > 1:
                data['left_ir'].data = signal.convolve(data['left_ir'].data, left_fir, mode='same')
                data['right_ir'].data = signal.convolve(data['right_ir'].data, right_fir, mode='same')
        except Exception as e:
            print(f"  ⚠️ {speaker}: 보정 적용 실패 ({e})")

    return {
        'v3_cross_validation': False,
        'single_speaker_fallback': True,
        'mic_error_estimate': corrector.mic_error_estimate
    }


def _plot_cross_validation_results(corrector, plot_dir):
    """교차검증 결과 플롯 생성"""
    os.makedirs(plot_dir, exist_ok=True)

    fig, axes = plt.subplots(2, 1, figsize=(14, 10))

    # 1. 스피커별 편차 비교
    ax1 = axes[0]

    speakers = list(corrector.all_speaker_deviations.keys())
    freqs = sorted(corrector.octave_bands)

    for speaker in speakers:
        deviations = corrector.all_speaker_deviations[speaker]
        values = [deviations.get(f, 0) for f in freqs]
        expected_sign = corrector.expected_ild_sign.get(speaker, 0)

        linestyle = '-' if expected_sign >= 0 else '--'
        ax1.semilogx(freqs, values, linestyle, marker='o', label=f'{speaker} (기대부호: {expected_sign:+.1f})')

    ax1.axhline(y=0, color='gray', linestyle=':', alpha=0.5)
    ax1.set_xlabel('주파수 (Hz)', fontsize=11)
    ax1.set_ylabel('편차 (dB) [양수=왼쪽>오른쪽]', fontsize=11)
    ax1.set_title('스피커별 좌우 편차 (교차검증 v3.0)', fontsize=13)
    ax1.legend(fontsize=9, loc='best', ncol=2)
    ax1.grid(True, alpha=0.3)

    # 2. 추정된 마이크 오차
    ax2 = axes[1]

    mic_error = corrector.mic_error_estimate
    mic_freqs = sorted(mic_error.keys())
    mic_values = [mic_error[f] for f in mic_freqs]

    ax2.semilogx(mic_freqs, mic_values, 'k-', marker='s', linewidth=2,
                 markersize=10, label='추정된 마이크 오차')
    ax2.axhline(y=0, color='gray', linestyle=':', alpha=0.5)

    # 보정 후 예상 편차 (각 스피커에 대해)
    for speaker in speakers:
        deviations = corrector.all_speaker_deviations[speaker]
        corrected = [deviations.get(f, 0) - mic_error.get(f, 0) for f in mic_freqs]
        ax2.semilogx(mic_freqs, corrected, '--', alpha=0.5, label=f'{speaker} 보정 후 예상')

    ax2.set_xlabel('주파수 (Hz)', fontsize=11)
    ax2.set_ylabel('편차 (dB)', fontsize=11)
    ax2.set_title('추정된 마이크 오차 및 보정 후 예상 편차', fontsize=13)
    ax2.legend(fontsize=9, loc='best', ncol=2)
    ax2.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(os.path.join(plot_dir, 'microphone_deviation_cross_validation_v3.png'),
                dpi=150, bbox_inches='tight')
    plt.close()

    print(f"✅ 교차검증 분석 플롯이 {plot_dir}에 저장되었습니다.")
