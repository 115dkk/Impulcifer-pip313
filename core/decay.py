"""Lightweight array operations for impulse-response decay adjustment."""


def apply_decay_window(data, params):
    """Apply a precomputed decay window to ``data`` in place."""
    if params is None:
        return data

    window_start, half_window, knee_point_index, window_level = params

    import numpy as np
    from scipy.signal.windows import hann

    window = (
        np.concatenate(
            [
                np.ones(window_start),
                hann(half_window * 2)[half_window:],
                np.zeros(len(data) - knee_point_index),
            ]
        )
        - 1.0
    )
    window *= -window_level
    window = 10 ** (window / 20)
    data *= window
    return data
