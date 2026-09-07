"""Export deterministic P03 float64 oracles, one JSON object per fixture.

Float arrays are JSON numbers at Python repr precision; nonfinite values use
strings "-inf", "+inf", "nan" so serde_json can read them. Complex arrays are
[real, imaginary] pairs. No rounding, timestamps or platform-dependent paths.
Set IMPULCIFER_GOLDEN_BATCH=p05 to write only p05_* fixtures and skip other batches.
P05 minimum phase follows the installed SciPy (1.18.0 lifter: win[stop] = 1 + n_fft % 2).
"""
from pathlib import Path
import json
import hashlib
import io
import contextlib
import os
import platform
import sys
import warnings

import numpy as np
import scipy
from scipy import fft, fftpack, ndimage, signal, special, stats, interpolate

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
    """Export scipy/2.x P03 oracles in p03_*.json, preserving batch 1 generation."""
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


def save_p05(name, inputs, outputs):
    """Freeze scipy/2.x P05 oracles in p05_<name>.json, bounded to 200 kB."""
    path = OUT / f"p05_{name}.json"
    payload = plain({"inputs": inputs, "outputs": outputs, "meta": META})
    data = (json.dumps(payload, allow_nan=False, separators=(",", ":")) + "\n").encode()
    assert len(data) < 200_000, (path.name, len(data))
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    WRITTEN.append((path.name, len(data)))


def binary_p05(name, values):
    """Store scipy/2.x arrays as LE-f64; p05_* descriptors pin count/hash/end taps."""
    values = np.asarray(values, dtype="<f8")
    data = values.tobytes()
    path = OUT / f"p05_{name}.f64"
    assert len(data) < 200_000, (path.name, len(data))
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    WRITTEN.append((path.name, len(data)))
    return {"file": path.name, "length": len(values), "sha256": hashlib.sha256(data).hexdigest(),
            "first": values[:256], "last": values[-256:]}


