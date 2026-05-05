# -*- coding: utf-8 -*-
"""Argparse builder driven by :class:`core.pipeline.ProcessingConfig` metadata.

Phase 3 of issue #87. The dataclass declared in ``core/pipeline.py`` carries
CLI metadata on every field — flag, help text, action, dest override,
``argparse.SUPPRESS`` opt-in, choices. This module reads that metadata and
adds the corresponding ``add_argument`` calls to a parser, so the CLI no
longer duplicates the parameter list as a wall of boilerplate.

Fields with ``cli_skip=True`` are not exposed (e.g. ``bass_boost_gain``,
``bass_boost_fc``, ``bass_boost_q`` are derived from a single ``--bass_boost``
argument by post-processing in the caller).
"""

from __future__ import annotations

import argparse
from dataclasses import fields
from typing import Iterable

from core.pipeline import ProcessingConfig


_TYPE_MAP = {"str": str, "int": int, "float": float}


def add_processing_config_arguments(
    parser: argparse.ArgumentParser,
    skip_fields: Iterable[str] = (),
) -> None:
    """Add one argparse argument per ``ProcessingConfig`` field with metadata.

    ``skip_fields`` lets the caller exclude specific fields when they prefer
    to register them manually (for example a field whose ``--flag`` is shared
    with another CLI-only argument).
    """
    skip_set = set(skip_fields)
    for f in fields(ProcessingConfig):
        meta = f.metadata
        if not meta or meta.get("cli_skip") or f.name in skip_set:
            continue

        flag = meta.get("cli_flag")
        if not flag:
            continue

        kwargs = {"help": meta.get("cli_help", "")}

        if "cli_dest" in meta:
            kwargs["dest"] = meta["cli_dest"]

        action = meta.get("cli_arg_action")
        if action:
            kwargs["action"] = action
        else:
            type_name = meta.get("cli_arg_type")
            if type_name:
                kwargs["type"] = _TYPE_MAP[type_name]

        if "cli_choices" in meta:
            kwargs["choices"] = list(meta["cli_choices"])

        if meta.get("cli_suppress_default"):
            kwargs["default"] = argparse.SUPPRESS
        elif action is None:
            # Mirror the dataclass default so explicit non-flag args keep their
            # previous default behavior.
            kwargs["default"] = f.default

        parser.add_argument(flag, **kwargs)
