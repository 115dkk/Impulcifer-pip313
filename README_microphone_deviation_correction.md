# 마이크 착용 편차 보정 (Microphone Deviation Correction) v2.0

## 개요

마이크 착용 편차 보정 v2.0은 바이노럴(양귀) 임펄스 응답(BRIR) 측정 시 발생하는 좌우 귀 마이크의 위치/깊이 차이로 인한 주파수 응답 편차를 음향학적으로 정밀하게 보정하는 고급 기능입니다.

v2.0은 단순한 좌우 밸런싱을 넘어, **음향심리학(psychoacoustics)** 원리와 **해부학적 검증**을 기반으로 측정 품질을 획기적으로 개선합니다.

## 문제 상황

바이노럴 녹음에서는 사람의 양쪽 귀 위치에 소형 마이크를 삽입하거나 더미 헤드를 사용합니다. 이때 다음과 같은 문제가 발생할 수 있습니다:

- **마이크 위치 편차**: 좌우 귀에 삽입된 마이크의 위치나 깊이가 완벽하게 동일하지 않음
- **주파수 응답 왜곡**: 특히 고주파수 대역에서 각 귀가 인지하는 소리의 주파수 응답에 차이 발생
- **음상 정위 오류**: ITD(Interaural Time Difference) 및 ILD(Interaural Level Difference) 왜곡으로 공간감 손상
- **측정 아티팩트**: 실제 공간의 음향 특성이 아닌 순전히 측정 과정에서의 편차

## v2.0의 4가지 핵심 개선

### 1. 적응형 비대칭 보정 (Adaptive Asymmetric Correction) ⭐⭐⭐

**문제점 (v1.0):**
- 무조건 좌우 대칭으로 50:50 보정
- 어느 쪽 마이크가 더 정확한지 판단 불가
- 양쪽 모두에 불필요한 보정이 적용될 수 있음

**해결책 (v2.0):**
```python
def _evaluate_response_quality(self, responses):
    # 3가지 지표로 품질 평가
    # 1. 평균 크기 (SNR 추정)
    avg_magnitude = np.mean(magnitudes)

    # 2. Smoothness (변동성, 낮을수록 좋음)
    smoothness = np.std(np.diff(log_mags))

    # 3. 고주파 일관성 (노이즈 플로어 추정)
    snr_estimate = np.mean(high_freq_mags) / np.std(high_freq_mags)

    # 종합 품질 점수
    quality_score = log10(avg_mag)*0.3 + 1/(smoothness+0.1)*0.4 + log10(snr)*0.3
```

**효과:**
- 더 높은 품질의 응답을 참조 기준(reference)으로 자동 선택
- 품질이 낮은 쪽에 80% 보정, 좋은 쪽에 20% 보정 적용
- 불필요한 왜곡 최소화

**예시:**
```
좌측 품질: 5.2, 우측 품질: 3.8
→ 참조: 좌측 (더 우수)
→ 좌측에 -20% 보정, 우측에 +80% 보정
→ 결과: 우측을 좌측에 맞춤
```

### 2. 위상 보정 (Phase Correction) ⭐⭐⭐

**문제점 (v1.0):**
- 크기(magnitude)만 보정, 위상(phase) 무시
- ITD (Interaural Time Difference) 정보 손실
- 음상 정위(sound localization) 부정확

**이론적 배경:**
음향심리학의 **Duplex Theory**에 따르면:
- **저주파 (<1.5kHz)**: ITD로 방향 인지 (양 귀 도달 시간 차이)
- **고주파 (>4kHz)**: ILD로 방향 인지 (양 귀 소리 크기 차이)

ITD 공식:
```
ITD = phase_difference / (2π × frequency)
```

**해결책 (v2.0):**
```python
# 위상 차이 계산
phase_diff = angle(left_resp) - angle(right_resp)

# ITD 계산 (저주파 대역)
itd_seconds = phase_diff / (2 * pi * freq)

# 위상 보정량
if freq < 700:  # 저주파: ITD 중심
    phase_weight = 1.0
elif freq < 4000:  # 중간주파: 혼합
    phase_weight = 0.6
else:  # 고주파: ILD 중심
    phase_weight = 0.2

phase_correction = phase_diff * strength * phase_weight
```

