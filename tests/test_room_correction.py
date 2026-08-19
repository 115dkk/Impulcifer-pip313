from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest

from autoeq.frequency_response import FrequencyResponse
from core.impulse_response import ImpulseResponse
from core.room_correction import (
    calculate_generic_room_correction,
    discover_room_measurements,
)


@pytest.mark.parametrize(
    ("file_name", "speakers", "side"),
    [
        ("room-FL,FR-left.wav", ("FL", "FR"), "left"),
        ("room-FL,FR-right.wav", ("FL", "FR"), "right"),
        ("room-FC-left.wav", ("FC",), "left"),
        ("room-BL,SL-right.wav", ("BL", "SL"), "right"),
        ("room-TFL,TFR-left.wav", ("TFL", "TFR"), "left"),
        ("room-FL,FR.wav", ("FL", "FR"), None),
    ],
)
def test_discover_measurement_filename_cases(tmp_path, file_name, speakers, side):
    (tmp_path / file_name).touch()

    discovery = discover_room_measurements(tmp_path)

    assert len(discovery.measurements) == 1
    measurement = discovery.measurements[0]
    assert Path(measurement.file_path) == tmp_path / file_name
    assert measurement.speakers == speakers
    assert measurement.side == side


def test_discover_ignores_nonconforming_filenames(tmp_path):
    for file_name in (
        "FL,FR-left.wav",
        "room-FL,FR-center.wav",
        "room-FL,FR-left.mp3",
        "room-target.csv",
        "room-responses.wav",
        "room-invalid-left.wav",
    ):
        (tmp_path / file_name).touch()

    assert discover_room_measurements(tmp_path).measurements == ()


def test_discover_prefers_csv_mic_calibration(tmp_path):
    csv_path = tmp_path / "room-mic-calibration.csv"
    txt_path = tmp_path / "room-mic-calibration.txt"
    csv_path.touch()
    txt_path.touch()

    discovery = discover_room_measurements(tmp_path)

    assert Path(discovery.mic_calibration_path) == csv_path


def test_discover_falls_back_to_txt_mic_calibration(tmp_path):
    txt_path = tmp_path / "room-mic-calibration.txt"
    txt_path.touch()

    discovery = discover_room_measurements(tmp_path)

    assert Path(discovery.mic_calibration_path) == txt_path


def test_discover_target_generic_and_output_paths(tmp_path):
    target_path = tmp_path / "room-target.csv"
    generic_path = tmp_path / "room.wav"
    target_path.touch()
    generic_path.touch()

    discovery = discover_room_measurements(tmp_path)

    assert Path(discovery.target_path) == target_path
    assert Path(discovery.generic_path) == generic_path
    assert Path(discovery.responses_path) == tmp_path / "room-responses.wav"


def test_discover_reports_missing_optional_inputs(tmp_path):
    discovery = discover_room_measurements(tmp_path)

    assert discovery.generic_path is None
    assert discovery.mic_calibration_path is None
    assert discovery.target_path is None


def _synthetic_room_inputs():
    fs = 8_000
    n = 2_048
    impulse = np.zeros(n)
    impulse[20] = 1.0

    colored = impulse.copy()
    colored[24] = 0.8
    colored[39] = -0.45
    irs = [
        ImpulseResponse(impulse, fs),
        ImpulseResponse(colored, fs),
    ]

    frequency = irs[0].frequency_response().frequency
    target = FrequencyResponse(
        name="synthetic-target",
        frequency=frequency,
        raw=np.zeros_like(frequency),
    )
    return irs, target


def test_generic_correction_returns_frequency_response_contract():
    irs, target = _synthetic_room_inputs()

    result = calculate_generic_room_correction(irs, target, method="average", limit=0)

    assert isinstance(result, FrequencyResponse)
    assert result.name == "generic_room"
    assert result.frequency.shape == target.frequency.shape
    assert result.raw.shape == target.raw.shape
    assert result.error.shape == target.raw.shape
    assert result.error_smoothed.shape == target.raw.shape
    assert np.all(np.isfinite(result.raw))
    assert np.all(np.isfinite(result.error))


def test_generic_correction_combination_methods_take_distinct_branches():
    irs, target = _synthetic_room_inputs()

    average = calculate_generic_room_correction(irs, target, method="average", limit=0)
    conservative = calculate_generic_room_correction(
        irs,
        target,
        method="conservative",
        limit=0,
    )

    assert average.frequency.shape == conservative.frequency.shape
    assert not np.allclose(average.error, conservative.error)


def test_room_correction_import_does_not_load_matplotlib():
    script = (
        "import sys; import core.room_correction; "
        "assert not any(name == 'matplotlib' or name.startswith('matplotlib.') "
        "for name in sys.modules)"
    )

    subprocess.run([sys.executable, "-c", script], check=True)


def test_discover_demo_room_measurements_read_only():
    demo_dir = Path(__file__).resolve().parents[1] / "data" / "demo"

    discovery = discover_room_measurements(demo_dir)

    assert len(discovery.measurements) == 8
    assert {measurement.side for measurement in discovery.measurements} == {"left", "right"}
    assert {speaker for item in discovery.measurements for speaker in item.speakers} == {
        "FL",
        "FR",
        "FC",
        "BL",
        "BR",
        "SL",
        "SR",
    }
    assert Path(discovery.mic_calibration_path).name == "room-mic-calibration.txt"
    assert Path(discovery.target_path).name == "room-target.csv"
