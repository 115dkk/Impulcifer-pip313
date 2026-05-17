# TrueHD/MLP 입력과 레이아웃 출력

이 문서는 `.mlp`, `.thd`, `.truehd` 파일을 측정 신호로 쓰는 경우와 BRIR 결과를 TrueHD용 채널 순서로 내보내는 경우를 다룹니다.

현재 구현 기준은 다음 파일입니다.

| 항목 | 기준 코드 |
| --- | --- |
| TrueHD/MLP 판별과 WAV 변환 | `core/ffmpeg_utils.py` |
| 녹음 CLI | `core/recorder.py` |
| BRIR 처리 CLI | `impulcifer.py`, `core/pipeline.py` |
| TrueHD 출력 채널 순서 | `core/constants.py`, `core/channel_generation.py` |

## 지원하는 입력

`--test_signal`과 Recorder의 재생 파일에는 다음 확장자를 쓸 수 있습니다.

| 확장자 | 설명 |
| --- | --- |
| `.wav` | 일반 sweep WAV입니다. |
| `.pkl` | ImpulseResponseEstimator pickle입니다. BRIR 처리에서만 씁니다. |
| `.mlp`, `.thd`, `.truehd` | FFmpeg로 임시 WAV로 변환한 뒤 처리합니다. |

TrueHD/MLP 파일을 실제로 열 때 FFmpeg가 필요합니다. 코드는 FFmpeg와 ffprobe를 늦게 확인하므로, 일반 WAV만 처리할 때는 FFmpeg 확인을 하지 않습니다.

## 중요한 제한

Dolby TrueHD + Dolby Atmos 오브젝트 마스터는 discrete height/wide sweep으로 쓸 수 없습니다. FFmpeg가 보통 7.1 bed만 디코드하고 오브젝트 채널은 일반 오디오 인터페이스의 개별 출력으로 복원하지 못하기 때문입니다.

이 경우 Recorder는 에러를 내고 중단합니다. 7.1.6 같은 다채널 측정이 필요하면 Impulcifer의 sweep 세트 생성 기능으로 WAV sweep을 만들어 쓰세요.

일반 Dolby TrueHD 5.1/7.1 파일은 오브젝트 마스터가 아니면 거부하지 않습니다. 다만 현재 `CHANNEL_LAYOUT_MAP`은 11채널과 13채널 커스텀 순서만 이름으로 매핑합니다. 보통의 5.1/7.1 TrueHD는 PCM 채널 수로 재생되지만 `_channels.txt`에 커스텀 채널명이 저장되지 않을 수 있습니다.

## GUI 사용

Recorder에서 재생 파일로 `.mlp`, `.thd`, `.truehd`를 고를 수 있습니다. Modern GUI는 녹음 파일명을 직접 쓰지 않고 녹음 폴더를 받습니다. 재생 파일 이름에 들어 있는 스피커 segment를 보고 `FL,FR.wav` 같은 이름으로 저장합니다.

Impulcifer 탭에서는 `Test signal used`에 같은 TrueHD/MLP 파일을 지정합니다. TrueHD 레이아웃 WAV를 추가로 만들려면 Advanced Options에서 `TrueHD layouts` 옵션을 켭니다.

헤드폰 보정 녹음에는 TrueHD/MLP 다채널 파일을 쓰지 마세요. 헤드폰 보정은 전용 버튼으로 `headphones.wav`를 만들고, 모노 또는 스테레오 sweep만 받습니다.

## CLI 사용

Recorder CLI는 직접 저장할 WAV 경로를 받습니다.

```bash
python -m core.recorder --play "test.thd" --record "data/my_hrir/FL,FR,SL,SR.wav"
```

BRIR 생성은 `impulcifer` 명령을 씁니다.

```bash
impulcifer --dir_path "data/my_hrir" --test_signal "test.thd"
```

TrueHD 레이아웃 출력을 추가하려면 현재 옵션명인 `--output_truehd_layouts`를 씁니다.

```bash
impulcifer --dir_path "data/my_hrir" --test_signal "test.thd" --output_truehd_layouts
```

## 출력 파일

기본 출력은 TrueHD 옵션과 관계없이 생성됩니다.

| 파일 | 설명 |
| --- | --- |
| `responses.wav` | 보정 전 응답 확인용 WAV입니다. |
| `hrir.wav` | 기본 BRIR 출력입니다. |
| `hesuvi.wav` | HeSuVi 채널 순서 출력입니다. |

`--output_truehd_layouts`를 켜면 다음 파일을 추가로 만듭니다.

| 파일 | 채널 순서 | 생성 조건 |
| --- | --- | --- |
| `truehd_11ch_Nch.wav` | `FL FR FC BL BR SL SR TFL TFR TBL TBR` | 이 순서에 속한 측정 채널이 8개 이상 있을 때 생성합니다. |
| `truehd_13ch_Nch.wav` | `FL FR FC BL BR SL SR TFL TFR TSL TSR TBL TBR` | 이 순서에 속한 측정 채널이 10개 이상 있을 때 생성합니다. |

`N`은 실제로 들어간 스피커 수입니다. 각 스피커는 `speaker-left`, `speaker-right` 순서의 두 트랙으로 기록됩니다.

현재 코드는 누락 채널을 자동 생성하지 않습니다. 필요한 채널이 부족하면 경고를 출력하고 해당 TrueHD 레이아웃 파일을 만들지 않습니다.

## 채널 정보 파일

Recorder가 TrueHD/MLP 파일에서 11채널 또는 13채널 커스텀 레이아웃을 확인하면, 녹음 WAV 옆에 `*_channels.txt`를 저장합니다. 이 파일에는 변환된 채널 이름이 쉼표로 기록됩니다.

일반 5.1/7.1 TrueHD처럼 커스텀 레이아웃 매핑이 없는 파일은 이 정보 파일이 생기지 않을 수 있습니다.

## 문제 해결

| 증상 | 확인할 점 |
| --- | --- |
| FFmpeg 관련 오류가 납니다. | FFmpeg와 ffprobe가 PATH에 있는지 확인하세요. TrueHD/MLP를 쓰는 순간에만 확인합니다. |
| Atmos TrueHD 파일이 거부됩니다. | 오브젝트 마스터는 discrete sweep으로 쓸 수 없습니다. Impulcifer에서 생성한 다채널 WAV sweep을 쓰세요. |
| TrueHD 레이아웃 파일이 생성되지 않습니다. | 11채널 출력은 최소 8개, 13채널 출력은 최소 10개의 해당 스피커 측정이 필요합니다. |
| 채널이 잘린 것 같습니다. | 오디오 출력 장치의 최대 출력 채널 수가 재생 파일 채널 수보다 작은지 확인하세요. |
