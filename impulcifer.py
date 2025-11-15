# -*- coding: utf-8 -*-

import os
import re
import argparse
from tabulate import tabulate
from datetime import datetime
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
from autoeq.frequency_response import FrequencyResponse
from impulse_response_estimator import ImpulseResponseEstimator
from hrir import HRIR, _get_center_value
from room_correction import room_correction
from utils import (
    sync_axes,
    save_fig_as_png,
    is_truehd_file,
    convert_truehd_to_wav,
    check_ffmpeg_available,
)
from constants import (
    SPEAKER_NAMES,
    SPEAKER_LIST_PATTERN,
    HESUVI_TRACK_ORDER,
    TEST_SIGNALS,
    get_data_path,
    TRUEHD_11CH_ORDER,
    TRUEHD_13CH_ORDER,
)
from parallel_utils import parallel_map, get_parallelization_info
from channel_generation import (
    get_available_channels_for_layout,
    create_truehd_layout_track_order,
    validate_channel_requirements,
)
from logger import get_logger

# PR3에서 추가된 import 문들
import copy
import contextlib
import io
from scipy.interpolate import interp1d  # 큐빅 보간을 위해 추가

# Bokeh Tabs/Panel import 추가
# from bokeh.models import Panel, Tabs # 이전 시도
from bokeh.models import TabPanel, Tabs  # 수정: Panel -> TabPanel
from bokeh.plotting import (
    output_file as bokeh_output_file,
    save as bokeh_save,
)  # 중복 방지

# 한글 폰트 설정 추가
import matplotlib.font_manager as fm
import platform
import importlib.resources  # 패키지 리소스 접근을 위해 추가

# Python 3.14 병렬 처리 지원
try:
    from parallel_processing import parallel_process_dict, is_free_threaded_available

    PARALLEL_PROCESSING_AVAILABLE = True
except ImportError:
    PARALLEL_PROCESSING_AVAILABLE = False
    parallel_process_dict = None

    def is_free_threaded_available():
        return False


# 운영체제별 기본 폰트 설정
def set_matplotlib_font():
    system = platform.system()
    font_name_pretendard = "Pretendard"
    font_loaded_pretendard = False

    plt.rcParams["axes.unicode_minus"] = False  # 마이너스 부호 문제 해결

    # Pretendard 폰트를 찾을 수 있는 여러 경로 시도
    font_search_paths = []

    try:
        # 1. 패키지 내 폰트 시도
        try:
            # Python 3.9+ 에서는 files() 사용
            if hasattr(importlib.resources, "files"):
                try:
                    # 설치된 패키지에서 시도
                    font_resource = (
                        importlib.resources.files("impulcifer_py313")
                        .joinpath("font")
                        .joinpath("Pretendard-Regular.otf")
                    )
                    font_search_paths.append(("bundled (files)", font_resource))
                except (FileNotFoundError, ModuleNotFoundError):
                    pass

            # Python 3.7, 3.8 호환 (path 사용)
            elif hasattr(importlib.resources, "path"):
                try:
                    font_resource = importlib.resources.path(
                        "impulcifer_py313.font", "Pretendard-Regular.otf"
                    )
                    font_search_paths.append(("bundled (path)", font_resource))
                except (FileNotFoundError, ModuleNotFoundError):
                    pass
        except ImportError:
            pass

        # 2. 로컬 개발 환경에서 시도
        script_dir = os.path.dirname(os.path.abspath(__file__))
        local_font_paths = [
            os.path.join(script_dir, "font", "Pretendard-Regular.otf"),  # font/
            os.path.join(script_dir, "fonts", "Pretendard-Regular.otf"),  # fonts/
            os.path.join(
                script_dir, "..", "font", "Pretendard-Regular.otf"
            ),  # 상위 디렉토리
            os.path.join(
                script_dir, "..", "fonts", "Pretendard-Regular.otf"
            ),  # 상위 디렉토리
        ]

        for local_path in local_font_paths:
            if os.path.exists(local_path):
                font_search_paths.append(("local", local_path))
                break  # 첫 번째로 찾은 것만 사용

        # 3. 시스템 전역에서 Pretendard 폰트 찾기
        try:
            # matplotlib의 fontManager를 사용해서 시스템에 설치된 Pretendard 찾기
            available_fonts = [f.name for f in fm.fontManager.ttflist]
            if "Pretendard" in available_fonts:
                font_search_paths.append(("system", "Pretendard"))
        except Exception:
            pass

        # 폰트 로딩 시도
        for source_type, font_path in font_search_paths:
            try:
                if source_type in ["bundled (files)"]:
                    with importlib.resources.as_file(font_path) as font_file_path:
                        fm.fontManager.addfont(str(font_file_path))
                        prop = fm.FontProperties(fname=str(font_file_path))
                        font_name_pretendard = prop.get_name()
                        plt.rcParams["font.family"] = font_name_pretendard
                        font_loaded_pretendard = True
                        # Font loading success (debug level, not critical)
                        pass
                        break
                elif source_type in ["bundled (path)"]:
                    with font_path as font_file_path:
                        fm.fontManager.addfont(str(font_file_path))
                        prop = fm.FontProperties(fname=str(font_file_path))
                        font_name_pretendard = prop.get_name()
                        plt.rcParams["font.family"] = font_name_pretendard
                        font_loaded_pretendard = True
                        # Font loading success (debug level, not critical)
                        pass
                        break
                elif source_type == "local":
                    fm.fontManager.addfont(font_path)
                    prop = fm.FontProperties(fname=font_path)
                    font_name_pretendard = prop.get_name()
                    plt.rcParams["font.family"] = font_name_pretendard
                    font_loaded_pretendard = True
                    print(f"Pretendard 폰트 로딩 성공 ({source_type}): {font_path}")
                    break
                elif source_type == "system":
                    plt.rcParams["font.family"] = "Pretendard"
                    font_loaded_pretendard = True
                    print(
                        f"Pretendard 폰트 로딩 성공 ({source_type}): 시스템 설치된 폰트"
                    )
                    break
            except Exception:
                # Font loading failure (not critical, suppress message)
                pass
                continue

    except Exception:
        # Font search error (not critical, suppress)
        pass

    # Pretendard 로딩 실패 시 시스템 기본 폰트 사용
    if not font_loaded_pretendard:
        # Pretendard font not found, using system default (suppress message)
        pass
        if system == "Windows":
            font_path_win = "C:/Windows/Fonts/malgun.ttf"
            if os.path.exists(font_path_win):
                font_prop = fm.FontProperties(fname=font_path_win)
                plt.rcParams["font.family"] = font_prop.get_name()
                pass  # System font loaded
            else:
                plt.rcParams["font.family"] = "Malgun Gothic"
                pass  # Malgun Gothic fallback
        elif system == "Darwin":
            plt.rcParams["font.family"] = "AppleGothic"
            pass  # AppleGothic
        elif system == "Linux":
            plt.rcParams["font.family"] = "NanumGothic"
            pass  # NanumGothic
        else:
            pass  # Unknown system, using matplotlib default


