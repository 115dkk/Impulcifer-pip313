"""Export P17 recovery oracles in temporary directories, never data/demo.

Run with CPython 3.14. Inputs use the portable recipe recorded in the JSON;
outputs retain SHA256 and the first/last 64 frames of each channel, not WAVs.
"""
from __future__ import annotations

import hashlib
import json
import os
from dataclasses import asdict
from pathlib import Path
import struct
import sys
import tempfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
import numpy as np
import soundfile as sf
from application.impulcifer_service import ImpulciferApplicationService
from core import brir_recovery
from core.constants import HEXADECAGONAL_TRACK_ORDER as HRIR, HESUVI_TRACK_ORDER as HESUVI, SPEAKER_NAMES

GOLDENS = ROOT / "tests/migration/goldens"
TRACKS = [f"{speaker}-{side}" for speaker in SPEAKER_NAMES for side in ("left", "right")]


def signal(name, count):
    if name == "zero" or name.startswith("LFE-"):
        return np.zeros(count)
    if name == "tiny":
        data = np.zeros(count)
        data[-1] = 1e-30
        return data
    if name == "nan":
        data = np.zeros(count)
        data[0] = np.nan
        return data
    if name == "inf":
        data = np.zeros(count)
        data[0] = np.inf
        return data
    if name == "rounding":
        values = np.array([-2., -1., -0.5, -2.5 / 2**31, -1.5 / 2**31, -0.5 / 2**31,
                           0.5 / 2**31, 1.5 / 2**31, 2.5 / 2**31, .5, 1., 2.])
        return np.resize(values, count)
    index = TRACKS.index(name) + 1
    return ((np.arange(count, dtype=np.int64) * 17 + index * 101) % 8192 - 4096) / 65536


def wav(path, rows, **kwargs):
    return {"path": path, "rows": rows, **kwargs}


def combined(kind="hrir", speakers=("FL", "FR"), **kwargs):
    order = HRIR if kind == "hrir" else HESUVI
    rows = [name if name.split("-")[0] in speakers else "zero" for name in order]
    return wav(f"{kind}.wav", rows, **kwargs)


def split(path="Hangloose", speakers=("FL", "FR"), prefix="", **kwargs):
    return [wav(f"{path + '/' if path else ''}{prefix}{s}.wav", [f"{s}-left", f"{s}-right"], **kwargs) for s in speakers]


