# 2.x와 3.x 비교 측정 (Linux, README용)

README의 '2.x와 3.x 측정값' 표를 만든 측정입니다. 사용자가 받는 배포물 그대로(PyPI 휠과 GitHub Release의 AppImage) 두 버전을 같은 머신에서 나란히 돌렸습니다. 크레이트별 성능 감사(`docs/rust/perf/*.md`의 다른 문서)와는 따로 한 측정이며, `features.toml`의 `perf.*` 항목과 관계가 없습니다.

## 환경

| 항목 | 값 |
| --- | --- |
| 날짜 | 2026-10-01 |
| 머신 | 클라우드 컨테이너, Intel Xeon 4 vCPU, 메모리 16 GiB |
| OS | Ubuntu 24.04.4 LTS, 커널 6.18 |
| Python | CPython 3.13.14 (GIL 있음) |
| 2.x | PyPI `impulcifer-py313==2.14.3` (`py3-none-any` 휠과 의존 패키지 30개), GitHub Release `v2.14.3`의 `Impulcifer-2.14.3-x86_64.AppImage` |
| 3.x | PyPI `impulcifer-py313==3.0.5` (`cp39-abi3-manylinux_2_28_x86_64` 휠), GitHub Release `v3.0.5`의 `Impulcifer-3.0.5-x86_64.AppImage` |
| 화면 | Xvfb 1440×960×24, GPU 없음(소프트웨어 렌더링), 시스템 WebKitGTK 2.52.6 |

2.x AppImage는 WebKitGTK를 담지 않고 시스템 것을 쓰므로 `libwebkit2gtk-4.1-0`과 `gir1.2-webkit2-4.1`을 설치했습니다. 3.x AppImage는 WebKitGTK를 안에 담고 있어, 시스템에는 `libEGL.so.1`만 더 필요했습니다. 두 AppImage는 `--appimage-extract`로 풀어 `AppRun`을 실행했습니다.

다른 작업이 함께 도는 공유 머신이라 시간 값의 편차가 큽니다(같은 조건에서 3.x 기본 처리가 1.25~3.56초). 그래서 여러 번 돌린 중앙값을 씁니다. 메모리 값은 회차 사이의 편차가 작습니다.

## 측정 방법

모든 측정은 `tests/migration/bench_2x_vs_3x.py`로 했습니다. 메모리는 `/proc`에서 읽은 PSS(여러 프로세스가 함께 쓰는 페이지를 나눠 센 값)를 프로세스 트리 전체에 더한 값입니다. 2.x는 처리할 때 작업 프로세스를 fork하는데, 이 프로세스들은 부모와 페이지를 많이 공유하므로 RSS를 더하면 같은 페이지를 여러 번 세게 됩니다(2.x 기본 처리의 RSS 합은 약 1,450 MiB로, PSS 합의 세 배가 넘습니다).

### 설치 크기

GitHub Release와 PyPI의 파일 크기는 각 API가 알려 주는 바이트 수를 MiB로 바꾼 값입니다. 설치 후 크기는 빈 가상 환경에 `uv pip install "impulcifer-py313==<버전>"`으로 설치한 뒤 `du -sm`으로 잰 `site-packages`의 크기이고, 패키지 수는 `uv pip list`의 줄 수입니다.

| 파일 | 2.14.3 | 3.0.5 |
| --- | ---: | ---: |
| `Impulcifer-win-Setup.exe` | 209.1 MiB | 25.8 MiB |
| `Impulcifer-win-Portable.zip` | 201.9 MiB | 18.6 MiB |
| `Impulcifer-<버전>-full.nupkg` (Velopack 전체 패키지) | 202.0 MiB | 18.7 MiB |
| `Impulcifer-<버전>-macOS.dmg` | 182.5 MiB | 17.1 MiB |
| `Impulcifer-<버전>-x86_64.AppImage` | 261.0 MiB | 92.6 MiB |
| `Impulcifer-2.14.3-linux-x86_64.tar.gz` | 260.6 MiB | 없음 |
| PyPI 휠 (Linux) | 67.8 MiB | 12.1 MiB |
| PyPI 휠 (Windows) | 67.8 MiB (공통 휠) | 12.0 MiB |
| PyPI 휠 (macOS) | 67.8 MiB (공통 휠) | 11.5 MiB |
| 설치 후 `site-packages` | 459 MiB, 패키지 31개 | 30 MiB, 패키지 1개 |
| AppImage를 푼 크기 | 689 MiB | 290 MiB |

