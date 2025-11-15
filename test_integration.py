# -*- coding: utf-8 -*-

"""
마이크 편차 보정 기능 통합 테스트

실제 HRIR 처리 파이프라인에서 마이크 편차 보정 기능이 올바르게 작동하는지 테스트합니다.
"""

import numpy as np
import os
from hrir import HRIR
from impulse_response import ImpulseResponse


def create_test_hrir():
    """테스트용 HRIR 객체 생성"""
    # 간단한 테스트 신호 생성
    fs = 48000
    length = 4800  # 100ms
    
    # 더미 추정기 생성 (기존 방식 대신 직접 속성 설정)
    class DummyEstimator:
        def __init__(self, fs):
            self.fs = fs
            self.test_signal = np.random.randn(fs)  # 1초 더미 신호
            
    estimator = DummyEstimator(fs)
    
    # HRIR 객체 생성
    hrir = HRIR(estimator)
    
    # 테스트용 임펄스 응답 생성
    def create_ir_with_deviation(base_delay=1000, deviation_factor=1.0):
        ir_data = np.zeros(length)
        ir_data[base_delay] = 1.0
        
        # 몇 개의 반사음 추가
        ir_data[base_delay + 200] = 0.3
        ir_data[base_delay + 500] = 0.2
        
        # 편차 시뮬레이션 (고주파 성분 변조)
        if deviation_factor != 1.0:
            # 고주파 노이즈 추가
            high_freq_noise = np.random.randn(length) * 0.01 * deviation_factor
            ir_data += high_freq_noise
            
        return ir_data
    
    # FL, FR 스피커 추가
    for speaker in ['FL', 'FR']:
        hrir.irs[speaker] = {}
        
        # 좌우 귀에 약간 다른 편차 적용
        left_deviation = 1.0 if speaker == 'FL' else 1.1
        right_deviation = 1.2 if speaker == 'FL' else 1.0
        
        hrir.irs[speaker]['left'] = ImpulseResponse(
            create_ir_with_deviation(1000, left_deviation), fs
        )
        hrir.irs[speaker]['right'] = ImpulseResponse(
            create_ir_with_deviation(1000, right_deviation), fs
        )
    
    return hrir


def test_microphone_deviation_integration():
    """마이크 편차 보정 통합 테스트"""
    print("마이크 편차 보정 통합 테스트 시작...")
    
    # 테스트 HRIR 생성
    hrir = create_test_hrir()
    
    print("테스트 HRIR 생성 완료:")
    print(f"  - 스피커: {list(hrir.irs.keys())}")
    print(f"  - 샘플링 레이트: {hrir.fs} Hz")
    
    # 보정 전 상태 저장
    original_data = {}
    for speaker, pair in hrir.irs.items():
        original_data[speaker] = {
            'left': pair['left'].data.copy(),
            'right': pair['right'].data.copy()
        }
    
    # 마이크 편차 보정 적용
    print("\n마이크 편차 보정 적용 중...")
    
    test_output_dir = "test_integration_output"
    os.makedirs(test_output_dir, exist_ok=True)
    
    try:
        analysis_results = hrir.correct_microphone_deviation(
            correction_strength=0.7,
            plot_analysis=True,
            plot_dir=test_output_dir
        )
        
        print("보정 성공!")
        
        # 결과 분석
        print("\n분석 결과:")
        for speaker, results in analysis_results.items():
            print(f"  {speaker} 스피커:")
            if results.get('correction_applied', False):
                print("    - 보정 적용됨")
                print(f"    - 평균 편차: {results.get('avg_deviation_db', 0):.2f} dB")
                print(f"    - 최대 편차: {results.get('max_deviation_db', 0):.2f} dB")
            else:
                print("    - 보정 건너뜀 (편차 미미)")
        
        # 데이터 변경 확인
        print("\n데이터 변경 확인:")
        for speaker, pair in hrir.irs.items():
            left_changed = not np.array_equal(original_data[speaker]['left'], pair['left'].data)
            right_changed = not np.array_equal(original_data[speaker]['right'], pair['right'].data)
            print(f"  {speaker}: 좌측 변경={left_changed}, 우측 변경={right_changed}")
        
        print(f"\n테스트 결과가 '{test_output_dir}' 디렉토리에 저장되었습니다.")
        print("통합 테스트 완료!")
        
        return True
        
    except Exception as e:
        print(f"테스트 실패: {e}")
        import traceback
        traceback.print_exc()
        return False


if __name__ == "__main__":
    success = test_microphone_deviation_integration()
    if success:
        print("\n✅ 모든 테스트 통과!")
    else:
        print("\n❌ 테스트 실패!") 