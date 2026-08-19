"""Round-trip tests for sweep deconvolution in the IR estimator (F013)."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
import pytest
from scipy.signal import convolve

from core.impulse_response_estimator import ImpulseResponseEstimator

FS = 48_000
MIN_DURATION = 1.0


@dataclass(frozen=True)
class RoundTrip:
    estimator: ImpulseResponseEstimator
    baseline_peak: int
    decaying_ir: np.ndarray
    decaying_estimate: np.ndarray


@pytest.fixture(scope="module")
def roundtrip() -> RoundTrip:
    estimator = ImpulseResponseEstimator(min_duration=MIN_DURATION, fs=FS)
    baseline = estimator.estimate(estimator.test_signal)
    baseline_peak = int(np.argmax(np.abs(baseline)))

    direct_delay = 31
    tail_length = 600
    samples = np.arange(tail_length)
    decaying_ir = np.zeros(tail_length)
    tail_samples = samples[direct_delay:]
    decaying_ir[direct_delay:] = (
        0.08
        * np.exp(-(tail_samples - direct_delay) / 110.0)
        * np.cos(2 * np.pi * 1700 * (tail_samples - direct_delay) / FS)
    )
    decaying_ir[direct_delay] = 1.0
    recording = convolve(estimator.test_signal, decaying_ir, mode="full")
    decaying_estimate = estimator.estimate(recording)

    return RoundTrip(estimator, baseline_peak, decaying_ir, decaying_estimate)


@pytest.mark.parametrize("delay", [0, 1, 127, 1000])
def test_estimate_recovers_delayed_unit_impulse_peak(
    roundtrip: RoundTrip, delay: int
) -> None:
    system_ir = np.zeros(delay + 1)
    system_ir[delay] = 1.0
    recording = convolve(roundtrip.estimator.test_signal, system_ir, mode="full")

    estimate = roundtrip.estimator.estimate(recording)

    assert len(estimate) == len(recording)
    assert int(np.argmax(np.abs(estimate))) == roundtrip.baseline_peak + delay


def test_estimate_unit_impulse_has_low_noise_floor(roundtrip: RoundTrip) -> None:
    estimate = roundtrip.estimator.estimate(roundtrip.estimator.test_signal)
    peak = roundtrip.baseline_peak
    outside_peak = np.concatenate((estimate[: peak - 50], estimate[peak + 50 :]))

    peak_to_floor_db = 20 * np.log10(
        np.abs(estimate[peak]) / np.max(np.abs(outside_peak))
    )

    assert np.abs(estimate[peak]) == pytest.approx(1.0, abs=0.05)
    assert peak_to_floor_db > 40.0


def test_estimate_recovers_exponential_decay_waveform(roundtrip: RoundTrip) -> None:
    start = roundtrip.baseline_peak
    recovered = roundtrip.decaying_estimate[start : start + len(roundtrip.decaying_ir)]
    correlation = np.dot(recovered, roundtrip.decaying_ir) / (
        np.linalg.norm(recovered) * np.linalg.norm(roundtrip.decaying_ir)
    )

    assert int(np.argmax(np.abs(recovered))) == int(
        np.argmax(np.abs(roundtrip.decaying_ir))
    )
    assert correlation > 0.99


def test_estimate_decay_peak_dominates_far_noise_floor(roundtrip: RoundTrip) -> None:
    start = roundtrip.baseline_peak
    stop = start + len(roundtrip.decaying_ir)
    recovered = roundtrip.decaying_estimate[start:stop]
    guard = 1000
    far = np.concatenate(
        (
            roundtrip.decaying_estimate[: max(start - guard, 0)],
            roundtrip.decaying_estimate[stop + guard :],
        )
    )

    peak_to_floor_db = 20 * np.log10(
        np.max(np.abs(recovered)) / np.max(np.abs(far))
    )

    assert peak_to_floor_db > 40.0