### 처리 시간과 최대 메모리

`data/demo`를 복사한 폴더 두 개(버전마다 하나)에 대해 PyPI로 설치한 `impulcifer` 명령을 실행했습니다. 2.x와 3.x를 한 번씩 번갈아 돌렸습니다.

```bash
python tests/migration/bench_2x_vs_3x.py run v3-default -- venv-3.0.5/bin/impulcifer --dir_path d3 --test_signal default
python tests/migration/bench_2x_vs_3x.py run v2-default -- venv-2.14.3/bin/impulcifer --dir_path d2 --test_signal default
```

| 시나리오 | 버전 | 회수 | 시간(초) | 시간 중앙값 | 최대 PSS 합(MiB) 중앙값 | 프로세스 수 |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| `--test_signal default` | 2.14.3 | 12 | 6.72, 6.41, 6.89, 5.53, 4.54, 7.05, 4.88, 7.72, 5.86, 6.30, 4.87, 5.45 | 6.08 | 428 | 5 |
| `--test_signal default` | 3.0.5 | 12 | 1.87, 2.59, 2.80, 3.56, 1.86, 2.09, 2.06, 1.68, 1.67, 2.58, 1.35, 1.35 | 1.97 | 389 | 1 |
| `--vbass --vbass_freq 250` | 2.14.3 | 5 | 4.68, 6.65, 4.96, 5.19, 7.82 | 5.19 | 428 | 5 |
| `--vbass --vbass_freq 250` | 3.0.5 | 5 | 2.15, 1.46, 1.50, 1.55, 1.86 | 1.55 | 391 | 1 |
| 테스트 신호 자동 감지 | 2.14.3 | 7 | 5.72, 5.26, 5.35, 6.58, 5.17, 4.42, 6.20 | 5.35 | 478 | 5 |
| 테스트 신호 자동 감지 | 3.0.5 | 7 | 2.57, 1.66, 2.58, 1.59, 1.64, 1.25, 1.97 | 1.66 | 389 | 1 |
| `--plot` | 2.14.3 | 5 | 56.76, 56.68, 52.87, 53.94, 51.26 | 53.94 | 3,214 | 6 |
| `--plot` | 3.0.5 | 5 | 3.60, 4.91, 3.67, 3.92, 4.46 | 3.92 | 671 | 1 |

`--plot`을 주면 두 버전 모두 같은 하위 폴더(`room`, `pre`, `post`, `ild`, `ipd`, `iacc`, `interaural_overlay`, `etc`)에 같은 수(55장)의 PNG를 저장합니다.

README의 '기본 처리' 행은 위의 세 시나리오(`--plot` 제외)를 합친 24회의 중앙값입니다. 2.x는 5.63초·453 MiB, 3.x는 1.86초·389 MiB입니다. 두 버전 모두 `--plot` 없이도 `plots/headphones.png`와 `plots/results.png` 두 장을 저장합니다.

2.x의 `--plot`에서 프로세스 하나의 최대 RSS(VmHWM)는 약 1,779 MiB였고, 그래프를 병렬로 그리는 작업 프로세스들이 함께 떠 있어 트리 전체의 PSS 합이 3.2 GiB까지 올라갑니다.

실행만 하고 끝나는 비용은 `impulcifer --version`을 5회씩 돌려 쟀습니다. 이 명령은 20 ms 간격 표본으로는 메모리를 잡기 어려울 만큼 짧아서, 자식 프로세스의 `ru_maxrss`(최대 RSS)를 썼습니다.

