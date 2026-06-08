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
