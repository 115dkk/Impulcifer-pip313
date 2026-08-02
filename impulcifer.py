# -*- coding: utf-8 -*-

def _get_version() -> str:
    """Get version from build marker, pyproject.toml, or package metadata."""
    # Method 0: 빌드 마커 (Nuitka/pip 빌드에서 가장 확실)
    try:
        from infra._build_info import VERSION as build_version
        if build_version is not None:
            return build_version
    except ImportError:
        pass

    # Method 1: pyproject.toml (개발 환경)
    try:
        import tomllib
    except ImportError:
        try:
            import tomli as tomllib
        except ImportError:
            tomllib = None

    if tomllib:
        try:
            from pathlib import Path
            possible_paths = [
                Path(__file__).parent / 'pyproject.toml',
                Path(__file__).parent.parent / 'pyproject.toml',
            ]
            for pyproject_path in possible_paths:
                if pyproject_path.exists():
                    with open(pyproject_path, 'rb') as f:
                        data = tomllib.load(f)
                        version_str = data.get('project', {}).get('version')
                        if version_str:
                            return version_str
        except Exception:
            pass

    # Method 2: Package metadata (pip 설치, 마커 없는 경우)
    try:
        from importlib.metadata import version as get_version
        return get_version('impulcifer-py313')
    except Exception:
        pass

    # Fallback
    return "2.5.0"

__version__ = _get_version()

import os
import re
import argparse
from tabulate import tabulate
from datetime import datetime
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
import matplotlib.font_manager as fm
from autoeq.frequency_response import FrequencyResponse
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.hrir import HRIR, _get_center_value
from core.utils import (
    sync_axes,
    save_fig_as_png,
    is_truehd_file,
    convert_truehd_to_wav,
    check_ffmpeg_available,
    set_matplotlib_font,
)
from core.constants import (
    SPEAKER_NAMES,
    SPEAKER_LIST_PATTERN,
    TEST_SIGNALS,
    get_data_path,
    speaker_side,
)
from core.eqapo import looks_like_eqapo_config, parse_eqapo_config
from infra.logger import get_logger

from bokeh.plotting import (
    output_file as bokeh_output_file,
    save as bokeh_save,
)

# Cancellation now lives in core.cancellation; these re-exports keep the
# historical import surface (GUI tabs, application service, tests) working.
from core.cancellation import (  # noqa: F401
    CancelledError,
    cancellation_scope,
    check_cancelled as _check_cancelled,
)

# Python 3.14 병렬 처리 지원
try:
    from core.parallel_processing import parallel_process_dict, is_free_threaded_available

    PARALLEL_PROCESSING_AVAILABLE = True
except ImportError:
    PARALLEL_PROCESSING_AVAILABLE = False
    parallel_process_dict = None

    def is_free_threaded_available():
        return False


def get_pretendard_font_for_gui():
    """Return the bundled or system Pretendard font for legacy callers."""
    try:
        from infra.resource_helper import find_pretendard_font_path

        font_path = find_pretendard_font_path()
        if font_path:
            return font_path

        try:
            available_fonts = [f.name for f in fm.fontManager.ttflist]
            if "Pretendard" in available_fonts:
                return "Pretendard"
        except Exception:
            pass

    except Exception:
        pass

    return None


set_matplotlib_font()


def _save_bokeh_analysis_plots(hrir, dir_path, logger):
    """분석 플롯(ILD/IPD/IACC/ETC)을 Bokeh HTML로 저장."""
    plot_configs = {
        "ild": ("ILD Analysis", hrir.generate_ild_bokeh_layout),
        "ipd": ("IPD Analysis", hrir.generate_ipd_bokeh_layout),
        "iacc": ("IACC Analysis", hrir.generate_iacc_bokeh_layout),
        "etc": ("ETC Analysis", hrir.generate_etc_bokeh_layout),
    }
    for name, (title, func) in plot_configs.items():
        try:
            layout = func()
            if layout is not None:
                out_dir = os.path.join(dir_path, "plots", name)
                os.makedirs(out_dir, exist_ok=True)
                out_path = os.path.join(out_dir, f"{name}_analysis.html")
                bokeh_output_file(out_path, title=title)
                bokeh_save(layout)
        except Exception as e:
            logger.warning("cli_warning_interactive_plot_error",
                          title=name, error=str(e))