def get_pretendard_font_for_gui():
    """GUI에서 사용할 Pretendard 폰트 경로를 반환합니다."""
    try:
        # 1. 패키지 내 폰트 시도
        try:
            if hasattr(importlib.resources, "files"):
                try:
                    font_resource = (
                        importlib.resources.files("impulcifer_py313")
                        .joinpath("font")
                        .joinpath("Pretendard-Regular.otf")
                    )
                    with importlib.resources.as_file(font_resource) as font_file_path:
                        return str(font_file_path)
                except (FileNotFoundError, ModuleNotFoundError):
                    pass

            elif hasattr(importlib.resources, "path"):
                try:
                    with importlib.resources.path(
                        "impulcifer_py313.font", "Pretendard-Regular.otf"
                    ) as font_file_path:
                        return str(font_file_path)
                except (FileNotFoundError, ModuleNotFoundError):
                    pass
        except ImportError:
            pass

        # 2. 로컬 개발 환경에서 시도
        script_dir = os.path.dirname(os.path.abspath(__file__))
        local_font_paths = [
            os.path.join(script_dir, "font", "Pretendard-Regular.otf"),
            os.path.join(script_dir, "fonts", "Pretendard-Regular.otf"),
            os.path.join(script_dir, "..", "font", "Pretendard-Regular.otf"),
            os.path.join(script_dir, "..", "fonts", "Pretendard-Regular.otf"),
        ]

        for local_path in local_font_paths:
            if os.path.exists(local_path):
                return local_path

        # 3. 시스템에 설치된 Pretendard 사용
        try:
            available_fonts = [f.name for f in fm.fontManager.ttflist]
            if "Pretendard" in available_fonts:
                return "Pretendard"  # 시스템 폰트 이름 반환
        except Exception:
            pass

    except Exception:
        pass  # GUI font search error (not critical)

    return None


set_matplotlib_font()  # 함수 호출하여 폰트 설정 실행


# 큐빅 스플라인 보간 적용 헬퍼 함수
def _apply_cubic_interp(
    fr_obj, target_freqs, fallback_interpolate_method_ref, operation_description=""
):
    """FrequencyResponse 객체에 큐빅 스플라인 보간을 적용합니다.
    실패 시 제공된 폴백 메소드를 사용합니다.
    """
    if fr_obj is None:
        return

    source_freqs = fr_obj.frequency
    source_raw = fr_obj.raw
    fr_obj.name if hasattr(fr_obj, "name") else "FrequencyResponse object"

    if (
        len(source_freqs) > 3 and len(source_raw) > 3
    ):  # interp1d 'cubic'은 최소 4개의 포인트 필요
        unique_src_freqs, unique_indices = np.unique(source_freqs, return_index=True)
        unique_src_raw = source_raw[unique_indices]

        if len(unique_src_freqs) > 3:
            try:
                # 경계값으로 fill_value를 설정하여 외삽 시 안정성 확보
                fill_val = (unique_src_raw[0], unique_src_raw[-1])
                interp_func = interp1d(
                    unique_src_freqs,
                    unique_src_raw,
                    kind="cubic",
                    bounds_error=False,
                    fill_value=fill_val,
                )

                new_raw = interp_func(target_freqs)
                fr_obj.raw = new_raw
                fr_obj.frequency = target_freqs.copy()
                # print(f"Successfully applied cubic interpolation{desc}.") # 필요시 주석 해제
                return True  # 성공
            except ValueError:
                # Cubic interpolation failed, using fallback (suppress message)
                pass
        else:
            # Not enough unique data points (suppress warning)
            pass
    else:
        # Not enough data points for cubic interpolation (suppress warning)
        pass

    # 큐빅 보간 실패 시 폴백
    try:
        fallback_interpolate_method_ref()
        # print(f"Fallback interpolation applied{desc}.") # 필요시 주석 해제
    except Exception:
        # Error in fallback interpolation (suppress error)
        pass
    return False  # 실패 또는 폴백 사용


# ============================================================================
# Parallel Processing Worker Functions (Phase 2 Optimization)
# ============================================================================

def _process_equalization_worker(args):
    """
    Worker function for parallel equalization processing.

    Args:
        args: Tuple of (speaker, side, ir, room_frs, hp_left, hp_right,
              eq_left, eq_right, target, common_freq, estimator_fs)

    Returns:
        Tuple of (speaker, side, fir_filter)
    """
    (speaker, side, ir, room_frs, hp_left, hp_right,
     eq_left, eq_right, target, common_freq, estimator_fs) = args

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


def _process_decay_worker(args):
    """
    Worker function for parallel decay adjustment.

    Args:
        args: Tuple of (speaker, side, ir_data, decay_value, fs)

    Returns:
        Tuple of (speaker, side, adjusted_data)
    """
    speaker, side, ir_data, decay_value, fs = args

    # Import here to avoid circular dependency in multiprocessing
    from impulse_response import ImpulseResponse

    # Create temporary IR object
    temp_ir = ImpulseResponse(name=f'{speaker}-{side}', data=ir_data.copy(), fs=fs)
    temp_ir.adjust_decay(decay_value)

    return (speaker, side, temp_ir.data)


def _process_plot_worker(args):
    """
    Worker function for parallel plotting convolution.

    Args:
        args: Tuple of (speaker, side, ir_data, test_signal, fs)

    Returns:
        Tuple of (speaker, side, recording)
    """
    speaker, side, ir_data, test_signal, fs = args

    # Import here to avoid circular dependency in multiprocessing
    from impulse_response import ImpulseResponse

    # Create temporary IR object
    temp_ir = ImpulseResponse(name=f'{speaker}-{side}', data=ir_data.copy(), fs=fs)
    recording = temp_ir.convolve(test_signal)

    return (speaker, side, recording)


