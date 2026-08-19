# -*- coding: utf-8 -*-
"""core/pipeline_stages.py 헬퍼 직접 테스트 (F015).

대상: ``open_binaural_measurements``, ``create_target``, ``write_readme``,
``headphone_compensation``. 모두 실제 ``ImpulseResponseEstimator``/``HRIR``와
합성(synthetic) 스윕 녹음(``core.audio_io.write_wav``로 기록한 convention-named WAV)
을 직접 구동해 검증하며, mock을 쓰지 않는다. 하드웨어/네트워크 의존은 없다
(모든 신호는 tmp_path 아래 in-memory로 생성한 배열을 WAV로 쓴 것뿐).

estimator 스텁 관례는 ``tests/test_eqapo.py``의
``TestEqualizationIntegration._estimator()``(``SimpleNamespace(fs=FS)``)를
따르되, 실제 스윕 컨볼루션이 필요한 함수(``open_binaural_measurements``,
``write_readme``, ``headphone_compensation``)에는 진짜
``ImpulseResponseEstimator``를 아주 짧게(fs=2000Hz, min_duration=0.05s) 만들어
사용한다 — 실제 estimate()/디컨볼루션 경로를 타면서도 테스트가 빠르게 끝나도록
하기 위함이다. 실측 결과 headphone_compensation의 전체 왕복(파일 열기 →
컴펜세이션 → PNG/WAV 부작용)도 이 크기에서 즉시 끝나므로 스모크 축소는
불필요했다.
"""

from types import SimpleNamespace

import numpy as np
import pytest

from autoeq.frequency_response import FrequencyResponse
from core.audio_io import write_wav
from core.hrir import HRIR
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.pipeline_stages import (
    create_target,
    headphone_compensation,
    open_binaural_measurements,
    write_readme,
)
from i18n.localization import t

# 진짜 estimator/녹음이 필요한 테스트용 초경량 스윕 파라미터.
EST_FS = 2000
EST_MIN_DURATION = 0.05
SILENCE_SECONDS = 2.0
MARGIN = 200

# create_target 등 estimator.fs만 필요한 곳을 위한 SimpleNamespace 관례(FS).
FS = 48000


def _build_estimator():
    """실제 스윕 컨볼루션을 수행할 수 있는 초경량 ImpulseResponseEstimator."""
    return ImpulseResponseEstimator(min_duration=EST_MIN_DURATION, fs=EST_FS)


def _build_track(estimator, delay, gain, silence_n, margin=MARGIN):
    """단일 채널의 '컬럼 내부' 신호를 생성한다.

    ``_ingest_recording``이 기대하는 컬럼 레이아웃(선행 무음 + 스윕)과
    일치하도록, 컬럼 앞쪽에 ``silence_n`` 개의 0을 두고 그 뒤 ``delay``
    샘플만큼 밀어서 스윕을 배치한다. ``margin``은 delay > 0인 경우에도
    스윕 전체가 배열 안에 들어가도록 하는 여유분이다.
    """
    sweep = estimator.test_signal
    track = np.zeros(silence_n + len(sweep) + margin)
    track[silence_n + delay : silence_n + delay + len(sweep)] += gain * sweep
    return track


def _write_binaural_wav(path, estimator, channel_specs, silence_seconds=SILENCE_SECONDS):
    """convention-named 바이노럴 녹음 WAV를 합성해 기록한다.

    ``channel_specs``는 파일에 기록될 채널 순서대로 ``(delay_samples, gain)``
    튜플의 리스트다(예: "FL,FR.wav" 4채널이면 FL-left, FL-right, FR-left,
    FR-right 순). 각 채널 앞에는 파일 전체에 대한 '바깥쪽' 무음
    (``silence_seconds``)이 추가로 붙는다 — ``_ingest_recording``이 파일을
    열자마자 이 구간을 잘라내고 남은 부분을 컬럼으로 분할한다.

    기록은 ``core.utils.write_wav``(core 오디오 유틸리티)로 수행한다.
    """
    silence_n = int(silence_seconds * estimator.fs)
    outer_silence = np.zeros(silence_n)
    rows = [
        np.concatenate([outer_silence, _build_track(estimator, delay, gain, silence_n)])
        for delay, gain in channel_specs
    ]
    data = np.vstack(rows)
    write_wav(path, estimator.fs, data, bit_depth=32)


