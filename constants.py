# -*- coding: utf-8 -*-

from utils import versus_distance
import os
import importlib.resources as pkg_resources

# https://en.wikipedia.org/wiki/Surround_sound
SPEAKER_NAMES = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'WL', 'WR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR']

# 파이썬 3.13.2 스타일로 f-string 사용
SPEAKER_PATTERN = f'({"|".join(SPEAKER_NAMES + ["X"])})'
# format() 대신 f-string 사용
SPEAKER_LIST_PATTERN = fr'{SPEAKER_PATTERN}+(,{SPEAKER_PATTERN})*'

SPEAKER_ANGLES = {
    'FL': 30,
    'FR': -30,
    'FC': 0,
    'BL': 150,
    'BR': -150,
    'SL': 90,
    'SR': -90,
    'WL': 0, # 기본값, 필요시 수정
    'WR': 0, # 기본값, 필요시 수정
    'TFL': 0, # 기본값, 필요시 수정
    'TFR': 0, # 기본값, 필요시 수정
    'TSL': 0, # 기본값, 필요시 수정
    'TSR': 0, # 기본값, 필요시 수정
    'TBL': 0, # 기본값, 필요시 수정
    'TBR': 0  # 기본값, 필요시 수정
}

# Speaker delays relative to the nearest speaker
SPEAKER_DELAYS = {
    _speaker: 0 for _speaker in SPEAKER_NAMES
}

# Each channel, left and right
IR_ORDER = []
# SPL change relative to middle of the head - PR3에서는 이부분이 비활성화됨
IR_ROOM_SPL = {
    sp: {'left': 0.0, 'right': 0.0}
    for sp in SPEAKER_NAMES
}
#for _speaker in SPEAKER_NAMES:
#    if _speaker not in IR_ROOM_SPL:
#        IR_ROOM_SPL[_speaker] = dict()
#    for _side in ['left', 'right']:
#        IR_ORDER.append(f'{_speaker}-{_side}')
#        IR_ROOM_SPL[_speaker][_side] = versus_distance(
#            angle=abs(SPEAKER_ANGLES[_speaker]),
#            ear='primary' if _side[0] == _speaker.lower()[1] else 'secondary'
#        )[2]

COLORS = {
    'lightblue': '#7db4db',
    'blue': '#1f77b4',
    'pink': '#dd8081',
    'red': '#d62728',
    'lightpurple': '#ecdef9',
    'purple': '#680fb9',
    'green': '#2ca02c'
}

HESUVI_TRACK_ORDER = ['FL-left', 'FL-right', 'SL-left', 'SL-right', 'BL-left', 'BL-right', 'FC-left', 'FR-right',
                      'FR-left', 'SR-right', 'SR-left', 'BR-right', 'BR-left', 'FC-right', 'WL-left', 'WL-right', 'WR-left', 'WR-right', 'TFL-left', 'TFL-right',
                             'TFR-left', 'TFR-right', 'TSL-left', 'TSL-right', 'TSR-left', 'TSR-right',
                             'TBL-left', 'TBL-right', 'TBR-left', 'TBR-right']

HEXADECAGONAL_TRACK_ORDER = ['FL-left', 'FL-right', 'FR-left', 'FR-right', 'FC-left', 'FC-right', 'LFE-left',
                             'LFE-right', 'BL-left', 'BL-right', 'BR-left', 'BR-right', 'SL-left', 'SL-right',
                             'SR-left', 'SR-right', 'WL-left', 'WL-right', 'WR-left', 'WR-right', 'TFL-left', 'TFL-right',
                             'TFR-left', 'TFR-right', 'TSL-left', 'TSL-right', 'TSR-left', 'TSR-right',
                             'TBL-left', 'TBL-right', 'TBR-left', 'TBR-right']

# 기본 테스트 신호 파일 목록
TEST_SIGNALS = {
    'default': 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl',
    'sweep': 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    'stereo': 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    'mono-left': 'sweep-seg-FL-mono-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    'left': 'sweep-seg-FL-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    'right': 'sweep-seg-FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    '1': 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl',
    '2': 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    '3': 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    '4': 'sweep-seg-FL-mono-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    '5': 'sweep-seg-FL-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav',
    '6': 'sweep-seg-FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'
}

# 패키지 내 데이터 폴더 경로
def get_data_path():
    """패키지 내 데이터 폴더 경로를 반환합니다."""
    try:
        # 패키지로 설치된 경우
        with pkg_resources.path('impulcifer_py313', 'data') as data_path:
            return data_path
    except (ImportError, ModuleNotFoundError):
        # 로컬 개발 환경인 경우
        script_dir = os.path.dirname(os.path.abspath(__file__))
        return os.path.join(script_dir, 'data')