def main(**kwargs):
    """Thin wrapper around :class:`core.pipeline.BRIRPipeline`.

    Accepts the keyword-argument dict assembled by the GUI
    (``gui.brir_args.build_brir_args`` / ``generate_brir()``) and by the CLI
    (``create_cli``) and forwards it through a
    :class:`~core.pipeline.ProcessingConfig`. ``ProcessingConfig`` is the
    single source of truth for parameter defaults (``core.cli_builder`` derives
    the CLI from it); unknown keys — e.g. retired compatibility flags such as
    the old ``mic_deviation_phase_correction`` — are ignored by
    :meth:`~core.pipeline.ProcessingConfig.from_kwargs`, so this call site stays
    stable as the parameter set evolves and no longer hand-mirrors the dataclass.
    BRIR output byte-exactness is pinned by tests/test_brir_integrity.py (see
    core/pipeline.py module docstring).
    """
    # Local import to avoid a circular dependency: core.pipeline imports back
    # into impulcifer for the legacy stage runner.
    from core.pipeline import ProcessingConfig, BRIRPipeline

    BRIRPipeline(ProcessingConfig.from_kwargs(**kwargs)).run()


def open_impulse_response_estimator(dir_path, file_path=None):
    """Opens impulse response estimator from a file

    Args:
        dir_path: Path to directory
        file_path: Explicitly given (if any) path to impulse response estimator Pickle or test signal WAV file,
                  or a simple name/number for predefined test signals

    Returns:
        ImpulseResponseEstimator instance
    """
    if file_path in TEST_SIGNALS:
        test_signal_name = TEST_SIGNALS[file_path]
        test_signal_path = os.path.join(get_data_path(), test_signal_name)

        if os.path.isfile(test_signal_path):
            file_path = test_signal_path
        else:
            local_path = os.path.join(
                os.path.dirname(os.path.abspath(__file__)), "data", test_signal_name
            )
            if os.path.isfile(local_path):
                file_path = local_path
            else:
                logger = get_logger()
                logger.warning("cli_warning_test_signal_not_found", signal=file_path, name=test_signal_name)

    if file_path is None:
        if os.path.isfile(os.path.join(dir_path, "test.pkl")):
            file_path = os.path.join(dir_path, "test.pkl")
        elif os.path.isfile(os.path.join(dir_path, "test.wav")):
            file_path = os.path.join(dir_path, "test.wav")
        else:
            default_signal_name = TEST_SIGNALS["default"]
            default_signal_path = os.path.join(get_data_path(), default_signal_name)

            if os.path.isfile(default_signal_path):
                file_path = default_signal_path
            else:
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
        estimator = ImpulseResponseEstimator.from_wav(file_path)
    elif re.match(r"^.+\.pkl$", file_path, flags=re.IGNORECASE):
        estimator = ImpulseResponseEstimator.from_pickle(file_path)
    elif re.match(r"^.+\.(mlp|thd|truehd)$", file_path, flags=re.IGNORECASE):
        # auto_install=True로 호출해 사용자가 .mlp/.thd/.truehd 파일을 직접
        # 지정한 경우 FFmpeg가 없으면 기존처럼 자동 설치 UX를 시도한다.
        if not check_ffmpeg_available(auto_install=True):
            raise RuntimeError(
                "TrueHD/MLP 파일을 처리하기 위해서는 FFmpeg가 필요합니다. FFmpeg를 설치해주세요."
            )

        if not is_truehd_file(file_path):
            raise ValueError(f"파일이 유효한 TrueHD/MLP 형식이 아닙니다: {file_path}")

        logger = get_logger()
        logger.info("cli_info_converting_truehd", file=file_path)
        temp_wav_path, channel_info = convert_truehd_to_wav(file_path)

        try:
            estimator = ImpulseResponseEstimator.from_wav(temp_wav_path)
        finally:
            if os.path.exists(temp_wav_path):
                os.remove(temp_wav_path)
    else:
        raise TypeError(
            f'알 수 없는 파일 확장자: "{file_path}"\n유효한 파일 확장자: .wav, .pkl, .mlp, .thd, .truehd'
        )

    return estimator


