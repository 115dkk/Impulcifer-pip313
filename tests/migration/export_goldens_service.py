"""P11: real 2.x CLI on temporary demo copies; no repository demo writes."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from unittest.mock import patch

import numpy as np
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tests/migration/goldens"
sys.path.insert(0, str(ROOT))
DATE = "2026-09-07 12:00:00"


def save(name, data):
    path = OUT / ("p11_" + name)
    if path.suffix == ".json":
        data = (json.dumps(data, ensure_ascii=False, separators=(",", ":"), allow_nan=False) + "\n").encode()
    path.write_bytes(data)


def child(directory, scenario, output):
    from core.pipeline import BRIRPipeline
    from core import pipeline_stages
    from infra.logger import get_logger
    import i18n.localization as localization
    import impulcifer

    class Locale:
        def __init__(self, language):
            self.strings = json.loads((ROOT / f"i18n/locales/{language}.json").read_text(encoding="utf-8"))
        def get(self, key, default=None, **kwargs):
            return self.strings.get(key, default or key).format(**kwargs)

    class Clock:
        @staticmethod
        def now():
            return datetime.datetime.fromisoformat(DATE)

    events = []
    logger = get_logger()
    locale = Locale("en")
    localization._localization_manager = locale
    logger.set_localization(locale)
    logger.set_gui_callback(lambda level, message, **kw: events.append(dict(level=level, key=kw.get("key"), args=kw.get("args"))))
    captured = {}
    original = pipeline_stages.write_readme
    def readme(*args):
        captured["hrir"] = args[1]
        captured["estimator"] = args[3]
        captured["gain"] = args[4]
        stats = []
        for speaker, pair in args[1].irs.items():
            for side, ir in pair.items():
                peak, knee, noise, window = ir.decay_params()
                stats.append(dict(speaker=speaker, side=side, peak=int(peak), knee=int(knee), noise=float(noise), window=int(window), peak_value=float(ir.data[peak]), pnr=float(20*np.log10(abs(ir.data[peak])+1e-9)-noise)))
        captured["stats"] = stats
        captured["fr_left"] = args[1].irs["FR"]["left"].data.copy()
        return original(*args)
    stage_samples = {}
    def trace_stage(original_stage, name):
        def traced(pipeline):
            if name == "equalize":
                captured["eq_sources"] = dict(room=pipeline.room_frs["FR"]["left"].error.tolist(), headphone=pipeline.hp_left.error.tolist(), target=pipeline.target.raw.tolist())
            original_stage(pipeline)
            stage_samples[name] = pipeline.hrir.irs["FR"]["left"].data.copy()
        return traced
    from contextlib import ExitStack
    start = time.perf_counter()
    # Wrappers only observe outputs; all real DSP and plots execute unchanged.
    with ExitStack() as stack:
        stack.enter_context(patch.object(pipeline_stages, "datetime", Clock))
        stack.enter_context(patch.object(pipeline_stages, "write_readme", readme))
        for name in ("crop_and_align", "virtual_bass", "equalize"):
            method = "_stage_" + name
            stack.enter_context(patch.object(BRIRPipeline, method, trace_stage(getattr(BRIRPipeline, method), name)))
        impulcifer.main(dir_path=directory, test_signal=str(ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"), vbass=scenario == "vbass", vbass_freq=250)
    seconds = time.perf_counter() - start
    for language in ("en", "ko"):
        localization._localization_manager = Locale(language)
        with patch.object(pipeline_stages, "datetime", Clock):
            original(str(Path(output) / f"{language}.md"), captured["hrir"], None, captured["estimator"], captured["gain"])
    for name, samples in stage_samples.items():
        Path(output, f"{name}.f64").write_bytes(samples.astype("<f8").tobytes())
    Path(output, "fr_left.f64").write_bytes(captured["fr_left"].astype("<f8").tobytes())
    Path(output, "metadata.json").write_text(json.dumps(dict(seconds=seconds, events=events, readme_stats=captured["stats"], eq_sources=captured["eq_sources"]), default=str), encoding="utf-8")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--child":
        child(*sys.argv[2:])
    else:
        from core.constants import HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER
        from core.brir_layout import append_track_names
        from core.sweep_detection import detect_sweep_parameters, snap_sweep_samples
        grid = []
        for fs in (8000, 44100, 48000, 96000):
            for duration in (0.1, 1.0, 3.08, 5.0, 6.15, 15.0):
                m, n, deviation = snap_sweep_samples(duration * fs, fs)
                grid.append(dict(fs=fs, estimate=duration*fs, m=m, n=n, deviation=deviation))
        save("sweep_grid.json", grid)
        detection = detect_sweep_parameters(str(ROOT / "data/demo"))
        save("detection.json", dict(fs=detection.fs, m=detection.m, samples=detection.sweep_samples, n_segments=detection.n_segments, speakers=detection.speakers, confidence=detection.confidence, generate_spec=detection.generate_spec(), source_files=detection.source_files))
        with tempfile.TemporaryDirectory(prefix="impulcifer-p11-") as tmp:
            tmp = Path(tmp)
            metadata = tmp / "metadata.wav"
            sf.write(metadata, np.zeros((8, 4)), 48000, subtype="PCM_32")
            append_track_names(metadata, ["FL-left", "FL-right", "FR-left", "FR-right"])
            save("python_ichl.wav", metadata.read_bytes())
            for scenario in ("default", "vbass"):
                directory = tmp / scenario
                directory.mkdir()
                for path in (ROOT / "data/demo").iterdir():
                    if path.is_file() and (path.suffix.lower() in (".wav", ".csv", ".txt")):
                        shutil.copy2(path, directory / path.name)
                output = tmp / f"{scenario}-metadata"
                output.mkdir()
                env = dict(os.environ, MPLBACKEND="Agg", PYTHONIOENCODING="utf-8")
                subprocess.run([sys.executable, str(Path(__file__).resolve()), "--child", str(directory), scenario, str(output)], cwd=ROOT, env=env, check=True)
                info = json.loads((output / "metadata.json").read_text(encoding="utf-8"))
                for language in ("en", "ko"):
                    # Golden stores LF; tests compare platform-native bytes separately.
                    save(f"{scenario}_{language}.txt", (output / f"{language}.md").read_text(encoding="utf-8").encode())
                products = {}
                for filename, order in (("hesuvi.wav", HESUVI_TRACK_ORDER), ("hrir.wav", HEXADECAGONAL_TRACK_ORDER)):
                    samples, fs = sf.read(directory / filename, always_2d=True, dtype="float64")
                    tracks = []
                    for index, name in enumerate(order[:samples.shape[1]]):
                        values = samples[:, index]
                        row = dict(name=name, sha256=hashlib.sha256(values.astype("<f8").tobytes()).hexdigest(), first=values[:256].tolist(), last=values[-256:].tolist(), max_abs=float(np.max(np.abs(values))), rms=float(np.sqrt(np.mean(values**2))), peak_index=int(np.argmax(np.abs(values))))
                        if name in ("FL-left", "FL-right", "FC-left"):
                            file = f"{scenario}_{filename}_{name}.f64"
                            save(file, values.astype("<f8").tobytes())
                            row["file"] = "p11_" + file
                        tracks.append(row)
                    products[filename] = dict(fs=fs, frames=len(samples), tracks=tracks)
                for name in ("crop_and_align", "virtual_bass", "equalize"):
                    path = output / f"{name}.f64"
                    if path.exists():
                        save(f"{scenario}_{name}_FR-left.f64", path.read_bytes())
                save(f"{scenario}_readme_FR-left.f64", (output / "fr_left.f64").read_bytes())
                save(f"{scenario}.json", dict(python_seconds=info["seconds"], date=DATE, products=products, events=info["events"], readme_stats=info["readme_stats"], eq_sources=info["eq_sources"]))
                print(f"P11 {scenario}: Python {info['seconds']:.6f}s; {[(k, v['frames'], len(v['tracks'])) for k,v in products.items()]}")
        budget = sum(p.stat().st_size for p in OUT.glob("p11_*"))
        assert budget <= 12 * 1024 * 1024, budget
        print(f"P11 fixtures: {budget} bytes (budget 12582912)")
