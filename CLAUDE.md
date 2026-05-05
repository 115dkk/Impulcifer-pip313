# CLAUDE.md

Impulcifer-py313 프로젝트에서 Claude Code가 작업할 때 참조하는 프로젝트 지침서.

## 프로젝트 개요

Impulcifer-py313은 HRIR(Head-Related Impulse Response)을 측정하고 헤드폰용 바이노럴 BRIR을 생성하는 오디오 DSP 도구다. 원본 Jaakko Pasanen의 Impulcifer를 Python 3.13/3.14 환경에 맞게 포크한 버전이며, CustomTkinter 기반의 Modern GUI, 다국어 지원(9개 언어), free-threaded 병렬 처리 최적화 등이 추가되어 있다.

핵심 기능은 sweep 신호로 녹음한 임펄스 응답 파일들을 처리하여 HeSuVi 등에서 사용 가능한 BRIR WAV 파일을 생성하는 것이다. 이 과정에서 룸 보정, 헤드폰 보상, EQ, virtual bass 등의 DSP 처리가 적용된다.

## 아키텍처

```
gui_main.py              ← 엔트리포인트 (standalone 빌드 대상)
impulcifer.py             ← 핵심 처리 파이프라인 (main 함수)
core/
  hrir.py                 ← HRIR 클래스 (가장 큰 모듈, ~2000줄)
  impulse_response.py     ← 임펄스 응답 단위 처리
  impulse_response_estimator.py  ← sweep → IR 변환
  room_correction.py      ← 룸 보정
  virtual_bass.py         ← 가상 저음 확장
  microphone_deviation_correction.py  ← 마이크 편차 보정
  recorder.py             ← 녹음/재생
  utils.py                ← WAV I/O, 유틸리티 (magnitude_response 등)
  constants.py            ← 스피커 이름/딜레이 등 상수
  parallel_workers.py     ← ProcessPoolExecutor 워커 (경량 모듈)
  parallel_processing.py  ← free-threaded 병렬 처리
  parallel_utils.py       ← 병렬 처리 유틸리티
  channel_generation.py   ← 가상 채널 생성
gui/
  modern_gui.py           ← CustomTkinter GUI (~2276줄)
  legacy_gui.py           ← 구버전 Tkinter GUI
autoeq/                   ← 벤더링된 AutoEQ (PR #63에서 in-tree 전환)
  frequency_response.py   ← 주파수 응답 처리 핵심
i18n/
  localization.py         ← 다국어 관리
  locales/*.json          ← 번역 파일 (en, ko, ja, de, es, fr, ru, zh_CN, zh_TW)
infra/
  logger.py               ← 통합 로거 (GUI 콜백 지원)
  resource_helper.py      ← 리소스 경로 헬퍼
  _build_info.py          ← 빌드 시 생성되는 버전/타입 마커
updater/
  update_checker.py       ← GitHub 릴리스 기반 업데이트 확인
  updater_core.py         ← Velopack/pip/레거시 업데이터
```

## 수정 시 주의사항

`impulcifer.py`의 `main()` 함수 시그니처(208-248행)를 변경하지 말 것. GUI의 `generate_brir()`가 이 시그니처에 1:1 대응하는 인자 딕셔너리를 조립한다.

`core/recorder.py`의 `play_and_record()`는 `sd.play(blocking=True)` + `Thread.join()`으로 완전한 블로킹 함수다. 이 동작을 변경하지 말 것.

`core/utils.py`의 `magnitude_response()`는 현재 검증된 NumPy `rfft` 기반 출력과 bit-identical해야 한다. full FFT 경로는 수치적으로 가까워도 BRIR md5를 바꿀 수 있으므로, `test_magnitude_response_parity.py`가 이 verified 동작을 고정한다.

데모 WAV 파일(`data/demo/*.wav`)은 raw 바이너리로 repo에 포함되어 있다(약 55MB). 일반 `git clone`으로 받아진다. `.gitignore`가 demo 폴더를 기본 무시하면서 화이트리스트로 필요한 파일들만 통과시키므로, 새 데모 파일을 추가할 때는 `.gitignore`의 `!data/demo/...` 라인을 갱신해야 한다.

