# 마이크 착용 편차 보정 (Microphone Deviation Correction)

## 개요

마이크 착용 편차 보정 기능은 바이노럴(양귀) 임펄스 응답(BRIR) 측정 시 발생하는 좌우 귀 마이크의 위치/깊이 차이로 인한 주파수 응답 편차를 보정하는 기능입니다.

## 문제 상황

바이노럴 녹음에서는 사람의 양쪽 귀 위치에 소형 마이크를 삽입하거나 더미 헤드를 사용합니다. 이때 다음과 같은 문제가 발생할 수 있습니다:

- **마이크 위치 편차**: 좌우 귀에 삽입된 마이크의 위치나 깊이가 완벽하게 동일하지 않음
- **주파수 응답 왜곡**: 특히 고주파수 대역에서 각 귀가 인지하는 소리의 주파수 응답에 차이 발생
- **측정 아티팩트**: 실제 공간의 음향 특성이 아닌 순전히 측정 과정에서의 편차

## 해결 방법

### MTW (Minimum Time Window) 개념 활용

REW(Room EQ Wizard)의 MTW 개념을 차용하여 다음과 같이 접근합니다:

1. **극도로 짧은 시간 창 적용**: 직접음(direct sound) 구간만을 분석
2. **주파수 대역별 가변 게이팅**: 각 주파수 대역에 최적화된 게이트 길이 사용
3. **편차 분석**: 좌우 귀 간의 크기 및 위상 차이 계산
4. **보정 필터 생성**: 편차를 상쇄하는 FIR 필터 설계 및 적용

### 핵심 특징

- **주파수 의존적 게이팅**: 고주파는 짧은 게이트, 저주파는 상대적으로 긴 게이트 사용
- **대칭적 보정**: 좌우 귀에 반대 방향으로 절반씩 보정 적용
- **보정 강도 조절**: 과도한 보정을 방지하기 위한 강도 제어
- **최대 보정량 제한**: 안전장치로 최대 보정량 제한

## 사용법

### 1. 명령줄 인터페이스

```bash
# 기본 사용법
python impulcifer.py --dir_path /path/to/measurements --microphone_deviation_correction

# 보정 강도 조절 (0.0~1.0)
python impulcifer.py --dir_path /path/to/measurements --microphone_deviation_correction --mic_deviation_strength 0.5

# 분석 플롯과 함께 실행
python impulcifer.py --dir_path /path/to/measurements --microphone_deviation_correction --plot
```

### 2. Python API

```python
from hrir import HRIR
from impulse_response_estimator import ImpulseResponseEstimator

# HRIR 객체 생성 및 데이터 로드
estimator = ImpulseResponseEstimator.from_wav('test_signal.wav')
hrir = HRIR(estimator)
hrir.open_recording('measurements.wav', speakers=['FL', 'FR'])

# 마이크 편차 보정 적용
analysis_results = hrir.correct_microphone_deviation(
    correction_strength=0.7,  # 보정 강도
    plot_analysis=True,       # 분석 플롯 생성
    plot_dir='output_plots'   # 플롯 저장 디렉토리
)

# 결과 확인
for speaker, results in analysis_results.items():
    print(f"{speaker} 스피커 편차 분석 결과:")
    for freq, deviation in results['deviations'].items():
        print(f"  {freq} Hz: {deviation['magnitude_diff_db']:.2f} dB")
```

### 3. 직접 보정기 사용

```python
from microphone_deviation_correction import MicrophoneDeviationCorrector
import numpy as np

# 보정기 생성
corrector = MicrophoneDeviationCorrector(
    sample_rate=48000,
    correction_strength=0.7,
    max_correction_db=6.0
)

# 보정 적용
corrected_left, corrected_right, analysis = corrector.correct_microphone_deviation(
    left_ir=left_impulse_response,
    right_ir=right_impulse_response,
    plot_analysis=True,
    plot_dir='analysis_output'
)
```

## 파라미터 설명

### MicrophoneDeviationCorrector 파라미터

