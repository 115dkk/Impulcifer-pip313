# Impulcifer-py313: Python 3.13/3.14 호환 및 최적화 버전

[![PyPI version](https://badge.fury.io/py/impulcifer-py313.svg)](https://badge.fury.io/py/impulcifer-py313)

이 프로젝트는 [Jaakko Pasanen의 원본 Impulcifer](https://github.com/jaakkopasanen/impulcifer) 프로젝트를 기반으로 하여, **Python 3.13/3.14 환경과의 완벽한 호환성 및 성능 최적화**를 제공하는 포크 버전입니다.

## 🌟 프로젝트 목표 및 주요 변경 사항

원본 Impulcifer는 훌륭한 도구이지만, 최신 Python 환경에서의 호환성 문제가 있었습니다. `Impulcifer-py313`은 다음을 목표로 합니다:

- **Python 3.13/3.14 완벽 지원**: 최신 Python 버전(3.13.x, 3.14.x)에서 문제없이 작동하도록 의존성 및 내부 코드를 수정했습니다.
- **Python 3.13+ free-threaded (no-GIL) 최적화**: GIL이 비활성화된 환경에서 자동으로 감지하여 최적의 병렬 처리를 수행합니다 (3-7배 속도 향상).
- **성능 최적화**: 메모리 사용량 10-20% 감소, 벡터화된 알고리즘으로 전체 4-8배 성능 향상.
- **간편한 설치**: PyPI를 통해 단 한 줄의 명령어로 쉽게 설치할 수 있습니다.

  ```bash
  pip install impulcifer-py313
  ```
  
  또는 요즘 떠오르는 최신 기술인 uv를 이용해서 같은 방식으로 설치하실 수 있습니다.

  ```bash
  uv pip install impulcifer-py313 --system
  ```

- **테스트 신호 지정 간소화**: 기존의 파일 경로 직접 지정 방식 외에, 미리 정의된 이름(예: "default", "stereo")이나 숫자(예: "1", "3")로 간편하게 테스트 신호를 선택할 수 있는 기능을 추가했습니다.
- **지속적인 유지보수**: Python 및 관련 라이브러리 업데이트에 맞춰 지속적으로 호환성을 유지하고 사용자 피드백을 반영할 예정입니다.

## 💿 설치 방법

Impulcifer-py313은 두 가지 방법으로 설치할 수 있습니다:

### 방법 1: 최종 사용자용 독립 실행 파일 (권장)

**Python 설치 없이** 바로 실행 가능한 독립 실행 파일을 제공합니다. [GitHub Releases](https://github.com/115dkk/Impulcifer-pip313/releases) 페이지에서 운영체제에 맞는 파일을 다운로드하세요.

#### Windows
1. `Impulcifer_Setup.exe` 다운로드
2. 인스톨러 실행 후 설치 마법사 따라가기
3. 시작 메뉴 또는 바탕화면 아이콘으로 실행

#### macOS
1. `Impulcifer-*-macOS.dmg` 다운로드
2. DMG 파일 열기
3. Impulcifer 아이콘을 Applications 폴더로 드래그
4. Applications 폴더에서 실행

#### Linux

**AppImage (권장):**
```bash
# 실행 권한 부여
chmod +x Impulcifer-*.AppImage

# 실행
./Impulcifer-*.AppImage
```

**Tarball:**
```bash
# 압축 해제
tar xzf Impulcifer-*-linux-x86_64.tar.gz

# 디렉토리 이동
cd Impulcifer-linux

# 실행
./run.sh
```

### 방법 2: Python 개발 환경에서 설치

Python 개발자이거나 최신 개발 버전을 사용하려는 경우 pip 또는 uv를 통해 설치할 수 있습니다.

#### 사전 요구 사항
- Python 3.9 이상, **3.13.x 또는 3.14.x 권장** (최신 버전에서 테스트 및 최적화되었습니다)
- Python 3.13+ free-threaded 빌드 사용 시 최대 성능 (GIL 없이 3-7배 빠름)
- `pip` (Python 패키지 설치 프로그램)

#### 설치 명령어

터미널 또는 명령 프롬프트에서 다음 명령어를 실행하여 `impulcifer-py313`을 설치합니다:

```bash
pip install impulcifer-py313
```

또는

```bash
uv pip install impulcifer-py313 --system
```

가상 환경(virtual environment) 내에 설치하는 것을 권장합니다:

```bash
# 가상 환경 생성 (예: venv 이름 사용)
python -m venv venv

# 가상 환경 활성화
# Windows:
venv\\Scripts\\activate
# macOS/Linux:
source venv/bin/activate

# Impulcifer-py313 설치
pip install impulcifer-py313
```

## 🚀 사용 방법

설치가 완료되면 `impulcifer` 명령어를 사용하여 프로그램을 실행할 수 있습니다.

### GUI (그래픽 사용자 인터페이스) 사용법

`impulcifer-py313`은 사용 편의성을 위해 그래픽 사용자 인터페이스(GUI)도 제공합니다.
GUI를 실행하려면 터미널 또는 명령 프롬프트에서 다음 명령어를 입력하세요:

```bash
impulcifer_gui
```

GUI를 통해 대부분의 기능을 직관적으로 설정하고 실행할 수 있습니다.

- **Recorder 창**: 오디오 녹음 관련 설정을 합니다.
- **Impulcifer 창**: HRIR 생성 및 보정 관련 설정을 합니다.

각 옵션에 마우스를 올리면 간단한 설명을 확인할 수 있습니다.

### CLI (명령줄 인터페이스) 사용법

기존의 명령줄 인터페이스도 동일하게 지원합니다.

#### 기본 명령어

```bash
impulcifer --help
```

사용 가능한 모든 옵션과 설명을 확인할 수 있습니다.

### 주요 개선 기능 사용 예시

#### 1. 간편한 테스트 신호 지정

`--test_signal` 옵션을 사용하여 미리 정의된 이름이나 숫자로 테스트 신호를 지정할 수 있습니다.

- **이름으로 지정**:

  ```bash
  impulcifer --test_signal="default" --dir_path="data/my_hrir"
  impulcifer --test_signal="stereo" --dir_path="data/my_hrir"
  ```

- **숫자로 지정**:

  ```bash
  impulcifer --test_signal="1" --dir_path="data/my_hrir" # "default"와 동일
  impulcifer --test_signal="3" --dir_path="data/my_hrir" # "stereo"와 동일
  ```

  사용 가능한 미리 정의된 테스트 신호:
  - `"default"` / `"1"`: 기본 Pickle 테스트 신호 (`sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl`)
  - `"sweep"` / `"2"`: 기본 WAV 테스트 신호 (`sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`)
  - `"stereo"` / `"3"`: FL,FR 스테레오 WAV 테스트 신호
  - `"mono-left"` / `"4"`: FL 모노 WAV 테스트 신호
  - `"left"` / `"5"`: FL 스테레오 WAV 테스트 신호 (채널 1만 사용)
  - `"right"` / `"6"`: FR 스테레오 WAV 테스트 신호 (채널 2만 사용)

#### 2. 데모 실행

프로젝트에 포함된 데모 데이터를 사용하여 Impulcifer의 기능을 테스트해볼 수 있습니다. `Impulcifer`가 설치된 환경에서, 데모 데이터가 있는 경로를 지정하여 실행합니다. (데모 데이터는 원본 프로젝트 저장소의 `data/demo` 폴더를 참고하거나, 직접 유사한 구조로 준비해야 합니다.)

만약 로컬에 원본 Impulcifer 프로젝트를 클론하여 `data/demo` 폴더가 있다면:

```bash
# Impulcifer 프로젝트 루트 디렉토리로 이동했다고 가정
impulcifer --test_signal="default" --dir_path="data/demo" --plot
```

또는 `impulcifer-py313` 패키지 내부에 포함된 데모용 테스트 신호를 사용하고, 측정 파일만 `my_measurements` 폴더에 준비했다면:

```bash
impulcifer --test_signal="default" --dir_path="path/to/your/my_measurements" --plot
```

#### 인터랙티브 플롯 생성

`--interactive_plots` 옵션을 사용하면 Bokeh 기반의 인터랙티브 플롯을 HTML 파일로 생성합니다.

```bash
impulcifer --dir_path="path/to/your/my_measurements" --interactive_plots
```

이 명령은 `path/to/your/my_measurements/interactive_plots/interactive_summary.html`에 플롯을 저장합니다.

### 기타 옵션

다른 모든 옵션(룸 보정, 헤드폰 보정, 채널 밸런스 등)은 원본 Impulcifer와 거의 동일하게 작동합니다. `--help` 명령어를 통해 자세한 내용을 확인하세요.

## 📚 추가 가이드

이 프로젝트에는 특정 기능에 대한 상세한 가이드 문서들이 제공됩니다:

### 🎵 [TrueHD/MLP 지원 및 자동 채널 생성 가이드](README_TrueHD.md)
- TrueHD/MLP 오디오 파일 지원
- 자동 채널 생성 기능 (FC, TSL, TSR)
- 11채널/13채널 TrueHD 레이아웃 출력
- GUI 및 CLI 사용법 상세 설명
- 측정 예시 및 문제 해결

### 🎧 [마이크 착용 편차 보정 가이드](README_microphone_deviation_correction.md)
- 바이노럴 측정 시 마이크 위치 편차 보정
- MTW(Minimum Time Window) 기반 분석
- 주파수 대역별 가변 게이팅
- 사용법 및 파라미터 설명
- 분석 결과 및 시각화

## ⚠️ 주의 사항

- 이 버전은 **Python 3.13.2** 환경에 맞춰 개발되고 테스트되었습니다. 다른 Python 버전에서는 예기치 않은 문제가 발생할 수 있습니다. (Python 3.9 이상 지원)
- 원본 Impulcifer의 핵심 기능은 대부분 유지하려고 노력했지만, 내부 코드 수정으로 인해 미세한 동작 차이가 있을 수 있습니다.
- `autoeq-py313` 등 Python 3.13.2 호환성을 위해 수정된 버전에 의존합니다.

## 🔄 업데이트

새로운 버전이 PyPI에 배포되면 다음 명령어로 업데이트할 수 있습니다:

```bash
pip install --upgrade impulcifer-py313
```

## 📄 라이선스 및 저작권

이 프로젝트는 원본 Impulcifer와 동일하게 **MIT 라이선스**를 따릅니다.

- **원본 프로젝트 저작자**: Jaakko Pasanen ([GitHub 프로필](https://github.com/jaakkopasanen))
- **Impulcifer-py313 포크 버전 기여자**: 115dkk ([GitHub 프로필](https://github.com/115dkk))

```text
MIT License

Copyright (c) 2018-2022 Jaakko Pasanen
Copyright (c) 2024- 115dkk (For the Python 3.13.2 compatibility modifications and enhancements)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## 🛠️ 기여 및 문의

버그를 발견하거나 개선 아이디어가 있다면, 이 저장소의 [이슈 트래커](https://github.com/115dkk/Impulcifer-pip313/issues)를 통해 알려주세요.

## 📋 변경사항

전체 변경 이력은 [CHANGELOG.md](CHANGELOG.md)를 참고하세요.
