# 마이크 착용 편차 보정

마이크 착용 편차 보정은 좌우 귀 마이크의 삽입 깊이, 각도, 감도 차이 때문에 생기는 **방향과 무관한** 좌우 크기 차이를 줄이는 기능입니다. 현재 구현은 v4.0 양이(interaural) 불일치 보정입니다.

이 문서는 코드 기준으로 정리했습니다.

| 항목 | 기준 코드 |
| --- | --- |
| 보정 알고리즘 | `core/microphone_deviation_correction.py` |
| HRIR 통합 지점 | `core/hrir.py` |
| BRIR 파이프라인 호출 순서 | `impulcifer.py` |
| CLI 옵션 정의 | `core/pipeline.py` |
| GUI 인자 조립 | `gui/brir_args.py` |

## 핵심: 헤드폰 보상이 켜져 있으면 자동으로 건너뜁니다

같은 인이어 마이크를 같은 위치에 둔 채 스피커와 헤드폰을 모두 측정하고 헤드폰 보상을 적용하면, 마이크 전달함수 `M(f)`가 귀별로 분자·분모에 함께 들어가 소거됩니다(`out = HRTF/HpTF`). 즉 표준 워크플로(헤드폰 보상 ON, 기본값)에서는 마이크 좌우 차이가 이미 제거되므로 별도 보정이 불필요하며, 보상 이전에 보정을 적용하면 좌우 밸런스를 이중으로 건드립니다.

그래서 `impulcifer.py`는 **`do_headphone_compensation`이 켜져 있으면 마이크 보정을 건너뛰고 안내 로그(`cli_mic_deviation_skipped_hpcomp`)를 출력**합니다. 이 보정은 헤드폰 보상을 끈 경우, 또는 측정 사이에 마이크를 다시 착용해 소거가 깨진 경우를 위한 것입니다.

(근거: Hammershøi & Møller 2005; Møller 1992. 정면 ILD≈0 및 방향·주파수 의존 ILD: Cai/Rakerd/Hartmann 2015. 분수옥타브 평활: Tylka/Boren/Choueiri 2017. 최소위상+지연 근사와 위상 둔감성: Kistler & Wightman 1992, Kulkarni/Isabelle/Colburn 1999. 규제화: Kirkeby & Nelson 1999, Gomez Bolaños/Mäkivirta/Pulkki 2016.)

## 현재 구현 요약 (v4.0)

단일 스피커의 좌우 차이를 그대로 마이크 오차로 보지 않습니다. 오프센터 스피커의 좌우 차이는 대부분 실제 ILD(양이 레벨차)이며 방향·주파수에 따라 변하기 때문입니다. v4.0은 **방향 무관 성분**만 추정합니다.

처리 흐름은 다음과 같습니다.

1. 각 스피커의 좌우 IR에서 직접음을 짧은 창(기본 약 5 ms)으로 잘라 풀 FFT 크기응답을 구합니다.
2. 추정 기준(anchor)을 정합니다.
   - `auto`(기본): 정면(FC) 측정이 있으면 정면(`frontal`), 없으면 확산음장 평균(`diffuse`).
   - `frontal`: 정면 측정의 좌우 차이만 사용(기대 ILD≈0).
   - `diffuse`: 모든 방향의 파워 평균(CTF)으로 좌우 차이를 구함(대칭 레이아웃에서 방향성 ILD가 상쇄).
3. 방향 무관 좌우 크기 차이 Δ(f)를 구하고 분수옥타브(기본 1/6 octave)로 평활합니다.
4. 보정 대역(기본 200 Hz ~ 16 kHz) 밖은 raised-cosine으로 0이 되도록 테이퍼링하고, 최대 보정량으로 클램프합니다(노치 역전 방지).
5. 좌우를 ±Δ/2 최소위상 FIR로 보정합니다. 크기만 보정하므로 ITD(양이 지연)는 보존됩니다.

최대 보정량은 한쪽 귀 기준 기본 6 dB입니다.

## 이전 구현과의 차이