def main(
    dir_path=None,
    test_signal=None,
    room_target=None,
    room_mic_calibration=None,
    headphone_compensation_file=None,
    fs=None,
    plot=False,
    channel_balance=None,
    decay=None,
    target_level=None,
    fr_combination_method="average",
    specific_limit=20000,
    generic_limit=1000,
    bass_boost_gain=0.0,
    bass_boost_fc=105,
    bass_boost_q=0.76,
    tilt=0.0,
    do_room_correction=True,
    do_headphone_compensation=True,
    do_equalization=True,
    # PR3에서 추가/변경된 파라미터 (항목 4, 6, 7)
    head_ms=1,  # --c 옵션에 해당 (기본값 1ms)
    jamesdsp=False,
    hangloose=False,
    interactive_plots=False,
    # 마이크 편차 보정 파라미터 추가 (v2.0)
    microphone_deviation_correction=False,
    mic_deviation_strength=0.7,
    mic_deviation_phase_correction=True,
    mic_deviation_adaptive_correction=True,
    mic_deviation_anatomical_validation=True,
    # TrueHD 레이아웃 관련 파라미터 추가
    output_truehd_layouts=False,
):
    """"""
    logger = get_logger()

    # Calculate total steps for progress tracking
    total_steps = 10  # Base steps: estimator, target, hrir, normalize, crop, write base files, plot results
    if do_room_correction:
        total_steps += 1
    if do_headphone_compensation:
        total_steps += 1
    if do_equalization:
        total_steps += 1
    if do_headphone_compensation or do_room_correction or do_equalization:
        total_steps += 1  # Equalizing
    if decay:
        total_steps += 1
    if channel_balance:
        total_steps += 1
    if plot:
        total_steps += 5  # Pre/post plots + additional plots
    if microphone_deviation_correction:
        total_steps += 1
    if interactive_plots:
        total_steps += 1
    if fs is not None:
        total_steps += 1
    if output_truehd_layouts:
        total_steps += 1
    if jamesdsp:
        total_steps += 1
    if hangloose:
        total_steps += 1

    logger.set_total_steps(total_steps)
    logger.info(f"Starting BRIR generation with {total_steps} processing steps")

    if plot:
        try:
            import seaborn as sns

            sns.set_theme(style="whitegrid")
            logger.debug("Seaborn style applied to plots")
        except ImportError:
            logger.debug("Seaborn not installed, using default matplotlib style")

    if dir_path is None or not os.path.isdir(dir_path):
        raise NotADirectoryError(f'Given dir path "{dir_path}"" is not a directory.')

    # Dir path as absolute
    dir_path = os.path.abspath(dir_path)

    # Impulse response estimator
    logger.step("Creating impulse response estimator")
    estimator = open_impulse_response_estimator(dir_path, file_path=test_signal)

    # Room correction frequency responses
    room_frs = None
    if do_room_correction:
        logger.step("Running room correction")
        _, room_frs = room_correction(
            estimator,
            dir_path,
            target=room_target,
            mic_calibration=room_mic_calibration,
            fr_combination_method=fr_combination_method,
            specific_limit=specific_limit,
            generic_limit=generic_limit,
            plot=plot,
        )

    # Headphone compensation frequency responses
    hp_left, hp_right = None, None
    if do_headphone_compensation:
        logger.step("Running headphone compensation")
        hp_left, hp_right = headphone_compensation(
            estimator, dir_path, headphone_compensation_file
        )

    # Equalization
    eq_left, eq_right = None, None
    if do_equalization:
        logger.step("Creating headphone equalization")
        eq_left, eq_right = equalization(estimator, dir_path)

    # Bass boost and tilt
    logger.step("Creating frequency response target")
    target = create_target(
        estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt
    )

    # HRIR measurements
    logger.step("Opening binaural measurements")
    hrir = open_binaural_measurements(estimator, dir_path)

    # Normalize gain
    logger.step("Normalizing gain")
    applied_gain = hrir.normalize(
        peak_target=None if target_level is not None else -0.1, avg_target=target_level
    )

    # Write info and stats in readme (gain 값 전달 추가)
    readme_content = write_readme(
        os.path.join(dir_path, "README.md"), hrir, fs, estimator, applied_gain
    )
    if readme_content:
        logger.info(readme_content)

    if plot:
        # Plot graphs pre processing
        os.makedirs(os.path.join(dir_path, "plots", "pre"), exist_ok=True)
        logger.step("Plotting BRIR graphs before processing")
        hrir.plot(dir_path=os.path.join(dir_path, "plots", "pre"))

    # Crop noise and harmonics from the beginning
    logger.step("Cropping impulse responses")
    hrir.crop_heads(head_ms=head_ms)

    # PR3에서 추가된 align_ipsilateral_all 호출 (항목 2)
    # SPEAKER_NAMES를 사용하므로 constants.py의 변경이 선행되어야 함
    hrir.align_ipsilateral_all(
        speaker_pairs=[
            ("FL", "FR"),
            ("SL", "SR"),
            ("BL", "BR"),
            ("TFL", "TFR"),
            ("TSL", "TSR"),
            ("TBL", "TBR"),
            ("FC", "FC"),
            ("WL", "WR"),
        ],  # FC, WL, WR 쌍은 적절히 수정 필요할 수 있음
        segment_ms=30,
    )

    # Crop noise from the tail
    hrir.crop_tails()

    # 마이크 착용 편차 보정 v2.0
    if microphone_deviation_correction:
        logger.step("Correcting microphone deviation v2.0")
        mic_deviation_plot_dir = os.path.join(dir_path, "plots") if plot else None
        hrir.correct_microphone_deviation(
            correction_strength=mic_deviation_strength,
            enable_phase_correction=mic_deviation_phase_correction,
            enable_adaptive_correction=mic_deviation_adaptive_correction,
            enable_anatomical_validation=mic_deviation_anatomical_validation,
            plot_analysis=plot,
            plot_dir=mic_deviation_plot_dir,
        )

    # Write multi-channel WAV file with sine sweeps for debugging
    hrir.write_wav(os.path.join(dir_path, "responses.wav"))

    # Equalize all
    if do_headphone_compensation or do_room_correction or do_equalization:
        logger.step("Equalizing")

        # Log parallelization info
        parallel_info = get_parallelization_info()
        logger.info(f"Using {parallel_info['executor_type']} for parallelization (Python {parallel_info['python_version']}, GIL {'disabled' if parallel_info['gil_disabled'] else 'enabled'})")

        # Optimization A1: Pre-generate common frequency array to reduce allocations
        common_freq = FrequencyResponse.generate_frequencies(
            f_step=1.01, f_min=10, f_max=estimator.fs / 2
        )

        # Phase 2 Optimization: Parallel processing of speaker-side pairs
        # Prepare arguments for parallel processing
        eq_tasks = []
        for speaker, pair in hrir.irs.items():
            for side, ir in pair.items():
                eq_tasks.append((
                    speaker, side, ir,
                    room_frs, hp_left, hp_right,
                    eq_left, eq_right, target,
                    common_freq, estimator.fs
                ))

        # Execute equalization in parallel
        logger.info(f"Processing {len(eq_tasks)} speaker-side pairs in parallel...")
        eq_results = parallel_map(_process_equalization_worker, eq_tasks)

        # Apply FIR filters to impulse responses
        for speaker, side, fir in eq_results:
            hrir.irs[speaker][side].equalize(fir)

    # Adjust decay time
    if decay:
        logger.step('Adjusting decay time')

        # Phase 2 Optimization: Parallel decay adjustment
        decay_tasks = []
        for speaker, pair in hrir.irs.items():
            if speaker in decay:
                for side, ir in pair.items():
                    decay_tasks.append((speaker, side, ir.data, decay[speaker], estimator.fs))

        if decay_tasks:
            logger.info(f"Processing {len(decay_tasks)} decay adjustments in parallel...")
            decay_results = parallel_map(_process_decay_worker, decay_tasks)

            # Apply results back to impulse responses
            for speaker, side, adjusted_data in decay_results:
                hrir.irs[speaker][side].data = adjusted_data

    # Correct channel balance
    if channel_balance is not None:
        logger.step("Correcting channel balance")
        hrir.correct_channel_balance(channel_balance)

    if plot:
        logger.step('Plotting BRIR graphs after processing')

        # Phase 2 Optimization: Parallel convolution for plotting
        plot_tasks = []
        for speaker, pair in hrir.irs.items():
            for side, ir in pair.items():
                plot_tasks.append((speaker, side, ir.data, estimator.test_signal, estimator.fs))

        logger.info(f"Processing {len(plot_tasks)} convolutions in parallel for plotting...")
        plot_results = parallel_map(_process_plot_worker, plot_tasks)

        # Apply results back to impulse responses
        for speaker, side, recording in plot_results:
            hrir.irs[speaker][side].recording = recording

        # Plot post processing
        hrir.plot(os.path.join(dir_path, "plots", "post"))

    # Plot results, always
    logger.step("Plotting results")
    hrir.plot_result(os.path.join(dir_path, "plots"))

    # PR4: 양이 응답 임펄스 오버레이 플롯 추가
    if plot:
        logger.step("Plotting additional analysis graphs")
        hrir.plot_interaural_impulse_overlay(
            os.path.join(dir_path, "plots", "interaural_overlay")
        )
        hrir.plot_ild(os.path.join(dir_path, "plots", "ild"))
        hrir.plot_ipd(os.path.join(dir_path, "plots", "ipd"))
        hrir.plot_iacc(os.path.join(dir_path, "plots", "iacc"))
        hrir.plot_etc(os.path.join(dir_path, "plots", "etc"))

    # 인터랙티브 플롯 생성 (추가)
    if interactive_plots:
        logger.step("Generating interactive plots")
        interactive_plot_dir = os.path.join(dir_path, "interactive_plots")
        os.makedirs(interactive_plot_dir, exist_ok=True)

        panels = []
        plot_functions_map = {
            "Interaural Overlay": hrir.generate_interaural_impulse_overlay_bokeh_layout,
            "ILD": hrir.generate_ild_bokeh_layout,
            "IPD": hrir.generate_ipd_bokeh_layout,
            "IACC": hrir.generate_iacc_bokeh_layout,
            "ETC": hrir.generate_etc_bokeh_layout,
            "Result Overview": hrir.generate_result_bokeh_figure,
        }

        for title, func in plot_functions_map.items():
            try:
                plot_obj = func()
                if plot_obj:
                    # Bokeh 3.x 에서는 Panel이 TabPanel로 이름 변경됨
                    panel = TabPanel(
                        child=plot_obj, title=title
                    )  # 수정: Panel -> TabPanel
                    panels.append(panel)
                else:
                    logger.debug(f"Skipping {title} plot as no data was generated")
            except Exception as e:
                logger.warning(f"Error generating interactive plot for {title}: {e}")

        if panels:
            tabs = Tabs(tabs=panels, sizing_mode="stretch_both")
            output_html_path = os.path.join(
                interactive_plot_dir, "interactive_summary.html"
            )
            bokeh_output_file(
                output_html_path, title="Interactive Plot Summary"
            )  # bokeh_output_file 사용
            bokeh_save(tabs)  # bokeh_save 사용
            logger.success(f"Interactive plot summary saved to {output_html_path}")
        else:
            logger.warning("No interactive plots were generated")

    # Re-sample
    if fs is not None and fs != hrir.fs:
        logger.step(f"Resampling BRIR to {fs} Hz")
        hrir.resample(fs)
        hrir.normalize(
            peak_target=None if target_level is not None else -0.1,
            avg_target=target_level,
        )

    # Write multi-channel WAV file with standard track order
    logger.step("Writing BRIRs")
    hrir.write_wav(os.path.join(dir_path, "hrir.wav"))

    # Write multi-channel WAV file with HeSuVi track order
    hrir.write_wav(os.path.join(dir_path, "hesuvi.wav"), track_order=HESUVI_TRACK_ORDER)

    # TrueHD 레이아웃 출력 (새로 추가)
    if output_truehd_layouts:
        logger.step("Generating TrueHD layouts")

        # 필요한 채널들이 있는지 확인하고 없으면 자동 생성하는 로직 제거
        # if auto_generate_channels:
        #     generated_channels = generate_missing_channels(hrir, auto_generate_channels)
        #     if generated_channels:
        #         logger.info(f'Generated channels: {generated_channels}')

        # 11채널 (7.0.4) 레이아웃 생성
        valid_11ch, count_11ch, msg_11ch = validate_channel_requirements(
            hrir, TRUEHD_11CH_ORDER, min_channels=8
        )
        if valid_11ch:
            available_11ch = get_available_channels_for_layout(hrir, TRUEHD_11CH_ORDER)
            track_order_11ch = create_truehd_layout_track_order(available_11ch)

            output_path_11ch = os.path.join(
                dir_path, f"truehd_11ch_{len(available_11ch)}ch.wav"
            )
            hrir.write_wav(output_path_11ch, track_order=track_order_11ch)
            logger.success(f"Generated 11-channel TrueHD layout: {output_path_11ch}")
        else:
            logger.warning(f"Cannot generate 11-channel layout: {msg_11ch}")

        # 13채널 (7.0.6) 레이아웃 생성
        valid_13ch, count_13ch, msg_13ch = validate_channel_requirements(
            hrir, TRUEHD_13CH_ORDER, min_channels=10
        )
        if valid_13ch:
            available_13ch = get_available_channels_for_layout(hrir, TRUEHD_13CH_ORDER)
            track_order_13ch = create_truehd_layout_track_order(available_13ch)

            output_path_13ch = os.path.join(
                dir_path, f"truehd_13ch_{len(available_13ch)}ch.wav"
            )
            hrir.write_wav(output_path_13ch, track_order=track_order_13ch)
            logger.success(f"Generated 13-channel TrueHD layout: {output_path_13ch}")
        else:
            logger.warning(f"Cannot generate 13-channel layout: {msg_13ch}")

    # PR3 jamesdsp 로직 추가 (항목 6)
    if jamesdsp:
        logger.step("Generating JamesDSP output")

        # 전체 HRIR 복사 후 FL/FR 외 모든 채널 제거
        dsp_hrir = copy.deepcopy(hrir)
        for sp in list(dsp_hrir.irs.keys()):
            if sp not in ["FL", "FR"]:
                del dsp_hrir.irs[sp]

        # normalize 내부의 print문 출력을 숨기기 위해 stdout 리디렉션
        # target_level 변수가 main 함수 스코프에 있어야 함
        with contextlib.redirect_stdout(io.StringIO()):
            dsp_hrir.normalize(
                peak_target=None if target_level is not None else -0.1,
                avg_target=target_level,
            )

        # FL-L, FL-R, FR-L, FR-R 순서로 파일 생성
        jd_order = ["FL-left", "FL-right", "FR-left", "FR-right"]
        out_path = os.path.join(dir_path, "jamesdsp.wav")
        dsp_hrir.write_wav(out_path, track_order=jd_order)
        logger.success(f"JamesDSP IR file created: {out_path}")

    # PR3 hangloose 로직 추가 (항목 7)
    if hangloose:
        logger.step("Generating Hangloose Convolver output")
        output_dir = os.path.join(dir_path, "Hangloose")
        os.makedirs(output_dir, exist_ok=True)

        # Hrir.wav 기준 최대 채널 순서 (constants.py의 SPEAKER_NAMES 순서와 일치시키는 것이 좋을 수 있음)
        # PR3의 full_order는 LFE를 포함하나, 현재 SPEAKER_NAMES에는 LFE가 없음.
        # 여기서는 hrir 객체에 있는 스피커만 사용하도록 단순화.
        processed_speakers = [sp for sp in SPEAKER_NAMES if sp in hrir.irs]

        for sp in processed_speakers:
            single_hrir = copy.deepcopy(hrir)
            for other_sp in list(single_hrir.irs.keys()):
                if other_sp != sp:
                    del single_hrir.irs[other_sp]

            # 각 스피커에 대해 normalize를 다시 수행할지 여부는 PR의 의도에 따라 결정.
            # 여기서는 생략하고 원본 hrir의 정규화 상태를 따름.

            track_order = [f"{sp}-left", f"{sp}-right"]
            out_path = os.path.join(output_dir, f"{sp}.wav")
            single_hrir.write_wav(out_path, track_order=track_order)
            logger.info(f"Created Hangloose file: {sp}.wav")

        logger.success(f"Hangloose Convolver files created in {output_dir}")

        # PR3의 LFE 채널 생성 로직은 FL, FR을 기반으로 하므로, 필요시 여기에 추가 구현.
        # 예시: if 'FL' in processed_speakers and 'FR' in processed_speakers:
        # LFE 생성 로직 ...


