# -*- coding: utf-8 -*-

from dataclasses import dataclass
import os
import re
from typing import Optional

import numpy as np
from scipy import signal

from autoeq.frequency_response import FrequencyResponse
from core.audio_io import read_wav
from core.constants import COLORS, IR_ROOM_SPL, SPEAKER_LIST_PATTERN, SPEAKER_NAMES


@dataclass(frozen=True)
class RoomMeasurement:
    """A speaker-ear-specific room measurement found on disk."""

    file_path: str
    speakers: tuple[str, ...]
    side: Optional[str]


@dataclass(frozen=True)
class RoomMeasurementDiscovery:
    """Paths and filename metadata used by the room-correction pipeline."""

    measurements: tuple[RoomMeasurement, ...]
    generic_path: Optional[str]
    mic_calibration_path: Optional[str]
    target_path: Optional[str]
    responses_path: str


def discover_room_measurements(dir_path):
    """Discover room-correction inputs without opening any files."""
    pattern = rf'^room-{SPEAKER_LIST_PATTERN}(-(left|right))?\.wav$'
    measurements = []
    for file_name in os.listdir(dir_path):
        if re.match(pattern, file_name) is None:
            continue
        speakers_match = re.search(SPEAKER_LIST_PATTERN, file_name)
        if speakers_match is None:
            continue
        side_match = re.search(r'(left|right)', file_name)
        measurements.append(
            RoomMeasurement(
                file_path=os.path.join(dir_path, file_name),
                speakers=tuple(speakers_match[0].split(',')),
                side=side_match[0] if side_match is not None else None,
            )
        )

    generic_path = os.path.join(dir_path, 'room.wav')
    if not os.path.isfile(generic_path):
        generic_path = None

    mic_calibration_path = os.path.join(dir_path, 'room-mic-calibration.csv')
    if not os.path.isfile(mic_calibration_path):
        mic_calibration_path = os.path.join(dir_path, 'room-mic-calibration.txt')
    if not os.path.isfile(mic_calibration_path):
        mic_calibration_path = None

    target_path = os.path.join(dir_path, 'room-target.csv')
    if not os.path.isfile(target_path):
        target_path = None

    return RoomMeasurementDiscovery(
        measurements=tuple(measurements),
        generic_path=generic_path,
        mic_calibration_path=mic_calibration_path,
        target_path=target_path,
        responses_path=os.path.join(dir_path, 'room-responses.wav'),
    )


def room_correction(
        estimator,
        dir_path,
        target=None,
        mic_calibration=None,
        fr_combination_method='average',
        specific_limit=400,
        generic_limit=300,
        plot=False):
    """Corrects room acoustics

    Args:
        estimator: ImpulseResponseEstimator
        dir_path: Path to directory
        target: Path to room target response CSV file
        mic_calibration: Path to room measurement microphone calibration file
        fr_combination_method: Method for combining generic room measurment frequency responses. "average" or
                               "conservative"
        specific_limit: Upper limit in Hertz for equalization of specific room eq. Defaults to 400.
                        0 disables limit.
        generic_limit: Upper limit in Hertz for equalization of generic room eq. Defaults to 300.
                       0 disables limit.
        plot: Plot graphs?

    Returns:
        - Room Impulse Responses as HRIR or None
        - Equalization frequency responses as dict of dicts (similar to HRIR) or None
    """
    discovery = discover_room_measurements(dir_path)
    target = _open_room_target(estimator, dir_path, target, discovery)
    mic_calibration = _open_mic_calibration(estimator, dir_path, mic_calibration, discovery)
    rir = _open_room_measurements(estimator, discovery, debug=plot)
    missing = [ch for ch in SPEAKER_NAMES if ch not in rir.irs]
    room_fr, generic_raws = _open_generic_room_measurement(
        estimator,
        discovery,
        mic_calibration,
        target,
        method=fr_combination_method,
        limit=generic_limit,
        collect_raws=plot,
    )

    if not len(rir.irs) and room_fr is None:
        return None, None

    frs = dict()
    fr_axes = []
    figs = None
    if len(rir.irs):
        for speaker, pair in rir.irs.items():
            for side, ir in pair.items():
                ir.crop_head()
        rir.crop_tails()
        rir.write_wav(discovery.responses_path)

        if plot:
            plot_dir = os.path.join(dir_path, 'plots', 'room')
            os.makedirs(plot_dir, exist_ok=True)
            figs = rir.plot(plot_fr=False, close_plots=False)

        frs = calculate_specific_room_corrections(
            rir,
            target,
            mic_calibration=mic_calibration,
            limit=specific_limit,
        )

        if plot:
            for speaker, pair in rir.irs.items():
                for side, ir in pair.items():
                    file_path = os.path.join(dir_path, 'plots', 'room', f'{speaker}-{side}.png')
                    fr = frs[speaker][side].copy()
                    fr.smoothen(window_size=1/3, treble_window_size=1/3)
                    _, fr_ax = ir.plot_fr(
                        fr=fr,
                        fig=figs[speaker][side],
                        ax=figs[speaker][side].get_axes()[4],
                        plot_raw=False,
                        plot_error=False,
                        plot_file_path=file_path,
                        fix_ylim=True
                    )
                    fr_axes.append(fr_ax)

    if len(missing) > 0 and room_fr is not None:
        for speaker in missing:
            frs[speaker] = {'left': room_fr.copy(), 'right': room_fr.copy()}

    if plot:
        if room_fr is not None:
            _plot_generic_room_measurement(dir_path, room_fr, generic_raws)
        if figs is not None:
            from core.plotting_utils import save_fig_as_png, sync_axes
            import matplotlib.pyplot as plt

            room_plots_dir = os.path.join(dir_path, 'plots', 'room')
            os.makedirs(room_plots_dir, exist_ok=True)
            sync_axes(fr_axes)
            for speaker, pair in figs.items():
                for side, fig in pair.items():
                    save_fig_as_png(os.path.join(room_plots_dir, f'{speaker}-{side}.png'), fig)
                    plt.close(fig)

    return rir, frs


