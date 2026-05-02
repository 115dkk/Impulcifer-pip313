# -*- coding: utf-8 -*-
"""경량 병렬 처리 워커 함수 모듈.

ProcessPoolExecutor 워커 프로세스가 이 모듈만 import하면 됨. 그러면 impulcifer.py
나 GUI 트리(bokeh, customtkinter, seaborn 등)는 워커 측에서 로드되지 않는다.
플롯/decay 워커는 scipy + numpy만 끌어오고, EQ 워커는 lazy import로
``autoeq.frequency_response``를 통해 matplotlib + Pillow + tabulate까지 끌어온다.
"""
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

    # Smoothen and equalize using the same AutoEQ pipeline as LionLion123/Impulcifer.
    fr.smoothen_heavy_light()
    fr.equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=estimator_fs / 2)

    # Create FIR filter
    fir = fr.minimum_phase_impulse_response(fs=estimator_fs, normalize=False, f_res=5)

    return (speaker, side, fir)
