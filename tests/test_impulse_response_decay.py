"""Synthetic decay-analysis tests for ``core.impulse_response`` (audit F014)."""

from __future__ import annotations

import numpy as np
import pytest

from core.impulse_response import ImpulseResponse

FS = 48_000


def _decaying_sine(
    fs: int,
    duration_s: float,
    rt60: float,
    freq: float = 1000.0,
    floor_db: float = -90.0,
    seed: int = 0,
) -> np.ndarray:
    """Build a synthetic decay curve with an exact theoretical RT60."""
    rng = np.random.default_rng(seed)
    n = int(duration_s * fs)
    t = np.arange(n) / fs
    slope = -60.0 / rt60
    envelope = 10 ** (slope * t / 20.0)
    tone = np.cos(2 * np.pi * freq * t) * envelope
    floor_amp = 10 ** (floor_db / 20.0)
    noise = rng.standard_normal(n) * floor_amp
    return tone + noise


def _ir(data: np.ndarray, fs: int = FS) -> ImpulseResponse:
    return ImpulseResponse(np.asarray(data, dtype=np.float64), fs)


@pytest.mark.parametrize("rt60_target", [0.3, 0.6, 1.0])
def test_decay_params_and_decay_times_track_theoretical_rt60(rt60_target: float) -> None:
    data = _decaying_sine(FS, duration_s=3.0, rt60=rt60_target, floor_db=-90.0)
    ir = _ir(data)

    peak_ind, knee_ind, noise_floor, window_size = ir.decay_params()

    assert 0 <= peak_ind < len(ir)
    assert peak_ind < knee_ind <= len(ir)
    assert window_size >= 1
    assert noise_floor == pytest.approx(-90.0, abs=5.0)

    edt, rt20, rt30, rt60 = ir.decay_times(
        peak_ind, knee_ind, noise_floor, window_size
    )

    assert edt == pytest.approx(rt60_target / 6, rel=0.1)
    assert rt20 == pytest.approx(rt60_target / 3, rel=0.1)
    assert rt30 == pytest.approx(rt60_target / 2, rel=0.1)
    assert rt60 == pytest.approx(rt60_target, rel=0.1)


def test_decay_params_handles_degenerate_short_signal() -> None:
    ir = _ir(np.array([1.0, 0.5, 0.25, 0.1]))
    peak_ind, knee_ind, noise_floor, window_size = ir.decay_params()

    assert peak_ind == 0
    assert knee_ind == len(ir)
    assert noise_floor == -200.0
    assert window_size == len(ir)


def test_decay_params_handles_empty_signal() -> None:
    ir = _ir(np.array([]))
    peak_ind, knee_ind, noise_floor, window_size = ir.decay_params()

    assert peak_ind == 0
    assert knee_ind == 0
    assert noise_floor == -200.0
    assert window_size == 1


def test_adjust_decay_shortens_slow_decay_toward_target() -> None:
    rt60_slow = 1.5
    target = 0.3
    data = _decaying_sine(FS, duration_s=4.0, rt60=rt60_slow, floor_db=-90.0)
    ir = _ir(data)

    peak_ind, knee_ind, noise_floor, window_size = ir.decay_params()
    _, _, _, rt60_before = ir.decay_times(
        peak_ind, knee_ind, noise_floor, window_size
    )
    tail_start = peak_ind + int(0.5 * FS)
    tail_end = peak_ind + int(1.0 * FS)
    energy_before = np.sum(ir.data[tail_start:tail_end] ** 2)

    ir.adjust_decay(target)

    energy_after = np.sum(ir.data[tail_start:tail_end] ** 2)
    peak_ind2, knee_ind2, noise_floor2, window_size2 = ir.decay_params()
    _, _, _, rt60_after = ir.decay_times(
        peak_ind2, knee_ind2, noise_floor2, window_size2
    )

    assert rt60_before is not None and rt60_after is not None
    assert rt60_after < rt60_before
    assert abs(rt60_after - target) < abs(rt60_before - target)
    assert energy_after < energy_before * 0.01


def test_adjust_decay_is_noop_when_target_is_slower_than_actual() -> None:
    data = _decaying_sine(FS, duration_s=3.0, rt60=0.3, floor_db=-90.0)
    ir = _ir(data)
    before = ir.data.copy()

    ir.adjust_decay(target=10.0)

    assert np.array_equal(ir.data, before)


def test_decay_worker_matches_direct_adjust_decay() -> None:
    from core.parallel_workers import process_decay_worker

    data = _decaying_sine(FS, duration_s=4.0, rt60=1.5, floor_db=-90.0)
    direct = _ir(data.copy())
    params = direct.decay_adjustment_params(0.3)

    direct.adjust_decay(0.3)
    speaker, side, adjusted = process_decay_worker(
        ("FL", "left", data, params)
    )

    assert (speaker, side) == ("FL", "left")
    assert np.array_equal(adjusted, direct.data)


def test_parallel_workers_import_stays_matplotlib_free() -> None:
    import subprocess
    import sys

    result = subprocess.run(
        [
            sys.executable,
            "-c",
            "import sys; import core.parallel_workers; "
            "assert 'matplotlib' not in sys.modules",
        ],
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0, result.stderr


def test_convolve_true_identity_preserves_signal() -> None:
    rng = np.random.default_rng(1)
    x = rng.standard_normal(500)
    ir = _ir(np.array([1.0]))

    result = ir.convolve(x)

    assert len(result) == len(x)
    assert np.allclose(result, x)


def test_convolve_length_matches_full_mode_formula() -> None:
    rng = np.random.default_rng(2)
    x = rng.standard_normal(733)
    fir = rng.standard_normal(211)
    ir = _ir(fir)

    result = ir.convolve(x)

    assert len(result) == len(x) + len(fir) - 1


def test_convolve_shifted_impulse_preserves_alignment() -> None:
    rng = np.random.default_rng(3)
    x = rng.standard_normal(300)
    k = 37
    ir_length = 100
    fir = np.zeros(ir_length)
    fir[k] = 1.0
    ir = _ir(fir)

    result = ir.convolve(x)

    assert len(result) == len(x) + ir_length - 1
    assert np.allclose(result[:k], 0.0)
    assert np.allclose(result[k : k + len(x)], x)
    assert np.allclose(result[k + len(x) :], 0.0)


def test_decay_times_per_channel_dict_contract() -> None:
    channel_targets = {"left": 0.4, "right": 0.7}
    seeds = {"left": 11, "right": 22}

    results: dict[str, tuple] = {}
    for name, rt60_target in channel_targets.items():
        data = _decaying_sine(
            FS, duration_s=3.0, rt60=rt60_target, seed=seeds[name]
        )
        results[name] = _ir(data).decay_times()

    assert set(results) == set(channel_targets)

    for name, times in results.items():
        assert isinstance(times, tuple)
        assert len(times) == 4
        edt, rt20, rt30, rt60 = times

        for value in times:
            assert value is None or isinstance(value, (float, np.floating))
            if value is not None:
                assert value > 0

        assert edt is not None
        assert rt20 is not None
        assert rt30 is not None
        assert rt60 is not None
        assert edt < rt20 < rt30 < rt60
        assert rt60 == pytest.approx(channel_targets[name], rel=0.1)