Nuitka 빌드 설정은 5개소에 중복 존재한다: `build_scripts/build_nuitka.py`, `build-linux.yml`, `build-macos.yml`, `release-cross-platform.yml`(2개소). 빌드 플래그 변경 시 5곳 모두 반영해야 한다.

`requirements.txt`와 `pyproject.toml`의 `[project] dependencies`는 동기화 상태를 유지해야 한다. 정본은 `pyproject.toml`이다.

ruff 설정은 `pyproject.toml`의 `[tool.ruff]` 섹션에 있다. `impulcifer.py`는 E402(import-not-at-top)가 의도적으로 면제되어 있다(`__version__` 계산이 import 전에 수행되는 구조).

## 작업 완료 규칙

코드 변경을 커밋하기 전에 아래 네 가지를 반드시 수행한다. 하나라도 누락하면 PR이 불완전한 상태로 올라가게 되므로, 체크리스트로 활용할 것.

### 1. GUI 문자열은 반드시 로컬라이제이션할 것

`gui/modern_gui.py`에 사용자에게 보이는 문자열(버튼 텍스트, 라벨, 메시지, 다이얼로그 등)을 추가하거나 변경할 때, 하드코딩 문자열을 직접 넣지 말고 반드시 i18n 키를 생성하여 `self.loc.get('키_이름')`으로 참조한다.

절차는 다음과 같다.

1. `i18n/locales/en.json`에 영어 키를 추가한다.
2. `i18n/locales/ko.json`에 한국어 번역을 추가한다.
3. 나머지 7개 파일(`de.json`, `es.json`, `fr.json`, `ja.json`, `ru.json`, `zh_CN.json`, `zh_TW.json`)에 최소한 영어 텍스트를 fallback으로 추가한다.
4. 코드에서 `self.loc.get('키_이름')`으로 참조한다.

키 네이밍은 기존 264개 키의 접두사 컨벤션을 따른다.

| 접두사 | 용도 | 예시 |
|--------|------|------|
| `label_` | GUI 라벨 | `label_host_api` |
| `button_` | 버튼 텍스트 | `button_browse` |
| `message_` | 다이얼로그/알림 메시지 | `message_recording_complete` |
| `section_` | 섹션 제목 | `section_audio_devices` |
| `checkbox_` | 체크박스 텍스트 | `checkbox_do_room_correction` |
| `option_` | 드롭다운 옵션 | `option_average` |
| `tab_` | 탭 이름 | `tab_recorder` |
| `error_` | 에러 메시지 | `error_file_not_found` |
| `tooltip_` | 툴팁 | `tooltip_bass_boost` |
| `dialog_` | 다이얼로그 제목 | `dialog_confirm_title` |
| `cli_` | CLI/로거 메시지 | `cli_creating_estimator` |

en.json과 ko.json의 키는 현재 264개로 완전히 일치한다. 이 동기화 상태를 유지해야 한다.

### 2. 런타임 변경 시 버전 bump를 포함할 것

`core/`, `autoeq/`, `impulcifer.py`, 의존성 등 임펄사이퍼 실행에 영향을 주는 변경이 포함된 PR에서는 `pyproject.toml`의 `version` 필드를 반드시 갱신한다. PyPI는 동일 버전의 재업로드를 허용하지 않으므로, 버전 bump 없이는 배포가 불가능하다.

SemVer 규칙에 따라 갱신한다.

- PATCH 증가(예: 2.4.11 → 2.4.12): 버그 수정, 성능 개선, 내부 리팩토링
- MINOR 증가(예: 2.4.11 → 2.5.0): 새 기능 추가, 하위 호환 유지
- MAJOR 증가(예: 2.4.11 → 3.0.0): 하위 호환이 깨지는 변경

빌드 설정만 변경한 경우, 문서만 수정한 경우에는 버전 bump가 불필요하다.

### 3. CHANGELOG에 변경사항을 기록할 것

작업이 완료되면 `CHANGELOG.md` 상단(헤더 설명문 바로 아래)에 새 항목을 추가한다. 기존 포맷을 따른다.

```
## X.Y.Z - YYYY-MM-DD
### 이모지 요약 제목

#### 이모지 카테고리
- **변경 내용 제목**: 상세 설명
```

