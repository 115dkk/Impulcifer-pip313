# 마이크 착용 편차 보정

마이크 착용 편차 보정은 좌우 귀 마이크의 삽입 깊이, 각도, 감도 차이 때문에 생기는 공통적인 좌우 레벨 차이를 줄이는 기능입니다. 현재 구현은 v3.0 교차검증 방식입니다.

이 문서는 코드 기준으로 정리했습니다.

| 항목 | 기준 코드 |
| --- | --- |
| 보정 알고리즘 | `core/microphone_deviation_correction.py` |
| HRIR 통합 지점 | `core/hrir.py` |
| BRIR 파이프라인 호출 순서 | `impulcifer.py` |
| CLI 옵션 정의 | `core/pipeline.py` |
| GUI 인자 조립 | `gui/brir_args.py` |

## 현재 구현 요약

v3.0은 단일 스피커의 좌우 차이를 바로 마이크 오차로 보지 않습니다. 여러 스피커에서 반복되는 좌우 차이를 모아, 스피커 방향 때문에 생기는 정상적인 HRTF 차이와 마이크 착용 오차를 나눠 추정합니다.

처리 흐름은 다음과 같습니다.

1. 각 스피커의 좌우 IR peak를 찾습니다.
2. 250, 500, 1000, 2000, 4000, 8000 Hz 대역에서 1/3 octave band-pass와 짧은 time gate를 적용합니다.
3. 대역별 `left - right` 레벨 차이를 수집합니다.
4. 스피커 방향별 기대 ILD 부호를 기준으로 마이크 오차를 추정합니다.
5. 추정값의 일관성을 검증합니다. 신뢰도가 낮으면 보정 강도를 절반으로 낮춥니다.
6. 크기 보정용 minimum-phase FIR을 만들고 좌우 IR에 적용합니다.

기본 분석 대역은 샘플레이트의 Nyquist 주파수를 넘지 않는 범위로 제한됩니다. 최대 보정량은 기본 6 dB입니다.

## v2 옵션에 대한 정리

예전 문서에는 위상 보정, adaptive 보정, anatomical validation이 v2.0 핵심 기능으로 설명되어 있었습니다. 현재 코드는 다릅니다.

| 예전 옵션 | 현재 동작 |
| --- | --- |
| `--no_mic_deviation_phase_correction` | 호환용 옵션입니다. v3.0은 위상을 직접 보정하지 않습니다. |
| `--no_mic_deviation_adaptive_correction` | 호환용 옵션입니다. v3.0은 교차검증으로 마이크 오차를 추정합니다. |
| `--no_mic_deviation_anatomical_validation` | 호환용 옵션입니다. v3.0은 스피커 방향별 기대 ILD 부호와 일관성 검증을 씁니다. |

이 옵션들은 CLI 호환성을 위해 남아 있지만, 현재 보정 결과를 바꾸지 않습니다.

## CLI 사용

기본 보정은 다음처럼 켭니다.

```bash
impulcifer --dir_path "measurements" --microphone_deviation_correction
```

보정 강도는 `0.0`부터 `1.0`까지 지정합니다.

```bash
impulcifer --dir_path "measurements" \
  --microphone_deviation_correction \
  --mic_deviation_strength 0.5
```

진단 플롯을 저장하려면 debug plot 옵션을 켭니다.

```bash
impulcifer --dir_path "measurements" \
  --microphone_deviation_correction \
  --mic_deviation_debug_plots
```

현재 CLI 옵션은 다음과 같습니다.

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--microphone_deviation_correction` | 꺼짐 | v3.0 교차검증 기반 보정을 켭니다. |
| `--mic_deviation_strength VALUE` | `0.7` | 보정 강도입니다. |
| `--mic_deviation_debug_plots` | 꺼짐 | `plots/microphone_deviation/` 아래에 진단 그래프를 저장합니다. |
| `--no_mic_deviation_phase_correction` | 호환용 | v3.0에서는 보정 결과를 바꾸지 않습니다. |
| `--no_mic_deviation_adaptive_correction` | 호환용 | v3.0에서는 보정 결과를 바꾸지 않습니다. |
| `--no_mic_deviation_anatomical_validation` | 호환용 | v3.0에서는 보정 결과를 바꾸지 않습니다. |

## GUI 사용

Stable GUI와 Studio GUI 모두 Advanced Options에서 마이크 착용 편차 보정을 켤 수 있습니다.

현재 GUI에서 조정하는 항목은 다음입니다.

| 항목 | 설명 |
| --- | --- |
| Mic Deviation Correction | 기능을 켭니다. |
| Strength | 보정 강도입니다. 기본값은 `0.7`입니다. |
| Debug plots | 진단 플롯 저장을 켭니다. |

v2 세부 옵션은 GUI에서 제거되어 있습니다.

## Python API

직접 호출할 때는 `core` 패키지 경로를 씁니다.

```python
from core.hrir import HRIR
from core.impulse_response_estimator import ImpulseResponseEstimator

