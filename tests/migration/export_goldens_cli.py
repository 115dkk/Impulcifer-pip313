"""P13: observe the real 2.x create_cli parser and post-processing, without DSP runs."""
from pathlib import Path
import argparse
import contextlib
import io
import json
import os
import subprocess
import sys
from unittest.mock import patch

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tests/migration/goldens"
sys.path.insert(0, str(ROOT))


def save(name, value):
    (OUT / name).write_text(
        json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False) + "\n",
        encoding="utf-8",
    )


def main():
    import impulcifer

    parsers = []
    original = argparse.ArgumentParser.parse_args

    def observe(parser, *args, **kwargs):
        parsers.append(parser)
        return original(parser, *args, **kwargs)

    def parse(argv):
        with patch.object(sys, "argv", ["impulcifer", *argv]), patch.object(
            argparse.ArgumentParser, "parse_args", observe
        ), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            try:
                return {"kwargs": impulcifer.create_cli(), "exit_code": 0}
            except ValueError as error:
                return {"error": str(error), "exit_code": 1}

    parse(["--dir_path", "measurements"])
    options = []
    for action in parsers[0]._actions:
        options.append({
            "option_strings": action.option_strings,
            "dest": action.dest,
            "help": action.help,
            "type": action.type.__name__ if action.type else None,
            "action": type(action).__name__,
            "choices": action.choices,
            "default": "SUPPRESS" if action.default == argparse.SUPPRESS else action.default,
            "required": action.required,
        })
    save("p13_options.json", options)
    all_values = {
        "dir_path": "measurements", "test_signal": "auto", "room_target": "target.csv",
        "room_mic_calibration": "mic.csv", "headphone_compensation_file": "hp.wav",
        "fs": "44100", "channel_balance": "trend", "decay": "fl:300,FR:250",
        "target_level": "-12", "fr_combination_method": "conservative",
        "specific_limit": "500", "generic_limit": "350", "tilt": "-0.5",
        "head_ms": "2.5", "mic_deviation_strength": "0.9", "vbass_freq": "200",
        "vbass_hp": "20", "vbass_polarity": "invert", "bass_boost": "6,150,0.69",
    }
    every = []
    for option in options:
        if option["dest"] in ("help", "version", "info"):
            continue
        flag = option["option_strings"][-1]
        every.append(flag)
        if option["action"] == "_StoreAction":
            every.append(all_values[option["dest"]])
    cases = [
        ("defaults", []), ("every_flag", every),
        ("bass_gain", ["--bass_boost=6"]), ("bass_shelf", ["--bass_boost=6,150,0.69"]),
        ("decay_uniform", ["--decay=300"]), ("decay_pairs", ["--decay=FL:300,FR:250"]),
        ("head", ["--c", "2.5"]), ("fs", ["--fs", "44100"]),
        ("balance", ["--channel_balance", "trend"]),
        ("vbass", ["--vbass", "--vbass_freq", "200", "--vbass_polarity", "invert"]),
        ("store_false", ["--no_room_correction", "--no_headphone_compensation", "--no_equalization"]),
        ("bad_bass", ["--bass_boost=1,2"]),
    ]
    for name, argv in cases:
        if name != "every_flag":
            argv = ["--dir_path", "measurements", *argv]
        save(f"p13_parse_{name}.json", {"argv": ["impulcifer", *argv], **parse(argv)})
    env = {**os.environ, "PYTHONIOENCODING": "utf-8", "PYTHONDONTWRITEBYTECODE": "1"}
    help_text = subprocess.run(
        [sys.executable, "-B", str(ROOT / "impulcifer.py"), "--help"],
        check=True, capture_output=True, encoding="utf-8", env=env,
    ).stdout
    (OUT / "p13_help.txt").write_text(help_text, encoding="utf-8")
    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        impulcifer._print_info()
    labels = [line.split(":", 1)[0] if ":" in line else line.split()[0]
              for line in output.getvalue().splitlines()]
    save("p13_info_keys.json", labels)
    print(f"Exported {len(options)} parser actions, 12 parse cases, help and info labels (Python {sys.version.split()[0]}).")


if __name__ == "__main__":
    main()