def minimum_phase_stages(h, n_fft, half):
    """Transcribe scipy.signal.minimum_phase(method="homomorphic") of the installed
    SciPy (1.18.0) to expose the intermediate log spectrum and lifter.

    https://github.com/scipy/scipy/blob/v1.18.0/scipy/signal/_fir_filter_design.py
    SciPy 1.18 sets win[stop] = 1 + (n_fft % 2) (even: 1, odd: 2); 1.17.1 used
    even: 0, odd: 1. The frozen oracle is the installed version, so the golden
    output `y` comes from scipy.signal.minimum_phase itself and this
    transcription only supplies the stages (it is asserted to agree with scipy).
    """
    magnitude = np.abs(fft.fft(h, n_fft))
    log = np.log(magnitude + 1e-7 * np.min(magnitude[magnitude > 0]))
    if half:
        log *= 0.5
    lifter = np.zeros(n_fft)
    lifter[0] = 1
    stop = n_fft // 2
    lifter[1:stop] = 2
    lifter[stop] = 1 + (n_fft % 2)
    y = fft.ifft(np.exp(fft.fft(fft.ifft(log).real * lifter))).real
    return y[:(len(h) + 1) // 2 if half else len(h)], log, lifter


def export_p05():
    """Export firwin2/minimum_phase/savgol/peaks/FITPACK fixtures p05_* only."""
    from autoeq.frequency_response import FrequencyResponse
    from core.decay import _peak_index
    from core.impulse_response import ImpulseResponse
    from core.hrir import get_center_value

    before = len(WRITTEN)
    # Record the real runtime/backend separately from the pinned algorithm tag.
    config = io.StringIO()
    with contextlib.redirect_stdout(config):
        np.show_config()
    save_p05("environment", {}, {"platform": platform.platform(), "machine": platform.machine(),
             "numpy_config": config.getvalue(), "minimum_phase_algorithm": f"scipy-{scipy.__version__}",
             "minimum_phase_source": "https://github.com/scipy/scipy/blob/v1.18.0/scipy/signal/_fir_filter_design.py"})
    rng = np.random.default_rng(505)
    log_frequency = FrequencyResponse.generate_frequencies(f_min=20, f_max=20000, f_step=1.01)
    assert len(log_frequency) == 695
    # Actual AutoEQ minimum_phase_impulse_response mesh and preceding log interpolation.
    f = np.linspace(0., 24000., 4800)
    targets = {}
    for name in ("smooth", "notch", "flat"):
        db = 3 * np.cos(np.log10(log_frequency / 500.))
        if name == "notch":
            db -= 24 * np.exp(-0.5 * (np.log2(log_frequency / 3200.) / 0.16) ** 2)
        elif name == "flat":
            db = np.zeros_like(log_frequency)
        fr = FrequencyResponse(name=name, frequency=log_frequency, raw=db)
        gain_min = interpolate.InterpolatedUnivariateSpline(np.log10(fr.frequency), fr.raw, k=1)(np.log10(20.))
        fr.interpolate(f, pol_order=1)
        fr.raw[fr.frequency <= 20.] = gain_min
        fr.raw -= np.max(fr.raw) + 0.5
        g = 10 ** (2 * fr.raw / 20)
        g[-1] = 0
        h = signal.firwin2(9600, f, g, fs=48000)
        targets[name] = h
        save_p05(f"firwin2_{name}", {"numtaps": 9600, "freq": binary_p05(f"fir_freq_{name}", f),
                 "gain": binary_p05(f"fir_gain_{name}", g), "nfreqs": None, "fs": 48000.},
                 {"h": binary_p05(f"fir_taps_{name}", h), "mesh_size": 16385,
                  "mesh": binary_p05(f"fir_mesh_{name}", np.interp(np.linspace(0, 24000, 16385), f, g)),
                  "window": binary_p05(f"fir_window_{name}", signal.windows.hamming(9600, sym=True))})
    for name, taps, freq, gain, mesh in [
            ("duplicate", 33, [0, .3, .3, 1], [1, 1, .1, .1], None),
            ("odd", 17, [0, .1, .7, 1], [1, .5, .2, .1], None),
            ("custom", 16, [0, .5, 1], [1, .8, 0], 29),
            ("one", 1, [0, 1], [1, 1], None)]:
        save_p05(f"firwin2_{name}", {"numtaps": taps, "freq": freq, "gain": gain, "nfreqs": mesh, "fs": 2.},
                 {"h": signal.firwin2(taps, freq, gain, nfreqs=mesh, fs=2.)})
    odd = signal.firwin2(31, [0, .2, .8, 1], [1, .7, .3, .2])
    for name, h, n_fft in [("smooth", targets["smooth"], 9600), ("notch", targets["notch"], 9600),
                            ("odd", odd, 63), ("padded", odd, 128), ("odd_equal", odd, 31)]:
        for half in (True, False):
            key = f"{name}_{'half' if half else 'full'}"
            y = signal.minimum_phase(h, n_fft=n_fft, half=half)
            transcribed, log, lifter = minimum_phase_stages(h, n_fft, half)
            transcription_error = float(np.max(np.abs(transcribed - y)))
            assert transcription_error <= 1e-12 * max(1.0, float(np.max(np.abs(y)))), (key, transcription_error)
            save_p05(f"minimum_{key}", {"h": binary_p05(f"min_input_{key}", h), "n_fft": n_fft, "half": half},
                     {"y": binary_p05(f"min_output_{key}", y), "log": binary_p05(f"min_log_{key}", log),
                      "lifter": binary_p05(f"min_lifter_{key}", lifter),
                      "algorithm": f"scipy-{scipy.__version__}", "transcription_max_tap_difference": transcription_error})
    lengths = [3, 4, 31, 32, 9600]
    save_p05("minimum_default", {"lengths": lengths},
             {"n_fft": [2 ** int(np.ceil(np.log2(2*(n-1)/.01))) for n in lengths]})
    t = np.linspace(-1., 1., 695)
    for name, x in [("noise", rng.normal(size=695)), ("quadratic", 3*t*t+2*t-1),
                    ("constant", np.full(695, 3.25)), ("linear", 2*t-1)]:
        for w in (7, 11, 23):
            for passes in (1, 1000):
                y = x.copy()
                for _ in range(passes):
                    y = signal.savgol_filter(y, w, 2)
                save_p05(f"savgol_{name}_{w}_{passes}", {"x": x, "window": w, "polyorder": 2,
                         "passes": passes, "ratio": 1.01}, {"y": y})
    for w, order in [(1, 0), (5, 0), (5, 1), (9, 3), (11, 4), (7, 6), (101, 2)]:
        x = rng.normal(size=111)
        save_p05(f"savgol_general_{w}_{order}", {"x": x, "window": w, "polyorder": order, "passes": 1},
                 {"y": signal.savgol_filter(x, w, order)})
    octaves = [0., 1/12, 1/7, 1/3, 1., 2.5, 3.5, 4.5]
    ratios = [1.01]*5 + [2.]*3
    windows = [round(np.log(2**o)/np.log(r)) for o, r in zip(octaves, ratios)]
    windows = [w+1 if w % 2 == 0 else w for w in windows]
    fr = FrequencyResponse(name="window", frequency=log_frequency, raw=np.zeros(695))
    assert [fr._window_size(o) for o in octaves[:5]] == windows[:5]
    save_p05("fractional_window", {"octaves": octaves, "ratios": ratios}, {"window": windows})
    peak_cases = [[], [1.], [1., 0], [0, 1, 1, 0], [0, 2, 2, 2, 0],
                  [2, 2, 0, 1, 0, 2, 2], [0, .12589, 0, np.nextafter(.12589, 0), 0, 1, 0],
                  [0, np.nan, 1, 0, 2, 0], [0, np.inf, 0], [0, 1, 1, np.nan, 0]]
    for i, x in enumerate(peak_cases):
        for j, height in enumerate((None, .12589, 1., np.nan)):
            save_p05(f"peaks_{i}_{j}", {"x": x, "height": height}, {"indices": signal.find_peaks(x, height=height)[0]})
    impulses = [([], 0, None), ([], 7, None), ([0, .12589, 0, -1, 0], 0, None),
                ([0, -.12589, 0, 1, 0], 0, None), ([0, .12589, 0, -1, 0], 2, None),
                ([1, 0, 0], 0, None), ([0, 0, 1], 0, None), ([1, -1], 0, None),
                ([0, .2, .2, 0, -1, 0], 0, 99), ([0, 0, 0], 1, None),
                ([0, 1e-21, 0], 0, None), ([0, 1e-20, 0], 0, None),
                ([1, 2, 3], 2, 1), ([1, 2, 3], 5, None), ([0, np.nan, 1], 0, None),
                ([0, 1, np.inf, 0], 0, None)]
    for i, (data, start, end) in enumerate(impulses):
        x = np.asarray(data, dtype=float)
        index = int(_peak_index(x, start, end, .12589))
        ir = ImpulseResponse(x.copy(), 48000)
        assert ir.peak_index(start, end, .12589) == index
        save_p05(f"first_peak_{i}", {"data": x, "start": start, "end": end, "height": .12589},
                 {"index": index})
    for k in (1, 2, 3):
        for name, x in [("irregular", np.array([.2, .55, .9, 1.8, 2., 2.7, 3.5, 4.3])),
                        ("four", np.array([.2, .7, 1.9, 3.])),
                        ("minimal", np.linspace(.2, 3., k+1))]:
            y = np.sin(2.3*x)+.4*x*x
            at = np.r_[x[0]-1., x[0]-.2, x, (x[:-1]+x[1:])/2, x[-1]+.2, x[-1]+1.]
            spline = interpolate.InterpolatedUnivariateSpline(x, y, k=k)
            save_p05(f"spline_k{k}_{name}", {"x": x, "y": y, "k": k, "at": at},
                     {"y": spline(at), "natural_cubic": interpolate.CubicSpline(x, y, bc_type="natural")(at)
                      if k == 3 else []})
    csv = ROOT / "data/harman-in-room-headphone-target.csv"
    target = np.genfromtxt(csv, delimiter=",", names=True)
    for k in (1, 2, 3):
        at = np.r_[0., 1., np.geomspace(10, 20000, 101), 40000.]
        fr = FrequencyResponse(name="harman", frequency=target["frequency"], raw=target["raw"])
        fr.interpolate(at, pol_order=k)
        save_p05(f"log_axis_harman_k{k}", {"freq": target["frequency"], "values": target["raw"], "k": k, "at": at},
                 {"y": fr.raw, "source": "data/harman-in-room-headphone-target.csv",
                  "source_sha256": hashlib.sha256(csv.read_bytes()).hexdigest()})
    fr = FrequencyResponse(name="center", frequency=target["frequency"], raw=target["raw"])
    at = [5., 100., 1000., 30000.]
    save_p05("log_axis_center", {"freq": fr.frequency, "values": fr.raw, "k": 1, "at": at},
             {"y": [-get_center_value(fr, f) for f in at], "retry": "k=1; except ValueError: k=1",
              "center_returns_negative_interpolated_gain": True})
    new = WRITTEN[before:]
    print(f"P05: exported {len(new)} files; total {sum(s for _, s in new)} bytes")
    name, size = max(new, key=lambda item: item[1])
    print(f"P05 largest: {name}: {size} bytes (<200000)")
    if os.environ.get("IMPULCIFER_GOLDEN_BATCH") == "p05":
        print("P03/P07: skipped; only p05_* files written")
    print(f"P05 minimum_phase: goldens from scipy.signal.minimum_phase {scipy.__version__} (lifter even=1, odd=2)")
    print("P05 log retry: actual hrir starts k=1 and retries k=1 only on ValueError")
    print("P05 first_peak_index: wholly empty data returns 0 like Python, regardless of start")


def export_p07():
    """Export soundfile/2.x I/O fixtures; never rewrite other golden families."""
    import base64
    import tempfile
    import soundfile as sf
    from dataclasses import asdict
    from core.audio_io import read_wav, write_wav
    from core.constants import SPEAKER_NAMES, TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER
    from core.recording_progress import infer_sweep_segments
    from core.sweep_signal import _quantize_like_bundled_wav

    meta = {**META, "soundfile": sf.__version__, "libsndfile": sf.__libsndfile_version__}
    count = 0

    def save_io(name, inputs, outputs):
        nonlocal count
        path = OUT / f"p07_{name}.json"
        data = (json.dumps(plain({"inputs": inputs, "outputs": outputs, "meta": meta}),
                           allow_nan=False, separators=(",", ":")) + "\n").encode()
        assert len(data) < 200_000, (path.name, len(data))
        if not path.exists() or path.read_bytes() != data:
            path.write_bytes(data)
        count += 1

    def encode(data):
        return base64.b64encode(data).decode("ascii")

    rounding = {}
    with tempfile.TemporaryDirectory(prefix="impulcifer-p07-") as tmp:
        for bits in (16, 24, 32):
            scale = 2 ** (bits - 1)
            units = np.array([-2.5, -1.5, -.5, 0., .5, 1.5, 2.5])
            # Include halfway values, neighbors near target-integer boundaries,
            # and i32 halfway values to distinguish floor from round-then-shift.
            x = np.r_[-1.1, -1., np.nextafter(-1., 0.), units / scale,
                      np.array([-.01, .01, .99999999, 1.00000001]) / scale,
                      (np.array([-65536., -256., 256., 65536.]) - .5) / 2**31,
                      1. - .5 / scale, np.nextafter(1., 0.), 1., 1.1, .123456789]
            tracks = np.vstack([x, x[::-1], np.roll(x, 5)])
            path = Path(tmp) / f"pcm{bits}.wav"
            write_wav(str(path), 48000, tracks, bit_depth=bits)
            standard = path.read_bytes()
            wavex = io.BytesIO()
            sf.write(wavex, tracks.T, 48000, format="WAVEX", subtype=f"PCM_{bits}")
            decoded, _ = sf.read(io.BytesIO(standard), always_2d=True)
            save_io(f"write_{bits}", {"sample_rate": 48000, "bits": bits, "tracks": tracks},
                    {"wav_base64": encode(standard), "wavex_base64": encode(wavex.getvalue()),
                     "tracks": decoded.T})
            buffer = io.BytesIO()
            sf.write(buffer, units / scale, 48000, format="WAV", subtype=f"PCM_{bits}")
            buffer.seek(0)
            rounding[bits] = (sf.read(buffer)[0] * scale).astype(int).tolist()
            # Mono/stereo output must match Python's default RIFF bytes, too.
            for channels in (1, 2):
                buffer = io.BytesIO()
                sf.write(buffer, tracks[:channels].T, 48000, format="WAV", subtype=f"PCM_{bits}")
                save_io(f"small_{bits}_{channels}", {"sample_rate": 48000, "bits": bits,
                         "tracks": tracks[:channels]}, {"wav_base64": encode(buffer.getvalue())})

        # Readers: real libsndfile RIFF, RF64 and extensible PCM/float, arbitrary masks.
        samples = np.array([[-1., -.25, 0., .125, .75], [.5, 0., -.5, .25, -.125],
                            [.25, .125, 0., -.125, -.25]])
        for format_name in ("WAV", "WAVEX", "RF64"):
            for subtype in ("PCM_16", "PCM_24", "PCM_32", "FLOAT", "DOUBLE"):
                if not sf.check_format(format_name, subtype):
                    continue
                buffer = io.BytesIO()
                sf.write(buffer, samples.T, 44100, format=format_name, subtype=subtype)
                raw = buffer.getvalue()
                decoded, rate = sf.read(io.BytesIO(raw), always_2d=True)
                save_io(f"read_{format_name}_{subtype}", {"wav_base64": encode(raw)},
                        {"sample_rate": rate, "tracks": decoded.T})

    bundled = ROOT / "data/sweep-seg-FL-mono-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
    rate, tracks = read_wav(str(bundled), expand=True)
    save_io("read_bundled", {"file": bundled.relative_to(ROOT).as_posix()},
            {"sample_rate": rate, "frames": tracks.shape[1], "first": tracks[0, :64],
             "last": tracks[0, -64:], "sha256_le_f64": hashlib.sha256(tracks.astype("<f8").tobytes()).hexdigest()})
    values = np.array([[-1.1, -1., -.75, -2.5 / 2**31, -1.5 / 2**31, -.5 / 2**31,
                        0., .5 / 2**31, 1.5 / 2**31, 2.5 / 2**31, .123456789,
                        np.nextafter(1., 0.), 1., 1.1]])
    save_io("pcm32_round_trip", {"tracks": values}, {"tracks": _quantize_like_bundled_wav(values, 48000)})
    save_io("constants", {}, {"SPEAKER_NAMES": SPEAKER_NAMES, "TRUEHD_11CH_ORDER": TRUEHD_11CH_ORDER,
                              "TRUEHD_13CH_ORDER": TRUEHD_13CH_ORDER})
    cases = [("prefix-sweep-seg-fl,fr-stereo-6.15s-any suffix", 11.),
             ("sweep-seg-FL,FR-stereo-0s-tail", 10.),
             ("sweep-seg-FL-mono-1s-", 1.), ("sweep-seg-X-mono-1s-", 20.)]
    save_io("segments", {"cases": cases},
            {"segments": [[asdict(s) for s in infer_sweep_segments(name, duration)] for name, duration in cases]})
    print(f"P07: exported {count} fixtures; soundfile {sf.__version__}, libsndfile {sf.__libsndfile_version__}")
    for bits, result in rounding.items():
        print(f"P07 PCM_{bits}: [-2.5,-1.5,-0.5,0,0.5,1.5,2.5] target LSB -> {result}")
    print("P07: scale 2^31, nearest ties-even, saturate i32, arithmetic shift 16/8/0 bits")
    print("P07: default 3-channel WAV is RIFF PCM; WAVEX is extensible DIRECTOUT + fact; compare payload and WAVEX bytes separately")


if __name__ == "__main__":
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", RuntimeWarning)
        warnings.simplefilter("ignore", signal.BadCoefficients)
        if os.environ.get("IMPULCIFER_GOLDEN_BATCH") != "p05":
            main()
        else:
            print(f"Oracle: Python {META['python']}, NumPy {META['numpy']}, SciPy {META['scipy']}")
        export_p05()
        if os.environ.get("IMPULCIFER_GOLDEN_BATCH") != "p05":
            export_p07()