class TestOpenBinauralMeasurements:
    """open_binaural_measurements: convention-named WAV 스캔/파싱/병합."""

    def test_combined_comma_named_file_splits_into_two_speakers(self, tmp_path):
        estimator = _build_estimator()
        _write_binaural_wav(
            str(tmp_path / "FL,FR.wav"),
            estimator,
            [(0, 1.0), (0, 0.6), (0, 0.3), (0, 0.9)],
        )

        hrir = open_binaural_measurements(estimator, str(tmp_path))

        assert isinstance(hrir, HRIR)
        assert sorted(hrir.irs.keys()) == ["FL", "FR"]
        for speaker in ("FL", "FR"):
            assert set(hrir.irs[speaker].keys()) == {"left", "right"}

    def test_separate_single_speaker_files_merge_into_one_hrir(self, tmp_path):
        estimator = _build_estimator()
        _write_binaural_wav(str(tmp_path / "FL.wav"), estimator, [(0, 1.0), (0, 0.5)])
        _write_binaural_wav(str(tmp_path / "FR.wav"), estimator, [(0, 0.4), (0, 0.7)])

        hrir = open_binaural_measurements(estimator, str(tmp_path))

        assert sorted(hrir.irs.keys()) == ["FL", "FR"]
        # 각 파일의 게인이 올바른 스피커/이어로 라우팅됐는지 피크 크기로 교차검증.
        assert hrir.irs["FL"]["left"].peak_index() is not None
        assert np.max(np.abs(hrir.irs["FL"]["left"].data)) > np.max(
            np.abs(hrir.irs["FL"]["right"].data)
        )
        assert np.max(np.abs(hrir.irs["FR"]["right"].data)) > np.max(
            np.abs(hrir.irs["FR"]["left"].data)
        )

    def test_no_matching_recordings_raises_value_error(self, tmp_path):
        estimator = _build_estimator()
        (tmp_path / "random.wav").write_bytes(b"not a real wav")
        (tmp_path / "notes.txt").write_text("hello", encoding="utf-8")

        with pytest.raises(ValueError):
            open_binaural_measurements(estimator, str(tmp_path))

    def test_empty_directory_raises_value_error(self, tmp_path):
        estimator = _build_estimator()

        with pytest.raises(ValueError):
            open_binaural_measurements(estimator, str(tmp_path))

    def test_debug_flag_does_not_change_result(self, tmp_path):
        estimator = _build_estimator()
        _write_binaural_wav(str(tmp_path / "FL.wav"), estimator, [(0, 1.0), (0, 0.5)])

        quiet = open_binaural_measurements(estimator, str(tmp_path), debug=False)
        verbose = open_binaural_measurements(estimator, str(tmp_path), debug=True)

        assert quiet.irs["FL"]["left"].peak_index() == verbose.irs["FL"]["left"].peak_index()
        np.testing.assert_array_equal(
            quiet.irs["FL"]["left"].data, verbose.irs["FL"]["left"].data
        )

    def test_delay_shifts_peak_index_by_exact_sample_count(self, tmp_path):
        """ILD/ITD 검증의 전제: 합성 WAV의 delay 파라미터가 실제 IR 피크를 그만큼 옮긴다."""
        estimator = _build_estimator()
        _write_binaural_wav(str(tmp_path / "FL.wav"), estimator, [(0, 1.0), (0, 1.0)])
        _write_binaural_wav(str(tmp_path / "FR.wav"), estimator, [(7, 1.0), (7, 1.0)])

        hrir = open_binaural_measurements(estimator, str(tmp_path))

        peak_fl = hrir.irs["FL"]["left"].peak_index()
        peak_fr = hrir.irs["FR"]["left"].peak_index()
        assert peak_fr - peak_fl == 7


class TestCreateTargetFrequencyGrid:
    """create_target: 주파수 그리드가 정확히 f_min=10Hz 바닥으로 생성되는지."""

    def test_grid_matches_generate_frequencies_with_10hz_floor(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=0.0
        )
        expected = FrequencyResponse.generate_frequencies(
            f_min=10, f_max=estimator.fs / 2, f_step=1.01
        )
        np.testing.assert_allclose(target.frequency, expected)
        assert target.frequency[0] == pytest.approx(10.0)
        assert target.frequency[-1] < estimator.fs / 2

    def test_grid_is_monotonically_increasing_geometric_series(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=0.0
        )
        assert np.all(np.diff(target.frequency) > 0)
        # f_step=1.01의 등비수열이므로 연속 비율이 거의 일정해야 한다.
        ratios = target.frequency[1:] / target.frequency[:-1]
        np.testing.assert_allclose(ratios, 1.01, atol=1e-6)

    def test_raw_length_matches_frequency_length(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=3.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=1.5
        )
        assert len(target.raw) == len(target.frequency)
        assert target.name == "bass_and_tilt"