estimator = ImpulseResponseEstimator.from_wav("test_signal.wav")
hrir = HRIR(estimator)
hrir.open_recording("measurements/FL,FR.wav", speakers=["FL", "FR"])

summary = hrir.correct_microphone_deviation(
    correction_strength=0.7,
    plot_analysis=True,
    plot_dir="measurements/plots",
)

print(summary["v3_cross_validation"])
print(summary.get("avg_error_db"))
```

`HRIR.correct_microphone_deviation()`은 기존 호출 코드와 맞추기 위해 v2 인자를 아직 받습니다. 새 코드에서는 `correction_strength`, `plot_analysis`, `plot_dir`만 의미가 있습니다.

## 파이프라인 위치

BRIR 생성 중 마이크 착용 편차 보정은 다음 순서로 실행됩니다.

1. 측정 파일을 열고 peak 기준으로 앞부분을 자릅니다.
2. ipsilateral alignment와 onset group alignment를 적용합니다.
3. 꼬리를 자릅니다.
4. Virtual Bass를 켰다면 먼저 적용합니다.
5. 마이크 착용 편차 보정을 적용합니다.
6. `responses.wav`를 저장합니다.
7. 룸 보정, 헤드폰 보정, Custom EQ, decay, channel balance, normalize를 진행합니다.

## 출력과 플롯

보정이 끝나면 콘솔에 처리된 스피커 수, 평균 보정량, 최대 보정량을 출력합니다.

`--mic_deviation_debug_plots`를 켠 경우에는 보통 다음 파일이 생성됩니다.

| 파일 | 설명 |
| --- | --- |
| `plots/microphone_deviation/microphone_deviation_cross_validation_v3.png` | 스피커별 좌우 편차와 추정된 마이크 오차를 보여줍니다. |
| `plots/microphone_deviation/microphone_deviation_analysis_v3.png` | 단일 스피커 fallback 경로에서 편차 분석을 보여줍니다. |
| `plots/microphone_deviation/microphone_deviation_correction_comparison_v2.png` | 단일 스피커 fallback 경로의 보정 전후 비교입니다. 파일명은 v2로 남아 있습니다. |

## 결과 딕셔너리

교차검증 경로에서는 요약 딕셔너리가 반환됩니다.

```python
{
    "mic_error_estimate": {250: 0.4, 500: 0.6},
    "avg_error_db": 0.5,
    "max_error_db": 0.8,
    "speakers_analyzed": ["FL", "FR", "FC"],
    "validation": {
        "consistency_score": 0.75,
        "is_valid": True,
        "confidence": "high",
    },
    "correction_strength": 0.7,
    "v3_cross_validation": True,
    "speakers_processed": ["FL", "FR", "FC"],
}
```

스피커가 2개 미만이면 단일 스피커 fallback으로 전환합니다. 이 경우 `v3_cross_validation`은 `False`이고, 수집된 편차를 직접 마이크 오차로 보고 보정합니다.

## 주의 사항

- 이 기능은 측정 오차를 줄이기 위한 보정입니다. 실제 HRTF 차이를 완전히 없애는 기능이 아닙니다.
- 여러 방향의 스피커 측정이 있을수록 교차검증이 안정적입니다.
- 보정 강도를 높이면 좌우 차이를 더 줄이지만, 실제 방향감까지 줄일 수 있습니다.
- 기본 측정 품질이 낮으면 보정 결과도 믿기 어렵습니다. 배경 소음, 클리핑, 잘못된 sweep 파일을 먼저 확인하세요.

## 테스트

관련 테스트는 다음 파일에 있습니다.

```bash
pytest tests/test_microphone_deviation.py tests/test_suite.py -q
```