def _find_eq_settings_file(dir_path, base_name):
    """EQ 설정 파일 경로를 찾는다.

    기존 ``.csv``가 우선이며, EqualizerAPO(-XT)에서 내보낸 ``.txt``도 허용한다.
    """
    for ext in (".csv", ".txt"):
        path = os.path.join(dir_path, base_name + ext)
        if os.path.isfile(path):
            return path
    return None


# equalization() 로그에서 한 파일당 개별 바이패스 경고를 출력하는 최대 개수
_EQAPO_MAX_BYPASS_WARNINGS = 20


def _read_eq_settings(file_path, estimator):
    """EQ 설정 파일을 읽어 FrequencyResponse로 변환한다.

    내용이 EqualizerAPO(-XT) 설정 형식이면 지원되는 필터(Filter 바이쿼드/IIR,
    Preamp, GraphicEQ, Convolution)를 크기 응답으로 합성하고, 지원되지 않는
    명령은 바이패스하며 경고를 남긴다. 그 외에는 기존 AutoEQ CSV 파서를
    사용하며, error 열이 없는 평문 gain 곡선 파일은 error = -raw 로 채운다.

    Args:
        file_path: EQ 설정 파일 경로
        estimator: ImpulseResponseEstimator (샘플레이트 참조)

    Returns:
        - FrequencyResponse (양쪽 공통 또는 좌측)
        - 우측 전용 FrequencyResponse 또는 None (좌우가 동일한 경우)
    """
    with open(file_path, "rb") as fh:
        raw = fh.read()
    try:
        text = raw.decode("utf-8-sig")
    except UnicodeDecodeError:
        text = raw.decode("cp1252", errors="replace")

    logger = get_logger()

    if not looks_like_eqapo_config(text):
        fr = FrequencyResponse.read_from_csv(file_path)
        if len(fr.error) == 0 and len(fr.raw) > 0:
            # error 열이 없는 평문(frequency, gain) 파일: raw를 적용할 EQ
            # 곡선으로 해석한다. 파이프라인은 error 필드를 소비하므로
            # error = -raw 로 채워 곡선이 그대로 적용되게 한다.
            logger.info("cli_eq_plain_gain_curve", file=os.path.basename(file_path))
            fr.error = -fr.raw.copy()
        return fr, None

    from i18n.localization import get_localization_manager

    loc = get_localization_manager()
    name = ".".join(os.path.split(file_path)[1].split(".")[:-1])
    frequency = FrequencyResponse.generate_frequencies(
        f_step=1.01, f_min=10, f_max=estimator.fs / 2
    )
    result = parse_eqapo_config(
        text, estimator.fs, frequency, base_dir=os.path.dirname(file_path)
    )

    logger.info(
        "cli_eqapo_detected",
        file=os.path.basename(file_path),
        applied=len(result.applied),
        bypassed=len(result.bypassed),
        skipped=len(result.skipped),
    )
    for description in result.applied:
        logger.debug(f"  EqualizerAPO: {description}")
    if result.preamp_left != 0.0 or result.preamp_right != 0.0:
        logger.info(
            "cli_eqapo_preamp",
            left=f"{result.preamp_left:g}",
            right=f"{result.preamp_right:g}",
        )
    for report in result.bypassed[:_EQAPO_MAX_BYPASS_WARNINGS]:
        logger.warning(
            "cli_eqapo_bypassed_line",
            line=report.line_number,
            command=report.command,
            reason=loc.get(f"cli_eqapo_reason_{report.reason}"),
        )
    if len(result.bypassed) > _EQAPO_MAX_BYPASS_WARNINGS:
        logger.warning(
            "cli_eqapo_bypassed_more",
            count=len(result.bypassed) - _EQAPO_MAX_BYPASS_WARNINGS,
        )

    # 파이프라인은 eq FrequencyResponse의 error 필드를 보정 대상 오차로
    # 사용하므로(잔차를 상쇄하도록 equalization을 계산), 설정 파일의 EQ 곡선이
    # 그대로 적용되게 하려면 error = -gain 으로 넣는다. raw는 플롯용 곡선이다.
    left_fr = FrequencyResponse(
        name=name,
        frequency=frequency.copy(),
        raw=result.left_db,
        error=-result.left_db,
    )
    right_fr = None
    if result.channel_split:
        logger.info("cli_eqapo_channel_split")
        right_fr = FrequencyResponse(
            name=f"{name} (right)",
            frequency=frequency.copy(),
            raw=result.right_db,
            error=-result.right_db,
        )
    return left_fr, right_fr


