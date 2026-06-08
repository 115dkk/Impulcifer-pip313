"""Regression contracts raised by the biweekly audit issues.

These tests intentionally describe desired public behavior before the
production fixes land. They should drive the follow-up work for issues #113
and #115 without changing runtime code in this PR.
"""

from __future__ import annotations

import inspect
from dataclasses import fields
from typing import Any

import impulcifer
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
    """Every public ``main`` kwarg must survive the ``ProcessingConfig`` seam."""
    main_kwargs = {
        name
        for name, parameter in inspect.signature(impulcifer.main).parameters.items()
        if parameter.kind
        in (inspect.Parameter.POSITIONAL_OR_KEYWORD, inspect.Parameter.KEYWORD_ONLY)
    }
    config_fields = {field.name for field in fields(ProcessingConfig)}

    assert main_kwargs <= config_fields


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