- **sample_rate** (int): 샘플링 레이트 (Hz)
- **octave_bands** (list): 분석할 옥타브 밴드 중심 주파수들. 기본값: [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
- **min_gate_cycles** (float): 최소 게이트 길이 (사이클 수). 기본값: 2
- **max_gate_cycles** (float): 최대 게이트 길이 (사이클 수). 기본값: 8
- **correction_strength** (float): 보정 강도 (0.0~1.0). 기본값: 0.7
- **smoothing_window** (float): 주파수 응답 스무딩 윈도우 크기 (옥타브). 기본값: 1/3
- **max_correction_db** (float): 최대 보정량 (dB). 기본값: 6.0

### CLI 파라미터

- **--microphone_deviation_correction**: 마이크 편차 보정 활성화
- **--mic_deviation_strength** (float): 보정 강도 (0.0~1.0). 기본값: 0.7

## 출력 결과

### 1. 분석 결과

각 스피커별로 다음 정보가 제공됩니다:

- **편차 메트릭**: 주파수 대역별 크기 차이(dB) 및 위상 차이(도)
- **보정 필터**: 좌우 귀에 적용된 FIR 필터
- **게이트 길이**: 각 주파수 대역별 사용된 게이트 길이

### 2. 시각화 플롯

`plot_analysis=True`로 설정하면 다음 플롯들이 생성됩니다:

- **편차 분석 결과**: 주파수별 크기 차이 및 위상 차이 그래프
- **보정 전후 비교**: 원본과 보정된 주파수 응답 비교

## 테스트

테스트 스크립트를 실행하여 기능을 확인할 수 있습니다:

```bash
python test_microphone_deviation.py
```

이 스크립트는:
1. 시뮬레이션된 편차가 있는 임펄스 응답 생성
2. 보정 알고리즘 적용
3. 보정 전후 결과 비교 및 시각화

## 주의사항

1. **적절한 보정 강도**: 과도한 보정은 오히려 음질을 해칠 수 있으므로 0.5~0.8 범위에서 시작하는 것을 권장
2. **처리 순서**: 마이크 편차 보정은 `crop_heads` 이후, `channel_balance` 이전에 수행됩니다
3. **반사음 영향**: 이 보정은 직접음 구간만을 대상으로 하므로 반사음이나 잔향에는 영향을 주지 않습니다
4. **측정 품질**: 기본적인 측정 품질이 좋아야 효과적인 보정이 가능합니다

## 기술적 세부사항

### 알고리즘 단계

1. **주파수 밴드 정의**: 옥타브 밴드별 분석 수행
2. **게이트 길이 계산**: 주파수에 따른 최적 게이트 길이 결정
3. **밴드패스 필터링**: 각 밴드별로 1/3 옥타브 필터 적용
4. **시간 게이팅**: 피크 이후 계산된 길이만큼 추출
5. **FFT 분석**: 각 밴드의 복소 응답 계산
6. **편차 계산**: 좌우 귀 간의 크기 및 위상 차이 분석
7. **보정 필터 설계**: 편차를 상쇄하는 FIR 필터 생성
8. **보정 적용**: 원본 임펄스 응답에 필터 적용

### 수학적 배경

- **게이트 길이**: `gate_samples = cycles * (fs / center_freq)`
- **크기 차이**: `magnitude_diff_db = 20 * log10(|left| / |right|)`
- **위상 차이**: `phase_diff = angle(left) - angle(right)`
- **보정량**: `correction = clip(deviation * strength, -max_db, max_db)`

## 관련 파일

- `microphone_deviation_correction.py`: 핵심 보정 알고리즘
- `hrir.py`: HRIR 클래스에 통합된 보정 메서드
- `impulcifer.py`: 메인 처리 파이프라인에 통합
- `test_microphone_deviation.py`: 테스트 스크립트

## 참고 문헌

- REW (Room EQ Wizard) MTW 개념
- 바이노럴 오디오 측정 및 처리 기법
- 디지털 신호 처리 및 FIR 필터 설계 