def equalization(estimator, dir_path):
    """Reads equalization FIR filter, CSV or EqualizerAPO settings

    Args:
        estimator: ImpulseResponseEstimator
        dir_path: Path to directory

    Returns:
        - Left side FIR as Numpy array or FrequencyResponse or None
        - Right side FIR as Numpy array or FrequencyResponse or None
    """
    if os.path.isfile(os.path.join(dir_path, "eq.wav")):
        logger = get_logger()
        logger.warning("cli_warning_eq_wav_deprecated")
    # Default for both sides
    eq_path = _find_eq_settings_file(dir_path, "eq")
    eq_fr = None
    eq_fr_right = None
    if eq_path is not None:
        eq_fr, eq_fr_right = _read_eq_settings(eq_path, estimator)

    left_path = _find_eq_settings_file(dir_path, "eq-left")
    left_fr = None
    if left_path is not None:
        left_fr, _ = _read_eq_settings(left_path, estimator)
    elif eq_fr is not None:
        left_fr = eq_fr
    if left_fr is not None:
        left_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2, pol_order=1)

    right_path = _find_eq_settings_file(dir_path, "eq-right")
    right_fr = None
    if right_path is not None:
        candidate, candidate_right = _read_eq_settings(right_path, estimator)
        right_fr = candidate_right if candidate_right is not None else candidate
    elif eq_fr_right is not None:
        right_fr = eq_fr_right
    elif eq_fr is not None:
        right_fr = eq_fr
    if right_fr is not None and right_fr != left_fr:
        right_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2, pol_order=1)

    if left_fr is not None or right_fr is not None:
        if left_fr == right_fr:
            fig, ax = plt.subplots()
            fig.set_size_inches(12, 9)
            left_fr.plot(fig=fig, ax=ax, show_fig=False)
        else:
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
    hp_irs = HRIR(estimator)

    logger = get_logger()

    if headphone_file_path:
        # Normalize the path to handle Windows/Unix path separators
        normalized_path = os.path.normpath(headphone_file_path)
        logger.info("cli_info_hp_param_provided", file=normalized_path)

        if os.path.isdir(normalized_path):
            logger.info("cli_info_hp_searching_dir")
            possible_names = ["headphones.wav", "headphone.wav", "hp.wav", "compensation.wav"]
            actual_hp_file = None

            for name in possible_names:
                candidate = os.path.join(normalized_path, name)
                logger.debug("cli_info_hp_checking", file=candidate)
                if os.path.isfile(candidate):
                    actual_hp_file = candidate
                    logger.info("cli_info_hp_found", file=actual_hp_file)
                    break

            if actual_hp_file is None:
                logger.info("cli_info_hp_searching_wav")
                try:
                    wav_files = [f for f in os.listdir(normalized_path) if f.lower().endswith('.wav')]
                    if wav_files:
                        actual_hp_file = os.path.join(normalized_path, wav_files[0])
                        logger.info("cli_info_hp_using_first_wav", file=actual_hp_file)
                    else:
                        logger.warning("cli_warning_hp_no_wav", dir=normalized_path)
                        actual_hp_file = None
                except Exception as e:
                    logger.error("cli_error_hp_list_dir", dir=normalized_path, error=str(e))
                    actual_hp_file = None
        else:
            if not os.path.isabs(normalized_path):
                actual_hp_file = os.path.join(dir_path, normalized_path)
                logger.debug("cli_info_hp_relative_path", file=actual_hp_file)
            else:
                actual_hp_file = normalized_path
                logger.debug("cli_info_hp_absolute_path", file=actual_hp_file)
    else:
        actual_hp_file = os.path.join(dir_path, "headphones.wav")
        logger.info("cli_info_hp_default", file=actual_hp_file)

    if actual_hp_file is None or not os.path.exists(actual_hp_file):
        if headphone_file_path:
            logger.warning("cli_warning_hp_file_not_found", file=actual_hp_file)
            actual_hp_file = os.path.join(dir_path, "headphones.wav")

        if not os.path.exists(actual_hp_file):
            logger.error("cli_error_hp_file_missing", file=actual_hp_file)
            logger.error("cli_error_hp_ensure_exists", dir=dir_path)
            return None, None  # Or raise an error

    logger.info("cli_info_using_hp_file", file=actual_hp_file)
    hp_irs.open_recording(actual_hp_file, speakers=["FL", "FR"])
    hp_irs.write_wav(os.path.join(dir_path, "headphone-responses.wav"))

    left = hp_irs.irs["FL"]["left"].frequency_response()
    right = hp_irs.irs["FR"]["right"].frequency_response()

    gain = left.center([100, 10000])
    right.raw += gain

    # Compensate exactly as the original/Lion pipeline: the headphone response
    # itself is the error against a flat zero target, without min-mean recentering.
    zero = FrequencyResponse(
        name="zero",
        frequency=left.frequency,
        raw=np.zeros(len(left.frequency)),
    )
    left.compensate(zero, min_mean_error=False)
    right.compensate(zero, min_mean_error=False)

    fig = plt.figure()
    gs = fig.add_gridspec(2, 3)
    fig.set_size_inches(22, 10)
    fig.suptitle("Headphones")

    axl = fig.add_subplot(gs[0, 0])
    left.plot(fig=fig, ax=axl, show_fig=False)
    axl.set_title("Left")
    axr = fig.add_subplot(gs[1, 0])
    right.plot(fig=fig, ax=axr, show_fig=False)
    axr.set_title("Right")
    sync_axes([axl, axr])

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

    file_path = os.path.join(dir_path, "plots", "headphones.png")
    os.makedirs(os.path.split(file_path)[0], exist_ok=True)
    save_fig_as_png(file_path, fig)
    plt.close(fig)

    return left, right