def open_impulse_response_estimator(dir_path, file_path=None):
    """Opens impulse response estimator from a file

    Args:
        dir_path: Path to directory
        file_path: Explicitly given (if any) path to impulse response estimator Pickle or test signal WAV file,
                  or a simple name/number for predefined test signals

    Returns:
        ImpulseResponseEstimator instance
    """
    # 테스트 신호가 숫자나 이름으로 지정된 경우
    if file_path in TEST_SIGNALS:
        # 패키지 내 데이터 폴더에서 해당 파일 경로 찾기
        test_signal_name = TEST_SIGNALS[file_path]
        test_signal_path = os.path.join(get_data_path(), test_signal_name)

        # 파일이 존재하는지 확인
        if os.path.isfile(test_signal_path):
            file_path = test_signal_path
        else:
            # 패키지 내 파일을 찾지 못한 경우 로컬 data 폴더에서 시도
            local_path = os.path.join(
                os.path.dirname(os.path.abspath(__file__)), "data", test_signal_name
            )
            if os.path.isfile(local_path):
                file_path = local_path
            else:
                logger = get_logger()
                logger.warning(
                    f"Test signal '{file_path}' ({test_signal_name}) not found. Using local file"
                )

    if file_path is None:
        # Test signal not explicitly given, try Pickle first then WAV
        if os.path.isfile(os.path.join(dir_path, "test.pkl")):
            file_path = os.path.join(dir_path, "test.pkl")
        elif os.path.isfile(os.path.join(dir_path, "test.wav")):
            file_path = os.path.join(dir_path, "test.wav")
        else:
            # 기본 테스트 신호 사용 (패키지 내부 또는 로컬)
            default_signal_name = TEST_SIGNALS["default"]
            default_signal_path = os.path.join(get_data_path(), default_signal_name)

            if os.path.isfile(default_signal_path):
                file_path = default_signal_path
            else:
                # 패키지 내 파일을 찾지 못한 경우 로컬 data 폴더에서 시도
                local_path = os.path.join(
                    os.path.dirname(os.path.abspath(__file__)),
                    "data",
                    default_signal_name,
                )
                if os.path.isfile(local_path):
                    file_path = local_path
                else:
                    raise FileNotFoundError(
                        f"기본 테스트 신호 파일을 찾을 수 없습니다: {default_signal_name}"
                    )

    if re.match(r"^.+\.wav$", file_path, flags=re.IGNORECASE):
        # Test signal is WAV file
        estimator = ImpulseResponseEstimator.from_wav(file_path)
    elif re.match(r"^.+\.pkl$", file_path, flags=re.IGNORECASE):
        # Test signal is Pickle file
        estimator = ImpulseResponseEstimator.from_pickle(file_path)
    elif re.match(r"^.+\.(mlp|thd|truehd)$", file_path, flags=re.IGNORECASE):
        # Test signal is TrueHD/MLP file - convert to temporary WAV first
        if not check_ffmpeg_available():
            raise RuntimeError(
                "TrueHD/MLP 파일을 처리하기 위해서는 FFmpeg가 필요합니다. FFmpeg를 설치해주세요."
            )

        if not is_truehd_file(file_path):
            raise ValueError(f"파일이 유효한 TrueHD/MLP 형식이 아닙니다: {file_path}")

        logger = get_logger()
        logger.info(f"Converting TrueHD/MLP file to WAV: {file_path}")
        temp_wav_path, channel_info = convert_truehd_to_wav(file_path)

        try:
            estimator = ImpulseResponseEstimator.from_wav(temp_wav_path)
        finally:
            # Clean up temporary file
            if os.path.exists(temp_wav_path):
                os.remove(temp_wav_path)
    else:
        raise TypeError(
            f'알 수 없는 파일 확장자: "{file_path}"\n유효한 파일 확장자: .wav, .pkl, .mlp, .thd, .truehd'
        )

    return estimator


