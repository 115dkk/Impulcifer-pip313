# -*- coding: utf-8 -*-
"""
Virtual Bass Synthesis for HRIR
Replaces low-frequency content with synthesized minimum-phase bass
for improved sub-bass response in binaural impulse responses.
"""

import numpy as np
from scipy.signal import butter, sosfilt, minimum_phase
from typing import Optional

from infra.logger import get_logger

# ---------------------------------------------------------------------------
# Speaker classification constants (DO NOT MODIFY)
# ---------------------------------------------------------------------------
_LEFT_SIDE_SPEAKERS = frozenset({
    'FL', 'SL', 'BL', 'WL', 'TFL', 'TSL', 'TBL',
})
_RIGHT_SIDE_SPEAKERS = frozenset({
    'FR', 'SR', 'BR', 'WR', 'TFR', 'TSR', 'TBR',
})
_CENTER_SPEAKERS = frozenset({
    'FC', 'LFE',
})

# ---------------------------------------------------------------------------
# ILD shelf parameters (DO NOT MODIFY)
#   Each entry: (min_crossover_hz, shelf_freq, shelf_gain_dB)
#   The highest-threshold entry where xo_hz >= min_crossover_hz is used.
# ---------------------------------------------------------------------------
_ILD_SHELF_TABLE = [
    (80,  50.0, 3.0),
    (160, 100.0, 4.5),
    (250, 150.0, 6.0),
]


def _classify_speaker(name: str) -> str:
    """Classify a speaker name into 'left', 'right', or 'center'.

    Args:
        name: Speaker name, e.g. 'FL', 'FR', 'FC', 'TFL'.

    Returns:
        'left', 'right', or 'center'.
    """
    name_upper = name.upper()
    if name_upper in _LEFT_SIDE_SPEAKERS:
        return 'left'
    elif name_upper in _RIGHT_SIDE_SPEAKERS:
        return 'right'
    else:
        return 'center'


def _detect_polarity(ir: np.ndarray) -> float:
    """Detect the polarity of an impulse response.

    Looks at the largest peak (positive or negative) and returns
    +1.0 if the peak is positive, -1.0 if negative.

    Args:
        ir: 1-D impulse response array.

    Returns:
        1.0 or -1.0
    """
    abs_max_idx = np.argmax(np.abs(ir))
    return 1.0 if ir[abs_max_idx] >= 0 else -1.0


def _shift(ir: np.ndarray, n_samples: int) -> np.ndarray:
    """Circular-shift an impulse response by *n_samples* (positive = right).

    Uses numpy roll — no Python loops.

    Args:
        ir: 1-D array.
        n_samples: Number of samples to shift.

    Returns:
        Shifted copy.
    """
    return np.roll(ir, n_samples)


def _rfft_magnitude(ir: np.ndarray, fs: int):
    """Compute one-sided FFT magnitude spectrum.

    Args:
        ir: 1-D impulse response.
        fs: Sample rate.

    Returns:
        (magnitude, freqs) tuple.
    """
    H = np.fft.rfft(ir)
    freqs = np.fft.rfftfreq(len(ir), 1.0 / fs)
    magnitude = np.abs(H)
    return magnitude, freqs


def _mag_at(ir: np.ndarray, fs: int, freq_hz: float) -> float:
    """Return the linear magnitude of *ir* at the given frequency.

    Args:
        ir: 1-D impulse response.
        fs: Sample rate.
        freq_hz: Target frequency in Hz.

    Returns:
        Linear magnitude (not dB).
    """
    magnitude, freqs = _rfft_magnitude(ir, fs)
    idx = np.argmin(np.abs(freqs - freq_hz))
    return float(magnitude[idx])


