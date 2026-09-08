"""Environment only; deliberately no psutil or process queries."""
import os
import platform
import sys
import numpy as np
import scipy
import scipy.fft._basic_backend as fft
import sounddevice as sd

print('caller kept machine quiet.')
print('executable', sys.executable)
print('version', sys.version)
print('GIL', sys._is_gil_enabled())
print('platform', platform.platform(), platform.processor())
print('logical cores', os.cpu_count())
print('numpy', np.__version__, 'scipy', scipy.__version__, 'sounddevice', sd.__version__)
print('PortAudio', sd.get_portaudio_version())
print('FFT', fft.__file__)
print('duccfft', getattr(fft, '_duccfft', None))
print('threads', {key: os.environ.get(key) for key in (
    'OMP_NUM_THREADS', 'MKL_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'NUMEXPR_NUM_THREADS',
    'BLIS_NUM_THREADS', 'RAYON_NUM_THREADS', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')})
np.show_config()
scipy.show_config()
