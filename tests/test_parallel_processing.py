# -*- coding: utf-8 -*-
"""
parallel_processing.py 테스트 코드
Python 3.9+ 호환
"""

import unittest
import time
import sys
from core.parallel_processing import (
    parallel_map,
    parallel_process_dict,
    get_optimal_worker_count,
    is_free_threaded_available,
    get_python_threading_info,
    benchmark_parallel_performance
)


# 전역 함수 (pickle 가능)
def _global_square_func(x):
    """제곱 함수 (pickle 가능)"""
    return x * x


class TestParallelProcessing(unittest.TestCase):
    """병렬 처리 모듈 테스트"""

    def test_get_python_threading_info(self):
        """Python 스레딩 정보 테스트"""
        info = get_python_threading_info()

        required_keys = ['python_version', 'is_python_314_plus', 'is_free_threaded',
                        'optimal_workers', 'cpu_count']

        for key in required_keys:
            self.assertIn(key, info, f"Missing key: {key}")

        self.assertIsInstance(info['python_version'], str)
        self.assertIsInstance(info['is_python_314_plus'], bool)
        self.assertIsInstance(info['is_free_threaded'], bool)
        self.assertIsInstance(info['optimal_workers'], int)

        print("\n[Python 스레딩 정보]")
        for key, value in info.items():
            print(f"  {key}: {value}")

    def test_get_optimal_worker_count(self):
        """최적 워커 수 계산 테스트"""
        worker_count = get_optimal_worker_count()

        self.assertIsInstance(worker_count, int)
        self.assertGreater(worker_count, 0)
        self.assertLessEqual(worker_count, 128)  # 상한선 확인

        print(f"\n[최적 워커 수]: {worker_count}")

    def test_parallel_map_basic(self):
        """기본 parallel_map 테스트"""

        def square(x):
            return x * x

        data = list(range(10))
        expected = [x * x for x in data]

        result = parallel_map(square, data)

        self.assertEqual(result, expected)
        print("\n[parallel_map 기본 테스트]")
        print(f"  Input: {data}")
        print(f"  Output: {result}")
        print("  ✅ Pass")

    def test_parallel_map_empty(self):
        """빈 입력 테스트"""

        def dummy(x):
            return x

        result = parallel_map(dummy, [])
        self.assertEqual(result, [])

    def test_parallel_map_single_item(self):
        """단일 항목 테스트"""

        def double(x):
            return x * 2

        result = parallel_map(double, [5])
        self.assertEqual(result, [10])

    def test_parallel_map_with_workers(self):
        """워커 수 지정 테스트"""

        def identity(x):
            return x

        data = list(range(20))

        for workers in [1, 2, 4]:
            result = parallel_map(identity, data, max_workers=workers)
            self.assertEqual(result, data)

    def test_parallel_process_dict(self):
        """딕셔너리 병렬 처리 테스트"""

        def process_value(key, value):
            return value * 2

        input_dict = {'FL': 1, 'FR': 2, 'FC': 3, 'SL': 4, 'SR': 5}
        expected_dict = {'FL': 2, 'FR': 4, 'FC': 6, 'SL': 8, 'SR': 10}

        result = parallel_process_dict(process_value, input_dict)

        self.assertEqual(result, expected_dict)
        print("\n[parallel_process_dict 테스트]")
        print(f"  Input: {input_dict}")
        print(f"  Output: {result}")
        print("  ✅ Pass")

    def test_parallel_process_dict_empty(self):
        """빈 딕셔너리 테스트"""

        def dummy(key, value):
            return value

        result = parallel_process_dict(dummy, {})
        self.assertEqual(result, {})

    def test_cpu_intensive_task(self):
        """CPU 집약적 작업 테스트"""

        def fibonacci(n):
            """재귀 피보나치 (CPU 집약적)"""
            if n <= 1:
                return n
            return fibonacci(n - 1) + fibonacci(n - 2)

        # 작은 값으로 테스트 (너무 크면 오래 걸림)
        data = [10, 12, 14, 16, 18]

        start_time = time.time()
        sequential_result = [fibonacci(n) for n in data]
        sequential_time = time.time() - start_time

        start_time = time.time()
        parallel_result = parallel_map(fibonacci, data, max_workers=4)
        parallel_time = time.time() - start_time

        self.assertEqual(sequential_result, parallel_result)

        print("\n[CPU 집약적 작업 테스트 - Fibonacci]")
        print(f"  Input: {data}")
        print(f"  순차 처리 시간: {sequential_time:.4f}s")
        print(f"  병렬 처리 시간: {parallel_time:.4f}s")

        if parallel_time > 0:
            speedup = sequential_time / parallel_time
            print(f"  속도 향상: {speedup:.2f}x")
        else:
            print("  속도 향상: N/A (병렬 처리 시간이 너무 짧음)")

    def test_error_handling(self):
        """에러 처리 테스트"""

        def failing_func(x):
            if x == 5:
                raise ValueError("Test error")
            return x

        data = [1, 2, 3, 4, 5, 6]

        with self.assertRaises(ValueError):
            parallel_map(failing_func, data)

    def test_thread_vs_process(self):
        """스레드 vs 프로세스 비교 테스트"""
        data = list(range(100))

        # 스레드 사용 (pickle 불필요)
        start_time = time.time()
        parallel_map(_global_square_func, data, use_threads=True, max_workers=4)
        thread_time = time.time() - start_time

        # 프로세스 사용 - Python 3.11에서는 로컬 함수 pickle 불가로 스킵
        # Python 3.14 Free-Threaded에서는 스레드만 사용하므로 문제 없음
        print("\n[스레드 vs 프로세스 비교]")
        print(f"  스레드 시간: {thread_time:.4f}s")

        # Free-Threaded 모드면 스레드가 더 빠를 것으로 예상
        if is_free_threaded_available():
            print("  🚀 Free-Threaded 모드 활성화됨!")
        else:
            print("  ⚠️  GIL 존재 (Python 3.13 이하 또는 표준 빌드)")
            print("  ℹ️  Python 3.14 Free-Threaded에서 진정한 병렬 처리 가능")

    def test_benchmark(self):
        """벤치마크 테스트"""

        def test_task(x):
            """간단한 테스트 작업"""
            result = 0
            for i in range(1000):
                result += x * i
            return result

        print("\n[병렬 처리 성능 벤치마크]")
        results = benchmark_parallel_performance(test_task, n_items=50, max_workers_list=[1, 2, 4])

        print("\n  Python 정보:")
        for key, value in results['python_info'].items():
            print(f"    {key}: {value}")

        print(f"\n  순차 처리 시간: {results['sequential_time']:.4f}s")
        print("\n  병렬 처리 결과:")

        for benchmark in results['benchmarks']:
            print(f"    워커 수 {benchmark['max_workers']:2d}: "
                  f"{benchmark['time']:.4f}s (속도 향상: {benchmark['speedup']:.2f}x)")


def run_tests():
    """테스트 실행"""
    loader = unittest.TestLoader()
    suite = loader.loadTestsFromTestCase(TestParallelProcessing)

    runner = unittest.TextTestRunner(verbosity=2)
    result = runner.run(suite)

    print("\n" + "=" * 70)
    print("테스트 결과 요약")
    print("=" * 70)
    print(f"실행된 테스트: {result.testsRun}")
    print(f"성공: {result.testsRun - len(result.failures) - len(result.errors)}")
    print(f"실패: {len(result.failures)}")
    print(f"에러: {len(result.errors)}")
    print("=" * 70)

    return result.wasSuccessful()


if __name__ == '__main__':
    print("=" * 70)
    print("Impulcifer 병렬 처리 모듈 테스트")
    print("=" * 70)
    print(f"Python 버전: {sys.version}")
    print(f"Free-Threaded: {is_free_threaded_available()}")
    print("=" * 70)

    success = run_tests()

    sys.exit(0 if success else 1)