def equalization(estimator, dir_path):
    """Reads equalization FIR filter or CSV settings

    Args:
        estimator: ImpulseResponseEstimator
        dir_path: Path to directory

    Returns:
        - Left side FIR as Numpy array or FrequencyResponse or None
        - Right side FIR as Numpy array or FrequencyResponse or None
    """
    if os.path.isfile(os.path.join(dir_path, "eq.wav")):
        logger = get_logger()
        logger.warning("eq.wav is no longer supported, use eq.csv!")
    # Default for both sides
    eq_path = os.path.join(dir_path, "eq.csv")
    eq_fr = None
    if os.path.isfile(eq_path):
        eq_fr = FrequencyResponse.read_from_csv(eq_path)

    # Left
    left_path = os.path.join(dir_path, "eq-left.csv")
    left_fr = None
    if os.path.isfile(left_path):
        left_fr = FrequencyResponse.read_from_csv(left_path)
    elif eq_fr is not None:
        left_fr = eq_fr
    if left_fr is not None:
        # left_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)
        new_freqs_left = FrequencyResponse.generate_frequencies(
            f_step=1.01, f_min=10, f_max=estimator.fs / 2
        )
        _apply_cubic_interp(
            left_fr,
            new_freqs_left,
            lambda: left_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2),
            "left equalization curve",
        )

    # Right
    right_path = os.path.join(dir_path, "eq-right.csv")
    right_fr = None
    if os.path.isfile(right_path):
        right_fr = FrequencyResponse.read_from_csv(right_path)
    elif eq_fr is not None:
        right_fr = eq_fr
    if right_fr is not None and right_fr != left_fr:
        # right_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)
        new_freqs_right = FrequencyResponse.generate_frequencies(
            f_step=1.01, f_min=10, f_max=estimator.fs / 2
        )
        _apply_cubic_interp(
            right_fr,
            new_freqs_right,
            lambda: right_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2),
            "right equalization curve",
        )

    # Plot
    if left_fr is not None or right_fr is not None:
        if left_fr == right_fr:
            # Both are the same, plot only one graph
            fig, ax = plt.subplots()
            fig.set_size_inches(12, 9)
            left_fr.plot(fig=fig, ax=ax, show_fig=False)
        else:
            # Left and right are different, plot two graphs in the same figure
            fig, ax = plt.subplots(1, 2)
            fig.set_size_inches(22, 9)
            if left_fr is not None:
                left_fr.plot(fig=fig, ax=ax[0], show_fig=False)
            if right_fr is not None:
                right_fr.plot(fig=fig, ax=ax[1], show_fig=False)
        save_fig_as_png(os.path.join(dir_path, "plots", "eq.png"), fig)

    return left_fr, right_fr