def calculate_specific_room_corrections(rir, target, mic_calibration=None, limit=400):
    """Calculate speaker-ear correction responses from already loaded IRs."""
    frs = dict()
    reference_gain = None
    for speaker, pair in rir.irs.items():
        frs[speaker] = dict()
        for side, ir in pair.items():
            fr = ir.frequency_response()

            if mic_calibration is not None:
                fr.raw -= mic_calibration.raw

            if reference_gain is None:
                reference_gain = fr.center([100, 10000])
            else:
                fr.raw += reference_gain

            target_adjusted = target.copy()
            target_adjusted.raw += IR_ROOM_SPL[speaker][side]
            fr.compensate(target_adjusted, min_mean_error=False)

            if limit > 0:
                _apply_correction_limit(fr, limit)

            frs[speaker][side] = fr
    return frs


def calculate_generic_room_correction(
        irs,
        target,
        mic_calibration=None,
        method='average',
        limit=1000):
    """Combine already loaded generic room IRs into one correction response."""
    room_fr, _ = _calculate_generic_room_correction(
        irs,
        target,
        mic_calibration=mic_calibration,
        method=method,
        limit=limit,
        collect_raws=False,
    )
    return room_fr


def _calculate_generic_room_correction(
        irs,
        target,
        mic_calibration=None,
        method='average',
        limit=1000,
        collect_raws=False):
    room_fr = FrequencyResponse(
        name='generic_room',
        frequency=FrequencyResponse.generate_frequencies(
            f_min=10,
            f_max=irs[0].fs / 2,
            f_step=1.01,
        ),
        raw=0,
        error=0,
        target=target.raw,
    )

    raws = []
    errors = []
    for ir in irs:
        fr = ir.frequency_response()
        if mic_calibration is not None:
            fr.raw -= mic_calibration.raw
        fr.center([100, 10000])
        room_fr.raw += fr.raw
        if collect_raws:
            raws.append(fr.copy())
        fr.compensate(target, min_mean_error=True)
        if method == 'conservative' and len(irs) > 1:
            fr.smoothen(window_size=1/3, treble_window_size=1/3)
            errors.append(fr.error_smoothed)
        else:
            errors.append(fr.error)
    room_fr.raw /= len(irs)
    errors = np.vstack(errors)

    if errors.shape[0] > 1:
        if method == 'conservative':
            mask = np.mean(errors > 0, axis=0)
            positive = mask == 1
            negative = mask == 0
            room_fr.error[positive] = np.min(errors[:, positive], axis=0)
            room_fr.error[negative] = np.max(errors[:, negative], axis=0)
            room_fr.smoothen(window_size=1 / 6, treble_window_size=1 / 6)
            room_fr.error = room_fr.error_smoothed.copy()
        elif method == 'average':
            room_fr.error = np.mean(errors, axis=0)
            room_fr.smoothen(window_size=1/3, treble_window_size=1/3)
        else:
            raise ValueError(
                f'Invalid value "{method}" for method. Supported values are "conservative" and "average"')
    else:
        room_fr.error = errors[0, :]
        room_fr.smoothen(window_size=1 / 3, treble_window_size=1 / 3)

    if limit > 0:
        _apply_correction_limit(room_fr, limit)
        room_fr.error_smoothed *= _correction_limit_mask(room_fr.frequency, limit)

    return room_fr, raws


def _correction_limit_mask(frequency, limit):
    start = np.argmax(frequency > limit / 2)
    end = np.argmax(frequency > limit)
    return np.concatenate([
        np.ones(start if start > 0 else 0),
        signal.windows.hann(end - start),
        np.zeros(len(frequency) - end)
    ])


def _apply_correction_limit(fr, limit):
    fr.error *= _correction_limit_mask(fr.frequency, limit)


def open_room_measurements(estimator, dir_path, debug=False):
    """Opens speaker-ear specific room measurements."""
    return _open_room_measurements(estimator, discover_room_measurements(dir_path), debug=debug)


