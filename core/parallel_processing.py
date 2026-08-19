# -*- coding: utf-8 -*-
"""
Free-threaded Python 대응 병렬 처리 유틸리티.

``core.parallel_utils.is_gil_disabled()``를 공유해 Python 3.13+의
free-threaded 빌드를 감지합니다. 일반 Python에서는 기존
``concurrent.futures`` executor로 동작합니다.

주요 기능:
- Free-Threaded Python 자동 감지
- 병렬 맵 함수 (parallel_map)
- CPU 집약적 작업 병렬화
- 하위 호환성 보장
"""

import sys
import os
from typing import Callable, Iterable, List, TypeVar, Optional, Any
import time

from core.parallel_utils import is_gil_disabled, parallel_map as _parallel_map

T = TypeVar('T')
R = TypeVar('R')

PYTHON_VERSION = sys.version_info
IS_PYTHON_314_PLUS = PYTHON_VERSION >= (3, 14)
IS_FREE_THREADED = is_gil_disabled()


def get_optimal_worker_count() -> int:
    """
    최적의 워커 수를 반환합니다.

    Free-Threaded 모드에서는 CPU 코어 수만큼 사용하고,
    일반 모드에서는 CPU 코어 수의 2배를 사용합니다.

    Returns:
        int: 최적의 워커 수
    """
    cpu_count = os.cpu_count() or 4

    if is_gil_disabled():
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
    return is_gil_disabled()


def get_python_threading_info() -> dict:
    """
    현재 Python 인터프리터의 스레딩 정보를 반환합니다.

    Returns:
        dict: 스레딩 관련 정보
    """
    info = {
        'python_version': f"{PYTHON_VERSION.major}.{PYTHON_VERSION.minor}.{PYTHON_VERSION.micro}",
        'is_python_314_plus': IS_PYTHON_314_PLUS,
        'is_free_threaded': is_gil_disabled(),
        'optimal_workers': get_optimal_worker_count(),
        'cpu_count': os.cpu_count() or 'unknown'
    }

    if hasattr(sys, '_is_gil_enabled'):
        info['gil_enabled'] = sys._is_gil_enabled()
    else:
        info['gil_enabled'] = 'unknown'

    return info


def parallel_map(
    func: Callable[[T], R],
    iterable: Iterable[T],
    max_workers: Optional[int] = None,
    timeout: Optional[float] = None,
    initializer: Optional[Callable[..., Any]] = None,
    initargs: tuple = (),
    use_threads: bool = True,
    show_progress: bool = False
) -> List[R]:
    """
    함수를 iterable의 각 항목에 병렬로 적용합니다.

    ``use_threads``가 True이거나 free-threaded 런타임이면
    ThreadPoolExecutor를 사용합니다. 그 외에는 ProcessPoolExecutor를
    사용합니다.

    Args:
        func: 적용할 함수
        iterable: 입력 데이터
        max_workers: 최대 워커 수 (None이면 자동)
        timeout: 타임아웃 (초)
        initializer: 워커 시작 시 1회 실행할 초기화 함수
        initargs: initializer에 전달할 인자 튜플
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
    if len(items) == 1 and initializer is None:
        return [func(items[0])]
    if max_workers is None:
        max_workers = get_optimal_worker_count()
    max_workers = min(max_workers, len(items))

    # core.parallel_utils.parallel_map is the canonical implementation. Keep
    # this wrapper's thread-first default and extended public signature intact.
    return _parallel_map(
        func,
        items,
        max_workers=max_workers,
        initializer=initializer,
        initargs=initargs,
        use_threads=use_threads,
        timeout=timeout,
        show_progress=show_progress,
    )


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

    def wrapper(item):
        key, value = item
        return key, func(key, value)

    results = parallel_map(
        wrapper,
        zip(keys, values),
        max_workers=max_workers,
        timeout=timeout,
        use_threads=use_threads,
        show_progress=show_progress
    )

    return dict(results)


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

    start_time = time.time()
    [func(item) for item in items]
    sequential_time = time.time() - start_time

    results['sequential_time'] = sequential_time

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
    print("=" * 60)
    print("Python 3.14 Free-Threaded 병렬 처리 유틸리티")
    print("=" * 60)

    info = get_python_threading_info()
    print("\n[Python 스레딩 정보]")
    for key, value in info.items():
        print(f"  {key}: {value}")

    print("\n[병렬 처리 테스트]")

    def test_func(x):
        return x * x

    test_data = list(range(10))
    results = parallel_map(test_func, test_data, show_progress=False)
    print(f"  Input: {test_data}")
    print(f"  Output: {results}")

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