def create_target(estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt):
    """Creates target frequency response with bass boost, tilt and high pass at 20 Hz"""
    target = FrequencyResponse(
        name="bass_and_tilt",
        frequency=FrequencyResponse.generate_frequencies(
            f_min=10, f_max=estimator.fs / 2, f_step=1.01
        ),
    )

    target.raw = target.create_target(
        bass_boost_gain=bass_boost_gain,
        bass_boost_fc=bass_boost_fc,
        bass_boost_q=bass_boost_q,
        tilt=tilt,
    )

    return target


def open_binaural_measurements(estimator, dir_path, debug=False):
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
        speakers = re.search(SPEAKER_LIST_PATTERN, file_name)[0].split(",")
        file_path = os.path.join(dir_path, file_name)
        hrir.open_recording(file_path, speakers=speakers, debug=debug)
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
    from i18n.localization import t

    content = f"# {t('cli_readme_title')}\n\n"
    content += t('cli_readme_processed', date=datetime.now().strftime('%Y-%m-%d %H:%M:%S'), fs=fs if fs is not None else hrir.fs) + "\n\n"

    if applied_gain is not None:
        content += f"## {t('cli_readme_gain_title')}\n"
        content += t('cli_readme_gain_value', gain=f"{applied_gain:.2f}") + "\n\n"

    table_data = []
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
                if speaker_side(speaker) == "left" and side == "right":
                    current_itd = itd
                elif speaker_side(speaker) == "right" and side == "left":
                    current_itd = itd

            pnr_val = np.nan
            length_ms = np.nan
            rt_val_ms = np.nan
            current_ir_rt_name = None

            peak_idx_current_ir = ir_obj.peak_index()
            if peak_idx_current_ir is not None:
                # PNR 계산
                peak_val_linear = np.abs(ir_obj.data[peak_idx_current_ir])
                peak_val_db = 20 * np.log10(
                    peak_val_linear + 1e-9
                )  # 피크값의 dBFS (최대값이 1.0이라고 가정)

                decay_params_tuple = ir_obj.decay_params()
                if decay_params_tuple:
                    noise_floor_db = decay_params_tuple[2]
                    if not np.isnan(noise_floor_db) and not np.isnan(peak_val_db):
                        pnr_val = peak_val_db - noise_floor_db

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

                edt, rt20, rt30, rt60 = ir_obj.decay_times(
                    peak_ind=decay_params_tuple[0] if decay_params_tuple else None,
                    knee_point_ind=decay_params_tuple[1]
                    if decay_params_tuple
                    else None,
                    noise_floor=decay_params_tuple[2] if decay_params_tuple else None,
                    window_size=decay_params_tuple[3] if decay_params_tuple else None,
                )

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
                    t(f'cli_readme_side_{side}'),
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

    if rt_values_for_naming:
        from collections import Counter

        final_rt_name = Counter(rt_values_for_naming).most_common(1)[0][0]
    else:
        final_rt_name = "RTxx"

    if table_data:
        headers = [t('cli_readme_header_speaker'), t('cli_readme_header_side'), "PNR", "ITD", t('cli_readme_header_length'), final_rt_name]
        content += tabulate(table_data, headers=headers, tablefmt="pipe")
        content += "\n\n"

    if estimator and hasattr(hrir, "calculate_reflection_levels"):
        reflection_data = hrir.calculate_reflection_levels()
        if reflection_data:
            content += f"## {t('cli_readme_reflection_title')}\n"
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
                ):
                    continue
                sides_data = reflection_data[speaker]
                content += f"### {speaker}\n"
                if "left" in sides_data and isinstance(sides_data["left"], dict):
                    content += f"- {t('cli_readme_left_ear')}: {t('cli_readme_early_label')}: {sides_data['left'].get('early_db', np.nan):.2f} dB, {t('cli_readme_late_label')}: {sides_data['left'].get('late_db', np.nan):.2f} dB\n"
                if "right" in sides_data and isinstance(sides_data["right"], dict):
                    content += f"- {t('cli_readme_right_ear')}: {t('cli_readme_early_label')}: {sides_data['right'].get('early_db', np.nan):.2f} dB, {t('cli_readme_late_label')}: {sides_data['right'].get('late_db', np.nan):.2f} dB\n"
            content += "\n"

    with open(file_path, "w", encoding="utf-8") as f:
        f.write(content)

    return content


