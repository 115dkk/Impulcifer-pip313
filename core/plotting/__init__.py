# -*- coding: utf-8 -*-
"""Plotting subsystem for HRIR / ImpulseResponse data classes.

The mixin classes here hold all matplotlib / Bokeh visualization logic so the
data classes in :mod:`core.hrir` and :mod:`core.impulse_response` stay focused
on numerical processing. This is Phase 1 of the issue #87 refactoring plan.

Usage:
    from core.plotting import HRIRPlotter, ImpulseResponsePlotter

The data classes inherit from these mixins, preserving the existing public API
(``hrir.plot(...)``, ``ir.plot_fr(...)``, etc.) while keeping the source files
themselves under the size budget.
"""

from core.plotting.hrir_plotter import HRIRPlotter
from core.plotting.impulse_response_plotter import ImpulseResponsePlotter

__all__ = ["HRIRPlotter", "ImpulseResponsePlotter"]
