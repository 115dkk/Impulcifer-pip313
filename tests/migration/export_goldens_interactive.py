"""P24: freeze the 2.x Bokeh analysis numbers behind --interactive_plots.

Run from the repository root with the 2.x environment (NumPy/SciPy/Bokeh):

    python tests/migration/export_goldens_interactive.py

The oracle is the unmodified 2.x code: the pure functions of
``core/plotting/analysis.py`` for the primitive cases, and the real
``HRIR.generate_*_bokeh_layout`` generators (their ColumnDataSources, titles and
legend labels) for the assembly cases. Signals are deterministic: synthetic
impulses, pure delays, level differences, silence and noise from
``numpy.random.default_rng(1234)``. Inputs are stored next to the outputs so the
Rust side reads exactly the same float64 samples. Arrays use repr-precision JSON
numbers; nonfinite values are the strings "nan", "-inf", "+inf". Each file stays
below 200,000 bytes. No timestamps or platform paths are written.
"""
from pathlib import Path
import json
import platform
import sys
import types

import numpy as np
import scipy

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from export_goldens import plain  # noqa: E402
from core.plotting import analysis  # noqa: E402

OUT = Path(__file__).with_name("goldens")
META = {"python": platform.python_version(), "numpy": np.__version__,
        "scipy": scipy.__version__, "dtype": "float64", "seed": 1234}
WRITTEN = []


def save(name, payload):
    path = OUT / f"p24_interactive_{name}.json"
    data = (json.dumps(plain({**payload, "meta": META}), allow_nan=False,
                       separators=(",", ":")) + "\n").encode()
    assert len(data) < 200_000, (path.name, len(data))
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    WRITTEN.append((path.name, len(data)))


def signals():
    """Named float64 inputs. Lengths exercise 2/3/5/7/11-smooth FFT sizes.

    Returns (signals, recipes): recipes describe the derived inputs exactly
    (shift by whole samples, then one IEEE multiply) so the fixture stores only
    the noise-based arrays.
    """
    rng = np.random.default_rng(1234)

    def reverb(n, fs, onset, tau):
        t = np.arange(n) / fs
        x = rng.standard_normal(n) * np.exp(-t / tau) * 0.05
        x[onset] += 1.0
        return x

    s = {}
    recipes = {}

    def derive(name, source, shift, gain):
        x = np.zeros_like(s[source])
        x[shift:] = s[source][:len(x) - shift] * gain
        s[name] = x
        recipes[name] = {"source": source, "shift": shift, "gain": gain}

    def impulse(name, n, index, value):
        x = np.zeros(n)
        x[index] = value
        s[name] = x
        recipes[name] = {"length": n, "index": index, "value": value}

    s["reverb"] = reverb(1001, 48000, 20, 0.005)            # fft 1008 = 2^4 3^2 7
    derive("reverb_delayed", "reverb", 11, 0.5)            # right 11 samples later, -6 dB
    derive("reverb_inverted", "reverb", 0, -1.0)           # IPD at the +/-180 branch cut
    s["noise_left"] = rng.standard_normal(1331)            # fft 1331 = 11^3
    s["noise_right"] = rng.standard_normal(1331)
    impulse("impulse_left", 1200, 100, 1.0)
    impulse("impulse_right", 1200, 110, 0.8)               # 2.x IPD test shape
    s["unequal_left"] = reverb(997, 48000, 5, 0.004)        # prime length, fft 1000
    s["unequal_right"] = reverb(1200, 48000, 9, 0.006)
    s["short_left"] = rng.standard_normal(20)              # shorter than the IACC window
    s["short_right"] = rng.standard_normal(20)
    impulse("silence", 500, 0, 0.0)
    s["tiny"] = rng.standard_normal(300) * 1e-8            # total energy below 1e-12
    impulse("single_left", 1, 0, 0.5)
    impulse("single_right", 1, 0, 0.25)
    s["reverb_44k"] = reverb(1103, 44100, 12, 0.005)
    derive("reverb_44k_delayed", "reverb_44k", 7, 0.7)
    s["reverb_8k"] = reverb(401, 8000, 3, 0.01)
    derive("reverb_8k_scaled", "reverb_8k", 0, 0.25)
    return s, recipes


