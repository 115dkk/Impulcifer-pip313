"""Export deterministic P03 float64 oracles, one JSON object per fixture.

Float arrays are JSON numbers at Python repr precision; nonfinite values use
strings "-inf", "+inf", "nan" so serde_json can read them. Complex arrays are
[real, imaginary] pairs. No rounding, timestamps or platform-dependent paths.
Only p03_*.json files named by this script are written; other batches are intact.
"""
from pathlib import Path
import json
import platform
import sys
import warnings

import numpy as np
import scipy
from scipy import fft, fftpack, ndimage, signal, special, stats

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from autoeq import biquad  # noqa: E402
from core.audio_io import magnitude_response, running_mean  # noqa: E402
from core.virtual_bass import _rbj_high_shelf  # noqa: E402

OUT = Path(__file__).with_name("goldens")
META = {"scipy": scipy.__version__, "numpy": np.__version__,
        "python": platform.python_version(), "dtype": "float64/complex128"}
WRITTEN = []


def plain(value):
    if isinstance(value, np.ndarray):
        return plain(value.tolist())
    if isinstance(value, dict):
        return {k: plain(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [plain(v) for v in value]
    if isinstance(value, (complex, np.complexfloating)):
        return [plain(float(value.real)), plain(float(value.imag))]
    if isinstance(value, (float, np.floating)):
        if np.isnan(value):
            return "nan"
        if np.isneginf(value):
            return "-inf"
        if np.isposinf(value):
            return "+inf"
        return float(value)
    if isinstance(value, np.integer):
        return int(value)
    return value


def save(name, inputs, outputs):
    path = OUT / f"p03_{name}.json"
    payload = plain({"inputs": inputs, "outputs": outputs, "meta": META})
    with path.open("w", encoding="utf-8", newline="\n") as stream:
        json.dump(payload, stream, allow_nan=True, separators=(",", ":"))
        stream.write("\n")
    size = path.stat().st_size
    assert size < 200_000, (path.name, size)
    WRITTEN.append((path.name, size))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(1234)
    signals = {f"noise{n}": rng.standard_normal(n) for n in (257, 1024, 1025)}
    signals.update({"one": np.array([2.5]), "two": np.array([2., -3.]),
                    "impulse8": np.eye(1, 8)[0], "impulse9": np.eye(1, 9)[0],
                    "zeros8": np.zeros(8), "zeros9": np.zeros(9)})
    for name, x in signals.items():
        spectrum = np.fft.rfft(x)
        save(f"rfft_{name}", {"x": x}, {"spectrum": spectrum})
        save(f"irfft_{name}", {"spectrum": spectrum, "n": len(x)},
             {"x": np.fft.irfft(spectrum, n=len(x))})
        with np.errstate(divide="ignore"):
            save(f"magnitude_{name}", {"x": x},
                 {"db": magnitude_response(x, 48000)[1]})
        z = x + 1j * np.roll(x, 1)
        save(f"fft_{name}", {"x": z}, {"spectrum": np.fft.fft(z)})
        save(f"ifft_{name}", {"x": z}, {"spectrum": np.fft.ifft(z)})
    for n in (1, 2, 5, 6):
        for size in (0, 1, 2, 7):
            z = np.arange(size) + 1j * np.arange(size, 2 * size)
            # NumPy 2.4.6 irfft(empty,n) exposes uninitialized values on this
            # machine. Never freeze allocator-dependent output as an oracle.
            save(f"irfft_resize_{n}_{size}", {"spectrum": z, "n": n},
                 {"error": "empty spectrum"} if size == 0 else
                 {"x": np.fft.irfft(z, n=n)})
    lengths = list(range(0, 40)) + list(range(1000, 1101)) + [295269, 295270, 295271]
    save("fast_lengths", {"n": lengths},
         {"legacy": [fftpack.next_fast_len(n) for n in lengths],
          "real": [fft.next_fast_len(n, real=True) for n in lengths],
          "complex_reference_only": [fft.next_fast_len(n) for n in lengths]})
    # Both parity combinations, first argument shorter, and both FFT/direct paths.
    pairs = [(np.arange(a, dtype=float) - 1, np.arange(v, dtype=float) + .5)
             for a in (1, 2, 3, 4, 7) for v in (1, 2, 3, 4, 8)]
    pairs += [(signals["noise257"], signals["noise1024"]),
              (signals["noise1025"], signals["noise257"]),
              (np.zeros(257), signals["noise257"])]
    for case, (x, h) in enumerate(pairs):
        for mode in ("full", "same", "valid"):
            save(f"convolve_{case}_{mode}", {"a": x, "v": h, "mode": mode},
                 {"y": signal.convolve(x, h, mode=mode)})
            save(f"correlate_{case}_{mode}", {"a": x, "v": h, "mode": mode},
                 {"y": signal.correlate(x, h, mode=mode),
                  "lags": signal.correlation_lags(len(x), len(h), mode=mode)})
    for n in (0, 1, 2, 8, 9, 257):
        for sym in (False, True):
            for name in ("hann", "hamming", "boxcar"):
                if n:
                    np.testing.assert_array_equal(signal.get_window(name, n, fftbins=not sym),
                                                  getattr(signal.windows, name)(n, sym=sym))
                else:
                    try:
                        signal.get_window(name, n, fftbins=not sym)
                    except ValueError:
                        pass
                    else:
                        raise AssertionError("oracle get_window zero policy changed")
                save(f"window_{name}_{n}_{int(sym)}", {"name": name, "n": n, "sym": sym},
                     {"y": getattr(signal.windows, name)(n, sym=sym),
                      "get_window_error": n == 0})
            for beta in (0., -5.65326, 5.65326, 20., 1000.):
                save(f"kaiser_{n}_{int(sym)}_{beta}", {"n": n, "sym": sym, "beta": beta},
                     {"y": signal.windows.kaiser(n, beta, sym=sym)})
    for sym in (False, True):
        y = signal.windows.kaiser(32001, 5.65326, sym=sym)
        save(f"kaiser_long_{int(sym)}", {"n": 32001, "sym": sym, "beta": 5.65326},
             {"first": y[:64], "last": y[-64:], "sum": y.sum()})
    t = np.arange(4096) / 48000
    chirp = signal.chirp(t, f0=5., f1=22000., t1=t[-1], method="linear")
    for order in (0, 1, 2, 3, 4, 5, 8):
        for cutoff in (15., 250., 18000.):
            for kind in ("lowpass", "highpass"):
                wn = cutoff / 24000
                sos = signal.butter(order, wn, btype=kind, output="sos")
                key = f"{order}_{int(cutoff)}_{kind}"
                save(f"butter_{key}", {"order": order, "wn": wn, "kind": kind}, {"sos": sos})
                if order in (4, 8) and cutoff != 18000.:
                    save(f"sosfilt_{key}", {"sos": sos, "x": chirp, "order": order,
                         "wn": wn, "kind": kind},
                         {"y": signal.sosfilt(sos, chirp)})
    for name, x in (("impulse", np.eye(1, 257)[0]), ("noise", signals["noise257"]),
                    ("dc", np.ones(257)), ("zeros", np.zeros(257))):
        sos = signal.butter(4, 250 / 24000, output="sos")
        save(f"sosfilt_{name}", {"sos": sos, "x": x}, {"y": signal.sosfilt(sos, x)})
    parameters = [(105., .76, 4.), (6000., 20., -12.), (150., .760, -1.5),
                  (400., .660, -3.), (800., .610, -3.5), (23900., .1, 60.),
                  (105., 1., -60.), (1000., 1., 0.)]
    freqs = np.r_[0., np.geomspace(10., 23999., 96), 24000.]
    for index, (fc, q, gain) in enumerate(parameters):
        for name in ("peaking", "low_shelf", "high_shelf"):
            coeff = getattr(biquad, name)(fc, q, gain, fs=48000.)
            a0, a1, a2, b0, b1, b2 = coeff
            b, a = [b0, b1, b2], [a0, -a1, -a2]
            save(f"rbj_{name}_{index}", {"name": name, "fc": fc, "q": q,
                 "gain": gain, "fs": 48000., "freqs": freqs},
                 {"b": b, "a": a, "db": biquad.digital_coeffs(freqs, 48000., *coeff)})
            save(f"tf2sos_{name}_{index}", {"b": b, "a": a}, {"sos": signal.tf2sos(b, a)})
        # Exercise the actual virtual_bass unnormalized polynomial oracle.
        if index in (2, 3, 4):
            amplitude = 10 ** (gain / 40.)
            w = 2 * np.pi * fc / 48000.
            alpha = np.sin(w) / (2 * q)
            a0 = (amplitude + 1) - (amplitude - 1) * np.cos(w) + 2 * np.sqrt(amplitude) * alpha
            coeff = biquad.high_shelf(fc, q, gain, fs=48000.)
            a = np.array([coeff[0], -coeff[1], -coeff[2]]) * a0
            b = np.array(coeff[3:]) * a0
            save(f"tf2sos_vbass_{index}", {"b": b, "a": a},
                 {"sos": _rbj_high_shelf(fc, 48000, gain, q)})
    for i, (b, a) in enumerate([([1.], [2.]), ([0., 1., 2.], [1., -.5, .1]),
                               ([1.], [1., -.5]), ([1., 2., 3.], [1.]),
                               ([0., 0., 0.], [1., -.5, .1]),
                               ([1., 0., 0.], [0., 2., -.5])]):
        save(f"tf2sos_edge_{i}", {"b": b, "a": a}, {"sos": signal.tf2sos(b, a)})
    x = np.linspace(-3, 9, 257)
    lines = [(x, 2.3 * x - .7 + rng.normal(0, .2, x.size)),
             (np.array([1., 2.]), np.array([3., 5.])),
             (np.arange(5.), np.ones(5)), (np.ones(3), np.arange(3.)),
             (np.array([]), np.array([])), (np.array([1.]), np.array([2.]))]
    for i, (x, y) in enumerate(lines):
        r = stats.linregress(x, y)
        save(f"linregress_{i}", {"x": x, "y": y},
             {k: getattr(r, k) for k in ("slope", "intercept", "rvalue", "stderr")})
    x = np.array([-np.inf, -1000., -745., -710., -100., -1., 0., 1., 100., 710., 1000., np.inf, np.nan])
    save("expit", {"x": x}, {"y": special.expit(x)})
    for i, x in enumerate((signals["noise257"], np.array([]), np.array([3.]))):
        for n in (1, 2, 7, 257, 258):
            save(f"running_{i}_{n}", {"x": x, "n": n}, {"y": running_mean(x, n)})
    for rows, cols in ((5, 7), (1, 7), (5, 1), (1, 1), (0, 7), (5, 0), (2, 2)):
        data = rng.normal(size=(rows, cols))
        save(f"uniform_{rows}_{cols}", {"rows": rows, "cols": cols, "data": data.ravel()},
             {"y": ndimage.uniform_filter(data, size=3, mode="constant", cval=0.).ravel()})
    print(f"Oracle: Python {META['python']}, NumPy {META['numpy']}, SciPy {META['scipy']}")
    print(f"Exported {len(WRITTEN)} deterministic fixtures; total {sum(s for _, s in WRITTEN)} bytes")
    name, size = max(WRITTEN, key=lambda item: item[1])
    print(f"Largest: {name}: {size} bytes (<200000)")
    print("real next_fast_len: 7->8, 11->12, 295270->300000 (235, not 235711)")


if __name__ == "__main__":
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        warnings.simplefilter("ignore", signal.BadCoefficients)
        main()
