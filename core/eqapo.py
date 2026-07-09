# -*- coding: utf-8 -*-
"""EqualizerAPO(-XT) 설정 파일 파서.

EqualizerAPO 및 EqualizerAPO-XT(115dkk/EqualizerAPO-XT)의 설정 텍스트를 파싱하여
좌/우 채널별 EQ 크기 응답(dB) 곡선으로 변환한다. 크기 응답으로 표현 가능한
명령(Filter 바이쿼드, Filter IIR, Preamp, GraphicEQ)은 적용하고, 표현할 수 없는
명령(Convolution, Copy, Delay, VSTPlugin 등)은 사유와 함께 바이패스 목록으로
보고한다. Channel 명령의 L/R 스코핑과 Include(상대 경로 해석 가능 시)도 지원한다.

수식은 EqualizerAPO-XT 소스와 동일하게 유지한다:
- filters/BiQuad.cpp: RBJ Audio EQ Cookbook 계수 및 gainAt() 폐형식
- filters/BiQuadFilterFactory.cpp: Filter 라인 파싱 규칙과 기본값
  (LP/HP/BP 기본 Q=1/sqrt(2), 셸프 기본 S=0.9, NO 기본 Q=30, 슬로프/12 규칙,
  코너 주파수 변환, REW 천단위 구분자 해석)
- filters/BiQuadFilter.cpp: 셸프 코너 주파수 → 중심 주파수 변환
- filters/GraphicEQCommand.cpp + helpers/GainIterator.cpp: 로그 주파수 축
  선형 보간(양끝 평탄 연장)
- filters/IIRFilterFactory.cpp: "ON IIR Order n Coefficients ..." 파싱
- filters/PreampFilterFactory.cpp: "Preamp: x dB" 파싱(콤마 소수점 허용)
- engine/FilterEngine.Configuration.cpp: 첫 ':' 기준 키/값 분리, 키 트리밍

이 모듈은 numpy(+IIR 평가 시 scipy.signal)만 사용하며 autoeq에 의존하지 않는다.
FrequencyResponse 객체 구성은 호출 측(impulcifer.equalization)에서 수행한다.
"""

from __future__ import annotations

import math
import os
import re
from dataclasses import dataclass, field

import numpy as np

# 바이패스 사유 토큰. 로깅 측에서 cli_eqapo_reason_<token> i18n 키로 변환된다.
REASON_UNSUPPORTED = "unsupported"
REASON_MALFORMED = "malformed"
REASON_CONDITIONAL = "conditional"
REASON_CHANNEL_SCOPE = "channel_scope"
REASON_INCLUDE_NOT_FOUND = "include_not_found"
REASON_EXPRESSION = "expression"
REASON_SCOPING_IGNORED = "scoping_ignored"
REASON_DISABLED = "disabled"

# EqualizerAPO-XT FilterFactoryRegistry에 등록된 명령 키워드 중 크기 응답으로
# 표현할 수 없어 바이패스되는 것들. 미지의 키워드(향후 XT 확장 포함)는
# _KNOWN_COMMAND_RE로 잡아 동일하게 바이패스한다.
_UNSUPPORTED_COMMANDS = frozenset(
    {
        "Delay",
        "Copy",
        "Convolution",
        "MultiConvolution",
        "Eval",
        "VSTPlugin",
        "LoudnessCorrection",
    }
)
# 파싱 흐름을 제어하지만 필터를 만들지 않는 명령. Device/Stage는 장치/스테이지
# 스코핑을 평가할 수 없으므로 무시하고 계속 진행한다(경고 보고).
_SCOPING_COMMANDS = frozenset({"Device", "Stage"})
_CONDITIONAL_COMMANDS = frozenset({"If", "ElseIf", "Else", "EndIf"})

# 명령처럼 보이는 키(영숫자 단어, "Filter 12"처럼 뒤에 숫자 허용).
_KNOWN_COMMAND_RE = re.compile(r"^[A-Za-z][A-Za-z0-9]*(?:\s+\d+)?$")

