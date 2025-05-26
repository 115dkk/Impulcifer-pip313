# -*- coding: utf-8 -*-

import os
import re
import argparse
from tabulate import tabulate
from datetime import datetime
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.ticker as ticker
from matplotlib.ticker import FormatStrFormatter
from autoeq.frequency_response import FrequencyResponse
from impulse_response_estimator import ImpulseResponseEstimator
from hrir import HRIR
from room_correction import room_correction
from utils import sync_axes, save_fig_as_png
from constants import SPEAKER_NAMES, SPEAKER_LIST_PATTERN, HESUVI_TRACK_ORDER, TEST_SIGNALS, get_data_path

# PR3에서 추가된 import 문들
import copy
import contextlib
import io
from scipy.io import wavfile # hangloose용, 필요시 추가

# 한글 폰트 설정 추가
import matplotlib.font_manager as fm
import platform

# 운영체제별 기본 폰트 설정
system = platform.system()
if system == 'Windows':
    # Windows 기본 폰트
    font_path = 'C:/Windows/Fonts/malgun.ttf'  # 맑은 고딕
    if os.path.exists(font_path):
        font_prop = fm.FontProperties(fname=font_path)
        plt.rcParams['font.family'] = font_prop.get_name()
    else:
        # 대체 폰트 시도
        plt.rcParams['font.family'] = 'Malgun Gothic'
elif system == 'Darwin':  # macOS
    plt.rcParams['font.family'] = 'AppleGothic'
elif system == 'Linux':
    # Linux 기본 폰트
    plt.rcParams['font.family'] = 'NanumGothic'


