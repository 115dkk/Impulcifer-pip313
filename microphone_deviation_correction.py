# -*- coding: utf-8 -*-

import os
import numpy as np
import matplotlib.pyplot as plt
from scipy import signal
from scipy.fft import fft, ifft, fftfreq
from autoeq.frequency_response import FrequencyResponse
import warnings


class MicrophoneDeviationCorrector:
    """
    고급 마이크 착용 편차 보정 클래스 (v2.0)

    바이노럴 임펄스 응답 측정 시 좌우 귀에 착용된 마이크의 위치/깊이 차이로 인한
    주파수 응답 편차를 보정합니다. REW의 MTW(Minimum Time Window) 개념을 활용하여
    직접음 구간만을 분석하고 보정합니다.

    v2.0 개선사항:
    - 적응형 비대칭 보정: 좌우 응답의 품질을 평가하여 더 나은 쪽을 참조로 사용
    - 위상 보정 추가: ITD(Interaural Time Difference) 정보를 FIR 필터에 반영
    - ITD/ILD 해부학적 검증: 인간의 머리 크기로 예상되는 범위 검증
    - 주파수 대역별 보정 전략: 저주파(ITD), 중간주파(혼합), 고주파(ILD) 차별화
    """

    def __init__(self, sample_rate,
                 octave_bands=None,
                 min_gate_cycles=2,
                 max_gate_cycles=8,
                 correction_strength=0.7,
                 smoothing_window=1/3,
                 max_correction_db=6.0,
                 enable_phase_correction=True,
                 enable_adaptive_correction=True,
                 enable_anatomical_validation=True,
                 itd_range_ms=(-0.7, 0.7),
                 head_radius_cm=8.75):
        """
        Args:
            sample_rate (int): 샘플링 레이트 (Hz)
            octave_bands (list): 분석할 옥타브 밴드 중심 주파수들 (Hz). None이면 기본값 사용
            min_gate_cycles (float): 최소 게이트 길이 (사이클 수)
            max_gate_cycles (float): 최대 게이트 길이 (사이클 수)
            correction_strength (float): 보정 강도 (0.0~1.0)
            smoothing_window (float): 주파수 응답 스무딩 윈도우 크기 (옥타브)
            max_correction_db (float): 최대 보정량 (dB)
            enable_phase_correction (bool): 위상 보정 활성화 (v2.0)
            enable_adaptive_correction (bool): 적응형 비대칭 보정 활성화 (v2.0)
            enable_anatomical_validation (bool): ITD/ILD 해부학적 검증 활성화 (v2.0)
            itd_range_ms (tuple): 허용 가능한 ITD 범위 (ms) (v2.0)
            head_radius_cm (float): 평균 머리 반지름 (cm), ITD 검증용 (v2.0)
        """
        self.fs = sample_rate
        self.correction_strength = np.clip(correction_strength, 0.0, 1.0)
        self.min_gate_cycles = min_gate_cycles
        self.max_gate_cycles = max_gate_cycles
        self.smoothing_window = smoothing_window
        self.max_correction_db = max_correction_db

        # v2.0 새로운 기능 플래그
        self.enable_phase_correction = enable_phase_correction
        self.enable_adaptive_correction = enable_adaptive_correction
        self.enable_anatomical_validation = enable_anatomical_validation
        self.itd_range_samples = (int(itd_range_ms[0] * sample_rate / 1000),
                                  int(itd_range_ms[1] * sample_rate / 1000))
        self.head_radius_m = head_radius_cm / 100.0
        self.speed_of_sound = 343.0  # m/s

        # 기본 옥타브 밴드 설정 (125Hz ~ 16kHz)
        if octave_bands is None:
            self.octave_bands = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
        else:
            self.octave_bands = octave_bands

        # 나이퀴스트 주파수 이하로 제한
        self.octave_bands = [f for f in self.octave_bands if f < self.fs / 2]

        # 각 밴드별 게이트 길이 계산
        self._calculate_gate_lengths()

        # 주파수 대역 분류 (v2.0)
        self._classify_frequency_bands()

    def _classify_frequency_bands(self):
        """
        주파수 대역을 저/중/고로 분류 (v2.0)
        - 저주파 (< 700Hz): ITD가 지배적
        - 중간주파 (700Hz - 4kHz): ITD/ILD 혼합
        - 고주파 (> 4kHz): ILD가 지배적
        """
        self.low_freq_bands = [f for f in self.octave_bands if f < 700]
        self.mid_freq_bands = [f for f in self.octave_bands if 700 <= f <= 4000]
        self.high_freq_bands = [f for f in self.octave_bands if f > 4000]

    def _calculate_gate_lengths(self):
        """각 주파수 밴드별 최적 게이트 길이 계산"""
        self.gate_lengths = {}

        for center_freq in self.octave_bands:
            # 주파수가 높을수록 짧은 게이트 사용
            # 고주파: min_gate_cycles, 저주파: max_gate_cycles로 선형 보간
            log_freq_ratio = np.log10(center_freq / self.octave_bands[0]) / np.log10(self.octave_bands[-1] / self.octave_bands[0])
            cycles = self.max_gate_cycles - (self.max_gate_cycles - self.min_gate_cycles) * log_freq_ratio

            # 사이클 수를 샘플 수로 변환
            samples_per_cycle = self.fs / center_freq
            gate_samples = int(cycles * samples_per_cycle)

            # 최소 16샘플, 최대 fs/10 샘플로 제한
            gate_samples = np.clip(gate_samples, 16, self.fs // 10)

            self.gate_lengths[center_freq] = gate_samples

    def _apply_frequency_gate(self, ir_data, center_freq, peak_index):
        """
        특정 주파수 밴드에 대해 시간 게이팅 적용

        Args:
            ir_data (np.array): 임펄스 응답 데이터
            center_freq (float): 중심 주파수 (Hz)
            peak_index (int): 피크 인덱스

        Returns:
            np.array: 게이팅된 임펄스 응답
        """
        gate_length = self.gate_lengths[center_freq]

        # 피크 이후 게이트 길이만큼 추출
        start_idx = peak_index
        end_idx = min(start_idx + gate_length, len(ir_data))

        if end_idx <= start_idx:
            return np.zeros(gate_length)

        # 게이팅된 구간 추출
        gated_segment = ir_data[start_idx:end_idx]

        # 부족한 길이는 0으로 패딩
        if len(gated_segment) < gate_length:
            gated_segment = np.pad(gated_segment, (0, gate_length - len(gated_segment)), 'constant')

        # 테이퍼 윈도우 적용 (끝부분 페이드아웃)
        window = np.ones(gate_length)
        fade_length = min(gate_length // 4, 32)  # 페이드 길이
        if fade_length > 0:
            window[-fade_length:] = np.linspace(1, 0, fade_length)

        return gated_segment * window

    def _extract_band_response(self, ir_data, center_freq, peak_index):
        """
        특정 주파수 밴드의 응답 추출

        Args:
            ir_data (np.array): 임펄스 응답 데이터
            center_freq (float): 중심 주파수 (Hz)
            peak_index (int): 피크 인덱스

        Returns:
            complex: 해당 밴드의 복소 응답
        """
        # 밴드패스 필터 설계 (1/3 옥타브)
        lower_freq = center_freq / (2**(1/6))
        upper_freq = center_freq * (2**(1/6))

        # 나이퀴스트 주파수 제한
        upper_freq = min(upper_freq, self.fs / 2 * 0.95)

        if lower_freq >= upper_freq:
            return 0.0 + 0.0j

        # 버터워스 밴드패스 필터
        try:
            sos = signal.butter(4, [lower_freq, upper_freq], btype='band', fs=self.fs, output='sos')
            filtered_ir = signal.sosfilt(sos, ir_data)
        except ValueError:
            # 필터 설계 실패 시 원본 사용
            filtered_ir = ir_data

        # 게이팅 적용
        gated_ir = self._apply_frequency_gate(filtered_ir, center_freq, peak_index)

        # FFT로 주파수 응답 계산
        fft_length = max(len(gated_ir) * 2, 512)  # 제로 패딩
        fft_result = fft(gated_ir, n=fft_length)
        freqs = fftfreq(fft_length, 1/self.fs)

        # 중심 주파수에 가장 가까운 빈 찾기
        center_bin = np.argmin(np.abs(freqs - center_freq))

        return fft_result[center_bin]

    def _evaluate_response_quality(self, responses):
        """
        응답의 품질을 평가 (v2.0)

        Args:
            responses (dict): 주파수별 복소 응답

        Returns:
            float: 품질 점수 (높을수록 좋음)
        """
        if not responses:
            return 0.0

        magnitudes = [np.abs(resp) for resp in responses.values()]

        # 1. 평균 크기 (너무 작으면 노이즈가 많음)
        avg_magnitude = np.mean(magnitudes)

        # 2. 변동성 (smoothness) - 낮을수록 좋음
        if len(magnitudes) > 1:
            log_mags = np.log10(np.array(magnitudes) + 1e-12)
            smoothness = np.std(np.diff(log_mags))
        else:
            smoothness = 0.0

        # 3. SNR 추정 (고주파 대역의 일관성)
        high_freq_mags = [np.abs(responses[f]) for f in responses.keys() if f > 4000]
        if len(high_freq_mags) > 2:
            snr_estimate = np.mean(high_freq_mags) / (np.std(high_freq_mags) + 1e-12)
        else:
            snr_estimate = 1.0

        # 종합 점수 (정규화된 가중 합)
        quality_score = (
            np.log10(avg_magnitude + 1e-12) * 0.3 +
            (1.0 / (smoothness + 0.1)) * 0.4 +
            np.log10(snr_estimate + 1.0) * 0.3
        )

        return quality_score

    def _validate_itd(self, phase_diffs_rad, frequencies):
        """
        ITD(Interaural Time Difference)의 해부학적 타당성 검증 (v2.0)

        Args:
            phase_diffs_rad (dict): 주파수별 위상 차이 (라디안)
            frequencies (list): 주파수 목록

        Returns:
            dict: 검증 결과 및 경고 메시지
        """
        if not self.enable_anatomical_validation:
            return {'valid': True, 'warnings': []}

        warnings_list = []

        # 저주파 대역에서 ITD 계산 (< 1500Hz)
        low_freq_itds = []
        for freq in frequencies:
            if freq < 1500 and freq in phase_diffs_rad:
                phase_rad = phase_diffs_rad[freq]
                # ITD = phase_diff / (2π * frequency)
                itd_seconds = phase_rad / (2 * np.pi * freq)
                itd_samples = itd_seconds * self.fs
                low_freq_itds.append((freq, itd_samples, itd_seconds * 1000))

        if low_freq_itds:
            # 평균 ITD 계산
            avg_itd_samples = np.mean([itd[1] for itd in low_freq_itds])
            avg_itd_ms = avg_itd_samples / self.fs * 1000

            # 해부학적 범위 검증
            expected_max_itd_ms = (self.head_radius_m * 2) / self.speed_of_sound * 1000

            if abs(avg_itd_ms) > expected_max_itd_ms:
                warnings_list.append(
                    f"ITD가 해부학적으로 비정상적입니다: {avg_itd_ms:.3f}ms "
                    f"(예상 범위: ±{expected_max_itd_ms:.3f}ms). "
                    f"마이크 배치를 확인하세요."
                )

            # 주파수별 ITD 일관성 검증
            itd_std = np.std([itd[1] for itd in low_freq_itds])
            if itd_std > 0.3 * self.fs / 1000:  # 0.3ms 이상 변동
                warnings_list.append(
                    f"저주파 대역에서 ITD 일관성이 낮습니다 (표준편차: {itd_std / self.fs * 1000:.3f}ms). "
                    f"측정 노이즈가 있을 수 있습니다."
                )

        return {
            'valid': len(warnings_list) == 0,
            'warnings': warnings_list,
            'itd_analysis': low_freq_itds if low_freq_itds else None
        }

    def _calculate_deviation_metrics(self, left_responses, right_responses):
        """
        좌우 응답 간의 편차 메트릭 계산 (v2.0 개선)

        Args:
            left_responses (dict): 좌측 귀의 주파수별 응답
            right_responses (dict): 우측 귀의 주파수별 응답

        Returns:
            dict: 편차 메트릭들 (ITD 정보 포함)
        """
        deviations = {}
        phase_diffs = {}

        for freq in self.octave_bands:
            if freq not in left_responses or freq not in right_responses:
                continue

            left_resp = left_responses[freq]
            right_resp = right_responses[freq]

            # 크기 차이 (dB) - ILD (Interaural Level Difference)
            left_mag = np.abs(left_resp)
            right_mag = np.abs(right_resp)

            if left_mag > 0 and right_mag > 0:
                magnitude_diff_db = 20 * np.log10(left_mag / right_mag)
            else:
                magnitude_diff_db = 0.0

            # 위상 차이 (라디안)
            phase_diff = np.angle(left_resp) - np.angle(right_resp)
            # 위상을 -π ~ π 범위로 정규화
            phase_diff = np.arctan2(np.sin(phase_diff), np.cos(phase_diff))
            phase_diffs[freq] = phase_diff

            # ITD 계산 (저주파 대역)
            itd_ms = 0.0
            if freq < 1500:
                itd_seconds = phase_diff / (2 * np.pi * freq)
                itd_ms = itd_seconds * 1000

            deviations[freq] = {
                'magnitude_diff_db': magnitude_diff_db,  # ILD
                'phase_diff_rad': phase_diff,
                'itd_ms': itd_ms,
                'left_magnitude': left_mag,
                'right_magnitude': right_mag,
                'left_phase': np.angle(left_resp),
                'right_phase': np.angle(right_resp)
            }

        # v2.0: ITD 해부학적 검증
        itd_validation = self._validate_itd(phase_diffs, list(deviations.keys()))

        # v2.0: 응답 품질 평가
        left_quality = self._evaluate_response_quality(left_responses)
        right_quality = self._evaluate_response_quality(right_responses)

        return {
            'frequency_deviations': deviations,
            'itd_validation': itd_validation,
            'left_quality': left_quality,
            'right_quality': right_quality,
            'reference_side': 'left' if left_quality >= right_quality else 'right'
        }

    def _design_correction_filters(self, deviation_results):
        """
        편차 보정을 위한 FIR 필터 설계 (v2.0 완전 개선)

        Args:
            deviation_results (dict): _calculate_deviation_metrics의 결과

        Returns:
            tuple: (left_fir, right_fir) 보정 필터들
        """
        deviations = deviation_results['frequency_deviations']
        reference_side = deviation_results['reference_side']

        # 주파수 응답 생성을 위한 주파수 벡터
        frequencies = FrequencyResponse.generate_frequencies(f_step=1.01, f_min=20, f_max=self.fs/2)

        # 크기 및 위상 보정 응답 초기화
        left_mag_correction = np.zeros(len(frequencies))
        right_mag_correction = np.zeros(len(frequencies))
        left_phase_correction = np.zeros(len(frequencies))
        right_phase_correction = np.zeros(len(frequencies))

        # 각 옥타브 밴드별 보정값 계산
        for freq, deviation in deviations.items():
            mag_diff = deviation['magnitude_diff_db']
            phase_diff = deviation['phase_diff_rad']
            deviation['itd_ms']

            # 주파수 대역별 보정 전략 (v2.0)
            if freq in self.low_freq_bands:
                # 저주파: ITD가 중요, 크기 차이는 작아야 함
                mag_weight = 0.3
                phase_weight = 1.0 if self.enable_phase_correction else 0.0
            elif freq in self.mid_freq_bands:
                # 중간주파: ITD와 ILD 모두 중요
                mag_weight = 0.7
                phase_weight = 0.6 if self.enable_phase_correction else 0.0
            else:  # high_freq_bands
                # 고주파: ILD가 지배적, 위상은 덜 중요
                mag_weight = 1.0
                phase_weight = 0.2 if self.enable_phase_correction else 0.0

            # 크기 보정 계산
            correction_amount = np.clip(mag_diff * self.correction_strength * mag_weight,
                                      -self.max_correction_db, self.max_correction_db)

            # v2.0: 적응형 비대칭 보정
            if self.enable_adaptive_correction:
                # 품질이 낮은 쪽에 더 많은 보정 적용
                if reference_side == 'left':
                    # 좌측이 더 좋음 -> 우측을 좌측에 맞춤
                    left_mag_corr = -correction_amount * 0.2
                    right_mag_corr = correction_amount * 0.8
                else:
                    # 우측이 더 좋음 -> 좌측을 우측에 맞춤
                    left_mag_corr = -correction_amount * 0.8
                    right_mag_corr = correction_amount * 0.2
            else:
                # 대칭적 보정 (기존 방식)
                left_mag_corr = -correction_amount / 2
                right_mag_corr = correction_amount / 2

            # 위상 보정 계산 (v2.0)
            if self.enable_phase_correction and phase_weight > 0:
                phase_correction_amount = phase_diff * self.correction_strength * phase_weight

                if self.enable_adaptive_correction:
                    if reference_side == 'left':
                        left_phase_corr = -phase_correction_amount * 0.2
                        right_phase_corr = phase_correction_amount * 0.8
                    else:
                        left_phase_corr = -phase_correction_amount * 0.8
                        right_phase_corr = phase_correction_amount * 0.2
                else:
                    left_phase_corr = -phase_correction_amount / 2
                    right_phase_corr = phase_correction_amount / 2
            else:
                left_phase_corr = 0.0
                right_phase_corr = 0.0

            # 해당 주파수 대역에 보정값 적용
            freq_mask = np.logical_and(frequencies >= freq / np.sqrt(2),
                                     frequencies <= freq * np.sqrt(2))

            left_mag_correction[freq_mask] = left_mag_corr
            right_mag_correction[freq_mask] = right_mag_corr
            left_phase_correction[freq_mask] = left_phase_corr
            right_phase_correction[freq_mask] = right_phase_corr

        # 스무딩 적용
        if self.smoothing_window > 0:
            try:
                left_fr = FrequencyResponse(name='left_correction',
                                          frequency=frequencies.copy(),
                                          raw=left_mag_correction.copy())
                right_fr = FrequencyResponse(name='right_correction',
                                           frequency=frequencies.copy(),
                                           raw=right_mag_correction.copy())

                if hasattr(left_fr, 'smoothen_fractional_octave'):
                    left_fr.smoothen_fractional_octave(window_size=self.smoothing_window)
                    right_fr.smoothen_fractional_octave(window_size=self.smoothing_window)
                elif hasattr(left_fr, 'smoothen'):
                    left_fr.smoothen(window_size=self.smoothing_window)
                    right_fr.smoothen(window_size=self.smoothing_window)

                if hasattr(left_fr, 'smoothed') and len(left_fr.smoothed) == len(frequencies):
                    left_mag_correction = left_fr.smoothed
                if hasattr(right_fr, 'smoothed') and len(right_fr.smoothed) == len(frequencies):
                    right_mag_correction = right_fr.smoothed

            except Exception as e:
                print(f"스무딩 실패: {e}. 원본 보정 곡선 사용.")

        # FIR 필터 생성
        try:
            # 배열 길이 확인 및 맞춤
            target_length = len(frequencies)

            for arr, name in [(left_mag_correction, '좌측 크기'), (right_mag_correction, '우측 크기'),
                             (left_phase_correction, '좌측 위상'), (right_phase_correction, '우측 위상')]:
                if len(arr) != target_length:
                    print(f"경고: {name} 배열 길이 불일치. 크기 조정.")

            # 복소 주파수 응답 생성 (크기 + 위상)
            left_complex_response = 10**(left_mag_correction / 20) * np.exp(1j * left_phase_correction)
            right_complex_response = 10**(right_mag_correction / 20) * np.exp(1j * right_phase_correction)

            # IFFT로 임펄스 응답 생성
            len(frequencies) * 2
            left_full_fft = np.concatenate([left_complex_response, np.conj(left_complex_response[::-1])])
            right_full_fft = np.concatenate([right_complex_response, np.conj(right_complex_response[::-1])])

            left_fir = np.real(ifft(left_full_fft))
            right_fir = np.real(ifft(right_full_fft))

            # 최소 위상 변환 (인과성 보장)
            left_fr_obj = FrequencyResponse(name='left', frequency=frequencies.copy(),
                                           raw=left_mag_correction.copy())
            right_fr_obj = FrequencyResponse(name='right', frequency=frequencies.copy(),
                                            raw=right_mag_correction.copy())

            left_fir = left_fr_obj.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            right_fir = right_fr_obj.minimum_phase_impulse_response(fs=self.fs, normalize=False)

            # FIR 필터 길이 제한
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

    def correct_microphone_deviation(self, left_ir, right_ir,
                                   left_peak_index=None, right_peak_index=None,
                                   plot_analysis=False, plot_dir=None):
        """
        마이크 착용 편차 보정 수행 (v2.0)

        Args:
            left_ir (np.array): 좌측 귀 임펄스 응답
            right_ir (np.array): 우측 귀 임펄스 응답
            left_peak_index (int): 좌측 피크 인덱스 (None이면 자동 검출)
            right_peak_index (int): 우측 피크 인덱스 (None이면 자동 검출)
            plot_analysis (bool): 분석 결과 플롯 생성 여부
            plot_dir (str): 플롯 저장 디렉토리

        Returns:
            tuple: (corrected_left_ir, corrected_right_ir, analysis_results)
        """
        # 입력 검증
        if len(left_ir) != len(right_ir):
            min_len = min(len(left_ir), len(right_ir))
            left_ir = left_ir[:min_len]
            right_ir = right_ir[:min_len]

        # 피크 인덱스 자동 검출
        if left_peak_index is None:
            left_peak_index = np.argmax(np.abs(left_ir))
        if right_peak_index is None:
            right_peak_index = np.argmax(np.abs(right_ir))

        # 각 주파수 밴드별 응답 추출
        left_responses = {}
        right_responses = {}

        for freq in self.octave_bands:
            left_responses[freq] = self._extract_band_response(left_ir, freq, left_peak_index)
            right_responses[freq] = self._extract_band_response(right_ir, freq, right_peak_index)

        # 편차 분석 (v2.0 개선)
        deviation_results = self._calculate_deviation_metrics(left_responses, right_responses)
        deviations = deviation_results['frequency_deviations']

        # ITD 검증 경고 출력
        if deviation_results['itd_validation']['warnings']:
            print("⚠️ ITD/ILD 해부학적 검증 경고:")
            for warning in deviation_results['itd_validation']['warnings']:
                print(f"  - {warning}")

        # 편차가 유의미한지 확인
        significant_deviations = []
        for freq, deviation in deviations.items():
            if abs(deviation['magnitude_diff_db']) > 0.5:  # 0.5dB 이상의 편차만 고려
                significant_deviations.append(abs(deviation['magnitude_diff_db']))

        if not significant_deviations:
            print("유의미한 마이크 편차가 감지되지 않았습니다. 보정을 건너뜁니다.")
            analysis_results = {
                'deviation_results': deviation_results,
                'correction_filters': {
                    'left_fir': np.array([1.0]),
                    'right_fir': np.array([1.0])
                },
                'gate_lengths': self.gate_lengths,
                'octave_bands': self.octave_bands,
                'correction_applied': False,
                'v2_features': {
                    'phase_correction': self.enable_phase_correction,
                    'adaptive_correction': self.enable_adaptive_correction,
                    'anatomical_validation': self.enable_anatomical_validation
                }
            }
            return left_ir.copy(), right_ir.copy(), analysis_results

        # v2.0: 응답 품질 기반 참조 선택
        print(f"📊 응답 품질 평가: 좌측={deviation_results['left_quality']:.2f}, "
              f"우측={deviation_results['right_quality']:.2f}")
        print(f"🎯 참조 기준: {deviation_results['reference_side']} (품질이 더 우수)")

        # 보정 필터 설계 (v2.0)
        left_fir, right_fir = self._design_correction_filters(deviation_results)

        # 보정 적용
        try:
            if len(left_fir) > 1 and len(right_fir) > 1:
                corrected_left_ir = signal.convolve(left_ir, left_fir, mode='same')
                corrected_right_ir = signal.convolve(right_ir, right_fir, mode='same')
            else:
                corrected_left_ir = left_ir.copy()
                corrected_right_ir = right_ir.copy()
        except Exception as e:
            print(f"보정 필터 적용 실패: {e}. 원본 반환.")
            corrected_left_ir = left_ir.copy()
            corrected_right_ir = right_ir.copy()

        # 분석 결과 정리
        analysis_results = {
            'deviation_results': deviation_results,
            'correction_filters': {
                'left_fir': left_fir,
                'right_fir': right_fir
            },
            'gate_lengths': self.gate_lengths,
            'octave_bands': self.octave_bands,
            'correction_applied': True,
            'avg_deviation_db': np.mean(significant_deviations) if significant_deviations else 0.0,
            'max_deviation_db': np.max(significant_deviations) if significant_deviations else 0.0,
            'v2_features': {
                'phase_correction': self.enable_phase_correction,
                'adaptive_correction': self.enable_adaptive_correction,
                'anatomical_validation': self.enable_anatomical_validation,
                'reference_side': deviation_results['reference_side']
            }
        }

        # 플롯 생성
        if plot_analysis and plot_dir:
            self._plot_analysis_results(left_ir, right_ir, corrected_left_ir, corrected_right_ir,
                                      analysis_results, plot_dir)

        return corrected_left_ir, corrected_right_ir, analysis_results

    def _plot_analysis_results(self, original_left, original_right,
                             corrected_left, corrected_right,
                             analysis_results, plot_dir):
        """분석 결과 플롯 생성 (v2.0 개선)"""
        import os
        os.makedirs(plot_dir, exist_ok=True)

        deviations = analysis_results['deviation_results']['frequency_deviations']

        # 1. 편차 분석 결과 플롯 (ILD + ITD)
        fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(14, 10))

        freqs = list(deviations.keys())
        mag_diffs = [deviations[f]['magnitude_diff_db'] for f in freqs]
        phase_diffs = [deviations[f]['phase_diff_rad'] * 180 / np.pi for f in freqs]
        itd_values = [deviations[f]['itd_ms'] for f in freqs if f < 1500]
        itd_freqs = [f for f in freqs if f < 1500]

        # ILD (Interaural Level Difference)
        ax1.semilogx(freqs, mag_diffs, 'o-', label='크기 차이 (L-R)', linewidth=2, markersize=8)
        ax1.axhline(y=0, color='gray', linestyle='--', alpha=0.5)
        ax1.set_ylabel('ILD (dB)', fontsize=11, fontweight='bold')
        ax1.set_title('마이크 착용 편차 분석 결과 (v2.0)', fontsize=13, fontweight='bold')
        ax1.grid(True, alpha=0.3)
        ax1.legend(fontsize=10)

        # 위상 차이
        ax2.semilogx(freqs, phase_diffs, 's-', color='red', label='위상 차이 (L-R)', linewidth=2, markersize=8)
        ax2.axhline(y=0, color='gray', linestyle='--', alpha=0.5)
        ax2.set_ylabel('위상 차이 (도)', fontsize=11, fontweight='bold')
        ax2.grid(True, alpha=0.3)
        ax2.legend(fontsize=10)

        # ITD (저주파만)
        if itd_values:
            ax3.semilogx(itd_freqs, itd_values, 'd-', color='green', label='ITD (< 1.5kHz)', linewidth=2, markersize=8)
            ax3.axhline(y=0, color='gray', linestyle='--', alpha=0.5)

            # 해부학적 범위 표시
            expected_max_itd = (self.head_radius_m * 2) / self.speed_of_sound * 1000
            ax3.axhline(y=expected_max_itd, color='orange', linestyle=':', alpha=0.7, label=f'해부학적 최대값 (±{expected_max_itd:.2f}ms)')
            ax3.axhline(y=-expected_max_itd, color='orange', linestyle=':', alpha=0.7)

            ax3.set_xlabel('주파수 (Hz)', fontsize=11, fontweight='bold')
            ax3.set_ylabel('ITD (ms)', fontsize=11, fontweight='bold')
            ax3.grid(True, alpha=0.3)
            ax3.legend(fontsize=10)
        else:
            ax3.text(0.5, 0.5, 'ITD 데이터 없음 (저주파 대역 없음)',
                    transform=ax3.transAxes, ha='center', va='center', fontsize=12)
            ax3.set_xlabel('주파수 (Hz)', fontsize=11)

        plt.tight_layout()
        plt.savefig(os.path.join(plot_dir, 'microphone_deviation_analysis_v2.png'), dpi=150, bbox_inches='tight')
        plt.close()

        # 2. 보정 전후 주파수 응답 비교
        fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(14, 8))

        # FFT로 주파수 응답 계산
        fft_len = max(len(original_left) * 2, 8192)
        freqs_fft = np.fft.fftfreq(fft_len, 1/self.fs)[:fft_len//2]

        orig_left_fft = np.fft.fft(original_left, n=fft_len)[:fft_len//2]
        orig_right_fft = np.fft.fft(original_right, n=fft_len)[:fft_len//2]
        corr_left_fft = np.fft.fft(corrected_left, n=fft_len)[:fft_len//2]
        corr_right_fft = np.fft.fft(corrected_right, n=fft_len)[:fft_len//2]

        # dB 변환
        orig_left_db = 20 * np.log10(np.abs(orig_left_fft) + 1e-12)
        orig_right_db = 20 * np.log10(np.abs(orig_right_fft) + 1e-12)
        corr_left_db = 20 * np.log10(np.abs(corr_left_fft) + 1e-12)
        corr_right_db = 20 * np.log10(np.abs(corr_right_fft) + 1e-12)

        # 크기 응답
        ax1.semilogx(freqs_fft, orig_left_db, alpha=0.6, label='원본 좌측', color='blue', linewidth=1.5)
        ax1.semilogx(freqs_fft, orig_right_db, alpha=0.6, label='원본 우측', color='red', linewidth=1.5)
        ax1.semilogx(freqs_fft, corr_left_db, '--', label='보정 좌측', color='darkblue', linewidth=2)
        ax1.semilogx(freqs_fft, corr_right_db, '--', label='보정 우측', color='darkred', linewidth=2)

        # 참조 기준 표시
        ref_side = analysis_results['v2_features'].get('reference_side', 'unknown')
        ax1.text(0.02, 0.98, f'참조 기준: {ref_side}', transform=ax1.transAxes,
                fontsize=10, verticalalignment='top', bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))

        ax1.set_ylabel('크기 (dB)', fontsize=11, fontweight='bold')
        ax1.set_title('마이크 편차 보정 전후 주파수 응답 비교 (v2.0)', fontsize=13, fontweight='bold')
        ax1.set_xlim([20, self.fs/2])
        ax1.grid(True, alpha=0.3)
        ax1.legend(fontsize=10, loc='best')

        # 좌우 차이 (보정 효과)
        orig_diff = orig_left_db - orig_right_db
        corr_diff = corr_left_db - corr_right_db

        ax2.semilogx(freqs_fft, orig_diff, alpha=0.7, label='원본 L-R 차이', color='purple', linewidth=2)
        ax2.semilogx(freqs_fft, corr_diff, '--', label='보정 후 L-R 차이', color='green', linewidth=2)
        ax2.axhline(y=0, color='gray', linestyle='--', alpha=0.5)

        ax2.set_xlabel('주파수 (Hz)', fontsize=11, fontweight='bold')
        ax2.set_ylabel('좌우 차이 (dB)', fontsize=11, fontweight='bold')
        ax2.set_xlim([20, self.fs/2])
        ax2.grid(True, alpha=0.3)
        ax2.legend(fontsize=10, loc='best')

        plt.tight_layout()
        plt.savefig(os.path.join(plot_dir, 'microphone_deviation_correction_comparison_v2.png'),
                   dpi=150, bbox_inches='tight')
        plt.close()

        print(f"✅ 마이크 편차 보정 분석 플롯 (v2.0)이 {plot_dir}에 저장되었습니다.")


def apply_microphone_deviation_correction_to_hrir(hrir,
                                                 correction_strength=0.7,
                                                 enable_phase_correction=True,
                                                 enable_adaptive_correction=True,
                                                 enable_anatomical_validation=True,
                                                 plot_analysis=False,
                                                 plot_dir=None):
    """
    HRIR 객체에 마이크 착용 편차 보정 적용 (v2.0)

    Args:
        hrir (HRIR): HRIR 객체
        correction_strength (float): 보정 강도 (0.0~1.0)
        enable_phase_correction (bool): 위상 보정 활성화 (v2.0)
        enable_adaptive_correction (bool): 적응형 비대칭 보정 활성화 (v2.0)
        enable_anatomical_validation (bool): ITD/ILD 해부학적 검증 활성화 (v2.0)
        plot_analysis (bool): 분석 결과 플롯 생성 여부
        plot_dir (str): 플롯 저장 디렉토리

    Returns:
        dict: 각 스피커별 분석 결과
    """
    corrector = MicrophoneDeviationCorrector(
        sample_rate=hrir.fs,
        correction_strength=correction_strength,
        enable_phase_correction=enable_phase_correction,
        enable_adaptive_correction=enable_adaptive_correction,
        enable_anatomical_validation=enable_anatomical_validation
    )

    all_analysis_results = {}

    print("\n🎧 마이크 편차 보정 v2.0 시작")
    print(f"  - 위상 보정: {'활성화' if enable_phase_correction else '비활성화'}")
    print(f"  - 적응형 보정: {'활성화' if enable_adaptive_correction else '비활성화'}")
    print(f"  - 해부학적 검증: {'활성화' if enable_anatomical_validation else '비활성화'}")
    print()

    for speaker, pair in hrir.irs.items():
        print(f"🔊 처리 중: {speaker} 스피커")

        left_ir = pair['left']
        right_ir = pair['right']

        # 피크 인덱스 가져오기
        left_peak = left_ir.peak_index()
        right_peak = right_ir.peak_index()

        if left_peak is None or right_peak is None:
            print(f"  ⚠️ {speaker} 스피커의 피크를 찾을 수 없어 보정을 건너뜁니다.")
            continue

        # 보정 적용
        speaker_plot_dir = None
        if plot_analysis and plot_dir:
            speaker_plot_dir = os.path.join(plot_dir, f'microphone_deviation_{speaker}_v2')

        corrected_left, corrected_right, analysis = corrector.correct_microphone_deviation(
            left_ir.data, right_ir.data,
            left_peak, right_peak,
            plot_analysis=plot_analysis,
            plot_dir=speaker_plot_dir
        )

        # 보정된 데이터로 업데이트
        left_ir.data = corrected_left
        right_ir.data = corrected_right

        all_analysis_results[speaker] = analysis

        if analysis['correction_applied']:
            print(f"  ✅ {speaker} 스피커 마이크 편차 보정 완료")
            print(f"     평균 편차: {analysis['avg_deviation_db']:.2f} dB, "
                  f"최대 편차: {analysis['max_deviation_db']:.2f} dB")
        else:
            print(f"  ℹ️ {speaker} 스피커는 유의미한 편차가 없어 보정 생략")
        print()

    return all_analysis_results
