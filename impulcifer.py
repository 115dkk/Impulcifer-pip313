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
import argparse
import matplotlib.font_manager as fm
from core.utils import set_matplotlib_font
from core.constants import SPEAKER_NAMES

# Cancellation now lives in core.cancellation; these re-exports keep the
# historical import surface (GUI tabs, application service, tests) working.
from core.cancellation import (  # noqa: F401
    CancelledError,
    cancellation_scope,
    check_cancelled as _check_cancelled,
)

# Stage helpers now live in core.pipeline_stages (audit #138 C1); re-exported
# for legacy callers (tests import equalization from here, etc.).
from core.pipeline_stages import (  # noqa: F401
    _find_eq_settings_file,
    _read_eq_settings,
    _save_bokeh_analysis_plots,
    create_target,
    equalization,
    headphone_compensation,
    open_binaural_measurements,
    open_impulse_response_estimator,
    write_readme,
)


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
    # Local import keeps `import impulcifer` cheap (version/CLI queries) —
    # the heavy DSP chain only loads when a BRIR run actually starts.
    from core.pipeline import ProcessingConfig, BRIRPipeline

    BRIRPipeline(ProcessingConfig.from_kwargs(**kwargs)).run()


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
