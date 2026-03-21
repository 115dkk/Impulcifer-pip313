# -*- coding: utf-8 -*-
"""scipy.ndimage shim — N-dimensional image processing using NumPy."""

import numpy as np


def uniform_filter(input, size=3, output=None, mode='reflect', cval=0.0,
                   origin=0, axes=None):
    """Multi-dimensional uniform filter (running mean).

    Simplified implementation for 1D and 2D arrays.
    """
    input = np.asarray(input, dtype=np.float64)

    if np.isscalar(size):
        size = [size] * input.ndim

    result = input.copy()

    for axis in range(input.ndim):
        if size[axis] <= 1:
            continue
        result = _uniform_filter_1d(result, size[axis], axis, mode, cval)

    if output is not None:
        output[...] = result
        return output
    return result


def uniform_filter1d(input, size, axis=-1, output=None, mode='reflect', cval=0.0, origin=0):
    """1D uniform filter along the given axis."""
    input = np.asarray(input, dtype=np.float64)
    result = _uniform_filter_1d(input, size, axis, mode, cval)
    if output is not None:
        output[...] = result
        return output
    return result


def _uniform_filter_1d(arr, size, axis, mode, cval):
    """Apply 1D uniform (mean) filter along an axis."""
    if arr.ndim == 1:
        return _uniform_1d_simple(arr, size, mode, cval)

    # For multi-dimensional, apply along the specified axis
    result = np.empty_like(arr)
    for idx in np.ndindex(*[s for i, s in enumerate(arr.shape) if i != axis]):
        # Build full index
        full_idx = list(idx)
        full_idx.insert(axis, slice(None))
        result[tuple(full_idx)] = _uniform_1d_simple(arr[tuple(full_idx)], size, mode, cval)

    return result


def _uniform_1d_simple(arr, size, mode, cval):
    """Simple 1D running mean matching scipy.ndimage behavior."""
    n = len(arr)
    half = size // 2

    # scipy.ndimage uses 'reflect' mode which reflects without duplicating edge
    # numpy's 'reflect' mode also does this, but scipy's padding differs:
    # scipy pads symmetrically around the edge value
    if mode == 'reflect':
        # scipy ndimage 'reflect': mirrors about the edge value
        # This is numpy's 'symmetric' mode (NOT numpy's 'reflect')
        padded = np.pad(arr, half, mode='symmetric')
    elif mode == 'constant':
        padded = np.pad(arr, half, mode='constant', constant_values=cval)
    elif mode == 'nearest':
        padded = np.pad(arr, half, mode='edge')
    elif mode == 'wrap':
        padded = np.pad(arr, half, mode='wrap')
    elif mode == 'mirror':
        # scipy mirror: d c b | a b c d | c b a
        padded = np.pad(arr, half, mode='symmetric')
    else:
        padded = np.pad(arr, half, mode='reflect')

    # Use direct convolution with uniform kernel for exact behavior
    kernel = np.ones(size) / size
    result = np.convolve(padded, kernel, mode='valid')

    # Ensure output length matches input
    if len(result) > n:
        result = result[:n]
    elif len(result) < n:
        result = np.pad(result, (0, n - len(result)), mode='edge')

    return result