카테고리별 이모지는 다음과 같다.

- ⚡ 성능 개선
- 🐛 버그 수정
- ⭐ 새로운 기능 / 개선
- 🔧 빌드 / 설정 변경

한국어로 작성하며, 하나의 PR에 여러 카테고리가 포함되면 각각 `####` 소제목으로 분리한다. 버전 bump를 했다면 해당 버전 번호를 사용하고, 버전 bump가 없는 변경(문서, 빌드 설정 등)이면 이전 버전 번호 아래에 날짜만 다르게 추가한다.

### 4. README.md 갱신 필요성을 확인하고 반영할 것

PR을 올리기 전에, 변경사항이 `README.md`의 내용과 관련이 있는지 조회하여 확인한다. 아래 항목에 해당하면 README를 실제로 갱신해야 한다.

- 새 기능이 추가되었는데 README에 설명이 없는 경우
- CLI 인자나 옵션이 추가/변경/제거되었는데 사용법 섹션이 맞지 않는 경우
- 의존성이 변경되었는데 설치 가이드의 요구사항이 맞지 않는 경우
- 지원 Python 버전 범위가 변경된 경우
- 설치 방법이나 실행 방법이 변경된 경우

README에 해당 사항이 없으면 갱신하지 않는다. 불필요한 변경은 diff를 오염시킬 뿐이다.

## PR 전 검증 절차

아래 3개 Tier를 순서대로 수행한다. Tier 1은 모든 커밋 전에, Tier 2는 PR 생성 전에, Tier 3는 런타임 코드 변경이 포함된 PR에서 수행한다. GitHub CI(`test.yml`)가 Python 3.9~3.14에서 동일한 검증을 수행하지만, Claude Code 단계에서 먼저 잡는 것이 안전하다.

### Tier 1: 빠른 검증 (매 커밋 전)

구문 검사, 린트, 빠른 유닛 테스트를 순서대로 실행한다.

```bash
# 1-1. 구문 검사 (CI의 lint 잡에 대응)
python -m py_compile impulcifer.py
python -m py_compile core/*.py gui/*.py i18n/*.py infra/*.py updater/*.py

# 1-2. ruff 린트
ruff check . --output-format=github

# 1-3. 빠른 유닛 테스트 (slow 마커 제외)
pytest tests/test_suite.py -v -m "not slow"
```

`test_suite.py`는 마이크 편차 보정기 초기화/ILD 부호/게이트 길이/편차 수집/보정, IR 생성/피크 검출, 핵심 모듈 임포트 가능 여부, 필수 데이터 파일 존재, `pyproject.toml` 유효성, 버전 형식(semantic versioning)을 검증한다.

ruff는 CI에서 `continue-on-error: true`로 되어 있어 린트 실패가 PR을 블로킹하지 않지만, 가능한 한 경고를 해소하는 것이 좋다.

### Tier 2: 전체 테스트 (PR 생성 전)

```bash
# 2-1. 전체 테스트 스위트
pytest tests/ -v
```

Tier 1의 `test_suite.py` 외에 아래 테스트가 추가로 실행된다.

`test_magnitude_response_parity.py`는 `core/utils.py`의 `magnitude_response()` 함수가 현재 검증된 NumPy `rfft` 경로와 bit-identical한 출력을 내는지 검증한다. even/odd 길이, 임펄스, sweep-like 신호에 대해 모두 확인한다. `core/utils.py`를 수정했다면 이 테스트가 가장 중요하다.

`test_virtual_bass.py`는 `_classify_speaker()`의 좌/우/중앙 분류, `_detect_polarity()`의 극성 감지, `_build_ild_shelf()`의 ILD 셸프 필터 생성, `apply_virtual_bass_to_hrir()`의 전체 플로우를 검증한다.

`test_parallel_processing.py`는 `parallel_map`, `parallel_process_dict`, `get_optimal_worker_count`, `is_free_threaded_available` 등 병렬 처리 함수의 정상 동작과 속도 회귀를 검증한다.

`test_integration.py`는 마이크 편차 보정의 전체 파이프라인(HRIR 생성 → 편차 수집 → 보정 적용 → 결과 검증)을 통합 테스트한다.