def headphone_compensation(estimator, dir_path, headphone_file_path=None):
    """Equalizes HRIR tracks with headphone compensation measurement.

    Args:
        estimator: ImpulseResponseEstimator instance
        dir_path: Path to output directory
        headphone_file_path: Optional path to the headphone compensation WAV file.
                             If None, defaults to 'headphones.wav' in dir_path.

    Returns:
        None
    """
    # Read WAV file
    hp_irs = HRIR(estimator)

    # Determine the headphone file to use
    if headphone_file_path:
        # If a specific path is provided, use it
        # If it's a relative path, consider it relative to the current working directory or dir_path
        # For simplicity, we'll assume it's either absolute or relative to dir_path if not absolute
        if not os.path.isabs(headphone_file_path):
            actual_hp_file = os.path.join(dir_path, headphone_file_path)
        else:
            actual_hp_file = headphone_file_path
        logger = get_logger()

    if not os.path.exists(actual_hp_file):
        logger.warning(
            f"Specified headphone compensation file not found: {actual_hp_file}. Trying default 'headphones.wav'"
        )
        actual_hp_file = os.path.join(dir_path, "headphones.wav")  # Fallback to default
    else:
        # Default to headphones.wav in the dir_path
        actual_hp_file = os.path.join(dir_path, "headphones.wav")

    if not os.path.exists(actual_hp_file):
        logger.error(f"Headphone compensation file not found: {actual_hp_file}")
        return None, None  # Or raise an error

    logger.info(f"Using headphone compensation file: {actual_hp_file}")
    hp_irs.open_recording(actual_hp_file, speakers=["FL", "FR"])
    hp_irs.write_wav(os.path.join(dir_path, "headphone-responses.wav"))

    # Frequency responses
    left = hp_irs.irs["FL"]["left"].frequency_response()
    right = hp_irs.irs["FR"]["right"].frequency_response()

    # 배열 길이 검증 및 일치시키기
    if len(left.frequency) != len(right.frequency):
        # 둘 중 더 작은 길이로 조정
        min_length = min(len(left.frequency), len(right.frequency))
        left.frequency = left.frequency[:min_length]
        left.raw = left.raw[:min_length]
        right.frequency = right.frequency[:min_length]
        right.raw = right.raw[:min_length]

    # Center by left channel
    gain = left.center([100, 10000])
    right.raw += gain

    # 저주파 롤오프 방지를 위한 타겟 생성
    freq = FrequencyResponse.generate_frequencies(
        f_min=10, f_max=estimator.fs / 2, f_step=1.01
    )

    # 새로운 타겟: 저주파에 6dB 부스트를 적용한 타겟
    target_raw = np.zeros(len(freq))

    # 타겟 응답 객체 생성
    target = FrequencyResponse(
        name="headphone_compensation_target", frequency=freq, raw=target_raw
    )

    # left와 right를 타겟의 주파수에 맞게 보간
    _apply_cubic_interp(left, target.frequency, lambda: left.interpolate(f=target.frequency), "left headphone response")
    _apply_cubic_interp(right, target.frequency, lambda: right.interpolate(f=target.frequency), "right headphone response")
    
    # 보상 적용
    left.compensate(target, min_mean_error=True)
    right.compensate(target, min_mean_error=True)

    # 기존 헤드폰 플롯
    fig = plt.figure()
    gs = fig.add_gridspec(2, 3)
    fig.set_size_inches(22, 10)
    fig.suptitle("Headphones")

    # Left
    axl = fig.add_subplot(gs[0, 0])
    left.plot(fig=fig, ax=axl, show_fig=False)
    axl.set_title("Left")
    # Right
    axr = fig.add_subplot(gs[1, 0])
    right.plot(fig=fig, ax=axr, show_fig=False)
    axr.set_title("Right")
    # Sync axes
    sync_axes([axl, axr])

    # Combined
    # Optimized: Use _get_center_value instead of .copy().center()
    gain_l = _get_center_value(left, [100, 10000])
    gain_r = _get_center_value(right, [100, 10000])
    ax = fig.add_subplot(gs[:, 1:])
    ax.plot(left.frequency, left.raw, linewidth=1, color='#1f77b4')
    ax.plot(right.frequency, right.raw, linewidth=1, color='#d62728')
    ax.plot(left.frequency, left.raw - right.raw, linewidth=1, color='#680fb9')
    sl = np.logical_and(left.frequency > 20, left.frequency < 20000)
    stack = np.vstack([left.raw[sl], right.raw[sl], left.raw[sl] - right.raw[sl]])
    ax.set_ylim([np.min(stack) * 1.1, np.max(stack) * 1.1])
    axl.set_ylim([np.min(stack) * 1.1, np.max(stack) * 1.1])
    axr.set_ylim([np.min(stack) * 1.1, np.max(stack) * 1.1])
    ax.set_title("Comparison")
    ax.legend(
        [f"Left raw {gain_l:+.1f} dB", f"Right raw {gain_r:+.1f} dB", "Difference"],
        fontsize=8,
    )
    ax.set_xlabel("Frequency (Hz)")
    ax.semilogx()
    ax.set_xlim([20, 20000])
    ax.set_ylabel("Amplitude (dB)")
    ax.grid(True, which="major")
    ax.grid(True, which="minor")
    ax.xaxis.set_major_formatter(ticker.StrMethodFormatter("{x:.0f}"))

    # Save headphone plots
    file_path = os.path.join(dir_path, "plots", "headphones.png")
    os.makedirs(os.path.split(file_path)[0], exist_ok=True)
    save_fig_as_png(file_path, fig)
    plt.close(fig)

    return left, right


def create_target(estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt):
    """Creates target frequency response with bass boost, tilt and high pass at 20 Hz"""
    # 타겟 주파수 응답 생성
    target = FrequencyResponse(
        name="bass_and_tilt",
        frequency=FrequencyResponse.generate_frequencies(
            f_min=10, f_max=estimator.fs / 2, f_step=1.01
        ),
    )

    # 베이스 부스트와 틸트 적용
    # 기본 베이스 부스트만 적용 (추가 부스트 제거)
    target.raw = target.create_target(
        bass_boost_gain=bass_boost_gain,  # +3dB 추가 부스트 제거
        bass_boost_fc=bass_boost_fc,
        bass_boost_q=bass_boost_q,
        tilt=tilt,
    )

    # 저주파 영역 베이스 부스트 값 출력 (디버깅용)
    # bass_boost_values = target.raw[:200]  # 저주파 영역만 추출
    # print("저주파 영역 Bass Boost 값:", bass_boost_values) # 주석 처리

    return target


def open_binaural_measurements(estimator, dir_path):
    """Opens binaural measurement WAV files.

    Args:
        estimator: ImpulseResponseEstimator
        dir_path: Path to directory

    Returns:
        HRIR instance
    """
    hrir = HRIR(estimator)
    pattern = r"^{pattern}\.wav$".format(pattern=SPEAKER_LIST_PATTERN)  # FL,FR.wav
    for file_name in [f for f in os.listdir(dir_path) if re.match(pattern, f)]:
        # Read the speaker names from the file name into a list
        speakers = re.search(SPEAKER_LIST_PATTERN, file_name)[0].split(",")
        # Form absolute path
        file_path = os.path.join(dir_path, file_name)
        # Open the file and add tracks to HRIR
        hrir.open_recording(file_path, speakers=speakers)
    if len(hrir.irs) == 0:
        raise ValueError("No HRIR recordings found in the directory.")
    return hrir


