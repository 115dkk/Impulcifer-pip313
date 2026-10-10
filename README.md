# Impulcifer-py313

[![PyPI version](https://badge.fury.io/py/impulcifer-py313.svg)](https://badge.fury.io/py/impulcifer-py313)

Impulcifer-py313은 [Jaakko Pasanen의 Impulcifer](https://github.com/jaakkopasanen/impulcifer)를 바탕으로 한 포크입니다. 귀에 넣은 바이노럴 마이크로 스피커 소리를 녹음하고, 그 녹음으로 헤드폰에서 스피커를 재현하는 개인 BRIR WAV를 만듭니다. 만든 파일은 HeSuVi(Equalizer APO), JamesDSP, Hangloose Convolver 같은 컨볼버에 넣어 쓸 수 있습니다.

지금 정식 버전은 3.x입니다. 2.x까지는 이 처리를 Python과 NumPy·SciPy·Matplotlib으로 했고, 3.x는 같은 처리를 컴파일 언어로 다시 써서 실행 파일 하나로 돌립니다. 화면은 운영체제에 들어 있는 웹뷰(Windows의 WebView2, macOS의 WKWebView)로 그립니다. 그래서 앱에는 Python 런타임이 들어가지 않고, Windows·macOS 앱에는 브라우저 엔진도 들어가지 않습니다. 그 결과 Windows 설치 파일이 209 MiB에서 26 MiB로, 데모 데이터 처리 시간이 5.6초에서 1.9초로 줄었습니다. 설치 크기, 처리 시간, 메모리를 2.x와 나란히 잰 표는 [2.x와 3.x 측정값](docs/2x-vs-3x.md)에 있습니다. 처리 결과는 단계마다 2.x의 출력과 대조해 허용 오차 안에서 맞췄습니다.

PyPI 패키지 `impulcifer-py313`도 3.x로 계속 올라갑니다. 3.x 패키지에는 CLI와 Python API가 들어 있고, 따로 설치되는 의존 패키지는 없습니다. 2.x(Python 판)는 유지보수 라인으로 남아 있습니다. 변경 내역은 [CHANGELOG.md](CHANGELOG.md)에 있습니다.

## 지원 환경

| 형태 | 플랫폼 | 필요한 것 |
| --- | --- | --- |
| 앱 | Windows 10/11 x64 | Microsoft Edge WebView2 런타임이 있어야 합니다(Windows 11에는 기본으로 들어 있습니다). |
| 앱 | macOS (Apple Silicon) | Intel Mac에서는 앱을 쓸 수 없습니다. |
| 앱 | Linux x86_64 (AppImage) | FUSE 2(`libfuse2`)가 있어야 합니다. Ubuntu 22.04에서 빌드하므로 그보다 오래된 배포판에서는 실행되지 않을 수 있습니다. |
| PyPI 패키지 | Python 3.9 이상 | Windows x64, macOS(Apple Silicon), Linux x86_64(glibc 2.28 이상)에서는 미리 빌드한 휠을 받습니다. |

PyPI 휠이 없는 플랫폼(Intel Mac, Linux ARM 등)에서는 pip가 소스 배포본을 받아 직접 빌드하므로 Rust 1.97 이상이 있어야 합니다([rustup](https://rustup.rs)으로 설치). Rust가 아예 없으면 빌드 도구(maturin)가 임시로 설치해 씁니다.

## 설치

### 앱

[GitHub Releases의 최신 릴리스](https://github.com/115dkk/Impulcifer-pip313/releases/latest)에서 운영체제에 맞는 파일을 받아야 합니다.

| 운영체제 | 파일 |
| --- | --- |
| Windows | `Impulcifer-win-Setup.exe`(설치) 또는 `Impulcifer-win-Portable.zip`(압축을 풀어 바로 실행) |
| macOS | `Impulcifer-<버전>-macOS.dmg` |
| Linux | `Impulcifer-<버전>-x86_64.AppImage` |

Linux에서는 받은 AppImage에 실행 권한을 줘야 실행할 수 있습니다.

```bash
chmod +x Impulcifer-*-x86_64.AppImage
./Impulcifer-*-x86_64.AppImage
```

### PyPI 패키지 (CLI와 Python API)

가상 환경 안에 설치하기를 권합니다.

```bash
python -m venv venv
source venv/bin/activate        # Windows에서는 venv\Scripts\activate
pip install impulcifer-py313
```

`uv`를 쓴다면 `uv pip install impulcifer-py313`으로 설치할 수 있습니다. 설치하면 `impulcifer` 명령과 Python API(`import impulcifer`)를 쓸 수 있고, `impulcifer_gui`로 앱 화면을 띄울 수 있습니다. `impulcifer_gui`는 처음 실행할 때 같은 버전의 앱(Windows 19 MiB, macOS 17 MiB, Linux 93 MiB)을 GitHub Releases에서 받아 릴리스의 SHA256SUMS.txt로 확인한 뒤 사용자 캐시에 두고 실행합니다. 그래서 처음 한 번은 인터넷에 연결돼 있어야 하고, 그다음부터는 받은 앱을 바로 실행합니다. 받은 앱은 릴리스 앱과 같은 프로그램이므로 녹음도 할 수 있습니다.

### 2.x를 계속 쓰려면

2.x는 PyPI와 [v2.14.3 릴리스](https://github.com/115dkk/Impulcifer-pip313/releases/tag/v2.14.3)에 남아 있습니다. pip로 2.x를 설치하려면 버전을 3 미만으로 고정해야 합니다. 2.x PyPI 패키지는 Python 3.9~3.14를 지원하고, `impulcifer_gui`(CustomTkinter 화면)와 `impulcifer_webview`(웹뷰 화면) 명령을 함께 설치합니다.

```bash
pip install "impulcifer-py313<3"
```

2.x 웹뷰 화면을 pip 환경에서 쓰려면 `pip install "impulcifer-py313[webview]<3"`로 설치해야 합니다. Linux에서는 그 전에 WebKitGTK와 PyGObject 빌드용 시스템 패키지를 설치해야 합니다(Debian/Ubuntu 기준).

```bash
sudo apt-get install -y gir1.2-gtk-3.0 gir1.2-webkit2-4.1 \
  libgirepository1.0-dev libgirepository-2.0-dev libcairo2-dev pkg-config gcc python3-dev
```

Arch 계열 배포판의 AUR 패키지 [`impulcifer-py313-bin`](https://aur.archlinux.org/packages/impulcifer-py313-bin)으로는 2.x만 설치할 수 있습니다. 3.x를 쓰려면 AppImage를 받아야 합니다.

## 업데이트

앱은 실행 중에 새 버전을 확인하고, 사용자가 허락하면 내려받아 설치합니다. Windows는 Velopack으로, macOS와 Linux는 서명을 확인한 업데이트 파일로 설치합니다. 2.x 앱에서 업데이트를 확인해도 3.x가 새 버전으로 표시됩니다.

PyPI 패키지는 다음 명령으로 갱신할 수 있습니다.

```bash
pip install --upgrade impulcifer-py313
```

pip로 설치한 2.x를 이 명령으로 갱신하면 3.x로 올라갑니다. 그 뒤로 `impulcifer_gui`는 3.x 앱을 띄우고, `impulcifer_webview`와 `impulcifer_gui_legacy` 명령은 사라집니다. 2.x 화면을 계속 쓰려면 위의 `"impulcifer-py313<3"`로 고정해야 합니다. pip 패키지를 갱신하면 `impulcifer_gui`는 다음 실행 때 새 버전의 앱을 받습니다.

## 사용법

### 앱

설치한 앱을 실행하거나, pip로 설치했다면 `impulcifer_gui`를 실행해야 합니다. 앱은 다섯 메뉴로 나뉩니다.

* **녹음:** 스피커마다 sweep을 재생하고 바이노럴 마이크로 녹음합니다. 기본으로는 sweep 파일 없이 sweep을 즉석에서 만들어 재생하고, 스피커 순서와 트랙 레이아웃(mono, stereo, 5.1, 7.1, 7.1.4, 7.1.6)을 고를 수 있습니다. 특수한 녹음에는 sweep 파일을 재생하는 방식을 쓸 수 있습니다. 스피커 녹음은 `FL,FR.wav` 같은 이름으로, 헤드폰 보정 녹음은 `headphones.wav`로 저장합니다.
* **처리:** 녹음 폴더에서 BRIR을 만듭니다. 테스트 신호는 녹음에서 자동으로 알아내며, '폴더 분석' 버튼으로 알아낸 샘플레이트·sweep 길이·신뢰도를 처리 전에 볼 수 있습니다. 처리 중에는 취소할 수 있습니다.
* **출력 복원:** 남아 있는 출력 파일로 빠진 형식을 다시 만듭니다. 아래 '출력 파일'에 자세히 적었습니다.
* **설정:** 언어(9개), 테마(다크, 라이트, 시스템), 레이아웃 프리셋을 고를 수 있습니다. Studio는 고급 옵션을 음색·레벨, 시간 응답, 출력 파일, 보정·그래프 네 탭으로 나눠 탭마다 켜고 끄며, Stable은 2.x의 CustomTkinter 화면처럼 한 목록으로 보여 줍니다.
* **정보:** 버전, 시스템 정보, 프로젝트 링크를 보여 줍니다.

옵션 위에 마우스를 올리면 짧은 설명이 나옵니다.

### CLI

`--dir_path`로 측정 폴더를 지정해야 합니다. 옵션은 2.x와 같습니다. 저장소의 `data/demo`에는 바로 처리해 볼 수 있는 데모 녹음이 있습니다.

```bash
impulcifer --dir_path "data/demo" --plot
```

전체 옵션은 `impulcifer --help`로 확인할 수 있습니다.

### Python API

```python
import impulcifer
from impulcifer import impulcifer_native as native

# 처리 옵션을 CLI와 같은 이름의 키워드 인자로 넘기면 hesuvi.wav의 경로를 돌려받습니다.
path = impulcifer.main(dir_path="measurements", vbass=True)

# 진행률과 로그가 필요하면 native.run에 콜백을 넘겨야 합니다.
native.run(
    {"dir_path": "measurements"},
    progress=lambda event: print(event["progress"], event["message"]),
)
```

`decay`를 숫자나 스피커별 딕셔너리로 넘길 때는 초 단위로 줘야 합니다(CLI는 밀리초). 이 밖에 `impulcifer.detect_sweep(폴더)`, `impulcifer.generate_sweep_set(폴더)`, `impulcifer.recover_brir_outputs(폴더)`를 쓸 수 있습니다.

## 입력 파일

`--dir_path`로 지정한 폴더에 측정 파일과 보정 파일을 둬야 합니다.

| 파일 | 설명 |
| --- | --- |
| `FL,FR.wav`, `FC.wav`, `SL,SR.wav` 등 | 스피커 측정 파일입니다. 파일 이름의 스피커 이름을 보고 채널을 판단합니다. `FC,X.wav`의 `X`는 건너뛸 sweep 자리입니다(센터 스피커를 스테레오 분절 sweep으로 녹음했을 때 나머지 한쪽). 원본 Impulcifer와 같은 규칙입니다. |
| `headphones.wav` | 기본 헤드폰 보정 측정 파일입니다. `--headphone_compensation_file`로 다른 파일을 지정할 수 있습니다. |
| `room-target.csv` | 룸 보정 목표 응답입니다. 없으면 평탄한 목표를 씁니다. |
| `room-mic-calibration.csv` 또는 `room-mic-calibration.txt` | 룸 측정 마이크 보정 파일입니다. 없으면 마이크 보정을 건너뜁니다. |
| `eq.csv`, `eq-left.csv`, `eq-right.csv` | Custom EQ 파일입니다. `eq.csv`는 양쪽 공통, `eq-left.csv`와 `eq-right.csv`는 좌우 개별 EQ입니다. 같은 이름의 `.txt`(예: `eq.txt`)도 읽습니다. |
| `test.wav` | 녹음 때 쓴 sweep입니다. 녹음 화면에서 sweep 설정을 바꿔 녹음하면 자동으로 저장되고, 처리할 때 그대로 씁니다. |

Custom EQ 파일은 두 가지 형식을 읽습니다. 형식은 확장자가 아니라 내용으로 판별합니다.

* **AutoEQ 결과 CSV:** `frequency,raw,error,...` 형식입니다. error 열이 없는 2열(`주파수 게인`) 파일은 값을 그대로 적용할 EQ 게인 곡선으로 읽습니다.
* **EqualizerAPO(-XT) 설정 텍스트:** `Preamp:`, `Filter n: ON PK Fc ... Hz Gain ... dB Q ...`, `GraphicEQ:` 형식입니다. AutoEQ의 ParametricEQ.txt·GraphicEQ.txt 내보내기와 EqualizerAPO-XT에서 저장한 설정을 그대로 쓸 수 있습니다. 크기 응답으로 나타낼 수 있는 명령(Filter 바이쿼드·IIR, Preamp, GraphicEQ, `Convolution`)은 적용하고, 나타낼 수 없는 명령(`Copy`, `Delay`, `MultiConvolution`, VSTPlugin 등)은 경고를 남기고 건너뜁니다. `Convolution:`은 IR 파일의 크기 응답만 반영하고(위상 제외), EqualizerAPO처럼 샘플레이트가 다르면 적용하지 않습니다. `Channel: L`·`Channel: R` 구간은 좌우 EQ 곡선에 따로 적용하고, `Include:`는 같은 폴더 기준 상대 경로면 따라 들어가며, `If: sampleRate == 48000` 같은 단순한 샘플레이트 조건은 평가합니다(그 밖의 조건식 구간은 건너뜁니다).

## 출력 파일

처리가 끝나면 측정 폴더에 다음 파일이 생깁니다.

* **`hesuvi.wav`:** HeSuVi(Equalizer APO)용 BRIR입니다.
* **`hrir.wav`:** 스피커 순서대로 늘어놓은 BRIR입니다.
* **`responses.wav`, `headphone-responses.wav`, `room-responses.wav`:** 처리 중간의 임펄스 응답입니다.
* **`README.md`:** 출력 샘플레이트, 적용한 정규화 게인, 스피커·귀별 PNR·ITD·RT60, 반사음 레벨을 적은 요약입니다.
* **`plots/`:** 헤드폰 보정 그래프와 결과 그래프입니다. `--plot`을 주면 스피커·귀별 응답 그래프와 양이 레벨차(ILD)·위상차(IPD)·IACC 그래프 등을 더 저장합니다(데모에서는 모두 55장).

스피커 하나는 왼쪽 귀·오른쪽 귀 트랙 두 개로 저장됩니다. 몇 번째 트랙이 어느 스피커의 소리를 어느 귀로 보내는지는 [채널 순서 문서](docs/brir-channel-order.md)에 정리했습니다.

뒤쪽의 무음 확장 채널은 자동으로 뺍니다. `hesuvi.wav`의 앞 14채널과 `hrir.wav`의 앞 16채널은 항상 남기고, 그 뒤로는 마지막 유효 스피커까지 남깁니다. 그래서 와이드·상단 스피커가 없는 일반 측정은 각각 14채널, 16채널로 저장됩니다. 중간의 빈자리와 한쪽 귀에만 응답이 있는 스피커는 그대로 두고, 전체 샘플이 정확히 0인 뒤쪽 스피커 쌍만 뺍니다. FL·FR만 측정해도 `hesuvi.wav`의 앞 14채널을 남기는 까닭은 Equalizer APO가 모자란 IR 채널을 처음부터 반복해 적용하기 때문입니다. 이 채널들을 빼면 다른 스피커에 엉뚱한 응답이 적용됩니다. 근거는 [채널 호환성 분석](docs/silent-channel-compatibility.md)에 있습니다.

'중간 무음 채널도 제거'(`--remove_silent_channels`, 기본값 꺼짐)를 켜면 개별 무음 채널까지 빼므로, FL·FR만 있는 출력은 4채널이 됩니다. 이렇게 만든 파일은 채널 위치가 바뀌어 HeSuVi 등에서 쓸 수 없을 수 있습니다. 줄인 WAV 안에는 남은 채널 이름을 기록해 두므로 출력 복원에서 원래 배치로 되돌릴 수 있습니다. 다만 오디오 편집기로 이 정보를 지우면 원래 배치로 되돌릴 수 없습니다.

출력 복원은 남아 있는 출력만으로 빠진 형식을 다시 만듭니다. `Hangloose` 폴더의 스피커별 WAV만 남았다면 정해진 채널 순서로 `hrir.wav`와 `hesuvi.wav`를 모두 다시 만들고, 둘 중 하나만 남았다면 나머지 하나를 만듭니다. `hrir.wav`나 `hesuvi.wav`에서 스피커별 Hangloose 파일을 함께 만들 수도 있습니다. 출력 폴더, 그 안의 `Hangloose` 폴더, 분할 WAV가 바로 들어 있는 폴더 중 어느 것을 골라도 되고, 이미 있는 파일은 건드리지 않고 빠진 파일만 만듭니다.

## CLI 옵션

### 입력과 파일

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--dir_path PATH` | 필수 | 측정 파일을 읽고 결과를 저장할 폴더입니다. |
| `--test_signal VALUE` | 자동 감지 (`test.wav` → 녹음 분석 → 내장 `default`) | 측정에 쓴 sweep WAV, TrueHD/MLP 파일, 미리 정한 이름, `auto`(녹음에서 sweep 파라미터 자동 복원) 또는 `generate:<길이>s@<샘플레이트>`(예: `generate:6.15s@48000`, 파라미터로 직접 생성)입니다. |
| `--room_target PATH` | `dir_path/room-target.csv` | 룸 보정 목표 응답 CSV입니다. 파일이 없으면 평탄한 목표를 씁니다. |
| `--room_mic_calibration PATH` | `dir_path/room-mic-calibration.csv`, 없으면 `.txt` | 룸 측정 마이크 보정 파일입니다. |
| `--headphone_compensation_file PATH` | `dir_path/headphones.wav` | 헤드폰 보정 측정 WAV입니다. 폴더를 주면 흔히 쓰는 파일 이름을 찾아봅니다. |
| `--fs HZ` | 측정 신호의 샘플레이트 | 출력 샘플레이트입니다. 지정하면 결과를 그 샘플레이트로 맞춥니다. |

`--test_signal`에는 다음 약칭을 쓸 수 있습니다.

| 값 | 의미 |
| --- | --- |
| `auto` | 폴더의 `test.wav` → 녹음 파일 분석(sweep 길이 그리드 복원) → 내장 기본 순으로 찾습니다. 지정하지 않았을 때와 같습니다. |
| `generate:<길이>s@<fs>` | 파라미터로 sweep을 직접 만듭니다. 길이는 생성기 그리드에 맞춰집니다. |
| `default`, `1`, `sweep`, `2` | 내장 기본 sweep WAV입니다. |
| `stereo`, `3` | `FL,FR` 스테레오 분절 sweep입니다. |
| `mono-left`, `4` | `FL` 모노 분절 sweep입니다. |
| `left`, `5` | `FL` 스테레오 분절 sweep입니다. |
| `right`, `6` | `FR` 스테레오 분절 sweep입니다. |

sweep 파일을 따로 준비하지 않아도 됩니다. 녹음 화면은 기본으로 sweep을 즉석에서 만들어 재생하고(내장 파일과 같은 신호), 설정을 바꿔 녹음하면 `test.wav`를 녹음 폴더에 저장해 처리 때 그대로 씁니다.

### 보정과 목표 응답

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--channel_balance VALUE` | 사용 안 함 | 좌우 레벨이나 응답 차이를 보정합니다. `trend`, `left`, `right`, `avg`, `min`, `mids` 또는 dB 값을 받습니다. |
| `--decay VALUE` | 사용 안 함 | 잔향 꼬리를 줄입니다. `300`처럼 전체 ms 값을 주거나 `FL:500,FC:100`처럼 채널별 ms 값을 줍니다. |
| `--target_level DB` | 사용 안 함 | 좌우 평균 레벨을 지정한 dB로 맞춥니다. 클리핑을 피하려면 보통 음수를 씁니다. |
| `--fr_combination_method average\|conservative` | `average` | 여러 룸 측정 응답을 합치는 방식입니다. |
| `--room_range modes\|schroeder\|extreme\|legacy` | `schroeder` | 룸 보정 범위입니다. `modes`는 룸 모드 공진만 줄이고 저역 밸런스는 그대로 둡니다. `schroeder`는 슈레더 주파수까지 평탄하게 맞추고, 실제 방 EQ로는 메울 수 없는 딥도 메웁니다. `extreme`은 10 kHz까지 넓게 스무딩한 음색 보정을 더합니다. `legacy`는 2.x 동작입니다. `legacy`를 뺀 세 범위는 스피커의 저역 롤오버를 자동으로 찾아 그 아래를 부스트하지 않습니다. `--room_mode eq`에서만 씁니다. 자세한 내용은 `docs/rust/ROOM_CORRECTION.md`에 있습니다. |
| `--room_volume M3` | 50으로 가정 | 슈레더 주파수를 계산할 방 부피(m³)입니다. |
| `--schroeder_freq HZ` | 측정에서 추정 | 슈레더 주파수를 직접 지정합니다. |
| `--room_max_boost DB` | `12` | 스피커·귀별 룸 측정으로 딥을 메울 때 쓰는 최대 부스트입니다. `--room_mode eq`에서만 씁니다. |
| `--room_mode eq\|tuning` | `eq` | 룸 보정 모드입니다. `eq`는 귀 위치마다 주파수 응답을 보정합니다(위의 범위 옵션). `tuning`(가상 룸 튜닝)은 녹음할 때 스피커 신호 경로에 룸 보정 프로세서(SECS 방식)가 있었다면 걸었을 보정을 스피커마다 두 귀에 같게 겁니다. 주파수 응답, 스피커 레벨, 시간 응답을 보정하고 모든 채널을 `--room_tuning_delay`만큼 늦춥니다. 보정의 기준은 룸 측정 지점이므로, 결과 BRIR의 저역 잔향(EDT)이나 초과 군지연이 줄어든다는 뜻은 아닙니다(데모에서는 둘 다 늘었습니다). 귀 위치 룸 측정(`room-<스피커>-left.wav`, `room-<스피커>-right.wav`)을 쓰고 `room.wav`는 쓰지 않습니다. 결정 근거는 ADR 0005입니다. |
| `--room_tuning_delay MS\|auto` | `10` | 튜닝 지연(2~20 ms)입니다. `auto`는 SECS와 같은 기준으로 2~10 ms 가운데 하나를 골라 모든 채널에 씁니다. |
| `--room_tuning_phase_limit off\|schroeder\|full\|HZ` | `full` | 시간 응답 보정의 상한입니다. `off`는 주파수 응답과 레벨만 보정하고 지연을 더하지 않습니다. `schroeder`는 슈레더 주파수(최대 300 Hz)까지, 숫자는 그 주파수(300~20000 Hz)까지 보정합니다. |
| `--room_tuning_max_boost DB` | `6` | 가상 룸 튜닝의 최대 부스트(0~12 dB)입니다. |
| `--room_tuning_curtain HZ` | `300` | 정밀 보정의 상한(100~5000 Hz)입니다. 그 위 한 옥타브에 걸쳐 풀리고, 더 위로는 넓은 음색 보정만 합니다. |
| `--room_tuning_level_match true\|false` | `true` | 청취 위치에서 스피커 레벨을 ±6 dB 안에서 맞춥니다. |
| `--specific_limit HZ` | `400` | 스피커·귀별 룸 보정의 상한 주파수입니다. `0`이면 제한을 끕니다. `--room_range legacy`에서만 씁니다. |
| `--generic_limit HZ` | `300` | 공통 룸 보정의 상한 주파수입니다. `0`이면 제한을 끕니다. `--room_range legacy`에서만 씁니다. |
| `--bass_boost DB` | 사용 안 함 | 저역 셸프 부스트입니다. `6`처럼 게인만 주거나(Fc 105 Hz, Q 0.76) `6,150,0.69`처럼 게인, Fc, Q를 줍니다. |
| `--tilt DB_PER_OCT` | `0.0` | 목표 응답 기울기입니다. 양수는 밝게, 음수는 어둡게 맞춥니다. |
| `--no_room_correction` | 룸 보정 켜짐 | 룸 보정을 건너뜁니다. |
| `--no_headphone_compensation` | 헤드폰 보정 켜짐 | 헤드폰 보정을 건너뜁니다. |
| `--no_equalization` | EQ 켜짐 | Custom EQ를 건너뜁니다. |

### 출력과 진단

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--plot` | 꺼짐 | 처리 그래프를 PNG로 모두 저장합니다. |
| `--interactive_plots` | 꺼짐 | 대화형 HTML 그래프를 `interactive_plots/`에 저장합니다. |
| `--c MS` | `1.0` | IR 앞부분을 자를 때 남길 여유 시간입니다. 단위는 ms입니다. |
| `--jamesdsp` | 꺼짐 | `FL`·`FR`로 만든 `jamesdsp.wav`를 추가로 저장합니다. |
| `--remove_silent_channels` | 꺼짐 | 중간 무음 채널까지 뺍니다. 채널 위치가 바뀌므로 HeSuVi 등과 맞지 않을 수 있습니다. |
| `--hangloose` | 꺼짐 | Hangloose Convolver용 스피커별 스테레오 IR 파일을 저장합니다. |
| `--output_truehd_layouts` | 꺼짐 | TrueHD용 레이아웃 출력을 추가로 저장합니다. |
| `--info` | 꺼짐 | 버전, 운영체제, CPU 코어 수, 데이터 폴더 등을 출력하고 끝냅니다. |
| `-V`, `--version` | 꺼짐 | 버전을 출력하고 끝냅니다. |

### Virtual Bass

스피커가 내지 못하는 저음을 합성한 저음으로 채웁니다. 크로스오버 아래의 측정 응답은 걸러내고, 모든 스피커가 함께 쓰는 합성 저음을 그 자리에 더합니다.

`auto`와 `manual`은 합성 저음을 룸 보정과 맞춥니다(ADR 0006). 레벨은 크로스오버 위 두 옥타브에서 잰 응답의 중앙값에 맞추고, 룸 EQ가 켜져 있으면 보정한 뒤의 응답으로 잽니다. 크로스오버에서 스피커와 위상이 맞도록 모든 스피커의 합성 저음을 같은 시간만큼 늦춥니다. 룸 보정이 룸 목표(`room-target.csv`)를 쓰면 합성 저음도 그 목표의 저역 모양을 따릅니다.

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--vbass` | 꺼짐 | Virtual Bass 합성을 켭니다. |
| `--vbass_mode auto\|manual\|legacy` | `auto` | 크로스오버를 정하는 방식입니다. `auto`는 측정에서 스피커마다 저음이 줄어드는 지점(평탄한 구간보다 6 dB 낮은 곳)을 찾고, 그중 가장 높은 지점의 한 옥타브 위를 크로스오버로 씁니다. 저음이 줄어드는 곳을 찾지 못하면 아무것도 더하지 않습니다. `manual`은 `--vbass_freq`를 씁니다. 스피커의 한계보다 높게 잡으면 그 주파수까지의 측정 저음을 룸 모드째 합성 저음으로 바꿉니다. `legacy`는 2.x 방식입니다. `--vbass_mode` 없이 `--vbass_freq`를 주면 `manual`입니다. |
| `--vbass_freq HZ` | `250` | `manual`과 `legacy`의 크로스오버 주파수입니다. |
| `--vbass_hp HZ` | `15.0` | 합성한 저역에 적용할 하이패스 주파수입니다. |
| `--vbass_polarity auto\|normal\|invert` | `auto` | 합성한 저역의 극성 처리 방식입니다. |

### 마이크 착용 편차 보정

방향과 상관없는 좌우 마이크 차이(착용 깊이·각도·감도)를 보정합니다. 헤드폰 보정을 같은 마이크로 측정하면 마이크 응답이 그 단계에서 이미 상쇄되므로, **헤드폰 보정이 켜져 있으면 이 보정은 건너뜁니다.** 자세한 내용은 [마이크 착용 편차 보정](docs/README_microphone_deviation_correction.md) 문서에 있습니다.

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--microphone_deviation_correction` | 꺼짐 | 좌우 마이크 차이를 보정합니다. 헤드폰 보정이 켜져 있으면 건너뜁니다. |
| `--mic_deviation_strength VALUE` | `0.7` | 보정 강도입니다. `0.0`은 보정 없음, `1.0`은 전체 보정입니다. |
| `--mic_deviation_debug_plots` | 꺼짐 | 추정한 좌우 불일치와 귀별 보정량을 `plots/microphone_deviation_v4.png`로 저장합니다. 보정을 실제로 적용했을 때만 저장합니다. |

## CLI 예시

데모 폴더를 처리하고 그래프를 모두 저장합니다.

```bash
impulcifer --dir_path "data/demo" --plot
```

룸 보정과 헤드폰 보정을 끄고 측정 IR만 정리합니다.

```bash
impulcifer --dir_path "measurements" --no_room_correction --no_headphone_compensation
```

Virtual Bass를 켜고(크로스오버는 측정에서 찾음) JamesDSP 출력도 함께 만듭니다.

```bash
impulcifer --dir_path "measurements" --vbass --jamesdsp
```

채널별 decay를 지정합니다.

```bash
impulcifer --dir_path "measurements" --decay "FL:500,FC:100,FR:500"
```

## 알려진 제한

3.x를 쓰기 전에 다음을 알아 두어야 합니다.

* **Intel Mac과 Linux ARM에서는 앱을 쓸 수 없습니다:** 앱은 Windows x64, macOS(Apple Silicon), Linux x86_64용만 있으므로, 그 밖의 플랫폼에서는 `impulcifer_gui`도 쓸 수 없고 PyPI 패키지의 CLI와 Python API만 쓸 수 있습니다.
* **`impulcifer_gui`를 처음 실행할 때는 인터넷에 연결돼 있어야 합니다:** pip 패키지에는 앱이 들어 있지 않고, 처음 실행할 때 GitHub Releases에서 받습니다. 연결할 수 없는 환경에서는 앱을 직접 받아 설치해야 합니다.
* **CustomTkinter 화면을 쓰려면 2.x를 설치해야 합니다:** 3.x 앱에서는 Stable 프리셋이 그 화면의 배치를 따릅니다.
* **처음 실행할 때 보안 경고가 뜰 수 있습니다:** 설치 파일에 코드 서명이 없어서 Windows SmartScreen이나 macOS Gatekeeper가 실행을 막을 수 있습니다. Windows에서는 '추가 정보'를 누른 뒤 '실행'을 눌러야 하고, macOS에서는 시스템 설정의 '개인정보 보호 및 보안'에서 실행을 허용해야 합니다.
* **TrueHD(`.mlp`, `.thd`, `.truehd`)를 입력하려면 FFmpeg 4.0 이상이 있어야 합니다:** 3.x는 FFmpeg를 자동으로 설치하지 않으므로 미리 설치해야 합니다(Windows는 `winget install Gyan.FFmpeg`, macOS는 `brew install ffmpeg`, Linux는 `sudo apt install ffmpeg`).

같은 측정을 2.x와 3.x로 처리하면 결과 파일이 비트 단위로 같지 않습니다. 그 차이는 처리 단계마다 2.x 출력과 대조하는 테스트의 허용 오차 안에 있습니다. 원본 Impulcifer와 비교해도 수치 라이브러리와 보정 옵션 차이로 결과가 조금 다를 수 있습니다.

## 소스에서 빌드하고 테스트하기

3.x는 저장소 루트의 Cargo 워크스페이스(`crates/`, `apps/impulcifer-app`)이고, 툴체인 버전은 `rust-toolchain.toml`(1.97)에 고정돼 있습니다. 구조는 [docs/rust/ARCHITECTURE.md](docs/rust/ARCHITECTURE.md)에 있습니다.

```bash
cargo test --workspace                                    # 전체 테스트
cargo run -p impulcifer-cli --release -- --dir_path data/demo   # CLI
```

앱 패키지는 Tauri CLI(`npm install -g @tauri-apps/cli@^2`)로 `apps/impulcifer-app`에서 만들 수 있습니다. Linux에서 빌드하려면 `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libasound2-dev` 등을 설치해야 하고, 플랫폼별 명령은 [docs/rust/PACKAGING.md](docs/rust/PACKAGING.md)에 있습니다. PyPI 휠 빌드는 [tests/migration/README-python.md](tests/migration/README-python.md)에 적었습니다.

2.x는 저장소 루트의 Python 코드(`impulcifer.py`, `core/`, `gui/` 등)입니다. `pip install -e .`로 설치하고 `pytest tests/`로 테스트할 수 있습니다. Nuitka 단독 실행 파일 빌드는 [빌드 가이드](docs/BUILD_README.md)에 있습니다.

## 추가 문서

* [hrir.wav·hesuvi.wav 채널 순서와 귀 배정](docs/brir-channel-order.md)
* [무음 확장 채널 호환성 분석](docs/silent-channel-compatibility.md)
* [TrueHD/MLP 지원 및 레이아웃 출력](docs/README_TrueHD.md)
* [마이크 착용 편차 보정](docs/README_microphone_deviation_correction.md)
* [2.x와 3.x 측정값 (설치 크기·처리 시간·메모리)](docs/2x-vs-3x.md)
* [2.x와 3.x 비교 측정 (Linux)](docs/rust/perf/2x-vs-3x-linux.md)
* [3.x 출시 전 성능 감사 (Windows)](docs/rust/perf/release.md)
* [3.x 구조](docs/rust/ARCHITECTURE.md)
* [2.x Python 3.14 및 Nuitka 빌드 메모](docs/README_PYTHON314.md)
* [2.x 빌드 가이드 (Nuitka)](docs/BUILD_README.md)

## 라이선스

이 프로젝트는 MIT License를 따릅니다. 전체 문구는 [LICENSE](LICENSE)에 있습니다.

저작권 표기는 `LICENSE`와 같습니다.

* Copyright (c) 2018- Jaakko Pasanen
* Copyright (c) 2024- 115dkk
* Copyright (c) 2025- LionLion123
* Copyright (c) 2025- SDC (DCinside)

## 기여와 문의

버그를 찾았거나 고칠 점이 있으면 [이슈 트래커](https://github.com/115dkk/Impulcifer-pip313/issues)에 남겨 주세요.

## 구걸

개발자는 돈이 필요합니다.\
이 프로그램이 좋다고 생각하시면 한 푼만 주십시오...

**지금 모으는 돈:** EV 인증서, 딱 한 번 $359. 개인 BRIR로 헤드폰에서 Dolby Atmos를 듣는 새 앱의 드라이버를 Microsoft 서명으로 내기 위한 비용입니다. 이게 있으면 Secure Boot를 끄지 않고 설치되는 앱을 낼 수 있습니다. (현재 $0 / $359)

**초과분은 이렇게 씁니다:**

* AI 구독료
* AI 개발사들이 요구하는 하드웨어 키 비용
* 개발자의 집값 대출 상환에 보탬

[GitHub Sponsors](https://github.com/sponsors/115dkk)
