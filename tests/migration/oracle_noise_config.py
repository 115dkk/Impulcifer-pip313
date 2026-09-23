"""How far the 2.x BRIR output moves by itself in a P25 scenario.

Reruns one scenario of p25_config_scenarios.json with the real 2.x pipeline
while every linear-phase FIR that autoeq hands to scipy.signal.minimum_phase is
perturbed by 1e-12 of its peak (the size of the Rust/Python firwin2 difference,
tests/migration/README-fr.md "Oracle noise floor"), and prints the max-abs and
RMS ratio deviations of hesuvi.wav against the golden. The spread is the noise
floor that the BRIR-level budget of config_parity.rs has to absorb.

    python tests/migration/oracle_noise_config.py no_headphone_compensation --trials 18
"""
from pathlib import Path
import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
GOLDENS = ROOT / "tests/migration/goldens"
SWEEP = ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"


def child(directory, config, seed):
    listdir = os.listdir
    os.listdir = lambda path=".": sorted(listdir(path))  # as in export_goldens_config.py
    sys.path.insert(0, str(ROOT))
    import autoeq.frequency_response as afr
    minimum_phase = afr.minimum_phase
    rng = np.random.default_rng(seed)

    def perturbed(h, *args, **kwargs):
        h = np.asarray(h)
        return minimum_phase(h + 1e-12 * np.max(np.abs(h)) * rng.standard_normal(len(h)), *args, **kwargs)

    afr.minimum_phase = perturbed
    import impulcifer
    impulcifer.main(dir_path=directory, test_signal=str(SWEEP), **json.loads(config))


def trial(name, spec, seed, tmp):
    import soundfile as sf
    directory = tmp / f"{name}-{seed}"
    outside = tmp / f"{name}-{seed}-outside"
    directory.mkdir()
    outside.mkdir()
    for path in (ROOT / "data/demo").iterdir():
        if path.is_file() and path.suffix.lower() in (".wav", ".csv", ".txt"):
            shutil.copy2(path, directory / path.name)
    for file in spec["remove"]:
        (directory / file).unlink()
    for source, target in spec["move"]:
        shutil.move(directory / source, target.replace("{outside}", str(outside)))
    for target, text in spec["write"].items():
        Path(target.replace("{outside}", str(outside))).write_text(text, encoding="utf-8")
    if spec["setup"].get("generic_room"):
        sys.path.insert(0, str(Path(__file__).parent))
        from export_goldens_config import generic_room
        generic_room(directory)
    config = json.dumps(spec["config"]).replace("{outside}", str(outside).replace("\\", "\\\\"))
    env = dict(os.environ, MPLBACKEND="Agg", PYTHONIOENCODING="utf-8")
    subprocess.run([sys.executable, str(Path(__file__).resolve()), "--child", str(directory), config, str(seed)],
                   cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    samples, _ = sf.read(directory / "hesuvi.wav", always_2d=True, dtype="float64")
    tracks = spec["products"]["hesuvi.wav"]["tracks"]
    peak = max(abs(np.max(np.abs(samples[:, i])) / t["max_abs"] - 1) for i, t in enumerate(tracks))
    rms = max(abs(np.sqrt(np.mean(samples[:, i] ** 2)) / t["rms"] - 1) for i, t in enumerate(tracks))
    return peak, rms


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("scenario")
    parser.add_argument("--trials", type=int, default=3)
    args = parser.parse_args()
    spec = json.loads((GOLDENS / "p25_config_scenarios.json").read_text(encoding="utf-8"))["scenarios"][args.scenario]
    results = []
    with tempfile.TemporaryDirectory(prefix="impulcifer-p25-noise-") as tmp:
        for seed in range(1, args.trials + 1):
            peak, rms = trial(args.scenario, spec, seed, Path(tmp))
            results.append((peak, rms))
            print(f"{args.scenario} seed {seed}: max-abs ratio {peak:.2e}, RMS ratio {rms:.2e}")
    print(f"{args.scenario}: {args.trials} trials, max-abs ratio up to {max(r[0] for r in results):.2e}, "
          f"RMS ratio up to {max(r[1] for r in results):.2e}")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--child":
        child(sys.argv[2], sys.argv[3], int(sys.argv[4]))
    else:
        main()