def cases():
    result = []
    def add(name, inputs, **kw):
        result.append({"name": name, "inputs": inputs, "selected": ".", "options": {}, **kw})
    add("hrir_only", [combined()], options={"include_hangloose": True})
    add("hesuvi_only", [combined("hesuvi", ("FL", "FR", "SL", "SR"))])
    add("both_consistent", [combined(s, SPEAKER_NAMES) for s in ("hrir", "hesuvi")])
    add("both_consistent_split", [combined(s) for s in ("hrir", "hesuvi")], options={"include_hangloose": True})
    add("both_mismatch", [combined(), combined("hesuvi", ("FL",))])
    add("both_rate", [combined(), combined("hesuvi", rate=44100)])
    add("both_count", [combined(), combined("hesuvi", count=4799)])
    for selected, directory, name in [(".", "Hangloose", "nested"), ("Hangloose", "Hangloose", "selected"), (".", "", "loose")]:
        add("split_" + name, split(directory, ("FL", "FR", "FC", "SL", "SR", "TFL", "TFR")), selected=selected)
    add("split_prefixed", split("", SPEAKER_NAMES[:7], "400se7chdelay"))
    add("split_longest_suffix", split("", ("TFL",), "profile"))
    add("prefix_casefold", [wav("StraßeFL.wav", ["FL-left", "FL-right"]), wav("STRASSEFR.wav", ["FR-left", "FR-right"])])
    add("prefix_case_insensitive", [wav("ProfileFL.WAV", ["FL-left", "FL-right"]), wav("profileFR.wav", ["FR-left", "FR-right"])])
    add("mixed_prefix", split("", ("FL",), "first") + split("", ("FR",), "second"))
    add("duplicate_speaker", split("", ("FL",), "profile") + [wav("PROFILEfl.WAV", ["FL-left", "FL-right"])], requires_case_sensitive=True)
    add("ambiguous_hrir", [combined(), {**combined(), "path": "HRIR.WAV"}], requires_case_sensitive=True)
    add("ambiguous_dirs", split("Hangloose") + split("HANGLOOSE"), requires_case_sensitive=True)
    add("named_case_insensitive", [{**combined(), "path": "HRIR.WAV"}], options={"include_hangloose": True})
    add("nested_case_insensitive", split("hAnGlOoSe"))
    add("selected_with_combined", [{**combined(), "path": "Hangloose/hrir.wav"}] + split(), selected="Hangloose")
    add("both_ignore_loose_raw", [combined("hesuvi"), wav("FC.wav", ["FL-left", "FR-right"]), wav("room-FC.wav", ["FL-left", "FR-right"]), wav("FL,FR.wav", ["FL-left", "FR-right"])], options={"include_hangloose": True})
    add("existing_subset", [combined()] + split(speakers=("FL",)), options={"include_hangloose": True})
    add("existing_prefixed_subset", [combined()] + split(speakers=("FL",), prefix="profile"), options={"include_hangloose": True})
    add("split_mismatch", [combined()] + split(speakers=("FC",)), options={"include_hangloose": True})
    add("split_rate", split(speakers=("FL",)) + split(speakers=("FR",), rate=44100))
    add("split_count", split(speakers=("FL",)) + split(speakers=("FR",), count=4799))
    for field, value in [("rate", 44100), ("count", 4799)]:
        add("subset_" + field, [combined()] + split(speakers=("FL",), **{field: value}), options={"include_hangloose": True})
    add("split_mono", [wav("FL.wav", ["FL-left"])])
    add("split_three", [wav("FL.wav", ["FL-left", "FL-right", "FR-left"])])
    for kind, count in [("hrir", 14), ("hrir", 17), ("hrir", 34), ("hesuvi", 4), ("hesuvi", 9), ("hesuvi", 15), ("hesuvi", 32)]:
        add(f"wrong_count_{kind}_{count}", [wav(f"{kind}.wav", ["zero"] * count)])
    lfe = combined()
    lfe["rows"][6] = "FC-left"
    add("non_silent_lfe", [lfe])
    add("empty", [])
    add("missing_directory", [], selected="absent")
    add("selected_file", [wav("FL.wav", ["FL-left", "FL-right"])], selected="FL.wav")
    add("lexical_directory", [combined()], selected="child/..", directories=["child"])
    add("zero_frames", [combined(count=0)])
    add("nan", [wav("hrir.wav", ["nan"] + ["zero"] * 15, subtype="DOUBLE")])
    add("infinity", [wav("hesuvi.wav", ["inf"] + ["zero"] * 13, subtype="DOUBLE")])
    add("broken_wav", [{"path": "hrir.wav", "broken": True}], library_reason=True)
    add("silent_combined", [combined(speakers=())], options={"include_hangloose": True})
    add("silent_split", [wav("FL.wav", ["zero", "zero"])])
    add("silent_compact", [combined(speakers=())], options={"remove_silent_channels": True})
    add("silent_split_compact", [wav("FL.wav", ["zero", "zero"])], options={"remove_silent_channels": True})
    add("silent_pair_compact_no_writes", [combined(s, ()) for s in ("hrir", "hesuvi")], options={"remove_silent_channels": True})
    add("compact_output", [combined()], options={"remove_silent_channels": True, "include_hangloose": True})
    add("compact_split_output", split(speakers=("TFL",)), options={"remove_silent_channels": True})
    add("rounding_clipping", [wav("hrir.wav", ["rounding"], subtype="DOUBLE", maps=[["FL-left"]])])
    for kind in ("hrir", "hesuvi"):
        for speaker in ("FL", "TFL", "TBR"):
            order = HRIR if kind == "hrir" else HESUVI
            end = max(16 if kind == "hrir" else 14, order.index(f"{speaker}-right") + 1)
            spec = combined(kind, (speaker,))
            spec["rows"] = spec["rows"][:end]
            spec["rows"][order.index(f"{speaker}-right")] = "zero"
            add(f"trimmed_{kind}_{speaker}", [spec], options={"include_hangloose": True})
        names = ["FL-left", "FR-left", "FR-right", "TBL-left", "TBL-right"]
        for compact in (False, True):
            add(f"mapped_{kind}_{compact}", [wav(f"{kind}.wav", names, maps=[names])], options={"remove_silent_channels": compact, "include_hangloose": True})
    add("mapped_mono", [wav("hrir.wav", ["FR-right"], maps=[["FR-right"]])])
    names = [f"{s}-{side}" for s in ("FL", "FR", "FC", "SL", "SR", "TFL", "TFR") for side in ("left", "right")]
    add("mapped_fourteen", [wav("hesuvi.wav", names, maps=[names])])
    names = [f"{s}-{side}" for s in SPEAKER_NAMES[:8] for side in ("left", "right")]
    add("mapped_sixteen", [wav("hrir.wav", names, maps=[names])])
    add("tiny_extension", [wav("hesuvi.wav", ["zero"] * 18 + ["tiny", "zero"], subtype="DOUBLE")])
    add("tiny_compact", [wav("hrir.wav", ["tiny"], subtype="DOUBLE", maps=[["FL-left"]])], options={"remove_silent_channels": True})
    for label, payloads in [
        ("duplicate_names", [["FL-left", "FL-left"]]), ("unknown_name", [["FL-left", "UNKNOWN"]]),
        ("wrong_map_count", [["FL-left"]]), ("bad_json", ["{"]),
        ("bad_version", [{"version": 2, "tracks": []}]), ("bool_version", [{"version": True, "tracks": []}]),
        ("float_version", [{"version": 1.0, "tracks": []}]), ("oversized", [" " * 4097]),
        ("duplicate_map", [["FL-left", "FL-right"], ["FL-left", "FL-right"]]),
        ("not_object", ["[]"]), ("non_string_name", [{"version": 1, "tracks": [1, "FL-right"]}]),
    ]:
        for compact in (False, True):
            add(f"map_{label}_{compact}", [wav("hrir.wav", ["FL-left", "FL-right"], maps=payloads)], options={"remove_silent_channels": compact})
    add("write_metadata_failure", split(), options={"remove_silent_channels": True}, fault="metadata")
    add("output_conflict_before_stage", split(), fault="stage_conflict")
    add("output_conflict_before_publish", split(), fault="publish_conflict")
    add("rollback_after_first_publish", split(), fault="second_publish")
    add("read_only_directory", split(), fault="readonly")
    return result