def write_readme(file_path, hrir, fs, estimator, applied_gain):
    """Writes info and stats to a README file and returns its content as a string.

    Args:
        file_path (str): Path to README file.
        hrir (HRIR): HRIR object.
        fs (int): Output sampling rate.
        estimator (ImpulseResponseEstimator): Estimator object for advanced stats.
        applied_gain (float): Applied gain level.

    Returns:
        str: Content of the README file.
    """
    # 기본 헤더 생성
    content = "# BRIR Info\n\n"
    content += f"Processed on {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}. Output sampling rate is {fs if fs is not None else hrir.fs} Hz.\n\n"

    # 항목 8: 적용된 노멀라이제이션 게인 추가
    if applied_gain is not None:
        content += "## Applied Normalization Gain\n"
        content += f"{applied_gain:.2f} dB was applied to all channels.\n\n"

    # 기존 통계 테이블 생성 로직 (rt_name, table, speaker_names 등)
    table_data = []  # 변수명 변경 (table -> table_data)
    # SPEAKER_NAMES 순서대로 정렬하되, 없는 스피커는 뒤로
    speaker_names_in_hrir = list(hrir.irs.keys())
    sorted_speaker_names = sorted(
        speaker_names_in_hrir,
        key=lambda x: SPEAKER_NAMES.index(x) if x in SPEAKER_NAMES else float("inf"),
    )

    final_rt_name = "Reverb"  # 최종적으로 사용될 RTxx 이름, 모든 IR 검토 후 결정
    rt_values_for_naming = []

    for speaker in sorted_speaker_names:
        if speaker not in hrir.irs:
            continue
        pair = hrir.irs[speaker]

        peak_left_idx = pair["left"].peak_index()
        peak_right_idx = pair["right"].peak_index()
        itd = np.nan
        if peak_left_idx is not None and peak_right_idx is not None:
            itd = np.abs(peak_right_idx - peak_left_idx) / hrir.fs * 1e6  # us

        for side, ir_obj in pair.items():
            current_itd = 0.0
            if not np.isnan(itd):
                if speaker.endswith("L") and side == "right":
                    current_itd = itd
                elif speaker.endswith("R") and side == "left":
                    current_itd = itd

            pnr_val = np.nan
            length_ms = np.nan
            rt_val_ms = np.nan
            current_ir_rt_name = None

            peak_idx_current_ir = ir_obj.peak_index()
            if peak_idx_current_ir is not None:
                # PNR 계산
                peak_val_linear = np.abs(ir_obj.data[peak_idx_current_ir])
                # 데이터가 0~1로 정규화되었다고 가정. 그렇지 않다면 최대값으로 나눠야 함.
                # peak_val_db = 20 * np.log10(peak_val_linear / np.max(np.abs(ir_obj.data)) + 1e-9) # 좀 더 안전한 방식
                peak_val_db = 20 * np.log10(
                    peak_val_linear + 1e-9
                )  # 피크값의 dBFS (최대값이 1.0이라고 가정)

                decay_params_tuple = ir_obj.decay_params()
                if decay_params_tuple:
                    noise_floor_db = decay_params_tuple[2]
                    if not np.isnan(noise_floor_db) and not np.isnan(peak_val_db):
                        pnr_val = peak_val_db - noise_floor_db

                    # Length 계산
                    tail_ind_calc = decay_params_tuple[
                        1
                    ]  # decay_params의 두 번째 값이 tail index (peak_idx + knee_idx)
                    if (
                        tail_ind_calc is not None
                        and tail_ind_calc > peak_idx_current_ir
                    ):
                        length_ms = (
                            (tail_ind_calc - peak_idx_current_ir) / ir_obj.fs * 1000
                        )

                # RTxx 계산 (decay_times 사용)
                # decay_times() 호출 시 peak_ind 등을 전달해야 할 수 있음 (API 확인)
                # 현재 API는 decay_params() 내부 값들을 사용하므로, decay_params() 호출 후 사용 가능
                edt, rt20, rt30, rt60 = ir_obj.decay_times(
                    peak_ind=decay_params_tuple[0] if decay_params_tuple else None,
                    knee_point_ind=decay_params_tuple[1]
                    if decay_params_tuple
                    else None,
                    noise_floor=decay_params_tuple[2] if decay_params_tuple else None,
                    window_size=decay_params_tuple[3] if decay_params_tuple else None,
                )

                # 가장 긴 유효한 RTxx 값 선택
                if rt60 is not None and not np.isnan(rt60):
                    rt_val_ms = rt60 * 1000
                    current_ir_rt_name = "RT60"
                elif rt30 is not None and not np.isnan(rt30):
                    rt_val_ms = rt30 * 1000
                    current_ir_rt_name = "RT30"
                elif rt20 is not None and not np.isnan(rt20):
                    rt_val_ms = rt20 * 1000
                    current_ir_rt_name = "RT20"
                elif edt is not None and not np.isnan(edt):
                    rt_val_ms = edt * 1000
                    current_ir_rt_name = "EDT"

                if current_ir_rt_name:
                    rt_values_for_naming.append(current_ir_rt_name)

            table_data.append(
                [
                    speaker,
                    side,
                    f"{pnr_val:.1f} dB" if not np.isnan(pnr_val) else "N/A",
                    f"{current_itd:.1f} us" if not np.isnan(current_itd) else "N/A",
                    f"{length_ms:.1f} ms"
                    if length_ms is not None
                    and not np.isnan(length_ms)
                    and length_ms >= 0
                    else "N/A",  # 음수 길이 방지
                    f"{rt_val_ms:.1f} ms"
                    if rt_val_ms is not None and not np.isnan(rt_val_ms)
                    else "N/A",
                ]
            )

    # 모든 IR을 살펴본 후 최종 RTxx 이름 결정 (가장 많이 나온 유효한 이름 또는 우선순위)
    if rt_values_for_naming:
        # 예: 가장 빈번하게 나타난 RTxx 이름 사용
        from collections import Counter

        final_rt_name = Counter(rt_values_for_naming).most_common(1)[0][0]
    else:
        final_rt_name = "RTxx"  # 기본값

    if table_data:
        headers = ["Speaker", "Side", "PNR", "ITD", "Length", final_rt_name]
        content += tabulate(table_data, headers=headers, tablefmt="pipe")
        content += "\n\n"

    # 항목 9: 반사음 레벨 추가
    if estimator and hasattr(hrir, "calculate_reflection_levels"):
        reflection_data = hrir.calculate_reflection_levels()  # 인자 없이 호출
        if reflection_data:
            content += "## Reflection Levels (Direct vs. Early/Late)\n"
            # SPEAKER_NAMES 순서대로 정렬하되, 없는 스피커는 뒤로
            sorted_reflection_speakers = sorted(
                reflection_data.keys(),
                key=lambda x: SPEAKER_NAMES.index(x)
                if x in SPEAKER_NAMES
                else float("inf"),
            )
            for speaker in sorted_reflection_speakers:
                if (
                    speaker not in reflection_data
                ):  # Should not happen due to sorted keys
                    continue
                sides_data = reflection_data[speaker]
                content += f"### {speaker}\n"
                if "left" in sides_data and isinstance(sides_data["left"], dict):
                    content += f"- Left Ear: Early (20-50ms): {sides_data['left'].get('early_db', np.nan):.2f} dB, Late (50-150ms): {sides_data['left'].get('late_db', np.nan):.2f} dB\n"
                if "right" in sides_data and isinstance(sides_data["right"], dict):
                    content += f"- Right Ear: Early (20-50ms): {sides_data['right'].get('early_db', np.nan):.2f} dB, Late (50-150ms): {sides_data['right'].get('late_db', np.nan):.2f} dB\n"
            content += "\n"

    # 파일에 쓰기
    with open(file_path, "w", encoding="utf-8") as f:
        f.write(content)

    return content


