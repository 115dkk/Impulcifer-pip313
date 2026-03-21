# Impulcifer Android 포팅 종합 계획서

## Context

Impulcifer는 Python 기반 HRIR(Head-Related Impulse Response) 측정 및 바이노럴 오디오 처리 시스템이다.
사용자의 목표는 Modern GUI의 "Impulcifer" 탭 기능, 즉 **PC에서 녹음한 데이터를 받아 BRIR을 생성하는 배치 처리 파이프라인**을 Android에서 실행하는 것이다. 실시간 오디오나 녹음 기능은 불필요하다.

핵심 장벽은 **scipy가 Android에서 사용 불가**하다는 점이며, 이 문서는 이를 극복하는 구체적 전략을 기술한다.

---

## 0. 프로젝트 관리 결정: 새로운 리포지토리 생성

### 결론: `Impulcifer-android` 별도 리포지토리 생성

| 고려사항 | 새 리포 | 별도 브랜치 | master 구조변경 |
|----------|---------|------------|----------------|
| 빌드 시스템 | Gradle 독립 | Gradle+Hatchling 충돌 | 서브디렉토리 분리 필요 |
| 코드 동기화 | monkey-patch로 원본 무수정 | cherry-pick 지옥 | 자연스럽지만 복잡 |
| 유지보수 | 낮음 | 매우 높음 | 중간 |
| Android 관행 | 표준 | 비표준 | 비표준 |

**핵심 설계**: Impulcifer 원본 Python 코드를 **한 줄도 수정하지 않고** 사용한다.
Android 앱 시작 시 `sys.modules` monkey-patch로 scipy를 scipy_shim으로 투명 교체:

```python
# android_bootstrap.py — 앱 초기화 시 최초 1회 실행
import sys
import scipy_shim
sys.modules['scipy'] = scipy_shim
sys.modules['scipy.fft'] = scipy_shim.fft
sys.modules['scipy.signal'] = scipy_shim.signal
sys.modules['scipy.signal.windows'] = scipy_shim.signal.windows
sys.modules['scipy.stats'] = scipy_shim.stats
sys.modules['scipy.interpolate'] = scipy_shim.interpolate
sys.modules['scipy.ndimage'] = scipy_shim.ndimage

# 이후 impulcifer.main()을 수정 없이 호출 가능
import impulcifer
impulcifer.main(dir_path=..., ...)
```

이를 통해:
- Impulcifer-pip313 데스크톱 버전의 버그픽스가 Android에도 자동 적용
- 코드 동기화 문제 원천 해소
- scipy_shim만 별도 유지보수하면 됨

---

## 1. 포팅 범위 (Scope)

### 포팅 대상 (Modern GUI "Impulcifer" 탭 기능)
| 모듈 | 파일 | 라인수 | 역할 |
|------|------|--------|------|
| 메인 파이프라인 | `impulcifer.py` | 1,549 | CLI 오케스트레이션 → `main()` |
| HRIR 처리 | `core/hrir.py` | 2,050 | 바이노럴 IR 관리, ITD 정렬, ILD 분석 |
| IR 분석 | `core/impulse_response.py` | 1,183 | IR 분석, 디케이 파라미터 |
| IR 추정 | `core/impulse_response_estimator.py` | 368 | 스웹 사인 기반 IR 추출 |
| 룸 보정 | `core/room_correction.py` | 392 | 주파수 응답 보정 |
| 마이크 편차 보정 | `core/microphone_deviation_correction.py` | 847 | v3.0 마이크 배치 보정 |
| 가상 베이스 | `core/virtual_bass.py` | 396 | 최소위상 베이스 합성 |
| 유틸리티 | `core/utils.py` | 732 | 오디오 I/O, DSP 헬퍼 |
| 채널 생성 | `core/channel_generation.py` | 109 | TrueHD 다채널 자동 생성 |
| 상수 | `core/constants.py` | 137 | 스피커 정의, 채널 레이아웃 |
| **AutoEq** | `autoeq/frequency_response.py` | ~2,000+ | FrequencyResponse 클래스 (11개 모듈에서 사용) |

### 포팅 제외
- `core/recorder.py` — 실시간 녹음 (sounddevice, PortAudio 필요)
- `gui/modern_gui.py`, `gui/legacy_gui.py` — CustomTkinter/Tkinter GUI
- `core/parallel_processing.py` — Python 3.14 free-threaded 최적화 (Android에서 불필요)
- `research/` — 연구용 스크립트
- 시각화 코드 (matplotlib, bokeh, seaborn)

