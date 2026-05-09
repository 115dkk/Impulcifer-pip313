import os
import runpy
import sys

lion_dir = r"E:\Impulcifer\_verification\lion"
sys.path.insert(0, lion_dir)
sys.path.insert(1, r"E:\Impulcifer")

import hrir
import scipy.signal
from scipy.signal.windows import hann

if not hasattr(scipy.signal, "hanning"):
    scipy.signal.hanning = hann

_orig_write_wav = hrir.HRIR.write_wav


def _write_wav_skip_room_responses(self, file_path, *args, **kwargs):
    try:
        return _orig_write_wav(self, file_path, *args, **kwargs)
    except RuntimeError:
        if os.path.basename(file_path).lower() == "room-responses.wav":
            return None
        raise


hrir.HRIR.write_wav = _write_wav_skip_room_responses

sys.argv = [
    os.path.join(lion_dir, "impulcifer.py"),
    "--test_signal",
    r"E:\Impulcifer\_verification\jaakko\data\sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl",
    "--dir_path",
    r"E:\Impulcifer\_verification\lion_py313_control_250",
    "--vbass",
    "250",
]

runpy.run_path(os.path.join(lion_dir, "impulcifer.py"), run_name="__main__")