def create_cli():
    arg_parser = argparse.ArgumentParser()
    arg_parser.add_argument(
        "--dir_path",
        type=str,
        required=True,
        help="Path to directory for recordings and outputs.",
    )
    arg_parser.add_argument(
        "--test_signal",
        type=str,
        default=argparse.SUPPRESS,
        help="Path to sine sweep test signal or pickled impulse response estimator. "
        "You can also use a predefined name or number: "
        '"default"/"1" (.pkl), "sweep"/"2" (.wav), "stereo"/"3" (FL,FR), '
        '"mono-left"/"4" (FL mono), "left"/"5" (FL stereo), "right"/"6" (FR stereo).',
    )
    arg_parser.add_argument(
        "--room_target",
        type=str,
        default=argparse.SUPPRESS,
        help="Path to room target response AutoEQ style CSV file.",
    )
    arg_parser.add_argument(
        "--room_mic_calibration",
        type=str,
        default=argparse.SUPPRESS,
        help="Path to room measurement microphone calibration file.",
    )
    arg_parser.add_argument(
        "--no_room_correction",
        action="store_false",
        dest="do_room_correction",
        help="Skip room correction.",
    )
    arg_parser.add_argument(
        "--no_headphone_compensation",
        action="store_false",
        dest="do_headphone_compensation",
        help="Skip headphone compensation.",
    )
    arg_parser.add_argument(
        "--headphone_compensation_file",
        type=str,
        default=None,
        help='Path to the headphone compensation WAV file. Defaults to "headphones.wav" in dir_path.',
    )
    arg_parser.add_argument(
        "--no_equalization",
        action="store_false",
        dest="do_equalization",
        help="Skip equalization.",
    )
    arg_parser.add_argument(
        "--fs",
        type=int,
        default=argparse.SUPPRESS,
        help="Output sampling rate in Hertz.",
    )
    arg_parser.add_argument(
        "--plot", action="store_true", help="Plot graphs for debugging."
    )
    arg_parser.add_argument(
        "--interactive_plots",
        action="store_true",
        help="Generate interactive Bokeh plots in HTML files.",
    )
    arg_parser.add_argument(
        "--channel_balance",
        type=str,
        default=argparse.SUPPRESS,
        help="Channel balance correction by equalizing left and right ear results to the same "
        'level or frequency response. "trend" equalizes right side by the difference trend '
        'of right and left side. "left" equalizes right side to left side fr, "right" '
        'equalizes left side to right side fr, "avg" equalizes both to the average fr, "min" '
        "equalizes both to the minimum of left and right side frs. Number values will boost "
        'or attenuate right side relative to left side by the number of dBs. "mids" is the '
        "same as the numerical values but guesses the value automatically from mid frequency "
        "levels.",
    )
    arg_parser.add_argument(
        "--decay",
        type=str,
        default=argparse.SUPPRESS,
        help="Target decay time in milliseconds to reach -60 dB. When the natural decay time is "
        "longer than the target decay time, a downward slope will be applied to decay tail. "
        "Decay cannot be increased with this. By default no decay time adjustment is done. "
        "A comma separated list of channel name and  reverberation time pairs, separated by "
        "a colon. If only a single numeric value is given, it is used for all channels. When "
        "some channel names are give but not all, the missing channels are not affected. For "
        'example "--decay=300" or "--decay=FL:500,FC:100,FR:500,SR:700,BR:700,BL:700,SL:700" '
        'or "--decay=FC:100".',
    )
    arg_parser.add_argument(
        "--target_level",
        type=float,
        default=argparse.SUPPRESS,
        help="Target average gain level for left and right channels. This will sum together all "
        "left side impulse responses and right side impulse responses respectively and take "
        "the average gain from mid frequencies. The averaged level is then normalized to the "
        "given target level. This makes it possible to compare HRIRs with somewhat similar "
        "loudness levels. This should be negative in most cases to avoid clipping.",
    )
    arg_parser.add_argument(
        "--fr_combination_method",
        type=str,
        default="average",
        help="Method for combining frequency responses of generic room measurements if there are "
        'more than one tracks in the file. "average" will simply average the frequency'
        'responses. "conservative" will take the minimum absolute value for each frequency '
        "but only if the values in all the measurements are positive or negative at the same "
        "time.",
    )
    arg_parser.add_argument(
        "--specific_limit",
        type=float,
        default=400,
        help="Upper limit for room equalization with speaker-ear specific room measurements. "
        "Equalization will drop down to 0 dB at this frequency in the leading octave. 0 "
        "disables limit.",
    )
    arg_parser.add_argument(
        "--generic_limit",
        type=float,
        default=300,
        help="Upper limit for room equalization with generic room measurements. "
        "Equalization will drop down to 0 dB at this frequency in the leading octave. 0 "
        "disables limit.",
    )
    arg_parser.add_argument(
        "--bass_boost",
        type=str,
        default=argparse.SUPPRESS,
        help="Bass boost shelf. Sub-bass frequencies will be boosted by this amount. Can be "
        "either a single value for a gain in dB or a comma separated list of three values for "
        "parameters of a low shelf filter, where the first is gain in dB, second is center "
        "frequency (Fc) in Hz and the last is quality (Q). When only a single value (gain) is "
        "given, default values for Fc and Q are used which are 105 Hz and 0.76, respectively. "
        'For example "--bass_boost=6" or "--bass_boost=6,150,0.69".',
    )
    arg_parser.add_argument(
        "--tilt",
        type=float,
        default=argparse.SUPPRESS,
        help="Target tilt in dB/octave. Positive value (upwards slope) will result in brighter "
        "frequency response and negative value (downwards slope) will result in darker "
        "frequency response. 1 dB/octave will produce nearly 10 dB difference in "
        "desired value between 20 Hz and 20 kHz. Tilt is applied with bass boost and both "
        "will affect the bass gain.",
    )
    arg_parser.add_argument(
        "--c",
        type=float,
        default=1.0,
        dest="head_ms",
        help="Head room in milliseconds for cropping impulse response heads. Default is 1.0 (ms). (항목 4)",
    )
    arg_parser.add_argument(
        "--jamesdsp",
        action="store_true",
        help="Generate true stereo IR file (jamesdsp.wav) for JamesDSP from FL/FR channels. (항목 6)",
    )
    arg_parser.add_argument(
        "--hangloose",
        action="store_true",
        help="Generate separate stereo IR for each channel for Hangloose Convolver. (항목 7)",
    )
    arg_parser.add_argument(
        "--microphone_deviation_correction",
        action="store_true",
        help="Enable microphone deviation correction v2.0 to compensate for microphone placement variations between left and right ears.",
    )
    arg_parser.add_argument(
        "--mic_deviation_strength",
        type=float,
        default=0.7,
        help="Microphone deviation correction strength (0.0-1.0). 0.0 = no correction, 1.0 = full correction. Default is 0.7.",
    )
    arg_parser.add_argument(
        "--no_mic_deviation_phase_correction",
        action="store_false",
        dest="mic_deviation_phase_correction",
        help="Disable phase correction in microphone deviation correction v2.0. (Default: enabled)",
    )
    arg_parser.add_argument(
        "--no_mic_deviation_adaptive_correction",
        action="store_false",
        dest="mic_deviation_adaptive_correction",
        help="Disable adaptive asymmetric correction in microphone deviation correction v2.0. (Default: enabled)",
    )
    arg_parser.add_argument(
        "--no_mic_deviation_anatomical_validation",
        action="store_false",
        dest="mic_deviation_anatomical_validation",
        help="Disable ITD/ILD anatomical validation in microphone deviation correction v2.0. (Default: enabled)",
    )
    arg_parser.add_argument(
        "--output_truehd_layouts", action="store_true", help="Generate TrueHD layouts."
    )
    args = vars(arg_parser.parse_args())
    if "bass_boost" in args:
        bass_boost = args["bass_boost"].split(",")
        if len(bass_boost) == 1:
            args["bass_boost_gain"] = float(bass_boost[0])
            args["bass_boost_fc"] = 105
            args["bass_boost_q"] = 0.76
        elif len(bass_boost) == 3:
            args["bass_boost_gain"] = float(bass_boost[0])
            args["bass_boost_fc"] = float(bass_boost[1])
            args["bass_boost_q"] = float(bass_boost[2])
        else:
            raise ValueError(
                '"--bass_boost" must have one value or three values separated by commas!'
            )
        del args["bass_boost"]
    if "decay" in args:
        decay = dict()
        try:
            # Single float value
            decay = {ch: float(args["decay"]) / 1000 for ch in SPEAKER_NAMES}
        except ValueError:
            # Channels separated
            for ch_t in args["decay"].split(","):
                decay[ch_t.split(":")[0].upper()] = float(ch_t.split(":")[1]) / 1000
        args["decay"] = decay
    return args


if __name__ == "__main__":
    cli_args = create_cli()
    # interactive_plots 인자를 main 함수에 전달
    main(**cli_args)