---

## 2. 접근 방식 비교

| 접근법 | 노력 | 성능 | APK 크기 | 코드 재사용 | 추천도 |
|--------|------|------|----------|------------|--------|
| **A. Chaquopy + scipy 심** | 중 (4-8주) | 양호 | ~50MB | **85%+** | **최우선 추천** |
| B. C++/NDK 전체 재작성 | 극대 (12-24주) | 최상 | ~10MB | 0% | 장기 목표 |
| C. WebAssembly/PWA (Pyodide) | 중상 (6-12주) | 보통 | ~40MB+ | 70% | 차선 대안 |
| D. Kotlin 전체 재작성 | 극대 (16-30주) | 상 | ~5MB | 0% | 비추천 |

---

## 3. 추천 전략: Approach A — Chaquopy + scipy 대체 심 라이브러리

### 3.1 핵심 아이디어

Android 앱을 **Kotlin(UI) + Chaquopy(Python 엔진)** 구조로 만든다.
- Chaquopy가 Python 3.x + NumPy를 Android에서 실행
- scipy 호출 ~12개 함수군을 **numpy 대체** 또는 **순수 Python/C 심 라이브러리**로 교체
- AutoEq의 scipy 의존성도 동일하게 교체
- GUI는 Jetpack Compose로 새로 작성

### 3.2 scipy 함수별 대체 전략

#### 즉시 대체 가능 (numpy 드롭인)
| scipy 함수 | 대체 방법 | 난이도 |
|------------|----------|--------|
| `scipy.fft.fft` / `rfft` | `numpy.fft.fft` / `rfft` | 즉시 |
| `scipy.fft.fftfreq` | `numpy.fft.fftfreq` | 즉시 |
| `scipy.signal.convolve` | `numpy.convolve` (1D) | 즉시 |
| `scipy.signal.windows.hann` | `numpy.hanning` | 즉시 |
| `scipy.signal.unit_impulse` | `np.zeros(n); arr[0] = 1.0` | 즉시 (2줄) |
| `scipy.stats.linregress` | `numpy.polyfit(x, y, 1)` | 즉시 (3줄) |
| `scipy.ndimage.uniform_filter` | `numpy.convolve` + uniform kernel | 쉬움 (5줄) |

#### 커스텀 구현 필요 (순수 Python+NumPy로 가능)
| scipy 함수 | 대체 방법 | 난이도 | 라인수 |
|------------|----------|--------|--------|
| `scipy.fft.next_fast_len` | 2/3/5-smooth 수 계산 | 쉬움 | ~15줄 |
| `scipy.signal.find_peaks` | NumPy 비교 연산 기반 구현 | 중 | ~40줄 |
| `scipy.signal.correlate` + `correlation_lags` | FFT 기반 교차상관 | 중 | ~20줄 |
| `scipy.interpolate.InterpolatedUnivariateSpline` | `numpy.interp` (선형) 또는 순수 Python 3차 스플라인 | 중 | ~60줄 |
| `scipy.interpolate.interp1d` | `numpy.interp` (선형) | 쉬움 | ~5줄 |

#### 핵심 난제 (C 라이브러리 또는 정밀 구현 필요)
| scipy 함수 | 대체 방법 | 난이도 | 비고 |
|------------|----------|--------|------|
| `scipy.signal.butter` + `sosfilt` | 양선형 변환(bilinear transform)으로 Butterworth 계수 직접 계산 + SOS 필터 적용 | **상** | 2차/4차만 사용됨 → 공식 직접 코딩 가능 |
| `scipy.signal.minimum_phase` | 힐베르트 변환 기반 구현 (numpy.fft 활용) | **상** | ~50줄, 수치 정밀도 검증 필수 |
| `scipy.signal.spectrogram` | STFT 직접 구현 (시각화 전용이므로 생략 가능) | 중 | Android에서는 생략 추천 |

### 3.3 AutoEq 내 scipy 의존성 처리

AutoEq의 `FrequencyResponse` 클래스가 내부적으로 사용하는 scipy:
- `scipy.interpolate.interp1d` → `numpy.interp`로 교체
- `scipy.signal` 관련 → 위 심 라이브러리 활용
- `scipy.fft` → `numpy.fft`로 교체

