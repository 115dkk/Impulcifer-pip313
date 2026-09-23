"""P25 oracle: the real 2.x BRIR pipeline once per ProcessingConfig variant.

The p10 and p11 goldens run the demo with the defaults, virtual bass at its
defaults and six single-field room/target variants. The scenarios below give
every other field that changes the written BRIR a non-default value, on a
temporary copy of data/demo, so crates/impulcifer-service/tests/config_parity.rs
can hold the 3.x service to the same numbers with the demo_parity budget.

Each scenario records its own setup (files removed, moved out of the folder or
written, the generic room.wav) and its config; "{outside}" in a value is a
second temporary directory next to the measurement folder. The Rust test reads
the scenario list from p25_config_scenarios.json, so both sides always run the
same thing. Nothing in data/demo is written.

Run with the 2.x environment: python tests/migration/export_goldens_config.py
"""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

import numpy as np
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tests/migration/goldens"
SWEEP = ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
WINDOW = (512, 256)  # first and last samples of every hesuvi.wav track

ROOM_TARGET = "frequency,raw\n20.0,3.0\n100.0,1.5\n1000.0,0.0\n10000.0,-2.0\n20000.0,-4.0\n"
MIC_CALIBRATION = "frequency,raw\n20.0,-1.5\n200.0,0.5\n2000.0,0.0\n8000.0,1.0\n20000.0,2.0\n"
SPECIFIC_ROOM = [
    "room-BL,SL-left.wav", "room-BL,SL-right.wav", "room-FC-left.wav", "room-FC-right.wav",
    "room-FL,FR-left.wav", "room-FL,FR-right.wav", "room-SR,BR-left.wav", "room-SR,BR-right.wav",
]

SCENARIOS = {
    "head_ms": dict(config={"head_ms": 2.5}),
    "bass_boost_shelf": dict(config={"bass_boost_gain": 6.0, "bass_boost_fc": 150.0, "bass_boost_q": 0.69}),
    "vbass_crossover": dict(config={"vbass": True, "vbass_freq": 200, "vbass_hp": 20.0}),
    "vbass_normal": dict(config={"vbass": True, "vbass_polarity": "normal"}),
    "vbass_invert": dict(config={"vbass": True, "vbass_polarity": "invert"}),
    "mic_deviation": dict(config={"microphone_deviation_correction": True, "do_headphone_compensation": False}),
    "mic_deviation_strength": dict(config={"microphone_deviation_correction": True, "do_headphone_compensation": False, "mic_deviation_strength": 0.3}),
    "no_room_correction": dict(config={"do_room_correction": False}),
    "no_headphone_compensation": dict(config={"do_headphone_compensation": False}),
    "no_equalization": dict(config={"do_equalization": False}),
    "decay": dict(config={"decay": {"FL": 0.3, "FR": 0.25}}),
    "channel_balance": dict(config={"channel_balance": "trend"}),
    # Explicit files outside the folder win over the demo's own room-target.csv
    # and room-mic-calibration.txt.
    "room_files": dict(
        config={"room_target": "{outside}/target-alt.csv", "room_mic_calibration": "{outside}/mic-alt.csv"},
        write={"{outside}/target-alt.csv": ROOM_TARGET, "{outside}/mic-alt.csv": MIC_CALIBRATION},
    ),
    # A room target path that does not exist is a flat target in 2.x
    # (room_correction._open_room_target), not the folder's room-target.csv.
    "room_target_missing": dict(config={"room_target": "{outside}/no-such-target.csv"}),
    # The headphone recording lives outside the folder and the folder has none,
    # so the run can only succeed through the requested path.
    "headphone_file": dict(
        config={"headphone_compensation_file": "{outside}/my-headphones.wav"},
        move=[["headphones.wav", "{outside}/my-headphones.wav"]],
    ),
    # The demo's specific room measurements without the 400 Hz limit.
    "specific_unlimited": dict(config={"specific_limit": 0}),
    # Generic room correction only reaches speakers without a specific room
    # measurement, so these two remove the specific ones and write room.wav
    # (the p10 generic-room construction from FL,FR.wav).
    "generic_conservative": dict(
        config={"fr_combination_method": "conservative"}, remove=SPECIFIC_ROOM, generic_room=True,
    ),
    "generic_unlimited": dict(config={"generic_limit": 0}, remove=SPECIFIC_ROOM, generic_room=True),
}


def save(name, data):
    path = OUT / ("p25_" + name)
    if path.suffix == ".json":
        data = (json.dumps(data, ensure_ascii=False, separators=(",", ":"), allow_nan=False) + "\n").encode()
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    return path.name


def expand(value, outside):
    if isinstance(value, str):
        return value.replace("{outside}", str(outside))
    if isinstance(value, dict):
        return {k: expand(v, outside) for k, v in value.items()}
    return value