**효과:**
- 음상 정위 정확도 향상
- 공간감(spatial impression) 개선
- 바이노럴 체험의 리얼리티 증가

### 3. ITD/ILD 해부학적 검증 (Anatomical Validation) ⭐⭐

**이론적 배경:**
인간 머리의 평균 반지름은 약 **8.75cm**입니다. 따라서 최대 ITD는:

```
최대 ITD = (머리 지름) / (음속)
         = (2 × 0.0875m) / (343 m/s)
         = 0.000510s
         ≈ 0.51ms
```

실제로는 귀 위치를 고려하여 약 ±0.7ms가 생리학적 한계입니다.

**구현:**
```python
def _validate_itd(self, phase_diffs_rad, frequencies):
    # 저주파 대역(<1500Hz)에서 ITD 계산
    for freq in low_frequencies:
        itd_ms = phase_diff / (2*pi*freq) * 1000

    # 해부학적 범위 검증
    expected_max_itd = (head_radius * 2) / speed_of_sound * 1000

    if abs(avg_itd) > expected_max_itd:
        warning: "ITD가 비정상적입니다 (마이크 배치 오류)"

    # ITD 일관성 검증
    if std(itd_samples) > 0.3ms:
        warning: "ITD 일관성이 낮습니다 (측정 노이즈)"
```

**효과:**
- 마이크 배치 오류 조기 감지
- 비현실적인 측정값에 대한 경고
- 측정 품질 보증 (QA)

**예시 경고:**
```
⚠️ ITD/ILD 해부학적 검증 경고:
  - ITD가 해부학적으로 비정상적입니다: 1.2ms (예상 범위: ±0.51ms).
    마이크 배치를 확인하세요.
```

### 4. 주파수 대역별 보정 전략 (Frequency-Dependent Strategy) ⭐⭐

**이론적 배경:**
Duplex Theory와 실험 데이터에 기반:

| 주파수 대역 | 지배적 큐 | 보정 전략 |
|------------|----------|----------|
| < 700Hz (저주파) | ITD | 위상 100%, 크기 30% |
| 700Hz - 4kHz (중간) | ITD + ILD | 위상 60%, 크기 70% |
| > 4kHz (고주파) | ILD | 위상 20%, 크기 100% |

**구현:**
```python
def _classify_frequency_bands(self):
    self.low_freq_bands = [f for f in octave_bands if f < 700]
    self.mid_freq_bands = [f for f in octave_bands if 700 <= f <= 4000]
    self.high_freq_bands = [f for f in octave_bands if f > 4000]

# 보정 시
if freq in low_freq_bands:
    mag_weight = 0.3
    phase_weight = 1.0
elif freq in mid_freq_bands:
    mag_weight = 0.7
    phase_weight = 0.6
else:  # high_freq_bands
    mag_weight = 1.0
    phase_weight = 0.2
```

**효과:**
- 주파수별 최적화된 보정
- 불필요한 왜곡 최소화
- 자연스러운 음질 보존

## 사용법

### 1. 명령줄 인터페이스 (CLI)

```bash
# 기본 사용법 (모든 v2.0 기능 활성화)
python impulcifer.py --dir_path /path/to/measurements --microphone_deviation_correction

# 보정 강도 조절 (0.0~1.0)
python impulcifer.py --dir_path /path/to/measurements \
  --microphone_deviation_correction \
  --mic_deviation_strength 0.5

# 분석 플롯과 함께 실행
python impulcifer.py --dir_path /path/to/measurements \
  --microphone_deviation_correction \
  --plot

# 특정 v2.0 기능 비활성화
python impulcifer.py --dir_path /path/to/measurements \
  --microphone_deviation_correction \
  --no_mic_deviation_phase_correction  # 위상 보정 OFF

python impulcifer.py --dir_path /path/to/measurements \
  --microphone_deviation_correction \
  --no_mic_deviation_adaptive_correction  # 적응형 보정 OFF (대칭 모드)

python impulcifer.py --dir_path /path/to/measurements \
  --microphone_deviation_correction \
  --no_mic_deviation_anatomical_validation  # 해부학적 검증 OFF
```

### 2. Python API