def main(dir_path=None,
         test_signal=None,
         room_target=None,
         room_mic_calibration=None,
         fs=None,
         plot=False,
         channel_balance=None,
         decay=None,
         target_level=None,
         fr_combination_method='average',
         specific_limit=20000,
         generic_limit=1000,
         bass_boost_gain=0.0,
         bass_boost_fc=105,
         bass_boost_q=0.76,
         tilt=0.0,
         do_room_correction=True,
         do_headphone_compensation=True,
         do_equalization=True,
         # PR3에서 추가/변경된 파라미터 (항목 4, 6, 7)
         head_ms=1, # --c 옵션에 해당 (기본값 1ms)
         jamesdsp=False,
         hangloose=False):
    """"""
    if dir_path is None or not os.path.isdir(dir_path):
        raise NotADirectoryError(f'Given dir path "{dir_path}"" is not a directory.')

    # Dir path as absolute
    dir_path = os.path.abspath(dir_path)

    # Impulse response estimator
    print('Creating impulse response estimator...')
    estimator = open_impulse_response_estimator(dir_path, file_path=test_signal)

    # Room correction frequency responses
    room_frs = None
    if do_room_correction:
        print('Running room correction...')
        _, room_frs = room_correction(
            estimator, dir_path,
            target=room_target,
            mic_calibration=room_mic_calibration,
            fr_combination_method=fr_combination_method,
            specific_limit=specific_limit,
            generic_limit=generic_limit,
            plot=plot
        )

    # Headphone compensation frequency responses
    hp_left, hp_right = None, None
    if do_headphone_compensation:
        print('Running headphone compensation...')
        hp_left, hp_right = headphone_compensation(estimator, dir_path)

    # Equalization
    eq_left, eq_right = None, None
    if do_equalization:
        print('Creating headphone equalization...')
        eq_left, eq_right = equalization(estimator, dir_path)

    # Bass boost and tilt
    print('Creating frequency response target...')
    target = create_target(estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt)

    # HRIR measurements
    print('Opening binaural measurements...')
    hrir = open_binaural_measurements(estimator, dir_path)

    # Normalize gain
    print('Normalizing gain...')
    applied_gain = hrir.normalize(peak_target=None if target_level is not None else -0.1, avg_target=target_level)

    # Write info and stats in readme (gain 값 전달 추가)
    readme_content = write_readme(os.path.join(dir_path, 'README.md'), hrir, fs, estimator, applied_gain)
    if readme_content:
        print(readme_content)

    if plot:
        # Plot graphs pre processing
        os.makedirs(os.path.join(dir_path, 'plots', 'pre'), exist_ok=True)
        print('Plotting BRIR graphs before processing...')
        hrir.plot(dir_path=os.path.join(dir_path, 'plots', 'pre'))

    # Crop noise and harmonics from the beginning
    print('Cropping impulse responses...')
    hrir.crop_heads(head_ms=head_ms)

    # PR3에서 추가된 align_ipsilateral_all 호출 (항목 2)
    # SPEAKER_NAMES를 사용하므로 constants.py의 변경이 선행되어야 함
    hrir.align_ipsilateral_all(
        speaker_pairs=[('FL','FR'), ('SL','SR'), ('BL','BR'),
                        ('TFL','TFR'), ('TSL','TSR'), ('TBL','TBR'),
                        ('FC','FC'), ('WL','WR')], # FC, WL, WR 쌍은 적절히 수정 필요할 수 있음
        segment_ms=30
    )

    # Crop noise from the tail
    hrir.crop_tails()

    # Write multi-channel WAV file with sine sweeps for debugging
    hrir.write_wav(os.path.join(dir_path, 'responses.wav'))

    # Equalize all
    if do_headphone_compensation or do_room_correction or do_equalization:
        print('Equalizing...')
        
        for speaker, pair in hrir.irs.items():
            for side, ir in pair.items():
                fr = FrequencyResponse(
                    name=f'{speaker}-{side} eq',
                    frequency=FrequencyResponse.generate_frequencies(f_step=1.01, f_min=10, f_max=estimator.fs / 2),
                    raw=0, error=0
                )

                # 룸 보정 적용
                if room_frs is not None and speaker in room_frs and side in room_frs[speaker]:
                    # Room correction
                    fr.error += room_frs[speaker][side].error

                # 헤드폰 보정 적용
                hp_eq = hp_left if side == 'left' else hp_right
                if hp_eq is not None:
                    # Headphone compensation
                    fr.error += hp_eq.error

                # 추가 EQ 적용
                eq = eq_left if side == 'left' else eq_right
                if eq is not None and type(eq) == FrequencyResponse:
                    # Equalization
                    fr.error += eq.error

                # Remove bass and tilt target from the error
                fr.error -= target.raw

                # Smoothen
                fr.smoothen(window_size=1/3, treble_window_size=1/5)

                # Equalize
                eq_result, _, _, _, _, _, _, _, _, _ = fr.equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=estimator.fs / 2)
                
                # Create FIR filter and equalize
                fir = fr.minimum_phase_impulse_response(fs=estimator.fs, normalize=False, f_res=5)
                
                # 실제 FIR 필터 적용
                ir.equalize(fir)

    # Adjust decay time
    if decay:
        print('Adjusting decay time...')
        for speaker, pair in hrir.irs.items():
            for side, ir in pair.items():
                if speaker in decay:
                    ir.adjust_decay(decay[speaker])

    # Correct channel balance
    if channel_balance is not None:
        print('Correcting channel balance...')
        hrir.correct_channel_balance(channel_balance)

    if plot:
        print('Plotting BRIR graphs after processing...')
        # Convolve test signal, re-plot waveform and spectrogram
        for speaker, pair in hrir.irs.items():
            for side, ir in pair.items():
                ir.recording = ir.convolve(estimator.test_signal)
        # Plot post processing
        hrir.plot(os.path.join(dir_path, 'plots', 'post'))

    # Plot results, always
    print('Plotting results...')
    hrir.plot_result(os.path.join(dir_path, 'plots'))

    # Re-sample
    if fs is not None and fs != hrir.fs:
        print(f'Resampling BRIR to {fs} Hz')
        hrir.resample(fs)
        hrir.normalize(peak_target=None if target_level is not None else -0.1, avg_target=target_level)

    # Write multi-channel WAV file with standard track order
    print('Writing BRIRs...')
    hrir.write_wav(os.path.join(dir_path, 'hrir.wav'))

    # Write multi-channel WAV file with HeSuVi track order
    hrir.write_wav(os.path.join(dir_path, 'hesuvi.wav'), track_order=HESUVI_TRACK_ORDER)

    # PR3 jamesdsp 로직 추가 (항목 6)
    if jamesdsp:
        print('Generating jamesdsp.wav (FL/FR only, normalized to FL/FR)...')
        
        # 전체 HRIR 복사 후 FL/FR 외 모든 채널 제거
        dsp_hrir = copy.deepcopy(hrir)
        for sp in list(dsp_hrir.irs.keys()):
            if sp not in ['FL', 'FR']:
                del dsp_hrir.irs[sp]

        # normalize 내부의 print문 출력을 숨기기 위해 stdout 리디렉션
        # target_level 변수가 main 함수 스코프에 있어야 함
        with contextlib.redirect_stdout(io.StringIO()):
            dsp_hrir.normalize(
                peak_target=None if target_level is not None else -0.1,
                avg_target=target_level
            )

        # FL-L, FL-R, FR-L, FR-R 순서로 파일 생성
        jd_order = ['FL-left', 'FL-right', 'FR-left', 'FR-right']
        out_path = os.path.join(dir_path, 'jamesdsp.wav')
        dsp_hrir.write_wav(out_path, track_order=jd_order)
        print(f'JamesDSP IR file created: {out_path}')

    # PR3 hangloose 로직 추가 (항목 7)
    if hangloose:
        print('Generating Hangloose Convolver IR files...')
        output_dir = os.path.join(dir_path, 'Hangloose')
        os.makedirs(output_dir, exist_ok=True)

        # Hrir.wav 기준 최대 채널 순서 (constants.py의 SPEAKER_NAMES 순서와 일치시키는 것이 좋을 수 있음)
        # PR3의 full_order는 LFE를 포함하나, 현재 SPEAKER_NAMES에는 LFE가 없음.
        # 여기서는 hrir 객체에 있는 스피커만 사용하도록 단순화.
        processed_speakers = [sp for sp in SPEAKER_NAMES if sp in hrir.irs]

        for sp in processed_speakers:
            single_hrir = copy.deepcopy(hrir)
            for other_sp in list(single_hrir.irs.keys()):
                if other_sp != sp:
                    del single_hrir.irs[other_sp]
            
            # 각 스피커에 대해 normalize를 다시 수행할지 여부는 PR의 의도에 따라 결정.
            # 여기서는 생략하고 원본 hrir의 정규화 상태를 따름.

            track_order = [f'{sp}-left', f'{sp}-right']
            out_path = os.path.join(output_dir, f'{sp}.wav')
            single_hrir.write_wav(out_path, track_order=track_order)
            print(f'[Hangloose] Created: {out_path}')
        
        # PR3의 LFE 채널 생성 로직은 FL, FR을 기반으로 하므로, 필요시 여기에 추가 구현.
        # 예시: if 'FL' in processed_speakers and 'FR' in processed_speakers:
        # LFE 생성 로직 ...


