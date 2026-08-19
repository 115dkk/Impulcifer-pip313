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
    """감쇠 조정 윈도우를 배열에 적용하는 워커.

    Args:
        args: Tuple of (speaker, side, ir_data, adjustment_params)

    Returns:
        Tuple of (speaker, side, adjusted_data)
    """
    speaker, side, ir_data, adjustment_params = args
    from core.decay import apply_decay_window

    adjusted_data = ir_data.copy()
    apply_decay_window(adjusted_data, adjustment_params)
    return (speaker, side, adjusted_data)


_EQUALIZATION_CONTEXT = None


def init_equalization_worker(
    room_frs,
    hp_left,
    hp_right,
    eq_left,
    eq_right,
    target,
    common_freq,
    estimator_fs,
):
    """Install equalization data once per worker process/thread."""
    global _EQUALIZATION_CONTEXT
    _EQUALIZATION_CONTEXT = (
        room_frs,
        hp_left,
        hp_right,
        eq_left,
        eq_right,
        target,
        common_freq,
        estimator_fs,
    )


def process_equalization_worker(args):
    """이퀄라이제이션 워커.

    Args:
        args: Tuple of (speaker, side, room_frs, hp_left, hp_right,
              eq_left, eq_right, target, common_freq, estimator_fs)

    Returns:
        Tuple of (speaker, side, fir_filter)
    """
    if len(args) == 2:
        if _EQUALIZATION_CONTEXT is None:
            raise RuntimeError("Equalization worker context was not initialized.")
        speaker, side = args
        (
            room_frs,
            hp_left,
            hp_right,
            eq_left,
            eq_right,
            target,
            common_freq,
            estimator_fs,
        ) = _EQUALIZATION_CONTEXT
    else:
        (speaker, side, room_frs, hp_left, hp_right,
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


def render_hrir_figure_worker(args):
    """스피커-사이드 한 장의 6패널 figure를 렌더링하는 워커.

    matplotlib figure(특히 3D waterfall/pcolormesh)는 수십만 개의 소형
    객체로 힙을 조각내 close 후에도 RSS가 잔류하므로, 렌더링을 워커
    프로세스에 격리해 종료 시 OS에 메모리를 반환한다.

    Args:
        args: (speaker, side, ir_data, recording, fs, plot_flags,
               sync_limits, out_path, suptitle) 튜플.
              plot_flags는 ImpulseResponse.plot()의 plot_* 키워드 dict.
              sync_limits가 None이면 축 한계 수집 패스(저장 없음),
              아니면 {axis_idx: {'xlim': [..], 'ylim': [..]}}를 적용해
              out_path에 PNG로 저장하는 렌더 패스.

    Returns:
        (speaker, side, limits). limits는 축 idx 0..5 순서의
        (x_min, x_max, y_min, y_max) 리스트 (축이 없으면 None).
    """
    (speaker, side, ir_data, recording, fs,
     plot_flags, sync_limits, out_path, suptitle) = args

    import gc

    import matplotlib
    matplotlib.use("Agg", force=True)
    import matplotlib.pyplot as plt
    try:
        # 파이프라인은 플롯 활성화 시 seaborn whitegrid 테마를 적용하므로
        # 워커에서도 동일하게 맞춰 기존 출력과 시각적으로 동일하게 유지한다.
        import seaborn as sns
        sns.set_theme(style="whitegrid")
    except ImportError:
        pass

    from core.impulse_response import ImpulseResponse

    ir = ImpulseResponse(ir_data, fs)
    fig = ir.plot(recording=recording, **plot_flags)
    axes_list = fig.get_axes()

    limits = []
    for idx in range(6):
        if idx < len(axes_list):
            xlim = axes_list[idx].get_xlim()
            ylim = axes_list[idx].get_ylim()
            limits.append((xlim[0], xlim[1], ylim[0], ylim[1]))
        else:
            limits.append(None)

    if sync_limits is not None:
        if suptitle:
            fig.suptitle(suptitle)
        for idx, sl in sync_limits.items():
            if idx < len(axes_list):
                axes_list[idx].set_xlim(sl["xlim"])
                axes_list[idx].set_ylim(sl["ylim"])
        if out_path is not None:
            from core.plotting_utils import save_fig_as_png
            save_fig_as_png(out_path, fig, n_colors=60)

    plt.close(fig)
    gc.collect()
    return (speaker, side, limits)
