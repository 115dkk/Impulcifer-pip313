# -*- coding: utf-8 -*-
"""
Impulcifer 병렬 처리 성능 벤치마크 스크립트
Python 3.14 Free-Threaded 성능 측정

이 스크립트는 Python 3.13과 3.14 Free-Threaded 간의 성능 차이를 측정합니다.
"""

import time
from core.parallel_processing import get_python_threading_info

def print_header():
    """벤치마크 헤더 출력"""
    print("=" * 80)
    print("Impulcifer Python 3.14 병렬 처리 성능 벤치마크")
    print("=" * 80)
    print()

    # Python 정보 출력
    info = get_python_threading_info()
    print("Python 환경 정보:")
    print(f"  Python 버전: {info['python_version']}")
    print(f"  Python 3.14+: {info['is_python_314_plus']}")
    print(f"  Free-Threaded: {info['is_free_threaded']}")
    print(f"  GIL 활성화: {info['gil_enabled']}")
    print(f"  CPU 코어 수: {info['cpu_count']}")
    print(f"  최적 워커 수: {info['optimal_workers']}")
    print()

    if info['is_free_threaded']:
        print("🚀 Free-Threaded 모드 활성화됨 - 진정한 병렬 처리 가능!")
    else:
        print("⚠️  GIL 존재 - Python 3.14 Free-Threaded로 업그레이드하면 성능 향상 가능")
    print()


def benchmark_normalize():
    """HRIR normalize() 함수 벤치마크"""
    print("-" * 80)
    print("테스트 1: HRIR 정규화 (normalize)")
    print("-" * 80)

    try:
        from core.impulse_response_estimator import ImpulseResponseEstimator
        from core.hrir import HRIR
        import numpy as np

        # 테스트 데이터 생성
        print("테스트 데이터 생성 중...")
        estimator = ImpulseResponseEstimator(fs=48000)
        hrir = HRIR(estimator)

        # 16채널 테스트 데이터 (Dolby Atmos 기준)
        speakers = ['FL', 'FR', 'FC', 'SL', 'SR', 'BL', 'BR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR', 'WL', 'WR', 'LFE']
        from core.impulse_response import ImpulseResponse

        for speaker in speakers[:12]:  # 12채널 테스트
            data = np.random.randn(48000)  # 1초 길이 랜덤 데이터
            hrir.irs[speaker] = {
                'left': ImpulseResponse(data=data.copy(), fs=48000),
                'right': ImpulseResponse(data=data.copy(), fs=48000)
            }

        print(f"  채널 수: {len(hrir.irs)} (좌우 합계 {len(hrir.irs)*2})")

        # 벤치마크 실행
        print("\n벤치마크 실행 중...")
        iterations = 5
        times = []

        for i in range(iterations):
            start_time = time.time()
            hrir.normalize(peak_target=-0.1)
            elapsed = time.time() - start_time
            times.append(elapsed)
            print(f"  반복 {i+1}/{iterations}: {elapsed:.4f}s")

        avg_time = sum(times) / len(times)
        print(f"\n평균 시간: {avg_time:.4f}s")
        print("✅ Normalize 벤치마크 완료\n")

    except Exception as e:
        print(f"❌ 벤치마크 실패: {e}\n")