def map_payload(value):
    if isinstance(value, list):
        value = {"version": 1, "tracks": value}
    if not isinstance(value, str):
        value = json.dumps(value, separators=(",", ":"))
    return value.encode()


def setup(root, case):
    for directory in case.get("directories", []):
        (root / directory).mkdir(parents=True, exist_ok=True)
    for item in case["inputs"]:
        path = root / item["path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        if item.get("broken"):
            path.write_bytes(b"broken")
            continue
        matrix = np.array([signal(name, item.get("count", 4800)) for name in item["rows"]])
        sf.write(path, matrix.T, item.get("rate", 48000), subtype=item.get("subtype", "PCM_32"))
        for value in item.get("maps", []):
            payload = map_payload(value)
            with path.open("r+b") as handle:
                handle.seek(0, 2)
                handle.write(b"ICHL" + struct.pack("<I", len(payload)) + payload + b"\0" * (len(payload) % 2))
                end = handle.tell()
                handle.seek(4)
                handle.write(struct.pack("<I", end - 8))


def normalize(value, root):
    if isinstance(value, str):
        # libsndfile includes repr(path), which doubles Windows separators.
        escaped_root = str(root).replace(chr(92), chr(92) * 2)
        return value.replace(escaped_root, "$ROOT").replace(str(root), "$ROOT").replace(chr(92) * 2, "/").replace(chr(92), "/")
    if isinstance(value, (list, tuple)):
        return [normalize(v, root) for v in value]
    if isinstance(value, dict):
        return {k: normalize(v, root) for k, v in value.items()}
    return value


def run_case(root, case):
    setup(root, case)
    original = {p: p.read_bytes() for p in root.rglob("*.wav")}
    written = []
    fault = case.get("fault")
    real_find = brir_recovery._find_named_file
    real_replace = brir_recovery.os.replace
    real_append = brir_recovery.append_track_names
    checks = 0
    def find(directory, name):
        nonlocal checks
        # First hrir lookup is discovery, later calls check planned targets.
        if name == "hrir.wav":
            checks += 1
            wanted = 3 if fault == "stage_conflict" else 4
            if fault in ("stage_conflict", "publish_conflict") and checks == wanted:
                (directory / name).write_bytes(b"competing output")
        return real_find(directory, name)
    def append(*args):
        if fault == "metadata":
            raise OSError("simulated metadata write failure")
        return real_append(*args)
    def replace(source, target):
        if fault == "second_publish" and target.name == "hesuvi.wav":
            raise OSError("simulated second publish failure")
        return real_replace(source, target)
    if fault == "readonly":
        if os.name == "nt" or hasattr(os, "geteuid") and os.geteuid() == 0:
            return {"skip": "Directory mode bits do not prohibit writes on Windows or as root; no ACL changes made."}
        root.chmod(0o555)
    try:
        with patch.object(brir_recovery, "_find_named_file", find), patch.object(brir_recovery, "append_track_names", append), patch.object(brir_recovery.os, "replace", replace):
            try:
                response = {"result": asdict(brir_recovery.recover_brir_outputs(root / case["selected"], **case["options"]))}
                for value in response["result"]["created_files"]:
                    path = Path(value)
                    data, rate = sf.read(path, always_2d=True)
                    written.append({"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "rate": rate,
                                    "channels": data.shape[1], "frames": len(data), "first64": data[:64].T.tolist(), "last64": data[-64:].T.tolist()})
            except brir_recovery.BrirRecoveryError as exc:
                response = {"error": {"code": exc.code, "message": str(exc), "details": exc.details}}
            except OSError as exc:
                # The oracle does not wrap mkdir/rename/temp creation failures.
                response = {"os_error": {"type": type(exc).__name__, "message": str(exc)}}
    finally:
        if fault == "readonly":
            root.chmod(0o755)
    assert all(p.read_bytes() == data for p, data in original.items()), case["name"]
    assert not list(root.rglob(".*.wav")), case["name"]
    return normalize({**response, "written": written}, root)


def validation(root):
    requests = [None, [], {}, {"dir_path": ""}, {"dir_path": 1}, {"dir_path": "  "},
                {"dir_path": str(root / "missing")}, {"dir_path": str(root), "z": 1, "a": 2},
                {"dir_path": str(root)}, {"dir_path": f"  {root}  "},
                {"dir_path": str(root), "remove_silent_channels": True, "include_hangloose": True}]
    for key in ("remove_silent_channels", "include_hangloose"):
        requests += [{"dir_path": str(root), key: v} for v in ("true", 1, None, [], {}, False)]
    requests += [{"dir_path": str(root / "missing"), "remove_silent_channels": 1},
                 {"dir_path": str(root), "remove_silent_channels": 1, "include_hangloose": 1}]
    return [normalize({"request": request, "response": ImpulciferApplicationService._validate_output_recovery_request(request)}, root) for request in requests]


def main():
    exported = []
    with tempfile.TemporaryDirectory(prefix="impulcifer-p17-") as temporary:
        base = Path(temporary).resolve()
        probe = base / "case-probe"
        probe.write_text("probe")
        case_sensitive = not (base / "CASE-PROBE").exists()
        for case in cases():
            root = base / case["name"]
            root.mkdir()
            if case.get("requires_case_sensitive") and not case_sensitive:
                response = {"skip": "Host filesystem is case-insensitive; two names differing only in case cannot coexist."}
            else:
                response = run_case(root, case)
            request = {"directory": "$ROOT" + ("/" + case["selected"] if case["selected"] != "." else ""),
                       "include_hangloose": case["options"].get("include_hangloose", False),
                       "remove_silent_channels": case["options"].get("remove_silent_channels", False)}
            exported.append({**case, "request": request, **response})
            outcome = "SKIP" if "skip" in response else response.get("error", {}).get("code", "OS_ERROR" if "os_error" in response else "OK")
            print(f"{case['name']}: {outcome}")
        payload = {"python": sys.version.split()[0], "soundfile": sf.__version__, "libsndfile": sf.__libsndfile_version__,
                   "recipe": "4800 frames; row k=1+TRACKS.index(name): ((i*17+k*101)%8192-4096)/65536; zero rows are exact zero; see exporter for tiny/nonfinite/rounding rows",
                   "scenarios": exported, "validation": validation(base)}
    GOLDENS.mkdir(exist_ok=True)
    target = GOLDENS / "p17_recovery.json"
    target.write_text(json.dumps(payload, ensure_ascii=False, separators=(",", ":"), allow_nan=False) + "\n", encoding="utf-8")
    total = sum(p.stat().st_size for p in GOLDENS.glob("p17_*") if p.is_file())
    assert total <= 12 * 1024 * 1024, total
    print(f"Exported {len(exported)} scenarios and {len(payload['validation'])} validations; p17 fixtures {total} bytes.")


if __name__ == "__main__":
    main()
