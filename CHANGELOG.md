# Changelog
Here you'll find the history of changes. The version numbering complies with [SemVer]() system which means that when the
first number changes, something has broken and you need to check your commands and/or files, when the second number
changes there are only new features available and nothing old has broken and when the last number changes, old bugs have
been fixed and old features improved.

## 1.9.1 - 2025-11-14
### 🌐 업데이트 시스템 번역 완료
v1.9.0에서 추가된 자동 업데이트 시스템의 모든 문자열을 나머지 언어로 번역 완료했습니다.

#### 📝 번역 완료 언어
- ✅ **프랑스어 (Français)**: 14개 업데이트 문자열 번역 완료
- ✅ **독일어 (Deutsch)**: 14개 업데이트 문자열 번역 완료
- ✅ **스페인어 (Español)**: 14개 업데이트 문자열 번역 완료
- ✅ **일본어 (日本語)**: 14개 업데이트 문자열 번역 완료
- ✅ **중국어 간체 (简体中文)**: 14개 업데이트 문자열 번역 완료
- ✅ **중국어 번체 (繁體中文)**: 14개 업데이트 문자열 번역 완료
- ✅ **러시아어 (Русский)**: 14개 업데이트 문자열 번역 완료

#### 🎯 번역된 문자열
- 업데이트 알림 다이얼로그 제목 및 메시지
- 버전 정보 표시 (현재 → 새 버전)
- 릴리스 노트 섹션
- 3개 버튼: "지금 업데이트", "나중에 알림", "이 버전 건너뛰기"
- 다운로드 진행 상황 메시지
- 설치 진행 메시지
- 오류 메시지 (다운로드 실패, 설치 실패, 일반 오류)

#### 🌍 완전한 다국어 지원
이제 **모든 9개 지원 언어**에서 자동 업데이트 시스템을 완전히 사용할 수 있습니다. 사용자는 자신의 언어로 업데이트 알림을 받고 원활하게 업데이트를 진행할 수 있습니다.

#### 📦 변경된 파일
- `locales/fr.json`: 프랑스어 업데이트 문자열 추가
- `locales/de.json`: 독일어 업데이트 문자열 추가
- `locales/es.json`: 스페인어 업데이트 문자열 추가
- `locales/ja.json`: 일본어 업데이트 문자열 추가
- `locales/zh-cn.json`: 중국어 간체 업데이트 문자열 추가
- `locales/zh-tw.json`: 중국어 번체 업데이트 문자열 추가
- `locales/ru.json`: 러시아어 업데이트 문자열 추가

#### ✨ 사용자 경험
- 모든 언어 사용자가 동일한 품질의 업데이트 경험을 제공받음
- 업데이트 다이얼로그가 사용자의 선택 언어로 자동 표시
- 일관된 번역 품질로 혼란 없이 업데이트 프로세스 진행

## 1.9.0 - 2025-11-14
### 🎉 자동 업데이트 시스템 추가
프로그램이 자동으로 새 버전을 확인하고 설치할 수 있는 기능을 추가했습니다.

#### ✨ 새로운 기능
- **자동 업데이트 체크**: 프로그램 시작 시 GitHub 릴리즈에서 새 버전 자동 확인
- **업데이트 알림 다이얼로그**: 새 버전 발견 시 릴리스 노트와 함께 알림 표시
- **원클릭 업데이트**: "지금 업데이트" 버튼 클릭으로 자동 다운로드 및 설치
- **진행 상황 표시**: 다운로드 진행률을 실시간으로 표시
- **자동 재시작**: 설치 완료 후 새 버전 자동 실행

#### 🔧 구현 세부사항
- **`update_checker.py`**: GitHub API를 사용한 버전 체크
  - Semantic versioning 비교
  - 플랫폼별 다운로드 URL 자동 선택 (Windows/macOS/Linux)
  - 릴리스 노트 자동 가져오기
  - GitHub API rate limiting 처리

- **`updater.py`**: 다운로드 및 설치 관리
  - 백그라운드 다운로드 with 진행 상황 콜백
  - Windows: Inno Setup 설치 파일 자동 실행
  - macOS: DMG/PKG 파일 열기
  - Linux: DEB/RPM/AppImage 지원

