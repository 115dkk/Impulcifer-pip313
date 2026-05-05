# -*- coding: utf-8 -*-

import os
import warnings
import numpy as np
from scipy import signal, fftpack
from scipy.signal.windows import hann
from scipy.interpolate import InterpolatedUnivariateSpline
from autoeq.frequency_response import FrequencyResponse
from core.impulse_response import ImpulseResponse
from core.utils import read_wav, write_wav, magnitude_response
from core.constants import SPEAKER_NAMES, SPEAKER_DELAYS, HEXADECAGONAL_TRACK_ORDER
from core.plotting.hrir_plotter import HRIRPlotter

# Python 3.14 병렬 처리 지원
try:
    from core.parallel_processing import parallel_process_dict, is_free_threaded_available

    PARALLEL_PROCESSING_AVAILABLE = True
except ImportError:
    PARALLEL_PROCESSING_AVAILABLE = False
    parallel_process_dict = None

    def is_free_threaded_available():
        return False


def _get_center_value(fr, frequency_range):
    """Calculate center value without modifying the FrequencyResponse object.

    This is an optimized version that avoids copying the entire FrequencyResponse
    object when only the center value is needed.

    Args:
        fr: FrequencyResponse object
        frequency_range: Frequency or list of two frequencies for centering

    Returns:
        The negative of the gain shift that would be applied by center()
    """
    # Create interpolator - use linear interpolation to avoid overshoot/undershoot
    k_order = 1  # Always use linear interpolation to prevent artifacts
    try:
        interpolator = InterpolatedUnivariateSpline(np.log10(fr.frequency), fr.raw, k=k_order)
    except ValueError:
        interpolator = InterpolatedUnivariateSpline(np.log10(fr.frequency), fr.raw, k=1)

    if isinstance(frequency_range, (list, np.ndarray)) and len(frequency_range) > 1:
        # Use the average of the gain values between the given frequencies
        diff = np.mean(fr.raw[np.logical_and(
            fr.frequency >= frequency_range[0],
            fr.frequency <= frequency_range[1]
        )])
    else:
        if isinstance(frequency_range, (list, np.ndarray)):
            frequency_range = frequency_range[0]
        # Use the gain value at the given frequency
        diff = interpolator(np.log10(frequency_range))

    return -diff