```python
from hrir import HRIR
from impulse_response_estimator import ImpulseResponseEstimator

# HRIR 객체 생성 및 데이터 로드
estimator = ImpulseResponseEstimator.from_wav('test_signal.wav')
hrir = HRIR(estimator)
hrir.open_recording('measurements.wav', speakers=['FL', 'FR'])

# v2.0 마이크 편차 보정 적용 (모든 기능 활성화)
analysis_results = hrir.correct_microphone_deviation(
    correction_strength=0.7,
    enable_phase_correction=True,        # 위상 보정
    enable_adaptive_correction=True,     # 적응형 비대칭 보정
    enable_anatomical_validation=True,   # ITD/ILD 검증
    plot_analysis=True,
    plot_dir='output_plots'
)

# 결과 확인
for speaker, results in analysis_results.items():
    print(f"\n{speaker} 스피커:")
    print(f"  참조 기준: {results['v2_features']['reference_side']}")
    print(f"  평균 편차: {results['avg_deviation_db']:.2f} dB")
    print(f"  최대 편차: {results['max_deviation_db']:.2f} dB")

    # ITD 검증 결과
    itd_validation = results['deviation_results']['itd_validation']
    if not itd_validation['valid']:
        for warning in itd_validation['warnings']:
            print(f"  ⚠️ {warning}")
```

### 3. GUI 사용 (v1.7.1+)

Modern GUI에서:
1. **Impulcifer 탭** 열기
2. **Advanced Options** 섹션으로 스크롤
3. **Mic Deviation Correction** 체크박스 활성화
4. **Strength** 값 설정 (0.0-1.0, 기본: 0.7)
5. **v2.0 Options** 세부 설정:
   - ☑ **Phase Correction**: 위상 보정 (기본: 활성화)
   - ☑ **Adaptive**: 적응형 비대칭 보정 (기본: 활성화)
   - ☑ **Anatomical Validation**: ITD/ILD 검증 (기본: 활성화)
6. **Run Impulcifer** 버튼 클릭

## 파라미터 설명

### MicrophoneDeviationCorrector 클래스 파라미터

```python
corrector = MicrophoneDeviationCorrector(
    sample_rate=48000,                      # 샘플링 레이트
    octave_bands=None,                      # 분석 주파수 밴드 (기본: [125-16000Hz])
    min_gate_cycles=2,                      # 최소 게이트 길이 (사이클)
    max_gate_cycles=8,                      # 최대 게이트 길이 (사이클)
    correction_strength=0.7,                # 보정 강도 (0.0-1.0)
    smoothing_window=1/3,                   # 스무딩 윈도우 (옥타브)
    max_correction_db=6.0,                  # 최대 보정량 (dB)

    # v2.0 새로운 파라미터
    enable_phase_correction=True,           # 위상 보정
    enable_adaptive_correction=True,        # 적응형 비대칭 보정
    enable_anatomical_validation=True,      # ITD/ILD 검증
    itd_range_ms=(-0.7, 0.7),              # 허용 ITD 범위
    head_radius_cm=8.75                     # 머리 반지름 (검증용)
)
```

### CLI 파라미터

| 파라미터 | 타입 | 기본값 | 설명 |
|---------|------|--------|------|
| `--microphone_deviation_correction` | flag | False | v2.0 보정 활성화 |
| `--mic_deviation_strength` | float | 0.7 | 보정 강도 (0.0-1.0) |
| `--no_mic_deviation_phase_correction` | flag | False | 위상 보정 비활성화 |
| `--no_mic_deviation_adaptive_correction` | flag | False | 적응형 보정 비활성화 |
| `--no_mic_deviation_anatomical_validation` | flag | False | 해부학적 검증 비활성화 |

## 출력 결과

### 1. 콘솔 출력

```
🎧 마이크 편차 보정 v2.0 시작
  - 위상 보정: 활성화
  - 적응형 보정: 활성화
  - 해부학적 검증: 활성화

🔊 처리 중: FL 스피커
📊 응답 품질 평가: 좌측=5.23, 우측=3.87
🎯 참조 기준: left (품질이 더 우수)

⚠️ ITD/ILD 해부학적 검증 경고:
  - 저주파 대역에서 ITD 일관성이 낮습니다 (표준편차: 0.42ms).
    측정 노이즈가 있을 수 있습니다.

  ✅ FL 스피커 마이크 편차 보정 완료
     평균 편차: 2.3 dB, 최대 편차: 4.7 dB
```

