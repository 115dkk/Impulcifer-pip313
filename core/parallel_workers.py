# -*- coding: utf-8 -*-
"""경량 병렬 처리 워커 함수 모듈.

ProcessPoolExecutor 워커 프로세스가 이 모듈만 import하면 됨.
impulcifer.py의 무거운 import 체인(matplotlib, bokeh, autoeq 등)을 피함.

워커 프로세스 메모리: ~50-80 MB (scipy + numpy) vs ~200-400 MB (impulcifer 전체)
"""
import numpy as np


def process_plot_worker(args):
    """플롯용 컨볼루션 워커. scipy.signal.convolve만 사용.

    Args:
        args: Tuple of (speaker, side, ir_data, test_signal, fs)

    Returns:
        Tuple of (speaker, side, recording)
    """
    speaker, side, ir_data, test_signal, fs = args
    from scipy.signal import convolve
    recording = convolve(test_signal, ir_data, mode="full")
    return (speaker, side, recording)


def process_decay_worker(args):
    """감쇠 조정 워커.

    Args:
        args: Tuple of (speaker, side, ir_data, decay_value, fs)

    Returns:
        Tuple of (speaker, side, adjusted_data)
    """
    speaker, side, ir_data, decay_value, fs = args
    # Lazy import to keep module-level imports minimal
    from core.impulse_response import ImpulseResponse
    temp_ir = ImpulseResponse(data=ir_data.copy(), fs=fs)
    temp_ir.adjust_decay(decay_value)
    return (speaker, side, temp_ir.data)


def process_equalization_worker(args):
    """이퀄라이제이션 워커.

    Args:
        args: Tuple of (speaker, side, ir, room_frs, hp_left, hp_right,
              eq_left, eq_right, target, common_freq, estimator_fs)

    Returns:
        Tuple of (speaker, side, fir_filter)
    """
    (speaker, side, ir, room_frs, hp_left, hp_right,
     eq_left, eq_right, target, common_freq, estimator_fs) = args

    # Lazy import to keep module-level imports minimal
    from autoeq.frequency_response import FrequencyResponse

    # Create frequency response for this speaker-side
    fr = FrequencyResponse(
        name=f'{speaker}-{side} eq',
        frequency=common_freq.copy(),
        raw=0, error=0
    )

    # Apply room correction
    if room_frs is not None and speaker in room_frs and side in room_frs[speaker]:
        fr.error += room_frs[speaker][side].error

    # Apply headphone compensation
    hp_eq = hp_left if side == 'left' else hp_right
    if hp_eq is not None:
        fr.error += hp_eq.error

    # Apply equalization
    eq = eq_left if side == 'left' else eq_right
    if eq is not None and isinstance(eq, FrequencyResponse):
        fr.error += eq.error

    # Remove bass and tilt target from the error
    fr.error -= target.raw

    # Equalize
    fr.equalize(
        max_gain=40,
        treble_f_lower=10000,
        treble_f_upper=estimator_fs / 2,
        window_size=1/3,
        treble_window_size=1/5
    )

    # Create FIR filter
    fir = fr.minimum_phase_impulse_response(fs=estimator_fs, normalize=False, f_res=5)

    return (speaker, side, fir)
