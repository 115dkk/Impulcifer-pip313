# Python 3.14 및 Nuitka 빌드 메모

이 문서는 Python 3.14 지원, free-threaded 런타임 감지, Nuitka standalone 빌드 경로를 현재 코드 기준으로 정리합니다. 고정된 성능 배속은 적지 않습니다. 입력 데이터, 채널 수, CPU, 디스크 I/O, Python 빌드 방식에 따라 결과가 달라집니다.

## 현재 기준

| 항목 | 현재 상태 | 기준 코드 |
| --- | --- | --- |
| Python 테스트 범위 | CI에서 Python 3.9부터 3.14까지 확인합니다. | `.github/workflows/test.yml` |
| Nuitka 릴리스 빌드 | 일반 CPython 3.14와 `nuitka>=4.1`을 씁니다. | `.github/workflows/build-linux.yml`, `.github/workflows/build-macos.yml`, `.github/workflows/release-cross-platform.yml` |
| free-threaded standalone | 대상에서 제외합니다. `--disable-gil`, `3.14t`를 쓰지 않습니다. | `tests/test_build_config.py` |
| Nuitka 플래그 | `build_scripts/nuitka_flags.py`가 단일 기준입니다. | `build_scripts/build_nuitka.py`, `build_scripts/nuitka_flags.py` |
| 런타임 병렬 정책 | GIL 비활성화 여부를 보고 ThreadPool 또는 ProcessPool을 고릅니다. | `core/parallel_utils.py` |

## Python 3.14 지원 범위

Python 3.14는 패키지 테스트와 standalone 빌드에서 지원합니다. 다만 standalone 릴리스 빌드는 free-threaded Python을 쓰지 않습니다. Nuitka의 free-threaded 지원이 충분히 안정화되기 전까지 일반 CPython 3.14를 기준으로 둡니다.

현재 프로젝트가 Python 3.14 기능을 직접 활용하는 부분은 런타임 감지입니다.

```python
import sys

if hasattr(sys, "_is_gil_enabled"):
    gil_enabled = sys._is_gil_enabled()
```

이 감지는 Python 3.13 이상의 free-threaded 빌드에서도 동작합니다. 그래서 코드는 "Python 3.14 전용 최적화"가 아니라 "GIL 비활성화 런타임 대응"으로 보는 편이 맞습니다.

## 병렬 처리 정책

`core.parallel_utils`는 BRIR 처리 중 무거운 작업에 쓰는 기본 병렬 유틸입니다.

| 런타임 | executor | 이유 |
| --- | --- | --- |
| GIL이 켜진 일반 Python | `ProcessPoolExecutor` | CPU 작업에서 GIL을 피합니다. |
| GIL이 꺼진 free-threaded Python | `ThreadPoolExecutor` | pickle 비용 없이 스레드 병렬 실행을 씁니다. |

현재 병렬 처리와 메모리 절감이 들어간 경로는 다음입니다.

| 경로 | 현재 동작 |
| --- | --- |
| Equalization worker | `parallel_map()`을 쓰고, room/headphone/EQ/target 공통 데이터는 worker initializer로 한 번 전달합니다. task에는 `(speaker, side)`만 넘깁니다. |
| Decay 조정 | 스피커/귀별 task를 `parallel_map()`으로 처리합니다. |
| Normalize | 스피커가 5개 이상이면 `parallel_process_dict(..., use_threads=True)`로 gain 적용을 나눕니다. 일반 Python에서는 CPU-bound 이득이 제한될 수 있습니다. |
| Resample | 스피커가 5개 이상이면 `parallel_process_dict(..., use_threads=True)`로 스피커별 리샘플링을 나눕니다. |
| Plot용 convolution | ProcessPool 비용과 메모리 잔류가 커서 직렬 처리로 유지합니다. |
| `HRIR.write_wav()` | `track_order`가 있으면 요청된 채널만 쌓습니다. JamesDSP, Hangloose, TrueHD 부분 출력에서 전체 HRIR을 먼저 쌓지 않습니다. |
| JamesDSP 출력 | `HRIR.subset(["FL", "FR"], copy_irs=True)`로 필요한 네 IR만 복사합니다. |
| Hangloose 출력 | HRIR 전체 deep copy 없이 `write_wav(track_order=...)`로 스피커별 파일을 씁니다. |

## 런타임 확인

설치된 명령으로 확인할 수 있습니다.

```bash
impulcifer --info
```

병렬 정책만 확인하려면 다음 코드를 실행합니다.

```bash
python -c "from core.parallel_utils import get_parallelization_info; print(get_parallelization_info())"
```

`executor_type`이 `ThreadPoolExecutor`이면 현재 런타임에서 GIL이 꺼져 있다고 판단한 것입니다. 일반 Python에서는 `ProcessPoolExecutor`가 나옵니다.

## Nuitka standalone 빌드

릴리스 빌드는 standalone folder 방식을 씁니다. onefile 모드는 쓰지 않습니다.

공통 플래그는 `build_scripts/nuitka_flags.py`에서 만듭니다.

주요 기준은 다음입니다.

| 항목 | 값 |
| --- | --- |
| 모드 | `--standalone` |
| 출력 정리 | `--remove-output` |
| jobs 기본값 | `--jobs=4` |
| LTO | `--lto=no` |
| 플러그인 | `tk-inter`, `matplotlib` |
| 포함 데이터 | `data`, `font`, `img`, `logo`, `i18n/locales`, `gui/theme`, `LICENSE`, `README.txt` |
| 명시 포함 모듈 | `scipy.*`, `bokeh`, `core.parallel_workers`, `infra._build_info` |

로컬에서 빌드하려면 다음 명령을 씁니다.

```bash
python build_scripts/build_nuitka.py
```

플래그만 확인하려면 다음 명령을 쓸 수 있습니다.

```bash
python -m build_scripts.nuitka_flags --platform linux --version 0.0.0
```

## 성능 문서 기준

예전 문서에는 특정 CPU에서 몇 배 빨라졌다는 벤치마크 표가 있었습니다. 현재 문서에서는 그 수치를 제거했습니다. 프로젝트 안에 그런 숫자를 계속 보장하는 회귀 테스트가 없고, 현재 성능 개선은 여러 작은 경로로 나뉘어 있기 때문입니다.

앞으로 성능 수치를 문서에 넣으려면 다음 조건을 같이 남겨야 합니다.

- 사용한 commit 해시
- Python 버전과 GIL 상태
- NumPy, SciPy, nnresample 버전
- CPU, RAM, 저장 장치
- 측정 데이터의 채널 수와 샘플레이트
- 실행 명령과 반복 횟수
- 비교 기준이 되는 commit 또는 릴리스

숫자를 재현할 수 없으면 changelog에는 "어떤 경로를 줄였는지"만 적고, 배속 표현은 피합니다.

## free-threaded Python 사용 시 주의

- PyPI 패키지 실행에서는 free-threaded Python을 직접 쓸 수 있습니다.
- standalone 릴리스는 일반 CPython 3.14 기준입니다.
- no-GIL 런타임에서 ThreadPool 경로는 pickle 비용을 줄일 수 있지만, 모든 작업이 빨라진다고 보장할 수는 없습니다.
- NumPy와 SciPy 내부 구현, BLAS 설정, 배열 크기에 따라 결과가 달라집니다.

## 관련 테스트

빌드 설정 drift는 다음 테스트가 막습니다.

```bash
pytest tests/test_build_config.py tests/test_nuitka_flags.py -q
```

병렬 유틸 동작은 다음 테스트가 다룹니다.

```bash
pytest tests/test_parallel_processing.py tests/test_hrir_outputs.py -q
```
