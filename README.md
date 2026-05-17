# Impulcifer-py313

[![PyPI version](https://badge.fury.io/py/impulcifer-py313.svg)](https://badge.fury.io/py/impulcifer-py313)

Impulcifer-py313은 [Jaakko Pasanen의 Impulcifer](https://github.com/jaakkopasanen/impulcifer)를 바탕으로 한 포크입니다. 스피커와 헤드폰 측정 파일에서 개인 BRIR WAV를 만들고, HeSuVi, JamesDSP, Hangloose Convolver 같은 컨볼버에서 쓸 수 있는 출력을 만듭니다.

이 포크는 원본 Impulcifer의 측정과 보정 흐름을 유지하면서, Python 3.13/3.14, PyPI 배포, standalone 빌드, Modern GUI에서 쓰기 쉽게 정리하는 데 초점을 둡니다. 세부 변경 내역은 [CHANGELOG.md](CHANGELOG.md)를 보세요.

## 지원 범위

- Python 3.9 이상에서 실행합니다. Python 3.13/3.14 경로를 계속 확인합니다.
- PyPI 패키지, standalone 릴리스, Modern GUI를 제공합니다.
- CLI와 GUI에서 BRIR 생성, 룸 보정, 헤드폰 보정, Custom EQ, Virtual Bass, TrueHD 레이아웃 출력, 마이크 착용 편차 보정을 다룹니다.
- 일반 Python에서는 process 기반 병렬 처리를, free-threaded Python에서는 thread 기반 병렬 처리를 우선 사용합니다. standalone 빌드는 free-threaded Python을 대상으로 하지 않습니다.

## 설치

### Python 패키지

가상 환경 안에 설치하는 방식을 권합니다.

```bash
python -m venv venv
```

Windows:

```bash
venv\Scripts\activate
pip install impulcifer-py313
```

macOS 또는 Linux:

```bash
source venv/bin/activate
pip install impulcifer-py313
```

`uv`를 쓴다면 다음처럼 설치할 수 있습니다.

```bash
uv pip install impulcifer-py313
```

### Standalone 릴리스

Python을 따로 설치하지 않고 쓰려면 [GitHub Releases](https://github.com/115dkk/Impulcifer-pip313/releases)에서 운영체제에 맞는 파일을 받으세요. 릴리스 파일 이름과 구성은 버전마다 달라질 수 있으므로, 각 릴리스의 설명을 확인해 주세요.

## 실행

GUI를 쓰려면 다음 명령을 실행합니다.

```bash
impulcifer_gui
```

CLI를 쓰려면 측정 폴더를 지정합니다.

```bash
impulcifer --dir_path "data/demo" --test_signal default --plot
```

사용 가능한 CLI 옵션은 다음 명령으로 확인할 수 있습니다.

```bash
impulcifer --help
```

## 입력 파일

`--dir_path`로 지정한 폴더에 측정 파일과 보정 파일을 둡니다.

| 파일 | 설명 |
| --- | --- |
| `FL,FR.wav`, `FC.wav`, `SL,SR.wav` 등 | 스피커 측정 파일입니다. 파일 이름의 스피커 이름을 보고 채널을 판단합니다. |
| `headphones.wav` | 기본 헤드폰 보정 측정 파일입니다. `--headphone_compensation_file`로 다른 파일을 지정할 수 있습니다. |
| `room-target.csv` | 룸 보정 목표 응답입니다. 없으면 flat target을 씁니다. |
| `room-mic-calibration.csv` 또는 `room-mic-calibration.txt` | 룸 측정 마이크 보정 파일입니다. 없으면 마이크 보정을 건너뜁니다. |
| `eq.csv`, `eq-left.csv`, `eq-right.csv` | Custom EQ 파일입니다. `eq.csv`는 양쪽 공통, `eq-left.csv`와 `eq-right.csv`는 좌우 개별 EQ입니다. |

Studio GUI에서 Custom EQ 파일을 다른 위치에서 고르면, 처리 전에 이 파일들이 측정 폴더의 `eq.csv`, `eq-left.csv`, `eq-right.csv`로 복사됩니다.

## CLI 옵션

### 입력과 파일

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--dir_path PATH` | 필수 | 측정 파일을 읽고 결과를 저장할 폴더입니다. |
| `--test_signal VALUE` | `test.pkl`, `test.wav`, 없으면 내장 `default` | 측정에 쓴 sweep WAV, estimator pickle, TrueHD/MLP 파일 또는 미리 정한 이름입니다. |
| `--room_target PATH` | `dir_path/room-target.csv` | 룸 보정 목표 응답 CSV입니다. 파일이 없으면 flat target을 씁니다. |
| `--room_mic_calibration PATH` | `dir_path/room-mic-calibration.csv`, 없으면 `.txt` | 룸 측정 마이크 보정 파일입니다. |
| `--headphone_compensation_file PATH` | `dir_path/headphones.wav` | 헤드폰 보정 측정 WAV입니다. 폴더를 주면 흔히 쓰는 파일명을 찾아봅니다. |
| `--fs HZ` | 측정 신호의 샘플레이트 | 출력 샘플레이트입니다. 지정하면 결과를 해당 샘플레이트로 맞춥니다. |

`--test_signal`에는 다음 약칭을 쓸 수 있습니다.

| 값 | 의미 |
| --- | --- |
| `default`, `1` | 내장 pickle sweep estimator입니다. |
| `sweep`, `2` | 내장 기본 sweep WAV입니다. |
| `stereo`, `3` | `FL,FR` 스테레오 분절 sweep입니다. |
| `mono-left`, `4` | `FL` 모노 분절 sweep입니다. |
| `left`, `5` | `FL` 스테레오 분절 sweep입니다. |
| `right`, `6` | `FR` 스테레오 분절 sweep입니다. |

### 보정과 목표 응답

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--channel_balance VALUE` | 사용 안 함 | 좌우 레벨이나 응답 차이를 보정합니다. `trend`, `left`, `right`, `avg`, `min`, `mids` 또는 dB 값을 받습니다. |
| `--decay VALUE` | 사용 안 함 | 잔향 꼬리를 줄입니다. `300`처럼 전체 ms 값을 주거나 `FL:500,FC:100`처럼 채널별 ms 값을 줄 수 있습니다. |
| `--target_level DB` | 사용 안 함 | 좌우 평균 레벨을 지정한 dB로 맞춥니다. 클리핑을 피하려면 보통 음수 값을 씁니다. |
| `--fr_combination_method average|conservative` | `average` | 여러 룸 측정 응답을 합치는 방식입니다. |
| `--specific_limit HZ` | `400` | speaker-ear specific 룸 보정의 상한 주파수입니다. `0`이면 제한을 끕니다. |
| `--generic_limit HZ` | `300` | generic 룸 보정의 상한 주파수입니다. `0`이면 제한을 끕니다. |
| `--bass_boost DB` | 사용 안 함 | 저역 shelf boost입니다. `6` 또는 `6,150,0.69`처럼 gain, Fc, Q를 줄 수 있습니다. |
| `--tilt DB_PER_OCT` | `0.0` | 목표 응답 기울기입니다. 양수는 밝게, 음수는 어둡게 맞춥니다. |
| `--no_room_correction` | 룸 보정 켜짐 | 룸 보정을 건너뜁니다. |
| `--no_headphone_compensation` | 헤드폰 보정 켜짐 | 헤드폰 보정을 건너뜁니다. |
| `--no_equalization` | EQ 켜짐 | Custom EQ를 건너뜁니다. |

### 출력과 진단

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--plot` | 꺼짐 | 처리 그래프를 PNG로 저장합니다. |
| `--interactive_plots` | 꺼짐 | Bokeh 기반 HTML 플롯을 저장합니다. |
| `--c MS` | `1.0` | IR 앞부분을 자를 때 남길 headroom입니다. 단위는 ms입니다. |
| `--jamesdsp` | 꺼짐 | `FL/FR` 기반의 `jamesdsp.wav`를 추가로 만듭니다. |
| `--hangloose` | 꺼짐 | Hangloose Convolver용 스피커별 stereo IR 파일을 만듭니다. |
| `--output_truehd_layouts` | 꺼짐 | TrueHD용 레이아웃 출력을 추가로 만듭니다. |
| `--info` | 꺼짐 | 버전, Python, 운영체제, 주요 의존성 정보를 출력하고 종료합니다. |
| `-V`, `--version` | 꺼짐 | Impulcifer 버전을 출력하고 종료합니다. |

### Virtual Bass

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--vbass` | 꺼짐 | Virtual Bass 합성을 켭니다. |
| `--vbass_freq HZ` | `250` | Virtual Bass crossover 주파수입니다. |
| `--vbass_hp HZ` | `15.0` | 합성 저역에 적용할 high-pass 주파수입니다. |
| `--vbass_polarity auto|normal|invert` | `auto` | 합성 저역 polarity 처리 방식입니다. |

### 마이크 착용 편차 보정

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--microphone_deviation_correction` | 꺼짐 | 좌우 귀 마이크 착용 차이를 보정합니다. |
| `--mic_deviation_strength VALUE` | `0.7` | 보정 강도입니다. `0.0`은 보정 없음, `1.0`은 전체 보정입니다. |
| `--no_mic_deviation_phase_correction` | phase 보정 켜짐 | phase 보정을 끕니다. |
| `--no_mic_deviation_adaptive_correction` | adaptive 보정 켜짐 | 좌우 비대칭 adaptive 보정을 끕니다. |
| `--no_mic_deviation_anatomical_validation` | anatomical 검증 켜짐 | ITD/ILD anatomical validation을 끕니다. |
| `--mic_deviation_debug_plots` | 꺼짐 | 마이크 착용 편차 보정 진단 그래프를 저장합니다. |

## CLI 예시

데모 폴더를 처리하고 그래프를 저장합니다.

```bash
impulcifer --dir_path "data/demo" --test_signal default --plot
```

룸 보정과 헤드폰 보정을 끄고 측정 IR만 정리합니다.

```bash
impulcifer --dir_path "measurements" --no_room_correction --no_headphone_compensation
```

Virtual Bass와 JamesDSP 출력을 함께 만듭니다.

```bash
impulcifer --dir_path "measurements" --vbass --vbass_freq 250 --jamesdsp
```

채널별 decay를 지정합니다.

```bash
impulcifer --dir_path "measurements" --decay "FL:500,FC:100,FR:500"
```

## GUI에서 할 수 있는 일

- Recorder에서 sweep 재생과 녹음을 진행합니다. 스피커 측정은 `FL,FR.wav` 같은 이름으로 저장하고, 헤드폰 보정은 별도 버튼으로 `headphones.wav`를 만듭니다.
- Impulcifer 탭에서 BRIR 생성 옵션을 지정하고 처리 중 취소할 수 있습니다.
- Studio skin에서는 같은 작업을 더 넓은 화면 구성으로 다룹니다.
- UI Settings에서 언어와 테마를 바꿀 수 있습니다.

각 옵션 위에 마우스를 올리면 짧은 설명을 확인할 수 있습니다.

## 추가 문서

- [TrueHD/MLP 지원 및 레이아웃 출력](README_TrueHD.md)
- [마이크 착용 편차 보정](README_microphone_deviation_correction.md)
- [Python 3.14 및 Nuitka 빌드 메모](README_PYTHON314.md)

## 주의 사항

- `.mlp`, `.thd`, `.truehd` 입력은 FFmpeg가 필요합니다. FFmpeg가 없으면 실행 중 설치 안내가 나올 수 있습니다.
- Custom EQ는 처리 시점에 측정 폴더의 `eq.csv`, `eq-left.csv`, `eq-right.csv`를 기준으로 읽습니다.
- 원본 Impulcifer와 같은 입력을 쓰더라도 Python, NumPy, SciPy, 보정 옵션 차이로 결과가 달라질 수 있습니다. 주요 경로는 회귀 테스트로 확인합니다.

## 업데이트

```bash
pip install --upgrade impulcifer-py313
```

## 라이선스

이 프로젝트는 MIT License를 따릅니다. 전체 문구는 [LICENSE](LICENSE)를 보세요.

저작권 표기는 `LICENSE`와 맞췄습니다.

- Copyright (c) 2018- Jaakko Pasanen
- Copyright (c) 2024- 115dkk
- Copyright (c) 2025- LionLion123
- Copyright (c) 2025- SDC (DCinside)

## 기여와 문의

버그를 찾았거나 개선할 점이 있으면 [이슈 트래커](https://github.com/115dkk/Impulcifer-pip313/issues)에 남겨 주세요.