def _open_room_measurements(estimator, discovery, debug=False):
    from core.hrir import HRIR

    rir = HRIR(estimator)
    for measurement in discovery.measurements:
        rir.open_recording(
            measurement.file_path,
            list(measurement.speakers),
            side=measurement.side,
            debug=debug,
        )
    return rir


def open_generic_room_measurement(estimator,
                                  dir_path,
                                  mic_calibration,
                                  target,
                                  method='average',
                                  limit=1000,
                                  plot=False):
    """Opens generic room measurment file."""
    room_fr, raws = _open_generic_room_measurement(
        estimator,
        discover_room_measurements(dir_path),
        mic_calibration,
        target,
        method=method,
        limit=limit,
        collect_raws=plot,
    )
    if plot and room_fr is not None:
        _plot_generic_room_measurement(dir_path, room_fr, raws)
    return room_fr


def _open_generic_room_measurement(
        estimator,
        discovery,
        mic_calibration,
        target,
        method='average',
        limit=1000,
        collect_raws=False):
    if discovery.generic_path is None:
        return None, []

    fs, data = read_wav(discovery.generic_path, expand=True)
    if fs != estimator.fs:
        raise ValueError(f'Sampling rate of "{discovery.generic_path}" doesn\'t match!')

    from core.impulse_response import ImpulseResponse

    irs = []
    for track in data:
        n_cols = int(round((len(track) / estimator.fs - 2) / (estimator.duration + 2)))
        for i in range(n_cols):
            start = int(2 * estimator.fs + i * (2 * estimator.fs + len(estimator)))
            end = int(start + 2 * estimator.fs + len(estimator))
            end = min(end, len(track))
            sweep = track[start:end]
            ir = ImpulseResponse(estimator.estimate(sweep), estimator.fs, sweep)
            ir.crop_head(head_ms=1)
            irs.append(ir)

    return _calculate_generic_room_correction(
        irs,
        target,
        mic_calibration=mic_calibration,
        method=method,
        limit=limit,
        collect_raws=collect_raws,
    )


def _plot_generic_room_measurement(dir_path, room_fr, raws):
    from core.plotting_utils import config_fr_axis, get_ylim, save_fig_as_png
    import matplotlib.pyplot as plt

    room_plots_dir = os.path.join(dir_path, 'plots', 'room')
    os.makedirs(room_plots_dir, exist_ok=True)

    fr = room_fr.copy()
    fr.name = 'Generic room measurement'
    fr.raw = fr.smoothed.copy()
    fr.error = fr.error_smoothed.copy()

    fig, ax = plt.subplots()
    fig.set_size_inches(15, 9)
    config_fr_axis(ax)
    ax.set_title('Generic room measurement')

    ax.plot(fr.frequency, fr.target, color=COLORS['lightpurple'], linewidth=5, label='Target')
    for raw in raws:
        raw.smoothen(window_size=1/3, treble_window_size=1/3)
        ax.plot(raw.frequency, raw.smoothed, color='grey', linewidth=0.5)
    ax.plot(fr.frequency, fr.raw, color=COLORS['blue'], label='Raw smoothed')
    ax.plot(fr.frequency, fr.error, color=COLORS['red'], label='Error smoothed')
    ax.legend()

    sl = np.logical_and(fr.frequency >= 20, fr.frequency <= 20000)
    stack = np.vstack([
        fr.raw[sl],
        fr.error[sl],
        fr.target[sl]
    ])
    ax.set_ylim(get_ylim(stack, padding=0.1))

    save_fig_as_png(os.path.join(room_plots_dir, 'room.png'), fig)
    plt.close(fig)


def open_room_target(estimator, dir_path, target=None):
    """Opens room frequency response target file."""
    return _open_room_target(estimator, dir_path, target, discover_room_measurements(dir_path))


def _open_room_target(estimator, dir_path, target, discovery):
    if target is None:
        target = discovery.target_path or os.path.join(dir_path, 'room-target.csv')
    if os.path.isfile(target):
        target = FrequencyResponse.read_csv(target)
        target.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)
        target.center()
    else:
        target = FrequencyResponse(name='room-target')
        target.raw = np.zeros(target.frequency.shape)
        target.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)
    return target


def open_mic_calibration(estimator, dir_path, mic_calibration=None):
    """Opens room measurement microphone calibration file."""
    return _open_mic_calibration(estimator, dir_path, mic_calibration, discover_room_measurements(dir_path))


def _open_mic_calibration(estimator, dir_path, mic_calibration, discovery):
    if mic_calibration is None:
        mic_calibration = discovery.mic_calibration_path
    elif not os.path.isfile(mic_calibration):
        raise FileNotFoundError(f'Room mic calibration file doesn\'t exist at "{mic_calibration}"')
    if mic_calibration is not None and os.path.isfile(mic_calibration):
        mic_calibration = FrequencyResponse.read_csv(mic_calibration)
        mic_calibration.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)
        mic_calibration.center()
    else:
        mic_calibration = None
    return mic_calibration