```bash
# 2-2. 모듈 임포트 검증 (CI의 test-imports 잡에 대응)
python -c "import impulcifer; print('impulcifer OK')"
python -c "from core.hrir import HRIR; print('core.hrir OK')"
python -c "from core.impulse_response import ImpulseResponse; print('core.impulse_response OK')"
python -c "from core.microphone_deviation_correction import MicrophoneDeviationCorrector; print('core.mic_dev OK')"
python -c "from i18n.localization import get_localization_manager; print('i18n OK')"
python -c "from infra.logger import get_logger; print('infra.logger OK')"
python -c "from updater.update_checker import UpdateChecker; print('updater OK')"
```

PortAudio가 설치된 환경이라면 `core.recorder`와 `gui.modern_gui` 임포트도 확인한다.

```bash
# 2-3. 의존성 동기화 확인
python -c "
import tomllib
with open('pyproject.toml', 'rb') as f:
    deps_toml = set(d.split('>=')[0].split('>')[0].strip().lower()
                    for d in tomllib.load(f)['project']['dependencies'])
with open('requirements.txt') as f:
    deps_req = set(l.split('>=')[0].split('>')[0].strip().lower()
                   for l in f if l.strip() and not l.startswith('#') and not l.startswith('-'))
if deps_toml != deps_req:
    print(f'pyproject.toml에만 있음: {deps_toml - deps_req}')
    print(f'requirements.txt에만 있음: {deps_req - deps_toml}')
else:
    print('OK: 의존성 목록 일치')
"
```

### Tier 3: 알고리즘 무결성 검증 (런타임 코드 변경 시)

코드 변경이 BRIR 출력에 영향을 주지 않는지 검증하는 절차다. `core/`, `autoeq/`, `impulcifer.py`를 수정한 PR에서는 반드시 수행한다. 빌드 설정 변경이나 리팩토링처럼 출력에 영향이 없어야 하는 PR에서도 수행을 권장한다.

#### Step 1: 데모 BRIR 생성

데모 데이터로 hesuvi.wav를 생성한다.

```bash
python impulcifer.py \
    --dir_path=data/demo \
    --test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl \
    --vbass --vbass_freq=250
```

기본값 경로도 함께 검증한다. 이 경로에는 기본 헤드폰 보정(`headphones.wav`)이 포함된다.

```bash
python impulcifer.py \
    --dir_path=data/demo \
    --test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl
```

출력 파일: `data/demo/hesuvi.wav`

#### Step 2: md5 해시 비교

```bash
md5sum data/demo/hesuvi.wav
```

현재 검증된 baseline md5(가상 베이스 경로):

```
d295982d021a6d16ab2c194c3517c162  data/demo/hesuvi.wav
```

CI의 `brir-integrity` job은 hardcoded md5 하나만 보지 않고, 같은 Ubuntu CPython 3.13 환경에서 무결성이 확인된 기준 ref(`origin/master`)와 현재 브랜치의 `hesuvi.wav`를 모두 생성해 비교한다. 비교 대상은 기본값(헤드폰 보정 포함)과 `--vbass --vbass_freq=250` 두 경로다.

이 해시는 PR #63(AutoEQ 벤더링) 이후 확립된 것이다. 일치하면 무결성 확인 완료. 불일치하면 Step 3으로 진행한다.

의도적으로 알고리즘을 변경하여 출력이 달라져야 하는 PR에서는, 변경 전후의 차이를 Step 3의 주파수 응답 분석으로 문서화한 뒤, 이 문서의 baseline 해시를 새 값으로 갱신한다.

#### Step 3: 주파수 응답 분석 (md5 불일치 시)

불일치가 발생하면, 채널별 주파수 응답을 1Hz 단위로 추출하여 비교한다.

**참조 파일 확보.** baseline 해시에 대응하는 hesuvi.wav를 무결성이 확인된 커밋(현재 master HEAD)에서 생성한다.

```bash
cp data/demo/hesuvi.wav /tmp/hesuvi_test.wav

git stash
git checkout master
python impulcifer.py \
    --dir_path=data/demo \
    --test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl \
    --vbass --vbass_freq=250
cp data/demo/hesuvi.wav /tmp/hesuvi_ref.wav

git checkout -
git stash pop
```

