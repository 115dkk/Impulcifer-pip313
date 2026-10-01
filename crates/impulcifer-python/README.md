# impulcifer-py313

[Impulcifer-py313](https://github.com/115dkk/Impulcifer-pip313)은 [Jaakko Pasanen의 Impulcifer](https://github.com/jaakkopasanen/impulcifer)를 바탕으로 한 포크입니다. 바이노럴 마이크로 녹음한 스피커 측정 파일에서 개인 BRIR WAV를 만들고, 그 파일을 HeSuVi(Equalizer APO), JamesDSP, Hangloose Convolver 같은 컨볼버에 넣어 헤드폰에서 스피커 소리를 재현할 수 있습니다.

이 패키지는 3.x입니다. 처리는 패키지에 든 컴파일된 확장 모듈 하나가 하므로 NumPy·SciPy 같은 의존 패키지를 따로 설치하지 않습니다. 이 패키지로는 `impulcifer` 명령과 Python API를 쓸 수 있습니다. 녹음하거나 화면에서 처리하려면 [GitHub Releases](https://github.com/115dkk/Impulcifer-pip313/releases/latest)에서 앱(Windows, macOS Apple Silicon, Linux AppImage)을 받아야 합니다.

## 설치

```bash
pip install impulcifer-py313
```

Python 3.9 이상이 필요합니다. Windows x64, macOS(Apple Silicon), Linux x86_64(glibc 2.28 이상)에서는 미리 빌드한 휠이 설치됩니다. 그 밖의 플랫폼(Intel Mac, Linux ARM 등)에서는 pip가 소스 배포본을 빌드하므로 Rust 1.97 이상이 있어야 합니다. Rust가 없으면 빌드 도구(maturin)가 임시로 설치해 씁니다.

## CLI

`--dir_path`로 측정 폴더를 지정해야 합니다.

```bash
impulcifer --dir_path "measurements" --plot
```

옵션은 2.x와 같습니다. 전체 옵션은 `impulcifer --help`로 확인할 수 있고, 입력 파일 이름과 옵션 설명은 [저장소 README](https://github.com/115dkk/Impulcifer-pip313#readme)에 있습니다.

## Python API

```python
import impulcifer
from impulcifer import impulcifer_native as native

# 처리 옵션을 CLI와 같은 이름의 키워드 인자로 넘기면 hesuvi.wav의 경로를 돌려받습니다.
path = impulcifer.main(dir_path="measurements", vbass=True, vbass_freq=250)

# 진행률과 로그가 필요하면 native.run에 콜백을 넘겨야 합니다.
native.run(
    {"dir_path": "measurements"},
    progress=lambda event: print(event["progress"], event["message"]),
)
```

`decay`를 숫자나 스피커별 딕셔너리로 넘길 때는 초 단위로 줘야 합니다(CLI는 밀리초). 이 밖에 `impulcifer.detect_sweep(폴더)`, `impulcifer.generate_sweep_set(폴더)`, `impulcifer.recover_brir_outputs(폴더)`를 쓸 수 있습니다.

## 2.x

2.x(Python 판, CustomTkinter·웹뷰 화면 포함)를 계속 쓰려면 버전을 3 미만으로 고정해야 합니다.

```bash
pip install "impulcifer-py313<3"
```

2.x를 설치한 환경에서 `pip install --upgrade impulcifer-py313`을 실행하면 3.x로 올라가면서 `impulcifer_gui`, `impulcifer_webview` 명령이 사라집니다.

## 링크

* [저장소와 전체 README](https://github.com/115dkk/Impulcifer-pip313)
* [변경 내역](https://github.com/115dkk/Impulcifer-pip313/blob/master/CHANGELOG.md)
* [이슈 트래커](https://github.com/115dkk/Impulcifer-pip313/issues)

MIT License로 배포합니다.