- **`modern_gui.py`**: GUI 통합
  - 시작 2초 후 백그라운드에서 업데이트 체크
  - `UpdateDialog`: 업데이트 알림 및 관리 다이얼로그
  - 사용자 선택: "지금 업데이트", "나중에 알림", "이 버전 건너뛰기"
  - 현재 버전 자동 감지 (pyproject.toml에서 읽기)

#### 🌍 다국어 지원
- 영어 (`en.json`): 모든 업데이트 관련 문자열 추가
- 한국어 (`ko.json`): 모든 업데이트 관련 문자열 번역
- 업데이트 다이얼로그, 버튼, 메시지 모두 번역됨

#### 📦 의존성 추가
- **`packaging>=23.0`**: Semantic versioning 비교를 위해 추가

#### ⚙️ 빌드 설정 업데이트
- **Nuitka 빌드**: `update_checker`, `updater` 모듈 포함 추가
- 업데이트 시스템이 빌드된 실행 파일에서도 정상 작동

#### 💡 사용법
1. 프로그램 시작 시 자동으로 업데이트 확인
2. 새 버전이 있으면 다이얼로그가 자동으로 표시됨
3. "지금 업데이트" 클릭 → 다운로드 및 설치 자동 진행
4. "나중에 알림" 클릭 → 다음 실행 시 다시 확인
5. "이 버전 건너뛰기" 클릭 → 해당 버전 무시

#### 🔒 보안 및 안정성
- GitHub API를 HTTPS로만 통신
- 다운로드 실패 시 사용자에게 수동 다운로드 안내
- 네트워크 오류 시 조용히 실패 (사용자 방해 안함)

## 1.8.5 - 2025-11-14
### Nuitka 빌드 설정 수정 - 번역 시스템 복구
Nuitka 빌드에서 누락된 필수 모듈과 데이터를 추가하여 빌드된 프로그램의 번역 기능을 복구했습니다.

#### 🔴 긴급 수정 (번역 시스템 복구)
- **`localization` 모듈 추가**: 번역 시스템 모듈이 빌드에 포함되지 않던 문제 해결
- **`locales/` 디렉토리 추가**: 모든 번역 파일 (9개 언어) 이 빌드에 포함되도록 수정
  - en.json, ko.json, fr.json, de.json, es.json, ja.json, zh-cn.json, zh-tw.json, ru.json

#### 🔧 필수 모듈 추가
- **`logger` 모듈 추가**: 로깅 시스템이 빌드에 포함되도록 수정
- **`channel_generation` 모듈 추가**: 채널 생성 기능이 빌드에 포함되도록 수정

#### 🗑️ 불필요한 모듈 제거
- **`scipy.io.wavfile` 제거**: v1.8.4에서 코드에서 제거된 모듈을 빌드 설정에서도 제거

#### ⚙️ 엔트리 포인트 수정
- **자동 생성 엔트리 포인트 수정**: `gui` → `modern_gui` 로 변경
  - 기존에는 legacy GUI를 호출하도록 자동 생성되었으나, modern GUI를 사용하도록 수정

#### 📝 변경된 빌드 설정
```python
# build_nuitka.py에 추가된 항목:
"--include-module=localization",   # 번역 시스템
"--include-module=logger",         # 로깅 시스템
"--include-module=channel_generation",  # 채널 생성
"--include-data-dir=locales=locales",  # 번역 파일

# 제거된 항목:
# "--include-module=scipy.io.wavfile"  # 사용하지 않음
```

#### 🎯 영향
- **이전 빌드 (v1.8.4 이하)**: 번역이 작동하지 않았음
- **현재 빌드 (v1.8.5)**: 모든 번역 기능이 정상 작동

#### ⚠️ 참고
- Legacy GUI (`gui` 모듈) 는 호환성을 위해 계속 포함됨
- numpy, matplotlib 플러그인은 안정성을 위해 유지

## 1.8.4 - 2025-11-14
### 코드 품질 개선 - 린터 에러 수정
모든 주요 린터 에러를 수정하여 코드 품질을 개선했습니다.

#### 🔧 수정 사항
- **F401 (Unused imports)**: 사용하지 않는 import 제거
  - `logger.py`: `sys` import 제거
  - `test_suite.py`: `FrequencyResponse` import 제거
  - `impulcifer.py`: `scipy.io.wavfile` import 제거
  - `utils.py`: 사용하지 않는 `Path` import 제거 후 실제 사용 확인하여 복원