def open_impulse_response_estimator(dir_path, file_path=None):
    """Opens impulse response estimator from a file

    Args:
        dir_path: Path to directory
        file_path: Explicitly given (if any) path to impulse response estimator Pickle or test signal WAV file,
                  or a simple name/number for predefined test signals

    Returns:
        ImpulseResponseEstimator instance
    """
    # 테스트 신호가 숫자나 이름으로 지정된 경우
    if file_path in TEST_SIGNALS:
        # 패키지 내 데이터 폴더에서 해당 파일 경로 찾기
        test_signal_name = TEST_SIGNALS[file_path]
        test_signal_path = os.path.join(get_data_path(), test_signal_name)
        
        # 파일이 존재하는지 확인
        if os.path.isfile(test_signal_path):
            file_path = test_signal_path
        else:
            # 패키지 내 파일을 찾지 못한 경우 로컬 data 폴더에서 시도
            local_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'data', test_signal_name)
            if os.path.isfile(local_path):
                file_path = local_path
            else:
                print(f"경고: 테스트 신호 '{file_path}'({test_signal_name})를 찾을 수 없습니다. 로컬 파일을 사용합니다.")
    
    if file_path is None:
        # Test signal not explicitly given, try Pickle first then WAV
        if os.path.isfile(os.path.join(dir_path, 'test.pkl')):
            file_path = os.path.join(dir_path, 'test.pkl')
        elif os.path.isfile(os.path.join(dir_path, 'test.wav')):
            file_path = os.path.join(dir_path, 'test.wav')
        else:
            # 기본 테스트 신호 사용 (패키지 내부 또는 로컬)
            default_signal_name = TEST_SIGNALS['default']
            default_signal_path = os.path.join(get_data_path(), default_signal_name)
            
            if os.path.isfile(default_signal_path):
                file_path = default_signal_path
            else:
                # 패키지 내 파일을 찾지 못한 경우 로컬 data 폴더에서 시도
                local_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'data', default_signal_name)
                if os.path.isfile(local_path):
                    file_path = local_path
                else:
                    raise FileNotFoundError(f"기본 테스트 신호 파일을 찾을 수 없습니다: {default_signal_name}")
    
    if re.match(r'^.+\.wav$', file_path, flags=re.IGNORECASE):
        # Test signal is WAV file
        estimator = ImpulseResponseEstimator.from_wav(file_path)
    elif re.match(r'^.+\.pkl$', file_path, flags=re.IGNORECASE):
        # Test signal is Pickle file
        estimator = ImpulseResponseEstimator.from_pickle(file_path)
    else:
        raise TypeError(f'알 수 없는 파일 확장자: "{file_path}"\n유효한 파일 확장자: .wav, .pkl')
    
    return estimator


