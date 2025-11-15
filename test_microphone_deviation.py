# -*- coding: utf-8 -*-

"""
마이크 착용 편차 보정 기능 테스트 스크립트

이 스크립트는 마이크 편차 보정 기능이 올바르게 작동하는지 테스트합니다.
시뮬레이션된 편차가 있는 임펄스 응답을 생성하고 보정을 적용한 후 결과를 확인합니다.
"""

import numpy as np
import matplotlib.pyplot as plt
from scipy import signal
from microphone_deviation_correction import MicrophoneDeviationCorrector
import os


def generate_test_impulse_response(fs=48000, length_ms=100, peak_delay_samples=1000):
    """테스트용 임펄스 응답 생성"""
    length_samples = int(fs * length_ms / 1000)
    ir = np.zeros(length_samples)
    
    # 주 임펄스 (델타 함수)
    ir[peak_delay_samples] = 1.0
    
    # 몇 개의 반사음 추가 (나중에 도착)
    reflection_delays = [peak_delay_samples + 500, peak_delay_samples + 1200, peak_delay_samples + 2000]
    reflection_gains = [0.3, 0.2, 0.15]
    
    for delay, gain in zip(reflection_delays, reflection_gains):
        if delay < length_samples:
            ir[delay] = gain
            
    # 약간의 노이즈 추가
    noise = np.random.normal(0, 0.001, length_samples)
    ir += noise
    
    return ir


def add_microphone_deviation(ir, fs, deviation_type='frequency_dependent'):
    """임펄스 응답에 마이크 편차 시뮬레이션 추가"""
    if deviation_type == 'frequency_dependent':
        # 주파수 의존적 편차 (고주파에서 더 큰 편차)
        # 간단한 고주파 부스트/컷 필터 적용
        
        # 고주파 부스트 필터 (4kHz 이상)
        sos = signal.butter(2, 4000, btype='high', fs=fs, output='sos')
        high_freq_component = signal.sosfilt(sos, ir)
        
        # 편차 적용 (고주파 성분을 약간 증폭)
        deviation_gain = 1.3  # 30% 증폭
        modified_ir = ir + high_freq_component * (deviation_gain - 1)
        
        return modified_ir
        
    elif deviation_type == 'level_shift':
        # 전체 레벨 시프트
        return ir * 1.2
        
    else:
        return ir


