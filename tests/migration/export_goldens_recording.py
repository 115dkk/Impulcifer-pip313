"""Export P16 recording contracts from the unmodified Python 2.x oracle.

Run with py -3.14. Only p16_* fixtures are written; audio inputs live in a
TemporaryDirectory, never data/. $ROOT is substituted by the Rust tests.
"""
from __future__ import annotations

from dataclasses import asdict, is_dataclass
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace

import numpy as np
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.modules["sounddevice"] = SimpleNamespace(
    query_hostapis=lambda: [{"name": "Windows WASAPI"}],
    query_devices=lambda: [], default=SimpleNamespace(device=(-1, -1)),
)
from application.impulcifer_service import ImpulciferApplicationService  # noqa: E402
from core.recording_naming import derive_record_filename, record_filename_for_speakers  # noqa: E402
from core.recording_progress import event_for_elapsed, infer_sweep_segments  # noqa: E402
from core.recording_status import analyze_recording  # noqa: E402
from core.sweep_signal import SweepSpec, build_sweep_playback  # noqa: E402

OUT = ROOT / "tests/migration/goldens"


def safe(value):
    if is_dataclass(value):
        return safe(asdict(value))
    if isinstance(value, dict):
        return {k: safe(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [safe(v) for v in value]
    return value


def main():
    service = ImpulciferApplicationService()
    result = {"validation": [], "sweep_validation": [], "paths": [], "naming": [],
              "speaker_names": [], "playback": [], "progress": [], "analysis": []}
    with tempfile.TemporaryDirectory(prefix="impulcifer-p16-oracle-") as temporary:
        folder = Path(temporary)
        for channels, name in [(1, "mono.wav"), (2, "stereo.wav"), (3, "three.wav"),
                               (2, "sweep-seg-FL,FR-stereo-1s-test.wav")]:
            sf.write(folder / name, np.zeros((32, channels)), 8000, subtype="PCM_32")
        (folder / "broken.wav").write_bytes(b"broken")

        def expand(v):
            if isinstance(v, str):
                return v.replace("$ROOT", str(folder))
            if isinstance(v, dict):
                return {k: expand(x) for k, x in v.items()}
            if isinstance(v, list):
                return [expand(x) for x in v]
            return v

        def normalize(v):
            v = safe(v)
            if isinstance(v, str):
                return v.replace(str(folder), "$ROOT").replace("\\", "/")
            if isinstance(v, dict):
                return {k: normalize(x) for k, x in v.items()}
            if isinstance(v, list):
                return [normalize(x) for x in v]
            return v

        def validation(name, request):
            response = service._validate_recording_request(expand(request))
            result["validation"].append({"name": name, "request": request, "response": normalize(response)})

        base = {"record_dir": "$ROOT/out", "sweep": {"mode": "default"}}
        for name, request in [("non_object", None), ("array", []), ("unknown_sorted", {"z": 1, "a": 2}),
                              ("mode_before_dir", {"mode": "bad"}), ("missing_dir", {}),
                              ("blank_dir", {"record_dir": "  "}), ("missing_play", {"record_dir": "$ROOT"}),
                              ("null_play", {"record_dir": "$ROOT", "play_path": None}),
                              ("directory_play", {"record_dir": "$ROOT", "play_path": "$ROOT"}),
                              ("default", base), ("null_dir_python_string", {**base, "record_dir": None}),
                              ("optional_strings", {**base, "input_device": " Mic ", "output_device": " ", "host_api": 5}),
                              ("channels_unforced", {**base, "channels": 64}),
                              ("forced_match", {**base, "channels": 4, "force_channels": True}),
                              ("forced_mismatch", {**base, "force_channels": True}),
                              ("confirmed", {**base, "force_channels": True, "confirm_warnings": True}),
                              ("headphones_ignore_append", {**base, "mode": "headphones", "channels": 14, "append": "bad"}),
                              ("mono_generated_path", {**base, "sweep": {"tracks": "mono", "speakers": "FR"}})]:
            validation(name, request)
        for key, values in {"channels": [True, False, None, "2", 2.0, 0, 65, -1],
                            "force_channels": [None, 1, "true"], "confirm_warnings": [None, 1, "true"],
                            "append": [None, 1, "true"], "debug_plots": [None, 1, "true"]}.items():
            for i, value in enumerate(values):
                validation(f"{key}_invalid_{i}", {**base, key: value})
        for name in ["mono.wav", "stereo.wav", "three.wav", "broken.wav", "absent.wav"]:
            for confirmed in [False, True]:
                validation(f"headphones_{name}_{confirmed}", {"record_dir": "$ROOT/out", "mode": "headphones",
                           "play_path": f"$ROOT/{name}", "confirm_warnings": confirmed})
        for name in ["stereo.wav", "sweep-seg-FL,FR-stereo-1s-test.wav"]:
            validation(f"file_{name}", {"record_dir": "$ROOT/out", "play_path": f"$ROOT/{name}", "force_channels": True})
        # Every branch of _validate_sweep_request, including headphone overrides.
        sweeps = [None, [], "bad", {"z": 1, "a": 2}, {"mode": "bad"}, {"mode": None},
                  {"mode": "file", "fs": False}, {}, {"fs": False, "duration": "bad"},
                  {"speakers": None}, {"speakers": 1}, {"speakers": []}, {"speakers": " , "},
                  {"speakers": [None]}, {"speakers": [True]}, {"speakers": ["XX"]},
                  {"speakers": ["FL", " fl "]}, {"speakers": " fl, fr, "},
                  {"tracks": None}, {"tracks": "bogus"}, {"speakers": "FL,FR,FC"},
                  {"speakers": "TFL", "tracks": "7.1"}, {"speakers": "TSL,TSR", "tracks": "stereo"},
                  {"speakers": "FR", "tracks": "mono"}, {"mode": "custom"}]
        for fs in [True, None, "48000", 8000.5, 7999, 384001, -1, 8000, 384000, 8000.0]:
            sweeps.append({"mode": "custom", "fs": fs})
        for duration in [True, None, "5", 0, 0.099, 60.1, 0.1, 60]:
            sweeps.append({"mode": "custom", "duration": duration})
        for mode in ["speakers", "headphones"]:
            for i, sweep in enumerate(sweeps):
                response = service._validate_sweep_request(sweep, mode)
                result["sweep_validation"].append({"name": f"{mode}_{i}", "mode": mode, "sweep": sweep, "response": normalize(response)})
                validation(f"sweep_{mode}_{i}", {**base, "mode": mode, "sweep": sweep, "play_path": "$ROOT/stereo.wav"})
        for args in [[""], [None], ["$ROOT/out"], ["$ROOT/out", None, "headphones", []],
                     ["$ROOT/out", "foo.mlp"], ["$ROOT/out/../out", "sweep-seg-tfl,tfr-stereo-x.wav"],
                     ["$ROOT/out", None, "speakers", {"speakers": "BL,BR"}],
                     ["$ROOT/out", "x.wav", "speakers", {"mode": "file"}],
                     ["$ROOT/out", None, "speakers", {"tracks": "bad"}],
                     [42, "foo.wav"]]:
            result["paths"].append({"args": args, "response": normalize(service.resolve_recording_paths(*expand(args)))})
        for name in ["", "foo.mlp", ".hidden", "...", "folder/", "folder/a.b.wav",
                     "sweep-seg-fl,fr-stereo-x.wav", "prefix-sweep-seg-tfl,tsr-7.1.6-x.wav", "sweep-seg-XX-stereo.wav"]:
            result["naming"].append({"input": name, "output": derive_record_filename(name)})
        for names in [[], ["FL", "FR"], [" fc "], ["tfl", "TFR"], ["FL", "fl"], ["XX"], ["", "FR"]]:
            try:
                response = {"ok": True, "name": record_filename_for_speakers(names)}
            except ValueError as exc:
                response = {"ok": False, "message": str(exc)}
            result["speaker_names"].append({"input": names, "response": response})
        for name, duration in [("sweep-seg-FL,FR-stereo-6.15s-x", 18.3),
                               ("prefix-sweep-seg-tfl,tsr-7.1.6-1s-x", 7),
                               ("sweep-seg-FL,FR-stereo-0s-x", 10),
                               ("sweep-seg-FL,FR-stereo-6.15s-x", 11),
                               ("sweep-seg-FL-mono-1s-x", 1), ("plain.wav", 0),
                               ("sweep-seg-XX-stereo-1s-x", 12), ("plain.wav", -1)]:
            segments = infer_sweep_segments(name, duration)
            times = [-1, 0, 2, 3, 4, 8.15, 9, 10.15, 16.3, 30]
            result["progress"].append({"file": name, "duration": duration, "segments": safe(segments),
                "events": [{"elapsed": t, "event": safe(event_for_elapsed(elapsed=t, duration=duration, segments=segments))} for t in times]})
        for i, data in enumerate([np.zeros((23, 1)), np.tile([0.5, 0.0, 1e-7], (37, 1)),
                                  np.tile([-0.25, 0.75], (8003, 1))]):
            name = f"p16_analysis_{i}.wav"
            sf.write(OUT / name, data, 8000, subtype="PCM_32")
            result["analysis"].append({"file": name, "summary": safe(analyze_recording(str(OUT / name)))})
    for i, spec in enumerate([SweepSpec(), SweepSpec(fs=8000, duration=1, speakers=("FR",), tracks="mono"),
                              SweepSpec(fs=8000, duration=1, speakers=("FL", "TBR"), tracks="7.1.4"),
                              SweepSpec(fs=12000, duration=2, speakers=("SL", "SR"))]):
        playback = build_sweep_playback(spec)
        # Store only the non-silent sweep as PCM32, reconstruct the entire
        # reference sequence in Rust, and hash all track-major LE-f64 bytes.
        first = playback.segments[0]
        start = int(first.start * spec.fs)
        signal = next(t[start:start + len(playback.estimator)] for t in playback.data if np.any(t[start:start + len(playback.estimator)]))
        name = f"p16_signal_{i}.pcm32"
        (OUT / name).write_bytes(np.rint(signal * 2**31).astype("<i4").tobytes())
        result["playback"].append({"input": asdict(spec), "spec": asdict(playback.spec),
            "shape": list(playback.data.shape), "segments": safe(playback.segments),
            "display_name": playback.display_name, "record_filename": playback.record_filename,
            "sha256": hashlib.sha256(playback.data.astype("<f8").tobytes()).hexdigest(), "signal": name})
    output = OUT / "p16_recording.json"
    output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    size = sum(p.stat().st_size for p in OUT.glob("p16_*"))
    assert size <= 12_000_000, size
    print("P16 oracle export (CPython " + sys.version.split()[0] + ")")
    for key, values in result.items():
        print(f"{key}: {len(values)}")
    print(f"p16_* bytes: {size} (budget 12000000)")


if __name__ == "__main__":
    main()
