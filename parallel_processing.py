# -*- coding: utf-8 -*-
"""
Python 3.14 Free-Threaded 병렬 처리 유틸리티
Python 3.13+ 호환 (하위 버전은 concurrent.futures 폴백)

Python 3.14의 Free-Threaded Python (PEP 703)을 활용하여
GIL 없이 진정한 병렬 처리를 수행합니다.

주요 기능:
- Free-Threaded Python 자동 감지
- 병렬 맵 함수 (parallel_map)
- CPU 집약적 작업 병렬화
- 하위 호환성 보장
"""

import sys
import os
import concurrent.futures
from typing import Callable, Iterable, List, TypeVar, Optional, Any
from functools import wraps
import time

# 타입 변수 정의
T = TypeVar('T')
R = TypeVar('R')

# Python 버전 및 Free-Threaded 지원 확인
PYTHON_VERSION = sys.version_info
IS_PYTHON_314_PLUS = PYTHON_VERSION >= (3, 14)
IS_FREE_THREADED = False

# Python 3.14+ Free-Threaded 지원 확인
if IS_PYTHON_314_PLUS:
    try:
        # sys._is_gil_enabled()가 False면 Free-Threaded 모드
        # PEP 703: Free-Threaded Python
        IS_FREE_THREADED = hasattr(sys, '_is_gil_enabled') and not sys._is_gil_enabled()
    except AttributeError:
        IS_FREE_THREADED = False


def get_optimal_worker_count() -> int:
    """
    최적의 워커 수를 반환합니다.

    Free-Threaded 모드에서는 CPU 코어 수만큼 사용하고,
    일반 모드에서는 CPU 코어 수의 2배를 사용합니다.

    Returns:
        int: 최적의 워커 수
    """
    cpu_count = os.cpu_count() or 4

    if IS_FREE_THREADED:
        # Free-Threaded: CPU 집약적 작업에 최적화
        return cpu_count
    else:
        # GIL 존재: I/O 바운드 작업에 최적화
        return min(cpu_count * 2, 32)


def is_free_threaded_available() -> bool:
    """
    Free-Threaded Python 사용 가능 여부를 반환합니다.

    Returns:
        bool: Free-Threaded 사용 가능 여부
    """
    return IS_FREE_THREADED


def get_python_threading_info() -> dict:
    """
    현재 Python 인터프리터의 스레딩 정보를 반환합니다.

    Returns:
        dict: 스레딩 관련 정보
    """
    info = {
        'python_version': f"{PYTHON_VERSION.major}.{PYTHON_VERSION.minor}.{PYTHON_VERSION.micro}",
        'is_python_314_plus': IS_PYTHON_314_PLUS,
        'is_free_threaded': IS_FREE_THREADED,
        'optimal_workers': get_optimal_worker_count(),
        'cpu_count': os.cpu_count() or 'unknown'
    }

    if IS_PYTHON_314_PLUS and hasattr(sys, '_is_gil_enabled'):
        info['gil_enabled'] = sys._is_gil_enabled()
    else:
        info['gil_enabled'] = 'unknown (pre-3.14)'

    return info