def equalization(estimator, dir_path):
    """Reads equalization FIR filter or CSV settings

    Args:
        estimator: ImpulseResponseEstimator
        dir_path: Path to directory

    Returns:
        - Left side FIR as Numpy array or FrequencyResponse or None
        - Right side FIR as Numpy array or FrequencyResponse or None
    """
    if os.path.isfile(os.path.join(dir_path, 'eq.wav')):
        print('eq.wav is no longer supported, use eq.csv!')
    # Default for both sides
    eq_path = os.path.join(dir_path, 'eq.csv')
    eq_fr = None
    if os.path.isfile(eq_path):
        eq_fr = FrequencyResponse.read_from_csv(eq_path)

    # Left
    left_path = os.path.join(dir_path, 'eq-left.csv')
    left_fr = None
    if os.path.isfile(left_path):
        left_fr = FrequencyResponse.read_from_csv(left_path)
    elif eq_fr is not None:
        left_fr = eq_fr
    if left_fr is not None:
        left_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)

    # Right
    right_path = os.path.join(dir_path, 'eq-right.csv')
    right_fr = None
    if os.path.isfile(right_path):
        right_fr = FrequencyResponse.read_from_csv(right_path)
    elif eq_fr is not None:
        right_fr = eq_fr
    if right_fr is not None and right_fr != left_fr:
        right_fr.interpolate(f_step=1.01, f_min=10, f_max=estimator.fs / 2)

    # Plot
    if left_fr is not None or right_fr is not None:
        if left_fr == right_fr:
            # Both are the same, plot only one graph
            fig, ax = plt.subplots()
            fig.set_size_inches(12, 9)
            left_fr.plot(fig=fig, ax=ax, show_fig=False)
        else:
            # Left and right are different, plot two graphs in the same figure
            fig, ax = plt.subplots(1, 2)
            fig.set_size_inches(22, 9)
            if left_fr is not None:
                left_fr.plot(fig=fig, ax=ax[0], show_fig=False)
            if right_fr is not None:
                right_fr.plot(fig=fig, ax=ax[1], show_fig=False)
        save_fig_as_png(os.path.join(dir_path, 'plots', 'eq.png'), fig)

    return left_fr, right_fr


def headphone_compensation(estimator, dir_path):
    """Equalizes HRIR tracks with headphone compensation measurement.

    Args:
        estimator: ImpulseResponseEstimator instance
        dir_path: Path to output directory

    Returns:
        None
    """
    # Read WAV file
    hp_irs = HRIR(estimator)
    hp_irs.open_recording(os.path.join(dir_path, 'headphones.wav'), speakers=['FL', 'FR'])
    hp_irs.write_wav(os.path.join(dir_path, 'headphone-responses.wav'))

    # Frequency responses
    left = hp_irs.irs['FL']['left'].frequency_response()
    right = hp_irs.irs['FR']['right'].frequency_response()
    
    # 배열 길이 검증 및 일치시키기
    if len(left.frequency) != len(right.frequency):
        # 둘 중 더 작은 길이로 조정
        min_length = min(len(left.frequency), len(right.frequency))
        left.frequency = left.frequency[:min_length]
        left.raw = left.raw[:min_length]
        right.frequency = right.frequency[:min_length]
        right.raw = right.raw[:min_length]

    # Center by left channel
    gain = left.center([100, 10000])
    right.raw += gain
    
    # 저주파 롤오프 방지를 위한 타겟 생성
    freq = FrequencyResponse.generate_frequencies(f_min=10, f_max=estimator.fs/2, f_step=1.01)
    
    # 새로운 타겟: 저주파에 6dB 부스트를 적용한 타겟
    target_raw = np.zeros(len(freq))
    
    # 헤드폰 저주파 보상용 타겟 생성 (6dB 부스트로 수정)
    for i, f in enumerate(freq):
        if f < 100:
            # 100Hz 이하에서 로그 스케일로 서서히 증가하는 저주파 부스트 적용
            # 10Hz에서 최대 6dB 부스트, 100Hz에서 0dB
            log_ratio = np.log10(f / 10) / np.log10(100 / 10)
            target_raw[i] = 6 * (1 - log_ratio) # 10dB에서 6dB로 수정
    
    # 타겟 응답 객체 생성
    target = FrequencyResponse(name='headphone_compensation_target', frequency=freq, raw=target_raw)

    # left와 right를 타겟의 주파수에 맞게 보간
    left_orig = left.copy()
    right_orig = right.copy()
    
    left.interpolate(f=target.frequency)
    right.interpolate(f=target.frequency)
    
    # 보상 적용
    left.compensate(target, min_mean_error=True)
    right.compensate(target, min_mean_error=True)
    
    # 저주파에서 올바른 보상을 유지하기 위한 후처리
    # 아주 낮은 주파수(20Hz 이하)에서 보상이 지나치게 감소하는 것을 방지
    for fr in [left, right]:
        # 주파수별 추가 보정
        for i, f in enumerate(fr.frequency):
            if f < 20:  # 20Hz 이하
                fr.error[i] += 6 * (1 - np.log10(f / 10) / np.log10(20 / 10))
            elif f < 50:  # 20-50Hz
                fr.error[i] += 3 * (1 - np.log10(f / 20) / np.log10(50 / 20))

    # 기존 헤드폰 플롯
    fig = plt.figure()
    gs = fig.add_gridspec(2, 3)
    fig.set_size_inches(22, 10)
    fig.suptitle('Headphones')

    # Left
    axl = fig.add_subplot(gs[0, 0])
    left.plot(fig=fig, ax=axl, show_fig=False)
    axl.set_title('Left')
    # Right
    axr = fig.add_subplot(gs[1, 0])
    right.plot(fig=fig, ax=axr, show_fig=False)
    axr.set_title('Right')
    # Sync axes
    sync_axes([axl, axr])

    # Combined
    _left = left.copy()
    _right = right.copy()
    gain_l = _left.center([100, 10000])
    gain_r = _right.center([100, 10000])
    ax = fig.add_subplot(gs[:, 1:])
    ax.plot(_left.frequency, _left.raw, linewidth=1, color='#1f77b4')
    ax.plot(_right.frequency, _right.raw, linewidth=1, color='#d62728')
    ax.plot(_left.frequency, _left.raw - _right.raw, linewidth=1, color='#680fb9')
    sl = np.logical_and(_left.frequency > 20, _left.frequency < 20000)
    stack = np.vstack([_left.raw[sl], _right.raw[sl], _left.raw[sl] - _right.raw[sl]])
    ax.set_ylim([np.min(stack) * 1.1, np.max(stack) * 1.1])
    axl.set_ylim([np.min(stack) * 1.1, np.max(stack) * 1.1])
    axr.set_ylim([np.min(stack) * 1.1, np.max(stack) * 1.1])
    ax.set_title('Comparison')
    ax.legend([f'Left raw {gain_l:+.1f} dB', f'Right raw {gain_r:+.1f} dB', 'Difference'], fontsize=8)
    ax.set_xlabel('Frequency (Hz)')
    ax.semilogx()
    ax.set_xlim([20, 20000])
    ax.set_ylabel('Amplitude (dB)')
    ax.grid(True, which='major')
    ax.grid(True, which='minor')
    ax.xaxis.set_major_formatter(ticker.StrMethodFormatter('{x:.0f}'))

    # Save headphone plots
    file_path = os.path.join(dir_path, 'plots', 'headphones.png')
    os.makedirs(os.path.split(file_path)[0], exist_ok=True)
    save_fig_as_png(file_path, fig)
    plt.close(fig)

    return left, right