**방법**: `autoeq-py313` 패키지를 포크하여 `autoeq-android` 변형을 만들고, scipy 임포트를 심 모듈로 리다이렉트

---

## 4. 구현 단계

### Phase 0: 환경 구축 (1주)
- Android Studio + Kotlin + Chaquopy 프로젝트 설정
- Chaquopy에서 NumPy 동작 확인
- 기본 Jetpack Compose UI 스캐폴딩

### Phase 1: scipy 심 라이브러리 작성 (2-3주)
- `scipy_shim/` 패키지 생성 (순수 Python+NumPy)
- `scipy_shim.fft` — numpy.fft 래퍼 + `next_fast_len`
- `scipy_shim.signal` — `convolve`, `correlate`, `find_peaks`, `hann`, `unit_impulse`, `butter`, `sosfilt`, `minimum_phase`
- `scipy_shim.interpolate` — `interp1d`, `InterpolatedUnivariateSpline`
- `scipy_shim.stats` — `linregress`
- `scipy_shim.ndimage` — `uniform_filter`
- **각 함수에 대해 데스크톱 scipy 결과와의 수치 일치 테스트** 작성

### Phase 2: AutoEq 안드로이드 호환 포크 (1주)
- `autoeq-py313` 포크 → scipy 임포트를 `scipy_shim`으로 교체
- FrequencyResponse 클래스 동작 검증
- 주요 메서드 테스트: `smoothen()`, `compensate()`, `center()`, `copy()`

### Phase 3: Impulcifer 코어 안드로이드 적응 (2주)
- `impulcifer.py`의 `main()` 함수에서 시각화/녹음 코드 분리
- scipy 임포트를 `scipy_shim`으로 전환
- `soundfile` 대체: Android `MediaExtractor`/`MediaCodec` 또는 순수 Python WAV 모듈 (`wave` 표준 라이브러리)
- `nnresample` 대체: NumPy FFT 기반 리샘플링 또는 `wave` + 정수비 변환
- 파일 경로를 Android scoped storage에 맞게 조정

### Phase 4: Android UI 구현 (2-3주)
- Jetpack Compose로 Modern GUI "Impulcifer" 탭 재현
  - 입력 파일 선택 (dir_path, test_signal)
  - 처리 옵션 (룸 보정, 헤드폰 보상, EQ)
  - 고급 옵션 (리샘플, 타겟 레벨, 베이스 부스트, 틸트, 밸런스, 디케이 등)
  - 가상 베이스 옵션
  - "BRIR 생성" 버튼 → Chaquopy로 Python `impulcifer.main()` 호출
- 진행률 표시 (Python 로깅 → Android UI 콜백)
- 결과 파일 공유/내보내기

### Phase 5: 통합 테스트 및 검증 (1-2주)
- 데스크톱 Impulcifer와 동일 입력으로 출력 WAV 비교
- 비트 정밀도 검증 (32-bit float WAV)
- 다양한 채널 구성 테스트 (2ch, 14ch, 22ch)
- Android 디바이스별 테스트 (ARM64, x86_64 에뮬레이터)

---

## 5. 프로젝트 구조 (예상)

```
impulcifer-android/                    # 새 리포지토리
├── app/
│   ├── src/main/
│   │   ├── java/com/impulcifer/android/
│   │   │   ├── MainActivity.kt
│   │   │   ├── ui/                    # Jetpack Compose UI
│   │   │   │   ├── ImpulciferScreen.kt
│   │   │   │   ├── SettingsScreen.kt
│   │   │   │   └── components/
│   │   │   └── processing/
│   │   │       └── PythonBridge.kt    # Chaquopy ↔ Kotlin 브릿지
│   │   └── python/                    # Chaquopy Python 소스
│   │       ├── android_bootstrap.py   # sys.modules monkey-patch (핵심!)
│   │       ├── impulcifer.py          # 원본 그대로 복사 (무수정)
│   │       ├── core/                  # 원본 그대로 복사 (무수정)
│   │       ├── autoeq/                # 원본 그대로 복사 (무수정)
│   │       ├── scipy_shim/            # scipy 대체 라이브러리 (신규 작성)
│   │       │   ├── __init__.py
│   │       │   ├── fft.py
│   │       │   ├── signal/
│   │       │   │   ├── __init__.py
│   │       │   │   └── windows.py
│   │       │   ├── interpolate.py
│   │       │   ├── stats.py
│   │       │   └── ndimage.py
│   │       └── i18n/                  # 로컬라이제이션 (원본 복사)
│   └── build.gradle                   # Chaquopy 플러그인 설정
├── scipy_shim_tests/                  # 데스크톱에서 실행하는 정밀도 테스트
├── sync_upstream.sh                   # Impulcifer-pip313에서 Python 소스 동기화 스크립트
└── build.gradle
```