def _build_ild_shelf(xo_hz: float, fs: int):
    """Build an ILD low-shelf SOS filter based on the crossover frequency.

    The shelf is only generated if the crossover is high enough to warrant
    an inter-aural level difference correction.  Returns *None* if no
    shelf is needed (crossover too low).

    Args:
        xo_hz: Crossover frequency in Hz.
        fs: Sample rate.

    Returns:
        SOS array for a low-shelf filter, or None.
    """
    best = None
    for threshold, shelf_freq, shelf_gain_db in _ILD_SHELF_TABLE:
        if xo_hz >= threshold:
            best = (shelf_freq, shelf_gain_db)
    if best is None:
        return None
    shelf_freq, shelf_gain_db = best
    gain_linear = 10 ** (shelf_gain_db / 20.0)
    sos_lp = butter(2, shelf_freq, btype='low', fs=fs, output='sos')
    return sos_lp, gain_linear, shelf_gain_db


def _apply_ild_shelf(ir: np.ndarray, shelf_info, side: str, speaker_class: str) -> np.ndarray:
    """Apply ILD shelf to an IR based on ear side and speaker position.

    For a LEFT speaker:
      - ipsilateral ear (left) gets a small boost
      - contralateral ear (right) gets a small cut
    For a RIGHT speaker: opposite.
    For CENTER: no ILD applied.

    Args:
        ir: 1-D impulse response.
        shelf_info: Tuple from _build_ild_shelf or None.
        side: 'left' or 'right' ear.
        speaker_class: 'left', 'right', or 'center'.

    Returns:
        Modified IR.
    """
    if shelf_info is None or speaker_class == 'center':
        return ir

    sos_lp, gain_linear, shelf_gain_db = shelf_info
    # Extract the low-frequency component
    lf_component = sosfilt(sos_lp, ir)

    # Determine if this is ipsilateral or contralateral
    if speaker_class == side:
        # Ipsilateral: boost low frequencies
        boost = (gain_linear - 1.0) * 0.5
        return ir + boost * lf_component
    else:
        # Contralateral: cut low frequencies
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
    """Core virtual bass synthesis engine.

    For each speaker-side pair, replaces the content below *crossover_freq*
    with a synthesized minimum-phase bass signal, gain-matched at the
    crossover point.

    The function modifies *irs* in-place.

    Args:
        irs: Dict of dicts, ``{speaker: {'left': IR, 'right': IR}}``.
             Each IR object must have a ``.data`` attribute (numpy 1-D array).
        fs: Sample rate in Hz.
        crossover_freq: Frequency below which bass is replaced (Hz).
        head_ms: Pre-response head room in ms (used for alignment).
        hp_freq: Sub-bass high-pass frequency to remove DC rumble (Hz).
        invert_polarity: True = always invert, False = never invert,
                         None = auto-detect per channel.
    """
    logger = get_logger()

    # Validate crossover against Nyquist
    if crossover_freq >= fs / 2:
        logger.error("vbass_error_sr_limit")
        return

    # Warn if crossover is unusually high
    if crossover_freq > 300:
        logger.warning("vbass_warning_high_crossover",
                       freq=crossover_freq)

    # Pre-build SOS filter chains (same for all speakers — hoisted out of loop)
    # 8th-order Butterworth crossover filters
    sos_lp8_xo = butter(8, crossover_freq, btype='low', fs=fs, output='sos')
    sos_hp8_xo = butter(8, crossover_freq, btype='high', fs=fs, output='sos')
    # Sub-bass high-pass to remove DC rumble
    sos_hp_sub = butter(4, hp_freq, btype='high', fs=fs, output='sos')

    # ILD shelf (same for all speakers)
    shelf_info = _build_ild_shelf(crossover_freq, fs)

    # Pre-compute frequency index for crossover magnitude matching
    # Use a dummy array to get frequency axis length
    dummy_len = None
    freq_idx = None

    for speaker, pair in irs.items():
        speaker_class = _classify_speaker(speaker)

        for side in ('left', 'right'):
            if side not in pair:
                continue

            ir_obj = pair[side]
            ir = ir_obj.data.copy()

            # Lazily compute freq_idx from the first IR we see
            if freq_idx is None:
                dummy_len = len(ir)
                freqs = np.fft.rfftfreq(dummy_len, 1.0 / fs)
                freq_idx = np.argmin(np.abs(freqs - crossover_freq))

            # --- Step 1: High-pass the original IR at crossover ---
            ir_hp = sosfilt(sos_hp8_xo, ir)

            # --- Step 2: Extract the low-frequency envelope ---
            ir_lp = sosfilt(sos_lp8_xo, ir)

            # --- Step 3: Detect / apply polarity ---
            if invert_polarity is None:
                polarity = _detect_polarity(ir_lp)
            elif invert_polarity:
                polarity = -1.0
            else:
                polarity = 1.0

            ir_lp_corrected = ir_lp * polarity

            # --- Step 4: Create minimum-phase version of the LP component ---
            # Ensure even length for minimum_phase
            lp_len = len(ir_lp_corrected)
            if lp_len % 2 != 0:
                ir_lp_padded = np.append(ir_lp_corrected, 0.0)
            else:
                ir_lp_padded = ir_lp_corrected

            try:
                ir_bass = minimum_phase(np.abs(np.fft.rfft(ir_lp_padded)),
                                        method='homomorphic',
                                        n_fft=len(ir_lp_padded))
                # Trim or pad to original length
                if len(ir_bass) < lp_len:
                    ir_bass = np.pad(ir_bass, (0, lp_len - len(ir_bass)))
                else:
                    ir_bass = ir_bass[:lp_len]
            except Exception:
                # Fallback: use the corrected LP directly
                ir_bass = np.abs(ir_lp_corrected)

            # --- Step 5: Apply sub-bass high-pass ---
            ir_bass = sosfilt(sos_hp_sub, ir_bass)

            # --- Step 6: Gain matching at crossover frequency ---
            # Use cached FFT approach
            H_hp = np.fft.rfft(ir_hp)
            H_bass = np.fft.rfft(ir_bass, n=len(ir_hp))

            mag_hp_at_xo = np.abs(H_hp[freq_idx]) if freq_idx < len(H_hp) else 1.0
            mag_bass_at_xo = np.abs(H_bass[freq_idx]) if freq_idx < len(H_bass) else 1.0

            if mag_bass_at_xo > 1e-10:
                gain = mag_hp_at_xo / mag_bass_at_xo
            else:
                gain = 1.0

            ir_bass *= gain

            # --- Step 7: Restore polarity ---
            ir_bass *= polarity

            # --- Step 8: Apply ILD shelf ---
            ir_bass = _apply_ild_shelf(ir_bass, shelf_info, side, speaker_class)

            # --- Step 9: Align bass onset with head_ms ---
            head_samples = int(head_ms * fs / 1000.0)
            # Find the peak of the bass signal
            bass_peak_idx = np.argmax(np.abs(ir_bass))
            if bass_peak_idx > head_samples:
                shift_amount = head_samples - bass_peak_idx
                ir_bass = _shift(ir_bass, shift_amount)

            # --- Step 10: Combine high-passed original + synthesized bass ---
            combined = ir_hp + ir_bass[:len(ir_hp)]

            # Write back
            ir_obj.data = combined


def apply_virtual_bass_to_hrir(
    hrir,
    crossover_freq: int = 250,
    head_ms: float = 1.0,
    hp_freq: float = 15.0,
    invert_polarity: Optional[bool] = None,
) -> None:
    """Apply virtual bass synthesis to all channels of an HRIR object.

    This is the public API.  It delegates to :func:`synthesize_virtual_bass`
    after extracting ``hrir.irs`` and ``hrir.fs``.

    Args:
        hrir: HRIR object with ``.irs`` (dict) and ``.fs`` (int) attributes.
        crossover_freq: Crossover frequency in Hz (default 250).
        head_ms: Pre-response head room in ms (default 1.0).
        hp_freq: Sub-bass high-pass frequency in Hz (default 15.0).
        invert_polarity: None = auto, False = normal, True = invert.
    """
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
