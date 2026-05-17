"""Shared BRIR argument assembly for Stable and Studio GUI skins."""

from __future__ import annotations

import os
import shutil
from typing import Any

from gui.utils import safe_get_double, safe_get_int, safe_get_string


def _copy_to_recording_dir(source_value: str, dir_path: str, target_name: str) -> None:
    source_value = source_value.strip()
    if not source_value or source_value == target_name:
        return

    source = source_value if os.path.isabs(source_value) else os.path.join(dir_path, source_value)
    target = os.path.join(dir_path, target_name)
    if os.path.abspath(source) == os.path.abspath(target):
        return
    if not os.path.exists(source):
        raise FileNotFoundError(source)
    os.makedirs(dir_path, exist_ok=True)
    shutil.copy2(source, target)


def sync_headphone_compensation_file(tab: Any) -> None:
    if not tab.do_headphone_compensation_var.get() or not tab.headphone_compensation_file_var.get():
        return
    _copy_to_recording_dir(
        tab.headphone_compensation_file_var.get(),
        tab.dir_path_var.get(),
        "headphones.wav",
    )


def sync_custom_eq_files(tab: Any) -> None:
    if not getattr(tab, "do_equalization_var").get():
        return
    for attr, target_name in (
        ("eq_file_var", "eq.csv"),
        ("eq_left_file_var", "eq-left.csv"),
        ("eq_right_file_var", "eq-right.csv"),
    ):
        var = getattr(tab, attr, None)
        if var is not None:
            _copy_to_recording_dir(var.get(), tab.dir_path_var.get(), target_name)


def build_brir_args(tab: Any, loc: Any) -> dict:
    """Build ``impulcifer.main`` kwargs from a GUI tab instance."""
    args = {
        "dir_path": tab.dir_path_var.get(),
        "test_signal": tab.test_signal_var.get(),
        "plot": tab.plot_var.get(),
        "do_room_correction": tab.do_room_correction_var.get(),
        "do_headphone_compensation": tab.do_headphone_compensation_var.get(),
        "do_equalization": tab.do_equalization_var.get(),
    }

    if tab.do_room_correction_var.get():
        args["room_target"] = tab.room_target_var.get() or None
        args["room_mic_calibration"] = tab.room_mic_calibration_var.get() or None
        args["specific_limit"] = safe_get_int(tab.specific_limit_var, 20000)
        args["generic_limit"] = safe_get_int(tab.generic_limit_var, 1000)
        args["fr_combination_method"] = tab.fr_combination_var.get()

    if tab.show_advanced_var.get():
        args["fs"] = safe_get_int(tab.fs_var, 48000) if tab.fs_check_var.get() else None

        target_level_str = safe_get_string(tab.target_level_var, "")
        if target_level_str.strip():
            try:
                args["target_level"] = float(target_level_str)
            except ValueError:
                args["target_level"] = None
        else:
            args["target_level"] = None

        if tab.channel_balance_var.get() == "number":
            args["channel_balance"] = safe_get_int(tab.channel_balance_db_var, 0)
        elif tab.channel_balance_var.get() != "none":
            args["channel_balance"] = tab.channel_balance_var.get()

        bass_gain = safe_get_double(tab.bass_boost_gain_var, 0.0)
        if bass_gain:
            args["bass_boost_gain"] = bass_gain
            args["bass_boost_fc"] = safe_get_int(tab.bass_boost_fc_var, 105)
            args["bass_boost_q"] = safe_get_double(tab.bass_boost_q_var, 0.76)

        tilt_val = safe_get_double(tab.tilt_var, 0.0)
        if tilt_val:
            args["tilt"] = tilt_val

        if tab.decay_per_channel_var.get():
            decay_dict = {}
            for ch, var in tab.decay_channel_vars.items():
                val_str = safe_get_string(var, "")
                if val_str.strip():
                    try:
                        decay_dict[ch] = float(val_str) / 1000
                    except ValueError:
                        pass
            if decay_dict:
                args["decay"] = decay_dict
        else:
            decay_str = safe_get_string(tab.decay_var, "")
            if decay_str.strip():
                try:
                    decay_val = float(decay_str) / 1000
                    args["decay"] = {
                        ch: decay_val for ch in ("FL", "FC", "FR", "SL", "SR", "BL", "BR")
                    }
                except ValueError:
                    pass

        args["head_ms"] = safe_get_double(tab.pre_response_var, 1.0)
        args["jamesdsp"] = tab.jamesdsp_var.get()
        args["hangloose"] = tab.hangloose_var.get()
        args["interactive_plots"] = tab.interactive_plots_var.get()
        args["microphone_deviation_correction"] = tab.microphone_deviation_correction_var.get()
        args["mic_deviation_strength"] = safe_get_double(tab.mic_deviation_strength_var, 0.7)
        args["mic_deviation_phase_correction"] = True
        args["mic_deviation_adaptive_correction"] = True
        args["mic_deviation_anatomical_validation"] = True
        args["mic_deviation_debug_plots"] = tab.mic_deviation_debug_plots_var.get()
        args["output_truehd_layouts"] = tab.output_truehd_layouts_var.get()

    if tab.vbass_enable_var.get():
        args["vbass"] = True
        args["vbass_freq"] = max(30, min(500, safe_get_int(tab.vbass_freq_var, 250)))
        args["vbass_hp"] = safe_get_double(tab.vbass_hp_var, 15.0)
        polarity_text = tab.vbass_polarity_var.get()
        if polarity_text == loc.get("vbass_polarity_normal") or polarity_text == "normal":
            args["vbass_polarity"] = "normal"
        elif polarity_text == loc.get("vbass_polarity_invert") or polarity_text == "invert":
            args["vbass_polarity"] = "invert"
        else:
            args["vbass_polarity"] = "auto"

    return args
