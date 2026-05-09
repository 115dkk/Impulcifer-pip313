import os
import sys
import __main__

import numpy as np

lion_dir = r"E:\Impulcifer\_verification\lion"
sys.path.insert(0, lion_dir)

from impulse_response_estimator import ImpulseResponseEstimator

__main__.ImpulseResponseEstimator = ImpulseResponseEstimator

import hrir

_orig_write_wav = hrir.HRIR.write_wav


def _write_wav_skip_room_responses(self, file_path, *args, **kwargs):
    try:
        return _orig_write_wav(self, file_path, *args, **kwargs)
    except RuntimeError:
        if os.path.basename(file_path).lower() == "room-responses.wav":
            return None
        raise


hrir.HRIR.write_wav = _write_wav_skip_room_responses

from autoeq.frequency_response import FrequencyResponse
from impulcifer import (
    create_target,
    equalization,
    headphone_compensation,
    open_impulse_response_estimator,
)
from room_correction import room_correction


dir_path = r"E:\Impulcifer\_verification\lion_control_250"
test_path = r"E:\Impulcifer\_verification\jaakko\data\sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"
out_path = r"E:\Impulcifer\_verification\lion_eq_dump.npz"

estimator = open_impulse_response_estimator(dir_path, file_path=test_path)
_, room_frs = room_correction(
    estimator,
    dir_path,
    specific_limit=400,
    generic_limit=300,
    plot=False,
)
hp_left, hp_right = headphone_compensation(estimator, dir_path)
eq_left, eq_right = equalization(estimator, dir_path)
target = create_target(estimator, 0.0, 105, 0.76, 0.0)

dump = {
    "hp_left_error": hp_left.error,
    "hp_right_error": hp_right.error,
    "target_raw": target.raw,
}

for speaker in sorted(room_frs):
    for side in ("left", "right"):
        dump[f"room_{speaker}_{side}"] = room_frs[speaker][side].error

for speaker in ["FL", "FR", "FC", "BL", "BR", "SL", "SR"]:
    for side in ("left", "right"):
        fr = FrequencyResponse(
            name=f"{speaker}-{side} eq",
            frequency=FrequencyResponse.generate_frequencies(
                f_step=1.01, f_min=10, f_max=estimator.fs / 2
            ),
            raw=0,
            error=0,
        )
        if room_frs is not None and speaker in room_frs and side in room_frs[speaker]:
            fr.error += room_frs[speaker][side].error
        hp_eq = hp_left if side == "left" else hp_right
        if hp_eq is not None:
            fr.error += hp_eq.error
        eq = eq_left if side == "left" else eq_right
        if eq is not None and type(eq) == FrequencyResponse:
            fr.error += eq.error
        fr.error -= target.raw
        dump[f"pre_error_{speaker}_{side}"] = fr.error.copy()
        fr.smoothen_heavy_light()
        fr.equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=estimator.fs / 2)
        dump[f"eq_{speaker}_{side}"] = fr.equalization.copy()
        dump[f"fir_{speaker}_{side}"] = fr.minimum_phase_impulse_response(
            fs=estimator.fs, normalize=False, f_res=5
        )

np.savez(out_path, **dump)
print(out_path)
