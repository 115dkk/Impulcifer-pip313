# -*- coding: utf-8 -*-
"""BRIR processing pipeline.

Two pieces:

* :class:`ProcessingConfig` — a dataclass holding every parameter that
  :func:`impulcifer.main` accepts. Each field carries CLI metadata
  (``cli_flag``, ``cli_help``, ``cli_arg_type`` …) so the argparse definition
  is auto-generated from this single source of truth.

* :class:`BRIRPipeline` — wraps the BRIR-generation stages
  (estimate → room correction → HP compensation → EQ → align → normalize →
  output) as explicit methods. The orchestration in
  :func:`impulcifer.main` constructs ``ProcessingConfig`` from kwargs and calls
  :meth:`BRIRPipeline.run`.

BRIR output byte-exactness is pinned by ``tests/test_brir_integrity.py``.
"""

from __future__ import annotations

from dataclasses import dataclass, field, fields
from typing import Any, Optional


@dataclass
class ProcessingConfig:
    """Structured representation of all BRIR generation parameters.

    Field metadata (``cli_flag``, ``cli_help``, ``cli_arg_type`` …) is consumed
    by the GUI/argparse generators. ``cli_skip=True`` means the field
    should not appear on the CLI (handled manually in ``create_cli``).
    """

    # ---- Paths and IO ---------------------------------------------------
    dir_path: Optional[str] = field(
        default=None,
        metadata={
            "cli_flag": "--dir_path",
            "cli_help": "Path to directory for recordings and outputs.",
            "cli_arg_type": "str",
        },
    )
    test_signal: Optional[str] = field(
        default=None,
        metadata={
            "cli_flag": "--test_signal",
            "cli_help": (
                "Path to sine sweep test signal or pickled impulse response estimator. "
                "You can also use a predefined name or number: "
                '"default"/"1" (.pkl), "sweep"/"2" (.wav), "stereo"/"3" (FL,FR), '
                '"mono-left"/"4" (FL mono), "left"/"5" (FL stereo), "right"/"6" (FR stereo).'
            ),
            "cli_arg_type": "str",
            "cli_suppress_default": True,
        },
    )
    room_target: Optional[str] = field(
        default=None,
        metadata={
            "cli_flag": "--room_target",
            "cli_help": "Path to room target response AutoEQ style CSV file.",
            "cli_arg_type": "str",
            "cli_suppress_default": True,
        },
    )
    room_mic_calibration: Optional[str] = field(
        default=None,
        metadata={
            "cli_flag": "--room_mic_calibration",
            "cli_help": "Path to room measurement microphone calibration file.",
            "cli_arg_type": "str",
            "cli_suppress_default": True,
        },
    )
    headphone_compensation_file: Optional[str] = field(
        default=None,
        metadata={
            "cli_flag": "--headphone_compensation_file",
            "cli_help": (
                'Path to the headphone compensation WAV file. Defaults to '
                '"headphones.wav" in dir_path.'
            ),
            "cli_arg_type": "str",
        },
    )

    # ---- Sampling rate / plotting --------------------------------------
    fs: Optional[int] = field(
        default=None,
        metadata={
            "cli_flag": "--fs",
            "cli_help": "Output sampling rate in Hertz.",
            "cli_arg_type": "int",
            "cli_suppress_default": True,
        },
    )
    plot: bool = field(
        default=False,
        metadata={
            "cli_flag": "--plot",
            "cli_help": "Plot graphs for debugging.",
            "cli_arg_action": "store_true",
        },
    )
    interactive_plots: bool = field(
        default=False,
        metadata={
            "cli_flag": "--interactive_plots",
            "cli_help": "Generate interactive Bokeh plots in HTML files.",
            "cli_arg_action": "store_true",
        },
    )

    # ---- Channel balance / decay / target level ------------------------
    channel_balance: Optional[str] = field(
        default=None,
        metadata={
            "cli_flag": "--channel_balance",
            "cli_help": (
                "Channel balance correction by equalizing left and right ear results to the same "
                'level or frequency response. "trend" equalizes right side by the difference trend '
                'of right and left side. "left" equalizes right side to left side fr, "right" '
                'equalizes left side to right side fr, "avg" equalizes both to the average fr, "min" '
                "equalizes both to the minimum of left and right side frs. Number values will boost "
                'or attenuate right side relative to left side by the number of dBs. "mids" is the '
                "same as the numerical values but guesses the value automatically from mid frequency "
                "levels."
            ),
            "cli_arg_type": "str",
            "cli_suppress_default": True,
        },
    )
    decay: Optional[Any] = field(
        default=None,
        metadata={
            "cli_flag": "--decay",
            "cli_help": (
                "Target decay time in milliseconds to reach -60 dB. When the natural decay time is "
                "longer than the target decay time, a downward slope will be applied to decay tail. "
                "Decay cannot be increased with this. By default no decay time adjustment is done. "
                "A comma separated list of channel name and  reverberation time pairs, separated by "
                "a colon. If only a single numeric value is given, it is used for all channels. When "
                "some channel names are give but not all, the missing channels are not affected. For "
                'example "--decay=300" or "--decay=FL:500,FC:100,FR:500,SR:700,BR:700,BL:700,SL:700" '
                'or "--decay=FC:100".'
            ),
            "cli_arg_type": "str",
            "cli_suppress_default": True,
            "cli_postprocess": "decay",
        },
    )
    target_level: Optional[float] = field(
        default=None,
        metadata={
            "cli_flag": "--target_level",
            "cli_help": (
                "Target average gain level for left and right channels. This will sum together all "
                "left side impulse responses and right side impulse responses respectively and take "
                "the average gain from mid frequencies. The averaged level is then normalized to the "
                "given target level. This makes it possible to compare HRIRs with somewhat similar "
                "loudness levels. This should be negative in most cases to avoid clipping."
            ),
            "cli_arg_type": "float",
            "cli_suppress_default": True,
        },
    )

    # ---- FR combination / room limits / bass / tilt --------------------
    fr_combination_method: str = field(
        default="average",
        metadata={
            "cli_flag": "--fr_combination_method",
            "cli_help": (
                "Method for combining frequency responses of generic room measurements if there are "
                'more than one tracks in the file. "average" will simply average the frequency'
                'responses. "conservative" will take the minimum absolute value for each frequency '
                "but only if the values in all the measurements are positive or negative at the same "
                "time."
            ),
            "cli_arg_type": "str",
        },
    )
    specific_limit: float = field(
        default=400,
        metadata={
            "cli_flag": "--specific_limit",
            "cli_help": (
                "Upper limit for room equalization with speaker-ear specific room measurements. "
                "Equalization will drop down to 0 dB at this frequency in the leading octave. 0 "
                "disables limit."
            ),
            "cli_arg_type": "float",
        },
    )
    generic_limit: float = field(
        default=300,
        metadata={
            "cli_flag": "--generic_limit",
            "cli_help": (
                "Upper limit for room equalization with generic room measurements. "
                "Equalization will drop down to 0 dB at this frequency in the leading octave. 0 "
                "disables limit."
            ),
            "cli_arg_type": "float",
        },
    )
    bass_boost_gain: float = field(
        default=0.0,
        metadata={"cli_skip": True},
    )
    bass_boost_fc: float = field(
        default=105,
        metadata={"cli_skip": True},
    )
    bass_boost_q: float = field(
        default=0.76,
        metadata={"cli_skip": True},
    )
    tilt: float = field(
        default=0.0,
        metadata={
            "cli_flag": "--tilt",
            "cli_help": (
                "Target tilt in dB/octave. Positive value (upwards slope) will result in brighter "
                "frequency response and negative value (downwards slope) will result in darker "
                "frequency response. 1 dB/octave will produce nearly 10 dB difference in "
                "desired value between 20 Hz and 20 kHz. Tilt is applied with bass boost and both "
                "will affect the bass gain."
            ),
            "cli_arg_type": "float",
            "cli_suppress_default": True,
        },
    )

    # ---- Stage toggles -------------------------------------------------
    do_room_correction: bool = field(
        default=True,
        metadata={
            "cli_flag": "--no_room_correction",
            "cli_help": "Skip room correction.",
            "cli_arg_action": "store_false",
            "cli_dest": "do_room_correction",
        },
    )
    do_headphone_compensation: bool = field(
        default=True,
        metadata={
            "cli_flag": "--no_headphone_compensation",
            "cli_help": "Skip headphone compensation.",
            "cli_arg_action": "store_false",
            "cli_dest": "do_headphone_compensation",
        },
    )
    do_equalization: bool = field(
        default=True,
        metadata={
            "cli_flag": "--no_equalization",
            "cli_help": "Skip equalization.",
            "cli_arg_action": "store_false",
            "cli_dest": "do_equalization",
        },
    )

    # ---- Misc ----------------------------------------------------------
    head_ms: float = field(
        default=1.0,
        metadata={
            "cli_flag": "--c",
            "cli_help": "Head room in milliseconds for cropping impulse response heads. Default is 1.0 (ms).",
            "cli_arg_type": "float",
            "cli_dest": "head_ms",
        },
    )
    jamesdsp: bool = field(
        default=False,
        metadata={
            "cli_flag": "--jamesdsp",
            "cli_help": "Generate true stereo IR file (jamesdsp.wav) for JamesDSP from FL/FR channels.",
            "cli_arg_action": "store_true",
        },
    )
    hangloose: bool = field(
        default=False,
        metadata={
            "cli_flag": "--hangloose",
            "cli_help": "Generate separate stereo IR for each channel for Hangloose Convolver.",
            "cli_arg_action": "store_true",
        },
    )

    # ---- Microphone deviation correction -------------------------------
    microphone_deviation_correction: bool = field(
        default=False,
        metadata={
            "cli_flag": "--microphone_deviation_correction",
            "cli_help": (
                "Enable v4.0 interaural microphone mismatch correction "
                "(direction-independent left/right level mismatch from mic "
                "placement/sensitivity). Skipped automatically when headphone "
                "compensation is enabled, since the mic response already cancels there."
            ),
            "cli_arg_action": "store_true",
        },
    )
    mic_deviation_strength: float = field(
        default=0.7,
        metadata={
            "cli_flag": "--mic_deviation_strength",
            "cli_help": "Microphone deviation correction strength (0.0-1.0). 0.0 = no correction, 1.0 = full correction. Default is 0.7.",
            "cli_arg_type": "float",
        },
    )
    mic_deviation_debug_plots: bool = field(
        default=False,
        metadata={
            "cli_flag": "--mic_deviation_debug_plots",
            "cli_help": "Save debug plots for microphone deviation correction. (Default: disabled)",
            "cli_arg_action": "store_true",
        },
    )

    # ---- TrueHD layouts ------------------------------------------------
    output_truehd_layouts: bool = field(
        default=False,
        metadata={
            "cli_flag": "--output_truehd_layouts",
            "cli_help": "Generate TrueHD layouts.",
            "cli_arg_action": "store_true",
        },
    )

    # ---- Virtual bass --------------------------------------------------
    vbass: bool = field(
        default=False,
        metadata={
            "cli_flag": "--vbass",
            "cli_help": "Enable virtual bass synthesis.",
            "cli_arg_action": "store_true",
        },
    )
    vbass_freq: int = field(
        default=250,
        metadata={
            "cli_flag": "--vbass_freq",
            "cli_help": "Virtual bass crossover frequency in Hz (default: 250).",
            "cli_arg_type": "int",
        },
    )
    vbass_hp: float = field(
        default=15.0,
        metadata={
            "cli_flag": "--vbass_hp",
            "cli_help": "Virtual bass sub-bass high-pass frequency in Hz (default: 15.0).",
            "cli_arg_type": "float",
        },
    )
    vbass_polarity: str = field(
        default="auto",
        metadata={
            "cli_flag": "--vbass_polarity",
            "cli_help": "Virtual bass polarity handling (default: auto).",
            "cli_arg_type": "str",
            "cli_choices": ("auto", "normal", "invert"),
        },
    )

    @classmethod
    def from_kwargs(cls, **kwargs) -> "ProcessingConfig":
        """Build a config, ignoring kwargs that aren't config fields.

        Used by ``impulcifer.main(**kwargs)`` so callers can pass extra keys
        (e.g. CLI-only sentinels) without breaking the dataclass.
        """
        valid_names = {f.name for f in fields(cls)}
        cleaned = {k: v for k, v in kwargs.items() if k in valid_names}
        return cls(**cleaned)