def create_target(estimator, bass_boost_gain, bass_boost_fc, bass_boost_q, tilt):
    """Creates target frequency response with bass boost, tilt and high pass at 20 Hz"""
    # 타겟 주파수 응답 생성
    target = FrequencyResponse(
        name='bass_and_tilt',
        frequency=FrequencyResponse.generate_frequencies(f_min=10, f_max=estimator.fs / 2, f_step=1.01)
    )
    
    # 베이스 부스트와 틸트 적용
    # 기본 베이스 부스트만 적용 (추가 부스트 제거)
    target.raw = target.create_target(
        bass_boost_gain=bass_boost_gain,  # +3dB 추가 부스트 제거
        bass_boost_fc=bass_boost_fc,
        bass_boost_q=bass_boost_q,
        tilt=tilt
    )
    
    # 수정된 하이패스 필터 적용
    # 10Hz까지 완만한 롤오프로 수정하여 저주파 응답을 향상
    high_pass = FrequencyResponse(
        name='high_pass_modified',
        frequency=[10, 15, 20, 25, 30, 20000],
        raw=[-6, -3, -1, -0.5, 0, 0]  # 더 완만한 롤오프
    )
    high_pass.interpolate(f_min=10, f_max=estimator.fs / 2, f_step=1.01)
    
    # 하이패스 필터 적용
    target.raw += high_pass.raw
    
    # 저주파 영역 베이스 부스트 값 출력 (디버깅용)
    # bass_boost_values = target.raw[:200]  # 저주파 영역만 추출
    # print("저주파 영역 Bass Boost 값:", bass_boost_values) # 주석 처리
    
    return target


def open_binaural_measurements(estimator, dir_path):
    """Opens binaural measurement WAV files.

    Args:
        estimator: ImpulseResponseEstimator
        dir_path: Path to directory

    Returns:
        HRIR instance
    """
    hrir = HRIR(estimator)
    pattern = r'^{pattern}\.wav$'.format(pattern=SPEAKER_LIST_PATTERN)  # FL,FR.wav
    for file_name in [f for f in os.listdir(dir_path) if re.match(pattern, f)]:
        # Read the speaker names from the file name into a list
        speakers = re.search(SPEAKER_LIST_PATTERN, file_name)[0].split(',')
        # Form absolute path
        file_path = os.path.join(dir_path, file_name)
        # Open the file and add tracks to HRIR
        hrir.open_recording(file_path, speakers=speakers)
    if len(hrir.irs) == 0:
        raise ValueError('No HRIR recordings found in the directory.')
    return hrir