# BiQuadFilterFactory.cpp의 정규식들을 그대로 이식.
_RE_TYPE = re.compile(r"^\s*ON\s+([A-Za-z]+)")
_RE_FREQ = re.compile(r"\s+Fc\s*([-+0-9.eE\u00A0]+)\s*H\s*z")
_RE_GAIN = re.compile(r"\s+Gain\s*([-+0-9.eE]+)\s*dB")
_RE_Q = re.compile(r"\s+Q\s*([-+0-9.eE]+)")
_RE_BW = re.compile(r"\s+BW\s+Oct\s*([-+0-9.eE]+)")
_RE_SLOPE = re.compile(r"^\s*([-+0-9.eE]+)\s*dB")
# IIRFilterFactory.cpp
_RE_IIR_ORDER = re.compile(r"\s*Order\s+([0-9]+)")
_RE_IIR_COEFFS = re.compile(r"\s+Coefficients((?: [-+0-9.eE]+)+)")
# GraphicEQCommand.cpp
_RE_NUMBER = re.compile(r"[-+0-9.eE]+")

# wcstod와 유사하게 문자열 선두의 유효한 실수만 파싱한다.
_LEADING_FLOAT_RE = re.compile(r"^\s*[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?")

# BiQuadCommand.cpp의 biquadTypeFromName과 동일(대소문자 구분 유지).
PEAKING = "PEAKING"
LOW_PASS = "LOW_PASS"
HIGH_PASS = "HIGH_PASS"
BAND_PASS = "BAND_PASS"
LOW_SHELF = "LOW_SHELF"
HIGH_SHELF = "HIGH_SHELF"
NOTCH = "NOTCH"
ALL_PASS = "ALL_PASS"

_BIQUAD_TYPE_FROM_NAME = {
    "PK": PEAKING,
    "PEQ": PEAKING,
    "Modal": PEAKING,
    "LP": LOW_PASS,
    "HP": HIGH_PASS,
    "LPQ": LOW_PASS,
    "HPQ": HIGH_PASS,
    "BP": BAND_PASS,
    "LS": LOW_SHELF,
    "HS": HIGH_SHELF,
    "LSC": LOW_SHELF,
    "HSC": HIGH_SHELF,
    "NO": NOTCH,
    "AP": ALL_PASS,
}

_MAX_INCLUDE_DEPTH = 8
_LOG_EPS = 1e-30


@dataclass
class EqApoCommandReport:
    """바이패스/스킵된 한 라인에 대한 보고."""

    line_number: int
    command: str
    text: str
    reason: str


@dataclass
class _BiquadSpec:
    type: str
    db_gain: float
    freq: float
    bandwidth_or_q_or_s: float
    is_bandwidth_or_s: bool
    is_corner_freq: bool
    description: str


@dataclass
class EqApoEqualization:
    """파싱 결과: 좌/우 EQ 곡선과 적용/바이패스 내역."""

    frequency: np.ndarray
    left_db: np.ndarray
    right_db: np.ndarray
    applied_left: int = 0
    applied_right: int = 0
    preamp_left: float = 0.0
    preamp_right: float = 0.0
    applied: list = field(default_factory=list)  # 사람이 읽을 수 있는 적용 필터 설명
    bypassed: list = field(default_factory=list)  # EqApoCommandReport
    skipped: list = field(default_factory=list)  # 비활성(OFF/None) 필터

    @property
    def channel_split(self) -> bool:
        return not np.array_equal(self.left_db, self.right_db)


def looks_like_eqapo_config(text: str) -> bool:
    """텍스트가 EqualizerAPO 설정 형식으로 보이는지 검사한다.

    AutoEQ CSV나 2열 숫자 데이터에는 존재할 수 없는 명령 키("Filter 1:",
    "Preamp:", "GraphicEQ:" 등)가 한 줄이라도 있으면 True.
    """
    known = (
        {"Preamp", "GraphicEQ", "Channel", "Include"}
        | _UNSUPPORTED_COMMANDS
        | _SCOPING_COMMANDS
        | _CONDITIONAL_COMMANDS
    )
    for line in text.splitlines():
        if ":" not in line:
            continue
        key = line.split(":", 1)[0].strip()
        if key.startswith("#"):
            continue
        if key in known or re.match(r"^Filter(?:\s*\d+)?$", key):
            return True
    return False


def _parse_double(s: str) -> float:
    """wcstod처럼 선두의 실수를 파싱하고 실패 시 0.0을 반환한다."""
    m = _LEADING_FLOAT_RE.match(s)
    if m is None:
        return 0.0
    try:
        return float(m.group())
    except ValueError:
        return 0.0


