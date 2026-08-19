"""Canonical registry for HRIR Bokeh analysis generators."""

from dataclasses import dataclass


@dataclass(frozen=True)
class BokehAnalysisGenerator:
    """Metadata for one Bokeh layout generator on ``HRIR``."""

    name: str
    title: str
    method_name: str
    save_individually: bool = False


BOKEH_ANALYSIS_GENERATORS = (
    BokehAnalysisGenerator(
        "interaural_overlay",
        "Interaural Overlay",
        "generate_interaural_impulse_overlay_bokeh_layout",
    ),
    BokehAnalysisGenerator(
        "ild", "ILD", "generate_ild_bokeh_layout", save_individually=True
    ),
    BokehAnalysisGenerator(
        "ipd", "IPD", "generate_ipd_bokeh_layout", save_individually=True
    ),
    BokehAnalysisGenerator(
        "iacc", "IACC", "generate_iacc_bokeh_layout", save_individually=True
    ),
    BokehAnalysisGenerator(
        "etc", "EDC", "generate_etc_bokeh_layout", save_individually=True
    ),
    BokehAnalysisGenerator(
        "result_overview", "Result Overview", "generate_result_bokeh_figure"
    ),
)
