"""Export deterministic SECS 2 oracle fixtures (s2_*), one JSON object per fixture.

The oracle is SECS 2 (Sound Equalization and Correction System 2, Beta 1.1.0,
Speaker Gallery, GPL-3.0, https://github.com/Kim-O-O/Sound-Equalization-and-Correction-System-2
at commit ceac0ad), the program virtual room tuning 2 reproduces (ADR 0008).
This script contains no SECS code. It imports the oracle's engine modules,
calls them, and records what they return; intermediate values are observed by
interposing on the module-level functions the engine calls (and on SciPy's
trust-region routine), never by re-implementing the engine. The one deliberate
change is applied in memory, not to the checkout: `design_mimo`'s minimum of
three positions is lowered to two (the BRIR is fixed at the two ears).

    cargo run --release -p impulcifer-dsp --example secs2_inputs -- <inputs>
    NPY_DISABLE_CPU_FEATURES="X86_V3 X86_V4 AVX512_ICL AVX512_SPR" \
        SECS2_SRC=<checkout>/src SECS2_INPUTS=<inputs> python tests/migration/export_goldens_secs2.py

NumPy's runtime SIMD dispatch above the X86-64-v2 baseline must be off (list
the features `numpy.show_runtime()` finds): with AVX-512, NumPy's float64
log10/exp come from SIMD kernels that differ from the C library in the last
bit (12% of log10 values here), enough to move a 150-evaluation trust-region
path, and Rust's f64 functions call the C library. `check_libm` refuses to run
otherwise. The same exporter writes the two end-to-end references.

Encoding as the other exporters (README.md): repr-precision float64 JSON,
complex as [real, imaginary], nonfinite as strings, every file below 200 kB;
longer arrays are headerless little-endian float64 sidecars `s2_*.f64` with
length/sha256/first-32/last-32 descriptors (lists of descriptors when chunked).
Spectra on the 131072-point analysis grid are stored at `s2_bins.json`'s bins.
Implementers of the Rust port must not open or run this script (clean room).
"""
from pathlib import Path
import csv
import hashlib
import json
import os
import platform
import sys
import types

import numpy as np
import scipy
import scipy.optimize
import scipy.signal

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).with_name("goldens")
SRC = Path(os.environ["SECS2_SRC"])
INPUTS = Path(os.environ["SECS2_INPUTS"])
META = {"scipy": scipy.__version__, "numpy": np.__version__, "python": platform.python_version(),
        "oracle": "SECS 2 Beta 1.1.0 (ceac0ad)", "dtype": "float64/complex128",
        "numpy_dispatch": "baseline X86-64-v2 (C library transcendental functions)"}
WRITTEN = []
LIMIT = 200_000
CHUNK = 24_000

# ------------------------------------------------------------------ the oracle
sys.path.insert(0, str(SRC))
from secs import correction_engine as ce  # noqa: E402
from secs import calibration as cal  # noqa: E402
_mimo_src = (SRC / "secs" / "mimo.py").read_text(encoding="utf-8")
assert _mimo_src.count("if len(positions) < 3:") == 1
mimo = types.ModuleType("secs.mimo")
mimo.__file__ = str(SRC / "secs" / "mimo.py")
exec(compile(_mimo_src.replace("if len(positions) < 3:", "if len(positions) < 2:"), mimo.__file__, "exec"),
     mimo.__dict__)
sys.modules["secs.mimo"] = mimo

