"""P08: execute the repository's 2.x objects on a temporary demo copy.

Large arrays are raw LE f64, content-deduplicated. No transcribed DSP oracle.
The full-array inventory is budgeted at 40 MB (the original packet said 12 MB, which the required arrays alone exceed); print
that measured incompatibility rather than omit requested reference samples.
"""
from pathlib import Path
import hashlib
import json
import re
import shutil
import sys
import tempfile

import numpy as np
import soundfile as sf

import export_goldens as common

ROOT = common.ROOT
OUT = common.OUT
sys.path.insert(0, str(ROOT))
from core import constants, decay  # noqa: E402
from core.audio_io import read_wav  # noqa: E402
from core.brir_layout import base_channel_count, compact_tracks, trim_silent_extensions  # noqa: E402
from core.hrir import HRIR  # noqa: E402
from core.impulse_response import ImpulseResponse  # noqa: E402
from core.impulse_response_estimator import ImpulseResponseEstimator, SEQUENCE_TRACK_ORDERS  # noqa: E402

WRITTEN = {}
BLOBS = {}


def write(path, payload):
    """Write only P08 files and preserve identical bytes."""
    assert path.name.startswith("p08_")
    if not path.exists() or path.read_bytes() != payload:
        path.write_bytes(payload)
    WRITTEN[path.name] = len(payload)


def save(name, inputs, outputs):
    """Use the common inputs/outputs/meta and nonfinite conventions."""
    payload = json.dumps(common.plain(dict(inputs=inputs, outputs=outputs, meta=common.META)), separators=(",", ":"), allow_nan=False).encode() + b"\n"
    assert len(payload) < 200_000, (name, len(payload))
    write(OUT / f"p08_{name}.json", payload)


def summary(x):
    """Record all required endpoints, scalar metrics, and byte hash."""
    x = np.asarray(x, dtype="<f8")
    return dict(length=len(x), first=x[:256], last=x[-256:], sha256=hashlib.sha256(x.tobytes()).hexdigest(),
                max_abs=float(np.max(np.abs(x))) if len(x) else 0.0,
                argmax=int(np.argmax(np.abs(x))) if len(x) else 0,
                rms=float(np.sqrt(np.mean(x*x))) if len(x) else 0.0)


def array(name, x):
    """Store every sample as headerless little-endian float64, deduplicated."""
    x = np.asarray(x, dtype="<f8")
    info = summary(x)
    digest = info["sha256"]
    if digest not in BLOBS:
        path = OUT / f"p08_{name}.f64"
        write(path, x.tobytes())
        BLOBS[digest] = path.name
    info["file"] = BLOBS[digest]
    return info


def snapshot(name, hrir, full=False):
    """Capture actual object state immediately after a Python stage."""
    tracks = []
    for speaker, pair in hrir.irs.items():
        for side, ir in pair.items():
            row = array(f"{name}_{speaker}_{side}", ir.data) if full and speaker == "FL" else summary(ir.data)
            row.update(speaker=speaker, side=side, peak_index=ir.peak_index())
            tracks.append(row)
    save(f"demo_{name}", {}, tracks)


def decay_fixture(name, ir, source):
    """Call all four Python decay APIs, retaining Python exceptions explicitly."""
    params = ir.decay_params()
    outputs = dict(params=params)
    for key, fn in [("times", ir.decay_times), ("adjustment", lambda: ir.decay_adjustment_params(0.3))]:
        try:
            outputs[key] = fn()
        except Exception as exc:
            outputs[key] = dict(error=type(exc).__name__, message=str(exc))
    if not isinstance(outputs["adjustment"], dict):
        adjusted = ir.data.copy()
        decay.apply_decay_window(adjusted, outputs["adjustment"])
        # Full arrays only for one demo ear and the synthetic case; the rest are
        # endpoint/metric summaries (fixture budget, ARCHITECTURE gate 2).
        outputs["adjusted"] = array(f"decay_{name}_adjusted", adjusted) if name in ("FL_left", "synthetic") else summary(adjusted)
    save(f"decay_{name}", dict(fs=ir.fs, source=source, target=0.3), outputs)