- **F541 (f-strings without placeholders)**: 불필요한 f-string을 일반 문자열로 변경
  - `hrir.py`: 20개 이상의 f-string 수정
  - 플레이스홀더가 없는 f-string을 일반 문자열로 변환

- **E722 (Bare except)**: 모든 bare except를 `except Exception`으로 변경
  - `impulcifer.py`: 2개 수정
  - `localization.py`: 3개 수정
  - `modern_gui.py`: 5개 수정
  - `utils.py`: 1개 수정
  - SystemExit, KeyboardInterrupt 등을 잘못 잡지 않도록 개선

- **E701/E702 (Multiple statements on one line)**: 한 줄에 여러 문장 분리
  - `hrir.py`: 9개의 복합 문장을 여러 줄로 분리
  - 가독성 및 디버깅 용이성 향상

- **E721 (Type comparison)**: `type() ==` 를 `isinstance()`로 변경
  - `hrir.py`: 4개 수정
  - `impulcifer.py`: 1개 수정
  - 상속을 고려한 올바른 타입 체크

#### ⚙️ 린터 설정 개선
- **Jupyter notebook 제외**: `pyproject.toml`에 Ruff/Flake8 설정 추가
  - `research/**/*` 디렉토리 제외
  - `*.ipynb` 파일 제외
  - 연구용 노트북은 린팅 대상에서 제외

#### ✅ 테스트
- 모든 단위 테스트 통과 (14 passed, 2 skipped)
- 코드 동작에 영향 없이 품질만 개선

## 1.8.3 - 2025-11-14
### 번역 시스템 버그 수정 및 UI 개선
v1.8.2에서 발생한 번역 관련 버그들을 수정하고 UI를 개선했습니다.

#### 🐛 버그 수정
- **번역 라벨 직접 출력 문제 해결**:
  - logger가 번역 키를 그대로 출력하던 문제 수정
  - LocalizationManager를 logger에 주입하여 자동 번역 활성화
  - `cli_*`, `message_*`, `error_*`, `warning_*`, `success_*`, `info_*` 접두사를 가진 메시지 자동 번역
  - 일반 텍스트는 그대로 출력 (하위 호환성 유지)

- **언어 선택 화면 짤림 현상 해결**:
  - 대화상자 크기를 400x300에서 400x550으로 확대
  - 9개 언어 옵션이 모두 정상적으로 표시됨
  - 스크롤 없이 모든 옵션 확인 가능

#### ⚙️ 기술적 개선
- **`logger.py` 현지화 지원 추가**:
  - `set_localization(loc_manager)`: LocalizationManager 주입
  - `_translate()`: 자동 번역 키 감지 및 변환
  - 모든 로깅 메서드에 `**kwargs` 추가로 포맷 파라미터 전달 지원

- **`modern_gui.py` 현지화 통합**:
  - BRIR 생성 시작 전 logger에 localization 설정
  - ProcessingDialog가 번역된 메시지를 실시간으로 표시

#### 📝 사용법
```python
# Logger with translation
from logger import get_logger
from localization import LocalizationManager

logger = get_logger()
loc = LocalizationManager()
logger.set_localization(loc)

# Translation keys are automatically translated
logger.info("cli_creating_estimator")  # → "Creating impulse response estimator" (en)
                                        # → "임펄스 응답 추정기 생성 중" (ko)

# Plain text works as before
logger.info("This is a plain message")  # → "This is a plain message"
```

#### ✅ 테스트
- 모든 pytest 테스트 통과 (15 passed, 2 skipped)
- 번역 키 자동 감지 및 변환 검증
- 다국어 전환 테스트 완료

## 1.8.2 - 2025-11-14
### GUI 처리 진행 상황 표시 및 CLI 메시지 통합
BRIR 생성 프로세스의 진행 상황을 GUI에서 실시간으로 확인할 수 있도록 개선했습니다.

#### 🎯 새로운 기능
- **처리 진행 다이얼로그** (`ProcessingDialog`):
  - 실시간 진행률 표시 (0-100%)
  - 현재 작업 단계 표시
  - 모든 처리 로그 실시간 표시
  - 완료 시 자동으로 닫기 버튼 활성화
  - 처리 중 다른 작업 방지 (모달 다이얼로그)

