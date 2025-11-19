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

- 이 버전은 **Python 3.13.2** 환경에 맞춰 개발되고 테스트되었습니다. 다른 Python 버전에서는 예기치 않은 문제가 발생할 수 있습니다. (Python 3.8 이상 지원 목표)
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
Copyright (c) 2023-2024 115dkk (For the Python 3.13.2 compatibility modifications and enhancements)

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

-----------------------------------------------
# 변경사항
-----------------------------------------------

## 최신 업데이트 (v2.1.5)

### macOS 빌드 수정
- **앱 이름 불일치 문제 해결**
  - Nuitka가 `gui_main.app`을 생성하는데 스크립트는 `Impulcifer.app`을 찾던 문제 수정
  - 빌드 후 자동으로 `gui_main.app`을 `Impulcifer.app`으로 이름 변경
  - DMG 생성 전 빌드 결과물 검증 강화

-----------------------------------------------

## 이전 업데이트 (v2.1.4)

### 크로스 플랫폼 빌드 시스템 개선

**macOS 빌드 수정**
- macOS `.app` 번들 생성 시 `--onefile` 옵션 제거
  - 이전에는 `--onefile`과 `--macos-create-app-bundle` 옵션 충돌로 빈 DMG(11KB) 생성 문제 발생
  - `.app` 번들 자체가 단일 패키지이므로 `--onefile` 불필요
  - 앱 실행 속도도 개선 (압축 해제 과정 제거)
- DMG 생성 시 검증 로직 추가
  - `.app` 파일 존재 확인 후 에러 발생 시 빌드 중단
  - 디버깅을 위한 상세 출력 추가

**Linux 빌드 수정**
- AppImage 생성 시 필수 아이콘 자동 생성
  - ImageMagick을 사용해 동적으로 플레이스홀더 아이콘 생성
  - 아이콘 부재로 인한 빌드 실패 문제 해결
- Desktop Entry 파일 형식 표준 준수
  - 들여쓰기 제거 (Desktop Entry 사양 준수)
- 파일 검증 및 에러 처리 개선
  - 실행 파일 존재 여부 확인
  - 에러 발생 시 명확한 메시지와 함께 빌드 중단
  - 디버깅 출력 추가

**CI/CD 안정성 향상**
- YAML 구문 오류 수정
  - heredoc 사용 시 발생하던 YAML 파서 충돌 해결
  - `{ echo ... } > file` 패턴으로 변경
- 시스템 의존성 정리
  - Linux 빌드에 ImageMagick 추가

-----------------------------------------------

### 1. 확장된 다채널 HRIR 처리 지원

Atmos와 같은 다채널 오디오 시스템의 HRIR 생성을 지원하기 위해 처리 가능한 스피커 채널 목록(`constants.py`의 `SPEAKER_NAMES`) 및 관련 파일명 패턴(`constants.py`의 `SPEAKER_LIST_PATTERN`)을 확장했습니다. 이제 `WL,WR.wav`, `TFL,TFR.wav`, `TSL,TSR.wav`, `TBL,TBR.wav` 등 기존 7.1 채널 구성을 넘어서는 다양한 높이 및 와이드 채널의 측정 파일(예: `FL,FR,FC,SL,SR,BL,BR,TFL,TFR,TSL,TSR,TBL,TBR,WL,WR.wav`)을 인식하고 처리할 수 있습니다.

- `hrir.wav` 출력 시, `constants.py`의 `HEXADECAGONAL_TRACK_ORDER`에 정의된 순서를 따르도록 수정하여 최대 16채널까지의 HRIR을 표준화된 순서로 저장합니다. (기존에는 8채널 기반)
- `hesuvi.wav` 출력 시, `constants.py`의 `HESUVI_TRACK_ORDER`에 정의된 HeSuVi 소프트웨어 고유의 채널 순서를 따르도록 했습니다. (사용자 정의 순서 적용)

-----------------------------------------------

