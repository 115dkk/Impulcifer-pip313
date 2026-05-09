import sys

sys.path.insert(0, r"E:\Impulcifer\_verification\lion")

from impulse_response_estimator import ImpulseResponseEstimator
from room_correction import room_correction


dir_path = r"E:\Impulcifer\_verification\lion_before_py38_b"
test_path = r"E:\Impulcifer\_verification\jaakko\data\sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl"

estimator = ImpulseResponseEstimator.from_pickle(test_path)
room_correction(estimator, dir_path)
print("done")