def _parse_freq(token: str) -> float:
    """BiQuadFilterFactory::getFreq 포팅.

    비줄바꿈 공백(천단위 구분자) 제거 후 실수를 파싱하고, "1.250"처럼 소수점이
    끝에서 4번째에 있으면 REW 천단위 구분자로 간주하여 1000을 곱한다.
    """
    s = token.replace("\u00A0", "")
    m = _LEADING_FLOAT_RE.match(s)
    if m is None:
        return -1.0
    result = float(m.group())
    if len(s) >= 5 and "e" not in s and "E" not in s and s[-4] == ".":
        result *= 1000.0
    return result


def _biquad_coefficients(type_, db_gain, freq, srate, bandwidth_or_q_or_s, is_bandwidth_or_s):
    """BiQuad.cpp 생성자 포팅. a0로 정규화된 (b0, b1, b2, a1, a2)를 반환한다."""
    if type_ in (PEAKING, LOW_SHELF, HIGH_SHELF):
        big_a = 10.0 ** (db_gain / 40.0)
    else:
        big_a = 10.0 ** (db_gain / 20.0)
    omega = 2.0 * math.pi * freq / srate
    sn = math.sin(omega)
    cs = math.cos(omega)

    if not is_bandwidth_or_s:  # Q
        alpha = sn / (2.0 * bandwidth_or_q_or_s)
    elif type_ in (LOW_SHELF, HIGH_SHELF):  # S
        alpha = sn / 2.0 * math.sqrt((big_a + 1.0 / big_a) * (1.0 / bandwidth_or_q_or_s - 1.0) + 2.0)
    else:  # BW
        alpha = sn * math.sinh(math.log(2.0) / 2.0 * bandwidth_or_q_or_s * omega / sn)

    beta = 2.0 * math.sqrt(big_a) * alpha

    if type_ == LOW_PASS:
        b0, b1, b2 = (1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0
        a0, a1, a2 = 1.0 + alpha, -2.0 * cs, 1.0 - alpha
    elif type_ == HIGH_PASS:
        b0, b1, b2 = (1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0
        a0, a1, a2 = 1.0 + alpha, -2.0 * cs, 1.0 - alpha
    elif type_ == BAND_PASS:
        b0, b1, b2 = alpha, 0.0, -alpha
        a0, a1, a2 = 1.0 + alpha, -2.0 * cs, 1.0 - alpha
    elif type_ == NOTCH:
        b0, b1, b2 = 1.0, -2.0 * cs, 1.0
        a0, a1, a2 = 1.0 + alpha, -2.0 * cs, 1.0 - alpha
    elif type_ == ALL_PASS:
        b0, b1, b2 = 1.0 - alpha, -2.0 * cs, 1.0 + alpha
        a0, a1, a2 = 1.0 + alpha, -2.0 * cs, 1.0 - alpha
    elif type_ == PEAKING:
        b0, b1, b2 = 1.0 + alpha * big_a, -2.0 * cs, 1.0 - alpha * big_a
        a0, a1, a2 = 1.0 + alpha / big_a, -2.0 * cs, 1.0 - alpha / big_a
    elif type_ == LOW_SHELF:
        b0 = big_a * ((big_a + 1.0) - (big_a - 1.0) * cs + beta)
        b1 = 2.0 * big_a * ((big_a - 1.0) - (big_a + 1.0) * cs)
        b2 = big_a * ((big_a + 1.0) - (big_a - 1.0) * cs - beta)
        a0 = (big_a + 1.0) + (big_a - 1.0) * cs + beta
        a1 = -2.0 * ((big_a - 1.0) + (big_a + 1.0) * cs)
        a2 = (big_a + 1.0) + (big_a - 1.0) * cs - beta
    elif type_ == HIGH_SHELF:
        b0 = big_a * ((big_a + 1.0) + (big_a - 1.0) * cs + beta)
        b1 = -2.0 * big_a * ((big_a - 1.0) + (big_a + 1.0) * cs)
        b2 = big_a * ((big_a + 1.0) + (big_a - 1.0) * cs - beta)
        a0 = (big_a + 1.0) - (big_a - 1.0) * cs + beta
        a1 = 2.0 * ((big_a - 1.0) - (big_a + 1.0) * cs)
        a2 = (big_a + 1.0) - (big_a - 1.0) * cs - beta
    else:  # pragma: no cover - 호출 측에서 타입을 검증한다
        raise ValueError(f"Unknown biquad type: {type_}")

    return b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0


def _biquad_gain_db(frequency, coeffs, srate):
    """BiQuad::gainAt 포팅(벡터화). 정규화 계수로부터 크기 응답(dB)을 계산한다."""
    b0, b1, b2, a1, a2 = coeffs
    a0 = 1.0
    sn = np.sin(np.pi * frequency / srate)  # sin(omega / 2)
    phi = sn * sn
    num = (b0 + b1 + b2) ** 2 - 4.0 * (b0 * b1 + 4.0 * b0 * b2 + b1 * b2) * phi + 16.0 * b0 * b2 * phi * phi
    den = (a0 + a1 + a2) ** 2 - 4.0 * (a0 * a1 + 4.0 * a0 * a2 + a1 * a2) * phi + 16.0 * a0 * a2 * phi * phi
    return 10.0 * np.log10(np.maximum(num, _LOG_EPS)) - 10.0 * np.log10(np.maximum(den, _LOG_EPS))


def _evaluate_biquad(spec: _BiquadSpec, frequency, srate):
    """BiQuadFilter::initialize의 코너 주파수 변환 후 크기 응답을 평가한다."""
    biquad_freq = spec.freq
    if spec.is_corner_freq and spec.type in (LOW_SHELF, HIGH_SHELF):
        s = spec.bandwidth_or_q_or_s
        if not spec.is_bandwidth_or_s:  # Q가 주어진 경우 동등한 S로 환산
            q = spec.bandwidth_or_q_or_s
            a = 10.0 ** (spec.db_gain / 40.0)
            s = 1.0 / ((1.0 / (q * q) - 2.0) / (a + 1.0 / a) + 1.0)
        center_freq_factor = 10.0 ** (abs(spec.db_gain) / 80.0 / s)
        if spec.type == LOW_SHELF:
            biquad_freq *= center_freq_factor
        else:
            biquad_freq /= center_freq_factor
    coeffs = _biquad_coefficients(
        spec.type, spec.db_gain, biquad_freq, srate, spec.bandwidth_or_q_or_s, spec.is_bandwidth_or_s
    )
    return _biquad_gain_db(frequency, coeffs, srate)


def _parse_biquad(parameters: str):
    """BiQuadFilterFactory::parseCommand 포팅.

    Returns:
        (_BiquadSpec, None) 성공 시,
        (None, "disabled") 비활성 필터(ON 누락 또는 타입 None) 시,
        (None, "malformed") 파싱 실패 시.
    """
    # 콤마 소수점을 마침표로 정규화 (StringHelper::normalizeDecimalComma)
    parameters = parameters.replace(",", ".")

    m = _RE_TYPE.search(parameters)
    if m is None:
        # "OFF ..." 또는 형식이 다른 라인: EqualizerAPO는 필터를 만들지 않는다.
        return None, REASON_DISABLED

    type_string = m.group(1)
    type_ = _BIQUAD_TYPE_FROM_NAME.get(type_string)
    if type_ is None:
        if type_string == "None":
            return None, REASON_DISABLED
        return None, REASON_MALFORMED

    parameters = parameters[m.end():]

    freq = 0.0
    gain = 0.0
    bandwidth_or_q_or_s = 0.0
    is_bandwidth_or_s = False
    is_corner_freq = False

    m = _RE_FREQ.search(parameters)
    if m is None:
        return None, REASON_MALFORMED
    freq = _parse_freq(m.group(1))
    if not math.isfinite(freq) or freq <= 0.0:
        # EqualizerAPO는 음수 주파수도 그대로 계산하지만 결과가 무의미하므로
        # 안전하게 바이패스한다.
        return None, REASON_MALFORMED

    m = _RE_GAIN.search(parameters)
    if m is not None:
        if type_ not in (LOW_PASS, HIGH_PASS, NOTCH, ALL_PASS):
            gain = _parse_double(m.group(1))
    elif type_ in (PEAKING, LOW_SHELF, HIGH_SHELF):
        return None, REASON_MALFORMED

    m = _RE_Q.search(parameters)
    if m is not None:
        bandwidth_or_q_or_s = _parse_double(m.group(1))

    m = _RE_BW.search(parameters)
    if m is not None and type_ not in (LOW_SHELF, HIGH_SHELF):
        bandwidth_or_q_or_s = _parse_double(m.group(1))
        is_bandwidth_or_s = True

    m = _RE_SLOPE.match(parameters)
    if m is not None and type_ in (LOW_SHELF, HIGH_SHELF):
        bandwidth_or_q_or_s = _parse_double(m.group(1))
        is_bandwidth_or_s = True

    if bandwidth_or_q_or_s == 0.0:
        if type_ in (PEAKING, ALL_PASS):
            return None, REASON_MALFORMED
        elif type_ in (LOW_PASS, HIGH_PASS, BAND_PASS):
            bandwidth_or_q_or_s = 1.0 / math.sqrt(2.0)
        elif type_ in (LOW_SHELF, HIGH_SHELF):
            bandwidth_or_q_or_s = 0.9  # RoomEQWizard 실험값 (BiQuadFilterFactory.cpp)
            is_bandwidth_or_s = True
        elif type_ == NOTCH:
            bandwidth_or_q_or_s = 30.0  # RoomEQWizard 실험값
    else:
        if not math.isfinite(bandwidth_or_q_or_s) or bandwidth_or_q_or_s < 0.0:
            return None, REASON_MALFORMED
        if type_ in (LOW_SHELF, HIGH_SHELF):
            if is_bandwidth_or_s:
                # 슬로프 최대값은 12 dB에서 S=1
                bandwidth_or_q_or_s /= 12.0
            if not type_string.endswith("C"):
                is_corner_freq = True

    description = f"{type_string} Fc {freq:g} Hz"
    if type_ in (PEAKING, LOW_SHELF, HIGH_SHELF):
        description += f" Gain {gain:g} dB"
    if is_bandwidth_or_s and type_ in (LOW_SHELF, HIGH_SHELF):
        description += f" S {bandwidth_or_q_or_s:g}"
    elif is_bandwidth_or_s:
        description += f" BW Oct {bandwidth_or_q_or_s:g}"
    else:
        description += f" Q {bandwidth_or_q_or_s:g}"

    return (
        _BiquadSpec(
            type=type_,
            db_gain=gain,
            freq=freq,
            bandwidth_or_q_or_s=bandwidth_or_q_or_s,
            is_bandwidth_or_s=is_bandwidth_or_s,
            is_corner_freq=is_corner_freq,
            description=description,
        ),
        None,
    )


def _parse_iir(parameters: str):
    """IIRFilterFactory::parseCommand 포팅. (b계수, a계수) 또는 None을 반환한다."""
    m = _RE_TYPE.search(parameters)
    if m is None or m.group(1) != "IIR":
        return None
    m = _RE_IIR_ORDER.search(parameters)
    if m is None:
        return None
    order = int(m.group(1))
    if order < 1:
        return None
    m = _RE_IIR_COEFFS.search(parameters)
    if m is None:
        return None
    tokens = m.group(1).split(" ")
    tokens = [t for t in tokens if t != ""]
    if len(tokens) != (order + 1) * 2:
        return None
    coefficients = [_parse_double(t) for t in tokens]
    return coefficients[: order + 1], coefficients[order + 1:]


def _evaluate_iir(b, a, frequency, srate):
    """IIR 필터의 크기 응답(dB)을 평가한다."""
    from scipy.signal import freqz

    _, h = freqz(b, a, worN=np.asarray(frequency, dtype=float), fs=srate)
    return 20.0 * np.log10(np.abs(h) + _LOG_EPS)


def _parse_graphic_eq_nodes(parameters: str):
    """GraphicEQCommand::parse 포팅. 주파수 오름차순 (freq, gain) 리스트를 반환한다."""
    value = parameters
    if "." not in value:
        value = value.replace(",", ".")
    numbers = _RE_NUMBER.findall(value)
    nodes = []
    for i in range(0, len(numbers) - 1, 2):
        freq = _parse_double(numbers[i])
        gain = _parse_double(numbers[i + 1])
        nodes.append((freq, gain))
    nodes.sort(key=lambda node: node[0])
    return nodes


def _evaluate_graphic_eq(nodes, frequency):
    """GainIterator::gainAt 포팅: 로그 주파수 축 선형 보간, 양끝 평탄 연장."""
    if not nodes:
        return np.zeros(len(frequency))
    node_f = np.array([max(node[0], 1e-9) for node in nodes], dtype=float)
    node_g = np.array([node[1] for node in nodes], dtype=float)
    return np.interp(np.log(np.asarray(frequency, dtype=float)), np.log(node_f), node_g)


def _parse_channel_scope(parameters: str):
    """ChannelCommand::parse 포팅 후 스테레오 L/R 스코프로 해석한다.

    공백/콤마로 토큰화하고 대문자화한다. "ALL"은 양쪽, "L"/"1"은 좌,
    "R"/"2"는 우. 그 외 채널(C, LFE, SL 등)은 헤드폰 출력에 없으므로
    스코프에 포함하지 않는다.
    """
    tokens = [t.upper() for t in re.split(r"[\s,]+", parameters) if t]
    scope = set()
    for token in tokens:
        if token == "ALL":
            scope.update(("L", "R"))
        elif token in ("L", "1"):
            scope.add("L")
        elif token in ("R", "2"):
            scope.add("R")
    if not tokens:
        # 빈 Channel 라인은 모든 채널 선택으로 되돌린다.
        scope = {"L", "R"}
    return scope


class _ParseState:
    def __init__(self, frequency, srate):
        self.frequency = np.asarray(frequency, dtype=float)
        self.srate = srate
        self.left_db = np.zeros(len(self.frequency))
        self.right_db = np.zeros(len(self.frequency))
        self.scope = {"L", "R"}
        self.applied = []
        self.bypassed = []
        self.skipped = []
        self.applied_left = 0
        self.applied_right = 0
        self.preamp_left = 0.0
        self.preamp_right = 0.0

    def add_response(self, gain_db, description):
        applied_to = []
        if "L" in self.scope:
            self.left_db += gain_db
            self.applied_left += 1
            applied_to.append("L")
        if "R" in self.scope:
            self.right_db += gain_db
            self.applied_right += 1
            applied_to.append("R")
        self.applied.append(f"{description} [{'+'.join(applied_to)}]")


def parse_eqapo_config(text, srate, frequency, base_dir=None):
    """EqualizerAPO 설정 텍스트를 파싱하여 EqApoEqualization을 반환한다.

    Args:
        text: 설정 파일 내용 전체
        srate: 샘플레이트 (biquad 크기 응답 계산에 필요)
        frequency: 응답을 평가할 주파수 배열 (Hz)
        base_dir: Include 상대 경로 해석의 기준 디렉토리 (None이면 Include 바이패스)

    Returns:
        EqApoEqualization
    """
    state = _ParseState(frequency, srate)
    _parse_lines(text.splitlines(), state, base_dir, depth=0, visited=set())
    return EqApoEqualization(
        frequency=state.frequency,
        left_db=state.left_db,
        right_db=state.right_db,
        applied_left=state.applied_left,
        applied_right=state.applied_right,
        preamp_left=state.preamp_left,
        preamp_right=state.preamp_right,
        applied=state.applied,
        bypassed=state.bypassed,
        skipped=state.skipped,
    )


def _read_include_file(path):
    """EqualizerAPO처럼 UTF-8 우선, 실패 시 시스템 코드페이지로 읽는다."""
    with open(path, "rb") as f:
        raw = f.read()
    try:
        return raw.decode("utf-8-sig")
    except UnicodeDecodeError:
        return raw.decode("cp1252", errors="replace")


def _parse_lines(lines, state, base_dir, depth, visited):
    if_depth = 0
    for line_number, line in enumerate(lines, start=1):
        # FilterEngine.Configuration.cpp: 첫 ':'에서 키/값을 분리하고 키를 트리밍
        pos = line.find(":")
        if pos < 0:
            continue
        key = line[:pos].strip()
        value = line[pos + 1:]
        if not key or key.startswith("#"):
            continue

        # If/EndIf 블록 내부는 조건을 평가할 수 없으므로 통째로 바이패스한다.
        if if_depth > 0:
            if key == "If":
                if_depth += 1
            elif key == "EndIf":
                if_depth -= 1
                continue
            if _KNOWN_COMMAND_RE.match(key):
                state.bypassed.append(
                    EqApoCommandReport(line_number, key, line.strip(), REASON_CONDITIONAL)
                )
            continue

        if key == "If":
            if_depth = 1
            state.bypassed.append(
                EqApoCommandReport(line_number, key, line.strip(), REASON_CONDITIONAL)
            )
            continue

        if key.startswith("Filter"):
            _handle_filter(key, value, line, line_number, state)
        elif key == "Preamp":
            _handle_preamp(key, value, line, line_number, state)
        elif key == "GraphicEQ":
            _handle_graphic_eq(key, value, line, line_number, state)
        elif key == "Channel":
            state.scope = _parse_channel_scope(value)
        elif key == "Include":
            _handle_include(key, value, line, line_number, state, base_dir, depth, visited)
        elif key in _SCOPING_COMMANDS:
            state.bypassed.append(
                EqApoCommandReport(line_number, key, line.strip(), REASON_SCOPING_IGNORED)
            )
        elif key in _UNSUPPORTED_COMMANDS or key in _CONDITIONAL_COMMANDS:
            state.bypassed.append(
                EqApoCommandReport(line_number, key, line.strip(), REASON_UNSUPPORTED)
            )
        elif _KNOWN_COMMAND_RE.match(key):
            state.bypassed.append(
                EqApoCommandReport(line_number, key, line.strip(), REASON_UNSUPPORTED)
            )
        # 그 외(주석성 텍스트 등)는 EqualizerAPO와 동일하게 조용히 무시한다.


def _scope_or_report(key, line, line_number, state):
    """현재 Channel 스코프에 L/R이 하나도 없으면 보고하고 False를 반환한다."""
    if not state.scope:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_CHANNEL_SCOPE)
        )
        return False
    return True