| 버전 | 시간(초) | 최대 RSS |
| --- | --- | ---: |
| 2.14.3 | 0.98, 0.99, 1.03, 1.20, 1.28 | 125 MiB |
| 3.0.5 | 0.020, 0.018, 0.018, 0.023, 0.018 | 13 MiB |

### 앱을 켜 두었을 때의 메모리

AppImage를 풀어 Xvfb 화면에 띄우고, 30초 기다린 뒤 15초 동안 1초마다 잰 값의 중앙값입니다. 두 앱 모두 새 `HOME`에서 처음 실행했으므로 3.x는 언어 선택 창을, 2.x는 업데이트 안내 창(2.14.3 → 3.0.5)을 띄운 상태였습니다. 스크린샷으로 두 앱 모두 화면이 그려진 것을 확인했습니다.

```bash
HOME=$PWD/home-3.0.5 dbus-run-session -- python tests/migration/bench_2x_vs_3x.py gui v3-gui :91 30 15 v3.png -- ex-3.0.5/squashfs-root/AppRun
HOME=$PWD/home-2.14.3 dbus-run-session -- python tests/migration/bench_2x_vs_3x.py gui v2-gui :91 30 15 v2.png -- ex-2.14.3/squashfs-root/AppRun
```

| 회차 | 버전 | 앱 프로세스 PSS | WebKitWebProcess PSS | WebKitNetworkProcess PSS | 합계 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | 2.14.3 | 339.2 | 346.1 | 26.2 | 711.6 |
| 2 | 2.14.3 | 333.6 | 310.8 | 25.6 | 670.0 |
| 3 | 2.14.3 | 333.3 | 307.2 | 25.5 | 666.1 |
| 1 | 3.0.5 | 110.4 | 318.1 | 22.0 | 450.4 |
| 2 | 3.0.5 | 100.6 | 306.5 | 22.2 | 429.3 |
| 3 | 3.0.5 | 100.3 | 273.4 | 22.1 | 395.9 |

단위는 MiB입니다. 합계 열은 15초 동안의 합계 중앙값이고, 프로세스별 열은 마지막 표본의 값이라 더해도 합계 열과 조금 다릅니다. README에는 세 회차의 중앙값(앱 프로세스 2.x 334 MiB, 3.x 101 MiB, 합계 2.x 670 MiB, 3.x 429 MiB)을 적었습니다.

앱 프로세스의 익명 메모리(파일에 매핑되지 않은 힙 등, `Pss_Anon`)만 보면 2.x가 약 143 MiB, 3.x가 약 39 MiB입니다. 나머지는 공유 라이브러리와 실행 파일을 매핑한 페이지입니다. 2.x 앱은 첫 녹음이나 처리가 라이브러리 로딩 때문에 몇 초씩 멈추지 않도록 시작하자마자 SciPy·Matplotlib·Bokeh 등을 백그라운드에서 불러 두므로(`application/impulcifer_service.py`의 `_start_dsp_prewarm`), 처리하지 않고 켜 두기만 해도 이 라이브러리들이 메모리에 올라와 있습니다.

웹 엔진 프로세스 두 개는 두 버전 모두 같은 WebKitGTK이고 크기도 비슷합니다. 이 측정은 GPU가 없는 가상 화면에서 소프트웨어로 그렸으므로, 실제 데스크톱에서 웹 엔진 프로세스의 크기는 다를 수 있습니다.

## 이 측정이 다루지 않는 것

Windows와 macOS의 앱 메모리, 녹음 중의 메모리, 앱 안에서 BRIR을 만드는 동안의 메모리는 재지 않았습니다. 같은 데모 처리를 Windows(Intel Core i5-12600KF)에서 잰 결과는 [release.md](release.md) 4절에 있습니다. 그 측정에서 3.x는 일반 Python 3.14의 2.x보다 11.6배, free-threaded Python 3.14t의 2.x보다 5.0배 빨랐습니다.
