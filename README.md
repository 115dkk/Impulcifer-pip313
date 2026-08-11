# Impulcifer-py313

[![PyPI version](https://badge.fury.io/py/impulcifer-py313.svg)](https://badge.fury.io/py/impulcifer-py313)

Impulcifer-py313은 [Jaakko Pasanen의 Impulcifer](https://github.com/jaakkopasanen/impulcifer)를 바탕으로 한 포크입니다. 스피커와 헤드폰 측정 파일에서 개인 BRIR WAV를 만들고, HeSuVi, JamesDSP, Hangloose Convolver 같은 컨볼버에서 쓸 수 있는 출력을 만듭니다.

이 포크는 원본 Impulcifer의 측정과 보정 흐름을 유지하면서, Python 3.13/3.14, PyPI 배포, standalone 빌드, Modern GUI에서 쓰기 쉽게 정리하는 데 초점을 둡니다. 세부 변경 내역은 [CHANGELOG.md](CHANGELOG.md)를 보세요.

## 지원 범위

- Python 3.9 이상에서 실행합니다. Python 3.13/3.14 경로를 계속 확인합니다.
- PyPI 패키지, standalone 릴리스, Modern GUI를 제공합니다.
- CLI와 GUI에서 BRIR 생성, 룸 보정, 헤드폰 보정, Custom EQ, Virtual Bass, TrueHD 레이아웃 출력, 마이크 착용 편차 보정을 다룹니다.
- 일반 Python에서는 process 기반 병렬 처리를, free-threaded Python에서는 thread 기반 병렬 처리를 우선 사용합니다. standalone 빌드는 free-threaded Python을 대상으로 하지 않습니다.
- free-threaded 런타임은 CPython 3.14.4 이상(가능하면 최신 패치)을 권합니다. 3.14.1~3.14.4 패치에 free-threaded GC 일시정지 증가, GC 성능 회귀, mimalloc 메모리 누수 수정이 순차 반영되었습니다. CI는 free-threaded 3.14t에서도 전체 테스트를 확인합니다.

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

WebView 프론트엔드(2.10부터 기본 인터페이스)를 pip 환경에서 쓰려면 선택적 extra를 설치합니다. 플랫폼별로 Windows는 Microsoft Edge WebView2, macOS는 WKWebView(Cocoa), Linux는 WebKit2GTK를 사용하며 Qt backend로 fallback하지 않습니다.

```bash
pip install "impulcifer-py313[webview]"
```

Linux에서는 PyGObject 소스 빌드를 위해 시스템 패키지가 먼저 필요합니다 (Debian/Ubuntu 기준):

```bash
sudo apt-get install -y gir1.2-gtk-3.0 gir1.2-webkit2-4.1 \
  libgirepository1.0-dev libgirepository-2.0-dev libcairo2-dev pkg-config gcc python3-dev
```

### Standalone 릴리스

Python을 따로 설치하지 않고 쓰려면 [GitHub Releases](https://github.com/115dkk/Impulcifer-pip313/releases)에서 운영체제에 맞는 파일을 받으세요. 릴리스 파일 이름과 구성은 버전마다 달라질 수 있으므로, 각 릴리스의 설명을 확인해 주세요.

### Arch Linux (AUR)

Arch 계열 배포판에서는 AUR의 [`impulcifer-py313-bin`](https://aur.archlinux.org/packages/impulcifer-py313-bin) 패키지로 설치할 수 있습니다. 릴리스 tarball 기반 바이너리 패키지이며 새 릴리스마다 자동으로 갱신됩니다.

```bash
yay -S impulcifer-py313-bin
```

WebView UI를 쓰려면 `webkit2gtk-4.1`을 함께 설치하세요 (없으면 CustomTkinter UI로 폴백).

## 실행

GUI를 쓰려면 다음 명령을 실행합니다.

```bash
impulcifer_gui
```

WebView 프론트엔드는 다음 명령으로 실행합니다 (Windows/macOS/Linux). Pulse 디자인의 Studio/Stable 스킨, Recorder / Processing / Output Recovery / Settings / Info 탭, CustomTkinter GUI와 동등한 BRIR 옵션 전체(가상 저음, decay, channel balance, 마이크 편차 보정 등), 네이티브 파일·폴더 선택, 자동 업데이트, 9개 언어와 dark/light/system 테마를 제공합니다.

```bash
impulcifer_webview
```

Standalone 릴리스(2.10+)의 기본 인터페이스는 WebView입니다. CustomTkinter 인터페이스도 계속 함께 설치되며, 설정 탭의 "기본 인터페이스" 선택이나 실행 인자 `--frontend=ctk`로 전환할 수 있습니다 (`--frontend=webview`로 되돌리기). WebView 스택을 사용할 수 없는 환경에서는 자동으로 CustomTkinter로 폴백합니다.

> **CustomTkinter 지원 안내**: CustomTkinter 인터페이스는 버전 2 동안 유지보수와 기능 추가를 포함해 계속 완전히 지원됩니다. 버전 3부터는 제거되지 않고 지금의 레거시 GUI처럼 업데이트 없이 동결 상태로 유지됩니다 — 버전 3에서 제거되는 것은 구버전 레거시 GUI(`impulcifer_gui_legacy`)입니다.

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
| `eq.csv`, `eq-left.csv`, `eq-right.csv` | Custom EQ 파일입니다. `eq.csv`는 양쪽 공통, `eq-left.csv`와 `eq-right.csv`는 좌우 개별 EQ입니다. 같은 이름의 `.txt`(예: `eq.txt`)도 인식합니다. |

Custom EQ 파일은 두 가지 형식을 지원합니다. 확장자가 아니라 내용으로 형식을 판별합니다.

- **AutoEQ 결과 CSV**: 기존과 동일한 `frequency,raw,error,...` 형식입니다. error 열이 없는 평문 2열(`주파수 게인`) 파일은 값을 그대로 적용할 EQ 게인 곡선으로 해석합니다.
- **EqualizerAPO(-XT) 설정 텍스트**: `Preamp:`, `Filter n: ON PK Fc ... Hz Gain ... dB Q ...`, `GraphicEQ:` 형식입니다. AutoEQ의 ParametricEQ.txt/GraphicEQ.txt 내보내기와 EqualizerAPO-XT에서 저장한 설정을 그대로 쓸 수 있습니다. 크기 응답으로 표현 가능한 명령(Filter 바이쿼드/IIR, Preamp, GraphicEQ, `Convolution`)은 적용하고, 그럴 수 없는 명령(`Copy`, `Delay`, `MultiConvolution`, VSTPlugin 등)은 경고와 함께 바이패스합니다. `Convolution:`은 IR 파일의 크기 응답만 반영하며(위상 제외) EqualizerAPO처럼 샘플레이트가 다르면 적용하지 않습니다. `Channel: L`/`Channel: R` 스코핑은 좌/우 EQ 곡선으로 분리 적용되고, `Include:`는 같은 폴더 기준 상대 경로면 따라 들어가며, `If: sampleRate == 48000` 같은 단순 샘플레이트 조건 분기는 평가됩니다(그 외 조건식 블록은 보수적으로 바이패스).

Studio GUI에서 Custom EQ 파일을 다른 위치에서 고르면, 처리 전에 이 파일들이 측정 폴더의 `eq.csv`, `eq-left.csv`, `eq-right.csv`로 복사됩니다.

## CLI 옵션

### 입력과 파일

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--dir_path PATH` | 필수 | 측정 파일을 읽고 결과를 저장할 폴더입니다. |
| `--test_signal VALUE` | 자동 감지 (`test.wav` → 녹음 분석 → 내장 `default`) | 측정에 쓴 sweep WAV, TrueHD/MLP 파일, 미리 정한 이름, `auto`(녹음에서 스윕 파라미터 자동 복원) 또는 `generate:<길이>s@<샘플레이트>`(예: `generate:6.15s@48000`, 파라미터로 직접 생성)입니다. |
| `--room_target PATH` | `dir_path/room-target.csv` | 룸 보정 목표 응답 CSV입니다. 파일이 없으면 flat target을 씁니다. |
| `--room_mic_calibration PATH` | `dir_path/room-mic-calibration.csv`, 없으면 `.txt` | 룸 측정 마이크 보정 파일입니다. |
| `--headphone_compensation_file PATH` | `dir_path/headphones.wav` | 헤드폰 보정 측정 WAV입니다. 폴더를 주면 흔히 쓰는 파일명을 찾아봅니다. |
| `--fs HZ` | 측정 신호의 샘플레이트 | 출력 샘플레이트입니다. 지정하면 결과를 해당 샘플레이트로 맞춥니다. |

`--test_signal`에는 다음 약칭을 쓸 수 있습니다.

| 값 | 의미 |
| --- | --- |
| `auto` | 폴더의 `test.wav` → 녹음 파일 분석(스윕 길이 그리드 복원) → 내장 기본 순으로 해석합니다. 미지정 시 기본 동작과 같습니다. |
| `generate:<길이>s@<fs>` | 파라미터로 sweep을 직접 생성합니다. 길이는 생성기 그리드에 스냅됩니다. |
| `default`, `1`, `sweep`, `2` | 내장 기본 sweep WAV입니다. |
| `stereo`, `3` | `FL,FR` 스테레오 분절 sweep입니다. |
| `mono-left`, `4` | `FL` 모노 분절 sweep입니다. |
| `left`, `5` | `FL` 스테레오 분절 sweep입니다. |
| `right`, `6` | `FR` 스테레오 분절 sweep입니다. |

스윕 파일(.pkl estimator 포함)을 반드시 준비할 필요는 없습니다. 2.11부터 레코더는 기본적으로 sweep을 즉석에서 생성해 재생하며(내장 파일과 비트 단위 동일), 커스텀 파라미터로 녹음하면 `test.wav`가 녹음 폴더에 자동 저장되어 BRIR 처리에서 그대로 인식됩니다. 레거시 `.pkl` estimator 입력은 제거되었습니다.

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

방향과 무관한 좌우 마이크 불일치(착용·감도)를 보정합니다(v4.0). 헤드폰 보상을 같은 마이크로 측정하면 마이크 응답이 보상 단계에서 이미 소거되므로, **헤드폰 보상이 켜져 있으면 이 보정은 자동으로 생략**됩니다. 자세한 내용은 [마이크 착용 편차 보정](docs/README_microphone_deviation_correction.md) 문서를 참고하세요.

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--microphone_deviation_correction` | 꺼짐 | 좌우 마이크 불일치를 보정합니다. 헤드폰 보상이 켜져 있으면 생략됩니다. |
| `--mic_deviation_strength VALUE` | `0.7` | 보정 강도입니다. `0.0`은 보정 없음, `1.0`은 전체 보정입니다. |
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

- Recorder에서 sweep 재생과 녹음을 진행합니다. 기본은 파일 없이 sweep을 즉석 생성해 재생하는 방식이며(스피커 목록·레이아웃 선택, 커스텀 모드에서 샘플레이트/길이 지정 — mono/stereo/5.1/7.1/7.1.4/7.1.6 지원), 특수한 녹음을 위해 파일 재생 모드도 유지됩니다. 스피커 측정은 `FL,FR.wav` 같은 이름으로 저장하고, 헤드폰 보정은 별도 버튼으로 `headphones.wav`를 만듭니다.
- Impulcifer 탭에서 BRIR 생성 옵션을 지정하고 처리 중 취소할 수 있습니다. 테스트 신호는 기본적으로 녹음에서 자동 감지되며, "폴더 분석" 버튼으로 감지 결과(샘플레이트/스윕 길이/신뢰도)를 미리 확인할 수 있습니다.
- Output Recovery(출력 복원) 탭은 DSP를 다시 실행하지 않고 남아 있는 출력에서 누락된 형식을 복원합니다. `Hangloose`의 스피커별 WAV만 남았다면 정해진 채널 순서로 `hrir.wav`와 `hesuvi.wav`를 모두 재조립하고, 둘 중 하나만 남았다면 다른 하나를 복원합니다. `hrir.wav` 또는 `hesuvi.wav`에서 스피커별 Hangloose 파일을 함께 만드는 옵션도 제공합니다. 출력 루트, 그 안의 `Hangloose` 폴더, 또는 분할 WAV가 바로 들어 있는 폴더를 선택할 수 있으며, 기존 파일은 검증 후 보존하고 덮어쓰지 않습니다.
- Studio skin에서는 같은 작업을 더 넓은 화면 구성으로 다룹니다.
- UI Settings에서 언어와 테마를 바꿀 수 있습니다.

각 옵션 위에 마우스를 올리면 짧은 설명을 확인할 수 있습니다.

## 추가 문서

- [TrueHD/MLP 지원 및 레이아웃 출력](docs/README_TrueHD.md)
- [마이크 착용 편차 보정](docs/README_microphone_deviation_correction.md)
- [Python 3.14 및 Nuitka 빌드 메모](docs/README_PYTHON314.md)
- [빌드 가이드 (Nuitka standalone)](docs/BUILD_README.md)
- [성능 최적화 요약](docs/OPTIMIZATION_SUMMARY.md)

## 주의 사항

- `.mlp`, `.thd`, `.truehd` 입력은 FFmpeg가 필요합니다. FFmpeg가 없으면 실행 중 설치 안내가 나올 수 있습니다.
- Custom EQ는 처리 시점에 측정 폴더의 `eq.csv`, `eq-left.csv`, `eq-right.csv`(없으면 같은 이름의 `.txt`)를 기준으로 읽습니다. AutoEQ CSV와 EqualizerAPO 설정 텍스트를 모두 인식합니다.
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