- **통합 로깅 시스템** (`logger.py`):
  - CLI와 GUI 양쪽에서 동작하는 통합 로거
  - 로그 레벨: DEBUG, INFO, SUCCESS, WARNING, ERROR, PROGRESS
  - GUI 콜백 지원으로 실시간 메시지 전달
  - 진행률 자동 계산 (step 기반 추적)

- **CLI 메시지 현지화**:
  - 모든 처리 단계 메시지에 대한 번역 키 추가
  - 영어/한국어 번역 완료
  - GUI에서 처리 메시지가 선택한 언어로 표시됨

#### ⚙️ 기술적 개선
- **`impulcifer.py` 리팩토링**:
  - 모든 `print()` 문을 `logger` 호출로 교체
  - 67개 이상의 print 문 → logger.step/info/success/warning/error
  - 처리 단계별 진행률 자동 추적
  - 총 단계 수 자동 계산 (활성화된 옵션 기반)

- **스레드 기반 처리**:
  - GUI가 멈추지 않도록 별도 스레드에서 BRIR 생성
  - 실시간 로그 및 진행률 업데이트
  - 안전한 에러 처리 및 사용자 알림

- **진행률 추적 시스템**:
  - `logger.set_total_steps()`: 총 단계 설정
  - `logger.step()`: 단계 실행 및 진행률 자동 증가
  - `logger.progress()`: 수동 진행률 설정 (0-100%)
  - 옵션별 동적 단계 계산

#### 📊 처리 단계 가시화
처리 중 다음과 같은 단계가 실시간으로 표시됩니다:
1. 임펄스 응답 추정기 생성
2. 룸 보정 (활성화 시)
3. 헤드폰 보상 (활성화 시)
4. 헤드폰 이퀄라이제이션 (활성화 시)
5. 주파수 응답 목표 생성
6. 바이노럴 측정값 로드
7. 게인 정규화
8. 임펄스 응답 잘라내기
9. 마이크 편차 보정 (활성화 시)
10. 이퀄라이제이션 적용
11. 감쇠 시간 조정 (활성화 시)
12. 채널 밸런스 보정 (활성화 시)
13. 그래프 생성 (활성화 시)
14. BRIR 파일 쓰기
15. 기타 출력 형식 생성 (TrueHD/JamesDSP/Hangloose)

#### 🌐 번역 추가
모든 CLI 처리 메시지에 대한 번역 키 추가:
- `cli_starting_brir_generation`: BRIR 생성 시작
- `cli_creating_estimator`: 임펄스 응답 추정기 생성 중
- `cli_running_room_correction`: 룸 보정 실행 중
- `cli_equalizing`: 이퀄라이제이션 적용 중
- `cli_writing_brirs`: BRIR 파일 쓰기 중
- ... 및 기타 25개 이상의 메시지 키

#### 🎨 사용자 경험
- **투명성**: 무엇이 처리되고 있는지 명확히 표시
- **신뢰성**: 프로그램이 멈춘 것처럼 보이지 않음
- **진행 추적**: 얼마나 남았는지 시각적으로 확인 가능
- **에러 가시성**: 오류 발생 시 즉시 확인 가능
- **언어 지원**: 선택한 언어로 진행 상황 표시

#### 🔧 파일 변경사항
- **신규 파일**:
  - `logger.py`: 통합 로깅 시스템

- **수정 파일**:
  - `impulcifer.py`: 67개 print 문을 logger 호출로 교체
  - `modern_gui.py`: ProcessingDialog 클래스 추가, 스레드 기반 처리
  - `locales/en.json`: 30개 CLI 메시지 번역 키 추가
  - `locales/ko.json`: 30개 CLI 메시지 한국어 번역 추가

#### 📝 코드 개선
- 폰트 로딩 메시지 정리 (debug 레벨로 변경, 필요시만 출력)
- 보간 경고 메시지 정리 (불필요한 출력 제거)
- 헤드폰 보상 파일 경고 개선 (logger 사용)
- TrueHD 변환 메시지 개선

#### 🚀 성능
- 멀티스레딩으로 GUI 응답성 유지
- 로그 메시지 실시간 전달 (버퍼링 없음)
- 진행률 계산 최적화

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
