# -*- coding: utf-8 -*-
"""BRIRPipeline 직접 계약 테스트 (감사 F012, 과제 D2).

실제 ``BRIRPipeline``을 생성해 스테이지 게이팅/진행률 산술을 고정하고,
48 kHz 합성 FL/FR 측정 폴더를 최소 구성으로 끝까지 처리한다. 네트워크나
오디오 하드웨어는 사용하지 않으며 모든 입출력은 ``tmp_path`` 아래에 둔다.
"""

from time import perf_counter

import numpy as np
import pytest
from scipy.signal import convolve

from core.audio_io import read_wav, write_wav
from core.constants import HESUVI_TRACK_ORDER
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.pipeline import BRIRPipeline, ProcessingConfig
from core.sweep_signal import write_sidecar
from infra.logger import get_logger, set_gui_callbacks

ESTIMATOR_FS = 48_000
ESTIMATOR_MIN_DURATION = 1.0
SILENCE_SECONDS = 2.0
RECORDING_MARGIN = 256

DEFAULT_ENABLED_STAGES = (
    "_stage_estimator",
    "_stage_room_correction",
    "_stage_headphone_compensation",
    "_stage_equalization_files",
    "_stage_target",
    "_stage_open_measurements",
    "_stage_crop_and_align",
    "_stage_write_responses",
    "_stage_equalize",
    "_stage_normalize",
    "_stage_write_readme",
    "_stage_plot_results",
    "_stage_write_brirs",
)


def _with_stage(default_stages, stage, *, after=None):
    stages = list(default_stages)
    index = stages.index(after) + 1 if after is not None else len(stages)
    stages.insert(index, stage)
    return tuple(stages)


def _without_stage(default_stages, stage):
    return tuple(name for name in default_stages if name != stage)


@pytest.mark.parametrize(
    ("config_overrides", "expected_enabled", "expected_total_steps"),
    [
        pytest.param({}, DEFAULT_ENABLED_STAGES, 11, id="defaults"),
        pytest.param(
            {"vbass": True},
            _with_stage(
                DEFAULT_ENABLED_STAGES,
                "_stage_virtual_bass",
                after="_stage_crop_and_align",
            ),
            12,
            id="virtual-bass",
        ),
        pytest.param(
            {"decay": {"FL": 300}},
            _with_stage(
                DEFAULT_ENABLED_STAGES,
                "_stage_decay",
                after="_stage_equalize",
            ),
            12,
            id="decay",
        ),
        pytest.param(
            {"do_room_correction": False},
            _without_stage(DEFAULT_ENABLED_STAGES, "_stage_room_correction"),
            10,
            id="room-correction-off",
        ),
        pytest.param(
            {"do_headphone_compensation": False},
            _without_stage(DEFAULT_ENABLED_STAGES, "_stage_headphone_compensation"),
            10,
            id="headphone-compensation-off",
        ),
        pytest.param(
            {"plot": True},
            (
                "_stage_estimator",
                "_stage_room_correction",
                "_stage_headphone_compensation",
                "_stage_equalization_files",
                "_stage_target",
                "_stage_open_measurements",
                "_stage_plot_pre",
                "_stage_crop_and_align",
                "_stage_write_responses",
                "_stage_equalize",
                "_stage_normalize",
                "_stage_write_readme",
                "_stage_plot_post",
                "_stage_plot_results",
                "_stage_plot_additional",
                "_stage_write_brirs",
            ),
            14,
            id="plots-on",
        ),
    ],
)
def test_stage_table_gates_and_progress_total(
    config_overrides,
    expected_enabled,
    expected_total_steps,
):
    """각 설정 조합의 활성 행과 활성 행 기반 진행률 총계를 고정한다."""
    pipeline = BRIRPipeline(ProcessingConfig(**config_overrides))

    table = pipeline._stage_table()
    enabled_rows = [(steps, method.__name__) for enabled, steps, method in table if enabled]

    assert tuple(name for _, name in enabled_rows) == expected_enabled
    assert sum(steps for steps, _ in enabled_rows) == expected_total_steps


def _delayed_sweep(estimator, delay, gain):
    """지연 단위 임펄스와 스윕을 컨볼브해 한 귀의 녹음을 만든다."""
    delayed_impulse = np.zeros(RECORDING_MARGIN + 1)
    delayed_impulse[delay] = gain
    return convolve(estimator.test_signal, delayed_impulse, mode="full")


def _write_synthetic_measurements(tmp_path, estimator):
    """파일 채널 순서가 FL-L, FL-R, FR-L, FR-R인 측정 WAV를 쓴다."""
    silence_samples = int(SILENCE_SECONDS * estimator.fs)
    outer_silence = np.zeros(silence_samples)
    channel_specs = (
        (0, 1.0),
        (12, 0.6),
        (12, 0.6),
        (0, 0.9),
    )
    tracks = []
    for delay, gain in channel_specs:
        recording = _delayed_sweep(estimator, delay, gain)
        column = np.concatenate([np.zeros(silence_samples), recording])
        tracks.append(np.concatenate([outer_silence, column]))

    write_wav(
        str(tmp_path / "FL,FR.wav"),
        estimator.fs,
        np.vstack(tracks),
        bit_depth=32,
    )
    write_sidecar(str(tmp_path), estimator)


def test_minimal_synthetic_pipeline_writes_finite_hesuvi_and_completes_progress(tmp_path):
    """축소 경로가 실제 합성 측정을 처리하고 진행률 100%로 끝난다."""
    estimator = ImpulseResponseEstimator(
        min_duration=ESTIMATOR_MIN_DURATION,
        fs=ESTIMATOR_FS,
    )
    _write_synthetic_measurements(tmp_path, estimator)
    config = ProcessingConfig(
        dir_path=str(tmp_path),
        do_room_correction=False,
        do_headphone_compensation=False,
        do_equalization=False,
        plot=False,
    )
    pipeline = BRIRPipeline(config)
    expected_total_steps = sum(
        steps for enabled, steps, _ in pipeline._stage_table() if enabled
    )

    progress_values = []
    logger = get_logger()
    previous_log_callback = logger.gui_callback
    previous_progress_callback = logger.progress_callback

    def collect_progress(value, _message, **_metadata):
        progress_values.append(value)

    set_gui_callbacks(
        log_callback=previous_log_callback,
        progress_callback=collect_progress,
    )
    started = perf_counter()
    try:
        pipeline.run()
    finally:
        elapsed = perf_counter() - started
        set_gui_callbacks(
            log_callback=previous_log_callback,
            progress_callback=previous_progress_callback,
        )

    output_path = tmp_path / "hesuvi.wav"
    assert output_path.is_file()
    output_fs, output = read_wav(str(output_path), expand=True)
    assert output_fs == ESTIMATOR_FS
    assert output.shape[0] == len(HESUVI_TRACK_ORDER)
    assert output.shape[1] > 0
    assert np.isfinite(output).all()

    assert len(progress_values) == expected_total_steps
    assert progress_values == sorted(progress_values)
    assert progress_values[-1] == 100
    assert logger.current_step == logger.total_steps == expected_total_steps
    assert elapsed < 30.0