def child(directory, kwargs):
    # 2.x takes speakers and room measurements in os.listdir order, and the
    # order reaches the output: the normalization gain is the peak of the
    # summed magnitude response, which sits where the minimum-phase EQ FIRs
    # are noise-limited (tests/migration/README-fr.md), so a Linux directory
    # order moves the demo gain by about 0.07 dB. The p10/p11 exports saw the
    # sorted order of Windows, and 3.x sorts its listing; pin it here too.
    listdir = os.listdir
    os.listdir = lambda path=".": sorted(listdir(path))
    import impulcifer
    impulcifer.main(dir_path=directory, test_signal=str(SWEEP), **json.loads(kwargs))


def generic_room(directory):
    from core.impulse_response_estimator import ImpulseResponseEstimator
    estimator = ImpulseResponseEstimator.from_wav(str(SWEEP))
    tracks, fs = sf.read(directory / "FL,FR.wav", always_2d=True)
    frames = 2 * fs + len(estimator) + 2 * fs
    first = tracks[:frames, 0]
    sf.write(directory / "room.wav", np.stack([first, np.roll(first, 17)]).T, fs, subtype="PCM_32")
    return dict(source="FL,FR.wav", frames=int(frames), roll=17)


def run(name, spec, tmp):
    directory = tmp / name / "measurements"
    outside = tmp / name / "outside"
    directory.mkdir(parents=True)
    outside.mkdir()
    for path in (ROOT / "data/demo").iterdir():
        if path.is_file() and path.suffix.lower() in (".wav", ".csv", ".txt"):
            shutil.copy2(path, directory / path.name)
    setup = {}
    for file in spec.get("remove", []):
        (directory / file).unlink()
    for source, target in spec.get("move", []):
        shutil.move(directory / source, expand(target, outside))
    for target, text in spec.get("write", {}).items():
        Path(expand(target, outside)).write_text(text, encoding="utf-8")
    if spec.get("generic_room"):
        setup["generic_room"] = generic_room(directory)
    kwargs = expand(spec["config"], outside)
    env = dict(os.environ, MPLBACKEND="Agg", PYTHONIOENCODING="utf-8")
    start = time.perf_counter()
    subprocess.run([sys.executable, str(Path(__file__).resolve()), "--child", str(directory), json.dumps(kwargs)],
                   cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
    seconds = time.perf_counter() - start
    from core.constants import HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER
    products = {}
    windows = []
    for filename, order in (("hesuvi.wav", HESUVI_TRACK_ORDER), ("hrir.wav", HEXADECAGONAL_TRACK_ORDER)):
        samples, fs = sf.read(directory / filename, always_2d=True, dtype="float64")
        rows = []
        for index, track in enumerate(order[:samples.shape[1]]):
            values = samples[:, index]
            rows.append(dict(name=track, sha256=hashlib.sha256(values.astype("<f8").tobytes()).hexdigest(),
                             max_abs=float(np.max(np.abs(values))), rms=float(np.sqrt(np.mean(values ** 2))),
                             peak_index=int(np.argmax(np.abs(values)))))
            if filename == "hesuvi.wav":
                windows.append(np.concatenate([values[:WINDOW[0]], values[-WINDOW[1]:]]))
        products[filename] = dict(fs=int(fs), frames=len(samples), tracks=rows)
    blob = save(f"config_{name}.f64", np.concatenate(windows).astype("<f8").tobytes())
    return dict(config=spec["config"], remove=spec.get("remove", []), move=spec.get("move", []),
                write=spec.get("write", {}), setup=setup, products=products, windows=blob,
                python_seconds=seconds)


def main():
    import numpy
    import scipy
    import platform
    scenarios = {}
    with tempfile.TemporaryDirectory(prefix="impulcifer-p25-") as tmp:
        for name, spec in SCENARIOS.items():
            scenarios[name] = run(name, spec, Path(tmp))
            hesuvi = scenarios[name]["products"]["hesuvi.wav"]
            print(f"P25 {name}: {len(hesuvi['tracks'])} tracks, {hesuvi['frames']} frames, "
                  f"Python {scenarios[name]['python_seconds']:.2f}s")
    save("config_scenarios.json", dict(
        meta=dict(python=platform.python_version(), numpy=numpy.__version__, scipy=scipy.__version__,
                  source="impulcifer.main -> core/pipeline.py BRIRPipeline.run", listdir="sorted",
                  sweep=str(SWEEP.relative_to(ROOT)).replace(os.sep, "/"), window=list(WINDOW)),
        scenarios=scenarios))
    size = sum(p.stat().st_size for p in OUT.glob("p25_config_*"))
    assert size <= 2 * 1024 * 1024, size
    print(f"P25 config fixtures: {size} bytes")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--child":
        sys.path.insert(0, str(ROOT))
        child(sys.argv[2], sys.argv[3])
    else:
        sys.path.insert(0, str(ROOT))
        main()