def _print_info():
    """Print diagnostic information for bug reports (English-only output)."""
    import sys
    import platform as pf
    lines = [f"Impulcifer {__version__}"]
    lines.append(f"Python {sys.version.split()[0]}")
    lines.append(f"OS: {pf.system()} {pf.release()} ({pf.machine()})")
    lines.append(f"CPU cores: {os.cpu_count() or 'unknown'}")

    if hasattr(sys, '_is_gil_enabled'):
        gil = "Disabled (Free-Threaded)" if not sys._is_gil_enabled() else "Enabled"
    else:
        gil = "Unavailable (GIL status API missing)"
    lines.append(f"GIL: {gil}")

    try:
        from core.parallel_processing import get_python_threading_info
        info = get_python_threading_info()
        lines.append(f"Optimal workers: {info.get('optimal_workers', 'unknown')}")
    except Exception:
        pass

    try:
        from updater.updater_core import is_velopack_environment, is_pip_environment
        if is_velopack_environment():
            lines.append("Installation: Standalone (Velopack)")
        elif is_pip_environment():
            lines.append("Installation: pip package")
        else:
            lines.append("Installation: Development")
    except Exception:
        lines.append("Installation: Development")

    dep_versions = []
    for pkg in ['numpy', 'scipy', 'matplotlib', 'soundfile', 'customtkinter', 'bokeh']:
        try:
            from importlib.metadata import version as get_ver
            dep_versions.append(f"{pkg}=={get_ver(pkg)}")
        except Exception:
            pass
    if dep_versions:
        lines.append(f"Dependencies: {', '.join(dep_versions)}")

    print('\n'.join(lines))


