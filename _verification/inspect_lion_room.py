import os
import sys

sys.path.insert(0, r"E:\Impulcifer\_verification\lion")

from impulse_response_estimator import ImpulseResponseEstimator
from room_correction import open_room_measurements, open_room_target, open_mic_calibration, open_generic_room_measurement


dir_path = r"E:\Impulcifer\_verification\lion_before_py38_b"
test_path = r"E:\Impulcifer\_verification\jaakko\data\sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"

estimator = ImpulseResponseEstimator.from_pickle(test_path)
target = open_room_target(estimator, dir_path)
mic = open_mic_calibration(estimator, dir_path)
rir = open_room_measurements(estimator, dir_path)
room_fr = open_generic_room_measurement(estimator, dir_path, mic, target)
print("generic", room_fr is not None)
print("speakers", sorted(rir.irs.keys()))
for speaker, pair in rir.irs.items():
    for side, ir in pair.items():
        print("before", speaker, side, len(ir.data), ir.data.dtype, ir.data[:5], ir.data[-5:])
        ir.crop_head()
        print("after_head", speaker, side, len(ir.data))
rir.crop_tails()
for speaker, pair in rir.irs.items():
    for side, ir in pair.items():
        print("after_tail", speaker, side, len(ir.data), ir.data.dtype)
print("write target", os.path.join(dir_path, "room-responses.wav"))
rir.write_wav(os.path.join(dir_path, "room-responses.wav"))
print("wrote")
