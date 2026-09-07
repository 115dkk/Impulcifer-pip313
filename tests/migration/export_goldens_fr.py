"""Freeze the P09 vendored AutoEQ subset without modifying other batches/demo.

Run in the foreground. Seed 909; arrays >4096 values use P05 LE-f64 descriptors.
Python line references and input/output fields are retained in each fixture.
"""
from pathlib import Path
import contextlib
import hashlib
import io
import json
import platform
import shutil
import sys
import tempfile

import numpy as np
import scipy
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from autoeq.frequency_response import FrequencyResponse as FR  # noqa: E402
from core.hrir import get_center_value  # noqa: E402
from core.impulse_response import ImpulseResponse  # noqa: E402
from core.impulse_response_estimator import ImpulseResponseEstimator  # noqa: E402
from core.pipeline_stages import headphone_compensation  # noqa: E402
from export_goldens import plain  # noqa: E402

OUT = Path(__file__).with_name("goldens")
META = {"python": platform.python_version(), "numpy": np.__version__,
        "scipy": scipy.__version__, "dtype": "float64", "seed": 909}
FIELDS = ("frequency", "raw", "smoothed", "error", "error_smoothed", "equalization",
          "equalized_raw", "equalized_smoothed", "target")
WRITTEN = []


def write(path, data):
    """P05 byte-stable, scoped fixture writing; no unrelated output files."""
    assert path.name.startswith("p09_")
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    WRITTEN.append((path.name, len(data)))