PAIRS = [
    # name, left, right, fs
    ("identical", "reverb", "reverb", 48000),
    ("delay_level", "reverb", "reverb_delayed", 48000),
    ("inverted", "reverb", "reverb_inverted", 48000),
    ("noise", "noise_left", "noise_right", 48000),
    ("impulse_delay", "impulse_left", "impulse_right", 48000),
    ("unequal_lengths", "unequal_left", "unequal_right", 48000),
    ("short", "short_left", "short_right", 48000),
    ("silence", "silence", "silence", 48000),
    ("left_silent", "silence", "tiny", 48000),
    ("single", "single_left", "single_right", 48000),
    ("fs44100", "reverb_44k", "reverb_44k_delayed", 44100),
    ("fs8000", "reverb_8k", "reverb_8k_scaled", 8000),
]


def primitive_cases(s):
    bands_cases = []
    for fs in (8000, 16000, 22050, 32000, 44100, 48000, 96000, 192000):
        bands_cases.append({"fs": fs, "centers": list(analysis.DEFAULT_OCTAVE_CENTERS),
                            "bands": analysis.octave_bands(fs)})
    third = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000, 20000, 25000]
    for fs in (44100, 48000):
        bands_cases.append({"fs": fs, "centers": third,
                            "bands": analysis.octave_bands(fs, centers=third)})

    pair_cases = []
    for name, left, right, fs in PAIRS:
        l, r = s[left], s[right]
        bands = analysis.octave_bands(fs)
        # Include a band without FFT bins and one at/above Nyquist (clamped to NaN).
        custom = bands + [(10.0, 10.5), (fs / 2, fs)]
        lags_ms, iacf, iacc, tau = analysis.interaural_cross_correlation(l, r, fs)
        wide = analysis.interaural_cross_correlation(l, r, fs, max_delay_ms=2.5)
        pair_cases.append({
            "name": name, "left": left, "right": right, "fs": fs, "bands": custom,
            "fft_len": int(scipy.fft.next_fast_len(max(len(l), len(r)))),
            "ild": analysis.band_interaural_level_difference(l, r, fs, custom),
            "ipd": analysis.band_interaural_phase_difference(l, r, fs, custom),
            "iacc": {"max_delay_ms": 1.0, "lags_ms": lags_ms, "iacf": iacf,
                     "iacc": iacc, "tau_ms": tau},
            "iacc_wide": {"max_delay_ms": 2.5, "lags_ms": wide[0], "iacf": wide[1],
                          "iacc": wide[2], "tau_ms": wide[3]},
        })

    edc_cases = []
    for name, floor in (("reverb", -80.0), ("reverb_delayed", -80.0),
                        ("impulse_right", -80.0), ("silence", -80.0),
                        ("silence", -100.0), ("tiny", -80.0), ("single_left", -80.0),
                        ("noise_left", -60.0)):
        edc_cases.append({"signal": name, "floor_db": floor,
                          "edc": analysis.energy_decay_curve_db(s[name], floor_db=floor)})
    edc_cases.append({"signal": "empty", "floor_db": -80.0,
                      "edc": analysis.energy_decay_curve_db(np.array([]))})
    return bands_cases, pair_cases, edc_cases


