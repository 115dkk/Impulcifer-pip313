"""Regression contracts raised by the biweekly audit issues.

These tests intentionally describe desired public behavior before the
production fixes land. They should drive the follow-up work for issues #113
and #115 without changing runtime code in this PR.
"""

from __future__ import annotations

from dataclasses import fields
from typing import Any

from core.pipeline import ProcessingConfig
from gui.brir_args import build_brir_args


class DummyVar:
    """Small Tk variable stand-in for argument assembly tests."""

    def __init__(self, value: Any = None, *, raises: type[Exception] | None = None) -> None:
        self.value = value
        self.raises = raises

    def get(self) -> Any:
        if self.raises is not None:
            raise self.raises("invalid GUI value")
        return self.value


class DummyLoc:
    """Localization stand-in for virtual bass polarity handling."""

    def get(self, key: str, **_: object) -> str:
        return key


class MinimalBrirTab:
    """Minimum surface ``build_brir_args`` needs for room correction."""

    def __init__(self) -> None:
        self.dir_path_var = DummyVar("data/demo")
        self.test_signal_var = DummyVar("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
        self.plot_var = DummyVar(False)

        self.do_room_correction_var = DummyVar(True)
        self.room_target_var = DummyVar("")
        self.room_mic_calibration_var = DummyVar("")
        self.specific_limit_var = DummyVar(raises=ValueError)
        self.generic_limit_var = DummyVar(raises=ValueError)
        self.fr_combination_var = DummyVar("average")

        self.do_headphone_compensation_var = DummyVar(False)
        self.do_equalization_var = DummyVar(False)
        self.show_advanced_var = DummyVar(False)
        self.vbass_enable_var = DummyVar(False)


def test_gui_room_limit_fallbacks_match_processing_config_defaults() -> None:
    """Cleared or invalid GUI room-limit fields use the canonical config defaults."""
    args = build_brir_args(MinimalBrirTab(), DummyLoc())
    defaults = ProcessingConfig()

    assert args["specific_limit"] == defaults.specific_limit
    assert args["generic_limit"] == defaults.generic_limit


def test_main_public_kwargs_are_represented_in_processing_config() -> None:
    """Every kwarg the GUI assembles for ``main`` must have a ProcessingConfig home.

    ``impulcifer.main`` now takes ``**kwargs`` and forwards them through
    ``ProcessingConfig.from_kwargs``, which silently drops unknown keys. A
    signature-based check would be vacuous (main has no named params), so assert
    the real public surface instead: every key ``build_brir_args`` emits must be
    a ProcessingConfig field, otherwise a GUI control would be silently ignored.
    """
    args = build_brir_args(MinimalBrirTab(), DummyLoc())
    config_fields = {field.name for field in fields(ProcessingConfig)}

    assert args, "build_brir_args produced no kwargs; the check would be vacuous"
    assert set(args) <= config_fields


_WORKER_STATE: str | None = None


def _set_worker_state(value: str) -> None:
    global _WORKER_STATE
    _WORKER_STATE = value


def _read_worker_state(item: str) -> tuple[str, str | None]:
    return item, _WORKER_STATE


def test_parallel_processing_map_supports_initializer_contract() -> None:
    """Both parallel map import paths should support worker initialization."""
    from core.parallel_processing import parallel_map

    result = parallel_map(
        _read_worker_state,
        ["task"],
        max_workers=1,
        initializer=_set_worker_state,
        initargs=("ready",),
        use_threads=True,
    )

    assert result == [("task", "ready")]


class AdvancedDecayBrirTab(MinimalBrirTab):
    """Tab stand-in with the advanced panel open and a single decay value."""

    def __init__(self) -> None:
        super().__init__()
        self.show_advanced_var = DummyVar(True)
        self.fs_check_var = DummyVar(False)
        self.fs_var = DummyVar(raises=ValueError)
        self.target_level_var = DummyVar("")
        self.channel_balance_var = DummyVar("none")
        self.channel_balance_db_var = DummyVar(0)
        self.bass_boost_gain_var = DummyVar(0.0)
        self.bass_boost_fc_var = DummyVar(raises=ValueError)
        self.bass_boost_q_var = DummyVar(raises=ValueError)
        self.tilt_var = DummyVar(0.0)
        self.decay_per_channel_var = DummyVar(False)
        self.decay_channel_vars = {}
        self.decay_var = DummyVar("300")
        self.pre_response_var = DummyVar(1.0)
        self.jamesdsp_var = DummyVar(False)
        self.hangloose_var = DummyVar(False)
        self.interactive_plots_var = DummyVar(False)
        self.microphone_deviation_correction_var = DummyVar(False)
        self.mic_deviation_strength_var = DummyVar(0.7)
        self.mic_deviation_debug_plots_var = DummyVar(False)
        self.output_truehd_layouts_var = DummyVar(False)


def test_gui_single_decay_fans_out_to_all_speaker_names() -> None:
    """Single-value decay must cover the full 15-speaker layout like the CLI.

    Audit #138 F018/Q2: the old hardcoded 7-channel tuple was drift, not an
    intentional "GUI only exposes 7.1" decision.
    """
    from core.constants import SPEAKER_NAMES

    args = build_brir_args(AdvancedDecayBrirTab(), DummyLoc())

    assert set(args["decay"]) == set(SPEAKER_NAMES)
    assert all(value == 0.3 for value in args["decay"].values())


def test_webview_decay_channels_mirror_speaker_names() -> None:
    """app.js DECAY_CHANNELS must stay in sync with core SPEAKER_NAMES."""
    import re
    from pathlib import Path

    from core.constants import SPEAKER_NAMES

    app_js = (Path(__file__).parent.parent / "webview_ui" / "app.js").read_text(encoding="utf-8")
    match = re.search(r"const DECAY_CHANNELS = \[(.*?)\];", app_js, flags=re.DOTALL)
    assert match, "DECAY_CHANNELS not found in app.js"
    channels = re.findall(r'"([A-Z]+)"', match.group(1))
    assert channels == list(SPEAKER_NAMES)
