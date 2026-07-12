# CLAUDE.md

Impulcifer-py313 프로젝트에서 Claude Code가 작업할 때 참조하는 프로젝트 지침서.

## 프로젝트 개요

Impulcifer-py313은 HRIR(Head-Related Impulse Response)을 측정하고 헤드폰용 바이노럴 BRIR을 생성하는 오디오 DSP 도구다. 원본 Jaakko Pasanen의 Impulcifer를 Python 3.13/3.14 환경에 맞게 포크한 버전이며, CustomTkinter 기반의 Modern GUI, 다국어 지원(9개 언어), free-threaded 병렬 처리 최적화 등이 추가되어 있다.

핵심 기능은 sweep 신호로 녹음한 임펄스 응답 파일들을 처리하여 HeSuVi 등에서 사용 가능한 BRIR WAV 파일을 생성하는 것이다. 이 과정에서 룸 보정, 헤드폰 보상, EQ, virtual bass 등의 DSP 처리가 적용된다.

## 아키텍처

```
gui_main.py              ← 런처 엔트리포인트 (standalone 빌드 대상). 기본 프론트엔드는
                            WebView(2.10+). --frontend=webview|ctk 인자 >
                            settings.json의 frontend 키 > 기본 webview 순으로 결정,
                            WebView 스택 불가 시 CTk 자동 폴백
impulcifer_webview.py     ← WebView 프론트엔드 (Windows=edgechromium / macOS=cocoa /
                            Linux=gtk 강제 맵; webview-backend-validation.yml이
                            3-플랫폼 실기 검증; pip에서는 선택적 [webview] extra,
                            Linux는 pywebview[gtk]+시스템 WebKit2GTK 필요)
application/
  impulcifer_service.py   ← Tk 비종속 JSON-safe application service (job 모델,
                            ProcessingConfig 전체 표면 검증, UI 설정/시스템 정보)
webview_ui/               ← WebView HTML/CSS/JS (Pulse Studio 디자인, i18n 문자열은
                            bootstrap 응답으로 주입; webview-gallery CI가 렌더 검증)
impulcifer.py             ← 핵심 처리 파이프라인 (main 함수)
core/
  hrir.py                 ← HRIR 클래스 (Phase 1: ~1150줄, plotting은 plotting/으로 이동)
  impulse_response.py     ← 임펄스 응답 단위 처리 (Phase 1: ~534줄)
  impulse_response_estimator.py  ← sweep → IR 변환
  room_correction.py      ← 룸 보정
  virtual_bass.py         ← 가상 저음 확장
  microphone_deviation_correction.py  ← 마이크 편차 보정
  recorder.py             ← 녹음/재생
  utils.py                ← audio_io/font_setup/plotting_utils/ffmpeg_utils 재export 셸 (audit #115-8)
  audio_io.py             ← WAV I/O + DSP 프리미티브 (magnitude_response 등, audit #115-8 분리)
  font_setup.py           ← matplotlib 한글 폰트 설정 (audit #115-8 분리)
  plotting_utils.py       ← 플롯/PNG 헬퍼 (audit #115-8 분리)
  ffmpeg_discovery.py     ← FFmpeg 검색/설치 + lazy 초기화 globals (audit #115-9 분리)
  audio_truehd.py         ← TrueHD/MLP 디코드 + read_audio (audit #115-9 분리)
  ffmpeg_utils.py         ← 위 둘의 하위 호환 re-export 셸
  constants.py            ← 스피커 이름/딜레이 등 상수
  pipeline.py             ← ProcessingConfig + BRIRPipeline (Phase 2)
  cli_builder.py          ← argparse 자동 생성기 (Phase 3)
  plotting/               ← HRIRPlotter / ImpulseResponsePlotter mixin (Phase 1)
  parallel_workers.py     ← ProcessPoolExecutor 워커 (경량 모듈)
  parallel_processing.py  ← free-threaded 병렬 처리
  parallel_utils.py       ← 병렬 처리 유틸리티
  channel_generation.py   ← 가상 채널 생성
gui/
  modern_gui.py           ← CustomTkinter GUI (~295줄, tabs/로 분리됨. 버전 2 동안
                            유지보수+기능추가 지속, 버전 3부터는 레거시 GUI처럼
                            동결 유지[패치 없음] — 제거 아님)
  tabs/                   ← Recorder / Impulcifer / Settings / Info 탭
  legacy_gui.py           ← 구버전 Tkinter GUI (deprecated, 신규 작업 금지,
                            버전 3에서 제거 예정)
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
  updater_core.py         ← 하위 호환 re-export 셸(이슈 #87 Phase 5에서 분리 완료,
                            ~58줄). 기존 import 경로를 유지하기 위해 아래 모듈들을
                            다시 export한다. 신규 코드는 아래 실제 모듈을 직접 import.
  environment.py          ← 설치 환경/플랫폼 감지
  velopack.py             ← Velopack 업데이터
  pip_updater.py          ← pip 기반 업데이터
  legacy.py               ← 레거시 인스톨러 업데이터
  executors.py            ← UpdateExecutor 계열(업데이트 실행)
```