def write_readme(file_path, hrir, fs, estimator, applied_gain):
    """Writes info and stats to a README file and returns its content as a string.

    Args:
        file_path (str): Path to README file.
        hrir (HRIR): HRIR object.
        fs (int): Output sampling rate.
        estimator (ImpulseResponseEstimator): Estimator object for advanced stats.
        applied_gain (float): Applied gain level.

    Returns:
        str: Content of the README file.
    """
    # 기본 헤더 생성
    content = f"# BRIR Info\n\n"
    content += f"Processed on {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}. Output sampling rate is {fs if fs is not None else hrir.fs} Hz.\n\n"

    # 항목 8: 적용된 노멀라이제이션 게인 추가
    if applied_gain is not None:
        content += f"## Applied Normalization Gain\n"
        content += f"{applied_gain:.2f} dB was applied to all channels.\n\n"

    # 기존 통계 테이블 생성 로직 (rt_name, table, speaker_names 등)
    rt_name = 'Reverb' # 기본값
    table_data = [] # 변수명 변경 (table -> table_data)
    # SPEAKER_NAMES 순서대로 정렬하되, 없는 스피커는 뒤로
    speaker_names_in_hrir = list(hrir.irs.keys())
    sorted_speaker_names = sorted(
        speaker_names_in_hrir,
        key=lambda x: SPEAKER_NAMES.index(x) if x in SPEAKER_NAMES else float('inf')
    )

    final_rt_name = 'Reverb' # 최종적으로 사용될 RTxx 이름, 모든 IR 검토 후 결정
    rt_values_for_naming = []

    for speaker in sorted_speaker_names:
        if speaker not in hrir.irs:
            continue
        pair = hrir.irs[speaker]
        
        peak_left_idx = pair['left'].peak_index()
        peak_right_idx = pair['right'].peak_index()
        itd = np.nan
        if peak_left_idx is not None and peak_right_idx is not None:
            itd = np.abs(peak_right_idx - peak_left_idx) / hrir.fs * 1e6 # us

        for side, ir_obj in pair.items():
            current_itd = 0.0
            if not np.isnan(itd):
                if speaker.endswith('L') and side == 'right': current_itd = itd
                elif speaker.endswith('R') and side == 'left': current_itd = itd

            pnr_val = np.nan
            length_ms = np.nan
            rt_val_ms = np.nan
            current_ir_rt_name = None

            peak_idx_current_ir = ir_obj.peak_index()
            if peak_idx_current_ir is not None:
                # PNR 계산
                peak_val_linear = np.abs(ir_obj.data[peak_idx_current_ir])
                # 데이터가 0~1로 정규화되었다고 가정. 그렇지 않다면 최대값으로 나눠야 함.
                # peak_val_db = 20 * np.log10(peak_val_linear / np.max(np.abs(ir_obj.data)) + 1e-9) # 좀 더 안전한 방식
                peak_val_db = 20 * np.log10(peak_val_linear + 1e-9) # 피크값의 dBFS (최대값이 1.0이라고 가정)
                
                decay_params_tuple = ir_obj.decay_params()
                if decay_params_tuple:
                    noise_floor_db = decay_params_tuple[2]
                    if not np.isnan(noise_floor_db) and not np.isnan(peak_val_db):
                        pnr_val = peak_val_db - noise_floor_db
                    
                    # Length 계산
                    tail_ind_calc = decay_params_tuple[1] # decay_params의 두 번째 값이 tail index (peak_idx + knee_idx)
                    if tail_ind_calc is not None and tail_ind_calc > peak_idx_current_ir:
                        length_ms = (tail_ind_calc - peak_idx_current_ir) / ir_obj.fs * 1000

                # RTxx 계산 (decay_times 사용)
                # decay_times() 호출 시 peak_ind 등을 전달해야 할 수 있음 (API 확인)
                # 현재 API는 decay_params() 내부 값들을 사용하므로, decay_params() 호출 후 사용 가능
                edt, rt20, rt30, rt60 = ir_obj.decay_times(peak_ind=decay_params_tuple[0] if decay_params_tuple else None,
                                                            knee_point_ind=decay_params_tuple[1] if decay_params_tuple else None,
                                                            noise_floor=decay_params_tuple[2] if decay_params_tuple else None,
                                                            window_size=decay_params_tuple[3] if decay_params_tuple else None)
                
                # 가장 긴 유효한 RTxx 값 선택
                if rt60 is not None and not np.isnan(rt60):
                    rt_val_ms = rt60 * 1000
                    current_ir_rt_name = 'RT60'
                elif rt30 is not None and not np.isnan(rt30):
                    rt_val_ms = rt30 * 1000
                    current_ir_rt_name = 'RT30'
                elif rt20 is not None and not np.isnan(rt20):
                    rt_val_ms = rt20 * 1000
                    current_ir_rt_name = 'RT20'
                elif edt is not None and not np.isnan(edt):
                    rt_val_ms = edt * 1000
                    current_ir_rt_name = 'EDT'
                
                if current_ir_rt_name:
                    rt_values_for_naming.append(current_ir_rt_name)

            table_data.append([
                speaker,
                side,
                f"{pnr_val:.1f} dB" if not np.isnan(pnr_val) else "N/A",
                f"{current_itd:.1f} us" if not np.isnan(current_itd) else "N/A",
                f"{length_ms:.1f} ms" if length_ms is not None and not np.isnan(length_ms) and length_ms >= 0 else "N/A", # 음수 길이 방지
                f"{rt_val_ms:.1f} ms" if rt_val_ms is not None and not np.isnan(rt_val_ms) else "N/A"
            ])

    # 모든 IR을 살펴본 후 최종 RTxx 이름 결정 (가장 많이 나온 유효한 이름 또는 우선순위)
    if rt_values_for_naming:
        # 예: 가장 빈번하게 나타난 RTxx 이름 사용
        from collections import Counter
        final_rt_name = Counter(rt_values_for_naming).most_common(1)[0][0]
    else:
        final_rt_name = 'RTxx' # 기본값

    if table_data:
        headers = ['Speaker', 'Side', 'PNR', 'ITD', 'Length', final_rt_name]
        content += tabulate(table_data, headers=headers, tablefmt='pipe')
        content += "\n\n"

    # 항목 9: 반사음 레벨 추가
    if estimator and hasattr(hrir, 'calculate_reflection_levels'):
        reflection_data = hrir.calculate_reflection_levels() # 인자 없이 호출
        if reflection_data:
            content += "## Reflection Levels (Direct vs. Early/Late)\n"
            # SPEAKER_NAMES 순서대로 정렬하되, 없는 스피커는 뒤로
            sorted_reflection_speakers = sorted(
                reflection_data.keys(),
                key=lambda x: SPEAKER_NAMES.index(x) if x in SPEAKER_NAMES else float('inf')
            )
            for speaker in sorted_reflection_speakers:
                if speaker not in reflection_data: # Should not happen due to sorted keys
                    continue
                sides_data = reflection_data[speaker]
                content += f"### {speaker}\n"
                if 'left' in sides_data and isinstance(sides_data['left'], dict):
                    content += f"- Left Ear: Early (20-50ms): {sides_data['left'].get('early_db', np.nan):.2f} dB, Late (50-150ms): {sides_data['left'].get('late_db', np.nan):.2f} dB\n"
                if 'right' in sides_data and isinstance(sides_data['right'], dict):
                    content += f"- Right Ear: Early (20-50ms): {sides_data['right'].get('early_db', np.nan):.2f} dB, Late (50-150ms): {sides_data['right'].get('late_db', np.nan):.2f} dB\n"
            content += "\n"
    
    # 파일에 쓰기
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(content)

    return content