def encode(name, value):
    """Use round-trip JSON or P05 headerless little-endian float64 arrays."""
    if isinstance(value, np.ndarray):
        value = np.asarray(value, dtype="<f8")
        if value.size > 4096:
            data = value.tobytes()
            path = OUT / f"p09_{name}.f64"
            write(path, data)
            return {"file": path.name, "length": value.size,
                    "sha256": hashlib.sha256(data).hexdigest(),
                    "first": plain(value[:256]), "last": plain(value[-256:])}
        return plain(value)
    if isinstance(value, dict):
        return {k: encode(f"{name}_{k}", v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [encode(f"{name}_{i}", v) for i, v in enumerate(value)]
    return plain(value)


def state(fr):
    """The eight application arrays and frequency, including empty reset fields."""
    return {key: np.array(getattr(fr, key), dtype=float, copy=True) for key in FIELDS}


def save(name, inputs, outputs, source):
    payload = encode(name, {"inputs": inputs, "outputs": outputs,
                            "meta": {**META, "source": source}})
    data = (json.dumps(payload, allow_nan=False, separators=(",", ":")) + "\n").encode()
    write(OUT / f"p09_{name}.json", data)


def main():
    OUT.mkdir(exist_ok=True)
    config = io.StringIO()
    with contextlib.redirect_stdout(config):
        np.show_config()
    save("environment", {}, {"platform": platform.platform(), "machine": platform.machine(),
         "numpy_config": config.getvalue()}, "installed oracle environment")
    grids = [(10., 24000., 1.01), (20., 20000., 1.01), (20., 24000., 1.01)]
    for i, (lo, hi, step) in enumerate(grids):
        f = FR.generate_frequencies(lo, hi, step)
        save(f"generate_{i}", {"min": lo, "max": hi, "step": step},
             {"frequency": f, "count": len(f), "last": f[-1]}, "frequency_response.py:850-857")
    grid = FR.generate_frequencies(10., 24000., 1.01)
    base = FR(name="grid", frequency=grid)
    octaves = [1/12, 1/6, 1/5, 1/3, 1.3, 2.]
    save("window", {"frequency": grid, "octaves": octaves},
         {"sizes": [base._window_size(o) for o in octaves]}, "frequency_response.py:1033-1050")
    for i, args in enumerate([(100., 10000., 0., 1.), (1000., 6000., 0., 1.),
                              (20000., 24000., 0., 1.), (10000., 24000., 40., 6.),
                              (20000., 24000., 15., 6.), (2000., 24000., 15., 6.),
                              (6000., 8000., 6., 6.), (6000., 8000., 1., 0.5)]):
        save(f"sigmoid_{i}", {"frequency": grid, "args": args},
             {"y": base._sigmoid(*args)}, "frequency_response.py:1052-1058")
    for i, args in enumerate([(0., 105., .76, 0.), (-6., 105., .76, None),
                              (4., 80., .71, -1.), (0., 105., .71, None)]):
        save(f"target_{i}", {"frequency": grid, "args": args},
             {"y": base.create_target(*args)}, "frequency_response.py:942-982; biquad.py:52-79,112-131")

    rng = np.random.default_rng(909)
    x = np.linspace(np.log10(15.), np.log10(22000.), 300)
    x[1:-1] += rng.uniform(-.003, .003, 298)
    f = 10**x
    raw = 3*np.cos(np.log10(f/500.)) - 12*np.exp(-.5*(np.log10(f/2400.)/.025)**2)
    synthetic = FR(name="synthetic", frequency=f, raw=raw)
    for name, query, nan in [("log", grid, False), ("linear", np.linspace(0, 24000, 4800), False),
                             ("nan", grid, True)]:
        fr = synthetic.copy()
        if nan:
            fr.raw[[0, 27, 28, 149, 298]] = np.nan
        else:
            for j, key in enumerate(FIELDS[2:]):
                setattr(fr, key, raw.copy() + j*.1)
        inputs = {"fr": state(fr), "query": query, "k": 1}
        fr.interpolate(query, pol_order=1)
        save(f"interpolate_{name}", inputs, state(fr), "frequency_response.py:859-901")
    for name, at in [("scalar", 1000.), ("band", [100., 10000.])]:
        fr = synthetic.copy()
        for j, key in enumerate(FIELDS[2:]):
            setattr(fr, key, raw.copy() + j*.1)
        inputs = {"fr": state(fr), "at": at}
        shift = fr.center(at)
        save(f"center_{name}", inputs, {"fr": state(fr), "shift": shift}, "frequency_response.py:903-940")
    bands = [[100., 3000.], [100., 10000.]]
    save("center_values", {"fr": state(synthetic), "bands": bands, "at": 1000.},
         {"values": [get_center_value(synthetic, b) for b in bands],
          "scalar": get_center_value(synthetic, 1000.)}, "core/hrir.py:44-74")
    target_path = ROOT / "data/harman-in-room-loudspeaker-target.csv"
    target = FR.read_from_csv(target_path)
    target.interpolate(grid)
    measurement = synthetic.copy()
    measurement.interpolate(grid)
    for name, compensation, minimum in [("zero", FR(name="zero", frequency=grid, raw=0), False),
                                         ("harman", target, True)]:
        fr = measurement.copy()
        inputs = {"fr": state(fr), "compensation": state(compensation), "min_mean_error": minimum,
                  "target_source": str(target_path.relative_to(ROOT)) if minimum else "zero"}
        fr.compensate(compensation, min_mean_error=minimum)
        save(f"compensate_{name}", inputs, state(fr), "frequency_response.py:984-1031")

    # All plot/WAV side effects remain in this temporary directory, including
    # any pre-existing files copied from demo. No cleanup touches the real demo.
    with tempfile.TemporaryDirectory(prefix="impulcifer-p09-") as temp:
        demo = Path(temp) / "demo"
        shutil.copytree(ROOT / "data/demo", demo)
        estimator = ImpulseResponseEstimator.from_wav(str(ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"))
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            left, right = headphone_compensation(estimator, str(demo))
        save("headphones", {"source": "data/demo/headphones.wav"},
             {"left": state(left), "right_error": right.error}, "core/pipeline_stages.py:353-470")
        heavy = left.copy()
        heavy.smoothen_heavy_light()
        save("heavy_light", {"fr": state(left)}, state(heavy), "frequency_response.py:1181-1239")
        for name, maximum, lower, resolution in [("default", 40., 10000., 5.),
                                                  ("balance_left", 15., 20000., 10.),
                                                  ("balance_trend", 15., 2000., 10.)]:
            fr = heavy.copy()
            p = {"max_gain": maximum, "treble_f_lower": lower, "treble_f_upper": 24000.}
            inputs = {"fr": state(fr), "params": p}
            fr.equalize(**p)
            save(f"equalize_{name}", inputs, state(fr), "frequency_response.py:1241-1310")
            ir = fr.minimum_phase_impulse_response(fs=48000, normalize=False, f_res=resolution)
            # Isolate the existing FIR primitives when the full chain fails.
            from scipy.interpolate import InterpolatedUnivariateSpline
            from scipy.fftpack import next_fast_len
            from scipy.signal import firwin2
            n = next_fast_len(round(24000 / (resolution / 2)))
            mesh = np.linspace(0, 24000, n)
            work = FR(name="fr_data", frequency=fr.frequency, raw=fr.equalization)
            f_min = max(work.frequency[0], resolution / 2)
            gain_min = InterpolatedUnivariateSpline(np.log10(work.frequency), work.raw, k=1)(np.log10(f_min))
            work.interpolate(mesh, pol_order=1)
            work.raw[work.frequency <= f_min] = gain_min
            gain = 10 ** (work.raw * 2 / 20)
            gain[-1] = 0
            linear_ir = firwin2(2*n, mesh, gain, fs=48000)
            save(f"minimum_{name}", {"fr": state(fr), "fs": 48000, "normalize": False, "f_res": resolution,
                 "mesh": mesh, "gain": gain, "linear_ir": linear_ir},
                 {"y": ir}, "frequency_response.py:637-681")
        for name, kwargs in [("third", {"window_size": 1/3, "treble_window_size": 1/3}),
                              ("sixth", {"window_size": 1/6, "treble_window_size": 1/6}),
                              ("two", {"window_size": 2., "treble_f_lower": 20000., "treble_f_upper": 24000.})]:
            fr = left.copy()
            fr.smoothen_fractional_octave(**kwargs)
            save(f"smoothen_{name}", {"fr": state(left), "params": kwargs}, state(fr), "frequency_response.py:1060-1179")

    texts = [("strict", "frequency,raw,target\n200.0,-3.5,1.0\n20.0,2.0,0.0\n"),
             ("guess", "# ignored comment\n20.0; -3.5; 12.0\n200.0; 2.0\n"),
             ("single", "frequency,raw,target\n5,2,0\n20.0,3,1\n200.0,-3.5,0.0\n")]
    for path in sorted((ROOT / "data").rglob("*.csv")):
        try:
            fr = FR.read_from_csv(path)
        except (ValueError, TypeError, IndexError) as exc:
            print(f"CSV rejected: {path.relative_to(ROOT)}: {exc}")
            continue
        save("csv_file_" + path.stem, {"name": path.stem, "text": path.read_text(encoding="utf-8"),
             "path": path.relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()},
             state(fr), "frequency_response.py:181-242")
    with tempfile.TemporaryDirectory(prefix="impulcifer-p09-csv-") as temp:
        for name, text in texts:
            path = Path(temp) / f"{name}.csv"
            path.write_text(text, encoding="utf-8", newline="\n")
            fr = FR.read_from_csv(path)
            save(f"csv_{name}", {"name": name, "text": text}, state(fr), "frequency_response.py:181-242")
    source = ROOT / "data/demo/FL.wav"
    if not source.exists():
        source = ROOT / "data/demo/FL,FR.wav"
    data, fs = sf.read(source, dtype="float64", always_2d=True)
    for name, samples in [("demo", data[:8192, 0]), ("one", np.array([1.])), ("empty", np.array([]))]:
        fr = ImpulseResponse(samples, fs).frequency_response()
        save(f"magnitude_{name}", {"data": samples, "fs": fs, "source": source.relative_to(ROOT).as_posix()}, state(fr), "core/impulse_response.py:157-188")
    print(f"Oracle: Python {META['python']}, NumPy {META['numpy']}, SciPy {META['scipy']}; seed 909")
    print("Room target: data/harman-in-room-loudspeaker-target.csv")
    print(f"P09: {len(WRITTEN)} files, {sum(size for _, size in WRITTEN)} bytes")
    print("Headphone export used a temporary demo copy; data/demo unchanged.")


if __name__ == "__main__":
    main()
