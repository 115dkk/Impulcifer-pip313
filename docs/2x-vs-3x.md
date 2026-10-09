# 2.x와 3.x 측정값

아래 값은 2026-10-01에 Linux 컨테이너 하나(Intel Xeon 4 vCPU, 메모리 16 GiB, Ubuntu 24.04)에서 2.14.3과 3.0.5를 나란히 돌려 잰 것입니다. 두 버전 모두 PyPI 휠과 GitHub Releases의 파일을 그대로 썼고, CLI는 Python 3.13에 설치해 실행했습니다. 메모리는 측정한 프로세스와 그 자식 프로세스들의 PSS(여러 프로세스가 함께 쓰는 메모리를 나눠 센 값)를 더한 값이며, 단위는 MiB입니다. 측정 방법과 회차별 값은 [docs/rust/perf/2x-vs-3x-linux.md](rust/perf/2x-vs-3x-linux.md)에 있습니다.

## 설치 크기

| 항목 | 2.14.3 | 3.0.5 |
| --- | ---: | ---: |
| Windows 설치 파일 (`Impulcifer-win-Setup.exe`) | 209.1 MiB | 25.8 MiB |
| Windows 포터블 (`Impulcifer-win-Portable.zip`) | 201.9 MiB | 18.6 MiB |
| macOS 디스크 이미지 (`.dmg`) | 182.5 MiB | 17.1 MiB |
| Linux AppImage | 261.0 MiB | 92.6 MiB |
| PyPI 휠 (Linux x86_64) | 67.8 MiB | 12.1 MiB |
| `pip install` 뒤의 `site-packages` | 459 MiB (패키지 31개) | 30 MiB (패키지 1개) |

Linux AppImage는 두 버전이 담는 내용이 다릅니다. 3.x AppImage는 웹 엔진(WebKitGTK)을 안에 넣었고, 2.x AppImage는 시스템에 설치된 WebKitGTK를 썼습니다.

## 처리 시간과 메모리

`data/demo`의 녹음으로 BRIR을 만드는 데 걸린 시간과 그동안의 최대 메모리입니다. 기본 처리 행은 기본 설정, `--vbass`, 테스트 신호 자동 감지 세 경우를 합친 24회의 중앙값이고, 나머지 행은 5회의 중앙값입니다.

| 작업 | 2.14.3 | 3.0.5 |
| --- | ---: | ---: |
| 기본 처리 (요약 그래프 2장 포함) | 5.6초, 453 MiB | 1.9초, 389 MiB |
| `--plot` (그래프 55장 저장) | 53.9초, 3,214 MiB | 3.9초, 671 MiB |
| `impulcifer --version` (실행 후 바로 종료) | 1.0초, 125 MiB | 0.02초, 13 MiB |

`--version` 행의 메모리는 프로세스 하나의 최대 RSS입니다. 2.x는 명령을 실행하자마자 NumPy·SciPy 등을 불러오므로, 아무 처리를 하지 않아도 1초가 걸리고 125 MiB를 씁니다. 기본 처리에서 2.x는 프로세스 5개(본 프로세스와 작업 프로세스 4개)로 일을 나누고, 3.x는 한 프로세스 안의 스레드로 나눕니다. 기본 처리의 최대 메모리는 두 버전이 비슷하고, 메모리 차이는 그래프를 모두 저장할 때 크게 벌어집니다.

같은 데모 처리를 Windows(Intel Core i5-12600KF)에서 잰 기록은 [docs/rust/perf/release.md](rust/perf/release.md)에 있습니다. 그 측정에서 3.x는 일반 Python 3.14에서 돌린 2.x보다 11.6배, free-threaded Python 3.14t에서 돌린 2.x보다 5.0배 빨랐습니다.

## 앱을 켜 두었을 때의 메모리

두 버전의 Linux AppImage를 가상 화면(Xvfb)에 띄우고 30초 기다린 뒤, 15초 동안 잰 값의 중앙값입니다. 세 번 띄운 결과의 중앙값을 적었습니다.

| 항목 | 2.14.3 | 3.0.5 |
| --- | ---: | ---: |
| 앱 프로세스 | 334 MiB | 101 MiB |
| 웹 엔진 프로세스까지 더한 전체 | 670 MiB | 429 MiB |

두 버전 모두 같은 웹 엔진(WebKitGTK)으로 화면을 그리므로, 차이는 거의 앱 프로세스에서 납니다. 2.x 앱은 첫 처리가 라이브러리 로딩으로 멈추지 않도록 시작하자마자 SciPy·Matplotlib 등을 미리 불러 두므로, 처리하지 않고 켜 두기만 해도 이 라이브러리들이 메모리를 차지합니다. 이 측정은 GPU가 없는 가상 화면에서 했으므로, 실제 데스크톱에서는 웹 엔진 프로세스의 크기가 다를 수 있습니다.