### 2. 시각화 플롯

`plot_analysis=True`로 설정하면 다음 플롯들이 생성됩니다:

#### A. 편차 분석 결과 (`microphone_deviation_analysis_v2.png`)

3개의 서브플롯:
1. **ILD (Interaural Level Difference)**: 주파수별 크기 차이 (dB)
2. **위상 차이**: 주파수별 위상 차이 (도)
3. **ITD (<1.5kHz)**: 저주파 대역 시간 차이 (ms) + 해부학적 범위 표시

#### B. 보정 전후 비교 (`microphone_deviation_correction_comparison_v2.png`)

2개의 서브플롯:
1. **주파수 응답**: 원본 vs 보정 후 (좌우 각각)
   - 참조 기준(left/right) 텍스트 박스 표시
2. **L-R 차이**: 보정 효과 가시화
   - 원본 차이 (보라색)
   - 보정 후 차이 (초록색)

### 3. 분석 결과 딕셔너리

```python
analysis_results = {
    'FL': {
        'deviation_results': {
            'frequency_deviations': {
                125: {
                    'magnitude_diff_db': 2.3,
                    'phase_diff_rad': 0.15,
                    'itd_ms': 0.19,
                    'left_magnitude': 0.85,
                    'right_magnitude': 0.73,
                    ...
                },
                ...
            },
            'itd_validation': {
                'valid': False,
                'warnings': ['ITD 일관성이 낮습니다...'],
                'itd_analysis': [(125, 12.3, 0.26), ...]
            },
            'left_quality': 5.23,
            'right_quality': 3.87,
            'reference_side': 'left'
        },
        'correction_filters': {
            'left_fir': array([...]),
            'right_fir': array([...])
        },
        'correction_applied': True,
        'avg_deviation_db': 2.3,
        'max_deviation_db': 4.7,
        'v2_features': {
            'phase_correction': True,
            'adaptive_correction': True,
            'anatomical_validation': True,
            'reference_side': 'left'
        }
    },
    ...
}
```

## 주의사항

### 1. 적절한 보정 강도

과도한 보정은 오히려 음질을 해칠 수 있습니다:
- **권장 범위**: 0.5 ~ 0.8
- **기본값**: 0.7 (적당한 보정)
- **0.5 이하**: 보수적 보정 (자연스러움 우선)
- **0.9 이상**: 공격적 보정 (정확도 우선, 부작용 위험)

### 2. 처리 순서

마이크 편차 보정은 다음 순서로 수행됩니다:
```
1. crop_heads (노이즈 제거)
2. crop_tails (꼬리 자르기)
3. ✅ correct_microphone_deviation (마이크 보정)
4. channel_balance (채널 밸런스)
5. room_correction (룸 보정)
6. headphone_compensation (헤드폰 보정)
...
```

### 3. 반사음 영향

이 보정은 **직접음 구간**만을 대상으로 하므로:
- ✅ 반사음이나 잔향에는 영향을 주지 않음
- ✅ 공간의 음향 특성 보존
- ✅ MTW (Minimum Time Window) 게이팅으로 분리

### 4. 측정 품질

기본적인 측정 품질이 좋아야 효과적:
- **필수**: 낮은 배경 노이즈
- **권장**: SNR > 40dB
- **중요**: 마이크 민감도 매칭

### 5. v2.0 기능 조합

모든 v2.0 기능을 함께 사용하는 것을 권장하지만, 필요시 개별 비활성화 가능:

| 시나리오 | 권장 설정 |
|---------|----------|
| 일반적인 경우 | 모두 활성화 (기본값) |
| 측정 환경이 좋음 | adaptive=False (대칭 보정) |
| 저품질 측정 | phase=False (크기만 보정) |
| 비표준 머리 크기 | anatomical=False |

## 테스트

테스트 스크립트를 실행하여 기능을 확인할 수 있습니다:

```bash
python test_microphone_deviation.py
```

이 스크립트는:
1. 시뮬레이션된 편차가 있는 임펄스 응답 생성
2. v2.0 보정 알고리즘 적용
3. 보정 전후 결과 비교 및 시각화
4. v2.0 기능별 효과 검증