class TestCreateTargetBassBoostAndTilt:
    """create_target: bass boost/tilt 파라미터의 수치적 효과."""

    @staticmethod
    def _at(target, freq):
        idx = int(np.argmin(np.abs(target.frequency - freq)))
        return target.raw[idx]

    def test_zero_gain_and_no_tilt_is_flat(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=0.0
        )
        np.testing.assert_allclose(target.raw, 0.0, atol=1e-9)

    def test_positive_bass_boost_raises_low_end_and_flattens_high_end(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=6.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=0.0
        )
        assert self._at(target, 30) > 3.0
        assert abs(self._at(target, 10000)) < 0.01

    def test_negative_bass_boost_lowers_low_end(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=-6.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=0.0
        )
        assert self._at(target, 30) < -3.0

    def test_positive_tilt_increases_with_frequency(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=3.0
        )
        assert self._at(target, 10000) > self._at(target, 30)

    def test_negative_tilt_decreases_with_frequency(self):
        estimator = SimpleNamespace(fs=FS)
        target = create_target(
            estimator, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=-3.0
        )
        assert self._at(target, 10000) < self._at(target, 30)

    def test_bass_boost_and_tilt_combine_additively(self):
        estimator = SimpleNamespace(fs=FS)
        bass = create_target(
            estimator, bass_boost_gain=6.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=0.0
        )
        tilt = create_target(
            estimator, bass_boost_gain=0.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=3.0
        )
        combo = create_target(
            estimator, bass_boost_gain=6.0, bass_boost_fc=105.0, bass_boost_q=0.71, tilt=3.0
        )
        np.testing.assert_allclose(combo.raw, bass.raw + tilt.raw, atol=1e-9)


class TestWriteReadme:
    """write_readme: 반환값/파일 내용, 코어 필드, ITD 귀 배정."""

    def _hrir_with_itd(self, tmp_path, delay_right_samples):
        """FL 스피커 하나짜리 HRIR: left=delay 0, right=delay_right_samples.

        FL은 speaker_side()가 'left'이므로 far ear(반대쪽 귀)는 'right'다.
        write_readme는 ITD를 항상 far ear 행에만 배정해야 한다.
        """
        estimator = _build_estimator()
        _write_binaural_wav(
            str(tmp_path / "FL.wav"), estimator, [(0, 1.0), (delay_right_samples, 1.0)]
        )
        hrir = open_binaural_measurements(estimator, str(tmp_path))
        return estimator, hrir

    def test_returned_content_matches_written_file(self, tmp_path):
        estimator, hrir = self._hrir_with_itd(tmp_path, delay_right_samples=2)
        readme_path = tmp_path / "README.md"

        content = write_readme(str(readme_path), hrir, estimator.fs, estimator, applied_gain=1.23)

        assert readme_path.read_text(encoding="utf-8") == content

    def test_content_includes_core_table_fields(self, tmp_path):
        estimator, hrir = self._hrir_with_itd(tmp_path, delay_right_samples=2)
        readme_path = tmp_path / "README.md"

        content = write_readme(str(readme_path), hrir, estimator.fs, estimator, applied_gain=1.23)

        # 헤더는 로컬라이즈되지 않는 리터럴("PNR"/"ITD")이어야 하고, 스피커명은
        # 항상 영문 리터럴이다.
        assert "PNR" in content
        assert "ITD" in content
        assert "FL" in content
        assert t("cli_readme_side_left") in content
        assert t("cli_readme_side_right") in content
        # RTxx류 열 이름(RT60/RT30/RT20/EDT/RTxx) 중 하나가 헤더에 등장해야 한다.
        assert any(name in content for name in ("RT60", "RT30", "RT20", "EDT", "RTxx"))

    def test_gain_section_present_only_when_gain_given(self, tmp_path):
        estimator, hrir = self._hrir_with_itd(tmp_path, delay_right_samples=0)

        with_gain = write_readme(
            str(tmp_path / "with_gain.md"), hrir, estimator.fs, estimator, applied_gain=2.5
        )
        without_gain = write_readme(
            str(tmp_path / "without_gain.md"), hrir, estimator.fs, estimator, applied_gain=None
        )

        assert t("cli_readme_gain_title") in with_gain
        assert "2.50" in with_gain
        assert t("cli_readme_gain_title") not in without_gain

    def test_itd_is_attributed_to_the_far_ear_only(self, tmp_path):
        # delay=2 샘플, fs=2000Hz -> ITD = 2/2000*1e6 = 1000.0 us 정확히.
        estimator, hrir = self._hrir_with_itd(tmp_path, delay_right_samples=2)
        readme_path = tmp_path / "README.md"

        content = write_readme(str(readme_path), hrir, estimator.fs, estimator, applied_gain=None)

        left_label = t("cli_readme_side_left")
        right_label = t("cli_readme_side_right")
        rows = {
            line.split("|")[2].strip(): line
            for line in content.splitlines()
            if line.startswith("|") and ("FL" in line)
        }
        assert left_label in rows
        assert right_label in rows
        # far ear(오른쪽)에만 1000.0 us, 가까운 귀(왼쪽)는 0.0 us.
        assert "1000.0 us" in rows[right_label]
        assert "0.0 us" in rows[left_label]
        assert "1000.0 us" not in rows[left_label]

    def test_empty_hrir_still_produces_title_and_processed_line(self, tmp_path):
        estimator = _build_estimator()
        hrir = HRIR(estimator)  # open_recording을 한 번도 호출하지 않은 빈 HRIR
        readme_path = tmp_path / "README.md"

        content = write_readme(str(readme_path), hrir, estimator.fs, estimator, applied_gain=None)

        assert t("cli_readme_title") in content
        assert "PNR" not in content  # 테이블이 비어 있으므로 헤더도 생기지 않는다
        assert readme_path.read_text(encoding="utf-8") == content