def parallel_map(
    func: Callable[[T], R],
    iterable: Iterable[T],
    max_workers: Optional[int] = None,
    timeout: Optional[float] = None,
    use_threads: bool = True,
    show_progress: bool = False
) -> List[R]:
    """
    함수를 iterable의 각 항목에 병렬로 적용합니다.

    Python 3.14 Free-Threaded 모드에서는 ThreadPoolExecutor를 사용하여
    진정한 병렬 처리를 수행하고, 이전 버전에서는 ProcessPoolExecutor로
    폴백하여 병렬 처리를 수행합니다.

    Args:
        func: 적용할 함수
        iterable: 입력 데이터
        max_workers: 최대 워커 수 (None이면 자동)
        timeout: 타임아웃 (초)
        use_threads: True면 스레드 사용, False면 프로세스 사용
        show_progress: 진행 상황 표시 여부

    Returns:
        List[R]: 결과 리스트

    Example:
        >>> def square(x):
        ...     return x * x
        >>> parallel_map(square, range(10))
        [0, 1, 4, 9, 16, 25, 36, 49, 64, 81]
    """
    items = list(iterable)

    if not items:
        return []

    # 단일 항목이면 병렬 처리 불필요
    if len(items) == 1:
        return [func(items[0])]

    # 워커 수 결정
    if max_workers is None:
        max_workers = get_optimal_worker_count()

    # 항목 수가 워커 수보다 적으면 조정
    max_workers = min(max_workers, len(items))

    # 병렬 처리 수행
    executor_class = (
        concurrent.futures.ThreadPoolExecutor
        if use_threads or IS_FREE_THREADED
        else concurrent.futures.ProcessPoolExecutor
    )

    start_time = time.time()
    results = []

    with executor_class(max_workers=max_workers) as executor:
        # 병렬 실행
        futures = {executor.submit(func, item): i for i, item in enumerate(items)}

        # 결과 수집
        completed_count = 0
        results = [None] * len(items)

        for future in concurrent.futures.as_completed(futures, timeout=timeout):
            idx = futures[future]
            try:
                results[idx] = future.result()
                completed_count += 1

                if show_progress and completed_count % max(1, len(items) // 10) == 0:
                    progress = completed_count / len(items) * 100
                    print(f"Progress: {progress:.1f}% ({completed_count}/{len(items)})")

            except Exception as exc:
                print(f"Item {idx} generated an exception: {exc}")
                raise

    elapsed_time = time.time() - start_time

    if show_progress:
        speedup = len(items) / elapsed_time if elapsed_time > 0 else 0
        print(f"Completed {len(items)} items in {elapsed_time:.2f}s ({speedup:.1f} items/s)")
        print(f"Workers used: {max_workers}, Free-threaded: {IS_FREE_THREADED}")

    return results


def parallel_process_dict(
    func: Callable[[str, Any], Any],
    data_dict: dict,
    max_workers: Optional[int] = None,
    timeout: Optional[float] = None,
    use_threads: bool = True,
    show_progress: bool = False
) -> dict:
    """
    딕셔너리의 각 키-값 쌍에 함수를 병렬로 적용합니다.

    Args:
        func: 적용할 함수 (key, value) -> result
        data_dict: 입력 딕셔너리
        max_workers: 최대 워커 수
        timeout: 타임아웃 (초)
        use_threads: True면 스레드 사용, False면 프로세스 사용
        show_progress: 진행 상황 표시 여부

    Returns:
        dict: 결과 딕셔너리

    Example:
        >>> def process_pair(key, value):
        ...     return value * 2
        >>> parallel_process_dict(process_pair, {'a': 1, 'b': 2, 'c': 3})
        {'a': 2, 'b': 4, 'c': 6}
    """
    if not data_dict:
        return {}

    keys = list(data_dict.keys())
    values = list(data_dict.values())

    # 키-값 쌍을 함수에 전달하는 래퍼
    def wrapper(item):
        key, value = item
        return key, func(key, value)

    # 병렬 처리
    results = parallel_map(
        wrapper,
        zip(keys, values),
        max_workers=max_workers,
        timeout=timeout,
        use_threads=use_threads,
        show_progress=show_progress
    )

    # 딕셔너리로 변환
    return dict(results)


def enable_parallel_processing(func: Callable) -> Callable:
    """
    함수를 병렬 처리 가능하게 만드는 데코레이터입니다.

    이 데코레이터를 사용하면 함수가 자동으로 병렬 처리를 활용합니다.
    첫 번째 인자가 iterable인 경우에만 병렬 처리를 수행합니다.

    Args:
        func: 병렬화할 함수

    Returns:
        Callable: 병렬 처리가 가능한 함수

    Example:
        >>> @enable_parallel_processing
        ... def process_items(items):
        ...     return [item * 2 for item in items]
    """
    @wraps(func)
    def wrapper(*args, **kwargs):
        # parallel 파라미터 확인
        use_parallel = kwargs.pop('use_parallel', False)
        max_workers = kwargs.pop('max_workers', None)

        if not use_parallel:
            return func(*args, **kwargs)

        # 첫 번째 인자가 iterable인지 확인
        if args and hasattr(args[0], '__iter__'):
            iterable = args[0]
            remaining_args = args[1:]

            # 병렬 처리 함수 생성
            def parallel_func(item):
                return func(item, *remaining_args, **kwargs)

            return parallel_map(parallel_func, iterable, max_workers=max_workers)
        else:
            return func(*args, **kwargs)

    return wrapper


# 성능 벤치마크 함수
def benchmark_parallel_performance(
    func: Callable[[int], Any],
    n_items: int = 100,
    max_workers_list: Optional[List[int]] = None
) -> dict:
    """
    병렬 처리 성능을 벤치마크합니다.

    Args:
        func: 테스트할 함수
        n_items: 테스트 항목 수
        max_workers_list: 테스트할 워커 수 리스트

    Returns:
        dict: 벤치마크 결과
    """
    if max_workers_list is None:
        max_workers_list = [1, 2, 4, 8, get_optimal_worker_count()]

    results = {
        'python_info': get_python_threading_info(),
        'benchmarks': []
    }

    items = list(range(n_items))

    # 순차 처리
    start_time = time.time()
    [func(item) for item in items]
    sequential_time = time.time() - start_time

    results['sequential_time'] = sequential_time

    # 병렬 처리
    for max_workers in max_workers_list:
        start_time = time.time()
        parallel_map(func, items, max_workers=max_workers)
        parallel_time = time.time() - start_time

        speedup = sequential_time / parallel_time if parallel_time > 0 else 0

        results['benchmarks'].append({
            'max_workers': max_workers,
            'time': parallel_time,
            'speedup': speedup
        })

    return results


if __name__ == '__main__':
    # 테스트 및 정보 출력
    print("=" * 60)
    print("Python 3.14 Free-Threaded 병렬 처리 유틸리티")
    print("=" * 60)

    info = get_python_threading_info()
    print("\n[Python 스레딩 정보]")
    for key, value in info.items():
        print(f"  {key}: {value}")

    print("\n[병렬 처리 테스트]")

    # 간단한 테스트
    def test_func(x):
        return x * x

    test_data = list(range(10))
    results = parallel_map(test_func, test_data, show_progress=False)
    print(f"  Input: {test_data}")
    print(f"  Output: {results}")

    # 딕셔너리 병렬 처리 테스트
    print("\n[딕셔너리 병렬 처리 테스트]")

    def process_speaker(key, value):
        return value * 2

    test_dict = {'FL': 1, 'FR': 2, 'FC': 3, 'SL': 4, 'SR': 5}
    result_dict = parallel_process_dict(process_speaker, test_dict, show_progress=False)
    print(f"  Input: {test_dict}")
    print(f"  Output: {result_dict}")

    print("\n" + "=" * 60)
    print("테스트 완료!")
    print("=" * 60)
