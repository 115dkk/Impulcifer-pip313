# -*- coding: utf-8 -*-
"""
Parallel processing utilities with support for Python 3.13+ free-threaded (no-GIL) mode.

This module provides adaptive parallelization that uses:
- ThreadPoolExecutor for Python 3.13+ with GIL disabled (free-threaded mode)
- ProcessPoolExecutor for standard Python with GIL
"""

import sys
from concurrent.futures import ThreadPoolExecutor, ProcessPoolExecutor
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


def get_optimal_executor(max_workers: Optional[int] = None):
    """
    Get the optimal executor based on Python version and GIL status.

    Args:
        max_workers: Maximum number of workers. If None, uses CPU count.

    Returns:
        ThreadPoolExecutor if GIL is disabled, ProcessPoolExecutor otherwise
    """
    if is_gil_disabled():
        # Python 3.13+ free-threaded: Use threads (much faster, no pickling overhead)
        return ThreadPoolExecutor(max_workers=max_workers)
    else:
        # Standard Python with GIL: Use processes to bypass GIL
        return ProcessPoolExecutor(max_workers=max_workers)


def parallel_map(func: Callable, items: List[Any], max_workers: Optional[int] = None) -> List[Any]:
    """
    Execute function on items in parallel using optimal executor.

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

    with get_optimal_executor(max_workers=max_workers) as executor:
        # Submit all tasks and maintain order
        futures = [executor.submit(func, item) for item in items]
        # Collect results in order
        results = [future.result() for future in futures]

    return results


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