class BRIRPipeline:
    """Orchestrates the BRIR generation stages.

    Constructs a logger, computes total step count from the config, and
    delegates per-stage work to small private methods. BRIR output
    byte-exactness is pinned by ``tests/test_brir_integrity.py`` (see the
    module docstring).
    """

    def __init__(self, config: ProcessingConfig):
        self.config = config

    def run(self) -> None:
        """Run the full stage sequence (estimator -> room/headphone/
        EQ -> target -> HRIR open -> crop/align -> virtual bass ->
        mic-deviation -> equalize -> decay -> balance -> normalize -> plots ->
        resample -> write). Output byte-exactness is guarded by
        tests/test_brir_integrity.py. DSP helpers still live in impulcifer and
        are imported lazily here to keep the import direction one-way."""
        import os
        import io
        import contextlib
        import matplotlib.pyplot as plt
        from autoeq.frequency_response import FrequencyResponse
        from bokeh.models import TabPanel, Tabs
        from bokeh.plotting import output_file as bokeh_output_file, save as bokeh_save
        from infra.logger import get_logger
        from core.room_correction import room_correction
        from core.parallel_utils import parallel_map, get_parallelization_info
        from core.parallel_workers import (
            init_equalization_worker,
            process_equalization_worker,
            process_decay_worker,
        )
        from core.constants import (
            HESUVI_TRACK_ORDER,
            TRUEHD_11CH_ORDER,
            TRUEHD_13CH_ORDER,
            SPEAKER_NAMES,
            IPSILATERAL_PAIRS,
        )
        from core.channel_generation import (
            validate_channel_requirements,
            get_available_channels_for_layout,
            create_truehd_layout_track_order,
        )
        from core.cancellation import check_cancelled as _check_cancelled
        from impulcifer import (
            open_impulse_response_estimator,
            headphone_compensation,
            equalization,
            create_target,
            open_binaural_measurements,
            write_readme,
            _save_bokeh_analysis_plots,
        )

        cfg = self.config
        dir_path = cfg.dir_path
        test_signal = cfg.test_signal
        room_target = cfg.room_target
        room_mic_calibration = cfg.room_mic_calibration
        headphone_compensation_file = cfg.headphone_compensation_file
        fs = cfg.fs
        plot = cfg.plot
        interactive_plots = cfg.interactive_plots
        channel_balance = cfg.channel_balance
        decay = cfg.decay
        target_level = cfg.target_level
        fr_combination_method = cfg.fr_combination_method
        specific_limit = cfg.specific_limit
        generic_limit = cfg.generic_limit
        bass_boost_gain = cfg.bass_boost_gain
        bass_boost_fc = cfg.bass_boost_fc
        bass_boost_q = cfg.bass_boost_q
        tilt = cfg.tilt
        do_room_correction = cfg.do_room_correction
        do_headphone_compensation = cfg.do_headphone_compensation
        do_equalization = cfg.do_equalization
        head_ms = cfg.head_ms
        jamesdsp = cfg.jamesdsp
        hangloose = cfg.hangloose
        microphone_deviation_correction = cfg.microphone_deviation_correction
        mic_deviation_strength = cfg.mic_deviation_strength
        mic_deviation_debug_plots = cfg.mic_deviation_debug_plots
        output_truehd_layouts = cfg.output_truehd_layouts
        vbass = cfg.vbass
        vbass_freq = cfg.vbass_freq
        vbass_hp = cfg.vbass_hp
        vbass_polarity = cfg.vbass_polarity

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
        # 헤드폰 보상이 켜져 있으면 마이크 보정은 보상 단계에서 이미 상쇄되므로
        # 건너뛴다(아래 게이팅 참조). 진행 단계 수도 그에 맞춰 센다.
        if microphone_deviation_correction and not do_headphone_compensation:
            total_steps += 1
        if vbass:
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
        logger.info("cli_starting_brir_generation", total_steps=total_steps)

        if plot:
            try:
                import seaborn as sns

                sns.set_theme(style="whitegrid")
                logger.debug("Seaborn style applied to plots")
            except ImportError:
                logger.debug("Seaborn not installed, using default matplotlib style")

        if dir_path is None or not os.path.isdir(dir_path):
            raise NotADirectoryError(f'Given dir path "{dir_path}"" is not a directory.')

        dir_path = os.path.abspath(dir_path)
        _check_cancelled()

        # Impulse response estimator
        logger.step("cli_creating_estimator")
        estimator = open_impulse_response_estimator(dir_path, file_path=test_signal)
        _check_cancelled()

        # Room correction frequency responses
        room_frs = None
        if do_room_correction:
            logger.step("cli_running_room_correction")
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
            _check_cancelled()

        # Headphone compensation frequency responses
        hp_left, hp_right = None, None
        if do_headphone_compensation:
            logger.step("cli_running_headphone_compensation")
            hp_left, hp_right = headphone_compensation(
                estimator, dir_path, headphone_compensation_file
            )
            _check_cancelled()

        # Equalization
        eq_left, eq_right = None, None
        if do_equalization:
            logger.step("cli_creating_equalization")
            eq_left, eq_right = equalization(estimator, dir_path)
            _check_cancelled()

        # Bass boost and tilt
        logger.step("cli_creating_target")
        target = create_target(
            estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt
        )
        _check_cancelled()

        # HRIR measurements
        logger.step("cli_opening_measurements")
        hrir = open_binaural_measurements(estimator, dir_path, debug=plot)
        _check_cancelled()

        if plot:
            # Plot graphs pre processing
            os.makedirs(os.path.join(dir_path, "plots", "pre"), exist_ok=True)
            logger.step("cli_plotting_pre")
            hrir.plot(dir_path=os.path.join(dir_path, "plots", "pre"))
            _check_cancelled()

        # Crop noise and harmonics from the beginning
        logger.step("cli_cropping_responses")
        hrir.crop_heads(head_ms=head_ms)

        hrir.align_ipsilateral_all(
            speaker_pairs=list(IPSILATERAL_PAIRS),
            segment_ms=30,
        )
        hrir.align_onset_groups_peak_leftref()

        # Crop noise from the tail
        hrir.crop_tails()
        _check_cancelled()

        # Virtual bass synthesis
        if vbass:
            logger.step("vbass_status_processing")
            from core.virtual_bass import apply_virtual_bass_to_hrir
            polarity_map = {'auto': None, 'normal': False, 'invert': True}
            apply_virtual_bass_to_hrir(
                hrir,
                crossover_freq=vbass_freq,
                head_ms=head_ms,
                hp_freq=vbass_hp,
                invert_polarity=polarity_map.get(vbass_polarity),
            )
            _check_cancelled()

        # 마이크 착용 편차 보정 v4.0
        # 같은 인이어 마이크를 같은 위치에 두고 스피커와 헤드폰을 모두 측정한 경우,
        # 마이크 전달함수는 헤드폰 보상 단계(out = HRTF/HpTF)에서 귀별로 소거된다
        # (Hammershøi & Møller 2005). 보상 이전에 마이크 보정을 적용하면 잉여이자
        # 좌우 밸런스를 이중 보정하므로, 헤드폰 보상이 켜져 있으면 건너뛴다.
        if microphone_deviation_correction:
            if do_headphone_compensation:
                logger.warning("cli_mic_deviation_skipped_hpcomp")
            else:
                logger.step("cli_correcting_deviation")
                mic_deviation_plot_dir = os.path.join(dir_path, "plots") if mic_deviation_debug_plots else None
                hrir.correct_microphone_deviation(
                    correction_strength=mic_deviation_strength,
                    plot_analysis=mic_deviation_debug_plots,
                    plot_dir=mic_deviation_plot_dir,
                )
                _check_cancelled()

        # Write multi-channel WAV file with sine sweeps for debugging
        _check_cancelled()
        hrir.write_wav(os.path.join(dir_path, "responses.wav"))

        # Equalize all
        if do_headphone_compensation or do_room_correction or do_equalization:
            logger.step("cli_equalizing")

            parallel_info = get_parallelization_info()
            logger.info("cli_info_parallel_executor", executor=parallel_info['executor_type'], version=parallel_info['python_version'], status='disabled' if parallel_info['gil_disabled'] else 'enabled')

            # Pre-generate common frequency array to reduce allocations
            common_freq = FrequencyResponse.generate_frequencies(
                f_step=1.01, f_min=10, f_max=estimator.fs / 2
            )

            # Worker는 ir 객체를 사용하지 않으므로 task tuple에서 제외해 IPC pickle
            # 비용과 ImpulseResponse 객체 직렬화 부담을 제거한다.
            eq_tasks = []
            for speaker, pair in hrir.irs.items():
                for side in pair.keys():
                    eq_tasks.append((speaker, side))

            logger.info("cli_info_parallel_eq", count=len(eq_tasks))
            eq_results = parallel_map(
                process_equalization_worker,
                eq_tasks,
                initializer=init_equalization_worker,
                initargs=(
                    room_frs,
                    hp_left,
                    hp_right,
                    eq_left,
                    eq_right,
                    target,
                    common_freq,
                    estimator.fs,
                ),
            )

            for speaker, side, fir in eq_results:
                hrir.irs[speaker][side].equalize(fir)
            _check_cancelled()

        # Adjust decay time
        if decay:
            logger.step("cli_adjusting_decay")

            decay_tasks = []
            for speaker, pair in hrir.irs.items():
                if speaker in decay:
                    for side, ir in pair.items():
                        decay_tasks.append((speaker, side, ir.data, decay[speaker], estimator.fs))

            if decay_tasks:
                logger.info("cli_info_parallel_decay", count=len(decay_tasks))
                decay_results = parallel_map(process_decay_worker, decay_tasks)

                for speaker, side, adjusted_data in decay_results:
                    hrir.irs[speaker][side].data = adjusted_data
            _check_cancelled()

        # Correct channel balance
        if channel_balance is not None:
            logger.step("cli_correcting_balance")
            hrir.correct_channel_balance(channel_balance)
            _check_cancelled()

        # Normalize gain after all processing, matching the original Impulcifer pipeline.
        logger.step("cli_normalizing_gain")
        applied_gain = hrir.normalize(
            peak_target=None if target_level is not None else -0.1, avg_target=target_level
        )
        _check_cancelled()

        # Write info and stats in readme
        readme_content = write_readme(
            os.path.join(dir_path, "README.md"), hrir, fs, estimator, applied_gain
        )
        if readme_content:
            logger.info(readme_content)
        _check_cancelled()

        if plot:
            logger.step("cli_plotting_post")

            # Compute convolutions for recording plots serially. Each convolution is
            # ~scipy.signal.convolve一행짜리 작업이라 ProcessPoolExecutor 14개 워커
            # (각 ~150MB) 비용이 직렬 실행보다 훨씬 비싸다. 추가로, 워커가 반환한
            # recording 배열을 plot_tasks/plot_results 튜플이 보유하면 hrir 메모리
            # 회수 시점까지 참조가 살아남아 ~3GB 잔류를 일으킨다.
            from scipy.signal import convolve
            for speaker, pair in hrir.irs.items():
                for side, ir in pair.items():
                    ir.recording = convolve(estimator.test_signal, ir.data, mode="full")

            # Plot post processing
            hrir.plot(os.path.join(dir_path, "plots", "post"))
            _check_cancelled()

        # Plot results, always
        logger.step("cli_plotting_results")
        hrir.plot_result(os.path.join(dir_path, "plots"))
        _check_cancelled()

        if plot:
            logger.step("cli_plotting_additional")
            hrir.plot_interaural_impulse_overlay(
                os.path.join(dir_path, "plots", "interaural_overlay")
            )
            # ILD/IPD/IACC/ETC는 Bokeh 레이아웃만 존재하므로 HTML로 저장
            _save_bokeh_analysis_plots(hrir, dir_path, logger)
            _check_cancelled()

        if interactive_plots:
            logger.step("cli_generating_interactive")
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
                        )
                        panels.append(panel)
                    else:
                        logger.debug("cli_warning_plot_skipped", title=title)
                except Exception as e:
                    logger.warning("cli_warning_interactive_plot_error", title=title, error=str(e))

            if panels:
                tabs = Tabs(tabs=panels, sizing_mode="stretch_both")
                output_html_path = os.path.join(
                    interactive_plot_dir, "interactive_summary.html"
                )
                bokeh_output_file(
                    output_html_path, title="Interactive Plot Summary"
                )
                bokeh_save(tabs)
                logger.success("cli_success_interactive_saved", path=output_html_path)
            else:
                logger.warning("cli_warning_no_interactive")
            _check_cancelled()

        # Re-sample
        if fs is not None and fs != hrir.fs:
            logger.step("cli_resampling", fs=fs)
            hrir.resample(fs)
            hrir.normalize(
                peak_target=None if target_level is not None else -0.1,
                avg_target=target_level,
            )
            _check_cancelled()

        # Write multi-channel WAV file with standard track order
        logger.step("cli_writing_brirs")
        _check_cancelled()
        hrir.write_wav(os.path.join(dir_path, "hrir.wav"))

        # Write multi-channel WAV file with HeSuVi track order
        _check_cancelled()
        hrir.write_wav(os.path.join(dir_path, "hesuvi.wav"), track_order=HESUVI_TRACK_ORDER)

        # TrueHD 레이아웃 출력
        if output_truehd_layouts:
            logger.step("cli_generating_truehd")

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
                logger.success("cli_success_truehd_11ch", path=output_path_11ch)
            else:
                logger.warning("cli_warning_truehd_11ch_fail", msg=msg_11ch)

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
                logger.success("cli_success_truehd_13ch", path=output_path_13ch)
            else:
                logger.warning("cli_warning_truehd_13ch_fail", msg=msg_13ch)
            _check_cancelled()

        if jamesdsp:
            logger.step("cli_generating_jamesdsp")

            dsp_hrir = hrir.subset(["FL", "FR"], copy_irs=True)

            # normalize 내부의 print문 출력을 숨기기 위해 stdout 리디렉션
            with contextlib.redirect_stdout(io.StringIO()):
                dsp_hrir.normalize(
                    peak_target=None if target_level is not None else -0.1,
                    avg_target=target_level,
                )

            # FL-L, FL-R, FR-L, FR-R 순서로 파일 생성
            jd_order = ["FL-left", "FL-right", "FR-left", "FR-right"]
            out_path = os.path.join(dir_path, "jamesdsp.wav")
            dsp_hrir.write_wav(out_path, track_order=jd_order)
            del dsp_hrir
            logger.success("cli_success_jamesdsp", path=out_path)
            _check_cancelled()

        if hangloose:
            logger.step("cli_generating_hangloose")
            output_dir = os.path.join(dir_path, "Hangloose")
            os.makedirs(output_dir, exist_ok=True)

            processed_speakers = [sp for sp in SPEAKER_NAMES if sp in hrir.irs]

            for sp in processed_speakers:
                track_order = [f"{sp}-left", f"{sp}-right"]
                out_path = os.path.join(output_dir, f"{sp}.wav")
                hrir.write_wav(out_path, track_order=track_order)
                logger.info("cli_success_hangloose_file", file=f"{sp}.wav")

            logger.success("cli_success_hangloose", path=output_dir)
            _check_cancelled()

        # 메모리 회수: 중간 변수 → 루트 객체 순서로 삭제 후 GC 수행.
        # eq_tasks/decay_tasks 등 중간 변수가 hrir/estimator 내부 데이터에 대한
        # 참조를 유지하고 있어, 루트 객체를 먼저 삭제하면 내부 데이터가 해제되지
        # 않는다. 반드시 중간 변수를 먼저 삭제한 후 루트 객체를 삭제할 것.
        try:
            del eq_tasks, eq_results
        except NameError:
            pass
        try:
            del decay_tasks, decay_results
        except NameError:
            pass

        del hrir
        del estimator
        try:
            del room_frs, hp_left, hp_right, eq_left, eq_right, target
        except NameError:
            pass

        # safety net: 정상 경로에서는 hrir.plot()이 figure를 닫지만, 예외 발생
        # 시 닫히지 않은 figure가 남을 수 있어 전체 close를 한 번 더 호출한다.
        plt.close('all')

        import gc
        gc.collect()
