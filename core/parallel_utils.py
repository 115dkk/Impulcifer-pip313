# -*- coding: utf-8 -*-
"""
Parallel processing utilities with support for Python 3.13+ free-threaded (no-GIL) mode.

This module provides adaptive parallelization that uses:
- ThreadPoolExecutor for Python 3.13+ with GIL disabled (free-threaded mode)
- ProcessPoolExecutor for standard Python with GIL
"""

import os
import sys
import concurrent.futures.process
from concurrent.futures import ThreadPoolExecutor, ProcessPoolExecutor, as_completed
from typing import Callable, List, Any, Optional


def is_gil_disabled() -> bool:
    """
    Check if Python is running in free-threaded mode (no-GIL).

    Returns:
        True if GIL is disabled (Python 3.13+ free-threaded build), False otherwise
    """
    # Python 3.13+ has sys._is_gil_enabled() function
    if hasattr(sys, '_is_gil_enabled'):
        try:
            return not sys._is_gil_enabled()
        except Exception:
            return False
    return False


def get_optimal_executor(
    max_workers: Optional[int] = None,
    initializer: Optional[Callable] = None,
    initargs: tuple = (),
):
    """
    Get the optimal executor based on Python version and GIL status.

    Args:
        max_workers: Maximum number of workers. If None, uses CPU count.

    Returns:
        ThreadPoolExecutor if GIL is disabled, ProcessPoolExecutor otherwise
    """
    if is_gil_disabled():
        # Python 3.13+ free-threaded: Use threads (much faster, no pickling overhead)
        return ThreadPoolExecutor(max_workers=max_workers, initializer=initializer, initargs=initargs)
    else:
        # Standard Python with GIL: Use processes to bypass GIL
        return ProcessPoolExecutor(max_workers=max_workers, initializer=initializer, initargs=initargs)


def _run_parallel_map(
    func: Callable,
    items: List[Any],
    max_workers: int,
    *,
    use_threads: bool,
    timeout: Optional[float] = None,
    initializer: Optional[Callable] = None,
    initargs: tuple = (),
    show_progress: bool = False,
) -> List[Any]:
    """Shared executor loop behind both public ``parallel_map`` entry points.

    ``max_workers`` must already be resolved by the caller. Executor choice:
    threads when ``use_threads`` is set or the runtime is free-threaded,
    otherwise processes (to bypass the GIL for CPU-bound work). Results are
    always returned in input order. On ProcessPool breakage in a free-threaded
    runtime we retry with threads; standard GIL builds re-raise so CPU-bound
    work is never silently demoted to threads.
    """
    executor_class = (
        ThreadPoolExecutor if (use_threads or is_gil_disabled()) else ProcessPoolExecutor
    )

    def _collect(executor) -> List[Any]:
        if timeout is None and not show_progress:
            # Fast path: submit in order, read results in order. This is
            # byte-for-byte the original ordered collection used by both
            # callers' common (no timeout / no progress) case.
            futures = [executor.submit(func, item) for item in items]
            return [future.result() for future in futures]
        # Timeout / progress path: as-completed with index mapping keeps the
        # output in input order while supporting a timeout and progress output.
        future_to_index = {executor.submit(func, item): i for i, item in enumerate(items)}
        results: List[Any] = [None] * len(items)
        completed = 0
        for future in as_completed(future_to_index, timeout=timeout):
            results[future_to_index[future]] = future.result()
            completed += 1
            if show_progress and completed % max(1, len(items) // 10) == 0:
                progress = completed / len(items) * 100
                print(f"Progress: {progress:.1f}% ({completed}/{len(items)})")
        return results

    try:
        with executor_class(
            max_workers=max_workers,
            initializer=initializer,
            initargs=initargs,
        ) as executor:
            return _collect(executor)
    except (concurrent.futures.process.BrokenProcessPool, RuntimeError):
        if not is_gil_disabled():
            raise
        # Free-threaded Python can safely keep CPU-bound fallback work in threads.
        # Standard GIL builds must keep CPU-bound parallelism in process workers.
        with ThreadPoolExecutor(
            max_workers=max_workers,
            initializer=initializer,
            initargs=initargs,
        ) as executor:
            return _collect(executor)


def parallel_map(
    func: Callable,
    items: List[Any],
    max_workers: Optional[int] = None,
    initializer: Optional[Callable] = None,
    initargs: tuple = (),
) -> List[Any]:
    """
    Execute function on items in parallel using the optimal executor.

    Process-first (GIL bypass) unless the runtime is free-threaded. Results are
    returned in the same order as ``items``. This is the canonical map; the
    thread-first ``core.parallel_processing.parallel_map`` wraps the same shared
    loop (:func:`_run_parallel_map`).

    Args:
        func: Function to execute on each item
        items: List of items to process
        max_workers: Maximum number of workers (None = CPU count)

    Returns:
        List of results in the same order as input items

    Example:
        >>> def process_item(x):
        ...     return x * 2
        >>> results = parallel_map(process_item, [1, 2, 3, 4])
        >>> print(results)
        [2, 4, 6, 8]
    """
    if not items:
        return []

    # 작업 수보다 많은 워커 생성 방지
    if max_workers is None:
        max_workers = min(os.cpu_count() or 4, len(items))

    return _run_parallel_map(
        func,
        items,
        max_workers,
        use_threads=False,
        initializer=initializer,
        initargs=initargs,
    )


def get_parallelization_info() -> dict:
    """
    Get information about current parallelization strategy.

    Returns:
        Dictionary with parallelization details
    """
    gil_disabled = is_gil_disabled()
    executor_type = "ThreadPoolExecutor" if gil_disabled else "ProcessPoolExecutor"

    return {
        "python_version": f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}",
        "gil_disabled": gil_disabled,
        "executor_type": executor_type,
        "parallel_strategy": "threads (no-GIL)" if gil_disabled else "processes (GIL bypass)"
    }


if __name__ == "__main__":
    # Print parallelization info when run directly
    info = get_parallelization_info()
    print("=== Parallelization Configuration ===")
    for key, value in info.items():
        print(f"{key}: {value}")