def create_cli():
    arg_parser = argparse.ArgumentParser()
    arg_parser.add_argument('--dir_path', type=str, required=True, help='Path to directory for recordings and outputs.')
    arg_parser.add_argument('--test_signal', type=str, default=argparse.SUPPRESS,
                            help='Path to sine sweep test signal or pickled impulse response estimator. '
                                 'You can also use a predefined name or number: '
                                 '"default"/"1" (.pkl), "sweep"/"2" (.wav), "stereo"/"3" (FL,FR), '
                                 '"mono-left"/"4" (FL mono), "left"/"5" (FL stereo), "right"/"6" (FR stereo).')
    arg_parser.add_argument('--room_target', type=str, default=argparse.SUPPRESS,
                            help='Path to room target response AutoEQ style CSV file.')
    arg_parser.add_argument('--room_mic_calibration', type=str, default=argparse.SUPPRESS,
                            help='Path to room measurement microphone calibration file.')
    arg_parser.add_argument('--no_room_correction', action='store_false', dest='do_room_correction',
                            help='Skip room correction.')
    arg_parser.add_argument('--no_headphone_compensation', action='store_false', dest='do_headphone_compensation',
                            help='Skip headphone compensation.')
    arg_parser.add_argument('--no_equalization', action='store_false', dest='do_equalization',
                            help='Skip equalization.')
    arg_parser.add_argument('--fs', type=int, default=argparse.SUPPRESS, help='Output sampling rate in Hertz.')
    arg_parser.add_argument('--plot', action='store_true', help='Plot graphs for debugging.')
    arg_parser.add_argument('--channel_balance', type=str, default=argparse.SUPPRESS,
                            help='Channel balance correction by equalizing left and right ear results to the same '
                                 'level or frequency response. "trend" equalizes right side by the difference trend '
                                 'of right and left side. "left" equalizes right side to left side fr, "right" '
                                 'equalizes left side to right side fr, "avg" equalizes both to the average fr, "min" '
                                 'equalizes both to the minimum of left and right side frs. Number values will boost '
                                 'or attenuate right side relative to left side by the number of dBs. "mids" is the '
                                 'same as the numerical values but guesses the value automatically from mid frequency '
                                 'levels.')
    arg_parser.add_argument('--decay', type=str, default=argparse.SUPPRESS,
                            help='Target decay time in milliseconds to reach -60 dB. When the natural decay time is '
                                 'longer than the target decay time, a downward slope will be applied to decay tail. '
                                 'Decay cannot be increased with this. By default no decay time adjustment is done. '
                                 'A comma separated list of channel name and  reverberation time pairs, separated by '
                                 'a colon. If only a single numeric value is given, it is used for all channels. When '
                                 'some channel names are give but not all, the missing channels are not affected. For '
                                 'example "--decay=300" or "--decay=FL:500,FC:100,FR:500,SR:700,BR:700,BL:700,SL:700" '
                                 'or "--decay=FC:100".')
    arg_parser.add_argument('--target_level', type=float, default=argparse.SUPPRESS,
                            help='Target average gain level for left and right channels. This will sum together all '
                                 'left side impulse responses and right side impulse responses respectively and take '
                                 'the average gain from mid frequencies. The averaged level is then normalized to the '
                                 'given target level. This makes it possible to compare HRIRs with somewhat similar '
                                 'loudness levels. This should be negative in most cases to avoid clipping.')
    arg_parser.add_argument('--fr_combination_method', type=str, default='average',
                            help='Method for combining frequency responses of generic room measurements if there are '
                                 'more than one tracks in the file. "average" will simply average the frequency'
                                 'responses. "conservative" will take the minimum absolute value for each frequency '
                                 'but only if the values in all the measurements are positive or negative at the same '
                                 'time.')
    arg_parser.add_argument('--specific_limit', type=float, default=400,
                            help='Upper limit for room equalization with speaker-ear specific room measurements. '
                                 'Equalization will drop down to 0 dB at this frequency in the leading octave. 0 '
                                 'disables limit.')
    arg_parser.add_argument('--generic_limit', type=float, default=300,
                            help='Upper limit for room equalization with generic room measurements. '
                                 'Equalization will drop down to 0 dB at this frequency in the leading octave. 0 '
                                 'disables limit.')
    arg_parser.add_argument('--bass_boost', type=str, default=argparse.SUPPRESS,
                            help='Bass boost shelf. Sub-bass frequencies will be boosted by this amount. Can be '
                                 'either a single value for a gain in dB or a comma separated list of three values for '
                                 'parameters of a low shelf filter, where the first is gain in dB, second is center '
                                 'frequency (Fc) in Hz and the last is quality (Q). When only a single value (gain) is '
                                 'given, default values for Fc and Q are used which are 105 Hz and 0.76, respectively. '
                                 'For example "--bass_boost=6" or "--bass_boost=6,150,0.69".')
    arg_parser.add_argument('--tilt', type=float, default=argparse.SUPPRESS,
                            help='Target tilt in dB/octave. Positive value (upwards slope) will result in brighter '
                                 'frequency response and negative value (downwards slope) will result in darker '
                                 'frequency response. 1 dB/octave will produce nearly 10 dB difference in '
                                 'desired value between 20 Hz and 20 kHz. Tilt is applied with bass boost and both '
                                 'will affect the bass gain.')
    arg_parser.add_argument('--c', type=float, default=1.0, dest='head_ms', help='Head room in milliseconds for cropping impulse response heads. Default is 1.0 (ms). (항목 4)')
    arg_parser.add_argument('--jamesdsp', action='store_true', help='Generate true stereo IR file (jamesdsp.wav) for JamesDSP from FL/FR channels. (항목 6)')
    arg_parser.add_argument('--hangloose', action='store_true', help='Generate separate stereo IR for each channel for Hangloose Convolver. (항목 7)')
    args = vars(arg_parser.parse_args())
    if 'bass_boost' in args:
        bass_boost = args['bass_boost'].split(',')
        if len(bass_boost) == 1:
            args['bass_boost_gain'] = float(bass_boost[0])
            args['bass_boost_fc'] = 105
            args['bass_boost_q'] = 0.76
        elif len(bass_boost) == 3:
            args['bass_boost_gain'] = float(bass_boost[0])
            args['bass_boost_fc'] = float(bass_boost[1])
            args['bass_boost_q'] = float(bass_boost[2])
        else:
            raise ValueError('"--bass_boost" must have one value or three values separated by commas!')
        del args['bass_boost']
    if 'decay' in args:
        decay = dict()
        try:
            # Single float value
            decay = {ch: float(args['decay']) / 1000 for ch in SPEAKER_NAMES}
        except ValueError:
            # Channels separated
            for ch_t in args['decay'].split(','):
                decay[ch_t.split(':')[0].upper()] = float(ch_t.split(':')[1]) / 1000
        args['decay'] = decay
    return args


if __name__ == '__main__':
    main(**create_cli())