N = ce.N_FFT
FS = ce.SR
F = ce.freqs()
BINS = sorted(set(range(0, 2731)) | set(range(2731, N // 2 + 1, 32)) | {N // 2})
SPEAKERS = ["FL", "FR", "FC", "BL", "BR", "SL", "SR"]
OMNI_FILE = {"FL": "room-FL,FR", "FR": "room-FL,FR", "FC": "room-FC", "BL": "room-BL,SL", "SL": "room-BL,SL",
             "SR": "room-SR,BR", "BR": "room-SR,BR"}


def plain(value):
    if isinstance(value, np.ndarray):
        return plain(value.tolist())
    if isinstance(value, dict):
        return {str(k): plain(v) for k, v in value.items()}
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
    if isinstance(value, (np.integer,)):
        return int(value)
    if isinstance(value, np.bool_):
        return bool(value)
    return value


def write_bytes(path, data):
    assert len(data) < LIMIT, (path.name, len(data))
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    WRITTEN.append((path.name, len(data)))


def save(name, inputs, outputs):
    payload = plain({"inputs": inputs, "outputs": outputs, "meta": META})
    write_bytes(OUT / f"s2_{name}.json", (json.dumps(payload, allow_nan=False, separators=(",", ":")) + "\n").encode())


def binary(name, values):
    """One sidecar, or a list of sidecars of at most CHUNK values each."""
    values = np.ascontiguousarray(np.asarray(values, dtype="<f8").ravel())
    if values.size > CHUNK:
        return [binary(f"{name}_{i}", values[i * CHUNK:(i + 1) * CHUNK]) for i in range(-(-values.size // CHUNK))]
    data = values.tobytes()
    write_bytes(OUT / f"s2_{name}.f64", data)
    return {"file": f"s2_{name}.f64", "length": int(values.size), "sha256": hashlib.sha256(data).hexdigest(),
            "first": values[:32], "last": values[-32:]}


def grid(name, values):
    """A spectrum on the analysis grid, at BINS: real -> one sidecar, complex -> re/im."""
    v = np.asarray(values)[BINS]
    if np.iscomplexobj(v):
        return {"re": binary(f"{name}_re", v.real), "im": binary(f"{name}_im", v.imag)}
    return binary(name, v)


def load_ir(name):
    return np.fromfile(INPUTS / name, dtype="<f8")


def omni(speaker, side):
    return load_ir(f"omni/{OMNI_FILE[speaker]}-{side}__{speaker}__{side}.f64")


def demo_target():
    rows = list(csv.reader((ROOT / "data/demo/room-target.csv").open()))[1:]
    tf = np.array([float(r[0]) for r in rows])
    td = np.array([float(r[1]) for r in rows])
    return lambda f: np.interp(np.asarray(f, float), tf, td)


def flat(f):
    return np.zeros(np.shape(f))


def mode_params(mode):
    return (dict(ce.MODES[mode]), mode) if isinstance(mode, str) else (cal.ultra_params(mode), f"ultra-{mode:g}")


def make_engine(irs, mains, mode, target=flat, rc_range=(20.0, None), volume=None, weight=1.0, boost=None):
    m, _ = mode_params(mode)
    ce.MAIN_WEIGHT = float(weight)
    ce.MAIN_WEIGHT_LO = float(weight)
    e = ce.CorrectionEngine(irs, mains, [], {"front": list(mains)}, target, xo={}, main_position=min(irs),
                            taps=m["taps"], lift_ms=m["lift_ms"], peq_bands=m.get("peq_bands"),
                            max_latency_ms=m["max_latency_ms"], phase_frac=m.get("phase_frac", 6.0),
                            rc_range=rc_range, room_volume_m3=volume)
    if boost is not None:                     # the oracle's own boost setting caps the engine default
        e.max_boost_db = min(float(e.max_boost_db), float(boost))
    return e


# ------------------------------------------------------------- interposition
class Recorder:
    """Calls observed while a session is open; nested calls of a recorded function are not recorded."""

    def __init__(self):
        self.calls = None
        self.depth = 0

    def open(self):
        self.calls = []

    def close(self):
        calls, self.calls = self.calls, None
        return calls


REC = Recorder()


def interpose(module, name, label):
    original = getattr(module, name)

    def wrapper(*args, **kwargs):
        if REC.calls is None or REC.depth:
            return original(*args, **kwargs)
        REC.depth += 1
        try:
            out = original(*args, **kwargs)
        finally:
            REC.depth -= 1
        REC.calls.append({"label": label, "args": args, "kwargs": kwargs, "out": out})
        return out

    setattr(module, name, wrapper)


for _name in ("smooth_power", "smooth_complex", "minimum_phase", "gd_curve"):
    interpose(ce, _name, _name)

_LS_MODULE = sys.modules["scipy.optimize._lsq.least_squares"]
_ORIGINAL_TRF = _LS_MODULE.trf
_ORIGINAL_LS = scipy.optimize.least_squares
_FIT_LOG = []      # every fit_peq call: inputs, least_squares trace, output
_CURRENT = []


def _trf(fun, jac, x0, f0, J0, *rest, **kwargs):
    trace = _CURRENT[-1]
    trace["J0"] = np.array(J0, dtype=float)
    trace["f0"] = np.array(f0, dtype=float)

    def fun_traced(x):
        f = fun(x)
        trace["main"].append(np.array(x, dtype=float))
        trace["main_cost"].append(0.5 * float(np.dot(f, f)))
        return f

    def jac_traced(x, *a):
        trace["jac"].append(np.array(x, dtype=float))
        return jac(x, *a)

    return _ORIGINAL_TRF(fun_traced, jac_traced, x0, f0, J0, *rest, **kwargs)


_LS_MODULE.trf = _trf


def _least_squares(fun, x0, *args, **kwargs):
    trace = {"x0": np.array(x0, dtype=float), "bounds": kwargs.get("bounds"), "max_nfev": kwargs.get("max_nfev"),
             "main": [], "main_cost": [], "jac": []}
    _CURRENT.append(trace)
    try:
        sol = _ORIGINAL_LS(fun, x0, *args, **kwargs)
    finally:
        _CURRENT.pop()
    trace["result"] = {"x": sol.x, "cost": float(sol.cost), "nfev": int(sol.nfev), "njev": int(sol.njev or 0),
                       "status": int(sol.status), "optimality": float(sol.optimality)}
    _FIT_LOG[-1]["least_squares"] = trace
    return sol


scipy.optimize.least_squares = _least_squares
_ORIGINAL_FIT = ce.fit_peq


def _fit_peq(g, corr_db, n, *args, **kwargs):
    entry = {"g": np.array(g, dtype=float), "corr_db": np.array(corr_db, dtype=float), "n": int(n),
             "args": args, "kwargs": dict(kwargs)}
    _FIT_LOG.append(entry)
    out = _ORIGINAL_FIT(g, corr_db, n, *args, **kwargs)
    entry["bands"] = [list(map(float, b)) for b in out]
    if REC.calls is not None:
        REC.calls.append({"label": "fit_peq", "fit": entry, "out": out})
    return out


ce.fit_peq = _fit_peq

# rfft of an FIR (length = the mode's taps) inside the realization step
_ORIGINAL_RFFT = np.fft.rfft
_TAPS = [0]


def _rfft(a, n=None, *args, **kwargs):
    out = _ORIGINAL_RFFT(a, n, *args, **kwargs)
    if REC.calls is not None and _TAPS[0] and np.ndim(a) == 1 and len(a) == _TAPS[0] and n == N:
        REC.calls.append({"label": "fir", "h": np.array(a, dtype=float)})
    return out


np.fft.rfft = _rfft
_SESSIONS = []
_ORIGINAL_CORRECTION = ce.CorrectionEngine._correction
_ORIGINAL_LONG = ce.CorrectionEngine._long_mode_mask


def _long_mode_mask(self, H):
    mask = _ORIGINAL_LONG(self, H)
    if REC.calls is not None:
        REC.calls.append({"label": "long_mode_mask", "mask": mask})
    return mask


def _correction(self, role, comb, level_db):
    _TAPS[0] = int(self.taps)
    REC.open()
    try:
        out = _ORIGINAL_CORRECTION(self, role, comb, level_db)
    finally:
        calls = REC.close()
    _SESSIONS.append({"role": role, "phase_frac": float(self.phase_frac), "calls": calls, "fir": out[0],
                      "response": out[1], "design_db": self.design_db[role].copy(), "peq": list(self.peq[role]),
                      "strength": float(self.phase_strength[role])})
    return out


ce.CorrectionEngine._long_mode_mask = _long_mode_mask
ce.CorrectionEngine._correction = _correction


# ---------------------------------------------------------------- primitives
def export_primitives():
    f4 = np.fft.rfftfreq(4096, 1.0 / FS)
    save("frac_for", {"f": f4}, {"frac": ce.frac_for(f4)})
    k = np.arange(f4.size, dtype=float)
    power = 1.0 + ((k * 7919.0) % 1000.0) / 1000.0
    signed = ((k * 104729.0) % 2001.0) / 100.0 - 10.0
    for name, values in (("power", power), ("signed", signed)):
        for frac in (1.0, 3.0, 6.0, 24.0, "frac_for"):
            fr = ce.frac_for(f4) if frac == "frac_for" else frac
            save(f"smooth_{name}_{frac}", {"f": f4, "x": values, "frac": frac}, {"y": ce.smooth_power(f4, values, fr)})
    z = power * np.exp(1j * signed / 10.0)
    save("smooth_complex_6", {"f": f4, "x": z, "frac": 6.0}, {"y": ce.smooth_complex(f4, z, 6.0)})
    # minimum phase: smooth, with exact zeros (floored at 1e-8) and with a deep notch
    mags = {"smooth": 1.0 + 0.5 * np.cos(k / 50.0), "zeros": np.where(k % 97 == 0, 0.0, 1.0 + k / k[-1]),
            "notch": 1.0 / (1.0 + ((k - 300.0) / 3.0) ** 2) + 1e-3}
    for name, mag in mags.items():
        save(f"minimum_phase_{name}", {"mag": mag}, {"h": ce.minimum_phase(mag)})
    big = 1.0 + 0.5 * np.cos(np.arange(N // 2 + 1) / 300.0)
    save("minimum_phase_n131072", {"mag": "1+0.5*cos(k/300), k=0..65536"}, {"h": grid("minimum_phase_n131072", ce.minimum_phase(big))})
    gl = np.geomspace(15, 20000, 700)
    bands = [(1000.0, 0.7, 6.0), (60.0, 15.0, -20.0), (19000.0, 2.0, 3.0), (15.0, 0.5, 10.0), (100.0, 40.0, -30.0),
             (440.0, 3.0, 0.0), (23900.0, 0.3, 12.0)]
    for i, (f0, q, gain) in enumerate(bands):
        save(f"peaking_{i}", {"f": gl, "f0": f0, "q": q, "gain_db": gain}, {"h": ce.peaking_response(gl, f0, q, gain)})
    save("peq_response", {"f": gl, "bands": bands}, {"h": ce.peq_response(gl, bands)})
    save("peq_response_grid", {"bands": bands}, {"h": grid("peq_response_grid", ce.peq_response(F, bands))})
    caps = [(f0, gain, ce._ring_q_cap(f0, gain, 30.0)) for f0 in (15.0, 50.0, 200.0, 1000.0) for gain in (-25.0, -3.0, 0.5, 10.0)]
    save("ring_q_cap", {"ring_ms": 30.0}, {"caps": caps})
    boxed = [(60.0, 4.0, 8.0), (120.0, 9.0, -14.0), (300.0, 2.0, 0.6), (20.0, 0.9, -0.6)]
    lb = np.array([v for _ in boxed for v in (np.log(15.0), np.log(0.3), -25.0)])
    ub = np.array([v for _ in boxed for v in (np.log(500.0), np.log(20.0), 10.0)])
    blb, bub = ce._ring_boxes(boxed, lb, ub, 30.0)
    save("ring_boxes", {"bands": boxed, "lb": lb, "ub": ub, "ring_ms": 30.0}, {"lb": blb, "ub": bub})
    # group delay on a 4096 grid of a known IR
    h = np.zeros(4096)
    h[40] = 1.0
    h[41] = -0.5
    h[300] = 0.25
    H = np.fft.rfft(h)
    g = np.geomspace(20, 2000, 300)
    for frac in (6.0, 24.0):
        save(f"gd_curve_{frac:g}", {"ir": h, "g": g, "frac": frac}, {"gd_ms": ce.gd_curve(H, f4, g, frac)})
    # onset and RC window on a synthetic IR
    ir = np.zeros(30000)
    ir[500:700] = np.linspace(0.0, 0.05, 200)
    ir[700] = -1.0
    ir[701:] = 0.3 * np.exp(-np.arange(ir.size - 701) / 2000.0) * np.cos(np.arange(ir.size - 701) / 7.0)
    w, a = ce.windowed(ir, FS, ce.onset_index(ir))
    save("onset_window", {"ir": binary("onset_window_ir", ir)}, {"onset": ce.onset_index(ir), "a": a, "w": binary("onset_window_w", w)})
    # low-rate conversion, late peaks and mode decay at 600 Hz
    f16 = np.fft.rfftfreq(16384, 1.0 / FS)
    t = np.arange(16384) / FS
    lowir = np.exp(-t / 0.25) * (np.sin(2 * np.pi * 47.0 * t) + 0.6 * np.sin(2 * np.pi * 63.5 * t) + 0.3 * np.sin(2 * np.pi * 101.0 * t))
    lowir[:48] = 0.0
    Hl = np.fft.rfft(lowir)
    for n in (300, 720):
        save(f"to_low_{n}", {"ir": binary(f"to_low_{n}_ir", lowir), "n": n, "grid_n": 16384}, {"x": mimo.to_low(Hl, f16, n)})
    tl = np.arange(720) / 600.0
    xs = [np.exp(-tl / 0.6) * np.sin(2 * np.pi * 47.3 * tl) + 0.5 * np.exp(-tl / 0.2) * np.sin(2 * np.pi * 88.0 * tl),
          np.exp(-tl / 0.4) * np.sin(2 * np.pi * 63.1 * tl + 0.3) + 0.4 * np.exp(-tl / 0.9) * np.sin(2 * np.pi * 120.4 * tl)]
    for x in xs:
        x[:5] = 0.0
    save("late_peaks", {"xs": xs, "count": 6}, {"peaks": mimo._late_peaks(xs, 6), "peaks8": mimo._late_peaks(xs[:1], 8)})
    cases = ((47.3, 1.5), (88.0, 1.5), (47.3, 3.0), (130.0, 1.5))
    save("mode_t60", {"x": xs[0], "cases": cases}, {"t60": [mimo.mode_t60(xs[0], 600.0, c, hb) for c, hb in cases]})
    for fsiso in (50.0, 100.0, 120.0, 150.0):
        save(f"firwin_{fsiso:g}", {"taps": 31, "cutoff": fsiso, "fs": 600}, {"h": mimo.lowpass_fir(fsiso)})
    save("hp4", {"f": gl, "fc": 50.0}, {"h": mimo.hp2(gl, 50.0) ** 2})
    for fc in (500.0, 1000.0):
        sos = scipy.signal.butter(4, [fc / np.sqrt(2), fc * np.sqrt(2)], btype="band", fs=FS, output="sos")
        save(f"butter_band_{fc:g}", {"order": 4, "band": [fc / np.sqrt(2), fc * np.sqrt(2)], "fs": FS}, {"sos": sos})
    for c in (47.3, 88.0):
        sos = scipy.signal.butter(2, [max(c - 1.5, 1.0), c + 1.5], btype="bandpass", fs=600.0, output="sos")
        save(f"butter_band_600_{c:g}", {"order": 2, "band": [max(c - 1.5, 1.0), c + 1.5], "fs": 600.0},
             {"sos": sos, "filtered": scipy.signal.sosfilt(sos, xs[0])})
    save("bins", {"n_fft": N}, {"bins": BINS})


def export_fit(name, entry, jacobian=False):
    ls = entry.get("least_squares")
    inputs = {"g": entry["g"], "corr_db": entry["corr_db"], "n": entry["n"],
              "kwargs": {k: (None if v is None else float(v)) for k, v in entry["kwargs"].items()}}
    outputs = {"bands": entry["bands"]}
    if ls is not None:
        lb, ub = ls["bounds"]
        outputs.update({
            "x0": ls["x0"], "lb": lb, "ub": ub, "max_nfev": ls["max_nfev"], "f0_cost": 0.5 * float(ls["f0"] @ ls["f0"]),
            "J0": binary(f"fit_{name}_J0", ls["J0"]) if jacobian else None, "J0_shape": list(ls["J0"].shape),
            "main_x": binary(f"fit_{name}_main", np.array(ls["main"]) if ls["main"] else np.zeros(0)),
            "main_cost": ls["main_cost"],
            "jac_x": binary(f"fit_{name}_jac", np.array(ls["jac"]) if ls["jac"] else np.zeros(0)),
            "params": len(ls["x0"]), "result": ls["result"]})
    save(f"fit_{name}", inputs, outputs)


def export_synthetic_fits():
    gl = np.geomspace(15, 20000, 700)
    lg = np.log2(gl)
    curves = {
        "bumps": 8 * np.exp(-((lg - np.log2(50)) / 0.15) ** 2) - 12 * np.exp(-((lg - np.log2(120)) / 0.08) ** 2)
                 + 5 * np.exp(-((lg - np.log2(300)) / 0.3) ** 2),
        "ripple": 6 * np.sin(lg * 3.0) * np.exp(-((lg - np.log2(100)) / 1.5) ** 2),
        "deep": -24 * np.exp(-((lg - np.log2(70)) / 0.05) ** 2) + 9 * np.exp(-((lg - np.log2(35)) / 0.2) ** 2),
    }
    for name, curve in curves.items():
        _FIT_LOG.clear()
        ce.fit_peq(gl, curve, 16, boost=10.0, ring_ms=30.0, f_max=500.0)
        export_fit(f"synthetic_{name}", _FIT_LOG[-1], jacobian=True)
    _FIT_LOG.clear()
    ce.fit_peq(gl, curves["bumps"], 16, boost=6.0, ring_ms=30.0, f_max=177.0)
    export_fit("synthetic_bumps_low_fmax", _FIT_LOG[-1], jacobian=True)
    _FIT_LOG.clear()
    ce.fit_peq(np.geomspace(15, 20000, 900), np.where(np.geomspace(15, 20000, 900) <= 500, np.interp(np.geomspace(15, 20000, 900), gl, curves["bumps"]), 0.0),
               16, lo=15.0, hi=400.0, boost=10.0, ring_ms=30.0)
    export_fit("synthetic_zero_low_part", _FIT_LOG[-1], jacobian=True)


# --------------------------------------------------------------------- stages
def position(calls, call):
    return next(i for i, c in enumerate(calls) if c is call)


def session_calls(session, label):
    return [c for c in session["calls"] if c["label"] == label]


def export_session(name, session):
    calls = session["calls"]
    fits = session_calls(session, "fit_peq")
    for i, c in enumerate(fits):
        export_fit(f"{name}_{i}", c["fit"])
    smooth = session_calls(session, "smooth_power")
    mp = session_calls(session, "minimum_phase")
    sc = session_calls(session, "smooth_complex")
    long_mask = session_calls(session, "long_mode_mask")
    firs = session_calls(session, "fir")
    outputs = {
        "role": session["role"], "phase_frac": session["phase_frac"], "strength": session["strength"],
        "peq": session["peq"], "fits": len(fits),
        "power": grid(f"{name}_power", smooth[0]["args"][1]),
        "smoothed_variable": grid(f"{name}_sm", smooth[0]["out"]),
        "smoothed_third": grid(f"{name}_sm3", smooth[1]["out"]),
        "design_db": grid(f"{name}_design", session["design_db"]),
        "long_mode_bins": np.nonzero(long_mask[0]["mask"])[0] if long_mask else [],
        "response": grid(f"{name}_response", session["response"]),
        "firs": [binary(f"{name}_fir{i}", c["h"]) for i, c in enumerate(firs)],
        "fir": binary(f"{name}_fir", np.asarray(session["fir"], dtype=float)),
    }
    if len(fits) and session["fir"].size > 1:
        first_fit = position(calls, fits[0])
        after = [c for c in calls[first_fit:] if c["label"] == "minimum_phase"]
        outputs["M"] = grid(f"{name}_M", after[0]["out"])
        phase = [c for c in calls[first_fit:] if c["label"] == "smooth_complex"]
        outputs["V"] = grid(f"{name}_V", phase[0]["args"][1])
        outputs["V_smoothed"] = grid(f"{name}_Vs", phase[0]["out"])
        k = position(calls, phase[0])
        tau = next(c for c in calls[k + 1:] if c["label"] == "smooth_power")
        outputs["tau"] = grid(f"{name}_tau", tau["args"][1])
        outputs["tau_smoothed"] = grid(f"{name}_tau_s", tau["out"])
        second = position(calls, fits[-1])
        touch = [c for c in calls[second:] if c["label"] == "minimum_phase"]
        outputs["touchup_M"] = [grid(f"{name}_M2_{i}", c["out"]) for i, c in enumerate(touch)]
    del mp, sc
    save(f"rc_{name}", {}, outputs)


RC_CASES = [
    ("fl_both_mixed", "FL", ["left", "right"], "mixed", {}),
    ("fl_left_mixed", "FL", ["left"], "mixed", {}),
    ("sl_both_linear", "SL", ["left", "right"], "linear", {}),
    ("bl_both_zero", "BL", ["left", "right"], "zero", {}),
    ("fc_both_long", "FC", ["left", "right"], "long", {}),
    ("sr_both_ultra60", "SR", ["left", "right"], 60.0, {}),
    ("fl_both_mixed_main5", "FL", ["left", "right"], "mixed", {"weight": 5.0}),
    ("fl_both_mixed_target_volume40", "FL", ["left", "right"], "mixed", {"target": "demo", "volume": 40.0}),
    ("br_both_mixed_band30_300", "BR", ["left", "right"], "mixed", {"rc_range": (30.0, 300.0)}),
]


def export_rc():
    for name, speaker, sides, mode, extra in RC_CASES:
        irs = {p + 1: {speaker: (omni(speaker, side), FS, 0.0)} for p, side in enumerate(sides)}
        target = demo_target() if extra.get("target") == "demo" else flat
        e = make_engine(irs, [speaker], mode, target=target, rc_range=extra.get("rc_range", (20.0, None)),
                        volume=extra.get("volume"), weight=extra.get("weight", 1.0))
        _SESSIONS.clear()
        res = e.compute()
        for i, session in enumerate(_SESSIONS):
            export_session(f"{name}_s{i}", session)
        _, mname = mode_params(mode)
        save(f"rc_{name}", {"speaker": speaker, "sides": sides, "mode": mname, "main_weight": extra.get("weight", 1.0),
                            "target": extra.get("target", "flat"), "rc_range": list(extra.get("rc_range", (20.0, None))),
                            "room_volume_m3": extra.get("volume")},
             {"sessions": [f"rc_{name}_s{i}" for i in range(len(_SESSIONS))], "rc_hi": float(e.rc_hi),
              "schroeder_hz": e.schroeder_hz(), "level_db": float(e._band_level(speaker, 300, 3000)),
              "phase_frac_used": float(getattr(e, "phase_frac_used", {}).get(speaker, e.phase_frac)),
              "strength": float(e.phase_strength[speaker]), "peq": [list(map(float, b)) for b in res.channels[speaker].peq],
              "response": grid(f"rc_{name}_final", e.chain[speaker]["F"]), "notes": [str(n) for n in res.notes]})
    # channel levels of all seven speakers, two positions, equal weights and SECS's weight 5
    for weight in (1.0, 5.0):
        irs = {p + 1: {s: (omni(s, side), FS, 0.0) for s in SPEAKERS} for p, side in enumerate(["left", "right"])}
        e = make_engine(irs, SPEAKERS, "mixed", weight=weight)
        e._prepare()
        save(f"levels_w{weight:g}", {"speakers": SPEAKERS, "sides": ["left", "right"], "main_weight": weight},
             {"level_db": {s: float(e._band_level(s, 300, 3000)) for s in SPEAKERS}})


# ----------------------------------------------------------------------- MIMO
def export_mimo():
    irs = {1: {s: (load_ir(f"brir/{s}__left.f64"), FS, 0.0) for s in SPEAKERS},
           2: {s: (load_ir(f"brir/{s}__right.f64"), FS, 0.0) for s in SPEAKERS}}
    for name, mode, fsiso, f_low in (("mixed_fsiso150_auto", "mixed", 150.0, None), ("mixed_fsiso100_low40", "mixed", 100.0, 40.0)):
        e = make_engine(irs, SPEAKERS, mode)
        _SESSIONS.clear()
        res = e.compute()
        e.align_delay_ms = {r: 0.0 for r in e.align_delay_ms}
        log = {"to_low": [], "conv": [], "vstack": [], "solve": [], "band": [], "tail": []}
        originals = {}

        def wrap(module, attr, key, record):
            originals[(module, attr)] = getattr(module, attr)
            original = originals[(module, attr)]

            def wrapper(*args, **kwargs):
                out = original(*args, **kwargs)
                log[key].append(record(args, kwargs, out))
                return out
            setattr(module, attr, wrapper)

        wrap(mimo, "to_low", "to_low", lambda a, k, o: (a[2], np.array(o)))
        wrap(mimo, "conv_matrix", "conv", lambda a, k, o: np.array(a[0]))
        wrap(np, "vstack", "vstack", lambda a, k, o: o)
        wrap(np.linalg, "solve", "solve", lambda a, k, o: (np.array(a[0]).diagonal().copy(), np.array(a[1]), np.array(o)))
        wrap(mimo, "_worst_band_db", "band", lambda a, k, o: float(o))
        wrap(mimo, "worst_t60_change", "tail", lambda a, k, o: float(o))
        per_input = {}
        try:
            design = None
            for i in e.chain:
                for key in log:
                    log[key].clear()
                design_i = mimo.design_mimo(e, fsiso=fsiso, positions=[1, 2], f_low=f_low, inputs=[i])
                design = design_i
                if not log["vstack"]:
                    per_input[i] = {"supports": {}, "level_db": design_i.level_db.get(i)}
                    continue
                A = log["vstack"][0]
                AtA = A.T @ A
                cands = list(design_i.supports.get(i, {}) or [r for r, v in design_i.ranges.items() if v and r != i])
                R = A.shape[1] // mimo.TAPS_LOW
                scale = float(np.mean(np.diag(AtA)))
                lam = [float(d[0] - AtA[0, 0]) for d, _rhs, _u in log["solve"]]
                p = [float(np.log10(v / scale)) for v in lam]
                lp = mimo.lowpass_fir(fsiso)
                direct = float(np.sum(mimo.to_low(e.chain[i]["F"], e.f, int(mimo.LEN_S * mimo.FS_LOW)) ** 2))
                ratios = []
                for _d, _rhs, u in log["solve"]:
                    sup = sum(float(np.sum(np.convolve(lp, u[k * mimo.TAPS_LOW:(k + 1) * mimo.TAPS_LOW]) ** 2)) for k in range(R))
                    ratios.append(sup / max(direct, 1e-30))
                kept = 0                                  # the bisection keeps the solution of every step that lowered hi
                for step in range(1, len(ratios)):
                    if ratios[step] <= 10 ** (mimo.LEVEL_DB / 10):
                        kept = step
                n = int(mimo.LEN_S * mimo.FS_LOW)
                lows = [x for (nn, x) in log["to_low"] if nn == n]
                x_pos = [lows[0], lows[1 + R]]           # per position: input, then R support paths
                g_pos = [lows[1:1 + R], lows[2 + R:2 + 2 * R]]
                tag = f"mimo_{name}_{i}"
                rows = np.array([AtA[k * mimo.TAPS_LOW] for k in range(R)])
                per_input[i] = {
                    "candidates": R, "x": [binary(f"{tag}_x{q}", x_pos[q]) for q in range(2)],
                    "g": [binary(f"{tag}_g{q}", np.array(g_pos[q])) for q in range(2)],
                    "lp": lp, "direct": direct,
                    "AtA_diag": binary(f"{tag}_diag", np.diag(AtA)), "AtA_block_first_rows": binary(f"{tag}_rows", rows),
                    "AtA_frobenius": float(np.linalg.norm(AtA)), "AtA_trace": float(np.trace(AtA)), "scale": scale,
                    "Atb": binary(f"{tag}_atb", log["solve"][0][1]), "log10_lambda_over_scale": p, "ratios": ratios,
                    "kept_solve": kept, "u": binary(f"{tag}_u", log["solve"][kept][2]),
                    "guard_band_db": list(log["band"]), "guard_tail": list(log["tail"]),
                    "supports": {j: s for j, s in design_i.supports.get(i, {}).items()},
                    "level_db": design_i.level_db.get(i)}
        finally:
            for (module, attr), original in originals.items():
                setattr(module, attr, original)
        save(f"mimo_{name}", {"brirs": "examples/secs2_inputs brir/*", "rc_mode": mode, "fsiso": fsiso, "f_low": f_low,
                              "positions": ["left", "right"], "main_weight": 1.0},
             {"ranges": {r: (list(v) if v else None) for r, v in design.ranges.items()},
              "gain_db": {s: float(res.channels[s].gain_db) for s in SPEAKERS}, "inputs": list(per_input),
              "rc_strength": {s: float(e.phase_strength[s]) for s in SPEAKERS}})
        for i, data in per_input.items():
            save(f"mimo_{name}_{i}", {"input": i}, data)


# --------------------------------------------------------------- end to end
def r10(values):
    return [float(f"{v:.10g}") for v in np.asarray(values, float)]


E2E_BINS = sorted(set(int(round(x / (FS / N))) for x in np.concatenate([
    15 * 2 ** (np.arange(0, 48 * np.log2(1000 / 15)) / 48), 1000 * 2 ** (np.arange(0, 6 * np.log2(23000 / 1000) + 1) / 6)])
    if x <= 23000))


def write_reference(name, payload):
    """The two end-to-end references are integration checks, not fixtures; they may exceed 200 kB."""
    data = (json.dumps(plain(payload), allow_nan=False, separators=(",", ":")) + "\n").encode()
    path = OUT / name
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    print(f"wrote {name}, {len(data)} bytes")


PROVENANCE = {
    "tool": "Sound Equalization and Correction System 2 (SECS 2) Beta 1.1.0, Speaker Gallery (DCinside), GPL-3.0",
    "source": "https://github.com/Kim-O-O/Sound-Equalization-and-Correction-System-2 (commit ceac0ad)",
    "exporter": "tests/migration/export_goldens_secs2.py (no SECS code; calls the oracle and records its results)",
    "numpy_dispatch": "SIMD above the X86-64-v2 baseline disabled: the oracle's float64 transcendental functions are the C library's",
}


def export_end_to_end_rc():
    lift_of = {}
    entries = {}

    def run(speakers, sides, mode, weight=1.0, target="flat", rc_range=(20.0, None), volume=None, boost=None):
        _, mname = mode_params(mode)
        irs = {p + 1: {s: (omni(s, side), FS, 0.0) for s in speakers} for p, side in enumerate(sides)}
        e = make_engine(irs, speakers, mode, target=flat if target == "flat" else demo_target(), rc_range=rc_range,
                        volume=volume, weight=weight, boost=boost)
        _FIT_LOG.clear()
        res = e.compute()
        lift = float(e.lift_ms) / 1000.0
        lift_of[mname] = lift
        fb = np.array(E2E_BINS) * (FS / N)
        out = {"mode": mname, "speakers": speakers, "sides": sides, "main_weight": weight, "target": target,
               "rc_lo": rc_range[0], "rc_hi_setting": rc_range[1], "room_volume_m3": volume,
               "max_boost_db": float(e.max_boost_db), "rc_hi": float(e.rc_hi),
               "schroeder_hz": None if volume is None else e.schroeder_hz(), "channels": {}}
        for s in speakers:
            Fs = e.chain[s]["F"][E2E_BINS]
            ch = res.channels[s]
            out["channels"][s] = {
                "gain_db": float(ch.gain_db), "level_db": float(e._band_level(s, 300, 3000)),
                "peq": [list(map(float, b)) for b in ch.peq], "phase_strength": float(e.phase_strength[s]),
                "phase_frac_used": float(getattr(e, "phase_frac_used", {}).get(s, e.phase_frac)),
                "fir_taps": int(np.asarray(ch.fir).size) if np.asarray(ch.fir).size > 1 else 0,
                "mag_db": r10(20 * np.log10(np.abs(Fs) + 1e-12)),
                "phase_rad": r10(np.angle(Fs * np.exp(2j * np.pi * fb * lift)))}
        return out

    for s in SPEAKERS:
        for side in ("left", "right"):
            entries[f"single/{s}/{side}/mixed"] = run([s], [side], "mixed")
    for mode in ("zero", "mixed", "linear", "long", 60.0):
        for s in SPEAKERS:
            entries[f"two/{s}/{mode_params(mode)[1]}"] = run([s], ["left", "right"], mode)
    for s in SPEAKERS:
        entries[f"two-main5/{s}/mixed"] = run([s], ["left", "right"], "mixed", weight=5.0)
    entries["two/FL/mixed/target"] = run(["FL"], ["left", "right"], "mixed", target="demo")
    entries["two/SL/linear/target"] = run(["SL"], ["left", "right"], "linear", target="demo")
    entries["two/FL/mixed/volume40"] = run(["FL"], ["left", "right"], "mixed", volume=40.0)
    entries["two/BR/mixed/band30-300"] = run(["BR"], ["left", "right"], "mixed", rc_range=(30.0, 300.0))
    entries["two/FC/mixed/boost6"] = run(["FC"], ["left", "right"], "mixed", boost=6.0)
    entries["all/mixed"] = run(SPEAKERS, ["left", "right"], "mixed")
    write_reference("room_tuning_secs2_rc.json", {
        "provenance": {**PROVENANCE, "configuration": (
            "mains = the listed speakers, no subwoofers, no crossover; positions = ear positions in order (position 1 "
            "is SECS's listening position); main_weight sets SECS MAIN_WEIGHT and MAIN_WEIGHT_LO; modes from SECS "
            "MODES, ultra-<ms> from its ultra mode; target flat (0 dB) or the demo room-target.csv interpolated "
            "linearly in Hz with end values held. In all/mixed the oracle places every speaker on one time origin "
            "(the earliest onset at position 1 over all seven); its responses therefore are not the per-speaker "
            "responses and only its gains and levels are a reference.")},
        "ir": {"estimator": "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav", "files": "data/demo/room-<speakers>-<side>.wav",
               "construction": "examples/secs2_inputs omni/*: open_recording_samples(estimator, fs, tracks, speakers, side, 2.0), uncropped (391270 samples)"},
        "n_fft": N, "bins": E2E_BINS, "lift_s": lift_of,
        "fields": {"mag_db": "20*log10(|F|+1e-12) of the channel correction (FIR and PEQ) at bins, 10 significant digits",
                   "phase_rad": "angle of F*exp(+j*2*pi*f*lift) at bins", "fir_taps": "0 = no FIR (zero mode)"},
        "entries": entries})


def export_end_to_end_mimo():
    irs = {1: {s: (load_ir(f"brir/{s}__left.f64"), FS, 0.0) for s in SPEAKERS},
           2: {s: (load_ir(f"brir/{s}__right.f64"), FS, 0.0) for s in SPEAKERS}}
    entries = {}
    for name, mode, fsiso, f_low in (("mixed/fsiso150/auto", "mixed", 150.0, None), ("mixed/fsiso100/low40", "mixed", 100.0, 40.0),
                                     ("linear/fsiso150/auto", "linear", 150.0, None)):
        e = make_engine(irs, SPEAKERS, mode)
        res = e.compute()
        e.align_delay_ms = {r: 0.0 for r in e.align_delay_ms}
        d = mimo.design_mimo(e, fsiso=fsiso, positions=[1, 2], f_low=f_low)
        H = e.long_spectra()
        n = int(mimo.LEN_S * mimo.FS_LOW)
        late = {}
        for i in e.chain:
            sup = d.supports.get(i, {})
            if not sup:
                late[i] = {"worst_band_db": None, "tail_change": None}
                continue
            xs, ys = zip(*(mimo.apply_mimo(e, d, i, H, p, n) for p in (1, 2)))
            gs = [{j: mimo.to_low(mimo.support_path(e, d, j, H, p), e.f, n) for j in sup} for p in (1, 2)]
            late[i] = {"worst_band_db": float(mimo._worst_band_db(list(xs), gs, sup, fsiso, n, None)),
                       "tail_change": float(mimo.worst_t60_change(list(xs), list(ys)))}
        entries[name] = {
            "rc_mode": mode, "fsiso": fsiso, "f_low": f_low,
            "gain_db": {s: float(res.channels[s].gain_db) for s in SPEAKERS},
            "ranges": {r: (list(v) if v else None) for r, v in d.ranges.items()},
            "level_db": {i: float(v) for i, v in d.level_db.items()},
            "supports": {i: {j: list(map(float, s)) for j, s in v.items()} for i, v in d.supports.items()},
            "late": late, "notes": [str(x) for x in d.notes]}
    write_reference("room_tuning_secs2_mimo.json", {
        "provenance": {**PROVENANCE, "configuration": (
            "positions = left ear, right ear of the aligned in-ear BRIRs (t0 = 0 for all, common time base); mains = "
            "all seven speakers, no subwoofers; MAIN_WEIGHT = MAIN_WEIGHT_LO = 1 (position 1 still drives the "
            "listening-position-only steps); SECS arrival delays set to zero after compute(); design_mimo's "
            "minimum-position rule lowered from 3 to 2 in memory")},
        "ir": {"estimator": "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav", "files": "data/demo/FL,FR.wav, FC.wav, BL,SL.wav, SR,BR.wav",
               "construction": "examples/secs2_inputs brir/*: open_recording_samples(…, None, 2.0) per file in that order, crop_heads(1.0), align_ipsilateral_all(IPSILATERAL_PAIRS, 30.0), align_onset_groups_peak_leftref(None), crop_tails(estimator) (24000 samples)"},
        "fields": {"supports": "input -> support speaker -> 600 Hz FIR (support FIR convolved with the Fsiso low-pass, after the guard)",
                   "level_db": "support level relative to the correction filter energy", "ranges": "support band (low, high) Hz or null",
                   "late": "the oracle's guard measures at the accepted scale; tail_change is its scaled form (dB x 0.15)"},
        "entries": entries})


def check_libm():
    """The oracle's float64 transcendental functions must be the C library's (NPY_DISABLE_CPU_FEATURES)."""
    import math
    x = np.linspace(1e-3, 1e3, 20001)
    assert all(float(a) == math.log10(float(b)) for a, b in zip(np.log10(x), x)), (
        "NumPy dispatches SIMD log10; run with NPY_DISABLE_CPU_FEATURES set to the found features above X86_V2")
    y = np.linspace(-50.0, 50.0, 20001)
    assert all(float(a) == math.exp(float(b)) for a, b in zip(np.exp(y), y))


def main():
    check_libm()
    OUT.mkdir(parents=True, exist_ok=True)
    export_end_to_end_rc()
    export_end_to_end_mimo()
    export_primitives()
    export_synthetic_fits()
    export_rc()
    export_mimo()
    total = sum(size for _name, size in WRITTEN)
    print(f"wrote {len(WRITTEN)} s2 files, {total} bytes")
    stale = sorted(p.name for p in OUT.glob("s2_*") if p.name not in {n for n, _s in WRITTEN})
    assert not stale, stale


if __name__ == "__main__":
    main()
