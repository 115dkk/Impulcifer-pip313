# Changelog
Here you'll find the history of changes. The version numbering complies with [SemVer]() system which means that when the
first number changes, something has broken and you need to check your commands and/or files, when the second number
changes there are only new features available and nothing old has broken and when the last number changes, old bugs have
been fixed and old features improved.

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
