# -*- coding: utf-8 -*-
"""
Virtual bass synthesis for HRIR data.

The synthesis path intentionally follows LionLion123/Impulcifer: one shared
band-limited bass impulse is gain-matched globally, then added to each
speaker/ear pair after the original response is high-passed at the crossover.
This keeps sub-bass level stable across channels.
"""

from typing import Optional

import numpy as np
from scipy import signal
from scipy.signal import butter, sosfilt

from infra.logger import get_logger


_LEFT_SIDE_SPEAKERS = frozenset({
    "FL", "SL", "BL", "WL", "TFL", "TSL", "TBL",
})
_RIGHT_SIDE_SPEAKERS = frozenset({
    "FR", "SR", "BR", "WR", "TFR", "TSR", "TBR",
})
_CENTER_SPEAKERS = frozenset({
    "FC", "LFE",
})

_ILD_SHELF_TABLE = [
    (80, 50.0, 3.0),
    (160, 100.0, 4.5),
    (250, 150.0, 6.0),
]


def _classify_speaker(name: str) -> str:
    """Classify a speaker name into 'left', 'right', or 'center'."""
    name_upper = name.upper()
    if name_upper in _LEFT_SIDE_SPEAKERS:
        return "left"
    if name_upper in _RIGHT_SIDE_SPEAKERS:
        return "right"
    return "center"


def _detect_polarity(ir: np.ndarray) -> float:
    """Return +1.0 if the largest peak is positive, else -1.0."""
    abs_max_idx = np.argmax(np.abs(ir))
    return 1.0 if ir[abs_max_idx] >= 0 else -1.0


def _shift(ir: np.ndarray, n_samples: int) -> np.ndarray:
    """Circular-shift an impulse response by n_samples."""
    return np.roll(ir, n_samples)


def _delay_signal(sig: np.ndarray, delay: int, length: int) -> np.ndarray:
    """Delay or advance a signal with zero padding, never wraparound."""
    out = np.zeros(length, dtype=sig.dtype)
    if delay >= 0:
        if delay < length:
            available = min(length - delay, len(sig))
            out[delay: delay + available] = sig[:available]
    else:
        advance = -delay
        if advance < len(sig):
            available = min(length, len(sig) - advance)
            out[:available] = sig[advance: advance + available]
    return out


def _rfft_magnitude(ir: np.ndarray, fs: int):
    """Compute one-sided FFT magnitude spectrum."""
    h = np.fft.rfft(ir)
    freqs = np.fft.rfftfreq(len(ir), 1.0 / fs)
    return np.abs(h), freqs


def _mag_at(ir: np.ndarray, fs: int, freq_hz: float) -> float:
    """Return linear magnitude of ir at freq_hz."""
    magnitude, freqs = _rfft_magnitude(ir, fs)
    idx = np.argmin(np.abs(freqs - freq_hz))
    return float(magnitude[idx])


def _duplicate_sos(sos: np.ndarray, times: int) -> np.ndarray:
    """Duplicate an SOS filter chain."""
    return np.vstack([sos for _ in range(times)])


def _rbj_high_shelf(fc: float, fs: int, gain_db: float, q: float) -> np.ndarray:
    """Design a single RBJ high-shelf biquad and return it as SOS."""
    a = 10 ** (gain_db / 40.0)
    w0 = 2 * np.pi * fc / fs
    alpha = np.sin(w0) / (2 * q)
    cos_w0 = np.cos(w0)

    b0 = a * ((a + 1) + (a - 1) * cos_w0 + 2 * np.sqrt(a) * alpha)
    b1 = -2 * a * ((a - 1) + (a + 1) * cos_w0)
    b2 = a * ((a + 1) + (a - 1) * cos_w0 - 2 * np.sqrt(a) * alpha)
    a0 = (a + 1) - (a - 1) * cos_w0 + 2 * np.sqrt(a) * alpha
    a1 = 2 * ((a - 1) - (a + 1) * cos_w0)
    a2 = (a + 1) - (a - 1) * cos_w0 - 2 * np.sqrt(a) * alpha

    return signal.tf2sos([b0, b1, b2], [a0, a1, a2])


def _build_ild_shelf(xo_hz: float, fs: int):
    """Build the legacy low-shelf descriptor kept for API/tests."""
    best = None
    for threshold, shelf_freq, shelf_gain_db in _ILD_SHELF_TABLE:
        if xo_hz >= threshold:
            best = (shelf_freq, shelf_gain_db)
    if best is None:
        return None
    shelf_freq, shelf_gain_db = best
    gain_linear = 10 ** (shelf_gain_db / 20.0)
    sos_lp = butter(2, shelf_freq, btype="low", fs=fs, output="sos")
    return sos_lp, gain_linear, shelf_gain_db


def _apply_ild_shelf(ir: np.ndarray, shelf_info, side: str, speaker_class: str) -> np.ndarray:
    """Legacy low-shelf helper kept for compatibility with older callers."""
    if shelf_info is None or speaker_class == "center":
        return ir

    sos_lp, gain_linear, _ = shelf_info
    lf_component = sosfilt(sos_lp, ir)
    if speaker_class == side:
        boost = (gain_linear - 1.0) * 0.5
        return ir + boost * lf_component

    cut = (1.0 - 1.0 / gain_linear) * 0.5
    return ir - cut * lf_component