## 수정 시 주의사항

`impulcifer.py`의 `main()`은 `**kwargs`를 받아 `core.pipeline.ProcessingConfig.from_kwargs()`로 전달하는 얇은 래퍼다(이슈 #113/#115 audit에서 기존의 32개 명시적 인자 시그니처를 통합). GUI의 `generate_brir()`/`gui.brir_args.build_brir_args`와 CLI(`create_cli`)가 인자 딕셔너리를 조립해 넘기며, `ProcessingConfig`에 없는 키(예: 폐기된 호환 플래그)는 `from_kwargs`가 무시한다. 따라서 파라미터의 정본 기본값은 `ProcessingConfig` 필드에만 존재하므로, 새 파라미터는 `ProcessingConfig`에 필드를 추가하면 CLI·GUI·파이프라인에 자동 반영된다. 실제 BRIR 파이프라인 단계 시퀀스는 `core.pipeline.BRIRPipeline.run()`이 보유한다(DSP 단계 헬퍼는 여전히 `impulcifer.py`에 있고 `run()` 내부에서 지연 import한다).

`core/recorder.py`의 `play_and_record()`는 `sd.play(blocking=True)` + `Thread.join()`으로 완전한 블로킹 함수다. 이 동작을 변경하지 말 것.

`core/utils.py`의 `magnitude_response()`는 현재 검증된 NumPy `rfft` 기반 출력과 bit-identical해야 한다. full FFT 경로는 수치적으로 가까워도 BRIR 해시를 바꿀 수 있으므로, `test_magnitude_response_parity.py`가 이 verified 동작을 고정한다.

데모 WAV 파일(`data/demo/*.wav`)은 raw 바이너리로 repo에 포함되어 있다(약 55MB). 일반 `git clone`으로 받아진다. `.gitignore`가 demo 폴더를 기본 무시하면서 화이트리스트로 필요한 파일들만 통과시키므로, 새 데모 파일을 추가할 때는 `.gitignore`의 `!data/demo/...` 라인을 갱신해야 한다.

pywebview는 Nuitka 플래그에 명시적 include로 넣지 말 것 — `--include-module=webview`든 `--include-package=webview`든 Nuitka의 follow 패턴에 들어가 내장 pywebview 플러그인(타 플랫폼 백엔드 제외 결정)과 충돌하고 빌드가 FATAL로 죽는다("Conflict between user and plugin decision for module 'webview.platforms.android'", 로컬 빌드로 확인). 정적 `import webview` 체인(gui_main → impulcifer_webview)의 자동 추적과 플러그인/패키지 설정이 백엔드 모듈 선택·webview js/lib·pythonnet/clr_loader DLL 번들을 전부 처리한다. 단, 그 플러그인의 Windows 화이트리스트에는 pywebview 6.x가 요구하는 `webview.platforms.win32`가 누락되어 있어(업스트림 버그) 빌드 스크립트가 `build_scripts/patch_nuitka_pywebview.py`로 빌드 직전에 화이트리스트를 패치한다 — 이 패치를 제거하면 패키징된 Windows 앱이 기동 시 조용히 CTk로 폴백한다. `tests/test_nuitka_flags.py`가 이 불변식들을 고정한다.

Nuitka 빌드 플래그의 정본은 `build_scripts/nuitka_flags.py`다. 모든 빌드 진입점 — 릴리스 파이프라인(`.github/workflows/publish.yml`의 `build-windows`/`build-macos`/`build-linux` 잡)과 수동 빌드 워크플로(`build-linux.yml`, `build-macos.yml`) — 은 인라인 Nuitka 명령 없이 `python build_scripts/build_nuitka.py`를 호출하고, 이 스크립트가 `nuitka_flags.py`를 import하므로 플래그는 한 곳에서 동기화된다(이슈 #87 Phase 4에서 인라인 명령 제거 완료). 빌드 플래그를 추가/변경할 때는 `nuitka_flags.py`만 갱신하면 된다. `python build_scripts/nuitka_flags.py --platform linux --version X` 으로 정본 플래그 목록을 한 줄씩 출력해 비교에 활용할 수 있다.

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

`core/`, `autoeq/`, `impulcifer.py`, `gui/`, `i18n/`, `infra/`, `updater/`, 번들 자산, 의존성 등 **출하물(PyPI wheel / standalone 앱)에 영향을 주는 변경**이 포함된 PR에서는 `pyproject.toml`의 `version` 필드를 갱신한다. PyPI는 동일 버전의 재업로드를 허용하지 않는다.

SemVer 규칙에 따라 갱신한다.

- PATCH 증가(예: 2.4.11 → 2.4.12): 버그 수정, 성능 개선, 내부 리팩토링
- MINOR 증가(예: 2.4.11 → 2.5.0): 새 기능 추가, 하위 호환 유지
- MAJOR 증가(예: 2.4.11 → 3.0.0): 하위 호환이 깨지는 변경

빌드 설정만(`.github/`, 빌드 워크플로 등) 변경한 경우, 문서(`*.md`, `docs/`)만 수정한 경우, 테스트(`tests/`)만 추가/수정한 경우에는 버전 bump가 불필요하다.

**자동 bump (안전망).** master에 머지되면 릴리스 파이프라인(`.github/workflows/publish.yml`의 `gate` job, 로직은 `.github/scripts/release_gate.py`)이 변경 경로를 검사한다.

- 출하 변경인데 수동 bump가 누락됐으면 → CI가 **PATCH를 자동 증가**하고 `[skip ci]` 커밋으로 master에 push한 뒤 릴리스를 진행한다. 따라서 PATCH 누락으로 배포가 막히는 일은 없다.
- 이미 수동으로 bump했으면(특히 MINOR/MAJOR) → **그대로 존중**한다(CI는 추가 bump하지 않음). 즉 MINOR/MAJOR가 필요한 변경은 여전히 손으로 `pyproject.toml`을 올려야 한다.
- 출하물에 영향이 없는 변경(docs/CI/tests만)이면 → bump도, PyPI publish도, Nuitka 빌드도 **일어나지 않는다**(러너 절약).

경로 판정·제외 목록의 정본은 `.github/scripts/release_gate.py`의 `EXCLUDE`이며, 동작은 `tests/test_release_gate.py`가 고정한다.

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

출하 변경을 수동 bump 없이 머지해 CI가 PATCH를 자동 증가시킨 경우에는, 자동 bump 커밋이 git log 커밋 제목 기반의 최소 항목을 CHANGELOG에 자동 삽입한다(🔧 카테고리, "CI auto-bump" 명시). 이는 출하 추적용 placeholder이므로, 가능하면 PR에서 수동으로 bump + 의미 있는 CHANGELOG 항목을 직접 작성해 자동 삽입을 피하는 것이 좋다.

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

#### Step 2: SHA-256 해시 비교

동일성 판정은 SHA-256 기반이다. 해시는 플랫폼(부동소수점/라이브러리)에 따라 달라지므로 절대값 baseline이 아니라 **같은 머신에서의 변경 전후 자기 비교**로 판정한다.

```bash
# 현재 브랜치 출력
sha256sum data/demo/hesuvi.wav

# 기준(master) 출력 — 작업 트리를 건드리지 않도록 임시 worktree 사용
git worktree add /tmp/brir-baseline origin/master
cd /tmp/brir-baseline && python impulcifer.py --dir_path=data/demo \
    --test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl \
    --vbass --vbass_freq=250
sha256sum data/demo/hesuvi.wav
cd - && git worktree remove --force /tmp/brir-baseline
```

주의: 로컬 `master` ref는 뒤처져 있을 수 있으므로 반드시 `origin/master`(또는 검증된 커밋 SHA)를 기준으로 하고, `git checkout master -- .` 같은 방식은 절대 쓰지 말 것(작업 트리를 오염시킨다).

CI의 `brir-integrity` job(`tests/test_brir_integrity.py`)도 같은 방식이다: 하나의 hardcoded 해시를 보지 않고, 같은 Ubuntu CPython 3.13 환경에서 기준 ref(`origin/master`)와 현재 브랜치의 `hesuvi.wav`를 모두 생성해 SHA-256으로 비교한다. 비교 대상은 기본값(헤드폰 보정 포함)과 `--vbass --vbass_freq=250` 두 경로다.

두 해시가 일치하면 무결성 확인 완료. 불일치하면 Step 3으로 진행한다.

의도적으로 알고리즘을 변경하여 출력이 달라져야 하는 PR에서는, 변경 전후의 차이를 Step 3의 주파수 응답 분석으로 문서화한다(CI는 상대 비교이므로 의도된 변경은 PR 설명에 근거를 남긴다).

#### Step 3: 주파수 응답 분석 (해시 불일치 시)

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

원인을 특정했으면 해당 코드를 수정한 뒤, Step 1부터 다시 수행한다. SHA-256 해시가 기준 출력과 일치할 때까지 반복한다. compare_fr.py의 출력을 PR 코멘트나 커밋 메시지에 첨부하면 추적에 도움이 된다.

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

## GitHub 작업 도구

이 개발 환경에는 GitHub CLI(`gh`)가 설치되어 있고 `github.com` 계정 인증도 완료되어 있다. Issue 생성·조회·댓글, PR 생성·조회·댓글, Actions 상태·로그 확인 등 GitHub 작업은 브라우저나 Computer Use 자동화보다 `gh` CLI를 우선 사용한다(`gh issue ...`, `gh pr ...`, `gh run ...`). `gh`로 지원되지 않는 작업임을 확인한 경우에만 다른 수단을 사용한다.

## PR push 이후 CI 감시 및 회귀 대응

`git push` 후 작업이 끝났다고 보고하지 말 것. 로컬 검증이 통과해도 CI에서만 재현되는 회귀가 있으므로, 푸시 직후 CI 결과까지 확인하고 실패가 있으면 같은 세션에서 후속 커밋으로 수정한 뒤 다시 그린이 될 때까지 따라간다.

### 감시 절차

푸시 직후 다음 명령으로 PR의 모든 체크가 끝날 때까지 기다린다.

```bash
gh pr checks <PR번호> --watch --fail-fast
```

`--fail-fast`는 한 체크가 실패하는 즉시 종료시켜 빠르게 다음 단계(수정)로 넘어가게 한다. 비대화 환경에서는 30초 폴링 루프로도 같은 효과를 낼 수 있다.

```bash
prev=""
while true; do
  s=$(gh pr checks <PR번호> --json name,bucket 2>/dev/null || echo "[]")
  cur=$(echo "$s" | jq -r '.[] | select(.bucket!="pending") | "\(.name): \(.bucket)"' | sort)
  comm -13 <(echo "$prev") <(echo "$cur")
  prev=$cur
  echo "$s" | jq -e 'all(.bucket!="pending")' >/dev/null && break
  sleep 30
done
```

### 실패 시 대응

CI 실패가 보고되면 다음 순서로 대응한다.

1. **실패한 체크의 로그를 끝까지 읽는다.** `gh run view <run-id> --log-failed` 또는 체크 페이지의 stdout/stderr를 받아온다. 첫 번째 traceback / assertion 메시지뿐 아니라 그 위 컨텍스트까지 본다.
2. **로컬에서 재현되는지 확인한다.** 같은 테스트를 동일한 환경 변수와 함께 돌려본다. Windows에서는 `tests/test_brir_integrity.py`처럼 Linux/Python 3.13만 동작하도록 skipif가 걸린 테스트가 있으므로, 재현이 안 되면 CI 환경 의존성을 의심한다.
3. **회귀 원인을 코드 차원에서 특정한다.** "테스트가 깐깐해서"라고 결론 내리지 말 것. 거의 모든 경우 진짜 회귀(미처 인지하지 못한 사이드이펙트)다. 특히 `core/cli_builder.py`가 `core/pipeline.py`의 dataclass field default를 그대로 argparse default로 옮긴다는 점에 주의한다 — `ProcessingConfig` field의 default를 만지면 CLI default 동작이 바뀌고 BRIR 해시가 깨진다.
4. **수정은 같은 PR에 후속 커밋으로 올린다.** 별도 PR을 만들지 말고, 같은 브랜치에 fixup commit을 올린 뒤 다시 CI를 따라간다. 커밋 메시지에 실패한 체크 이름과 원인을 한 줄로 적어둔다.
5. **CHANGELOG도 같이 갱신한다.** 회귀 원인과 수정 방향이 처음 PR에 적힌 의도와 다르다면, CHANGELOG의 해당 항목을 새 의도에 맞게 다시 쓴다(잘못 적힌 채로 release되는 것을 막기 위해).

### "작업 완료" 기준

다음 셋이 모두 충족될 때만 보고한다.

- 로컬 Tier 1+2(런타임 변경이면 + Tier 3) 통과
- `gh pr checks <PR번호>`의 모든 required 체크가 `pass`
- `gh pr view <PR번호> --json mergeStateStatus`가 `CLEAN` 또는 `UNSTABLE`(non-required 체크만 실패) 상태

체크가 `pending`/`queued`인 상태로 보고하지 말 것 — 사용자가 추가로 모니터링해야 할 짐이 된다.

## 빌드 / 릴리스 파이프라인

Nuitka standalone 빌드의 엔트리포인트는 `gui_main.py`다. `pyproject.toml`의 `[project.scripts]`에 정의된 콘솔 스크립트(`impulcifer`, `impulcifer_gui`, `impulcifer_gui_legacy`)는 pip 설치 전용이며 standalone 빌드와 무관하다.

릴리스는 단일 게이트 파이프라인 `.github/workflows/publish.yml` 하나로 처리한다(이전의 `publish.yml` + `python-publish.yml` + `release-cross-platform.yml`을 통합). master push 시 다음 순서로 진행한다.

```
master push
  └─ gate           : 변경 경로 검사 → 출하 변경이면 (수동 bump 없을 때) PATCH 자동 bump
  └─ publish-pypi   : (should_release일 때만) wheel 빌드 + PyPI 발행 — environment: PyPI (OIDC)
  └─ build-*        : (publish-pypi 성공 후에만) Windows/macOS/Linux Nuitka 빌드
  └─ create-release : 산출물 모아 GitHub Release (태그 vX.Y.Z)
```

핵심 불변식:

- **파일명은 `publish.yml`을 유지해야 한다.** PyPI Trusted Publisher가 OIDC를 워크플로 파일명 + `environment: PyPI`에 바인딩하므로, 파일명을 바꾸면 pypi.org 설정을 함께 갱신하지 않는 한 발행이 깨진다.
- **빌드는 PyPI 발행 성공 후에만 돈다**(`build-*` 잡의 `needs: publish-pypi`). 출하물에 영향 없는 push(docs/CI/tests만)는 `gate`가 `should_release=false`로 판정해 PyPI·Nuitka 모두 건너뛴다 — 러너 절약. 모든 post-gate 잡은 명시적 `if: needs.gate.outputs.should_release == 'true'`를 유지하고 `always()`/`!cancelled()`를 쓰지 말 것.
- `gate`의 auto-bump 커밋은 `[skip ci]`를 달아 push하므로 파이프라인이 재트리거되지 않는다(무한 루프 방지). 후속 잡은 bump 커밋 SHA(`release_sha`)를 checkout한다.
- 게이트 결정 로직은 `.github/scripts/release_gate.py`(순수 함수는 `tests/test_release_gate.py`로 고정), 빌드 플래그는 `build_scripts/nuitka_flags.py`가 정본이다.

`build-linux.yml`·`build-macos.yml`은 `workflow_dispatch`/`workflow_call` 전용 수동 단일-플랫폼 빌드 도구로, master push에 자동 실행되지 않는다(러너 낭비 없음).

### master 머지 후 릴리스 파이프라인 확인

PR이 master에 머지되면 위 파이프라인이 돈다. 출하 변경이 포함된 PR을 머지한 뒤에는 `publish.yml` 실행 결과까지 확인한다 — `gate`가 의도대로 release/bump를 판정했는지, auto-bump가 발생했다면 master에 `chore(release): auto-bump ...` 커밋이 올라왔는지, PyPI 발행과 3-플랫폼 빌드가 통과했는지 본다. docs/CI/tests만 바꾼 PR이라면 `gate`가 `should_release=false`로 끝나고 빌드 잡들은 skip되는 것이 정상이다.