**주파수 응답 추출 및 비교.** 아래 스크립트로 두 파일의 차이를 분석한다.

```python
#!/usr/bin/env python3
"""hesuvi.wav 주파수 응답 비교 스크립트.

사용법:
    python compare_fr.py /tmp/hesuvi_ref.wav /tmp/hesuvi_test.wav
"""
import sys
import numpy as np
from scipy.io import wavfile
from scipy.fft import rfft, rfftfreq


def extract_fr(wav_path, freq_min=20, freq_max=20000):
    """WAV 파일에서 채널별 주파수 응답을 1Hz 해상도로 추출한다."""
    fs, data = wavfile.read(wav_path)
    if data.dtype == np.int16:
        data = data / 32768.0
    elif data.dtype == np.int32:
        data = data / 2147483648.0
    elif data.dtype != np.float32 and data.dtype != np.float64:
        data = data.astype(np.float64)

    n_channels = data.shape[1] if data.ndim > 1 else 1
    n_samples = data.shape[0]

    # 1Hz 해상도를 위해 최소 fs 샘플로 zero-pad
    n_fft = max(n_samples, fs)
    freqs = rfftfreq(n_fft, 1.0 / fs)
    mask = (freqs >= freq_min) & (freqs <= freq_max)
    target_freqs = freqs[mask]

    results = {}
    for ch in range(n_channels):
        ch_data = data[:, ch] if n_channels > 1 else data
        spectrum = rfft(ch_data, n=n_fft)
        magnitude_db = 20 * np.log10(np.abs(spectrum[mask]) + 1e-30)
        results[ch] = magnitude_db

    return target_freqs, results, fs, n_channels


def compare(ref_path, test_path, threshold_db=0.01):
    """두 hesuvi.wav의 주파수 응답을 비교하고 차이를 보고한다."""
    freqs_r, fr_r, fs_r, nch_r = extract_fr(ref_path)
    freqs_t, fr_t, fs_t, nch_t = extract_fr(test_path)

    if fs_r != fs_t:
        print(f"ERROR: 샘플레이트 불일치 (ref={fs_r}, test={fs_t})")
        return
    if nch_r != nch_t:
        print(f"ERROR: 채널 수 불일치 (ref={nch_r}, test={nch_t})")
        return

    # hesuvi.wav 채널 순서: FL_L, FL_R, FR_L, FR_R, FC_L, FC_R, ...
    speakers = ["FL", "FR", "FC", "BL", "BR", "SL", "SR"]
    sides = ["left", "right"]

    print(f"샘플레이트: {fs_r} Hz, 채널 수: {nch_r}")
    print(f"분석 범위: {int(freqs_r[0])}-{int(freqs_r[-1])} Hz (1Hz 해상도)")
    print(f"임계값: {threshold_db} dB\n")

    has_diff = False
    for ch in range(nch_r):
        diff = fr_t[ch] - fr_r[ch]
        max_diff = np.max(np.abs(diff))

        if max_diff > threshold_db:
            has_diff = True
            spk_idx = ch // 2
            side_idx = ch % 2
            spk = speakers[spk_idx] if spk_idx < len(speakers) else f"CH{spk_idx}"
            side = sides[side_idx]

            problem_mask = np.abs(diff) > threshold_db
            problem_freqs = freqs_r[problem_mask]

            if len(problem_freqs) > 0:
                freq_lo = int(problem_freqs[0])
                freq_hi = int(problem_freqs[-1])
                peak_idx = np.argmax(np.abs(diff))
                peak_freq = int(freqs_r[peak_idx])
                peak_diff = diff[peak_idx]

                print(f"[DIFF] {spk} {side}: 최대 {max_diff:.4f} dB 차이")
                print(f"       영향 범위: {freq_lo}-{freq_hi} Hz")
                print(f"       최대 편차 위치: {peak_freq} Hz ({peak_diff:+.4f} dB)")

                bands = [
                    ("저역 (20-250Hz)", 20, 250),
                    ("중저역 (250-1kHz)", 250, 1000),
                    ("중역 (1-4kHz)", 1000, 4000),
                    ("중고역 (4-8kHz)", 4000, 8000),
                    ("고역 (8-20kHz)", 8000, 20000),
                ]
                for name, lo, hi in bands:
                    band_mask = (freqs_r >= lo) & (freqs_r <= hi)
                    band_diff = diff[band_mask]
                    if len(band_diff) > 0 and np.max(np.abs(band_diff)) > threshold_db:
                        avg = np.mean(band_diff)
                        mx = np.max(np.abs(band_diff))
                        print(f"       {name}: 평균 {avg:+.4f} dB, 최대 {mx:.4f} dB")
                print()

    if not has_diff:
        print("모든 채널에서 차이가 임계값 이내입니다.")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("사용법: python compare_fr.py <reference.wav> <test.wav>")
        sys.exit(1)
    compare(sys.argv[1], sys.argv[2])
```