**`sync_upstream.sh`**: Impulcifer-pip313 데스크톱 리포에서 Python 소스를
`app/src/main/python/`으로 복사하는 스크립트. scipy_shim은 건드리지 않음.

---

## 6. 대안 접근법: Approach C — Pyodide/WebAssembly (차선)

만약 Chaquopy 접근이 너무 복잡하다면:

1. **Pyodide**가 scipy를 WASM으로 컴파일하여 포함 → scipy 대체 불필요
2. Android WebView 안에서 Pyodide 실행
3. JavaScript ↔ Python 브릿지로 파일 전달
4. 단점: 초기 로드 ~40MB, 성능 2-5x 느림, 네이티브 파일 접근 제한

이 접근법은 scipy 심을 만들 필요가 없어 **Phase 1-2를 완전히 건너뛸 수 있다**는 장점이 있다.

---

## 7. 검증 방법

1. **수치 정밀도 테스트**: `data/demo/` 샘플 데이터를 동일 파라미터로 처리하여 데스크톱 vs Android 출력 WAV의 RMSE < 1e-6 확인
2. **기능 테스트**: 모든 GUI 옵션 조합 (룸 보정 on/off, 헤드폰 보상 on/off, 가상 베이스 on/off) 검증
3. **scipy 심 단위 테스트**: 각 대체 함수의 입출력이 원본 scipy 함수와 수치적으로 일치하는지 확인
4. **성능 테스트**: 7채널 HRIR 처리가 Android에서 합리적 시간 내 완료되는지 (목표: 1분 이내)
5. **메모리 테스트**: 22채널/26채널 처리 시 Android OOM 발생하지 않는지 확인

---

## 8. 리스크 및 완화

| 리스크 | 영향 | 완화 방안 |
|--------|------|----------|
| Butterworth 필터 구현 정밀도 | 가상 베이스/마이크 보정 품질 저하 | 2차/4차 한정 공식으로 정밀 구현 + 수치 검증 |
| Chaquopy NumPy 성능 부족 | 처리 시간 과다 | NumPy 자체는 C 백엔드라 성능 양호; 병목 시 NDK 함수 추가 |
| soundfile 미동작 | WAV 파일 읽기 불가 | Python 표준 `wave` 모듈로 대체 (24/32bit 지원 추가 필요) |
| AutoEq 내부 scipy 체인 복잡 | 예상보다 많은 수정 필요 | AutoEq 소스를 직접 분석하여 심 범위 확정 (Phase 2에서 수행) |
| Android scoped storage 제한 | 파일 접근 패턴 변경 필요 | SAF(Storage Access Framework) + `DocumentFile` API 사용 |

---

## 9. 핵심 파일 경로 참조

- `/home/user/Impulcifer-pip313/impulcifer.py` — 메인 파이프라인 (`main()` 함수)
- `/home/user/Impulcifer-pip313/core/hrir.py` — HRIR 처리 핵심
- `/home/user/Impulcifer-pip313/core/impulse_response.py` — IR 분석
- `/home/user/Impulcifer-pip313/core/impulse_response_estimator.py` — IR 추출
- `/home/user/Impulcifer-pip313/core/room_correction.py` — 룸 보정
- `/home/user/Impulcifer-pip313/core/microphone_deviation_correction.py` — 마이크 편차 보정
- `/home/user/Impulcifer-pip313/core/virtual_bass.py` — 가상 베이스
- `/home/user/Impulcifer-pip313/core/utils.py` — 오디오 I/O, DSP 유틸
- `/home/user/Impulcifer-pip313/gui/modern_gui.py:967-1431` — Impulcifer 탭 UI 정의
- `/home/user/Impulcifer-pip313/gui/modern_gui.py:1671-1780` — `generate_brir()` 함수
- `/home/user/Impulcifer-pip313/requirements.txt` — 의존성 목록
- `/home/user/AutoEq-pip313/` — AutoEq 소스 (FrequencyResponse 클래스)