def _handle_filter(key, value, line, line_number, state):
    if "`" in value:
        # 인라인 표현식(백틱)은 평가할 수 없다.
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_EXPRESSION)
        )
        return

    iir = _parse_iir(value)
    if iir is not None:
        if not _scope_or_report(key, line, line_number, state):
            return
        b, a = iir
        gain_db = _evaluate_iir(b, a, state.frequency, state.srate)
        state.add_response(gain_db, f"IIR Order {len(b) - 1}")
        return

    spec, failure = _parse_biquad(value)
    if spec is None:
        if failure == REASON_DISABLED:
            state.skipped.append(
                EqApoCommandReport(line_number, key, line.strip(), REASON_DISABLED)
            )
        else:
            state.bypassed.append(
                EqApoCommandReport(line_number, key, line.strip(), failure)
            )
        return

    if not _scope_or_report(key, line, line_number, state):
        return
    gain_db = _evaluate_biquad(spec, state.frequency, state.srate)
    state.add_response(gain_db, spec.description)


def _handle_preamp(key, value, line, line_number, state):
    if "`" in value:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_EXPRESSION)
        )
        return
    normalized = value.replace(",", ".")
    m = _LEADING_FLOAT_RE.match(normalized)
    if m is None:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_MALFORMED)
        )
        return
    if not _scope_or_report(key, line, line_number, state):
        return
    preamp_db = float(m.group())
    if "L" in state.scope:
        state.preamp_left += preamp_db
        state.left_db += preamp_db
    if "R" in state.scope:
        state.preamp_right += preamp_db
        state.right_db += preamp_db