class TestHeadphoneCompensation:
    """headphone_compensation: 헤드폰 sweep-convolution 스모크 및 반환 계약.

    실제 estimate()/compensate() 경로를 타는 완전한(축소하지 않은) 스모크
    테스트다 — fs=2000Hz/min_duration=0.05s의 초경량 estimator를 쓰면 실측상
    즉시(수 초 이내) 끝나므로, 과제에서 허용한 "너무 무거우면 축소"가
    필요하지 않았다.
    """

    def test_missing_headphones_file_returns_none_none(self, tmp_path):
        estimator = _build_estimator()

        left, right = headphone_compensation(estimator, str(tmp_path))

        assert left is None
        assert right is None

    def test_synthetic_sweep_convolution_smoke_and_return_contract(self, tmp_path):
        estimator = _build_estimator()
        _write_binaural_wav(
            str(tmp_path / "headphones.wav"),
            estimator,
            [(0, 1.0), (0, 0.6), (0, 0.3), (0, 0.9)],  # FL-left, FL-right, FR-left, FR-right
        )

        left, right = headphone_compensation(estimator, str(tmp_path))

        assert isinstance(left, FrequencyResponse)
        assert isinstance(right, FrequencyResponse)
        assert len(left.raw) == len(left.frequency) > 0
        assert len(right.raw) == len(right.frequency) > 0
        assert np.all(np.isfinite(left.raw))
        assert np.all(np.isfinite(right.raw))
        # 부작용: 응답 WAV와 플롯 PNG가 기록되어야 한다.
        assert (tmp_path / "headphone-responses.wav").exists()
        assert (tmp_path / "plots" / "headphones.png").exists()

    def test_explicit_headphone_file_path_is_honored(self, tmp_path):
        estimator = _build_estimator()
        custom_dir = tmp_path / "custom"
        custom_dir.mkdir()
        custom_path = custom_dir / "hp.wav"
        _write_binaural_wav(
            str(custom_path), estimator, [(0, 1.0), (0, 0.6), (0, 0.3), (0, 0.9)]
        )

        # headphones.wav라는 기본 이름은 dir_path에 존재하지 않으므로, 명시
        # 경로(디렉터리 -> possible_names 탐색으로 hp.wav를 찾는 경로)를 타야
        # 성공해야 한다.
        left, right = headphone_compensation(
            estimator, str(tmp_path), headphone_file_path=str(custom_dir)
        )

        assert isinstance(left, FrequencyResponse)
        assert isinstance(right, FrequencyResponse)
