"""P19: observe real matplotlib figures and the real pipeline in temporary copies.

Full native FR grids (stride=1). Figure.savefig observation records the actual
Line2D x/y data, not a second implementation of plotting math. The result-stage
snapshot also exports summed IRs for an isolated strict smoothing comparison.
"""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
SIGNAL = ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"
GOLDENS = ROOT / "tests/migration/goldens"


def canvas_size(fig):
    return [int(round(value * fig.dpi)) for value in fig.get_size_inches()]


def record_canvas(fig, path):
    # Sidecars are temporary evidence, including figures saved by spawned workers.
    Path(str(path) + ".canvas.json").write_text(json.dumps(canvas_size(fig)), encoding="utf-8")


def observed_panel_worker(args):
    from matplotlib.figure import Figure
    from core.parallel_workers import render_hrir_figure_worker

    savefig = Figure.savefig

    def capture(fig, path, *positional, **kwargs):
        record_canvas(fig, path)
        return savefig(fig, path, *positional, **kwargs)

    Figure.savefig = capture
    try:
        return render_hrir_figure_worker(args)
    finally:
        Figure.savefig = savefig


def main():
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.figure import Figure
    import numpy as np
    from PIL import Image
    import impulcifer
    from core.pipeline import BRIRPipeline
    from core.pipeline_stages import equalization
    from core.impulse_response_estimator import ImpulseResponseEstimator
    from bench_oracle_pipeline import INPUTS

    work = Path(tempfile.mkdtemp(prefix="impulcifer-p19-oracle-"))
    print(f"P19_ORACLE_DIRECTORY {work}", flush=True)
    result = {"stride": 1, "capture": "Figure.savefig: actual Line2D native arrays", "runs": {}}
    active = {}
    savefig = Figure.savefig
    stage = BRIRPipeline._stage_plot_results
    import core.parallel_workers as workers
    panel_worker = workers.render_hrir_figure_worker

    def capture(fig, path, *args, **kwargs):
        record_canvas(fig, path)
        name = Path(path).name
        if name in ("results.png", "headphones.png", "eq.png"):
            active[name] = [{"title": ax.get_title(), "xlim": list(ax.get_xlim()),
                             "ylim": list(ax.get_ylim()),
                             "legend": [t.get_text() for t in ax.get_legend().get_texts()] if ax.get_legend() else [],
                             "lines": [{"x": np.asarray(line.get_xdata(), dtype=float).tolist(),
                                        "y": np.asarray(line.get_ydata(), dtype=float).tolist(),
                                        "color": line.get_color(), "label": line.get_label()}
                                       for line in ax.lines]} for ax in fig.axes]
        return savefig(fig, path, *args, **kwargs)

    def snapshot(self):
        for side in ("left", "right"):
            data = np.sum(np.vstack([pair[side].data for pair in self.hrir.irs.values()]), axis=0)
            name = f"p19_results_{side}.f64"
            if not self.config.plot:
                (GOLDENS / name).write_bytes(np.asarray(data, dtype="<f8").tobytes())
            active[f"{side}_ir"] = {"file": name, "length": len(data), "fs": self.hrir.fs}
        return stage(self)

    Figure.savefig = capture
    BRIRPipeline._stage_plot_results = snapshot
    workers.render_hrir_figure_worker = observed_panel_worker
    try:
        for plot in (False, True):
            name = "plot" if plot else "default"
            directory = work / name
            directory.mkdir()
            for filename in INPUTS:
                src = ROOT / "data/demo" / filename
                if src.is_file():
                    shutil.copy2(src, directory / filename)
            active = {}
            impulcifer.main(dir_path=str(directory), test_signal=str(SIGNAL), plot=plot)
            pngs = {}
            for path in sorted(directory.rglob("*.png")):
                with Image.open(path) as image:
                    pngs[path.relative_to(directory).as_posix()] = {
                        "size": list(image.size),
                        "canvas": json.loads(Path(str(path) + ".canvas.json").read_text()),
                    }
            result["runs"][name] = {"pngs": pngs, "series": active}
            print(name, json.dumps(pngs), flush=True)
            plt.close("all")
        result["eq_cases"] = {}
        estimator = ImpulseResponseEstimator.from_wav(str(SIGNAL))
        for name, files in {
            "common": {"eq.csv": "frequency,raw\n20.0,1.0\n100.0,-2.0\n1000.0,3.0\n20000.0,-1.0\n"},
            "split": {"eq-left.csv": "frequency,raw\n20.0,1.0\n100.0,-2.0\n1000.0,3.0\n20000.0,-1.0\n", "eq-right.csv": "frequency,raw\n20.0,-1.0\n100.0,2.0\n1000.0,-3.0\n20000.0,1.0\n"},
        }.items():
            directory = work / f"eq-{name}"
            directory.mkdir()
            for filename, text in files.items():
                (directory / filename).write_text(text, encoding="utf-8")
            active = {}
            equalization(estimator, str(directory))
            with Image.open(directory / "plots/eq.png") as image:
                size = list(image.size)
            result["eq_cases"][name] = {"files": files, "axes": active["eq.png"], "size": size,
                                         "canvas": json.loads((directory / "plots/eq.png.canvas.json").read_text())}
            plt.close("all")
        directory = work / "generic-room"
        directory.mkdir()
        for filename in ("FL,FR.wav", "headphones.wav", "room.wav"):
            source = filename if filename != "room.wav" else "room-FL,FR-left.wav"
            shutil.copy2(ROOT / "data/demo" / source, directory / filename)
        active = {}
        impulcifer.main(dir_path=str(directory), test_signal=str(SIGNAL), plot=True)
        with Image.open(directory / "plots/room/room.png") as image:
            result["generic_room"] = {
                "source": "data/demo/room-FL,FR-left.wav copied as room.wav (demo has no room.wav)",
                "size": list(image.size),
                "canvas": json.loads((directory / "plots/room/room.png.canvas.json").read_text()),
            }
        plt.close("all")
    finally:
        Figure.savefig = savefig
        BRIRPipeline._stage_plot_results = stage
        workers.render_hrir_figure_worker = panel_worker
    # Directly observe two complete panel figures for an independent two-pass
    # limit oracle. Synthetic inputs are deterministic, not platform RNG state.
    from core.impulse_response import ImpulseResponse
    result["panel_cases"] = []
    for index in range(2):
        fs = 8000
        t = np.arange(8000 + index * 800) / fs
        data = (np.sin(2 * np.pi * 431 * t) + 0.3 * np.sin(2 * np.pi * 1231 * t)) * np.exp(-t * (12 + index))
        data += 1e-5 * np.sin(2 * np.pi * 2701 * t)
        data[20 + index * 10] += 2
        recording = np.sin(2 * np.pi * (70 * t + 900 * t * t)) * (0.5 + 0.1 * index)
        ir = ImpulseResponse(data, fs, recording=recording)
        fig = ir.plot()
        limits = [{"x": list(ax.get_xlim()), "y": list(ax.get_ylim())} for ax in fig.axes[:6]]
        result["panel_cases"].append({"fs": fs, "data": data.tolist(), "recording": recording.tolist(), "limits": limits})
        plt.close(fig)
    encoded = json.dumps(result, separators=(",", ":"), allow_nan=False).encode()
    assert len(encoded) < 3 * 1024 * 1024, len(encoded)
    (GOLDENS / "p19_plots.json").write_bytes(encoded)
    print(f"p19_plots.json {len(encoded)} bytes; stride=1; native grids retained", flush=True)


if __name__ == "__main__":
    main()