## 기술적 세부사항

### 알고리즘 흐름

```
1. 주파수 밴드 정의
   [125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz]
   ↓
2. 게이트 길이 계산 (주파수별)
   고주파: 2 사이클, 저주파: 8 사이클
   ↓
3. 밴드패스 필터링 (1/3 옥타브)
   ↓
4. 시간 게이팅 (MTW)
   피크 이후 N 사이클만 추출
   ↓
5. FFT 분석
   복소 응답 계산 (크기 + 위상)
   ↓
6. 품질 평가 (v2.0)
   SNR, smoothness, consistency
   ↓
7. 편차 계산
   ILD: magnitude_diff_db
   ITD: phase_diff → time_diff
   ↓
8. ITD 검증 (v2.0)
   해부학적 타당성 체크
   ↓
9. 보정 필터 설계 (v2.0)
   주파수 대역별 전략 적용
   적응형 비대칭 보정
   위상 보정 포함
   ↓
10. FIR 필터 적용
    컨볼루션 (원본 IR * 보정 FIR)
```

### 수학적 배경

#### 품질 평가 점수
```
Q = 0.3 × log₁₀(avg_magnitude)
  + 0.4 × 1/(smoothness + 0.1)
  + 0.3 × log₁₀(SNR + 1.0)
```

#### ITD 계산
```
ITD(f) = Δφ(f) / (2π × f)

여기서:
- Δφ(f): 주파수 f에서의 위상 차이 (라디안)
- 저주파(<1500Hz)만 신뢰 가능
```

#### 보정량 계산
```
C_mag(f) = clip(ILD(f) × strength × w_mag(f), -6dB, +6dB)
C_phase(f) = Δφ(f) × strength × w_phase(f)

여기서:
- w_mag(f), w_phase(f): 주파수 대역별 가중치
- strength: 사용자 지정 보정 강도
```

#### 적응형 보정 비율
```
if Q_left ≥ Q_right:
    ratio = (0.2, 0.8)  # 좌측이 참조
else:
    ratio = (0.8, 0.2)  # 우측이 참조

C_left = -C × ratio[0]
C_right = C × ratio[1]
```

## 관련 파일

- `microphone_deviation_correction.py`: v2.0 핵심 알고리즘 (~829줄)
- `hrir.py`: HRIR 클래스에 통합된 보정 메서드
- `impulcifer.py`: 메인 처리 파이프라인에 통합
- `modern_gui.py`: GUI 인터페이스 (v1.7.1+)
- `test_microphone_deviation.py`: 테스트 스크립트
- `CHANGELOG.md`: 버전 히스토리

## 버전 히스토리

### v2.0 (2025-11-13) - 완전 재설계
- ✨ 적응형 비대칭 보정
- ✨ 위상 보정 추가 (ITD 반영)
- ✨ ITD/ILD 해부학적 검증
- ✨ 주파수 대역별 보정 전략
- 📊 개선된 시각화 (3개 플롯)
- 🔧 CLI 파라미터 4개 추가

### v1.0 (2024) - 초기 구현
- 기본 마이크 편차 보정
- MTW 게이팅
- 대칭 보정 (50:50)
- 크기(magnitude)만 보정

## 참고 문헌

1. **Duplex Theory of Sound Localization**
   - Lord Rayleigh (1907)
   - 저주파: ITD, 고주파: ILD

2. **REW (Room EQ Wizard) - Minimum Time Window**
   - 직접음 분리 기술
   - 주파수별 가변 게이팅

3. **Psychoacoustic Principles**
   - Mills, A. W. (1972). "Auditory localization"
   - Blauert, J. (1997). "Spatial Hearing"

4. **HRTF Measurement Standards**
   - AES-69 (2015)
   - ITU-R BS.2051

## 지원

문제가 발생하거나 질문이 있으면:
- GitHub Issues: https://github.com/115dkk/Impulcifer-pip313/issues
- 원본 프로젝트: https://github.com/jaakkopasanen/impulcifer

---

**마이크 편차 보정 v2.0**은 단순한 밸런싱 도구를 넘어, 음향학적 원리에 기반한 **측정 품질 향상 시스템**입니다.
