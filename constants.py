# -*- coding: utf-8 -*-

import os
import importlib.resources

# https://en.wikipedia.org/wiki/Surround_sound
# TrueHD 지원을 위해 확장된 스피커 이름 목록
SPEAKER_NAMES = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'WL', 'WR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR']

# 파이썬 3.13.2 스타일로 f-string 사용 - FC와 TSL/TSR 같은 3글자 채널 지원
SPEAKER_PATTERN = f'({"|".join(SPEAKER_NAMES + ["X"])})'
# format() 대신 f-string 사용 - 3글자 채널 지원
SPEAKER_LIST_PATTERN = r'([A-Z]{2,3}(,[A-Z]{2,3})*)'

# TrueHD 채널 레이아웃 정의
TRUEHD_11CH_ORDER = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'TFL', 'TFR', 'TBL', 'TBR']  # 7.0.4
TRUEHD_13CH_ORDER = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR']  # 7.0.6

# 채널 레이아웃 매핑
CHANNEL_LAYOUT_MAP = {
    11: TRUEHD_11CH_ORDER,
    13: TRUEHD_13CH_ORDER
}

# 자동 생성 가능한 채널 정의 (FC, TSL, TSR 제거)
AUTO_GENERATABLE_CHANNELS = {
    # 'FC': {
    #     'sources': ['FL', 'FR'],
    #     'weights': [0.5, 0.5],
    #     'description': 'Center from Front Left/Right'
    # },
    # 'TSL': {
    #     'sources': ['TFL', 'SL'],
    #     'weights': [0.6, 0.4],
    #     'description': 'Top Side Left from Top Front Left and Side Left'
    # },
    # 'TSR': {
    #     'sources': ['TFR', 'SR'],
    #     'weights': [0.6, 0.4],
    #     'description': 'Top Side Right from Top Front Right and Side Right'
    # }
}

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
        if hasattr(importlib.resources, 'files'):
            return str(importlib.resources.files('impulcifer_py313').joinpath('data'))
        elif hasattr(importlib.resources, 'path'):
            with importlib.resources.path('impulcifer_py313', 'data') as data_path:
                return str(data_path)
    except (ImportError, ModuleNotFoundError):
        pass
    
    # 폴백: 현재 파일 기준 상대 경로
    script_dir = os.path.dirname(os.path.abspath(__file__))
    return os.path.join(script_dir, 'data')