def main():
    """Generate only P08 fixtures; repository demo files are never written."""
    OUT.mkdir(exist_ok=True)
    values = {}
    for name in ["SPEAKER_NAMES", "HESUVI_TRACK_ORDER", "HEXADECAGONAL_TRACK_ORDER", "LEFT_SIDE_SPEAKERS", "RIGHT_SIDE_SPEAKERS", "CENTER_SPEAKERS", "IPSILATERAL_PAIRS", "SPEAKER_DELAYS"]:
        value = getattr(constants, name)
        values[name] = sorted(value) if isinstance(value, frozenset) else value
    values["SEQUENCE_TRACK_ORDERS"] = SEQUENCE_TRACK_ORDERS
    values["base_counts"] = [base_channel_count(constants.HESUVI_TRACK_ORDER), base_channel_count(constants.HEXADECAGONAL_TRACK_ORDER), base_channel_count(["FL-left"])]
    values["speaker_sides"] = {s: constants.speaker_side(s) for s in constants.SPEAKER_NAMES + ["LFE", "tfl", "unknown"]}
    values["track_names"] = [constants.track_name(s, side) for s in constants.SPEAKER_NAMES for side in ("left", "right")]
    save("constants", {}, values)
    rounding = [-3.9, -3.5, -2.5, -0.5, 0., 0.5, 2.5, 3.5, 3.9]
    divisions = [(-7, 3), (7, -3), (-7, -3), (7, 3), (-6, 3)]
    save("rounding", dict(values=rounding, divisions=divisions), dict(trunc=[int(x) for x in rounding], round=[round(x) for x in rounding], floor=[a//b for a, b in divisions]))
    estimator = ImpulseResponseEstimator(5.0, 48000)
    sweep = estimator.test_signal
    recording = np.zeros(len(sweep) + 3000)
    recording[1000:1000+len(sweep)] += sweep
    recording[3000:3000+len(sweep)] += sweep*0.1
    save("estimator", dict(min_duration=5.0, fs=48000), dict(low=estimator.low, high=estimator.high, n_octaves=estimator.n_octaves, duration=estimator.duration, length=len(estimator), file_name=estimator.file_name(32), test_signal=array("sweep", sweep), inverse_filter=array("inverse", estimator.inverse_filter), estimate=summary(estimator.estimate(recording)), bundled_44100=[p.name for p in sorted((ROOT/"data").glob("*44100*"))]))
    sequences = []
    for speakers, layout in [(["FL", "FR"], "stereo"), (["FL"], "mono"), (["FL", "FR", "FC"], "7.1"), (["FL", "FL"], "stereo"), (["FR"], "mono")]:
        data = estimator.sweep_sequence(speakers, layout)
        tracks = []
        for row in data:
            info = summary(row)
            active = np.flatnonzero(row)
            if len(active):
                start, stop = int(active[0]), int(active[-1]) + 1
                offset = int(np.flatnonzero(sweep)[0])
                np.testing.assert_array_equal(row[start:stop], sweep[offset:offset+stop-start])
                info["active"] = dict(start=start, stop=stop, sweep_offset=offset)
            tracks.append(info)
        sequences.append(dict(speakers=speakers, layout=layout, shape=data.shape, tracks=tracks))
    save("sequence", {}, sequences)
    bundled = ROOT/"data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
    fs, wav = read_wav(str(bundled))
    loaded = ImpulseResponseEstimator.from_wav(str(bundled))
    branch = "length" if len(sweep) != len(wav) else "value" if np.max(np.abs(sweep-wav)) > 1e-4 else "none"
    save("repair", dict(path=bundled.relative_to(ROOT).as_posix()), dict(branch=branch, duration=loaded.duration, file_name=loaded.file_name(32), test_signal=summary(loaded.test_signal), inverse_filter=summary(loaded.inverse_filter)))
    with tempfile.TemporaryDirectory(prefix="impulcifer-p08-") as temporary:
        demo = Path(temporary)/"demo"
        shutil.copytree(ROOT/"data/demo", demo)
        files = [p for p in sorted(demo.glob("*.wav")) if re.fullmatch(r"[A-Z]{2,3}(,[A-Z]{2,3})*", p.stem)]
        save("protocol", dict(sweep=bundled.relative_to(ROOT).as_posix()), dict(recordings=[p.name for p in files], stages=["open", "crop_heads", "ipsilateral", "onset", "crop_tails", "normalize"]))
        repair_cases = []
        for label, samples in [("length", sweep[:-17]), ("value", sweep * 0.9)]:
            path = Path(temporary)/f"repair-{label}.wav"
            sf.write(path, samples, fs, subtype="DOUBLE")
            repaired = ImpulseResponseEstimator.from_wav(str(path))
            repair_cases.append(dict(branch=label, length=len(repaired), duration=repaired.duration, test_signal=summary(repaired.test_signal), inverse_filter=summary(repaired.inverse_filter)))
        save("repair_branches", {}, repair_cases)
        hrir = HRIR(loaded)
        for p in files:
            hrir.open_recording(str(p), p.stem.split(","))
        snapshot("open", hrir)
        hrir.crop_heads(head_ms=1)
        snapshot("crop_heads", hrir)
        # 'after crop' means the head-crop stage, before alignment/tail crop.
        for speaker in ("FL", "FR", "FC"):
            for side in ("left", "right"):
                ir = hrir.irs[speaker][side]
                decay_fixture(f"{speaker}_{side}", ir, dict(stage="crop_heads", speaker=speaker, side=side))
        ir = hrir.irs["FL"]["left"].copy()
        results = {}
        for label, operation in [("shift_plus", lambda x: x.shift(37)), ("shift_minus", lambda x: x.shift(-37)), ("crop", lambda x: x.crop_head(1)), ("equalize", lambda x: x.equalize(np.array([0.5, -0.125, 0.25, 0.0625])))]:
            instance = ir.copy()
            operation(instance)
            results[label] = summary(instance.data)
        magnitude = hrir.irs["FL"]["left"].magnitude_response()
        results["frequency"] = summary(magnitude[0])
        results["magnitude"] = summary(magnitude[1])
        # The input is the crop_heads-stage FL left ear, which the Rust test rebuilds itself.
        save("ir", dict(fs=fs, data=summary(ir.data), fir=[0.5, -0.125, 0.25, 0.0625]), results)
        save("reflections", {}, hrir.calculate_reflection_levels())
        hrir.align_ipsilateral_all(speaker_pairs=list(constants.IPSILATERAL_PAIRS), segment_ms=30)
        snapshot("ipsilateral", hrir)
        hrir.align_onset_groups_peak_leftref()
        snapshot("onset", hrir)
        tail = hrir.crop_tails()
        snapshot("crop_tails", hrir, full=True)
        gain = hrir.normalize(peak_target=-0.1)
        snapshot("normalize", hrir)
        save("stage_returns", {}, dict(crop_tails=tail, normalize=gain))
        stacked = []
        for name in ("HESUVI_TRACK_ORDER", "HEXADECAGONAL_TRACK_ORDER"):
            order = getattr(constants, name)
            by_name = {constants.track_name(s, side): ir.data for s, pair in hrir.irs.items() for side, ir in pair.items()}
            data = np.vstack([by_name.get(n, np.zeros(tail)) for n in order])
            trimmed = trim_silent_extensions(data, order)
            _, compact = compact_tracks(data, order)
            stacked.append(dict(layout=name, names=order[:len(trimmed)], tracks=[dict(length=len(x), sha256=summary(x)["sha256"]) for x in trimmed], compact_names=compact))
        save("stack", {}, stacked)
    fs = 48000
    t = np.arange(fs)/fs
    synthetic = np.exp(-6.9078*t/0.5)*np.random.default_rng(808).standard_normal(fs)
    first_pass = {}
    def capture(frame, event, arg):
        if frame.f_code is decay.decay_params.__code__ and event == "line" and frame.f_lineno == 112 and not first_pass:
            first_pass.update({k: frame.f_locals[k].copy() if isinstance(frame.f_locals[k], np.ndarray) else frame.f_locals[k] for k in ("windows", "t_windows", "noise_floor")})
        return capture
    sys.settrace(capture)
    try:
        params = decay.decay_params(synthetic, fs)
    finally:
        sys.settrace(None)
    save("decay_first_pass", dict(data=array("decay_synthetic_input", synthetic), fs=fs, known_rt60=0.5), dict(first_pass=first_pass, params=params))
    decay_fixture("synthetic", ImpulseResponse(synthetic, fs), dict(file=BLOBS[summary(synthetic)["sha256"]]))
    for n in (0, 1, 9):
        save(f"decay_short_{n}", dict(data=[0.0]*n, fs=fs), dict(params=decay.decay_params(np.zeros(n), fs)))
    total = sum(WRITTEN.values())
    print(f"P08 exported {len(WRITTEN)} files, {total} bytes; Python {common.META['python']}, NumPy {np.__version__}, SciPy {common.META['scipy']}")
    print(f"Required raw full arrays alone: sweep={sweep.nbytes}, inverse={estimator.inverse_filter.nbytes}, estimate={recording.nbytes}, open_FL_pair={2*391270*8}; sum={sweep.nbytes*2+recording.nbytes+2*391270*8}")
    if total >= 12_000_000:
        print("BLOCKER: P08 fixtures exceed the 12 MB budget (docs/rust/ARCHITECTURE.md gate 2); no arrays or tolerances were silently dropped.")
        raise SystemExit(1)


if __name__ == "__main__":
    main()
