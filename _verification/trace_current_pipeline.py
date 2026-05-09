import os
import sys
import __main__

sys.path.insert(0, r"E:\Impulcifer")

from core.impulse_response_estimator import ImpulseResponseEstimator

__main__.ImpulseResponseEstimator = ImpulseResponseEstimator

from impulcifer import (
    create_target,
    equalization,
    headphone_compensation,
    open_binaural_measurements,
    open_impulse_response_estimator,
)
from core.room_correction import room_correction
from core.virtual_bass import apply_virtual_bass_to_hrir


dir_path = r"E:\Impulcifer\_verification\current_trace"
test_path = r"E:\Impulcifer\_verification\jaakko\data\sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"


def print_lengths(label, hrir):
    lengths = {
        f"{speaker}-{side}": len(ir.data)
        for speaker, pair in hrir.irs.items()
        for side, ir in pair.items()
    }
    print(label, min(lengths.values()), max(lengths.values()), sorted(lengths.items()))


estimator = open_impulse_response_estimator(dir_path, file_path=test_path)
room_correction(estimator, dir_path, specific_limit=400, generic_limit=300)
headphone_compensation(estimator, dir_path, None)
equalization(estimator, dir_path)
create_target(estimator, 0.0, 105, 0.76, 0.0)
hrir = open_binaural_measurements(estimator, dir_path)
print_lengths("open", hrir)
hrir.crop_heads(head_ms=1)
print_lengths("crop_heads", hrir)
hrir.align_ipsilateral_all(
    speaker_pairs=[
        ("FL", "FR"),
        ("SL", "SR"),
        ("BL", "BR"),
        ("TFL", "TFR"),
        ("TSL", "TSR"),
        ("TBL", "TBR"),
        ("FC", "FC"),
        ("WL", "WR"),
    ],
    segment_ms=30,
)
print_lengths("align_ipsi", hrir)
hrir.align_onset_groups_peak_leftref()
print_lengths("align_onset", hrir)
hrir.crop_tails()
print_lengths("crop_tails", hrir)
apply_virtual_bass_to_hrir(hrir, crossover_freq=250, head_ms=1, hp_freq=15.0, invert_polarity=None)
print_lengths("vbass", hrir)