### 2. 동측 귀 임펄스 응답 정렬 기능 개선

![002](https://github.com/user-attachments/assets/f90fda17-ce6e-495c-8c04-370dedfa4f0f)

HRIR에서 각 채널의 상대적인 시간차는 정위감에 큰 영향을 미칩니다. `hrir.py`의 `align_ipsilateral_all` 메소드는 동일한 쪽 귀(예: 왼쪽 스피커들의 왼쪽 귀 응답)에 도달하는 임펄스 응답들을 정렬하여 시간적 일관성을 향상시키는 역할을 합니다. 이 기능은 특히 여러 스피커로부터 측정된 HRIR 세트에서 미세한 시간축 불일치를 보정하여 보다 정확하고 선명한 음상을 만드는 데 기여합니다.

- 정렬 기준: 지정된 스피커 쌍(예: `('FL','FR')`, `('SL','SR')`)에 대해, 동측 귀(ipsilateral ear)의 임펄스 응답 중 더 일찍 도착한 신호를 기준으로 다른 신호를 정렬합니다. 예를 들어, `FL`과 `FR` 스피커 쌍의 경우, `FL`의 왼쪽 귀 응답과 `FR`의 왼쪽 귀 응답을 비교하여 정렬합니다.
- 정렬 방식: 각 임펄스 응답의 피크로부터 특정 시간(기본 30ms)만큼의 세그먼트를 추출하고, 이 세그먼트 간의 상호 상관(cross-correlation)을 계산하여 최대 상관값을 가지는 지점으로 시간 지연을 보정합니다. 이를 통해 신호의 주요 에너지 시작점을 일치시킵니다.

-----------------------------------------------

### 3. 저음 부스팅 로직 수정 및 하이패스 필터 조정

기존 Impulcifer 코드에서 의도치 않게 적용될 수 있었던 과도한 저음 부스팅 및 고역 통과 필터(high-pass filter) 로직을 검토하고 수정했습니다.

- **저음 부스팅 (Bass Boost):** `impulcifer.py`의 `create_target` 함수에서, `--bass_boost` CLI 옵션으로 사용자가 명시적으로 지정한 값 외에 추가적인 +3dB 부스트가 적용되던 부분을 제거하여, 사용자의 설정값만이 정확하게 반영되도록 수정했습니다.
- **하이패스 필터 (High-Pass Filter):** `create_target` 함수 내에서 약 20Hz 이하를 급격히 감쇠시키는 하이패스 필터가 기본적으로 적용되었으나, 이 필터의 필요성 및 적용 방식에 대한 재검토 후, 사용자가 의도하지 않은 저역 감쇠를 최소화하는 방향으로 조정하거나, 관련 옵션을 명확히 할 수 있도록 변경 여지를 두었습니다.

-----------------------------------------------

### 4. 임펄스 응답 사전 응답(Pre-response) 길이 조절 옵션 추가

`--c` (또는 내부적으로 `head_ms`) CLI 옵션을 추가하여, 임펄스 응답의 피크 이전 부분(사전 응답, pre-response 또는 head room)의 길이를 밀리초(ms) 단위로 사용자가 직접 설정할 수 있도록 개선했습니다. 이 옵션을 사용하지 않으면 기본값(1.0ms)이 적용됩니다.

- **CLI 옵션:** `impulcifer --c <milliseconds> ...` (예: `--c=10` 또는 `--c=50`)
- **적용 로직:** `hrir.py`의 `crop_heads` 메소드에서 이 값을 사용하여, 각 채널의 임펄스 응답을 자를 때 피크 지점으로부터 지정된 시간만큼의 사전 응답 구간을 확보합니다. 이 구간에는 Hanning 윈도우의 앞부분이 적용되어 부드러운 페이드-인 효과를 줍니다.
- **효용성:** 충분한 사전 응답 확보는 일부 디지털 신호 처리(DSP) 과정(예: 혼합 위상 필터 적용, 크로스토크 제거 시스템)에서 발생할 수 있는 사전 링잉(pre-ringing) 현상을 줄이거나, 시스템의 정확한 응답을 측정하는 데 중요합니다.

-----------------------------------------------

### 5. 주파수 응답 보간법 개선 (Cubic Spline Interpolation)

주파수 응답(Frequency Response) 객체의 보간(interpolation) 정확도를 향상시키기 위해, 기존 선형 보간 방식에서 Scipy 라이브러리의 `interp1d`를 사용한 3차 스플라인(cubic spline) 보간을 우선적으로 적용하도록 개선했습니다.

- **적용 대상 함수:** `impulcifer.py`의 `headphone_compensation` 및 `equalization` 함수 내에서 `FrequencyResponse` 객체의 주파수 축을 재조정할 때 적용됩니다.
- **헬퍼 함수 도입:** `_apply_cubic_interp` 라는 내부 헬퍼 함수를 추가하여, 3차 스플라인 보간을 시도하고 실패할 경우(예: 데이터 포인트 부족) 기존의 선형 보간 방식(`FrequencyResponse.interpolate`)으로 안전하게 폴백(fallback)하도록 구현했습니다.
- **AutoEq 라이브러리 수정 연동:** 관련된 `AutoEq/autoeq/frequency_response.py`의 보간 로직도 데이터 포인트 수에 따라 3차 스플라인 또는 선형 보간을 선택적으로 사용하도록 함께 수정되었습니다. (원본 AutoEq 저장소에 반영되었는지 확인 필요)

-----------------------------------------------

### 6. JamesDSP용 트루 스테레오 IR(.wav) 생성 기능 추가

![image](https://github.com/user-attachments/assets/152603cd-8ba4-401d-aa08-b9594ac20881)
![image](https://github.com/user-attachments/assets/e022b813-4e93-41e5-862c-c04499b66ec3)

`--jamesdsp` CLI 옵션을 통해, 오디오 편집/재생 소프트웨어인 JamesDSP에서 사용할 수 있는 "트루 스테레오(True Stereo)" HRIR 파일을 간편하게 생성하는 기능을 추가했습니다.

- **CLI 옵션:** `impulcifer --jamesdsp ...`
- **동작 방식:**
    1. 전체 HRIR 세트에서 전방 좌측(FL)과 전방 우측(FR) 채널의 임펄스 응답만을 추출합니다.
    2. 추출된 FL, FR 채널을 기준으로 전체 레벨을 다시 정규화합니다. (`hrir.normalize` 호출 시 `target_level` 인자 사용)
    3. `FL-left`, `FL-right`, `FR-left`, `FR-right` 순서의 4채널 WAV 파일을 생성하여, 입력 데이터 폴더 내에 `jamesdsp.wav`라는 이름으로 저장합니다.
- **특징:** 입력 폴더에 다른 채널(예: 서라운드, 높이 채널)의 측정 파일이 존재하더라도, 이 기능은 오직 FL, FR 채널만을 사용하여 스테레오 환경에 최적화된 HRIR을 생성합니다.

-----------------------------------------------

### 7. Hangloose Convolver용 개별 채널 스테레오 IR(.wav) 생성 기능 추가

`--hangloose` CLI 옵션을 사용하여, 각 HRIR 채널(예: FL, FR, FC, SL 등)을 독립적인 스테레오 IR 파일(.wav)로 출력하는 기능을 추가했습니다. 이 파일들은 Hangloose Convolver와 같은 멀티채널 컨볼버에서 사용될 수 있습니다.

- **CLI 옵션:** `impulcifer --hangloose ...`
- **동작 방식:**
    1. 입력 데이터 폴더 내에 `Hangloose`라는 하위 폴더를 생성합니다.
    2. `SPEAKER_NAMES`에 정의된 각 스피커 채널에 대해 다음을 반복합니다:
        a. 해당 스피커 채널의 좌측 귀(left)와 우측 귀(right) 임펄스 응답만을 포함하는 임시 HRIR 객체를 만듭니다. (다른 채널 데이터는 제외)
        b. 이 임시 HRIR 객체를 사용하여 `스피커명.wav` (예: `FL.wav`, `FC.wav`) 형식의 2채널 스테레오 WAV 파일을 `Hangloose` 폴더 내에 저장합니다.
- **참고:** 각 채널별 파일 생성 시, 원본 HRIR 세트의 정규화 상태를 따릅니다. (개별적으로 재정규화하지 않음)

-----------------------------------------------

### 8. 처리 결과 요약 (`README.md`) 자동 생성 및 정규화 게인 표시 개선

![image](https://github.com/user-attachments/assets/33840a8e-b244-4ab4-ab63-a75a406fd39c)

`--interactive_plots` CLI 옵션을 사용하면 Bokeh 기반의 인터랙티브 플롯을 HTML 파일로 생성합니다.

* **주요 기능 및 플롯 종류:**
  * **양이 응답 임펄스 오버레이 (Interaural Impulse Overlay):** 각 스피커에 대한 좌우 귀 임펄스 응답 중첩 표시.
  * **양이 레벨 차이 (ILD - Interaural Level Difference):** 주파수 대역별 양 귀 레벨 차이.
  * **양이 위상차 (IPD - Interaural Phase Difference):** 주파수 대역별 양 귀 위상차.
  * **양이 간 상호 상관 계수 (IACC - Interaural Cross-Correlation Coefficient):** 양쪽 귀 신호 간 유사성 및 공간감 지표.
  * **에너지 시간 곡선 (ETC - Energy Time Curve):** 임펄스 응답 에너지의 시간적 감쇠 특성.
  * **종합 결과 (Result Overview):** 전체 채널 종합 좌/우 귀 최종 주파수 응답 및 차이.
* **사용자 경험:**
  * 모든 인터랙티브 플롯은 단일 HTML 파일 내의 탭으로 제공되어 사용 편의성을 높였습니다. (`impulcifer.py`에서 `Tabs`와 `TabPanel` 사용)
  * 플롯 크기는 브라우저 창에 맞춰 자동 조절되어 다양한 화면에서 가독성을 확보합니다. (`hrir.py`의 `figure` 및 `gridplot`의 `sizing_mode` 조정, `impulcifer.py`의 `Tabs`에 `sizing_mode='stretch_both'` 적용)
  * 커스텀 폰트(Pretendard) 적용 및 Matplotlib 마이너스 부호 문제 해결 (`impulcifer.py`의 `set_matplotlib_font` 함수, `plt.rcParams['axes.unicode_minus'] = False` 설정)

-----------------------------------------------

### 9. 기존 Matplotlib 플롯 개선 (Seaborn Style and Custom Font)

기존 `impulse_response.py` 및 `impulcifer.py`에서 생성되던 Matplotlib 기반 플롯들의 시각적 품질을 개선했습니다.

- **Seaborn Style 적용:** `seaborn` 라이브러리를 사용하여 플롯에 전반적으로 `whitegrid` 스타일을 적용하여 가독성을 높였습니다. (Matplotlib 플롯 생성 전 `sns.set_theme(style="whitegrid")` 호출)
- **커스텀 한글 폰트(Pretendard) 적용:**
  - `Pretendard-Regular.otf` 폰트 파일을 패키지에 포함시키고 (`pyproject.toml`의 `package_data` 설정), `impulcifer.py`에서 `importlib.resources`를 사용하여 프로그램 실행 시 동적으로 로드하도록 구현했습니다. 이를 통해 시스템에 해당 폰트가 설치되어 있지 않아도 일관된 한글 표시가 가능합니다.
  - 폰트 로드 실패 시 시스템 기본 한글 폰트(Windows: Malgun Gothic, macOS: AppleGothic, Linux: NanumGothic)를 사용하도록 폴백 로직을 추가했습니다.
- **마이너스 부호 문제 해결:** Matplotlib에서 유니코드 마이너스 부호가 깨지는 현상을 방지하기 위해 `plt.rcParams['axes.unicode_minus'] = False` 설정을 전역적으로 적용했습니다.

-----------------------------------------------

### 10. 주요 기능 및 플롯 종류 개선

* **주요 기능 및 플롯 종류:**
  * **양이 응답 임펄스 오버레이 (Interaural Impulse Overlay):** 각 스피커에 대한 좌우 귀 임펄스 응답 중첩 표시.
  * **양이 레벨 차이 (ILD - Interaural Level Difference):** 주파수 대역별 양 귀 레벨 차이.
  * **양이 위상차 (IPD - Interaural Phase Difference):** 주파수 대역별 양 귀 위상차.
  * **양이 간 상호 상관 계수 (IACC - Interaural Cross-Correlation Coefficient):** 양쪽 귀 신호 간 유사성 및 공간감 지표.
  * **에너지 시간 곡선 (ETC - Energy Time Curve):** 임펄스 응답 에너지의 시간적 감쇠 특성.
  * **종합 결과 (Result Overview):** 전체 채널 종합 좌/우 귀 최종 주파수 응답 및 차이.
* **사용자 경험:**
  * 모든 인터랙티브 플롯은 단일 HTML 파일 내의 탭으로 제공되어 사용 편의성을 높였습니다. (`impulcifer.py`에서 `Tabs`와 `TabPanel` 사용)
  * 플롯 크기는 브라우저 창에 맞춰 자동 조절되어 다양한 화면에서 가독성을 확보합니다. (`hrir.py`의 `figure` 및 `gridplot`의 `sizing_mode` 조정, `impulcifer.py`의 `Tabs`에 `sizing_mode='stretch_both'` 적용)
  * 커스텀 폰트(Pretendard) 적용 및 Matplotlib 마이너스 부호 문제 해결 (`impulcifer.py`의 `set_matplotlib_font` 함수, `plt.rcParams['axes.unicode_minus'] = False` 설정)

-----------------------------------------------

### 11. 기존 Matplotlib 플롯 개선 (Seaborn 스타일 및 커스텀 폰트 적용)

* **주요 기능 및 플롯 종류:**
  * **양이 응답 임펄스 오버레이 (Interaural Impulse Overlay):** 각 스피커에 대한 좌우 귀 임펄스 응답 중첩 표시.
  * **양이 레벨 차이 (ILD - Interaural Level Difference):** 주파수 대역별 양 귀 레벨 차이.
  * **양이 위상차 (IPD - Interaural Phase Difference):** 주파수 대역별 양 귀 위상차.
  * **양이 간 상호 상관 계수 (IACC - Interaural Cross-Correlation Coefficient):** 양쪽 귀 신호 간 유사성 및 공간감 지표.
  * **에너지 시간 곡선 (ETC - Energy Time Curve):** 임펄스 응답 에너지의 시간적 감쇠 특성.
  * **종합 결과 (Result Overview):** 전체 채널 종합 좌/우 귀 최종 주파수 응답 및 차이.
* **사용자 경험:**
  * 모든 인터랙티브 플롯은 단일 HTML 파일 내의 탭으로 제공되어 사용 편의성을 높였습니다. (`impulcifer.py`에서 `Tabs`와 `TabPanel` 사용)
  * 플롯 크기는 브라우저 창에 맞춰 자동 조절되어 다양한 화면에서 가독성을 확보합니다. (`hrir.py`의 `figure` 및 `gridplot`의 `sizing_mode` 조정, `impulcifer.py`의 `Tabs`에 `sizing_mode='stretch_both'` 적용)
  * 커스텀 폰트(Pretendard) 적용 및 Matplotlib 마이너스 부호 문제 해결 (`impulcifer.py`의 `set_matplotlib_font` 함수, `plt.rcParams['axes.unicode_minus'] = False` 설정)

-----------------------------------------------