#### Step 4: 진단 가이드

주파수 응답 차이의 패턴에 따라 원인을 특정할 수 있다.

**전 대역 균일한 편차** (전체 주파수에서 동일한 dB 차이): 정규화 게인 변경. `core/hrir.py`의 `normalize()` 또는 `impulcifer.py`의 `target_level` 처리를 확인한다.

**20-250Hz 저역만 영향:** virtual bass 처리 변경. `core/virtual_bass.py`의 `apply_virtual_bass_to_hrir()` 파라미터(crossover 주파수, HP 필터, polarity 감지)를 확인한다.

**250-1000Hz 중저역:** 룸 보정이나 bass boost 처리. `core/room_correction.py`와 `impulcifer.py`의 `bass_boost_gain`, `bass_boost_fc`, `bass_boost_q` 인자를 확인한다.

**1k-8kHz 중역~중고역:** 헤드폰 보상 EQ 또는 `fr_combination_method` 변경. `autoeq/frequency_response.py`의 필터 계산이나 `specific_limit`/`generic_limit` 파라미터를 확인한다.

**8k-20kHz 고역:** 고역 EQ 또는 리샘플링 처리. `nnresample` 관련 코드나 `core/impulse_response.py`의 윈도우 함수를 확인한다.

**빗살(comb filter) 패턴** (주기적인 피크/딥): ITD(Interaural Time Delay) 정렬 변경. `core/hrir.py`의 ITD 계산/보정 로직을 확인한다.

**특정 채널만 영향:** 해당 스피커의 IR 처리 경로를 집중 확인한다. `core/constants.py`의 `SPEAKER_NAMES`, `SPEAKER_DELAYS` 매핑이 변경되었는지 살펴본다.

#### Step 5: 수정 및 재검증

원인을 특정했으면 해당 코드를 수정한 뒤, Step 1부터 다시 수행한다. md5 해시가 baseline과 일치할 때까지 반복한다. compare_fr.py의 출력을 PR 코멘트나 커밋 메시지에 첨부하면 추적에 도움이 된다.

### 변경 유형별 필수 검증 범위

| 수정 대상 | Tier 1 | Tier 2 테스트 | Tier 3 |
|-----------|--------|--------------|--------|
| `core/utils.py` | 필수 | `test_magnitude_response_parity.py` 필수 | 필수 |
| `core/hrir.py`, `impulcifer.py` | 필수 | 전체 | 필수 |
| `core/virtual_bass.py` | 필수 | `test_virtual_bass.py` 필수 | 필수 |
| `core/parallel_processing.py` | 필수 | `test_parallel_processing.py` 필수 | 불필요 |
| `autoeq/` | 필수 | 불필요 | 필수 |
| `gui/` | 필수 | 모듈 임포트 검증 | 불필요 |
| `i18n/` | 필수 | 모듈 임포트 검증 | 불필요 |
| 빌드 설정만 | 필수 | 불필요 | 권장 |
| 의존성 변경 | 필수 | 전체 + 동기화 확인 | 권장 |

## 빌드 참고

Nuitka standalone 빌드의 엔트리포인트는 `gui_main.py`다. `pyproject.toml`의 `[project.scripts]`에 정의된 콘솔 스크립트(`impulcifer`, `impulcifer_gui`, `impulcifer_gui_legacy`)는 pip 설치 전용이며 standalone 빌드와 무관하다.
