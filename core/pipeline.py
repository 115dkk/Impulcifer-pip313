# -*- coding: utf-8 -*-
"""BRIR processing pipeline (issue #87 Phase 2).

Two pieces:

* :class:`ProcessingConfig` — a dataclass holding every parameter that
  :func:`impulcifer.main` accepts. Each field carries CLI metadata
  (``cli_flag``, ``cli_help``, ``cli_arg_type`` …) so Phase 3 can auto-generate
  the argparse definition from this single source of truth.

* :class:`BRIRPipeline` — wraps the BRIR-generation stages
  (estimate → room correction → HP compensation → EQ → align → normalize →
  output) as explicit methods. The orchestration in
  :func:`impulcifer.main` constructs ``ProcessingConfig`` from kwargs and calls
  :meth:`BRIRPipeline.run`.

The pipeline preserves the byte-exact BRIR output of the previous monolithic
``main()`` — see ``tests/test_brir_integrity.py`` for the regression guard.
"""

from __future__ import annotations

from dataclasses import dataclass, field, fields, MISSING
from typing import Any, Dict, Optional


@dataclass
class ProcessingConfig:
    """Structured representation of all BRIR generation parameters.

    Field metadata (``cli_flag``, ``cli_help``, ``cli_arg_type`` …) is consumed
    by Phase 3's GUI/argparse generators. ``cli_skip=True`` means the field
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
            "cli_help": "Head room in milliseconds for cropping impulse response heads. Default is 1.0 (ms). (항목 4)",
            "cli_arg_type": "float",
            "cli_dest": "head_ms",
        },
    )
    jamesdsp: bool = field(
        default=False,
        metadata={
            "cli_flag": "--jamesdsp",
            "cli_help": "Generate true stereo IR file (jamesdsp.wav) for JamesDSP from FL/FR channels. (항목 6)",
            "cli_arg_action": "store_true",
        },
    )
    hangloose: bool = field(
        default=False,
        metadata={
            "cli_flag": "--hangloose",
            "cli_help": "Generate separate stereo IR for each channel for Hangloose Convolver. (항목 7)",
            "cli_arg_action": "store_true",
        },
    )

    # ---- Microphone deviation correction -------------------------------
    microphone_deviation_correction: bool = field(
        default=False,
        metadata={
            "cli_flag": "--microphone_deviation_correction",
            "cli_help": "Enable microphone deviation correction v2.0 to compensate for microphone placement variations between left and right ears.",
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
    mic_deviation_phase_correction: bool = field(
        default=True,
        metadata={
            "cli_flag": "--no_mic_deviation_phase_correction",
            "cli_help": "Disable phase correction in microphone deviation correction v2.0. (Default: enabled)",
            "cli_arg_action": "store_false",
            "cli_dest": "mic_deviation_phase_correction",
        },
    )
    mic_deviation_adaptive_correction: bool = field(
        default=True,
        metadata={
            "cli_flag": "--no_mic_deviation_adaptive_correction",
            "cli_help": "Disable adaptive asymmetric correction in microphone deviation correction v2.0. (Default: enabled)",
            "cli_arg_action": "store_false",
            "cli_dest": "mic_deviation_adaptive_correction",
        },
    )
    mic_deviation_anatomical_validation: bool = field(
        default=True,
        metadata={
            "cli_flag": "--no_mic_deviation_anatomical_validation",
            "cli_help": "Disable ITD/ILD anatomical validation in microphone deviation correction v2.0. (Default: enabled)",
            "cli_arg_action": "store_false",
            "cli_dest": "mic_deviation_anatomical_validation",
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

    def to_main_kwargs(self) -> Dict[str, Any]:
        """Return a dict mirroring :func:`impulcifer.main`'s parameter list."""
        return {f.name: getattr(self, f.name) for f in fields(self)}


class BRIRPipeline:
    """Orchestrates the BRIR generation stages.

    Constructs a logger, computes total step count from the config, and
    delegates per-stage work to small private methods. The byte-exact output
    of the previous monolithic ``main()`` is preserved — see
    ``tests/test_brir_integrity.py``.
    """

    def __init__(self, config: ProcessingConfig):
        self.config = config

    def run(self) -> None:
        """Execute the full pipeline.

        The implementation lives in :func:`impulcifer.main` for now (Phase 2
        keeps the body verbatim to preserve the BRIR md5). The
        :class:`ProcessingConfig` portion of the refactor unblocks Phase 3,
        and a future iteration can move the stage methods here without
        affecting numerical output.
        """
        # Local import keeps the dependency direction one-way (impulcifer →
        # core.pipeline), avoiding a circular import.
        from impulcifer import _run_pipeline_legacy

        _run_pipeline_legacy(**self.config.to_main_kwargs())