def test_microphone_deviation_correction():
    """마이크 편차 보정 기능 테스트"""
    print("마이크 착용 편차 보정 기능 테스트 시작...")
    
    # 테스트 파라미터
    fs = 48000
    length_ms = 200
    peak_delay = 2000
    
    # 원본 임펄스 응답 생성
    original_ir = generate_test_impulse_response(fs, length_ms, peak_delay)
    
    # 좌우 귀에 서로 다른 편차 추가
    left_ir = add_microphone_deviation(original_ir, fs, 'frequency_dependent')
    right_ir = add_microphone_deviation(original_ir, fs, 'level_shift')
    
    print(f"원본 IR 길이: {len(original_ir)} 샘플")
    print(f"좌측 IR 피크: {np.argmax(np.abs(left_ir))}, 크기: {np.max(np.abs(left_ir)):.4f}")
    print(f"우측 IR 피크: {np.argmax(np.abs(right_ir))}, 크기: {np.max(np.abs(right_ir)):.4f}")
    
    # 보정기 생성
    corrector = MicrophoneDeviationCorrector(
        sample_rate=fs,
        correction_strength=0.8,
        max_correction_db=6.0
    )
    
    # 보정 적용
    test_output_dir = "test_microphone_deviation_output"
    os.makedirs(test_output_dir, exist_ok=True)
    
    corrected_left, corrected_right, analysis = corrector.correct_microphone_deviation(
        left_ir, right_ir,
        plot_analysis=True,
        plot_dir=test_output_dir
    )
    
    print("보정 완료!")
    print(f"보정된 좌측 IR 크기: {np.max(np.abs(corrected_left)):.4f}")
    print(f"보정된 우측 IR 크기: {np.max(np.abs(corrected_right)):.4f}")
    
    # 편차 분석 결과 출력
    print("\n편차 분석 결과:")
    for freq, deviation in analysis['deviations'].items():
        print(f"  {freq} Hz: 크기 차이 = {deviation['magnitude_diff_db']:.2f} dB, "
              f"위상 차이 = {deviation['phase_diff_rad']*180/np.pi:.1f}°")
    
    # 보정 적용 여부 및 통계 출력
    if analysis.get('correction_applied', False):
        print("\n보정 통계:")
        print(f"  - 평균 편차: {analysis.get('avg_deviation_db', 0):.2f} dB")
        print(f"  - 최대 편차: {analysis.get('max_deviation_db', 0):.2f} dB")
        print(f"  - 보정 필터 길이: 좌측 {len(analysis['correction_filters']['left_fir'])} 샘플, "
              f"우측 {len(analysis['correction_filters']['right_fir'])} 샘플")
    else:
        print("\n보정이 적용되지 않았습니다 (편차가 임계값 이하)")
    
    # 게이트 길이 정보 출력
    print("\n게이트 길이 정보:")
    for freq, gate_len in analysis['gate_lengths'].items():
        print(f"  {freq} Hz: {gate_len} 샘플 ({gate_len/fs*1000:.2f} ms)")
    
    # 간단한 비교 플롯 생성
    fig, axes = plt.subplots(2, 2, figsize=(15, 10))
    
    # 시간 축
    time_axis = np.arange(len(original_ir)) / fs * 1000  # ms
    
    # 원본 vs 편차가 있는 IR
    axes[0, 0].plot(time_axis, left_ir, label='좌측 (편차 있음)', alpha=0.7)
    axes[0, 0].plot(time_axis, right_ir, label='우측 (편차 있음)', alpha=0.7)
    axes[0, 0].set_title('보정 전 임펄스 응답')
    axes[0, 0].set_xlabel('시간 (ms)')
    axes[0, 0].set_ylabel('진폭')
    axes[0, 0].legend()
    axes[0, 0].grid(True)
    axes[0, 0].set_xlim([40, 60])  # 피크 주변 확대
    
    # 보정된 IR
    axes[0, 1].plot(time_axis, corrected_left, label='좌측 (보정됨)', alpha=0.7)
    axes[0, 1].plot(time_axis, corrected_right, label='우측 (보정됨)', alpha=0.7)
    axes[0, 1].set_title('보정 후 임펄스 응답')
    axes[0, 1].set_xlabel('시간 (ms)')
    axes[0, 1].set_ylabel('진폭')
    axes[0, 1].legend()
    axes[0, 1].grid(True)
    axes[0, 1].set_xlim([40, 60])  # 피크 주변 확대
    
    # 주파수 응답 비교
    fft_len = 8192
    freqs = np.fft.fftfreq(fft_len, 1/fs)[:fft_len//2]
    
    # 보정 전
    left_fft = np.fft.fft(left_ir, n=fft_len)[:fft_len//2]
    right_fft = np.fft.fft(right_ir, n=fft_len)[:fft_len//2]
    
    axes[1, 0].semilogx(freqs, 20*np.log10(np.abs(left_fft) + 1e-12), label='좌측', alpha=0.7)
    axes[1, 0].semilogx(freqs, 20*np.log10(np.abs(right_fft) + 1e-12), label='우측', alpha=0.7)
    axes[1, 0].set_title('보정 전 주파수 응답')
    axes[1, 0].set_xlabel('주파수 (Hz)')
    axes[1, 0].set_ylabel('크기 (dB)')
    axes[1, 0].legend()
    axes[1, 0].grid(True)
    axes[1, 0].set_xlim([100, 20000])
    
    # 보정 후
    corr_left_fft = np.fft.fft(corrected_left, n=fft_len)[:fft_len//2]
    corr_right_fft = np.fft.fft(corrected_right, n=fft_len)[:fft_len//2]
    
    axes[1, 1].semilogx(freqs, 20*np.log10(np.abs(corr_left_fft) + 1e-12), label='좌측', alpha=0.7)
    axes[1, 1].semilogx(freqs, 20*np.log10(np.abs(corr_right_fft) + 1e-12), label='우측', alpha=0.7)
    axes[1, 1].set_title('보정 후 주파수 응답')
    axes[1, 1].set_xlabel('주파수 (Hz)')
    axes[1, 1].set_ylabel('크기 (dB)')
    axes[1, 1].legend()
    axes[1, 1].grid(True)
    axes[1, 1].set_xlim([100, 20000])
    
    plt.tight_layout()
    plt.savefig(os.path.join(test_output_dir, 'test_comparison.png'), dpi=150, bbox_inches='tight')
    plt.show()
    
    print(f"\n테스트 결과가 '{test_output_dir}' 디렉토리에 저장되었습니다.")
    print("테스트 완료!")


if __name__ == "__main__":
    test_microphone_deviation_correction() 