def benchmark_equalization():
    """이퀄라이제이션 병렬 처리 벤치마크"""
    print("-" * 80)
    print("테스트 2: 이퀄라이제이션 (CPU 집약적)")
    print("-" * 80)

    try:
        from core.parallel_processing import parallel_process_dict
        import numpy as np

        # 이퀄라이제이션 시뮬레이션 함수
        def simulate_eq_channel(speaker, data):
            """이퀄라이제이션 시뮬레이션 (FFT/IFFT)"""
            # FFT
            spectrum = np.fft.rfft(data)
            # 주파수 응답 처리 시뮬레이션
            processed = spectrum * (1 + 0.1 * np.random.randn(len(spectrum)))
            # IFFT
            result = np.fft.irfft(processed, n=len(data))
            return result

        # 테스트 데이터
        n_channels = 12
        data_length = 48000 * 2  # 2초
        test_data = {
            f'CH{i:02d}': np.random.randn(data_length)
            for i in range(n_channels)
        }

        print(f"  채널 수: {n_channels}")
        print(f"  데이터 길이: {data_length} 샘플")

        # 순차 처리
        print("\n순차 처리 벤치마크...")
        start_time = time.time()
        {
            speaker: simulate_eq_channel(speaker, data)
            for speaker, data in test_data.items()
        }
        sequential_time = time.time() - start_time
        print(f"  시간: {sequential_time:.4f}s")

        # 병렬 처리
        print("\n병렬 처리 벤치마크...")
        start_time = time.time()
        parallel_process_dict(
            simulate_eq_channel,
            test_data,
            use_threads=True
        )
        parallel_time = time.time() - start_time
        print(f"  시간: {parallel_time:.4f}s")

        # 결과
        speedup = sequential_time / parallel_time if parallel_time > 0 else 0
        print(f"\n속도 향상: {speedup:.2f}x")

        if speedup > 1.5:
            print("🚀 병렬 처리로 인한 유의미한 성능 향상!")
        elif speedup > 1.0:
            print("✅ 병렬 처리로 성능 향상")
        else:
            print("⚠️  병렬 처리 오버헤드로 인한 성능 저하")
            print("   (작은 데이터셋이거나 GIL이 활성화됨)")

        print("\n✅ Equalization 벤치마크 완료\n")

    except Exception as e:
        print(f"❌ 벤치마크 실패: {e}\n")


def benchmark_resample():
    """리샘플링 벤치마크"""
    print("-" * 80)
    print("테스트 3: 리샘플링")
    print("-" * 80)

    try:
        from core.parallel_processing import parallel_process_dict
        import numpy as np
        from scipy import signal

        def simulate_resample(speaker, data):
            """리샘플링 시뮬레이션 (48kHz -> 44.1kHz)"""
            # scipy.signal.resample 사용
            new_length = int(len(data) * 44100 / 48000)
            return signal.resample(data, new_length)

        # 테스트 데이터
        n_channels = 12
        data_length = 48000 * 3  # 3초
        test_data = {
            f'CH{i:02d}': np.random.randn(data_length)
            for i in range(n_channels)
        }

        print(f"  채널 수: {n_channels}")
        print("  원본 샘플링 레이트: 48000Hz")
        print("  타겟 샘플링 레이트: 44100Hz")

        # 순차 처리
        print("\n순차 처리 벤치마크...")
        start_time = time.time()
        {
            speaker: simulate_resample(speaker, data)
            for speaker, data in test_data.items()
        }
        sequential_time = time.time() - start_time
        print(f"  시간: {sequential_time:.4f}s")

        # 병렬 처리
        print("\n병렬 처리 벤치마크...")
        start_time = time.time()
        parallel_process_dict(
            simulate_resample,
            test_data,
            use_threads=True
        )
        parallel_time = time.time() - start_time
        print(f"  시간: {parallel_time:.4f}s")

        # 결과
        speedup = sequential_time / parallel_time if parallel_time > 0 else 0
        print(f"\n속도 향상: {speedup:.2f}x")

        if speedup > 1.5:
            print("🚀 병렬 처리로 인한 유의미한 성능 향상!")
        elif speedup > 1.0:
            print("✅ 병렬 처리로 성능 향상")
        else:
            print("⚠️  병렬 처리 오버헤드")

        print("\n✅ Resample 벤치마크 완료\n")

    except Exception as e:
        print(f"❌ 벤치마크 실패: {e}\n")


def print_summary():
    """요약 출력"""
    print("=" * 80)
    print("벤치마크 완료")
    print("=" * 80)
    print()

    info = get_python_threading_info()

    if info['is_free_threaded']:
        print("🎉 Python 3.14 Free-Threaded 모드에서 실행됨!")
        print("   병렬 처리로 최대 성능을 발휘하고 있습니다.")
    else:
        print("💡 Python 3.14 Free-Threaded로 업그레이드 권장")
        print("   예상 성능 향상: 2~3배")
        print()
        print("설치 방법:")
        print("  1. https://www.python.org/downloads/ 에서 Python 3.14 다운로드")
        print("  2. Free-Threaded 빌드 선택")
        print("  3. pip install impulcifer-py313 재설치")

    print()
    print("상세한 정보는 docs/README_PYTHON314.md를 참고하세요.")
    print()


def main():
    """메인 벤치마크 실행"""
    print_header()

    # 벤치마크 실행
    benchmark_normalize()
    benchmark_equalization()
    benchmark_resample()

    # 요약
    print_summary()


if __name__ == '__main__':
    main()