def synthesize_virtual_bass(
    irs: dict,
    fs: int,
    crossover_freq: int = 250,
    head_ms: float = 1.0,
    hp_freq: float = 15.0,
    invert_polarity: Optional[bool] = None,
) -> None:
    """Apply Lion-style virtual bass synthesis in-place."""
    logger = get_logger()

    if crossover_freq >= fs / 2:
        logger.error("vbass_error_sr_limit")
        return
    if crossover_freq > 300:
        logger.warning("vbass_warning_high_crossover", freq=crossover_freq)

    n_ir = max(len(ir.data) for pair in irs.values() for ir in pair.values())
    for pair in irs.values():
        for side in ("left", "right"):
            if side in pair and len(pair[side].data) < n_ir:
                pair[side].data = np.pad(pair[side].data, (0, n_ir - len(pair[side].data)))

    imp = np.zeros(n_ir)
    imp[0] = 1.0

    sos_hp4_sub = signal.butter(4, hp_freq / (fs / 2), btype="high", output="sos")
    mpbass_hp_only = signal.sosfilt(sos_hp4_sub, imp)

    sos_lp4_xo = signal.butter(4, crossover_freq / (fs / 2), btype="low", output="sos")
    sos_lp8_xo = _duplicate_sos(sos_lp4_xo, 2)
    mpbass = signal.sosfilt(sos_lp8_xo, mpbass_hp_only)

    shelves = [
        (150.0, -1.5, 0.760),
        (400.0, -3.0, 0.660),
        (800.0, -3.5, 0.610),
    ]
    sos_ild = np.vstack([_rbj_high_shelf(fc, fs, gain, q) for fc, gain, q in shelves])

    sos_hp4_xo = signal.butter(4, crossover_freq / (fs / 2), btype="high", output="sos")
    sos_hp8_xo = _duplicate_sos(sos_hp4_xo, 2)

    all_xo_mags = []
    for pair in irs.values():
        if "left" not in pair or "right" not in pair:
            continue
        hi_left = signal.sosfilt(sos_hp8_xo, pair["left"].data)
        hi_right = signal.sosfilt(sos_hp8_xo, pair["right"].data)
        all_xo_mags.append(_mag_at(hi_left, fs, crossover_freq))
        all_xo_mags.append(_mag_at(hi_right, fs, crossover_freq))

    if not all_xo_mags:
        return

    mean_xo_mag = float(np.mean(all_xo_mags))
    mpbass_mag = _mag_at(mpbass, fs, crossover_freq) + 1e-20
    global_gain = mean_xo_mag / mpbass_mag

    head_samples = int(round(head_ms * 1e-3 * fs))
    polarity = -1.0 if invert_polarity else 1.0

    for speaker, pair in irs.items():
        if "left" not in pair or "right" not in pair:
            continue

        speaker_on_left = speaker.upper().endswith("L")
        left_peak = pair["left"].peak_index()
        right_peak = pair["right"].peak_index()
        if not isinstance(left_peak, (int, np.integer, float, np.floating)):
            left_peak = None
        if not isinstance(right_peak, (int, np.integer, float, np.floating)):
            right_peak = None
        if left_peak is None:
            left_peak = int(np.argmax(np.abs(pair["left"].data)))
        if right_peak is None:
            right_peak = int(np.argmax(np.abs(pair["right"].data)))
        left_peak = int(left_peak)
        right_peak = int(right_peak)
        itd_samples = right_peak - left_peak

        synth_direct_undelayed = mpbass * global_gain * polarity
        synth_cross_undelayed = signal.sosfilt(sos_ild, synth_direct_undelayed)

        direct_delay = head_samples
        cross_delay = head_samples + (itd_samples if speaker_on_left else -itd_samples)
        synth_direct = _delay_signal(synth_direct_undelayed, direct_delay, n_ir)
        synth_cross = _delay_signal(synth_cross_undelayed, cross_delay, n_ir)

        hi_left = signal.sosfilt(sos_hp8_xo, pair["left"].data)
        hi_right = signal.sosfilt(sos_hp8_xo, pair["right"].data)

        pair["left"].data = hi_left + (synth_direct if speaker_on_left else synth_cross)
        pair["right"].data = hi_right + (synth_cross if speaker_on_left else synth_direct)


def apply_virtual_bass_to_hrir(
    hrir,
    crossover_freq: int = 250,
    head_ms: float = 1.0,
    hp_freq: float = 15.0,
    invert_polarity: Optional[bool] = None,
) -> None:
    """Apply virtual bass synthesis to all channels of an HRIR object."""
    logger = get_logger()
    logger.info("vbass_status_processing")

    synthesize_virtual_bass(
        irs=hrir.irs,
        fs=hrir.fs,
        crossover_freq=crossover_freq,
        head_ms=head_ms,
        hp_freq=hp_freq,
        invert_polarity=invert_polarity,
    )

    logger.success("vbass_status_complete")
