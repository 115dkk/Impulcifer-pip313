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
                "Test signal source. Defaults to automatic detection: <dir>/test.wav if present, "
                "otherwise the sweep parameters are recovered from the recordings themselves "
                "(falling back to the bundled default sweep). Accepts a path to a sine sweep WAV "
                'file, the literal "auto", "generate:<duration>s@<fs>" (e.g. "generate:6.15s@48000") '
                "to construct the sweep from parameters, or a predefined name/number: "
                '"default"/"1", "sweep"/"2", "stereo"/"3" (FL,FR), '
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

    :meth:`run` builds a stage table (one row per stage: enabled flag,
    progress-step count, bound method), derives the progress total from the
    same table it executes, and runs the enabled stages in order. Stage
    methods read :attr:`config` directly and share intermediate state as
    instance attributes. BRIR output byte-exactness is pinned by
    ``tests/test_brir_integrity.py`` (see the module docstring).
    """

    def __init__(self, config: ProcessingConfig):
        self.config = config
        self.logger = None
        self.dir_path = None
        self.estimator = None
        self.hrir = None
        self.room_frs = None
        self.hp_left = None
        self.hp_right = None
        self.eq_left = None
        self.eq_right = None
        self.target = None
        self.applied_gain = None

    # ------------------------------------------------------------------
    # Orchestration
    # ------------------------------------------------------------------
    def _stage_table(self):
        """One row per stage: ``(enabled, progress_steps, method)``.

        The progress total and the execution sequence both derive from this
        table, so each stage's gating policy is written exactly once.
        """
        cfg = self.config
        any_equalization = (
            cfg.do_headphone_compensation or cfg.do_room_correction or cfg.do_equalization
        )
        # 헤드폰 보상이 켜져 있으면 마이크 전달함수는 보상 단계(out = HRTF/HpTF)
        # 에서 귀별로 소거되므로(Hammershøi & Møller 2005) 마이크 편차 보정은
        # 잉여이자 좌우 밸런스 이중 보정이 된다 — 건너뛰고 경고만 남긴다.
        mic_deviation_active = (
            cfg.microphone_deviation_correction and not cfg.do_headphone_compensation
        )
        mic_deviation_skipped = (
            cfg.microphone_deviation_correction and cfg.do_headphone_compensation
        )
        return [
            (True, 1, self._stage_estimator),
            (cfg.do_room_correction, 1, self._stage_room_correction),
            (cfg.do_headphone_compensation, 1, self._stage_headphone_compensation),
            (cfg.do_equalization, 1, self._stage_equalization_files),
            (True, 1, self._stage_target),
            (True, 1, self._stage_open_measurements),
            (cfg.plot, 1, self._stage_plot_pre),
            (True, 1, self._stage_crop_and_align),
            (cfg.vbass, 1, self._stage_virtual_bass),
            (mic_deviation_skipped, 0, self._stage_mic_deviation_skipped),
            (mic_deviation_active, 1, self._stage_mic_deviation),
            (True, 0, self._stage_write_responses),
            (any_equalization, 1, self._stage_equalize),
            (bool(cfg.decay), 1, self._stage_decay),
            (cfg.channel_balance is not None, 1, self._stage_channel_balance),
            (True, 1, self._stage_normalize),
            (True, 0, self._stage_write_readme),
            (cfg.plot, 1, self._stage_plot_post),
            (True, 1, self._stage_plot_results),
            (cfg.plot, 1, self._stage_plot_additional),
            (cfg.interactive_plots, 1, self._stage_interactive_plots),
            (cfg.fs is not None, 1, self._stage_resample),
            (True, 1, self._stage_write_brirs),
            (cfg.output_truehd_layouts, 1, self._stage_truehd_layouts),
            (cfg.jamesdsp, 1, self._stage_jamesdsp),
            (cfg.hangloose, 1, self._stage_hangloose),
        ]

    def run(self) -> None:
        """Run the full stage sequence (estimator -> room/headphone/
        EQ -> target -> HRIR open -> crop/align -> virtual bass ->
        mic-deviation -> equalize -> decay -> balance -> normalize -> plots ->
        resample -> write). Output byte-exactness is guarded by
        tests/test_brir_integrity.py. DSP stage helpers live in
        core.pipeline_stages and are imported lazily inside the stage methods
        so constructing a pipeline stays cheap until run() is called."""
        import os

        from core.cancellation import check_cancelled
        from infra.logger import get_logger

        cfg = self.config
        self.logger = get_logger()

        stages = self._stage_table()
        total_steps = sum(steps for enabled, steps, _ in stages if enabled)
        self.logger.set_total_steps(total_steps)
        self.logger.info("cli_starting_brir_generation", total_steps=total_steps)

        if cfg.plot:
            try:
                import seaborn as sns

                sns.set_theme(style="whitegrid")
                self.logger.debug("Seaborn style applied to plots")
            except ImportError:
                self.logger.debug("Seaborn not installed, using default matplotlib style")

        if cfg.dir_path is None or not os.path.isdir(cfg.dir_path):
            raise NotADirectoryError(f'Given dir path "{cfg.dir_path}"" is not a directory.')

        self.dir_path = os.path.abspath(cfg.dir_path)
        check_cancelled()

        for enabled, _steps, stage in stages:
            if enabled:
                stage()

        self._cleanup()

    # ------------------------------------------------------------------
    # Stages
    # ------------------------------------------------------------------
    def _stage_estimator(self):
        from core.cancellation import check_cancelled
        from core.pipeline_stages import open_impulse_response_estimator

        self.logger.step("cli_creating_estimator")
        self.estimator = open_impulse_response_estimator(
            self.dir_path, file_path=self.config.test_signal
        )
        check_cancelled()

    def _stage_room_correction(self):
        from core.cancellation import check_cancelled
        from core.room_correction import room_correction

        cfg = self.config
        self.logger.step("cli_running_room_correction")
        _, self.room_frs = room_correction(
            self.estimator,
            self.dir_path,
            target=cfg.room_target,
            mic_calibration=cfg.room_mic_calibration,
            fr_combination_method=cfg.fr_combination_method,
            specific_limit=cfg.specific_limit,
            generic_limit=cfg.generic_limit,
            plot=cfg.plot,
        )
        check_cancelled()

    def _stage_headphone_compensation(self):
        from core.cancellation import check_cancelled
        from core.pipeline_stages import headphone_compensation

        self.logger.step("cli_running_headphone_compensation")
        self.hp_left, self.hp_right = headphone_compensation(
            self.estimator, self.dir_path, self.config.headphone_compensation_file
        )
        check_cancelled()

    def _stage_equalization_files(self):
        from core.cancellation import check_cancelled
        from core.pipeline_stages import equalization

        self.logger.step("cli_creating_equalization")
        self.eq_left, self.eq_right = equalization(self.estimator, self.dir_path)
        check_cancelled()

    def _stage_target(self):
        from core.cancellation import check_cancelled
        from core.pipeline_stages import create_target

        cfg = self.config
        self.logger.step("cli_creating_target")
        self.target = create_target(
            self.estimator, cfg.bass_boost_gain, cfg.bass_boost_fc, cfg.bass_boost_q, cfg.tilt
        )
        check_cancelled()

    def _stage_open_measurements(self):
        from core.cancellation import check_cancelled
        from core.pipeline_stages import open_binaural_measurements

        self.logger.step("cli_opening_measurements")
        self.hrir = open_binaural_measurements(
            self.estimator, self.dir_path, debug=self.config.plot
        )
        check_cancelled()

    def _stage_plot_pre(self):
        import os

        from core.cancellation import check_cancelled

        os.makedirs(os.path.join(self.dir_path, "plots", "pre"), exist_ok=True)
        self.logger.step("cli_plotting_pre")
        self.hrir.plot(dir_path=os.path.join(self.dir_path, "plots", "pre"))
        check_cancelled()

    def _stage_crop_and_align(self):
        from core.cancellation import check_cancelled
        from core.constants import IPSILATERAL_PAIRS

        # Crop noise and harmonics from the beginning
        self.logger.step("cli_cropping_responses")
        self.hrir.crop_heads(head_ms=self.config.head_ms)

        self.hrir.align_ipsilateral_all(
            speaker_pairs=list(IPSILATERAL_PAIRS),
            segment_ms=30,
        )
        self.hrir.align_onset_groups_peak_leftref()

        # Crop noise from the tail
        self.hrir.crop_tails()
        check_cancelled()

    def _stage_virtual_bass(self):
        from core.cancellation import check_cancelled
        from core.virtual_bass import apply_virtual_bass_to_hrir

        cfg = self.config
        self.logger.step("vbass_status_processing")
        polarity_map = {'auto': None, 'normal': False, 'invert': True}
        apply_virtual_bass_to_hrir(
            self.hrir,
            crossover_freq=cfg.vbass_freq,
            head_ms=cfg.head_ms,
            hp_freq=cfg.vbass_hp,
            invert_polarity=polarity_map.get(cfg.vbass_polarity),
        )
        check_cancelled()

    def _stage_mic_deviation_skipped(self):
        self.logger.warning("cli_mic_deviation_skipped_hpcomp")

    def _stage_mic_deviation(self):
        import os

        from core.cancellation import check_cancelled

        cfg = self.config
        self.logger.step("cli_correcting_deviation")
        mic_deviation_plot_dir = (
            os.path.join(self.dir_path, "plots") if cfg.mic_deviation_debug_plots else None
        )
        self.hrir.correct_microphone_deviation(
            correction_strength=cfg.mic_deviation_strength,
            plot_analysis=cfg.mic_deviation_debug_plots,
            plot_dir=mic_deviation_plot_dir,
        )
        check_cancelled()

    def _stage_write_responses(self):
        """Write multi-channel WAV file with sine sweeps for debugging."""
        import os

        from core.cancellation import check_cancelled

        check_cancelled()
        self.hrir.write_wav(os.path.join(self.dir_path, "responses.wav"))

    def _stage_equalize(self):
        from autoeq.frequency_response import FrequencyResponse
        from core.cancellation import check_cancelled
        from core.parallel_utils import parallel_map, get_parallelization_info
        from core.parallel_workers import (
            init_equalization_worker,
            process_equalization_worker,
        )

        self.logger.step("cli_equalizing")

        parallel_info = get_parallelization_info()
        self.logger.info("cli_info_parallel_executor", executor=parallel_info['executor_type'], version=parallel_info['python_version'], status='disabled' if parallel_info['gil_disabled'] else 'enabled')

        # Pre-generate common frequency array to reduce allocations
        common_freq = FrequencyResponse.generate_frequencies(
            f_step=1.01, f_min=10, f_max=self.estimator.fs / 2
        )

        # Worker는 ir 객체를 사용하지 않으므로 task tuple에서 제외해 IPC pickle
        # 비용과 ImpulseResponse 객체 직렬화 부담을 제거한다.
        eq_tasks = []
        for speaker, pair in self.hrir.irs.items():
            for side in pair.keys():
                eq_tasks.append((speaker, side))

        self.logger.info("cli_info_parallel_eq", count=len(eq_tasks))
        eq_results = parallel_map(
            process_equalization_worker,
            eq_tasks,
            initializer=init_equalization_worker,
            initargs=(
                self.room_frs,
                self.hp_left,
                self.hp_right,
                self.eq_left,
                self.eq_right,
                self.target,
                common_freq,
                self.estimator.fs,
            ),
        )

        for speaker, side, fir in eq_results:
            self.hrir.irs[speaker][side].equalize(fir)
        check_cancelled()

    def _stage_decay(self):
        from core.cancellation import check_cancelled
        from core.parallel_utils import parallel_map
        from core.parallel_workers import process_decay_worker

        decay = self.config.decay
        self.logger.step("cli_adjusting_decay")

        decay_tasks = []
        for speaker, pair in self.hrir.irs.items():
            if speaker in decay:
                for side, ir in pair.items():
                    decay_tasks.append((speaker, side, ir.data, decay[speaker], self.estimator.fs))

        if decay_tasks:
            self.logger.info("cli_info_parallel_decay", count=len(decay_tasks))
            decay_results = parallel_map(process_decay_worker, decay_tasks)

            for speaker, side, adjusted_data in decay_results:
                self.hrir.irs[speaker][side].data = adjusted_data
        check_cancelled()

    def _stage_channel_balance(self):
        from core.cancellation import check_cancelled

        self.logger.step("cli_correcting_balance")
        self.hrir.correct_channel_balance(self.config.channel_balance)
        check_cancelled()

    def _stage_normalize(self):
        """Normalize gain after all processing, matching the original
        Impulcifer pipeline."""
        from core.cancellation import check_cancelled

        target_level = self.config.target_level
        self.logger.step("cli_normalizing_gain")
        self.applied_gain = self.hrir.normalize(
            peak_target=None if target_level is not None else -0.1, avg_target=target_level
        )
        check_cancelled()

    def _stage_write_readme(self):
        """Write info and stats in readme."""
        import os

        from core.cancellation import check_cancelled
        from core.pipeline_stages import write_readme

        readme_content = write_readme(
            os.path.join(self.dir_path, "README.md"),
            self.hrir,
            self.config.fs,
            self.estimator,
            self.applied_gain,
        )
        if readme_content:
            self.logger.info(readme_content)
        check_cancelled()

    def _stage_plot_post(self):
        import os

        from core.cancellation import check_cancelled

        self.logger.step("cli_plotting_post")

        # Compute convolutions for recording plots serially. Each convolution is
        # ~scipy.signal.convolve一행짜리 작업이라 ProcessPoolExecutor 14개 워커
        # (각 ~150MB) 비용이 직렬 실행보다 훨씬 비싸다. 추가로, 워커가 반환한
        # recording 배열을 plot_tasks/plot_results 튜플이 보유하면 hrir 메모리
        # 회수 시점까지 참조가 살아남아 ~3GB 잔류를 일으킨다.
        from scipy.signal import convolve
        for speaker, pair in self.hrir.irs.items():
            for side, ir in pair.items():
                ir.recording = convolve(self.estimator.test_signal, ir.data, mode="full")

        # Plot post processing
        self.hrir.plot(os.path.join(self.dir_path, "plots", "post"))
        check_cancelled()

    def _stage_plot_results(self):
        """Plot results, always."""
        import os

        from core.cancellation import check_cancelled

        self.logger.step("cli_plotting_results")
        self.hrir.plot_result(os.path.join(self.dir_path, "plots"))
        check_cancelled()

    def _stage_plot_additional(self):
        import os

        from core.cancellation import check_cancelled
        from core.pipeline_stages import _save_bokeh_analysis_plots

        self.logger.step("cli_plotting_additional")
        self.hrir.plot_interaural_impulse_overlay(
            os.path.join(self.dir_path, "plots", "interaural_overlay")
        )
        # ILD/IPD/IACC/ETC는 Bokeh 레이아웃만 존재하므로 HTML로 저장
        _save_bokeh_analysis_plots(self.hrir, self.dir_path, self.logger)
        check_cancelled()

    def _stage_interactive_plots(self):
        import os

        from bokeh.models import TabPanel, Tabs
        from bokeh.plotting import output_file as bokeh_output_file, save as bokeh_save
        from core.cancellation import check_cancelled

        self.logger.step("cli_generating_interactive")
        interactive_plot_dir = os.path.join(self.dir_path, "interactive_plots")
        os.makedirs(interactive_plot_dir, exist_ok=True)

        panels = []
        plot_functions_map = {
            "Interaural Overlay": self.hrir.generate_interaural_impulse_overlay_bokeh_layout,
            "ILD": self.hrir.generate_ild_bokeh_layout,
            "IPD": self.hrir.generate_ipd_bokeh_layout,
            "IACC": self.hrir.generate_iacc_bokeh_layout,
            "EDC": self.hrir.generate_etc_bokeh_layout,
            "Result Overview": self.hrir.generate_result_bokeh_figure,
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
                    self.logger.debug("cli_warning_plot_skipped", title=title)
            except Exception as e:
                self.logger.warning("cli_warning_interactive_plot_error", title=title, error=str(e))

        if panels:
            tabs = Tabs(tabs=panels, sizing_mode="stretch_both")
            output_html_path = os.path.join(
                interactive_plot_dir, "interactive_summary.html"
            )
            bokeh_output_file(
                output_html_path, title="Interactive Plot Summary"
            )
            bokeh_save(tabs)
            self.logger.success("cli_success_interactive_saved", path=output_html_path)
        else:
            self.logger.warning("cli_warning_no_interactive")
        check_cancelled()

    def _stage_resample(self):
        from core.cancellation import check_cancelled

        cfg = self.config
        if cfg.fs is None or cfg.fs == self.hrir.fs:
            return
        self.logger.step("cli_resampling", fs=cfg.fs)
        self.hrir.resample(cfg.fs)
        self.hrir.normalize(
            peak_target=None if cfg.target_level is not None else -0.1,
            avg_target=cfg.target_level,
        )
        check_cancelled()

    def _stage_write_brirs(self):
        import os

        from core.cancellation import check_cancelled
        from core.constants import HESUVI_TRACK_ORDER

        # Write multi-channel WAV file with standard track order
        self.logger.step("cli_writing_brirs")
        check_cancelled()
        self.hrir.write_wav(os.path.join(self.dir_path, "hrir.wav"))

        # Write multi-channel WAV file with HeSuVi track order
        check_cancelled()
        self.hrir.write_wav(os.path.join(self.dir_path, "hesuvi.wav"), track_order=HESUVI_TRACK_ORDER)

    def _stage_truehd_layouts(self):
        import os

        from core.cancellation import check_cancelled
        from core.constants import TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER

        self.logger.step("cli_generating_truehd")

        # 레이아웃별로 보유 채널을 세고, 최소 채널 수를 만족하면 좌/우 트랙
        # 순서를 만들어 기록한다. (구 core/channel_generation.py 인라인 —
        # validate가 만든 리스트를 버리고 같은 것을 다시 계산하던 3-함수
        # 시퀀스였다.)
        for layout_name, layout_order, min_channels, ok_key, fail_key in (
            ("11ch", TRUEHD_11CH_ORDER, 8, "cli_success_truehd_11ch", "cli_warning_truehd_11ch_fail"),  # 7.0.4
            ("13ch", TRUEHD_13CH_ORDER, 10, "cli_success_truehd_13ch", "cli_warning_truehd_13ch_fail"),  # 7.0.6
        ):
            available = [ch for ch in layout_order if ch in self.hrir.irs]
            if len(available) >= min_channels:
                track_order = [
                    f"{ch}-{side}" for ch in available for side in ("left", "right")
                ]
                output_path = os.path.join(
                    self.dir_path, f"truehd_{layout_name}_{len(available)}ch.wav"
                )
                self.hrir.write_wav(output_path, track_order=track_order)
                self.logger.success(ok_key, path=output_path)
            else:
                missing = [ch for ch in layout_order if ch not in self.hrir.irs]
                self.logger.warning(fail_key, msg=(
                    f"Insufficient channels: need {min_channels}, "
                    f"have {len(available)}. Missing: {missing}"
                ))
        check_cancelled()

    def _stage_jamesdsp(self):
        import contextlib
        import io
        import os

        from core.cancellation import check_cancelled

        cfg = self.config
        self.logger.step("cli_generating_jamesdsp")

        dsp_hrir = self.hrir.subset(["FL", "FR"], copy_irs=True)

        # normalize 내부의 print문 출력을 숨기기 위해 stdout 리디렉션
        with contextlib.redirect_stdout(io.StringIO()):
            dsp_hrir.normalize(
                peak_target=None if cfg.target_level is not None else -0.1,
                avg_target=cfg.target_level,
            )

        # FL-L, FL-R, FR-L, FR-R 순서로 파일 생성
        jd_order = ["FL-left", "FL-right", "FR-left", "FR-right"]
        out_path = os.path.join(self.dir_path, "jamesdsp.wav")
        dsp_hrir.write_wav(out_path, track_order=jd_order)
        del dsp_hrir
        self.logger.success("cli_success_jamesdsp", path=out_path)
        check_cancelled()

    def _stage_hangloose(self):
        import os

        from core.cancellation import check_cancelled
        from core.constants import SPEAKER_NAMES

        self.logger.step("cli_generating_hangloose")
        output_dir = os.path.join(self.dir_path, "Hangloose")
        os.makedirs(output_dir, exist_ok=True)

        processed_speakers = [sp for sp in SPEAKER_NAMES if sp in self.hrir.irs]

        for sp in processed_speakers:
            track_order = [f"{sp}-left", f"{sp}-right"]
            out_path = os.path.join(output_dir, f"{sp}.wav")
            self.hrir.write_wav(out_path, track_order=track_order)
            self.logger.info("cli_success_hangloose_file", file=f"{sp}.wav")

        self.logger.success("cli_success_hangloose", path=output_dir)
        check_cancelled()

    def _cleanup(self):
        """중간 산출물 참조 해제 + figure 정리 + GC.

        기존 run() 꼬리의 del 블록과 동일하게 성공 경로에서만 수행한다. 스테이지
        메서드의 지역 변수(eq_tasks 등)는 메서드 반환 시점에 이미 해제되므로,
        여기서는 루트 객체 참조만 끊으면 된다.
        """
        import gc

        import matplotlib.pyplot as plt

        self.hrir = None
        self.estimator = None
        self.room_frs = None
        self.hp_left = None
        self.hp_right = None
        self.eq_left = None
        self.eq_right = None
        self.target = None

        # safety net: 정상 경로에서는 hrir.plot()이 figure를 닫지만, 예외 발생
        # 시 닫히지 않은 figure가 남을 수 있어 전체 close를 한 번 더 호출한다.
        plt.close('all')

        gc.collect()