def layouts(s):
    """The real Bokeh generators on a synthetic HRIR: labels, NaN filtering, order."""
    from core.hrir import HRIR
    from core.impulse_response import ImpulseResponse

    fs = 48000
    hrir = HRIR(types.SimpleNamespace(fs=fs))
    speakers = {
        "FL": ("reverb", "reverb_delayed"),
        "FR": ("reverb_delayed", "reverb"),
        "FC": ("short_left", "short_right"),   # NaN bands are dropped from the bars
        "BL": ("silence", "silence"),          # no IACC chart, EDC at the floor
        "SR": ("noise_left", "noise_right"),
    }
    for speaker, (left, right) in speakers.items():
        hrir.irs[speaker] = {"left": ImpulseResponse(s[left].copy(), fs),
                             "right": ImpulseResponse(s[right].copy(), fs)}
    out = {"fs": fs, "speakers": [[k, *v] for k, v in speakers.items()]}
    for key, method in (("overlay", "generate_interaural_impulse_overlay_bokeh_layout"),
                        ("ild", "generate_ild_bokeh_layout"),
                        ("ipd", "generate_ipd_bokeh_layout"),
                        ("iacc", "generate_iacc_bokeh_layout"),
                        ("edc", "generate_etc_bokeh_layout")):
        layout = getattr(hrir, method)()
        charts = []
        for fig, _row, _col in sorted(layout.children, key=lambda c: (c[1], c[2])):
            sources = [r.data_source.data for r in fig.renderers]
            legend = [item.label.value for item in fig.legend[0].items]
            chart = {"title": fig.title.text, "x_label": fig.xaxis[0].axis_label,
                     "y_label": fig.yaxis[0].axis_label, "legend": legend}
            if key == "overlay":
                chart["series"] = [{"time_first": float(d["time"][0]),
                                    "time_second": float(d["time"][1]),
                                    "time_last": float(d["time"][-1]),
                                    "length": len(d["time"])} for d in sources]
                chart["x_range"] = [fig.x_range.start, fig.x_range.end]
            elif key in ("ild", "ipd"):
                d = sources[0]
                chart["bands"] = list(d["bands"])
                chart["values"] = list(d["ilds" if key == "ild" else "ipds"])
            elif key == "iacc":
                d = sources[0]
                chart["lags_ms"] = d["lags_ms"]
                chart["iacf"] = d["correlation"]
                chart["x_range"] = [fig.x_range.start, fig.x_range.end]
            else:
                chart["series"] = [{"length": len(d["etc"]), "first": d["etc"][:4],
                                    "last": d["etc"][-4:], "time_last": float(d["time"][-1])}
                                   for d in sources]
                chart["x_range"] = [fig.x_range.start, fig.x_range.end]
                chart["y_range"] = [fig.y_range.start, fig.y_range.end]
            charts.append(chart)
        out[key] = charts
    return out


def overview(fs):
    """HRIR.generate_result_bokeh_figure data for a two-speaker synthetic HRIR."""
    from core.hrir import HRIR
    from core.impulse_response import ImpulseResponse

    rng = np.random.default_rng(1234 + fs)
    n = 600
    t = np.arange(n) / fs
    hrir = HRIR(types.SimpleNamespace(fs=fs))
    inputs = []
    for speaker, onset in (("FL", 30), ("FR", 34)):
        pair = {}
        for side, gain in (("left", 1.0), ("right", 0.6)):
            x = rng.standard_normal(n) * np.exp(-t / 0.004) * 0.1
            x[onset] += gain
            pair[side] = ImpulseResponse(x, fs)
            inputs.append({"speaker": speaker, "side": side, "data": x})
        hrir.irs[speaker] = pair
    fig = hrir.generate_result_bokeh_figure()
    lines = []
    for item in fig.legend[0].items:
        d = item.renderers[0].data_source.data
        column = [k for k in d if k != "freq"][0]
        assert np.array_equal(d["freq"], fig.renderers[0].data_source.data["freq"])
        lines.append({"label": item.label.value, "values": d[column]})
    return {"fs": fs, "treble_f_upper": max(20001, int(fs / 2 - 1)), "inputs": inputs,
            "frequency": fig.renderers[0].data_source.data["freq"], "lines": lines,
            "title": fig.title.text, "x_range": [fig.x_range.start, fig.x_range.end]}


def main():
    s, recipes = signals()
    bands_cases, pair_cases, edc_cases = primitive_cases(s)
    save("signals", {"signals": {k: v for k, v in s.items() if k not in recipes},
                     "derived": recipes})
    save("analysis", {"octave_bands": bands_cases, "pairs": pair_cases, "edc": edc_cases})
    save("layouts", layouts(s))
    for fs in (44100, 48000):
        save(f"overview_{fs}", overview(fs))
    for name, size in WRITTEN:
        print(f"{name}: {size} bytes")


if __name__ == "__main__":
    main()
