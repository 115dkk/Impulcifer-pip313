# Changelog
Here you'll find the history of changes. The version numbering complies with [SemVer]() system which means that when the
first number changes, something has broken and you need to check your commands and/or files, when the second number
changes there are only new features available and nothing old has broken and when the last number changes, old bugs have
been fixed and old features improved.

## 1.8.1 - 2025-11-14
### 완전한 GUI 현지화 - 모든 텍스트 번역 완료
GUI의 **모든 하드코딩된 텍스트**를 번역 키로 교체하여 완전한 다국어 지원을 구현했습니다.

#### 개선사항
- **100% GUI 번역**: 모든 레이블, 버튼, 메시지가 번역 가능
  - Recorder 탭: 오디오 장치, 파일, 녹음 옵션 등 모든 UI 요소
  - Impulcifer 탭: 처리 옵션, 룸 보정, 헤드폰 보상, 고급 옵션 등
  - 메시지 다이얼로그: 오류, 경고, 확인 메시지 모두 번역

- **번역 품질 개선**:
  - "Room Correction" → "룸 보정" (명확한 의미 전달)
  - "Headphone Compensation" → "헤드폰 보상" (음향 보정)
  - "Tilt" → "기울기" (스펙트럼 경사)
  - "per channel" → "채널별 설정" (명확한 표현)
  - 모든 기술 용어의 의미를 재검토하여 적절한 번역어 선택

- **메시지 현지화**:
  - 오류 메시지 (파일 없음, 녹음 실패 등)
  - 경고 메시지 (채널 불일치 등)
  - 확인 다이얼로그 (녹음 시작 등)
  - 완료 메시지 (녹음 완료, 처리 완료 등)

#### 기술적 개선
- 모든 하드코딩된 문자열을 `self.loc.get('key')` 형태로 교체
- 47개 UI 텍스트 일괄 교체 스크립트 사용
- 메시지 다이얼로그 번역 자동화
- 포맷 문자열 지원 (파일명, 오류 메시지 등 동적 텍스트)

#### 사용자 경험
- 선택한 언어로 모든 UI가 표시됨
- 오류 메시지도 모국어로 이해하기 쉬움
- 일관된 용어 사용으로 혼란 감소
- 전문 용어도 적절한 번역으로 명확하게 이해 가능

#### 번역 완료 현황
- ✅ 영어 (English): 완전 업데이트
- ✅ 한국어: 완전 업데이트, 번역 품질 재검토 완료
- ⏳ 기타 언어: 1.8.0 키 사용 (기본 번역), 추후 업데이트 예정

## 1.8.0 - 2025-11-14
### 다국어 지원 - 전 세계 사용자를 위한 현지화
Impulcifer GUI가 이제 9개 언어를 지원합니다! 영어를 모르는 사용자도 쉽게 사용할 수 있습니다.

#### 🌍 지원 언어
- 🇬🇧 English (영어)
- 🇰🇷 한국어 (Korean)
- 🇫🇷 Français (프랑스어)
- 🇩🇪 Deutsch (독일어)
- 🇪🇸 Español (스페인어)
- 🇯🇵 日本語 (일본어)
- 🇨🇳 简体中文 (중국어 간체)
- 🇹🇼 繁體中文 (중국어 번체)
- 🇷🇺 Русский (러시아어)

#### 새로운 기능
- **자동 언어 감지**:
  - 첫 실행 시 시스템 언어를 자동으로 감지
  - 지원하지 않는 언어는 영어로 기본 설정
  - 사용자 친화적인 언어 선택 다이얼로그

- **UI Settings 탭** (`⚙️ UI 설정`):
  - **언어 설정**: 9개 언어 중 선택 가능
  - **테마 설정**: Dark/Light/System 테마 선택
  - 모든 설정은 자동 저장 (~/.impulcifer/settings.json)