def _handle_graphic_eq(key, value, line, line_number, state):
    if "`" in value:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_EXPRESSION)
        )
        return
    nodes = _parse_graphic_eq_nodes(value)
    if not nodes:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_MALFORMED)
        )
        return
    if not _scope_or_report(key, line, line_number, state):
        return
    gain_db = _evaluate_graphic_eq(nodes, state.frequency)
    state.add_response(gain_db, f"GraphicEQ ({len(nodes)} nodes)")


def _handle_include(key, value, line, line_number, state, base_dir, depth, visited):
    include_path = value.strip().strip('"')
    resolved = None
    if include_path:
        if os.path.isabs(include_path):
            candidate = include_path
        elif base_dir is not None:
            candidate = os.path.join(base_dir, include_path)
        else:
            candidate = None
        if candidate is not None and os.path.isfile(candidate):
            resolved = os.path.normcase(os.path.normpath(os.path.abspath(candidate)))

    if resolved is None or depth >= _MAX_INCLUDE_DEPTH or resolved in visited:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_INCLUDE_NOT_FOUND)
        )
        return

    try:
        text = _read_include_file(resolved)
    except OSError:
        state.bypassed.append(
            EqApoCommandReport(line_number, key, line.strip(), REASON_INCLUDE_NOT_FOUND)
        )
        return
    # FilterEngine::loadConfigFile은 포함 파일 종료 시 채널 선택을 복원한다.
    # visited는 현재 include 체인에만 적용하여(순환 방지) 같은 파일을 서로 다른
    # 위치에서 두 번 포함하는 것은 EqualizerAPO처럼 허용한다.
    saved_scope = set(state.scope)
    _parse_lines(
        text.splitlines(), state, os.path.dirname(resolved), depth + 1, visited | {resolved}
    )
    state.scope = saved_scope