class HRIR(HRIRPlotter):
    def __init__(self, estimator):
        self.estimator = estimator
        self.fs = self.estimator.fs
        self.irs = dict()

    def copy(self):
        hrir = HRIR(self.estimator)
        hrir.irs = dict()
        for speaker, pair in self.irs.items():
            hrir.irs[speaker] = {
                "left": pair["left"].copy(),
                "right": pair["right"].copy(),
            }
        return hrir

    def open_recording(self, file_path, speakers, side=None, silence_length=2.0):
        """Open combined recording and splits it into separate speaker-ear pairs.

        Args:
            file_path: Path to recording file.
            speakers: Sequence of recorded speakers.
            side: Which side (ear) tracks are contained in the file if only one. "left" or "right" or None for both.
            silence_length: Length of silence used during recording in seconds.

        Returns:
            None
        """
        if self.fs != self.estimator.fs:
            raise ValueError(
                "Refusing to open recording because HRIR's sampling rate doesn't match impulse response "
                "estimator's sampling rate."
            )

        fs, recording = read_wav(file_path, expand=True)
        if fs != self.fs:
            raise ValueError(
                "Sampling rate of recording must match sampling rate of test signal."
            )

        # Debug information
        print(">>>>>>>>> Recording Analysis Debug Info:")
        print(f"  File: {file_path}")
        print(f"  Recording shape: {recording.shape}")
        print(f"  Requested speakers: {speakers}")
        print(f"  Side: {side}")
        print(f"  Silence length: {silence_length} seconds")
        print("  Estimator info:")
        print(
            f"    Length: {len(self.estimator)} samples ({len(self.estimator) / self.fs:.2f} seconds)"
        )
        print(f"    Sample rate: {self.estimator.fs} Hz")
        print(f"    Type: {type(self.estimator).__name__}")
        if (
            hasattr(self.estimator, "test_signal")
            and self.estimator.test_signal is not None
        ):
            print(
                f"    Test signal length: {len(self.estimator.test_signal)} samples ({len(self.estimator.test_signal) / self.estimator.fs:.2f} seconds)"
            )
        else:
            print("    Test signal: Not available or None")

        # Calculate expected recording length
        expected_length_with_silence = silence_length + len(self.estimator)
        print(
            f"  Expected minimum recording length: {expected_length_with_silence} samples ({expected_length_with_silence / self.fs:.2f} seconds)"
        )
        print(
            f"  Actual recording length: {recording.shape[1]} samples ({recording.shape[1] / self.fs:.2f} seconds)"
        )
        length_difference = recording.shape[1] - expected_length_with_silence
        print(
            f"  Length difference: {length_difference} samples ({length_difference / self.fs:.2f} seconds)"
        )

        if length_difference < 0:
            print(
                f"  WARNING: Recording is {abs(length_difference)} samples ({abs(length_difference) / self.fs:.2f} seconds) too short!"
            )
            print("  This could be caused by:")
            print("    1. Recording stopped too early")
            print("    2. Wrong test signal file used")
            print("    3. Estimator was created with different parameters")

        # Analyze each channel for actual content
        print("  Channel content analysis:")
        for ch in range(recording.shape[0]):
            max_val = np.max(np.abs(recording[ch, :]))
            rms_val = np.sqrt(np.mean(recording[ch, :] ** 2))
            print(
                f"    Channel {ch}: Max={max_val:.6f}, RMS={rms_val:.6f}, {'ACTIVE' if max_val > 1e-6 else 'EMPTY'}"
            )

        if silence_length * self.fs != int(silence_length * self.fs):
            raise ValueError(
                "Silence length must produce full samples with given sampling rate."
            )
        silence_length = int(silence_length * self.fs)

        # 2 tracks per speaker when side is not specified, only 1 track per speaker when it is
        tracks_k = 2 if side is None else 1
        print(f"  Tracks per speaker: {tracks_k}")

        # Number of speakers in each track
        n_columns = round(len(speakers) / (recording.shape[0] // tracks_k))
        print(f"  Calculated n_columns: {n_columns}")
        print(f"  Expected total tracks needed: {len(speakers) * tracks_k}")
        print(f"  Available tracks in recording: {recording.shape[0]}")

        # Warning if mismatch
        if len(speakers) * tracks_k > recording.shape[0]:
            print(
                f"  WARNING: Not enough tracks in recording! Need {len(speakers) * tracks_k}, have {recording.shape[0]}"
            )

        # Crop out initial silence
        recording = recording[:, silence_length:]
        print(f"  After silence crop: {recording.shape}")

        # Split sections in time to columns
        columns = []
        column_size = silence_length + len(self.estimator)
        print(f"  Column size (silence + estimator): {column_size}")
        print(f"  Estimator length: {len(self.estimator)}")
        print(f"  Available recording length after silence crop: {recording.shape[1]}")

        # Adjust column_size if it exceeds available recording length
        if column_size > recording.shape[1]:
            print(
                f"  WARNING: Calculated column_size ({column_size}) exceeds recording length ({recording.shape[1]})"
            )
            print(
                "  This suggests the recording was too short or estimator is longer than expected"
            )

            # Try to use the entire available length as a single column
            if n_columns <= 1:
                # Single column case - use all available data
                column_size = recording.shape[1]
                n_columns = 1
                print(f"  Adjusted to single column with size: {column_size}")
            else:
                # Multiple columns case - divide available length equally
                column_size = recording.shape[1] // n_columns
                print(
                    f"  Adjusted column_size to: {column_size} (divided by {n_columns} columns)"
                )
                if column_size < len(self.estimator):
                    print(
                        f"  ERROR: Even after adjustment, column_size ({column_size}) is smaller than estimator length ({len(self.estimator)})"
                    )
                    print(
                        "  This recording is too short for proper impulse response estimation"
                    )

        for i in range(n_columns):
            start_sample = i * column_size
            end_sample = min(
                (i + 1) * column_size, recording.shape[1]
            )  # Ensure we don't exceed recording length

            if end_sample > start_sample and (end_sample - start_sample) >= len(
                self.estimator
            ):
                column_data = recording[:, start_sample:end_sample]
                columns.append(column_data)
                print(
                    f"  Column {i}: samples {start_sample}-{end_sample}, shape {column_data.shape}"
                )
            else:
                print(
                    f"  Column {i}: SKIPPED - insufficient length ({end_sample - start_sample} < {len(self.estimator)})"
                )

        if not columns:
            # Try fallback options for short recordings
            print("  Attempting fallback solutions for short recording...")

            # Option 1: Reduce silence length
            if silence_length > 0:
                min_silence = int(0.5 * self.fs)  # Minimum 0.5 seconds silence
                available_for_silence = recording.shape[1] - len(self.estimator)

                if available_for_silence >= min_silence:
                    adjusted_silence = max(min_silence, available_for_silence)
                    print(
                        f"  Fallback 1: Reducing silence from {silence_length} to {adjusted_silence} samples"
                    )

                    # Recalculate with adjusted silence
                    adjusted_recording = recording[:, adjusted_silence:]
                    column_size = len(self.estimator)  # No additional silence in column

                    for i in range(n_columns):
                        start_sample = i * column_size
                        end_sample = min(
                            (i + 1) * column_size, adjusted_recording.shape[1]
                        )

                        if end_sample > start_sample and (
                            end_sample - start_sample
                        ) >= len(self.estimator):
                            column_data = adjusted_recording[:, start_sample:end_sample]
                            columns.append(column_data)
                            print(
                                f"  Fallback Column {i}: samples {start_sample}-{end_sample}, shape {column_data.shape}"
                            )
                        else:
                            print(
                                f"  Fallback Column {i}: SKIPPED - still insufficient length"
                            )

                    if columns:
                        print(
                            f"  Fallback 1 successful: Created {len(columns)} columns with reduced silence"
                        )
                        # Update the cropped recording for further processing
                        recording = adjusted_recording

            # Option 2: If still no columns, try using available length even if shorter than estimator
            if (
                not columns and recording.shape[1] > len(self.estimator) * 0.8
            ):  # At least 80% of estimator length
                print(
                    "  Fallback 2: Using available recording length even though it's shorter than estimator"
                )
                print("  WARNING: This may result in reduced impulse response quality")

                available_length = recording.shape[1]
                if n_columns == 1:
                    columns.append(recording)
                    print(
                        f"  Fallback 2: Single column with {available_length} samples"
                    )
                else:
                    # Divide equally among columns
                    column_size = available_length // n_columns
                    for i in range(n_columns):
                        start_sample = i * column_size
                        end_sample = min((i + 1) * column_size, available_length)
                        if end_sample > start_sample:
                            column_data = recording[:, start_sample:end_sample]
                            columns.append(column_data)
                            print(
                                f"  Fallback 2 Column {i}: samples {start_sample}-{end_sample}, shape {column_data.shape}"
                            )

            if not columns:
                raise ValueError(
                    f"No valid columns could be extracted even with fallback methods.\n"
                    f"Recording length ({recording.shape[1]} samples, {recording.shape[1] / self.fs:.2f}s) is too short "
                    f"for the required estimator length ({len(self.estimator)} samples, {len(self.estimator) / self.fs:.2f}s).\n"
                    f"Solutions:\n"
                    f"  1. Re-record with longer duration (minimum {(len(self.estimator) + silence_length) / self.fs:.1f}s)\n"
                    f"  2. Use a shorter test signal\n"
                    f"  3. Check if the correct test signal file was used for recording"
                )

        print(f"  Successfully created {len(columns)} columns")

        # Split each track by columns
        i = 0
        speaker_track_mapping = []

        while i < recording.shape[0]:
            for j, column in enumerate(columns):
                n = int(i // 2 * len(columns) + j)
                if n >= len(speakers):
                    print(
                        f"  Speaker index {n} exceeds speakers list length {len(speakers)} - skipping"
                    )
                    continue

                speaker = speakers[n]
                speaker_track_mapping.append(f"Track {i}: Speaker {speaker}")

                if speaker not in SPEAKER_NAMES:
                    print(f"  Skipping non-standard speaker: {speaker}")
                    continue

                if speaker not in self.irs:
                    self.irs[speaker] = dict()

                if side is None:
                    # Left first, right then
                    if i + 1 < recording.shape[0]:
                        left_data = column[i, :]
                        right_data = column[i + 1, :]

                        print(
                            f"  Processing {speaker}: Left track {i} (max={np.max(np.abs(left_data)):.6f}), Right track {i + 1} (max={np.max(np.abs(right_data)):.6f})"
                        )

                        self.irs[speaker]["left"] = ImpulseResponse(
                            self.estimator.estimate(left_data), self.fs, left_data
                        )
                        self.irs[speaker]["right"] = ImpulseResponse(
                            self.estimator.estimate(right_data), self.fs, right_data
                        )
                    else:
                        print(
                            f"  WARNING: Not enough tracks for stereo processing of {speaker}"
                        )
                else:
                    # Only the given side
                    data = column[i, :]
                    print(
                        f"  Processing {speaker} {side}: Track {i} (max={np.max(np.abs(data)):.6f})"
                    )

                    self.irs[speaker][side] = ImpulseResponse(
                        self.estimator.estimate(data), self.fs, data
                    )
            i += tracks_k

        print("  Speaker-Track mapping:")
        for mapping in speaker_track_mapping:
            print(f"    {mapping}")

        print(f"  Final processed speakers: {list(self.irs.keys())}")
        print(">>>>>>>>> Recording Analysis Complete")

    def write_wav(self, file_path, track_order=None, bit_depth=32):
        """Writes impulse responses to a WAV file

        Args:
            file_path: Path to output WAV file
            track_order: List of speaker-side names for the order of impulse responses in the output file
            bit_depth: Number of bits per sample. 16, 24 or 32

        Returns:
            None
        """
        # Duplicate speaker names as left and right side impulse response names
        if track_order is None:
            track_order = HEXADECAGONAL_TRACK_ORDER

        # Add all impulse responses to a list and save channel names
        irs = []
        ir_order = []
        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                irs.append(ir.data)
                ir_order.append(f"{speaker}-{side}")

        # Add silent tracks
        for ch in track_order:
            if ch not in ir_order:
                irs.append(np.zeros(len(irs[0])))
                ir_order.append(ch)
        irs = np.vstack(irs)

        # Sort to output order
        irs = irs[[ir_order.index(ch) for ch in track_order], :]

        # Write to file
        write_wav(file_path, self.fs, irs, bit_depth=bit_depth)

    def normalize(self, peak_target=-0.1, avg_target=None):
        """Normalizes output gain to target.

        Args:
            peak_target: Target gain of the peak in dB
            avg_target: Target gain of the mid frequencies average in dB

        Returns:
            gain: Applied normalization gain in dB
        """
        # Stack and sum all left and right ear impulse responses separately
        left = []
        right = []
        for speaker, pair in self.irs.items():
            left.append(pair["left"].data)
            right.append(pair["right"].data)

        # Filter out empty arrays before stacking
        left = [arr for arr in left if arr.size > 0]
        right = [arr for arr in right if arr.size > 0]

        if not left or not right:
            raise ValueError(
                "No valid impulse response data found for normalization. All channels appear to be empty."
            )

        # Check if all arrays have the same length
        left_lengths = [len(arr) for arr in left]
        right_lengths = [len(arr) for arr in right]

        if len(set(left_lengths)) > 1 or len(set(right_lengths)) > 1:
            # Arrays have different lengths, pad shorter ones to match the longest
            max_left_len = max(left_lengths) if left_lengths else 0
            max_right_len = max(right_lengths) if right_lengths else 0

            left = [
                np.pad(arr, (0, max_left_len - len(arr)), "constant") for arr in left
            ]
            right = [
                np.pad(arr, (0, max_right_len - len(arr)), "constant") for arr in right
            ]

        left = np.sum(np.vstack(left), axis=0)
        right = np.sum(np.vstack(right), axis=0)

        # Calculate magnitude responses
        f_l, mr_l = magnitude_response(left, self.fs)
        f_r, mr_r = magnitude_response(right, self.fs)

        if peak_target is not None and avg_target is None:
            # Maximum absolute gain from both sides
            gain = np.max(np.vstack([mr_l, mr_r])) * -1 + peak_target

        elif peak_target is None and avg_target is not None:
            # Mid frequencies average from both sides
            gain = np.mean(
                np.concatenate(
                    [
                        mr_l[np.logical_and(f_l > 80, f_l < 6000)],
                        mr_r[np.logical_and(f_r > 80, f_r < 6000)],
                    ]
                )
            )
            gain = gain * -1 + avg_target

        else:
            raise ValueError(
                'One and only one of the parameters "peak_target" and "avg_target" must be given!'
            )

        # 전체 정규화 gain만 출력 (항목 8)
        print(
            f">>>>>>>>> Applied a normalization gain of {gain:.2f} dB to all channels"
        )

        # Scale impulse responses (Python 3.14 병렬 처리 적용)
        gain_scalar = 10 ** (gain / 20)

        if PARALLEL_PROCESSING_AVAILABLE and len(self.irs) > 4:
            # 병렬 처리: 각 스피커 채널에 gain 적용
            def apply_gain_to_pair(speaker, pair):
                """각 스피커 채널에 gain을 적용"""
                for ir in pair.values():
                    ir.data *= gain_scalar
                return pair

            # 병렬 실행
            self.irs = parallel_process_dict(
                apply_gain_to_pair, self.irs, use_threads=True
            )

            if is_free_threaded_available():
                print(f"  🚀 Free-Threaded 병렬 정규화 완료 ({len(self.irs)} 채널)")
        else:
            # 순차 처리 (채널 수가 적거나 병렬 처리 모듈 없음)
            for speaker, pair in self.irs.items():
                for ir in pair.values():
                    ir.data *= gain_scalar

        return gain  # 적용된 게인 값 반환

    def crop_heads(self, head_ms=1):
        """Crops heads of impulse responses

        Args:
            head_ms: Milliseconds of head room in the beginning before impulse response max which will not be cropped

        Returns:
            None
        """
        if self.fs != self.estimator.fs:
            raise ValueError(
                "Refusing to crop heads because HRIR sampling rate doesn't match impulse response "
                "estimator's sampling rate."
            )

        for speaker, pair in self.irs.items():
            # Peaks
            peak_left = pair["left"].peak_index()
            peak_right = pair["right"].peak_index()

            # Handle cases where peak_index returns None (empty arrays)
            if peak_left is None or peak_right is None:
                print(
                    f"Warning: Could not find peaks for {speaker}. Skipping crop_heads processing for this speaker."
                )
                # Skip this speaker entirely if we can't find peaks
                continue

            itd = np.abs(peak_left - peak_right) / self.fs

            # Speaker channel delay
            head = int(head_ms * self.fs / 1000)  # PR의 head 계산 방식 (항목 4 연관)
            delay = (
                int(np.round(SPEAKER_DELAYS[speaker] * self.fs)) + head
            )  # Channel delay in samples

            if peak_left < peak_right:
                # Delay to left ear is smaller, this is must left side speaker
                if speaker[1] == "R":
                    # Speaker name indicates this is right side speaker but delay to left ear is smaller than to right.
                    # There is something wrong with the measurement
                    warnings.warn(
                        f"Warning: {speaker} measurement has lower delay to left ear than to right ear. "
                        f"{speaker} should be at the right side of the head so the sound should arrive first "
                        "in the right ear. This is usually a problem with the measurement process or the "
                        "speaker order given is not correct. Detected delay difference is "
                        f"{itd * 1000:.4f} milliseconds."
                    )
                # Crop out silence from the beginning, only required channel delay remains
                # Secondary ear has additional delay for inter aural time difference

                crop_index = max(0, peak_left - delay)
                pair["left"].data = pair["left"].data[crop_index:]
                pair["right"].data = pair["right"].data[crop_index:]
            else:
                # Delay to right ear is smaller, this is must right side speaker
                if speaker[1] == "L":
                    # Speaker name indicates this is left side speaker but delay to right ear is smaller than to left.
                    # There si something wrong with the measurement
                    warnings.warn(
                        f"Warning: {speaker} measurement has lower delay to right ear than to left ear. "
                        f"{speaker} should be at the left side of the head so the sound should arrive first "
                        "in the left ear. This is usually a problem with the measurement process or the "
                        "speaker order given is not correct. Detected delay difference is "
                        f"{itd * 1000:.4f} milliseconds."
                    )
                # Crop out silence from the beginning, only required channel delay remains
                # Secondary ear has additional delay for inter aural time difference

                crop_index = max(0, peak_right - delay)
                pair["right"].data = pair["right"].data[crop_index:]
                pair["left"].data = pair["left"].data[crop_index:]

            # Make sure impulse response starts from silence
            # Ensure we have enough data for the windowing
            if len(pair["left"].data) >= head and len(pair["right"].data) >= head:
                window = hann(head * 2)[:head]  # scipy.signal.windows.hann 사용
                pair["left"].data[:head] *= window
                pair["right"].data[:head] *= window

    def crop_tails(self):
        """Crops out tails after every impulse response has decayed to noise floor.

        Uses the Lundeby method via decay_params() to find the knee point where
        each IR reaches the noise floor, then crops all IRs to an FFT-optimized
        length based on these indices.
        """
        if self.fs != self.estimator.fs:
            raise ValueError(
                "Refusing to crop tails because HRIR sampling rate doesn't match estimator sampling rate."
            )

        # Find indices after which there is only noise in each track
        tail_indices = []
        lengths = []
        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                try:
                    _, tail_ind, _, _ = ir.decay_params()
                    tail_indices.append(tail_ind)
                except Exception:
                    # If decay_params fails, use full length as fallback
                    tail_indices.append(len(ir.data))
                lengths.append(len(ir.data))

        if not tail_indices:
            return 0

        # Crop all tracks by FFT-optimized tail index
        seconds_per_octave = len(self.estimator) / self.estimator.fs / self.estimator.n_octaves
        fade_out = 2 * int(self.fs * seconds_per_octave * (1 / 24))
        window = hann(fade_out)[fade_out // 2:]
        fft_len = fftpack.next_fast_len(max(tail_indices))
        tail_ind = min(np.min(lengths), fft_len)

        for speaker, pair in self.irs.items():
            for ir in pair.values():
                # Crop to tail_ind
                ir.data = ir.data[:tail_ind]
                # Apply fade-out window
                ir.data *= np.concatenate([np.ones(len(ir.data) - len(window)), window])

        return tail_ind

    def channel_balance_firs(self, left_fr, right_fr, method):
        """Creates FIR filters for correcting channel balance

        Args:
            left_fr: Left side FrequencyResponse instance
            right_fr: Right side FrequencyResponse instance
            method: "trend" equalizes right side by the difference trend of right and left side. "left" equalizes
                    right side to left side fr, "right" equalizes left side to right side fr, "avg" equalizes both
                    to the average fr, "min" equalizes both to the minimum of left and right side frs. Number
                    values will boost or attenuate right side relative to left side by the number of dBs. "mids" is
                    the same as the numerical values but guesses the value automatically from mid frequency levels.

        Returns:
            List of two FIR filters as numpy arrays, first for left and second for right
        """
        if method == "mids":
            # Find gain for right side
            # R diff - L diff = L mean - R mean
            gain = _get_center_value(right_fr, [100, 3000]) - _get_center_value(left_fr, [100, 3000])
            gain = 10 ** (gain / 20)
            n = int(round(self.fs * 0.1))  # 100 ms
            firs = [signal.unit_impulse(n), signal.unit_impulse(n) * gain]

        elif method == "trend":
            trend = FrequencyResponse(
                name="trend",
                frequency=left_fr.frequency,
                raw=left_fr.raw - right_fr.raw,
            )
            trend.smoothen_fractional_octave(
                window_size=2,
                treble_f_lower=20000,
                treble_f_upper=int(round(self.fs / 2)),
            )
            # Trend is the equalization target
            right_fr.equalization = trend.smoothed
            # Unit impulse for left side and equalization FIR filter for right side
            fir = right_fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            firs = [signal.unit_impulse((len(fir))), fir]

        elif method == "left" or method == "right":
            if method == "left":
                ref = left_fr
                subj = right_fr
            else:
                ref = right_fr
                subj = left_fr

            # Smoothen reference
            ref.smoothen_fractional_octave(
                window_size=1 / 3,
                treble_f_lower=20000,
                treble_f_upper=int(round(self.fs / 2)),
            )
            # Center around 0 dB
            gain = ref.center([100, 10000])
            subj.raw += gain
            # Compensate and equalize to reference
            subj.target = ref.smoothed
            subj.error = subj.raw - subj.target
            subj.smoothen_heavy_light()
            subj.equalize(max_gain=15, treble_f_lower=20000, treble_f_upper=self.fs / 2)
            # Unit impulse for left side and equalization FIR filter for right side
            fir = subj.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            if method == "left":
                firs = [signal.unit_impulse((len(fir))), fir]
            else:
                firs = [fir, signal.unit_impulse((len(fir)))]

        elif method == "avg" or method == "min":
            # Center around 0 dB
            left_gain = _get_center_value(left_fr, [100, 10000])
            right_gain = _get_center_value(right_fr, [100, 10000])
            gain = (left_gain + right_gain) / 2
            left_fr.raw += gain
            right_fr.raw += gain

            # Smoothen
            left_fr.smoothen_fractional_octave(
                window_size=1 / 3, treble_f_lower=20000, treble_f_upper=23999
            )
            right_fr.smoothen_fractional_octave(
                window_size=1 / 3, treble_f_lower=20000, treble_f_upper=23999
            )

            # Target
            if method == "avg":
                # Target is the average between the two FRs
                target = (left_fr.raw + right_fr.raw) / 2
            else:
                # Target is the  frequency-vise minimum of the two FRs
                target = np.min([left_fr.raw, right_fr.raw], axis=0)

            # Compensate and equalize both to the target
            firs = []
            for fr in [left_fr, right_fr]:
                # Optimized: No need to copy target array since it's not modified
                fr.target = target
                fr.error = fr.raw - fr.target
                fr.smoothen_fractional_octave(
                    window_size=1 / 3, treble_f_lower=20000, treble_f_upper=23999
                )
                fr.equalize(
                    max_gain=15, treble_f_lower=2000, treble_f_upper=self.fs / 2
                )
                firs.append(
                    fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
                )

        else:
            # Must be numerical value
            try:
                gain = 10 ** (float(method) / 20)
                n = int(round(self.fs * 0.1))  # 100 ms
                firs = [signal.unit_impulse(n), signal.unit_impulse(n) * gain]
            except ValueError:
                raise ValueError(
                    f'"{method}" is not valid value for channel balance method.'
                )

        return firs

    def correct_channel_balance(self, method):
        """Channel balance correction by equalizing left and right ear results to the same frequency response.

        Args:
            method: "trend" equalizes right side by the difference trend of right and left side. "left" equalizes
                    right side to left side fr, "right" equalizes left side to right side fr, "avg" equalizes both
                    to the average fr, "min" equalizes both to the minimum of left and right side frs. Number
                    values will boost or attenuate right side relative to left side by the number of dBs. "mids" is
                    the same as the numerical values but guesses the value automatically from mid frequency levels.

        Returns:
            HRIR with FIR filter for equalizing each speaker-side
        """
        # Create frequency responses for left and right side IRs
        stacks = [[], []]
        for speaker, pair in self.irs.items():
            if speaker not in ["FL", "FR"]:
                continue
            for i, ir in enumerate(pair.values()):
                stacks[i].append(ir.data)

        # Group the same left and right side speakers
        eqir = HRIR(self.estimator)
        for speakers in [["FC"], ["FL", "FR"], ["SL", "SR"], ["BL", "BR"]]:
            if len([ch for ch in speakers if ch in self.irs]) < len(speakers):
                # All the speakers in the current speaker group must exist, otherwise balancing makes no sense
                continue
            # Stack impulse responses
            left, right = [], []
            for speaker in speakers:
                left.append(self.irs[speaker]["left"].data)
                right.append(self.irs[speaker]["right"].data)
            # Create frequency responses
            left_fr = ImpulseResponse(
                np.mean(np.vstack(left), axis=0), self.fs
            ).frequency_response()
            right_fr = ImpulseResponse(
                np.mean(np.vstack(right), axis=0), self.fs
            ).frequency_response()
            # Create EQ FIR filters
            firs = self.channel_balance_firs(left_fr, right_fr, method)
            # Assign to speakers in EQ HRIR
            for speaker in speakers:
                self.irs[speaker]["left"].equalize(firs[0])
                self.irs[speaker]["right"].equalize(firs[1])

        return eqir

    def correct_microphone_deviation(
        self,
        correction_strength=0.7,
        enable_phase_correction=False,
        enable_adaptive_correction=False,
        enable_anatomical_validation=False,
        plot_analysis=False,
        plot_dir=None,
    ):
        """
        마이크 착용 편차 보정 (v2.0)

        바이노럴 임펄스 응답 측정 시 좌우 귀에 착용된 마이크의 위치/깊이 차이로 인한
        주파수 응답 편차를 보정합니다. REW의 MTW(Minimum Time Window) 개념을 활용하여
        직접음 구간만을 분석하고 보정합니다.

        v2.0 개선사항:
        - 적응형 비대칭 보정: 좌우 응답의 품질을 평가하여 더 나은 쪽을 참조로 사용
        - 위상 보정: ITD(Interaural Time Difference) 정보를 FIR 필터에 반영
        - ITD/ILD 해부학적 검증: 인간의 머리 크기로 예상되는 범위 검증
        - 주파수 대역별 보정 전략: 저주파(ITD), 중간주파(혼합), 고주파(ILD) 차별화

        Args:
            correction_strength (float): 보정 강도 (0.0~1.0). 0.0은 보정 없음, 1.0은 완전 보정
            enable_phase_correction (bool): 위상 보정 활성화 (v2.0, 기본: True)
            enable_adaptive_correction (bool): 적응형 비대칭 보정 활성화 (v2.0, 기본: True)
            enable_anatomical_validation (bool): ITD/ILD 해부학적 검증 활성화 (v2.0, 기본: True)
            plot_analysis (bool): 분석 결과 플롯 생성 여부
            plot_dir (str): 플롯 저장 디렉토리 경로

        Returns:
            dict: 각 스피커별 분석 결과
        """
        from core.microphone_deviation_correction import (
            apply_microphone_deviation_correction_to_hrir,
        )

        print("마이크 착용 편차 보정 v2.0 중...")

        # 플롯 디렉토리 설정
        if plot_analysis and plot_dir:
            mic_deviation_plot_dir = os.path.join(plot_dir, "microphone_deviation")
            os.makedirs(mic_deviation_plot_dir, exist_ok=True)
        else:
            mic_deviation_plot_dir = None

        # 보정 적용 (v2.0 파라미터 포함)
        analysis_results = apply_microphone_deviation_correction_to_hrir(
            self,
            correction_strength=correction_strength,
            enable_phase_correction=enable_phase_correction,
            enable_adaptive_correction=enable_adaptive_correction,
            enable_anatomical_validation=enable_anatomical_validation,
            plot_analysis=plot_analysis,
            plot_dir=mic_deviation_plot_dir,
        )

        # 보정 결과 요약 출력 (v3.0 flat summary dict)
        if analysis_results and not analysis_results.get('error'):
            speakers_processed = analysis_results.get('speakers_processed', [])
            avg_error = analysis_results.get('avg_error_db', 0)
            max_error = analysis_results.get('max_error_db', 0)

            print("마이크 편차 보정 완료:")
            print(f"  - 처리된 스피커: {len(speakers_processed)}개 ({', '.join(speakers_processed)})")
            print(f"  - 평균 보정량: {avg_error:.2f} dB, 최대 보정량: {max_error:.2f} dB")
        else:
            print("마이크 편차 보정: 처리된 스피커가 없습니다.")

        return analysis_results

    def equalize(self, fir):
        """Equalizes all impulse responses with given FIR filters.

        First row of the fir matrix will be used for all left side impulse responses and the second row for all right
        side impulse responses.

        Args:
            fir: FIR filter as an array like. Must have same sample rate as this HRIR instance.

        Returns:
            None
        """
        if isinstance(fir, list):
            # Turn list (list|array|ImpulseResponse) into Numpy array
            if isinstance(fir[0], np.ndarray):
                fir = np.vstack(fir)
            elif isinstance(fir[0], list):
                fir = np.array(fir)
            elif isinstance(fir[0], ImpulseResponse):
                if len(fir) > 1:
                    fir = np.vstack([fir[0].data, fir[1].data])
                else:
                    fir = fir[0].data.copy()

        if len(fir.shape) == 1 or fir.shape[0] == 1:
            # Single track in the WAV file, use it for both channels
            fir = np.tile(fir, (2, 1))

        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                ir.equalize(fir[0] if side == "left" else fir[1])

    def resample(self, fs):
        """Resamples all impulse response to the given sampling rate.

        Sets internal sampling rate to the new rate. This will disable file reading and cropping so this should be
        the last method called in the processing pipeline.

        Args:
            fs: New sampling rate in Hertz

        Returns:
            None
        """
        if PARALLEL_PROCESSING_AVAILABLE and len(self.irs) > 4:
            # 병렬 처리: 각 스피커 채널 리샘플링
            def resample_pair(speaker, pair):
                """각 스피커 채널을 리샘플링"""
                for side, ir in pair.items():
                    ir.resample(fs)
                return pair

            # 병렬 실행
            self.irs = parallel_process_dict(resample_pair, self.irs, use_threads=True)

            if is_free_threaded_available():
                print(
                    f"  🚀 Free-Threaded 병렬 리샘플링 완료 ({len(self.irs)} 채널, {self.fs}Hz → {fs}Hz)"
                )
        else:
            # 순차 처리
            for speaker, pair in self.irs.items():
                for side, ir in pair.items():
                    ir.resample(fs)

        self.fs = fs

    def align_ipsilateral_all(self, speaker_pairs=None, segment_ms=30):
        if speaker_pairs is None:
            speaker_pairs = [
                ("FL", "FR"),
                ("SL", "SR"),
                ("BL", "BR"),
                ("WL", "WR"),
                ("TFL", "TFR"),
                ("TSL", "TSR"),
                ("TBL", "TBR"),
                ("FC", "FC"),
            ]

        segment_len = int(self.fs * segment_ms / 1000)

        for sp1, sp2 in speaker_pairs:
            if sp1 not in self.irs or sp2 not in self.irs:
                continue

            if sp1 == sp2:
                data_l = self.irs[sp1]["left"].data[:segment_len]
                data_r = self.irs[sp1]["right"].data[:segment_len]
                corr = signal.correlate(data_l, data_r, mode="full")
                lags = np.arange(-len(data_l) + 1, len(data_l))
                lag = lags[np.argmax(corr)]

                if lag > 0:
                    data = self.irs[sp1]["right"].data
                    self.irs[sp1]["right"].data = np.concatenate((np.zeros(lag), data))[:len(data)]
                elif lag < 0:
                    data = self.irs[sp1]["left"].data
                    self.irs[sp1]["left"].data = np.concatenate((np.zeros(-lag), data))[:len(data)]
                continue

            data1 = self.irs[sp1]["left"].data[:segment_len]
            data2 = self.irs[sp2]["right"].data[:segment_len]
            corr = signal.correlate(data1, data2, mode="full")
            lags = np.arange(-len(data1) + 1, len(data1))
            lag = lags[np.argmax(corr)]

            if lag > 0:
                for side in ("left", "right"):
                    data = self.irs[sp2][side].data
                    self.irs[sp2][side].data = np.concatenate((np.zeros(lag), data))[:len(data)]
            elif lag < 0:
                for side in ("left", "right"):
                    data = self.irs[sp1][side].data
                    self.irs[sp1][side].data = np.concatenate((np.zeros(-lag), data))[:len(data)]

    def align_onset_groups_peak_leftref(self, groups=None):
        """Align speaker groups to FL using each group's left-channel peak.

        This mirrors the Lion virtual-bass pipeline: each speaker pair is kept
        intact while group onset is aligned from the first speaker's left ear.
        """
        if groups is None:
            groups = [
                ("FL", "FR"),
                ("SL", "SR"),
                ("BL", "BR"),
                ("WL", "WR"),
                ("TFL", "TFR"),
                ("TSL", "TSR"),
                ("TBL", "TBR"),
                ("FC",),
            ]

        def group_left_peak(group):
            speaker = group[0]
            if speaker not in self.irs or "left" not in self.irs[speaker]:
                return None
            return self.irs[speaker]["left"].peak_index()

        ref_group = ("FL", "FR")
        ref_peak = group_left_peak(ref_group)
        if ref_peak is None:
            raise RuntimeError("Cannot find FL left channel reference for onset alignment.")

        for group in groups:
            if group == ref_group:
                continue

            group_peak = group_left_peak(group)
            if group_peak is None:
                continue

            shift = group_peak - ref_peak
            for speaker in group:
                if speaker not in self.irs:
                    continue
                for side in ("left", "right"):
                    data = self.irs[speaker][side].data
                    n = len(data)
                    if shift > 0:
                        trimmed = data[shift:]
                        if len(trimmed) < n:
                            trimmed = np.pad(trimmed, (0, n - len(trimmed)))
                        self.irs[speaker][side].data = trimmed
                    elif shift < 0:
                        delay = -shift
                        self.irs[speaker][side].data = np.concatenate((np.zeros(delay), data))[:n]

    def calculate_reflection_levels(
        self,
        direct_sound_duration_ms=2,
        early_ref_start_ms=20,
        early_ref_end_ms=50,
        late_ref_start_ms=50,
        late_ref_end_ms=150,
        epsilon=1e-12,
    ):
        """Calculates early and late reflection levels relative to direct sound for all IRs.

        Args:
            direct_sound_duration_ms (float): Duration of direct sound after peak in ms.
            early_ref_start_ms (float): Start time of early reflections after peak in ms.
            early_ref_end_ms (float): End time of early reflections after peak in ms.
            late_ref_start_ms (float): Start time of late reflections after peak in ms.
            late_ref_end_ms (float): End time of late reflections after peak in ms.
            epsilon (float): Small value to avoid division by zero in log.

        Returns:
            dict: A dictionary containing reflection levels for each speaker and side.
                  Example: {\'FL\': {\'left\': {\'early_db\': -10.5, \'late_db\': -15.2}}}
        """
        reflection_data = {}
        for speaker, pair in self.irs.items():
            reflection_data[speaker] = {}
            for side, ir in pair.items():
                peak_idx = ir.peak_index()
                if peak_idx is None:
                    reflection_data[speaker][side] = {
                        "early_db": np.nan,
                        "late_db": np.nan,
                    }
                    continue

                # Convert ms to samples
                direct_end_sample = peak_idx + int(
                    direct_sound_duration_ms * self.fs / 1000
                )
                early_start_sample = peak_idx + int(early_ref_start_ms * self.fs / 1000)
                early_end_sample = peak_idx + int(early_ref_end_ms * self.fs / 1000)
                late_start_sample = peak_idx + int(late_ref_start_ms * self.fs / 1000)
                late_end_sample = peak_idx + int(late_ref_end_ms * self.fs / 1000)

                # Ensure slices are within bounds
                data_len = len(ir.data)
                direct_sound_segment = ir.data[
                    peak_idx : min(direct_end_sample, data_len)
                ]
                early_ref_segment = ir.data[
                    min(early_start_sample, data_len) : min(early_end_sample, data_len)
                ]
                late_ref_segment = ir.data[
                    min(late_start_sample, data_len) : min(late_end_sample, data_len)
                ]

                # Calculate RMS, handle potentially empty segments
                rms_direct = (
                    np.sqrt(np.mean(direct_sound_segment**2))
                    if len(direct_sound_segment) > 0
                    else epsilon
                )
                rms_early = (
                    np.sqrt(np.mean(early_ref_segment**2))
                    if len(early_ref_segment) > 0
                    else 0
                )
                rms_late = (
                    np.sqrt(np.mean(late_ref_segment**2))
                    if len(late_ref_segment) > 0
                    else 0
                )

                # Add epsilon to rms_direct before division to prevent log(0) or division by zero
                rms_direct = rms_direct if rms_direct > epsilon else epsilon

                db_early = (
                    20 * np.log10(rms_early / rms_direct + epsilon)
                    if rms_direct > 0
                    else -np.inf
                )
                db_late = (
                    20 * np.log10(rms_late / rms_direct + epsilon)
                    if rms_direct > 0
                    else -np.inf
                )

                reflection_data[speaker][side] = {
                    "early_db": db_early,
                    "late_db": db_late,
                }
        return reflection_data