- **현지화 시스템** (`localization.py`):
  - 완전한 번역 관리 시스템
  - JSON 기반 언어 파일 (locales/*.json)
  - 실시간 언어 변경 (재시작 권장)
  - 사용자 설정 영구 저장

#### 기술적 개선
- **설정 관리**:
  - 사용자별 설정 디렉토리: `~/.impulcifer/`
  - 언어 설정 자동 저장
  - 테마 설정 자동 저장
  - 첫 실행 감지 시스템

- **확장성**:
  - 새로운 언어 추가가 간편함 (JSON 파일만 추가)
  - 모든 UI 텍스트가 번역 가능하도록 설계
  - 번역 키 기반 시스템으로 유지보수 용이

#### 사용자 경험 개선
- 깔끔해진 헤더 UI (테마 버튼 제거, UI Settings 탭으로 이동)
- 언어 변경 시 재시작 안내 메시지
- 테마 변경 시 즉시 적용
- 직관적인 언어 선택 인터페이스

#### 파일 구조
```
locales/
├── en.json      # English
├── ko.json      # 한국어
├── fr.json      # Français
├── de.json      # Deutsch
├── es.json      # Español
├── ja.json      # 日本語
├── zh_CN.json   # 简体中文
├── zh_TW.json   # 繁體中文
└── ru.json      # Русский
```

## 1.7.2 - 2025-11-13
### CI/CD 개선 - 자동화된 테스트 및 품질 보증
배포 전 자동 테스트로 코드 품질을 보장합니다. TestPyPI와 PyPI 발행 전에 유닛 테스트가 자동으로 실행됩니다.

#### 새로운 기능
- **포괄적인 유닛 테스트 스위트** (`test_suite.py`):
  - 마이크 편차 보정 v2.0 테스트
  - ImpulseResponse 클래스 테스트
  - 모듈 임포트 테스트
  - 데이터 파일 존재 확인
  - 설정 파일 검증
  - 버전 일관성 테스트
  - 통합 테스트 (느린 테스트 별도 분류)

- **GitHub Actions 테스트 워크플로우** (`.github/workflows/test.yml`):
  - Python 3.9-3.13 다중 버전 테스트
  - pytest 기반 자동 테스트
  - 코드 커버리지 측정 (Codecov 통합)
  - 모듈 임포트 검증
  - 코드 품질 체크 (ruff)

- **PyPI 배포 워크플로우 개선** (`.github/workflows/python-publish.yml`):
  - **테스트 우선 배포**: 유닛 테스트 통과 후에만 빌드 및 배포
  - TestPyPI 발행 전 자동 검증
  - PyPI 발행 전 자동 검증
  - 테스트 실패 시 배포 자동 중단

#### 개발 환경 개선
- **requirements-dev.txt** 추가:
  - pytest >= 7.4.0
  - pytest-cov >= 4.1.0 (커버리지)
  - pytest-xdist >= 3.3.1 (병렬 테스트)
  - pytest-timeout >= 2.1.0 (타임아웃)

#### 워크플로우 구조
```
1. 코드 푸시/PR 생성
   ↓
2. 테스트 워크플로우 자동 실행
   - 유닛 테스트 (Python 3.9-3.13)
   - 임포트 테스트
   - 코드 품질 체크
   ↓
3. 테스트 통과 시에만 빌드
   ↓
4. TestPyPI / PyPI 발행
```

#### 사용법
```bash
# 로컬에서 테스트 실행
python test_suite.py

# pytest로 실행 (더 상세한 출력)
pytest test_suite.py -v

# 커버리지 포함
pytest test_suite.py --cov=. --cov-report=term-missing

# 느린 테스트 제외
pytest test_suite.py -m "not slow"

# 개발 환경 설치
pip install -r requirements-dev.txt
```

#### 기술적 개선사항
- 자동화된 회귀 테스트로 버그 조기 발견
- 배포 전 자동 검증으로 안정성 향상
- CI/CD 파이프라인 신뢰도 대폭 개선
- 다중 Python 버전 호환성 보장

### 사용자 임팩트
- ✅ **안정성**: 배포 전 자동 테스트로 품질 보증
- 🚀 **신뢰성**: TestPyPI 발행 전 검증으로 실수 방지
- 🔍 **투명성**: GitHub Actions에서 테스트 결과 실시간 확인
- 🛡️ **보호**: 테스트 실패 시 자동으로 배포 중단

## 1.7.1 - 2025-11-13
### GUI 개선 - 마이크 편차 보정 v2.0 완전 지원
Modern GUI에서 마이크 편차 보정 v2.0의 모든 고급 기능을 사용할 수 있습니다.

#### GUI 변경사항
- **v2.0 Options 섹션 추가**: Mic Deviation Correction 활성화 시 3개의 고급 옵션 사용 가능
  - ☑ **Phase Correction**: 위상 보정 (ITD 반영)
  - ☑ **Adaptive**: 적응형 비대칭 보정 (품질 기반 참조 선택)
  - ☑ **Anatomical Validation**: ITD/ILD 해부학적 검증
- 모든 v2.0 옵션은 기본값으로 활성화
- Mic Deviation Correction 체크박스로 일괄 활성화/비활성화

#### 문서 업데이트
- **README_microphone_deviation_correction.md**: 완전 재작성 (~567줄)
  - v2.0 4가지 핵심 개선사항 상세 설명
  - 음향학적 이론 배경 (Duplex Theory, ITD/ILD, 해부학적 검증)
  - 수학적 공식 및 알고리즘 흐름도
  - CLI/API/GUI 사용법 전체 문서화
  - 주의사항 및 권장 설정 가이드
  - 참고 문헌 (AES, ITU, psychoacoustics)

#### 기술 파일 변경
- `modern_gui.py` (lines 643-675, 803-814, 1010-1012):
  - v2.0 체크박스 3개 추가
  - `toggle_mic_deviation()` 함수 업데이트 (v2.0 옵션 동기화)
  - `run_impulcifer()` args에 v2.0 파라미터 3개 전달

### 사용법 (GUI)
1. Impulcifer 탭 → Advanced Options 섹션
2. **Mic Deviation Correction** 체크박스 활성화
3. **Strength** 값 조정 (0.0-1.0, 기본: 0.7)
4. **v2.0 Options** 세부 조정 (선택사항, 모두 기본 활성화)
5. Run Impulcifer 버튼 클릭

## 1.7.0 - 2025-11-13
### 🎯 주요 기능 개선 - 마이크 편차 보정 v2.0
완전히 재설계된 음향학적 마이크 편차 보정 시스템으로, 측정 품질을 획기적으로 개선합니다.

#### 새로운 기능 (v2.0)
1. **적응형 비대칭 보정** ⭐⭐⭐
   - 좌우 응답의 품질을 자동으로 평가 (SNR, smoothness, consistency 기반)
   - 더 높은 품질의 응답을 참조 기준으로 사용
   - 기존: 무조건 좌우 대칭 보정 → 개선: 품질 기반 비대칭 보정 (80:20 또는 20:80)

2. **위상 보정 추가** ⭐⭐⭐
   - ITD (Interaural Time Difference) 정보를 FIR 필터에 반영
   - 음상 정위(sound localization) 정확도 향상
   - 기존: 크기(magnitude)만 보정 → 개선: 크기 + 위상 동시 보정

3. **ITD/ILD 해부학적 검증** ⭐⭐
   - 인간 머리 크기(평균 반지름 8.75cm)에 기반한 ITD 범위 검증 (±0.7ms)
   - 비정상적인 측정값에 대한 경고 메시지 출력
   - 마이크 배치 오류 조기 감지 가능

4. **주파수 대역별 보정 전략** ⭐⭐
   - **저주파 (< 700Hz)**: ITD 중심, 크기 보정 30% 가중치
   - **중간주파 (700Hz - 4kHz)**: ITD/ILD 혼합, 크기 70%, 위상 60% 가중치
   - **고주파 (> 4kHz)**: ILD 중심, 크기 100%, 위상 20% 가중치
   - 음향심리학적 원리에 기반한 과학적 접근

#### CLI 파라미터 추가
- `--microphone_deviation_correction`: v2.0 활성화 (기본: 비활성화)
- `--mic_deviation_strength`: 보정 강도 (0.0-1.0, 기본: 0.7)
- `--no_mic_deviation_phase_correction`: 위상 보정 비활성화 (기본: 활성화)
- `--no_mic_deviation_adaptive_correction`: 적응형 보정 비활성화 (기본: 활성화)
- `--no_mic_deviation_anatomical_validation`: 해부학적 검증 비활성화 (기본: 활성화)

#### 개선된 시각화
- **ILD (Interaural Level Difference)** 플롯: 주파수별 크기 차이
- **ITD (Interaural Time Difference)** 플롯: 저주파 대역 시간 차이 + 해부학적 범위 표시
- **보정 효과** 플롯: 보정 전후 좌우 차이 비교
- 참조 기준(left/right) 및 품질 점수 표시

#### 성능 및 호환성
- 기존 v1.0 API와 100% 하위 호환
- 모든 v2.0 기능은 기본값으로 활성화됨
- 개별 기능을 선택적으로 비활성화 가능

#### 기술적 세부사항
- `microphone_deviation_correction.py`: 전면 재작성 (~829줄)
- `hrir.py`: v2.0 파라미터 지원 추가
- `impulcifer.py`: CLI 파라미터 4개 추가
- 음향심리학 논문 및 REW MTW 개념 기반 설계

## 1.6.2 - 2025-11-13
### 버그 수정
- **GUI 레이아웃 문제 해결**: Modern GUI에서 컨텐츠가 창 전체를 사용하지 않고 일부만 사용하던 문제 수정
  - Recorder와 Impulcifer 탭에 `grid_rowconfigure(0, weight=1)` 추가
  - 이제 GUI가 창 크기에 맞춰 동적으로 확장됨
- **Light 모드 가시성 문제 해결**: Light 모드 전환 시 테마 토글 버튼이 배경과 거의 같은 색으로 표시되어 식별 불가능하던 문제 수정
  - 버튼 색상을 명시적으로 지정 (Light/Dark 모드별)
  - Light 모드: 회색 배경에 검은색 텍스트
  - Dark 모드: 어두운 회색 배경에 밝은 텍스트

## 1.6.1 - 2025-11-12
### 주요 기능 추가
- **완전히 새로운 Modern GUI**: CustomTkinter 기반의 전문적인 GUI 구현
  - Windows 11/macOS Big Sur 스타일의 현대적인 디자인
  - 다크/라이트 모드 지원 (테마 토글 버튼)
  - 탭 UI로 Recorder와 Impulcifer 통합
  - 모든 CLI 기능 100% 구현 (30+ 기능)
  - 직관적인 레이아웃과 사용자 친화적 인터페이스
  - 실시간 validation 및 에러 핸들링

### GUI 세부 기능
- **Recorder 탭**: 오디오 장치 선택, 멀티채널 녹음 (14/22/26 채널), 동적 채널 가이던스
- **Impulcifer 탭**: 룸 보정, 헤드폰 보정, 커스텀 EQ, 15개 고급 옵션
- 레거시 GUI는 `impulcifer_gui_legacy` 명령으로 계속 사용 가능

### 성능 최적화
- **CI/CD 워크플로우**: pip → uv 전환으로 의존성 설치 50-80% 단축
- **Nuitka 빌드**: 멀티코어 컴파일 + LTO 비활성화로 빌드 시간 75-85% 단축 (2-4시간 → 15-30분)
- Nuitka 캐싱 추가로 재빌드 시 90%+ 시간 단축

### 버그 수정
- CustomTkinter 패키지를 Nuitka 빌드에 올바르게 포함하도록 개선 (`--include-package` 사용)
- tkinter 플러그인 명시적 활성화로 GUI 안정성 향상

### 개발자 경험 개선
- PyPI 엔트리 포인트: `impulcifer_gui` → 현대적인 GUI, `impulcifer_gui_legacy` → 레거시 GUI
- Nuitka 빌드 스크립트에 CustomTkinter 전체 패키지 포함
- 더 나은 주석과 코드 구조

## 1.5.2 - 2025-11-12
### 버그 수정
- **AutoEQ 훼손 문제 해결**: `headphone_compensation` 함수의 큐빅 스플라인 보간 fallback 로직에서 발생하던 치명적인 버그를 수정했습니다.
  - 문제: 큐빅 보간이 실패할 때 fallback이 복사본(`left_orig`)을 수정하고 실제 객체(`left`)는 그대로 두어, 잘못된 주파수 그리드로 보상이 이루어졌습니다.
  - 결과: 헤드폰과 룸의 이도 응답을 합성할 때 FR(Frequency Response) 및 임펄스 응답이 훼손되었습니다.
  - 해결: Fallback 람다 함수가 실제 객체를 수정하도록 변경하여 주파수 그리드 정렬이 올바르게 이루어지도록 했습니다.
- 이 수정으로 구버전에서 제대로 작동하던 결과가 복원되었습니다.

## 1.4.0 - 2024-12-20
### GUI에 추가된 기능들
- **임펄스 응답 사전 응답(Pre-response) 길이 조절 옵션**: 임펄스 응답의 시작 부분을 자르는 길이를 ms 단위로 조절할 수 있습니다. (기본값: 1.0ms)
- **JamesDSP용 트루 스테레오 IR(.wav) 생성 기능**: FL/FR 채널만 포함하는 jamesdsp.wav 파일을 생성합니다.
- **Hangloose Convolver용 개별 채널 스테레오 IR(.wav) 생성 기능**: 각 스피커 채널별로 별도의 스테레오 IR 파일을 생성합니다.
- **인터랙티브 플롯 HTML 파일 생성 기능**: Bokeh 기반의 대화형 플롯을 HTML 파일로 생성합니다.
- **마이크 착용 편차 보정(Microphone Deviation Correction) 기능**: 좌우 마이크 위치 차이로 인한 편차를 보정합니다. (강도: 0.0-1.0)

### 개선사항
- GUI의 고급 옵션(Advanced options) 섹션에 모든 새로운 기능들이 추가되었습니다.
- 각 기능에 대한 툴팁이 추가되어 사용자가 쉽게 이해할 수 있도록 했습니다.

## 1.0.0 - 2020-07-20
Performance improvements. Main features are supported and Impulcifer is relatively stable.
