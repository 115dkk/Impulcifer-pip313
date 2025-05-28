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

# Bokeh imports
from bokeh.plotting import figure, output_file, save
from bokeh.models import HoverTool, ColumnDataSource, Range1d
from bokeh.palettes import Category10
from bokeh.layouts import gridplot


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

    def correct_microphone_deviation(self, correction_strength=0.7, plot_analysis=False, plot_dir=None):
        """
        마이크 착용 편차 보정
        
        바이노럴 임펄스 응답 측정 시 좌우 귀에 착용된 마이크의 위치/깊이 차이로 인한
        주파수 응답 편차를 보정합니다. REW의 MTW(Minimum Time Window) 개념을 활용하여
        직접음 구간만을 분석하고 보정합니다.
        
        Args:
            correction_strength (float): 보정 강도 (0.0~1.0). 0.0은 보정 없음, 1.0은 완전 보정
            plot_analysis (bool): 분석 결과 플롯 생성 여부
            plot_dir (str): 플롯 저장 디렉토리 경로
            
        Returns:
            dict: 각 스피커별 분석 결과
        """
        from microphone_deviation_correction import apply_microphone_deviation_correction_to_hrir
        
        print('마이크 착용 편차 보정 중...')
        
        # 플롯 디렉토리 설정
        if plot_analysis and plot_dir:
            mic_deviation_plot_dir = os.path.join(plot_dir, 'microphone_deviation')
            os.makedirs(mic_deviation_plot_dir, exist_ok=True)
        else:
            mic_deviation_plot_dir = None
            
        # 보정 적용
        analysis_results = apply_microphone_deviation_correction_to_hrir(
            self, 
            correction_strength=correction_strength,
            plot_analysis=plot_analysis,
            plot_dir=mic_deviation_plot_dir
        )
        
        # 보정 결과 요약 출력
        if analysis_results:
            corrected_speakers = []
            skipped_speakers = []
            total_deviations = []
            
            for speaker, results in analysis_results.items():
                if results.get('correction_applied', False):
                    corrected_speakers.append(speaker)
                    if 'avg_deviation_db' in results:
                        total_deviations.append(results['avg_deviation_db'])
                else:
                    skipped_speakers.append(speaker)
                    
            print(f"마이크 편차 보정 완료:")
            print(f"  - 보정 적용: {len(corrected_speakers)}개 스피커 ({', '.join(corrected_speakers)})")
            if skipped_speakers:
                print(f"  - 보정 건너뜀: {len(skipped_speakers)}개 스피커 ({', '.join(skipped_speakers)}) - 유의미한 편차 없음")
            
            if total_deviations:
                avg_deviation = np.mean(total_deviations)
                max_deviation = max([results.get('max_deviation_db', 0) for results in analysis_results.values()])
                print(f"  - 평균 편차: {avg_deviation:.2f} dB, 최대 편차: {max_deviation:.2f} dB")
        else:
            print("마이크 편차 보정: 처리된 스피커가 없습니다.")
            
        return analysis_results

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

    def generate_interaural_impulse_overlay_bokeh_layout(self, time_range_ms=(-5, 30)):
        """Generates Bokeh layout for interaural impulse response overlay for each speaker.

        Returns:
            LayoutDOM: Bokeh gridplot object or None if no data.
        """
        plots = []
        num_speakers = len(self.irs.items())
        colors = Category10[max(3, min(10, num_speakers * 2))]
        color_idx = 0

        for speaker, pair in self.irs.items():
            ir_left = pair.get('left')
            ir_right = pair.get('right')

            if not ir_left or not ir_right:
                continue

            peak_idx_left = ir_left.peak_index() if ir_left else None
            peak_idx_right = ir_right.peak_index() if ir_right else None

            if peak_idx_left is None or peak_idx_right is None:
                continue
            
            align_peak_idx = min(peak_idx_left, peak_idx_right)
            time_vector_ms_left = (np.arange(len(ir_left.data)) - align_peak_idx) / self.fs * 1000
            time_vector_ms_right = (np.arange(len(ir_right.data)) - align_peak_idx) / self.fs * 1000

            source_left = ColumnDataSource(data=dict(time=time_vector_ms_left, amplitude=ir_left.data.squeeze()))
            source_right = ColumnDataSource(data=dict(time=time_vector_ms_right, amplitude=ir_right.data.squeeze()))

            p = figure(
                title=f"Interaural Impulse Response - {speaker}",
                x_axis_label="Time (ms relative to peak)",
                y_axis_label="Amplitude",
                tools="pan,wheel_zoom,box_zoom,reset,save,hover",
                active_drag="pan",
                active_scroll="wheel_zoom",
                height=200, sizing_mode='scale_both'
            )

            line_left = p.line('time', 'amplitude', source=source_left, legend_label="Left Ear", line_width=2, color=colors[color_idx % len(colors)])
            color_idx += 1
            line_right = p.line('time', 'amplitude', source=source_right, legend_label="Right Ear", line_width=2, color=colors[color_idx % len(colors)], line_dash="dashed")
            color_idx += 1
            
            p.x_range = Range1d(time_range_ms[0], time_range_ms[1])
            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [("Channel", "$name"), ("Time", "$x{0.00} ms"), ("Amplitude", "$y{0.0000}")]
            line_left.name = "Left Ear"
            line_right.name = "Right Ear"
            hover.renderers = [line_left, line_right]
            p.legend.location = "top_right"
            p.legend.click_policy="hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode='scale_both')
            return grid
        else:
            return None

    def generate_ild_bokeh_layout(self, freq_bands=None):
        """Generates Bokeh layout for Interaural Level Difference (ILD).

        Returns:
            LayoutDOM: Bokeh gridplot object or None if no data.
        """
        plots = []
        if freq_bands is None:
            octave_centers = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
            freq_bands = []
            for center in octave_centers:
                lower = center / (2**(1/2))
                upper = center * (2**(1/2))
                if upper > self.fs / 2: upper = self.fs / 2
                if lower < upper: freq_bands.append((lower, upper))
                if upper >= self.fs / 2: break
        
        unique_freq_bands_str = [f"{int(fb[0])}-{int(fb[1])}Hz" for fb in freq_bands]
        num_unique_speakers = len(self.irs.keys())
        palette_size = max(3, min(10, num_unique_speakers if num_unique_speakers > 0 else 3))
        colors = Category10[palette_size]

        for i, (speaker, pair) in enumerate(self.irs.items()):
            ir_left = pair.get('left')
            ir_right = pair.get('right')
            if not ir_left or not ir_right: continue

            ild_values = []
            for f_low, f_high in freq_bands:
                if f_high > self.fs / 2: f_high = self.fs / 2
                if f_low >= f_high: ild_values.append(np.nan); continue

                fft_len = next_fast_len(max(len(ir_left.data), len(ir_right.data)))
                data_l_sq = ir_left.data.squeeze()
                data_r_sq = ir_right.data.squeeze()
                if data_l_sq.ndim > 1 or data_r_sq.ndim > 1: ild_values.append(np.nan); continue
                
                fft_l_full = fft(data_l_sq, n=fft_len)
                fft_r_full = fft(data_r_sq, n=fft_len)
                freqs = np.fft.fftfreq(fft_len, d=1/self.fs)
                band_idx = np.where((freqs >= f_low) & (freqs < f_high))[0]
                if not len(band_idx): ild_values.append(np.nan); continue
                
                power_l = np.sum(np.abs(fft_l_full[band_idx])**2)
                power_r = np.sum(np.abs(fft_r_full[band_idx])**2)
                ild = 10 * np.log10((power_l + 1e-12) / (power_r + 1e-12))
                ild_values.append(ild)

            if not ild_values or all(np.isnan(v) for v in ild_values): continue
            valid_indices = [k for k, v in enumerate(ild_values) if not np.isnan(v)]
            if not valid_indices: continue

            plot_bands = [unique_freq_bands_str[k] for k in valid_indices]
            plot_ilds = [ild_values[k] for k in valid_indices]
            source = ColumnDataSource(data=dict(bands=plot_bands, ilds=plot_ilds, color=[colors[i % palette_size]]*len(plot_bands)))

            p = figure(x_range=plot_bands, title=f"ILD - {speaker}",
                       toolbar_location=None, tools="hover,save,pan,wheel_zoom,box_zoom,reset", height=175, sizing_mode='scale_both',
                       x_axis_label="Frequency Band", y_axis_label="ILD (dB, Left/Right)")
            p.vbar(x='bands', top='ilds', width=0.9, source=source, legend_label=speaker, line_color='color')
            
            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [("Band", "@bands"), ("ILD", "@ilds{0.0} dB")]
            p.xgrid.grid_line_color = None
            p.legend.orientation = "horizontal"
            p.legend.location = "top_center"
            p.legend.click_policy="hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode='scale_both')
            return grid
        else:
            return None

    def generate_ipd_bokeh_layout(self, freq_bands=None, unwrap_phase=True):
        """Generates Bokeh layout for Interaural Phase Difference (IPD).

        Returns:
            LayoutDOM: Bokeh gridplot object or None if no data.
        """
        plots = []
        if freq_bands is None:
            octave_centers = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
            freq_bands = []
            for center in octave_centers:
                lower = center / (2**(1/2))
                upper = center * (2**(1/2))
                if upper > self.fs / 2: upper = self.fs / 2
                if lower < upper: freq_bands.append((lower, upper))
                if upper >= self.fs / 2: break
        
        unique_freq_bands_str = [f"{int(fb[0])}-{int(fb[1])}Hz" for fb in freq_bands]
        num_unique_speakers = len(self.irs.keys())
        palette_size = max(3, min(10, num_unique_speakers if num_unique_speakers > 0 else 3))
        colors = Category10[palette_size]

        for i, (speaker, pair) in enumerate(self.irs.items()):
            ir_left = pair.get('left')
            ir_right = pair.get('right')
            if not ir_left or not ir_right: continue

            ipd_values = []
            for f_low, f_high in freq_bands:
                if f_high > self.fs / 2: f_high = self.fs / 2
                if f_low >= f_high: ipd_values.append(np.nan); continue

                fft_len = next_fast_len(max(len(ir_left.data), len(ir_right.data)))
                data_l_sq = ir_left.data.squeeze()
                data_r_sq = ir_right.data.squeeze()
                if data_l_sq.ndim > 1 or data_r_sq.ndim > 1: ipd_values.append(np.nan); continue
                
                fft_l_full = fft(data_l_sq, n=fft_len)
                fft_r_full = fft(data_r_sq, n=fft_len)
                freqs = np.fft.fftfreq(fft_len, d=1/self.fs)
                band_idx = np.where((freqs >= f_low) & (freqs < f_high))[0]
                if not len(band_idx): ipd_values.append(np.nan); continue
                
                complex_sum_l = np.sum(fft_l_full[band_idx])
                complex_sum_r = np.sum(fft_r_full[band_idx])
                phase_l = np.angle(complex_sum_l)
                phase_r = np.angle(complex_sum_r)
                ipd = phase_l - phase_r
                if unwrap_phase:
                    ipd = (ipd + np.pi) % (2 * np.pi) - np.pi
                ipd_values.append(np.degrees(ipd))

            if not ipd_values or all(np.isnan(v) for v in ipd_values): continue
            valid_indices = [k for k, v in enumerate(ipd_values) if not np.isnan(v)]
            if not valid_indices: continue

            plot_bands = [unique_freq_bands_str[k] for k in valid_indices]
            plot_ipds = [ipd_values[k] for k in valid_indices]
            source = ColumnDataSource(data=dict(bands=plot_bands, ipds=plot_ipds, color=[colors[i % palette_size]]*len(plot_bands)))

            p = figure(x_range=plot_bands, title=f"IPD - {speaker}",
                       toolbar_location=None, tools="hover,save,pan,wheel_zoom,box_zoom,reset", height=175, sizing_mode='scale_both',
                       x_axis_label="Frequency Band", y_axis_label="IPD (Degrees, Left - Right)")
            p.vbar(x='bands', top='ipds', width=0.9, source=source, legend_label=speaker, line_color='color')
            
            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [("Band", "@bands"), ("IPD", "@ipds{0.0} deg")]
            p.xgrid.grid_line_color = None
            p.y_range = Range1d(-180, 180)
            p.yaxis.ticker = [-180, -135, -90, -45, 0, 45, 90, 135, 180]
            p.legend.orientation = "horizontal"
            p.legend.location = "top_center"
            p.legend.click_policy="hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode='scale_both')
            return grid
        else:
            return None

    def generate_iacc_bokeh_layout(self, max_delay_ms=1):
        """Generates Bokeh layout for Interaural Cross-Correlation (IACC).

        Returns:
            LayoutDOM: Bokeh gridplot object or None if no data.
        """
        plots = []
        max_delay_samples = int(max_delay_ms * self.fs / 1000)
        num_unique_speakers = len(self.irs.keys())
        palette_size = max(3, min(10, num_unique_speakers if num_unique_speakers > 0 else 3))
        colors = Category10[palette_size]

        for i, (speaker, pair) in enumerate(self.irs.items()):
            ir_left = pair.get('left')
            ir_right = pair.get('right')
            if not ir_left or not ir_right: continue

            data_l_sq = ir_left.data.squeeze()
            data_r_sq = ir_right.data.squeeze()
            if data_l_sq.ndim > 1 or data_r_sq.ndim > 1 or not len(data_l_sq) or not len(data_r_sq):
                continue

            norm_l = data_l_sq / (np.sqrt(np.mean(data_l_sq**2)) + 1e-12)
            norm_r = data_r_sq / (np.sqrt(np.mean(data_r_sq**2)) + 1e-12)
            
            len_diff = len(norm_l) - len(norm_r)
            if len_diff > 0: norm_r_pad = np.pad(norm_r, (0, len_diff), 'constant'); norm_l_pad = norm_l
            elif len_diff < 0: norm_l_pad = np.pad(norm_l, (0, -len_diff), 'constant'); norm_r_pad = norm_r
            else: norm_l_pad = norm_l; norm_r_pad = norm_r

            correlation = signal.correlate(norm_l_pad, norm_r_pad, mode='full')
            lags = signal.correlation_lags(len(norm_l_pad), len(norm_r_pad), mode='full')
            
            mask = np.abs(lags) <= max_delay_samples
            relevant_lags_s = lags[mask]
            relevant_corr = correlation[mask]

            if not len(relevant_corr): continue

            max_iacc_val = np.max(relevant_corr)
            tau_iacc_s = relevant_lags_s[np.argmax(relevant_corr)]
            tau_iacc_ms_val = tau_iacc_s * 1000 / self.fs

            source = ColumnDataSource(data=dict(lags_ms=relevant_lags_s * 1000 / self.fs, correlation=relevant_corr))

            p = figure(title=f"IACC - {speaker}", tools="hover,save,pan,wheel_zoom,box_zoom,reset", 
                       height=175, sizing_mode='scale_both', x_axis_label="Interaural Delay (ms)", y_axis_label="Cross-Correlation Coefficient")
            p.line('lags_ms', 'correlation', source=source, line_width=2, color=colors[i % palette_size], legend_label=f"Max: {max_iacc_val:.2f} at {tau_iacc_ms_val:.2f}ms")
            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [("Delay", "@lags_ms{0.00} ms"), ("Correlation", "@correlation{0.00}")]
            p.x_range = Range1d(-max_delay_ms * 1.1, max_delay_ms * 1.1)
            p.legend.location = "top_right"
            p.legend.click_policy="hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode='scale_both')
            return grid
        else:
            return None

    def generate_etc_bokeh_layout(self, time_range_ms=(0, 200), y_range_db=(-80, 0)):
        """Generates Bokeh layout for Energy Time Curve (ETC).

        Returns:
            LayoutDOM: Bokeh gridplot object or None if no data.
        """
        plots = []
        num_speakers = len(self.irs.items())
        palette_size = max(3, min(10, num_speakers * 2 if num_speakers > 0 else 3))
        colors = Category10[palette_size]
        color_idx = 0

        for speaker, pair in self.irs.items():
            p = figure(title=f"ETC - {speaker}", tools="hover,save,pan,wheel_zoom,box_zoom,reset",
                       height=200, sizing_mode='scale_both', x_axis_label="Time (ms)", y_axis_label="Energy (dBFS)")
            has_data_for_speaker = False
            current_plot_lines = []

            for side, ir_obj in pair.items():
                if not ir_obj or len(ir_obj.data) == 0: continue

                data_sq = ir_obj.data.squeeze()
                if data_sq.ndim > 1: continue

                squared_response = data_sq**2
                energy = np.cumsum(squared_response[::-1])[::-1]
                if np.max(energy) > 1e-12:
                    etc_db_vals = 10 * np.log10(energy / (np.max(energy) + 1e-12) + 1e-12)
                else:
                    etc_db_vals = np.full_like(energy, y_range_db[0])
                
                time_axis = np.arange(len(etc_db_vals)) * 1000 / self.fs
                
                source = ColumnDataSource(data=dict(time=time_axis, etc=etc_db_vals))
                line = p.line('time', 'etc', source=source, legend_label=f"{side.capitalize()} Ear", 
                              line_width=2, color=colors[color_idx % palette_size])
                line.name = f"{side.capitalize()} Ear"
                current_plot_lines.append(line)
                color_idx += 1
                has_data_for_speaker = True
            
            if has_data_for_speaker:
                p.x_range = Range1d(time_range_ms[0], time_range_ms[1])
                p.y_range = Range1d(y_range_db[0], y_range_db[1])
                hover = p.select(dict(type=HoverTool))
                hover.tooltips = [("Channel", "$name"), ("Time", "$x{0.00} ms"), ("Energy", "$y{0.00} dB")]
                hover.renderers = current_plot_lines
                p.legend.location = "top_right"
                p.legend.click_policy="hide"
                plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode='scale_both')
            return grid
        else:
            return None

    def generate_result_bokeh_figure(self):
        """Generates Bokeh figure for stacked left and right side results.

        Returns:
            Figure: Bokeh Figure object or None if no data.
        """
        if not self.irs:
            return None

        stacks = [[], []]
        for speaker, pair in self.irs.items():
            if pair.get('left') and hasattr(pair['left'], 'data'):
                stacks[0].append(pair['left'].data)
            if pair.get('right') and hasattr(pair['right'], 'data'):
                stacks[1].append(pair['right'].data)

        if not stacks[0] or not stacks[1]:
            return None
        
        summed_left_data = np.sum(np.vstack(stacks[0]), axis=0) if stacks[0] else np.array([0.0])
        summed_right_data = np.sum(np.vstack(stacks[1]), axis=0) if stacks[1] else np.array([0.0])

        if len(summed_left_data) <= 1 or len(summed_right_data) <= 1:
            return None

        left_ir = ImpulseResponse(summed_left_data, self.fs)
        left_fr = left_ir.frequency_response()
        left_fr.smoothen(window_size=1/3, treble_window_size=1/5, treble_f_lower=20000, treble_f_upper=max(20001, int(self.fs/2-1)))

        right_ir = ImpulseResponse(summed_right_data, self.fs)
        right_fr = right_ir.frequency_response()
        right_fr.smoothen(window_size=1/3, treble_window_size=1/5, treble_f_lower=20000, treble_f_upper=max(20001, int(self.fs/2-1)))

        p = figure(
            title="Overall Smoothed Frequency Response",
            x_axis_label="Frequency (Hz)",
            y_axis_label="Amplitude (dB)",
            x_axis_type="log",
            tools="pan,wheel_zoom,box_zoom,reset,save,hover",
            active_drag="pan",
            active_scroll="wheel_zoom",
            height=300, sizing_mode='scale_both'
        )

        source_left_raw = ColumnDataSource(data=dict(freq=left_fr.frequency, raw=left_fr.raw))
        source_left_smooth = ColumnDataSource(data=dict(freq=left_fr.frequency, smooth=left_fr.smoothed))
        source_right_raw = ColumnDataSource(data=dict(freq=right_fr.frequency, raw=right_fr.raw))
        source_right_smooth = ColumnDataSource(data=dict(freq=right_fr.frequency, smooth=right_fr.smoothed))
        
        diff_smooth = left_fr.smoothed - right_fr.smoothed
        source_diff = ColumnDataSource(data=dict(freq=left_fr.frequency, diff=diff_smooth))

        p.line('freq', 'raw', source=source_left_raw, line_width=1, color=Category10[3][0], alpha=0.5, legend_label="Left Raw", muted_alpha=0.1)
        p.line('freq', 'raw', source=source_right_raw, line_width=1, color=Category10[3][1], alpha=0.5, legend_label="Right Raw", muted_alpha=0.1)

        l_smooth = p.line('freq', 'smooth', source=source_left_smooth, line_width=2, color=Category10[3][0], legend_label="Left Smoothed")
        r_smooth = p.line('freq', 'smooth', source=source_right_smooth, line_width=2, color=Category10[3][1], legend_label="Right Smoothed")
        d_smooth = p.line('freq', 'diff', source=source_diff, line_width=2, color=Category10[3][2], legend_label="Difference (L-R)", line_dash="dashed")

        p.x_range = Range1d(20, 20000)
        hover = p.select(dict(type=HoverTool))
        hover.tooltips = [("Legend", "$name"), ("Frequency", "$x{0.0} Hz"), ("Amplitude", "$y{0.00} dB")]
        hover.renderers = [l_smooth, r_smooth, d_smooth]
        p.legend.location = "top_right"
        p.legend.click_policy = "mute"

        return p
