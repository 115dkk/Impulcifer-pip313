# -*- coding: utf-8 -*-

# https://en.wikipedia.org/wiki/Surround_sound
# TrueHD 지원을 위해 확장된 스피커 이름 목록
SPEAKER_NAMES = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'WL', 'WR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR']

SPEAKER_PATTERN = f'({"|".join(SPEAKER_NAMES + ["X"])})'
SPEAKER_LIST_PATTERN = r'([A-Z]{2,3}(,[A-Z]{2,3})*)'

# Left/right/center partition of the speaker layout. Includes LFE (present in
# HEXADECAGONAL_TRACK_ORDER but not SPEAKER_NAMES). Single source of truth for
# "which side of the head is this speaker on" — positional checks like
# speaker[1] == 'R' break on 3-letter top-layer names ('TFL'[1] == 'F').
LEFT_SIDE_SPEAKERS = frozenset({'FL', 'SL', 'BL', 'WL', 'TFL', 'TSL', 'TBL'})
RIGHT_SIDE_SPEAKERS = frozenset({'FR', 'SR', 'BR', 'WR', 'TFR', 'TSR', 'TBR'})
CENTER_SPEAKERS = frozenset({'FC', 'LFE'})


def speaker_side(name: str) -> str:
    """Classify a speaker name as 'left', 'right' or 'center'."""
    name_upper = name.upper()
    if name_upper in LEFT_SIDE_SPEAKERS:
        return 'left'
    if name_upper in RIGHT_SIDE_SPEAKERS:
        return 'right'
    return 'center'

TRUEHD_11CH_ORDER = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'TFL', 'TFR', 'TBL', 'TBR']  # 7.0.4
TRUEHD_13CH_ORDER = ['FL', 'FR', 'FC', 'BL', 'BR', 'SL', 'SR', 'TFL', 'TFR', 'TSL', 'TSR', 'TBL', 'TBR']  # 7.0.6

CHANNEL_LAYOUT_MAP = {
    11: TRUEHD_11CH_ORDER,
    13: TRUEHD_13CH_ORDER
}

SPEAKER_ANGLES = {
    'FL': 30,
    'FR': -30,
    'FC': 0,
    'BL': 150,
    'BR': -150,
    'SL': 90,
    'SR': -90,
    'WL': 0,  # 기본값, 필요시 수정
    'WR': 0,  # 기본값, 필요시 수정
    'TFL': 0,  # 기본값, 필요시 수정
    'TFR': 0,  # 기본값, 필요시 수정
    'TSL': 0,  # 기본값, 필요시 수정
    'TSR': 0,  # 기본값, 필요시 수정
    'TBL': 0,  # 기본값, 필요시 수정
    'TBR': 0  # 기본값, 필요시 수정
}

# Speaker delays relative to the nearest speaker
SPEAKER_DELAYS = {
    _speaker: 0 for _speaker in SPEAKER_NAMES
}

# Ipsilateral (left/right) speaker pairs used for onset alignment in
# ``HRIR.align_ipsilateral_all``. ``FC`` is paired with itself so its own
# left/right ears are aligned. Single source of truth shared by
# ``impulcifer.main`` and the ``align_ipsilateral_all`` default.
# (This is the alignment *pair* list; the *group* list in
# ``align_onset_groups_peak_leftref`` uses a 1-tuple ``("FC",)`` and is a
# deliberately different structure — do not merge it here.)
IPSILATERAL_PAIRS = (
    ("FL", "FR"),
    ("SL", "SR"),
    ("BL", "BR"),
    ("TFL", "TFR"),
    ("TSL", "TSR"),
    ("TBL", "TBR"),
    ("FC", "FC"),
    ("WL", "WR"),
)

# Each channel, left and right
IR_ORDER = []
# SPL change relative to middle of the head (currently unused)
IR_ROOM_SPL = {
    sp: {'left': 0.0, 'right': 0.0}
    for sp in SPEAKER_NAMES
}

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
                      'FR-left', 'SR-right', 'SR-left', 'BR-right', 'BR-left', 'FC-right', 'WL-left', 'WL-right',
                      'WR-left', 'WR-right', 'TFL-left', 'TFL-right', 'TFR-left', 'TFR-right', 'TSL-left',
                      'TSL-right', 'TSR-left', 'TSR-right', 'TBL-left', 'TBL-right', 'TBR-left', 'TBR-right']

HEXADECAGONAL_TRACK_ORDER = ['FL-left', 'FL-right', 'FR-left', 'FR-right', 'FC-left', 'FC-right', 'LFE-left',
                             'LFE-right', 'BL-left', 'BL-right', 'BR-left', 'BR-right', 'SL-left', 'SL-right',
                             'SR-left', 'SR-right', 'WL-left', 'WL-right', 'WR-left', 'WR-right', 'TFL-left',
                             'TFL-right', 'TFR-left', 'TFR-right', 'TSL-left', 'TSL-right', 'TSR-left',
                             'TSR-right', 'TBL-left', 'TBL-right', 'TBR-left', 'TBR-right']

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


def get_data_path():
    """패키지 내 데이터 폴더 경로를 반환합니다.

    런타임 데이터 위치는 ``infra.resource_helper``가 단일 진실 공급원이다.
    dev / pip-install / Nuitka standalone 모두를 처리하는
    ``infra.resource_helper.get_resource_path('data')``로 위임한다.
    """
    from infra.resource_helper import get_resource_path
    return get_resource_path('data')