이전 구현(v3.0)은 스피커별 기대 ILD 부호표(FL=+1, FR=−1, FC=0 …)와 "기대와 반대 방향 편차의 중앙값" 휴리스틱으로 마이크 오차를 추정하고, 250 Hz~8 kHz의 6개 옥타브 점만 사용했습니다. 이는 실제 방향·주파수 의존 ILD를 단순화해 편향이 있었고, 3.7 kHz 이상의 협대역 개인차를 담기에 해상도가 부족했습니다. v4.0은 이를 방향 무관 추정 + 풀 FFT + 분수옥타브 평활로 교체했습니다.

v2.0 호환용으로 남아 있던 옵션 `--no_mic_deviation_phase_correction`, `--no_mic_deviation_adaptive_correction`, `--no_mic_deviation_anatomical_validation`은 **제거**되었습니다(이미 동작에 영향이 없는 no-op였습니다).

## CLI 사용

기본 보정은 다음처럼 켭니다. 단, 헤드폰 보상이 켜져 있으면 자동으로 건너뜁니다.

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
| `--microphone_deviation_correction` | 꺼짐 | v4.0 양이 불일치 보정을 켭니다. 헤드폰 보상이 켜져 있으면 자동 생략됩니다. |
| `--mic_deviation_strength VALUE` | `0.7` | 보정 강도입니다. `0.0`은 보정 없음, `1.0`은 전체 보정입니다. |
| `--mic_deviation_debug_plots` | 꺼짐 | `plots/microphone_deviation/` 아래에 진단 그래프를 저장합니다. |

## GUI 사용

Stable GUI와 Studio GUI 모두 Advanced Options에서 마이크 착용 편차 보정을 켤 수 있습니다.

| 항목 | 설명 |
| --- | --- |
| Mic Deviation Correction | 기능을 켭니다. |
| Strength | 보정 강도입니다. 기본값은 `0.7`입니다. |
| Debug plots | 진단 플롯 저장을 켭니다. |

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
    anchor="auto",
    plot_analysis=True,
    plot_dir="measurements/plots",
)

print(summary["anchor"])         # 'frontal' 또는 'diffuse'
print(summary.get("avg_error_db"))
```

`HRIR.correct_microphone_deviation()`의 인자는 `correction_strength`, `anchor`, `plot_analysis`, `plot_dir`입니다.

## 파이프라인 위치

BRIR 생성 중 마이크 착용 편차 보정은 다음 순서로 실행됩니다.

1. 측정 파일을 열고 peak 기준으로 앞부분을 자릅니다.
2. ipsilateral alignment와 onset group alignment를 적용합니다.
3. 꼬리를 자릅니다.
4. Virtual Bass를 켰다면 먼저 적용합니다.
5. 마이크 착용 편차 보정을 적용합니다. **(헤드폰 보상이 켜져 있으면 건너뜀)**
6. `responses.wav`를 저장합니다.
7. 룸 보정, 헤드폰 보정, Custom EQ, decay, channel balance, normalize를 진행합니다.

## 결과 딕셔너리

```python
{
    "method": "interaural_v4",
    "anchor": "frontal",            # 또는 "diffuse"
    "avg_error_db": 0.5,
    "max_error_db": 0.8,
    "speakers_analyzed": ["FL", "FR", "FC"],
    "correction_strength": 0.7,
    "speakers_processed": ["FL", "FR", "FC"],
}
```

유의미한 좌우 불일치가 없으면 보정을 건너뛰고 `speakers_processed`는 빈 리스트가 됩니다.

## 주의 사항

- 이 기능은 측정 오차(마이크 좌우 불일치)를 줄이기 위한 보정입니다. 실제 HRTF 방향 차이를 없애는 기능이 아닙니다.
- 방향 무관 성분과 피험자의 해부학적 좌우 비대칭을 완벽히 분리할 수는 없으므로, 보정 강도를 너무 높이면 실제 방향감을 줄일 수 있습니다.
- 대칭 레이아웃(FL/FR, SL/SR …) 측정이 많을수록 확산음장 추정이 안정적입니다.
- 기본 측정 품질이 낮으면 보정 결과도 믿기 어렵습니다. 배경 소음, 클리핑, 잘못된 sweep 파일을 먼저 확인하세요.

## 테스트

관련 테스트는 다음 파일에 있습니다.

```bash
pytest tests/test_microphone_deviation.py tests/test_suite.py tests/test_integration.py -q
```
