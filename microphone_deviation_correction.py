# -*- coding: utf-8 -*-

import os
import numpy as np
import matplotlib.pyplot as plt
from scipy import signal
from scipy.signal.windows import hann, tukey
from scipy.fft import fft, ifft, fftfreq
from autoeq.frequency_response import FrequencyResponse
import warnings


class MicrophoneDeviationCorrector:
    """
    마이크 착용 편차 보정 클래스
    
    바이노럴 임펄스 응답 측정 시 좌우 귀에 착용된 마이크의 위치/깊이 차이로 인한
    주파수 응답 편차를 보정합니다. REW의 MTW(Minimum Time Window) 개념을 활용하여
    직접음 구간만을 분석하고 보정합니다.
    """
    
    def __init__(self, sample_rate, 
                 octave_bands=None,
                 min_gate_cycles=2,
                 max_gate_cycles=8,
                 correction_strength=0.7,
                 smoothing_window=1/3,
                 max_correction_db=6.0):
        """
        Args:
            sample_rate (int): 샘플링 레이트 (Hz)
            octave_bands (list): 분석할 옥타브 밴드 중심 주파수들 (Hz). None이면 기본값 사용
            min_gate_cycles (float): 최소 게이트 길이 (사이클 수)
            max_gate_cycles (float): 최대 게이트 길이 (사이클 수)
            correction_strength (float): 보정 강도 (0.0~1.0)
            smoothing_window (float): 주파수 응답 스무딩 윈도우 크기 (옥타브)
            max_correction_db (float): 최대 보정량 (dB)
        """
        self.fs = sample_rate
        self.correction_strength = np.clip(correction_strength, 0.0, 1.0)
        self.min_gate_cycles = min_gate_cycles
        self.max_gate_cycles = max_gate_cycles
        self.smoothing_window = smoothing_window
        self.max_correction_db = max_correction_db
        
        # 기본 옥타브 밴드 설정 (125Hz ~ 16kHz)
        if octave_bands is None:
            self.octave_bands = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
        else:
            self.octave_bands = octave_bands
            
        # 나이퀴스트 주파수 이하로 제한
        self.octave_bands = [f for f in self.octave_bands if f < self.fs / 2]
        
        # 각 밴드별 게이트 길이 계산
        self._calculate_gate_lengths()
        
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
        
    def _calculate_deviation_metrics(self, left_responses, right_responses):
        """
        좌우 응답 간의 편차 메트릭 계산
        
        Args:
            left_responses (dict): 좌측 귀의 주파수별 응답
            right_responses (dict): 우측 귀의 주파수별 응답
            
        Returns:
            dict: 편차 메트릭들
        """
        deviations = {}
        
        for freq in self.octave_bands:
            if freq not in left_responses or freq not in right_responses:
                continue
                
            left_resp = left_responses[freq]
            right_resp = right_responses[freq]
            
            # 크기 차이 (dB)
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
            
            deviations[freq] = {
                'magnitude_diff_db': magnitude_diff_db,
                'phase_diff_rad': phase_diff,
                'left_magnitude': left_mag,
                'right_magnitude': right_mag
            }
            
        return deviations
        
    def _design_correction_filters(self, deviations):
        """
        편차 보정을 위한 FIR 필터 설계
        
        Args:
            deviations (dict): 편차 메트릭들
            
        Returns:
            tuple: (left_fir, right_fir) 보정 필터들
        """
        # 주파수 응답 생성을 위한 주파수 벡터
        frequencies = FrequencyResponse.generate_frequencies(f_step=1.01, f_min=20, f_max=self.fs/2)
        
        # 보정 응답 초기화
        left_correction = np.zeros(len(frequencies))
        right_correction = np.zeros(len(frequencies))
        
        # 각 옥타브 밴드별 보정값 계산
        for freq, deviation in deviations.items():
            mag_diff = deviation['magnitude_diff_db']
            
            # 보정 강도 적용 및 최대 보정량 제한
            correction_amount = np.clip(mag_diff * self.correction_strength, 
                                      -self.max_correction_db, self.max_correction_db)
            
            # 대칭적 보정: 좌우 귀에 반대 방향으로 절반씩 적용
            left_corr = -correction_amount / 2
            right_corr = correction_amount / 2
            
            # 해당 주파수 대역에 보정값 적용
            freq_mask = np.logical_and(frequencies >= freq / np.sqrt(2), 
                                     frequencies <= freq * np.sqrt(2))
            
            left_correction[freq_mask] = left_corr
            right_correction[freq_mask] = right_corr
            
        # 스무딩 적용
        if self.smoothing_window > 0:
            try:
                left_fr = FrequencyResponse(name='left_correction', 
                                          frequency=frequencies.copy(), 
                                          raw=left_correction.copy())
                right_fr = FrequencyResponse(name='right_correction', 
                                           frequency=frequencies.copy(), 
                                           raw=right_correction.copy())
                
                # smoothen_fractional_octave 메서드가 있는지 확인
                if hasattr(left_fr, 'smoothen_fractional_octave'):
                    left_fr.smoothen_fractional_octave(window_size=self.smoothing_window)
                    right_fr.smoothen_fractional_octave(window_size=self.smoothing_window)
                elif hasattr(left_fr, 'smoothen'):
                    # 대체 스무딩 메서드 사용
                    left_fr.smoothen(window_size=self.smoothing_window)
                    right_fr.smoothen(window_size=self.smoothing_window)
                else:
                    print("스무딩 메서드를 찾을 수 없습니다. 원본 보정 곡선 사용.")
                    raise AttributeError("No smoothing method available")
                
                # 스무딩 후 배열 길이 검증 및 보정
                if hasattr(left_fr, 'smoothed') and len(left_fr.smoothed) == len(frequencies):
                    left_correction = left_fr.smoothed
                if hasattr(right_fr, 'smoothed') and len(right_fr.smoothed) == len(frequencies):
                    right_correction = right_fr.smoothed
                    
            except Exception as e:
                print(f"스무딩 실패: {e}. 원본 보정 곡선 사용.")
                
        # FIR 필터 생성
        try:
            # 배열 길이 재확인 및 강제 맞춤
            target_length = len(frequencies)
            
            if len(left_correction) != target_length:
                print(f"경고: 좌측 보정 배열 길이 불일치 ({len(left_correction)} vs {target_length}). 크기 조정.")
                if len(left_correction) > target_length:
                    left_correction = left_correction[:target_length]
                else:
                    # 부족한 부분을 0으로 패딩
                    left_correction = np.pad(left_correction, (0, target_length - len(left_correction)), 'constant')
                    
            if len(right_correction) != target_length:
                print(f"경고: 우측 보정 배열 길이 불일치 ({len(right_correction)} vs {target_length}). 크기 조정.")
                if len(right_correction) > target_length:
                    right_correction = right_correction[:target_length]
                else:
                    # 부족한 부분을 0으로 패딩
                    right_correction = np.pad(right_correction, (0, target_length - len(right_correction)), 'constant')
            
            # 최종 길이 확인
            assert len(left_correction) == len(frequencies), f"좌측 배열 길이 여전히 불일치: {len(left_correction)} vs {len(frequencies)}"
            assert len(right_correction) == len(frequencies), f"우측 배열 길이 여전히 불일치: {len(right_correction)} vs {len(frequencies)}"
            
            # FrequencyResponse 객체 생성 (복사본 사용)
            left_fr = FrequencyResponse(name='left_correction', 
                                      frequency=frequencies.copy(), 
                                      raw=left_correction.copy())
            right_fr = FrequencyResponse(name='right_correction', 
                                       frequency=frequencies.copy(), 
                                       raw=right_correction.copy())
            
            # 최소 위상 임펄스 응답 생성
            left_fir = left_fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            right_fir = right_fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            
            # FIR 필터 길이 제한 (너무 길면 성능 문제)
            max_fir_length = min(1024, self.fs // 10)  # 최대 1024 샘플 또는 fs/10
            if len(left_fir) > max_fir_length:
                left_fir = left_fir[:max_fir_length]
            if len(right_fir) > max_fir_length:
                right_fir = right_fir[:max_fir_length]
            
        except Exception as e:
            warnings.warn(f"FIR 필터 생성 실패: {e}. 단위 임펄스 반환.")
            # 실패 시 단위 임펄스 반환
            left_fir = np.array([1.0])
            right_fir = np.array([1.0])
            
        return left_fir, right_fir
        
    def correct_microphone_deviation(self, left_ir, right_ir, 
                                   left_peak_index=None, right_peak_index=None,
                                   plot_analysis=False, plot_dir=None):
        """
        마이크 착용 편차 보정 수행
        
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
            
        # 편차 분석
        deviations = self._calculate_deviation_metrics(left_responses, right_responses)
        
        # 편차가 유의미한지 확인
        significant_deviations = []
        for freq, deviation in deviations.items():
            if abs(deviation['magnitude_diff_db']) > 0.5:  # 0.5dB 이상의 편차만 고려
                significant_deviations.append(abs(deviation['magnitude_diff_db']))
                
        if not significant_deviations:
            print("유의미한 마이크 편차가 감지되지 않았습니다. 보정을 건너뜁니다.")
            # 분석 결과만 반환하고 보정은 적용하지 않음
            analysis_results = {
                'deviations': deviations,
                'correction_filters': {
                    'left_fir': np.array([1.0]),
                    'right_fir': np.array([1.0])
                },
                'gate_lengths': self.gate_lengths,
                'octave_bands': self.octave_bands,
                'correction_applied': False
            }
            return left_ir.copy(), right_ir.copy(), analysis_results
        
        # 보정 필터 설계
        left_fir, right_fir = self._design_correction_filters(deviations)
        
        # 보정 적용 (안전한 컨볼루션)
        try:
            if len(left_fir) > 1 and len(right_fir) > 1:
                corrected_left_ir = signal.convolve(left_ir, left_fir, mode='same')
                corrected_right_ir = signal.convolve(right_ir, right_fir, mode='same')
            else:
                # 단위 임펄스인 경우 원본 반환
                corrected_left_ir = left_ir.copy()
                corrected_right_ir = right_ir.copy()
        except Exception as e:
            print(f"보정 필터 적용 실패: {e}. 원본 반환.")
            corrected_left_ir = left_ir.copy()
            corrected_right_ir = right_ir.copy()
        
        # 분석 결과 정리
        analysis_results = {
            'deviations': deviations,
            'correction_filters': {
                'left_fir': left_fir,
                'right_fir': right_fir
            },
            'gate_lengths': self.gate_lengths,
            'octave_bands': self.octave_bands,
            'correction_applied': True,
            'avg_deviation_db': np.mean(significant_deviations) if significant_deviations else 0.0,
            'max_deviation_db': np.max(significant_deviations) if significant_deviations else 0.0
        }
        
        # 플롯 생성
        if plot_analysis and plot_dir:
            self._plot_analysis_results(left_ir, right_ir, corrected_left_ir, corrected_right_ir,
                                      analysis_results, plot_dir)
            
        return corrected_left_ir, corrected_right_ir, analysis_results
        
    def _plot_analysis_results(self, original_left, original_right, 
                             corrected_left, corrected_right, 
                             analysis_results, plot_dir):
        """분석 결과 플롯 생성"""
        import os
        os.makedirs(plot_dir, exist_ok=True)
        
        # 1. 편차 분석 결과 플롯
        fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(12, 8))
        
        freqs = list(analysis_results['deviations'].keys())
        mag_diffs = [analysis_results['deviations'][f]['magnitude_diff_db'] for f in freqs]
        phase_diffs = [analysis_results['deviations'][f]['phase_diff_rad'] * 180 / np.pi for f in freqs]
        
        ax1.semilogx(freqs, mag_diffs, 'o-', label='크기 차이 (L-R)')
        ax1.set_ylabel('크기 차이 (dB)')
        ax1.set_title('마이크 착용 편차 분석 결과')
        ax1.grid(True)
        ax1.legend()
        
        ax2.semilogx(freqs, phase_diffs, 's-', color='red', label='위상 차이 (L-R)')
        ax2.set_xlabel('주파수 (Hz)')
        ax2.set_ylabel('위상 차이 (도)')
        ax2.grid(True)
        ax2.legend()
        
        plt.tight_layout()
        plt.savefig(os.path.join(plot_dir, 'microphone_deviation_analysis.png'), dpi=150, bbox_inches='tight')
        plt.close()
        
        # 2. 보정 전후 주파수 응답 비교
        fig, ax = plt.subplots(figsize=(12, 6))
        
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
        
        ax.semilogx(freqs_fft, orig_left_db, alpha=0.7, label='원본 좌측', color='blue')
        ax.semilogx(freqs_fft, orig_right_db, alpha=0.7, label='원본 우측', color='red')
        ax.semilogx(freqs_fft, corr_left_db, '--', label='보정 좌측', color='darkblue')
        ax.semilogx(freqs_fft, corr_right_db, '--', label='보정 우측', color='darkred')
        
        ax.set_xlabel('주파수 (Hz)')
        ax.set_ylabel('크기 (dB)')
        ax.set_title('마이크 편차 보정 전후 주파수 응답 비교')
        ax.set_xlim([20, self.fs/2])
        ax.grid(True)
        ax.legend()
        
        plt.tight_layout()
        plt.savefig(os.path.join(plot_dir, 'microphone_deviation_correction_comparison.png'), 
                   dpi=150, bbox_inches='tight')
        plt.close()
        
        print(f"마이크 편차 보정 분석 플롯이 {plot_dir}에 저장되었습니다.")


def apply_microphone_deviation_correction_to_hrir(hrir, 
                                                 correction_strength=0.7,
                                                 plot_analysis=False,
                                                 plot_dir=None):
    """
    HRIR 객체에 마이크 착용 편차 보정 적용
    
    Args:
        hrir (HRIR): HRIR 객체
        correction_strength (float): 보정 강도 (0.0~1.0)
        plot_analysis (bool): 분석 결과 플롯 생성 여부
        plot_dir (str): 플롯 저장 디렉토리
        
    Returns:
        dict: 각 스피커별 분석 결과
    """
    corrector = MicrophoneDeviationCorrector(
        sample_rate=hrir.fs,
        correction_strength=correction_strength
    )
    
    all_analysis_results = {}
    
    for speaker, pair in hrir.irs.items():
        left_ir = pair['left']
        right_ir = pair['right']
        
        # 피크 인덱스 가져오기
        left_peak = left_ir.peak_index()
        right_peak = right_ir.peak_index()
        
        if left_peak is None or right_peak is None:
            print(f"경고: {speaker} 스피커의 피크를 찾을 수 없어 보정을 건너뜁니다.")
            continue
            
        # 보정 적용
        speaker_plot_dir = None
        if plot_analysis and plot_dir:
            speaker_plot_dir = os.path.join(plot_dir, f'microphone_deviation_{speaker}')
            
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
        
        print(f"{speaker} 스피커 마이크 편차 보정 완료")
        
    return all_analysis_results 