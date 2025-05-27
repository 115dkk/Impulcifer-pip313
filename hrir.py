# -*- coding: utf-8 -*-

import os
import warnings
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from scipy import signal
from scipy.signal.windows import hann
from scipy.fft import fft, ifft, next_fast_len
from PIL import Image
from autoeq.frequency_response import FrequencyResponse
from impulse_response import ImpulseResponse
from utils import read_wav, write_wav, magnitude_response, sync_axes
from constants import SPEAKER_NAMES, SPEAKER_DELAYS, HEXADECAGONAL_TRACK_ORDER


class HRIR:
    def __init__(self, estimator):
        self.estimator = estimator
        self.fs = self.estimator.fs
        self.irs = dict()

    def copy(self):
        hrir = HRIR(self.estimator)
        hrir.irs = dict()
        for speaker, pair in self.irs.items():
            hrir.irs[speaker] = {
                'left': pair['left'].copy(),
                'right': pair['right'].copy()
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
            raise ValueError('Refusing to open recording because HRIR\'s sampling rate doesn\'t match impulse response '
                             'estimator\'s sampling rate.')

        fs, recording = read_wav(file_path, expand=True)
        if fs != self.fs:
            raise ValueError('Sampling rate of recording must match sampling rate of test signal.')

        if silence_length * self.fs != int(silence_length * self.fs):
            raise ValueError('Silence length must produce full samples with given sampling rate.')
        silence_length = int(silence_length * self.fs)

        # 2 tracks per speaker when side is not specified, only 1 track per speaker when it is
        tracks_k = 2 if side is None else 1

        # Number of speakers in each track
        n_columns = round(len(speakers) / (recording.shape[0] // tracks_k))

        # Crop out initial silence
        recording = recording[:, silence_length:]

        # Split sections in time to columns
        columns = []
        column_size = silence_length + len(self.estimator)
        for i in range(n_columns):
            columns.append(recording[:, i * column_size:(i + 1) * column_size])

        # Split each track by columns
        i = 0
        while i < recording.shape[0]:
            for j, column in enumerate(columns):
                n = int(i // 2 * len(columns) + j)
                speaker = speakers[n]
                if speaker not in SPEAKER_NAMES:
                    # Skip non-standard speakers. Useful for skipping the other sweep in center channel recording.
                    continue
                if speaker not in self.irs:
                    self.irs[speaker] = dict()
                if side is None:
                    # Left first, right then
                    self.irs[speaker]['left'] = ImpulseResponse(
                        self.estimator.estimate(column[i, :]),
                        self.fs,
                        column[i, :]
                    )
                    self.irs[speaker]['right'] = ImpulseResponse(
                        self.estimator.estimate(column[i + 1, :]),
                        self.fs,
                        column[i + 1, :]
                    )
                else:
                    # Only the given side
                    self.irs[speaker][side] = ImpulseResponse(
                        self.estimator.estimate(column[i, :]),
                        self.fs,
                        column[i, :]
                    )
            i += tracks_k

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
                ir_order.append(f'{speaker}-{side}')

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
            left.append(pair['left'].data)
            right.append(pair['right'].data)
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
            gain = np.mean(np.concatenate([
                mr_l[np.logical_and(f_l > 80, f_l < 6000)],
                mr_r[np.logical_and(f_r > 80, f_r < 6000)]
            ]))
            gain = gain * -1 + avg_target

        else:
            raise ValueError('One and only one of the parameters "peak_target" and "avg_target" must be given!')

        # 전체 정규화 gain만 출력 (항목 8)
        print(f">>>>>>>>> Applied a normalization gain of {gain:.2f} dB to all channels")

        # Scale impulse responses
        for speaker, pair in self.irs.items():
            for ir in pair.values():
                ir.data *= 10 ** (gain / 20)
        
        return gain # 적용된 게인 값 반환

    def crop_heads(self, head_ms=1):
        """Crops heads of impulse responses

        Args:
            head_ms: Milliseconds of head room in the beginning before impulse response max which will not be cropped

        Returns:
            None
        """
        if self.fs != self.estimator.fs:
            raise ValueError('Refusing to crop heads because HRIR sampling rate doesn\'t match impulse response '
                             'estimator\'s sampling rate.')

        for speaker, pair in self.irs.items():
            # Peaks
            peak_left = pair['left'].peak_index()
            peak_right = pair['right'].peak_index()
            itd = np.abs(peak_left - peak_right) / self.fs

            # Speaker channel delay
            head = int(head_ms * self.fs / 1000) # PR의 head 계산 방식 (항목 4 연관)
            delay = int(np.round(SPEAKER_DELAYS[speaker] * self.fs)) + head  # Channel delay in samples

            if peak_left < peak_right:
                # Delay to left ear is smaller, this is must left side speaker
                if speaker[1] == 'R':
                    # Speaker name indicates this is right side speaker but delay to left ear is smaller than to right.
                    # There is something wrong with the measurement
                    warnings.warn(f'Warning: {speaker} measurement has lower delay to left ear than to right ear. '
                                  f'{speaker} should be at the right side of the head so the sound should arrive first '
                                  f'in the right ear. This is usually a problem with the measurement process or the '
                                  f'speaker order given is not correct. Detected delay difference is '
                                  f'{itd * 1000:.4f} milliseconds.')
                # Crop out silence from the beginning, only required channel delay remains
                # Secondary ear has additional delay for inter aural time difference
                pair['left'].data = pair['left'].data[peak_right - delay:]
                pair['right'].data = pair['right'].data[peak_right - delay:]
            else:
                # Delay to right ear is smaller, this is must right side speaker
                if speaker[1] == 'L':
                    # Speaker name indicates this is left side speaker but delay to right ear is smaller than to left.
                    # There si something wrong with the measurement
                    warnings.warn(f'Warning: {speaker} measurement has lower delay to right ear than to left ear. '
                                  f'{speaker} should be at the left side of the head so the sound should arrive first '
                                  f'in the left ear. This is usually a problem with the measurement process or the '
                                  f'speaker order given is not correct. Detected delay difference is '
                                  f'{itd * 1000:.4f} milliseconds.')
                # Crop out silence from the beginning, only required channel delay remains
                # Secondary ear has additional delay for inter aural time difference
                pair['right'].data = pair['right'].data[peak_left - delay:]
                pair['left'].data = pair['left'].data[peak_left - delay:]

            # Make sure impulse response starts from silence
            window = hann(head * 2)[:head] # scipy.signal.windows.hann 사용
            pair['left'].data[:head] *= window
            pair['right'].data[:head] *= window

    def crop_tails(self):
        """Crops tails of all the impulse responses in a way that makes them all equal length.
        Shorter IRs will be padded with zeros. A fade-out window is applied."""
        if self.fs != self.estimator.fs:
            raise ValueError('Refusing to crop tails because HRIR sampling rate doesn\'t match estimator sampling rate.')

        lengths = []
        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                lengths.append(len(ir.data))
        
        if not lengths:
            return 0
            
        max_len = np.max(lengths)

        # 페이드 아웃 윈도우 계산 (PR의 로직 참고)
        # self.estimator가 HRIR 객체 생성 시 주입되므로 사용 가능해야 함
        # 다만, estimator의 n_octaves, low, high 속성이 ImpulseResponseEstimator에 있는지 확인 필요
        # 해당 속성이 없다면, 일반적인 짧은 페이드 아웃 시간으로 대체 (예: 5ms)
        fade_out_duration_ms = 5 # 기본 페이드 아웃 5ms
        if hasattr(self.estimator, 'n_octaves') and hasattr(self.estimator, 'low') and hasattr(self.estimator, 'high') and self.estimator.low > 0 and self.estimator.high > 0 and self.estimator.n_octaves > 0:
             try:
                # PR의 페이드 아웃 계산 시도
                seconds_per_octave = len(self.estimator) / self.estimator.fs / self.estimator.n_octaves
                fade_out_samples = 2 * int(self.fs * seconds_per_octave * (1 / 24))
             except ZeroDivisionError:
                fade_out_samples = int(self.fs * fade_out_duration_ms / 1000)
        else:
            fade_out_samples = int(self.fs * fade_out_duration_ms / 1000)
            
        if fade_out_samples <= 0:
            fade_out_samples = int(self.fs * 0.005) # 최소 5ms 보장
        if fade_out_samples > max_len // 2: # 너무 길지 않도록 조정
             fade_out_samples = max_len // 2 if max_len // 2 > 0 else 1

        window = hann(fade_out_samples * 2)[-fade_out_samples:] # 끝부분 사용
        if len(window) == 0 and fade_out_samples > 0 : # window 생성 실패 시 대비
            window = np.ones(fade_out_samples)
            
        for speaker, pair in self.irs.items():
            for ir in pair.values():
                current_len = len(ir.data)
                if current_len < max_len:
                    # 0으로 패딩하여 길이를 max_len으로 맞춤
                    padding = np.zeros(max_len - current_len)
                    ir.data = np.concatenate([ir.data, padding])
                elif current_len > max_len:
                    # 이 경우는 발생하지 않아야 하지만, 안전을 위해 자름
                    ir.data = ir.data[:max_len]
                
                # 페이드 아웃 적용 (윈도우 길이가 IR 길이보다 길면 문제 발생 가능)
                if len(ir.data) >= len(window):
                    ir.data[-len(window):] *= window
                elif len(ir.data) > 0: # IR 데이터가 있고 윈도우보다 짧으면 전체에 적용 시도 (또는 다른 처리)
                    # 간단히 끝부분만 처리하거나, 전체에 적용 (여기선 IR이 window보다 짧으므로 window를 잘라서 적용)
                    ir.data[-len(ir.data):] *= window[:len(ir.data)]
        return max_len

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
        if method == 'mids':
            # Find gain for right side
            # R diff - L diff = L mean - R mean
            gain = right_fr.copy().center([100, 3000]) - left_fr.copy().center([100, 3000])
            gain = 10 ** (gain / 20)
            n = int(round(self.fs * 0.1))  # 100 ms
            firs = [signal.unit_impulse(n), signal.unit_impulse(n) * gain]

        elif method == 'trend':
            trend = FrequencyResponse(name='trend', frequency=left_fr.frequency, raw=left_fr.raw - right_fr.raw)
            trend.smoothen_fractional_octave(
                window_size=2,
                treble_f_lower=20000,
                treble_f_upper=int(round(self.fs / 2))
            )
            # Trend is the equalization target
            right_fr.equalization = trend.smoothed
            # Unit impulse for left side and equalization FIR filter for right side
            fir = right_fr.minimum_phase_impulse_response(fs=self.fs, normalize=False)
            firs = [signal.unit_impulse((len(fir))), fir]

        elif method == 'left' or method == 'right':
            if method == 'left':
                ref = left_fr
                subj = right_fr
            else:
                ref = right_fr
                subj = left_fr

            # Smoothen reference
            ref.smoothen_fractional_octave(
                window_size=1 / 3,
                treble_f_lower=20000,
                treble_f_upper=int(round(self.fs / 2))
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
            if method == 'left':
                firs = [signal.unit_impulse((len(fir))), fir]
            else:
                firs = [fir, signal.unit_impulse((len(fir)))]

        elif method == 'avg' or method == 'min':
            # Center around 0 dB
            left_gain = left_fr.copy().center([100, 10000])
            right_gain = right_fr.copy().center([100, 10000])
            gain = (left_gain + right_gain) / 2
            left_fr.raw += gain
            right_fr.raw += gain

            # Smoothen
            left_fr.smoothen_fractional_octave(window_size=1 / 3, treble_f_lower=20000, treble_f_upper=23999)
            right_fr.smoothen_fractional_octave(window_size=1 / 3, treble_f_lower=20000, treble_f_upper=23999)

            # Target
            if method == 'avg':
                # Target is the average between the two FRs
                target = (left_fr.raw + right_fr.raw) / 2
            else:
                # Target is the  frequency-vise minimum of the two FRs
                target = np.min([left_fr.raw, right_fr.raw], axis=0)

            # Compensate and equalize both to the target
            firs = []
            for fr in [left_fr, right_fr]:
                fr.target = target.copy()
                fr.error = fr.raw - fr.target
                fr.smoothen_fractional_octave(window_size=1 / 3, treble_f_lower=20000, treble_f_upper=23999)
                fr.equalize(max_gain=15, treble_f_lower=2000, treble_f_upper=self.fs / 2)
                firs.append(fr.minimum_phase_impulse_response(fs=self.fs, normalize=False))

        else:
            # Must be numerical value
            try:
                gain = 10 ** (float(method) / 20)
                n = int(round(self.fs * 0.1))  # 100 ms
                firs = [signal.unit_impulse(n), signal.unit_impulse(n) * gain]
            except ValueError:
                raise ValueError(f'"{method}" is not valid value for channel balance method.')

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
            if speaker not in ['FL', 'FR']:
                continue
            for i, ir in enumerate(pair.values()):
                stacks[i].append(ir.data)

        # Group the same left and right side speakers
        eqir = HRIR(self.estimator)
        for speakers in [['FC'], ['FL', 'FR'], ['SL', 'SR'], ['BL', 'BR']]:
            if len([ch for ch in speakers if ch in self.irs]) < len(speakers):
                # All the speakers in the current speaker group must exist, otherwise balancing makes no sense
                continue
            # Stack impulse responses
            left, right = [], []
            for speaker in speakers:
                left.append(self.irs[speaker]['left'].data)
                right.append(self.irs[speaker]['right'].data)
            # Create frequency responses
            left_fr = ImpulseResponse(np.mean(np.vstack(left), axis=0), self.fs).frequency_response()
            right_fr = ImpulseResponse(np.mean(np.vstack(right), axis=0), self.fs).frequency_response()
            # Create EQ FIR filters
            firs = self.channel_balance_firs(left_fr, right_fr, method)
            # Assign to speakers in EQ HRIR
            for speaker in speakers:
                self.irs[speaker]['left'].equalize(firs[0])
                self.irs[speaker]['right'].equalize(firs[1])

        return eqir

    def plot(self,
             dir_path=None,
             plot_recording=True,
             plot_spectrogram=True,
             plot_ir=True,
             plot_fr=True,
             plot_decay=True,
             plot_waterfall=True,
             close_plots=True):
        """Plots all impulse responses."""
        # Plot and save max limits
        figs = dict()
        for speaker, pair in self.irs.items():
            if speaker not in figs:
                figs[speaker] = dict()
            for side, ir in pair.items():
                fig = ir.plot(
                    plot_recording=plot_recording,
                    plot_spectrogram=plot_spectrogram,
                    plot_ir=plot_ir,
                    plot_fr=plot_fr,
                    plot_decay=plot_decay,
                    plot_waterfall=plot_waterfall
                )
                fig.suptitle(f'{speaker}-{side}')
                figs[speaker][side] = fig

        # Synchronize axes limits
        plot_flags = [plot_recording, plot_ir, plot_decay, plot_spectrogram, plot_fr, plot_waterfall]
        for r in range(2):
            for c in range(3):
                if not plot_flags[r * 3 + c]:
                    continue
                axes = []
                for speaker, pair in figs.items():
                    for side, fig in pair.items():
                        axes.append(fig.get_axes()[r * 3 + c])
                sync_axes(axes)

        # Show write figures to files
        if dir_path is not None:
            os.makedirs(dir_path, exist_ok=True)
            for speaker, pair in self.irs.items():
                for side, ir in pair.items():
                    file_path = os.path.join(dir_path, f'{speaker}-{side}.png')
                    figs[speaker][side].savefig(file_path, bbox_inches='tight')
                    # Optimize file size
                    im = Image.open(file_path)
                    im = im.convert('P', palette=Image.ADAPTIVE, colors=60)
                    im.save(file_path, optimize=True)

        # Close plots
        if close_plots:
            for speaker, pair in self.irs.items():
                for side, ir in pair.items():
                    plt.close(figs[speaker][side])

        return figs

    def plot_result(self, dir_path):
        """Plot left and right side results with all impulse responses stacked

        Args:
            dir_path: Path to directory for saving the figure

        Returns:
            None
        """
        stacks = [[], []]
        for speaker, pair in self.irs.items():
            for i, ir in enumerate(pair.values()):
                stacks[i].append(ir.data)
        left = ImpulseResponse(np.sum(np.vstack(stacks[0]), axis=0), self.fs)
        left_fr = left.frequency_response()
        left_fr.smoothen(window_size=1/3, treble_window_size=1/5, treble_f_lower=20000, treble_f_upper=23999)
        right = ImpulseResponse(np.sum(np.vstack(stacks[1]), axis=0), self.fs)
        right_fr = right.frequency_response()
        right_fr.smoothen(window_size=1/3, treble_window_size=1/5, treble_f_lower=20000, treble_f_upper=23999)

        fig, ax = plt.subplots()
        fig.set_size_inches(12, 9)
        left.plot_fr(fig=fig, ax=ax, fr=left_fr, plot_raw=True, raw_color='#7db4db', plot_smoothed=False)
        right.plot_fr(fig=fig, ax=ax, fr=right_fr, plot_raw=True, raw_color='#dd8081', plot_smoothed=False)
        left.plot_fr(fig=fig, ax=ax, fr=left_fr, plot_smoothed=True, smoothed_color='#1f77b4', plot_raw=False)
        right.plot_fr(fig=fig, ax=ax, fr=right_fr, plot_smoothed=True, smoothed_color='#d62728', plot_raw=False)
        ax.plot(left_fr.frequency, left_fr.smoothed - right_fr.smoothed, color='#680fb9')
        ax.legend(['Left raw', 'Right raw', 'Left smoothed', 'Right smoothed', 'Difference'])

        # Save figures
        file_path = os.path.join(dir_path, f'results.png')
        fig.savefig(file_path, bbox_inches='tight')
        plt.close(fig)
        # Optimize file size
        im = Image.open(file_path)
        im = im.convert('P', palette=Image.ADAPTIVE, colors=60)
        im.save(file_path, optimize=True)

    def equalize(self, fir):
        """Equalizes all impulse responses with given FIR filters.

        First row of the fir matrix will be used for all left side impulse responses and the second row for all right
        side impulse responses.

        Args:
            fir: FIR filter as an array like. Must have same sample rate as this HRIR instance.

        Returns:
            None
        """
        if type(fir) == list:
            # Turn list (list|array|ImpulseResponse) into Numpy array
            if type(fir[0]) == np.ndarray:
                fir = np.vstack(fir)
            elif type(fir[0]) == list:
                fir = np.array(fir)
            elif type(fir[0]) == ImpulseResponse:
                if len(fir) > 1:
                    fir = np.vstack([fir[0].data, fir[1].data])
                else:
                    fir = fir[0].data.copy()

        if len(fir.shape) == 1 or fir.shape[0] == 1:
            # Single track in the WAV file, use it for both channels
            fir = np.tile(fir, (2, 1))

        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                ir.equalize(fir[0] if side == 'left' else fir[1])

    def resample(self, fs):
        """Resamples all impulse response to the given sampling rate.

        Sets internal sampling rate to the new rate. This will disable file reading and cropping so this should be
        the last method called in the processing pipeline.

        Args:
            fs: New sampling rate in Hertz

        Returns:
            None
        """
        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                ir.resample(fs)
        self.fs = fs

    def align_ipsilateral_all(self,
                              speaker_pairs=None,
                              segment_ms=30):
        """Aligns ipsilateral ear impulse responses for all speaker pairs to the earliest one.

        Best results are achieved when the impulse responses are already cropped fairly well.
        This means that there is no silence in the beginning of any of the impulse responses which is longer than
        the true delay caused by the distance from speaker to ear.

        Args:
            speaker_pairs: List of speaker pairs to align. Each speaker pair is a list of two speakers, eg. [['FL', 'FR'], ['SL', 'SR']]. Default None aligns all available L/R pairs.
            segment_ms: Length of the segment from impulse response peak to be used for cross-correlation in milliseconds
        """
        if speaker_pairs is None:
            speaker_pairs = []
            for i in range(len(SPEAKER_NAMES) // 2):
                speaker_pairs.append(SPEAKER_NAMES[i*2:i*2+2])

        segment_len = int(self.fs / 1000 * segment_ms)

        for pair_speakers in speaker_pairs:
            # Skip if either one of the pair is not found
            if pair_speakers[0] not in self.irs or pair_speakers[1] not in self.irs:
                continue

            # Left side speakers, left ear
            # Right side speakers, right ear
            # Center channel speakers skip (FC)
            if pair_speakers[0].endswith('L'):
                # Left side speaker pair
                ir_a = self.irs[pair_speakers[0]]['left']
                ir_b = self.irs[pair_speakers[1]]['left']
            elif pair_speakers[0].endswith('R'):
                # Right side speaker pair
                ir_a = self.irs[pair_speakers[0]]['right']
                ir_b = self.irs[pair_speakers[1]]['right']
            else:
                # Must be FC, skip
                continue

            # Cross correlate selected segments
            # Peak indices
            peak_a = ir_a.peak_index()
            peak_b = ir_b.peak_index()
            # Segments from peaks
            segment_a = ir_a.data[peak_a:peak_a + segment_len]
            segment_b = ir_b.data[peak_b:peak_b + segment_len]
            # Cross correlation
            corr = signal.correlate(segment_a, segment_b, mode='full')
            # Delay from peak b to peak a in samples
            delay = np.argmax(corr) - (len(segment_b) - 1)  # delay = peak_a - peak_b

            # peak_b + delay = peak_a
            # Corrected peak_b is at the same position as peak_a
            # If delay is positive, peak_a is further than peak_b --> shift b forward by delay amount
            # If delay is negative, peak_a is closer than peak_b --> shift b backward by delay amount
            if delay > 0:
                # B is earlier than A, pad B from beginning
                ir_b.data = np.concatenate([np.zeros(delay), ir_b.data])
            else:
                # A is earlier than B or same, pad A from beginning
                ir_a.data = np.concatenate([np.zeros(np.abs(delay)), ir_a.data])

    def calculate_reflection_levels(self, direct_sound_duration_ms=2, early_ref_start_ms=20, early_ref_end_ms=50, late_ref_start_ms=50, late_ref_end_ms=150, epsilon=1e-12):
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
                    reflection_data[speaker][side] = {'early_db': np.nan, 'late_db': np.nan}
                    continue

                # Convert ms to samples
                direct_end_sample = peak_idx + int(direct_sound_duration_ms * self.fs / 1000)
                early_start_sample = peak_idx + int(early_ref_start_ms * self.fs / 1000)
                early_end_sample = peak_idx + int(early_ref_end_ms * self.fs / 1000)
                late_start_sample = peak_idx + int(late_ref_start_ms * self.fs / 1000)
                late_end_sample = peak_idx + int(late_ref_end_ms * self.fs / 1000)

                # Ensure slices are within bounds
                data_len = len(ir.data)
                direct_sound_segment = ir.data[peak_idx : min(direct_end_sample, data_len)]
                early_ref_segment = ir.data[min(early_start_sample, data_len) : min(early_end_sample, data_len)]
                late_ref_segment = ir.data[min(late_start_sample, data_len) : min(late_end_sample, data_len)]

                # Calculate RMS, handle potentially empty segments
                rms_direct = np.sqrt(np.mean(direct_sound_segment**2)) if len(direct_sound_segment) > 0 else epsilon
                rms_early = np.sqrt(np.mean(early_ref_segment**2)) if len(early_ref_segment) > 0 else 0
                rms_late = np.sqrt(np.mean(late_ref_segment**2)) if len(late_ref_segment) > 0 else 0
                
                # Add epsilon to rms_direct before division to prevent log(0) or division by zero
                rms_direct = rms_direct if rms_direct > epsilon else epsilon

                db_early = 20 * np.log10(rms_early / rms_direct + epsilon) if rms_direct > 0 else -np.inf
                db_late = 20 * np.log10(rms_late / rms_direct + epsilon) if rms_direct > 0 else -np.inf
                
                reflection_data[speaker][side] = {
                    'early_db': db_early,
                    'late_db': db_late
                }
        return reflection_data

    def plot_interaural_impulse_overlay(self, dir_path, time_range_ms=(-5, 30)):
        """Plots interaural impulse response overlay for each speaker.

        Args:
            dir_path (str): Path to directory for saving the figures.
            time_range_ms (tuple): Time range for the plot in milliseconds, relative to the peak.
        """
        os.makedirs(dir_path, exist_ok=True)
        sns.set_theme(style="whitegrid") # Seaborn 스타일 적용

        for speaker, pair in self.irs.items():
            fig, ax = plt.subplots(figsize=(12, 7))
            
            ir_left = pair.get('left')
            ir_right = pair.get('right')

            if not ir_left or not ir_right:
                plt.close(fig)
                continue

            # Find the peak of the earlier channel to align
            peak_idx_left = ir_left.peak_index() if ir_left else None
            peak_idx_right = ir_right.peak_index() if ir_right else None

            if peak_idx_left is None or peak_idx_right is None:
                plt.close(fig)
                continue
                
            # 기준 피크 설정 (더 일찍 도달하는 채널의 피크 또는 좌측 채널 피크)
            # 여기서는 설명을 위해 좌측 채널 피크를 기준으로 하지만, 실제로는 더 복잡한 정렬이 필요할 수 있음
            # 혹은, 각 채널의 피크를 0으로 맞추고 상대적인 시간차(ITD)를 고려하여 플롯할 수도 있음
            # 지금은 각 IR의 피크를 중심으로 플롯 범위를 설정합니다.
            
            max_val = 0 # Y축 스케일 조정을 위해

            for side, ir_obj in [('left', ir_left), ('right', ir_right)]:
                if not ir_obj:
                    continue
                
                peak_idx = ir_obj.peak_index()
                if peak_idx is None:
                    continue

                start_sample = peak_idx + int(time_range_ms[0] * self.fs / 1000)
                end_sample = peak_idx + int(time_range_ms[1] * self.fs / 1000)

                start_sample = max(0, start_sample)
                end_sample = min(len(ir_obj.data), end_sample)

                if start_sample >= end_sample:
                    continue

                segment = ir_obj.data[start_sample:end_sample]
                time_axis = np.linspace(time_range_ms[0] + (start_sample - (peak_idx + int(time_range_ms[0] * self.fs / 1000))) * 1000 / self.fs, 
                                        time_range_ms[0] + (end_sample - (peak_idx + int(time_range_ms[0] * self.fs / 1000)) -1) * 1000 / self.fs, 
                                        num=len(segment))
                
                # Normalize segment for better visualization if desired, or use raw data
                # segment_normalized = segment / (np.max(np.abs(segment)) + 1e-9)
                # sns.lineplot(x=time_axis, y=segment_normalized, label=f'{side.capitalize()} Ear')
                sns.lineplot(x=time_axis, y=segment, label=f'{side.capitalize()} Ear')
                max_val = max(max_val, np.max(np.abs(segment)))

            ax.set_title(f'{speaker} - Interaural Impulse Response Overlay')
            ax.set_xlabel('Time relative to peak (ms)')
            ax.set_ylabel('Amplitude')
            if max_val > 0:
                ax.set_ylim(-max_val*1.1, max_val*1.1)
            ax.legend()
            ax.grid(True)

            plot_file_path = os.path.join(dir_path, f'{speaker}_interaural_overlay.png')
            try:
                fig.savefig(plot_file_path, bbox_inches='tight')
                im = Image.open(plot_file_path)
                im = im.convert('P', palette=Image.ADAPTIVE, colors=128) # 색상 수 조정 가능
                im.save(plot_file_path, optimize=True)
            except Exception as e:
                print(f"Error saving/optimizing image {plot_file_path}: {e}")
            finally:
                plt.close(fig)

    def plot_ild(self, dir_path, freq_bands=None):
        """Plots Interaural Level Difference (ILD) for each speaker.

        Args:
            dir_path (str): Path to directory for saving the figures.
            freq_bands (list of tuples, optional): List of frequency bands for ILD calculation,
                                                  e.g., [(100, 500), (500, 2000), (2000, 8000)].
                                                  Defaults to octave bands if None.
        """
        os.makedirs(dir_path, exist_ok=True)
        sns.set_theme(style="whitegrid")

        if freq_bands is None:
            # Default to octave bands (approximate centers)
            octave_centers = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
            freq_bands = []
            for center in octave_centers:
                lower = center / (2**(1/2))
                upper = center * (2**(1/2))
                if upper > self.fs / 2:
                    upper = self.fs / 2
                if lower < upper: # Ensure lower is less than upper before adding
                    freq_bands.append((lower, upper))
                if upper >= self.fs / 2: # Stop if upper bound reaches Nyquist
                    break
        
        unique_freq_bands_str = [f"{int(fb[0])}-{int(fb[1])}Hz" for fb in freq_bands]


        for speaker, pair in self.irs.items():
            ir_left = pair.get('left')
            ir_right = pair.get('right')

            if not ir_left or not ir_right:
                continue

            ild_values = []
            for f_low, f_high in freq_bands:
                if f_high > self.fs / 2: 
                    f_high = self.fs / 2
                if f_low >= f_high: # If band is invalid after adjustment
                    ild_values.append(np.nan)
                    continue

                fft_len = next_fast_len(max(len(ir_left.data), len(ir_right.data)))
                
                # Ensure data is 1D for FFT
                data_left = ir_left.data.squeeze()
                data_right = ir_right.data.squeeze()
                if data_left.ndim > 1 or data_right.ndim > 1:
                    print(f"Warning: Skipping ILD for {speaker} due to unexpected IR data dimensions.")
                    ild_values.append(np.nan) # Or handle differently
                    continue


                fft_left_full = fft(data_left, n=fft_len)
                fft_right_full = fft(data_right, n=fft_len)
                
                freqs = np.fft.fftfreq(fft_len, d=1/self.fs)

                band_indices = np.where((freqs >= f_low) & (freqs < f_high))[0]

                if len(band_indices) == 0:
                    ild_values.append(np.nan)
                    continue
                    
                power_left = np.sum(np.abs(fft_left_full[band_indices])**2)
                power_right = np.sum(np.abs(fft_right_full[band_indices])**2)

                if power_right == 0: 
                    ild = np.nan if power_left == 0 else np.inf 
                else:
                    ild = 10 * np.log10((power_left + 1e-12) / (power_right + 1e-12)) # Epsilon in numerator as well
                ild_values.append(ild)

            # Skip plot if no valid ILD data
            if not ild_values or all(np.isnan(v) for v in ild_values): 
                continue

            fig, ax = plt.subplots(figsize=(10, 6))
            # Filter out NaN values for plotting if seaborn struggles with them directly in barplot
            valid_indices = [i for i, v in enumerate(ild_values) if not np.isnan(v)]
            
            # valid_indices가 비어있는 경우에 대한 추가 방어 코드
            if not valid_indices:
                 plt.close(fig) # fig는 위에서 선언되었으므로 안전하게 호출 가능
                 continue
            
            plot_bands_str = [unique_freq_bands_str[i] for i in valid_indices]
            plot_ild_values = [ild_values[i] for i in valid_indices]

            sns.barplot(x=plot_bands_str, y=plot_ild_values, ax=ax, palette="vlag")
            ax.set_title(f'{speaker} - Interaural Level Difference (ILD)')
            ax.set_xlabel('Frequency Band (Hz)')
            ax.set_ylabel('ILD (dB, Left/Right)')
            ax.axhline(0, color='grey', linestyle='--')
            plt.xticks(rotation=45, ha="right")
            plt.tight_layout()

            plot_file_path = os.path.join(dir_path, f'{speaker}_ild.png')
            try:
                fig.savefig(plot_file_path, bbox_inches='tight')
                im = Image.open(plot_file_path)
                im = im.convert('P', palette=Image.ADAPTIVE, colors=128)
                im.save(plot_file_path, optimize=True)
            except Exception as e:
                print(f"Error saving/optimizing image {plot_file_path}: {e}")
            finally:
                plt.close(fig)

    def plot_ipd(self, dir_path, freq_bands=None, unwrap_phase=True):
        """Plots Interaural Phase Difference (IPD) for each speaker.

        Args:
            dir_path (str): Path to directory for saving the figures.
            freq_bands (list of tuples, optional): List of frequency bands for IPD calculation.
                                                  Defaults to octave bands if None.
            unwrap_phase (bool): Whether to unwrap phase differences to avoid 2*pi jumps.
        """
        os.makedirs(dir_path, exist_ok=True)
        sns.set_theme(style="whitegrid")

        if freq_bands is None:
            octave_centers = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
            freq_bands = []
            for center in octave_centers:
                lower = center / (2**(1/2))
                upper = center * (2**(1/2))
                if upper > self.fs / 2:
                    upper = self.fs / 2
                if lower < upper:
                    freq_bands.append((lower, upper))
                if upper >= self.fs / 2:
                    break
        
        unique_freq_bands_str = [f"{int(fb[0])}-{int(fb[1])}Hz" for fb in freq_bands]

        for speaker, pair in self.irs.items():
            ir_left = pair.get('left')
            ir_right = pair.get('right')

            if not ir_left or not ir_right:
                continue

            ipd_values = []
            for f_low, f_high in freq_bands:
                if f_high > self.fs / 2:
                    f_high = self.fs / 2
                if f_low >= f_high:
                    ipd_values.append(np.nan)
                    continue

                fft_len = next_fast_len(max(len(ir_left.data), len(ir_right.data)))
                data_left = ir_left.data.squeeze()
                data_right = ir_right.data.squeeze()

                if data_left.ndim > 1 or data_right.ndim > 1:
                    ipd_values.append(np.nan)
                    continue

                fft_left_full = fft(data_left, n=fft_len)
                fft_right_full = fft(data_right, n=fft_len)
                freqs = np.fft.fftfreq(fft_len, d=1/self.fs)
                
                band_indices = np.where((freqs >= f_low) & (freqs < f_high))[0]
                if not len(band_indices):
                    ipd_values.append(np.nan)
                    continue
                
                complex_sum_left = np.sum(fft_left_full[band_indices])
                complex_sum_right = np.sum(fft_right_full[band_indices])

                phase_left = np.angle(complex_sum_left)
                phase_right = np.angle(complex_sum_right)
                
                ipd = phase_left - phase_right
                
                if unwrap_phase:
                    ipd = (ipd + np.pi) % (2 * np.pi) - np.pi 

                ipd_values.append(np.degrees(ipd))

            if not ipd_values or all(np.isnan(v) for v in ipd_values):
                continue

            fig, ax = plt.subplots(figsize=(10, 6))
            valid_indices = [i for i, v in enumerate(ipd_values) if not np.isnan(v)]
            if not valid_indices:
                plt.close(fig)
                continue
            
            plot_bands_str = [unique_freq_bands_str[i] for i in valid_indices]
            plot_ipd_values = [ipd_values[i] for i in valid_indices]

            sns.barplot(x=plot_bands_str, y=plot_ipd_values, ax=ax, palette="coolwarm")
            ax.set_title(f'{speaker} - Interaural Phase Difference (IPD)')
            ax.set_xlabel('Frequency Band (Hz)')
            ax.set_ylabel('IPD (Degrees, Left - Right)')
            ax.axhline(0, color='grey', linestyle='--')
            ax.set_ylim(-180, 180) 
            ax.set_yticks(np.arange(-180, 181, 45))
            plt.xticks(rotation=45, ha="right")
            plt.tight_layout()

            plot_file_path = os.path.join(dir_path, f'{speaker}_ipd.png')
            try:
                fig.savefig(plot_file_path, bbox_inches='tight')
                im = Image.open(plot_file_path)
                im = im.convert('P', palette=Image.ADAPTIVE, colors=128)
                im.save(plot_file_path, optimize=True)
            except Exception as e:
                print(f"Error saving/optimizing image {plot_file_path}: {e}")
            finally:
                plt.close(fig)

    def plot_iacc(self, dir_path, max_delay_ms=1):
        """Plots Interaural Cross-Correlation (IACC) function and its maximum value for each speaker.

        Args:
            dir_path (str): Path to directory for saving the figures.
            max_delay_ms (float): Maximum interaural delay to consider for cross-correlation in milliseconds.
                                   Determines the range of the x-axis for the cross-correlation plot.
        """
        os.makedirs(dir_path, exist_ok=True)
        sns.set_theme(style="whitegrid")

        max_delay_samples = int(max_delay_ms * self.fs / 1000)

        for speaker, pair in self.irs.items():
            ir_left = pair.get('left')
            ir_right = pair.get('right')

            if not ir_left or not ir_right:
                continue
            
            data_left = ir_left.data.squeeze()
            data_right = ir_right.data.squeeze()

            if data_left.ndim > 1 or data_right.ndim > 1 or len(data_left) == 0 or len(data_right) == 0:
                print(f"Warning: Skipping IACC for {speaker} due to unexpected IR data dimensions or empty data.")
                continue
            
            norm_left = data_left / (np.sqrt(np.mean(data_left**2)) + 1e-12)
            norm_right = data_right / (np.sqrt(np.mean(data_right**2)) + 1e-12)
            
            len_diff = len(norm_left) - len(norm_right)
            if len_diff > 0: 
                norm_right_padded = np.pad(norm_right, (0, len_diff), 'constant')
                norm_left_padded = norm_left
            elif len_diff < 0: 
                norm_left_padded = np.pad(norm_left, (0, -len_diff), 'constant')
                norm_right_padded = norm_right
            else:
                norm_left_padded = norm_left
                norm_right_padded = norm_right

            correlation = signal.correlate(norm_left_padded, norm_right_padded, mode='full')
            
            center_idx = len(norm_right_padded) - 1 
            lags = np.arange(-center_idx, len(norm_left_padded))

            start_lag_idx = np.searchsorted(lags, -max_delay_samples)
            end_lag_idx = np.searchsorted(lags, max_delay_samples, side='right')
            
            relevant_lags_samples = lags[start_lag_idx:end_lag_idx]
            relevant_correlation = correlation[start_lag_idx:end_lag_idx]
            
            if len(relevant_correlation) == 0:
                print(f"Warning: No relevant correlation data for IACC for {speaker} with max_delay_ms={max_delay_ms}.")
                continue

            max_iacc_val = np.max(relevant_correlation)
            max_iacc_idx = np.argmax(relevant_correlation)
            tau_iacc_samples = relevant_lags_samples[max_iacc_idx]
            tau_iacc_ms = tau_iacc_samples * 1000 / self.fs

            fig, ax = plt.subplots(figsize=(10, 6))
            relevant_lags_ms = relevant_lags_samples * 1000 / self.fs
            sns.lineplot(x=relevant_lags_ms, y=relevant_correlation, ax=ax)
            
            ax.axvline(tau_iacc_ms, color='r', linestyle='--', label=f'Max IACC: {max_iacc_val:.2f} at {tau_iacc_ms:.2f} ms')
            ax.set_title(f'{speaker} - Interaural Cross-Correlation (IACC)')
            ax.set_xlabel('Interaural Delay (ms)')
            ax.set_ylabel('Cross-Correlation Coefficient')
            ax.legend()
            ax.grid(True)
            
            ax.set_xlim(-max_delay_ms * 1.1, max_delay_ms * 1.1)

            plot_file_path = os.path.join(dir_path, f'{speaker}_iacc.png')
            try:
                fig.savefig(plot_file_path, bbox_inches='tight')
                im = Image.open(plot_file_path)
                im = im.convert('P', palette=Image.ADAPTIVE, colors=128)
                im.save(plot_file_path, optimize=True)
            except Exception as e:
                print(f"Error saving/optimizing image {plot_file_path}: {e}")
            finally:
                plt.close(fig)

    def plot_etc(self, dir_path, time_range_ms=(0, 200), y_range_db=(-80, 0)):
        """Plots Energy Time Curve (ETC) for each ear of each speaker.

        Args:
            dir_path (str): Path to directory for saving the figures.
            time_range_ms (tuple): Time range for the plot in milliseconds, relative to data start.
            y_range_db (tuple): Y-axis range in dBFS for the plot.
        """
        os.makedirs(dir_path, exist_ok=True)
        sns.set_theme(style="whitegrid")

        for speaker, pair in self.irs.items():
            fig, ax = plt.subplots(figsize=(12, 7))
            has_data_to_plot = False

            for side, ir_obj in pair.items():
                if not ir_obj or len(ir_obj.data) == 0:
                    continue

                data = ir_obj.data.squeeze()
                if data.ndim > 1:
                    print(f"Warning: Skipping ETC for {speaker}-{side} due to unexpected IR data dimensions.")
                    continue

                squared_response = data**2
                energy = np.cumsum(squared_response[::-1])[::-1] 
                
                if np.max(energy) > 1e-12: 
                    etc_db = 10 * np.log10(energy / (np.max(energy) + 1e-12) + 1e-12)
                else:
                    etc_db = np.full_like(energy, y_range_db[0]) 
                
                time_axis_ms = np.arange(len(etc_db)) * 1000 / self.fs

                start_sample_idx = np.searchsorted(time_axis_ms, time_range_ms[0])
                end_sample_idx = np.searchsorted(time_axis_ms, time_range_ms[1], side='right')

                if start_sample_idx >= end_sample_idx: 
                    continue
                
                sns.lineplot(x=time_axis_ms[start_sample_idx:end_sample_idx], 
                             y=etc_db[start_sample_idx:end_sample_idx], 
                             label=f'{side.capitalize()} Ear')
                has_data_to_plot = True
            
            if not has_data_to_plot:
                plt.close(fig)
                continue

            ax.set_title(f'{speaker} - Energy Time Curve (ETC)')
            ax.set_xlabel('Time (ms)')
            ax.set_ylabel('Energy (dBFS)')
            ax.set_xlim(time_range_ms)
            ax.set_ylim(y_range_db)
            ax.legend()
            ax.grid(True)

            plot_file_path = os.path.join(dir_path, f'{speaker}_etc.png')
            try:
                fig.savefig(plot_file_path, bbox_inches='tight')
                im = Image.open(plot_file_path)
                im = im.convert('P', palette=Image.ADAPTIVE, colors=128)
                im.save(plot_file_path, optimize=True)
            except Exception as e:
                print(f"Error saving/optimizing image {plot_file_path}: {e}")
            finally:
                plt.close(fig)