def create_cli():
    """Build and parse CLI args, with definitions sourced from ProcessingConfig.

    Most ``--flag`` arguments are auto-registered from
    :class:`core.pipeline.ProcessingConfig` metadata via
    :func:`core.cli_builder.add_processing_config_arguments`. Only non-config
    arguments (``--info``, ``--version``, the ``--bass_boost`` shelf splitter)
    and post-processing remain here.
    """
    from dataclasses import fields as _dataclass_fields

    from core.cli_builder import add_processing_config_arguments
    from core.pipeline import ProcessingConfig

    _config_defaults = {f.name: f.default for f in _dataclass_fields(ProcessingConfig)}
    _bass_fc_default = _config_defaults["bass_boost_fc"]
    _bass_q_default = _config_defaults["bass_boost_q"]

    arg_parser = argparse.ArgumentParser()
    arg_parser.add_argument(
        "-V", "--version",
        action="version",
        version=f"Impulcifer {__version__}",
    )
    arg_parser.add_argument(
        "--info",
        action="store_true",
        default=False,
        help="Print diagnostic information (version, Python, OS, etc.) and exit.",
    )

    # All BRIR-pipeline parameters come from the dataclass:
    add_processing_config_arguments(arg_parser)

    # bass_boost is a CLI convenience that splits into 3 fields in
    # ProcessingConfig (bass_boost_gain/fc/q), so it stays manual here.
    arg_parser.add_argument(
        "--bass_boost",
        type=str,
        default=argparse.SUPPRESS,
        help="Bass boost shelf. Sub-bass frequencies will be boosted by this amount. Can be "
        "either a single value for a gain in dB or a comma separated list of three values for "
        "parameters of a low shelf filter, where the first is gain in dB, second is center "
        "frequency (Fc) in Hz and the last is quality (Q). When only a single value (gain) is "
        f"given, default values for Fc and Q are used which are {_bass_fc_default:g} Hz and "
        f"{_bass_q_default:g}, respectively. "
        'For example "--bass_boost=6" or "--bass_boost=6,150,0.69".',
    )

    args = vars(arg_parser.parse_args())

    if args.get("info"):
        _print_info()
        raise SystemExit(0)
    del args["info"]

    if args.get("dir_path") is None:
        arg_parser.error("the following arguments are required: --dir_path")

    if "bass_boost" in args:
        bass_boost = args["bass_boost"].split(",")
        if len(bass_boost) == 1:
            args["bass_boost_gain"] = float(bass_boost[0])
            args["bass_boost_fc"] = _bass_fc_default
            args["bass_boost_q"] = _bass_q_default
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
            decay = {ch: float(args["decay"]) / 1000 for ch in SPEAKER_NAMES}
        except ValueError:
            for ch_t in args["decay"].split(","):
                decay[ch_t.split(":")[0].upper()] = float(ch_t.split(":")[1]) / 1000
        args["decay"] = decay
    return args


if __name__ == "__main__":
    cli_args = create_cli()
    main(**cli_args)
