# PA06b: 출시 전 성능 감사 2차

> **2026-09-09 소유자 승인: M5 통과.** 아래 보고서의 "미통과" 판정은 오디오 headphones wall time 예외(PortAudio보다 15~17 ms, 0.25~0.33% 느림, 40 ms shared 버퍼의 대가)를 소유자가 명시적으로 승인하면서 해소됐다. 등록부의 `perf.release`·`perf.impulcifer-audio-io`·`perf.impulcifer-sys-win`이 구현으로 바뀌었다. 나머지 수치(파이프라인 11.6~12.0배/5.0~5.1배, 크레이트별 1.0 이상, 워크스페이스 테스트 379 통과)는 그대로다.

2026-09-08. 부모 세션과 측정 워커가 시작한 HEAD는 `e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8`입니다. 측정은 16:26~17:43(KST)에 끝났으며, 이 문서는 이미 남아 있는 `E:/Impulcifer/target/pa06b/*.log`와 `aggregates.json`을 대조해 작성했습니다. 문서를 작성하면서 명령·벤치·테스트·프로세스를 새로 실행하지 않았습니다.

**최종 판정은 M5 미통과입니다.** 플롯을 포함한 default/vbass 파이프라인은 일반·FT Python보다 빠릅니다. IO·DSP·service의 집계 비율도 모두 1.0 이상이고, CRLF 체크아웃의 workspace 테스트는 379 passed / 0 failed / 10 ignored입니다. 하지만 명시적 오디오의 wall·overhead·첫 샘플 전달은 여전히 1.0 미만입니다. production 녹음도 모든 항목을 통과하지 못했습니다. PA05b의 독립 녹음 무결성 미달은 그대로 남아 있습니다.

FFT는 과거 커밋과 같은 시간대에 비교해 Rust 지연 증가가 없음을 확인했습니다. Service는 scratch 빌드의 최초 지연 증가 때문에 실제 `git bisect`를 수행했지만, 문서만 바꾼 HEAD를 원인으로 판정한 결과는 재확인에서 성립하지 않았습니다. **소스 회귀를 찾거나 수정했다고 주장하지 않습니다.** 최초 scratch 값의 변동 원인은 미확정입니다.

수정한 추적 파일은 이 보고서 하나입니다. `features.toml`, CHANGELOG, 크레이트 소스, 벤치, 오라클, 골든, 허용 오차, 의존성은 바꾸지 않았습니다. 기존 PA06 본문은 제목과 부록까지 문서 끝의 `## First run (2026-09-08)` 아래에 그대로 보존했습니다. 과거 본문의 실패·미실행 서술은 당시 결과이며 이번 결과와 구분해야 합니다.

## 1. 환경과 비교 방법

| 항목 | 확인한 환경 |
|---|---|
| CPU / OS | Intel Core i5-12600KF, 물리 10 / 논리 16코어; Windows 11 Education 10.0.22621, x86-64 |
| Rust / Cargo | rustc 1.97.0 (2d8144b78 2026-07-07), LLVM 22.1.6, MSVC; Cargo 1.97.0; release/bench 최적화 빌드 |
| 일반 Python | `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`; 3.14.5 (tags/v3.14.5:5607950, May 10 2026, 10:43:50), MSC v.1944 AMD64; GIL True |
| FT Python | `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`; 3.14.7 free-threading build (main, Sep 1 2026, 14:18:33), MSC v.1944 AMD64; GIL False |
| NumPy / SciPy | 두 환경 모두 2.5.3 / 1.18.1 |
| soundfile / libsndfile | 0.14.0 / 1.2.2 (IO 오라클 환경 출력) |
| sounddevice / PortAudio | 0.5.6 / `(1246976, 'PortAudio V19.7.0-devel, revision unknown')` |
| NumPy BLAS/LAPACK | scipy-openblas 0.3.34.106.0, ILP64 (`USE64BITINT`), DYNAMIC_ARCH, NO_AFFINITY, Haswell, MAX_THREADS=24 |
| SciPy BLAS/LAPACK | scipy-openblas 0.3.31.dev, LP64 (`has ilp64: false`), DYNAMIC_ARCH, NO_AFFINITY, Haswell, MAX_THREADS=24 |
| FFT | NumPy pocketfft; SciPy `_basic_backend.py`가 실제로 임포트한 `scipy.fft._duccfft` |
| 일반 SciPy 파일 | `C:/Users/32170336/AppData/Local/Programs/Python/Python314/Lib/site-packages/scipy/fft/_basic_backend.py`, 같은 디렉터리의 `_duccfft/__init__.py` |
| FT SciPy 파일 | `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Lib/site-packages/scipy/fft/_basic_backend.py`, 같은 디렉터리의 `_duccfft/__init__.py` |
| 스레드 변수 | OMP_NUM_THREADS, MKL_NUM_THREADS, OPENBLAS_NUM_THREADS, NUMEXPR_NUM_THREADS, BLIS_NUM_THREADS, RAYON_NUM_THREADS 모두 미설정 |
| 빌드·벤치 변수 | RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, IMPULCIFER_PERF_DIR, IMPULCIFER_PA05_SHARED_PERIODS 미설정. 본 트리는 CARGO_TARGET_DIR 미설정, scratch는 아래 지정 디렉터리 사용 |
| 로그 실행기 | PYTHONIOENCODING=utf-8, PYTHONDONTWRITEBYTECODE=1; 명령마다 argv/cwd/start/environment 및 END/EXIT 기록 |
| 본 작업 디렉터리 | `E:/Impulcifer` |
| detached worktree | `C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect` |
| scratch 빌드 디렉터리 | `C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect-target` |

**other processes were not queried; caller kept machine quiet.** 측정 워커는 다른 프로세스를 조회하거나 종료하지 않았고 명령을 포그라운드에서 순차 실행했습니다. 두 환경 로그는 프로세스를 조회하지 않는 `seventh_environment.py`의 결과입니다. PA05b 과거 보고서의 프로세스 조회 위반과 이번 실행을 혼동하지 않습니다. CPU와 Rust 도구 체인은 이번 `pipeline.log`의 PA03_ENV에도 확인되며 Cargo의 정확한 빌드는 `cargo 1.97.0 (c980f4866 2026-06-30)`입니다. 두 Python의 psutil은 7.2.2, SciPy FFT 기본 worker 수는 1, EQ worker 수는 14입니다. 이는 스레드 환경 변수를 1로 강제한 결과가 아닙니다. 일반·FT 환경 로그로 Python·수치 라이브러리·FFT·스레드 설정도 다시 확인했습니다. 오라클의 관례적인 SciPy pocketfft 표시는 실제 `_duccfft` 임포트와 달라 채택하지 않았습니다.

### 집계 규칙과 입력

- `ratio = Python median / Rust median`이며 1.0 이상이어야 합니다. 이전 크레이트 보고서 비율의 80% 미만도 조사 대상입니다. PA05의 0.2% 오디오 허용치를 PA06의 1.0 기준 대신 쓰지 않습니다.
- IO·DSP·service는 워밍업 3회 후 측정 11회의 median/min을 사용했습니다. 조사 대상 다섯 연산은 새 프로세스 세 차례의 각 median을 모아, **Python median들의 median / Rust median들의 median**으로 집계했습니다. 세 비율의 median과 다르며 가장 유리한 값을 고른 것도 아닙니다.
- IO 일반·DSP 전체 표는 세 실행을 같은 방식으로 집계했습니다. IO FT는 한 오라클 실행입니다. IO의 Rust 분모는 각 오라클이 같은 fixture에서 호출한 Rust 결과이며 standalone 결과를 대신 넣지 않았습니다.
- Service 일반은 일반 오라클 한 실행과 `service-rust-1`을 비교합니다. Service FT는 세 Rust·FT 실행의 median을 각각 집계합니다. 따라서 일반·FT의 Rust 분모가 다를 수 있습니다.
- IO 입력은 32×96000, 30×96000, 2×295000, 번들 sweep 1×295270, demo 2×878540, float32 8×480000, 파일명 10000개입니다. DSP의 모든 크기, service의 batch 크기는 부록 표에 보존했습니다.
- 파이프라인은 Rust 워밍업 1회+측정 5회, Python 각 시나리오·모드·인터프리터 워밍업 1회+측정 3회입니다. 양쪽 플롯을 포함합니다. 메모리는 기존 psutil의 peak working set과 측정 자식·후손의 RSS 표본으로 얻었습니다.
- 오디오는 headphones 295270프레임(6.151458333333초) 5회, seven 2066890프레임(43.060208333333초) 3회, 첫 샘플 10회, 열거·duplex open/close 20 calls/batch입니다. 명시적/production과 일반/FT를 모두 별도로 실행했습니다. Rust 비교 분모는 CPU ACK observer 실행 전체로 고정하고 standalone은 따로 보존했습니다.

## 2. 크레이트별 전체 비율

이전 크레이트 열은 PA01의 최신 일반/FT 추가 측정, PA02·PA04의 최종 표를 사용합니다. `일반 / FT` 순서이며, ‘통과’는 이번 집계 시간 기준의 판정입니다. 개별 실행의 미달이나 원인 미확정을 없던 일로 처리하지 않습니다.

### 2.1. impulcifer-io

| 연산 | 크레이트 보고서 일반 / FT | PA06 1차 일반 / FT | PA06b 2차 일반 / FT | 판정 |
|---|---:|---:|---:|---|
| write_pcm32_32tracks | 2.26 / 2.11 | 2.248870 / 2.002853 | 2.172899 / 2.191555 | 통과 |
| write_pcm32_30tracks | 2.63 / 2.47 | 2.520842 / 2.413090 | 2.431777 / 2.505104 | 통과 |
| write_pcm16_2tracks | 4.05 / 3.33 | 3.580682 / 3.748897 | 3.739434 / 3.531891 | 통과 |
| read_pcm32_32tracks | 1.70 / 1.29 | 1.279369 / 1.346057 | 1.406553 / 1.382235 | 집계 통과, 일반 감소 17.262% |
| read_bundled_sweep | 1.47 / 1.18 | 1.023646 / 1.224123 | 1.224805 / 1.521618 | 집계 통과, 일반 첫 실행 0.908921 |
| read_demo_recording | 1.26 / 1.49 | 1.372607 / 1.456057 | 1.384798 / 1.363420 | 통과 |
| read_float32_8tracks | 1.31 / 1.39 | 1.399126 / 1.406850 | 1.368196 / 1.374577 | 통과 |
| pcm32_round_trip | 4.79 / 4.49 | 4.640081 / 4.073999 | 4.661953 / 4.100200 | 통과 |
| sweep_file_name_parse | 1.29 / 1.28 | 1.340598 / 1.132295 | 1.337953 / 1.141646 | 통과, 작업 의미 차이 주의 |

Python 읽기는 interleaved 배열의 transposed view, Rust는 독립 track 벡터를 반환합니다. 파일명 비교도 Python 부분 regex 검색 대 Rust 전체 필드 검증이므로 완전히 같은 작업은 아닙니다. 기존 감사 방법은 유지했습니다.

### 2.2. impulcifer-dsp

| 연산 | 크레이트 보고서 | PA06 1차 | PA06b 2차 | 판정 |
|---|---:|---:|---:|---|
| convolve_full_ir_fir | 1.800 | 1.877551 | 1.719323 | 통과 |
| convolve_same_estimate | 1.840 | 1.896370 | 1.893729 | 통과 |
| correlate_full_30ms | 9.867 | 9.859143 | 9.914876 | 통과 |
| rfft_irfft_96000 | 2.021 | 1.563819 | 1.601840 | 시간 통과, 20.740% 감소 조사 수행 |
| magnitude_response_96000 | 3.508 | 2.921079 | 2.818870 | 통과 |
| butter8_sosfilt_96000 | 1.415 | 1.471577 | 1.430038 | 통과 |
| firwin2_19200 | 2.187 | 2.252274 | 2.104755 | 통과 |
| minimum_phase_19200 | 1.269 | 1.232030 | 1.250146 | 통과 |
| savgol_heavy_light_783 | 17.858 | 18.174107 | 17.621861 | 통과 |
| find_peaks_96000 | 1.346 | 0.922716 | 1.184307 | 집계 통과, 2번 실행 0.944461 |
| first_peak_index_96000 | 1.158 | 1.579252 | 1.470158 | 통과 |
| spline_k1_783_to_4800 | 1.378 | 1.355769 | 1.422192 | 통과 |
| spline_k2_783 | 2.685 | 2.648208 | 2.707937 | 통과 |
| spline_k3_783 | 2.975 | 2.504926 | 2.625000 | 통과 |
| linregress_1000 | 135.278 | 142.388889 | 140.111111 | 통과 |
| nnresample_design_147_160 | 2.026 | 1.851836 | 2.006745 | 통과 |
| resample_poly_96000_48k_44k1 | 1.693 | 1.713252 | 1.703778 | 통과 |
| nnresample_96000_48k_96k | 2.427 | 2.437212 | 2.440136 | 통과 |
| spectrogram_295000_4800 | 2.951 | 3.231426 | 3.073192 | 통과 |
| windows_32001 | 1.988 | 2.068395 | 1.993959 | 통과 |
| expit_783 | 1.138 | 1.172414 | 1.214286 | 통과 |
| next_fast_len_1e6 | 27.700 | 30.444444 | 27.000000 | 통과 |

DSP primitive는 일반 Python으로 측정했습니다. 스레드 병렬 처리가 있는 service·전체 파이프라인은 FT도 비교합니다.

### 2.3. impulcifer-service

| 연산 | 크레이트 보고서 일반 / FT | PA06 1차 일반 / FT | PA06b 2차 일반 / FT | 판정 |
|---|---:|---:|---:|---|
| bootstrap | 4.149 / 4.315 | 4.138001 / 4.324591 | 4.265495 / 4.368280 | 통과 |
| get_ui_settings | 2.476 / 2.662 | 2.735937 / 2.938621 | 2.539849 / 2.698274 | 통과 |
| set_language_round_trip | 1.533 / 1.555 | 1.637455 / 1.727286 | 1.513346 / 1.612389 | 통과 |
| resolve_recording_paths | 2.274 / 5.124 | 2.215480 / 2.345780 | 2.253169 / 2.613759 | 시간 통과, FT 48.990% 감소; 변동 원인 미확정 |
| start_brir_to_first_event | 1.515 / 1.642 | 1.518266 / 1.612946 | 1.467904 / 1.586005 | 통과 |
| poll_job_drain | 4.459 / 4.869 | 4.598223 / 5.043069 | 4.344316 / 5.020809 | 통과 |
| job_event_emit | 15.595 / 16.578 | 14.535267 / 15.499733 | 15.755203 / 16.969270 | 통과 |
| detect_sweep | 2.647 / 2.919 | 2.393420 / 2.613097 | 2.703659 / 2.763670 | 통과 |
| catalog_translate | 2.794 / 3.118 | 2.694009 / 3.149894 | 2.752733 / 3.188656 | 통과 |

### 2.4. impulcifer-audio-io와 impulcifer-sys-win

크레이트 보고서의 최신 결과는 PA05b 7차입니다. PA06 1차보다 뒤에 측정한 값이므로 아래 표의 시간 순서는 PA06 → PA05b → PA06b입니다. 전체 명시적/production 비교와 원시 분모는 5절에 있습니다.

| 연산 | 크레이트 보고서(PA05b) 일반 / FT | PA06 1차 일반 / FT | PA06b 2차 일반 / FT | 판정 |
|---|---:|---:|---:|---|
| 캐시 열거 | 78.547893 / 76.911877 | 75.650558 / 74.828996 | 77.003846 / 78.626923 | 통과 |
| duplex open/close | 1.363745 / 1.245404 | 1.273951 / 1.292799 | 1.280979 / 1.291691 | 통과 |
| headphones wall | 0.997478 / 0.997627 | 0.996738 / 0.996549 | 0.997030 / 0.997440 | 미달 |
| headphones overhead | 0.723585 / 0.739944 | 0.660448 / 0.640820 | 0.668897 / 0.714574 | 미달 |
| seven wall | 0.999639 / 0.999685 | 0.999532 / 0.999568 | 0.999621 / 0.999555 | 미달 |
| seven overhead | 0.777739 / 0.805918 | 0.719489 / 0.740656 | 0.771701 / 0.732015 | 미달 |
| 첫 샘플 전달 | 0.764189 / 0.711882 | 0.869886 / 0.890895 | 0.809602 / 0.845734 | 미달 |
| CPU | 3.000000 / 2.500000 | 2.333333 / 2.000000 | 2.000000 / 1.666667 | 1.0 이상, PA05b 대비 33.333% 감소; 계측 단위 주의 |
| sys-win fresh COM 열거 / PA 캐시 | 2.0501÷272.0077 / 2.0074÷272.0077 | 0.007555 / 0.007473 | 2.0021÷267.2525 / 2.0443÷267.2525 | 약 0.00749 / 0.00765; 동등 작업 아님, 통과 근거로 쓰지 않음 |

CPU의 이전 Rust 31.25ms와 이번 46.875ms는 Windows user+kernel 카운터의 15.625ms 한 단위 차이입니다. 일반/FT Python도 93.75/78.125ms로 PA05b와 같습니다. 33.333% 비율 감소를 숨기지 않되 1.5배의 계산량 증가나 소스 회귀로 단정하지 않습니다. 오디오를 여기서 수정하지 말라는 PA06b 지시를 따랐으며 이 계측 변동의 원인은 미확정입니다.

## 3. 다섯 연산의 재측정과 조건부 과거 커밋 비교

각 표의 세 행에 R median, P median, P/R을 기록했습니다. 시간 단위는 ms입니다. 집계는 별도 행에 표시합니다. `calculations.log`와 `aggregates.json`의 숫자를 해당 벤치 원문 행과 대조했습니다.

### 3.1. IO read_pcm32_32tracks

| 독립 실행 | R median | P median | P/R |
|---|---:|---:|---:|
| 1 | 7.243700 | 7.607900 | 1.0502781727570165 |
| 2 | 5.320300 | 7.564300 | 1.421780726650753 |
| 3 | 5.377900 | 7.558900 | 1.4055486342252552 |
| median들의 집계 | 5.377900 | 7.564300 | 1.4065527436359917 |

PA01 1.70 대비 **−17.261603%**, 조건부 과거 비교 기준 `1.70×0.8=1.36` 이상입니다. 1.0도 넘으므로 PA01 `39c00c3`의 detached 비교 조건은 성립하지 않습니다. 과거 커밋을 측정하지 않았습니다. 첫 실행 Rust 7.2437ms와 이후 5.3203/5.3779ms의 변동은 그대로 기록하며 그 원인을 확정하지 않았습니다.

### 3.2. IO read_bundled_sweep

| 독립 실행 | R median | P median | P/R |
|---|---:|---:|---:|
| 1 | 0.751000 | 0.682600 | 0.9089214380825565 |
| 2 | 0.472200 | 0.690300 | 1.4618805590851334 |
| 3 | 0.563600 | 0.690800 | 1.2256919801277502 |
| median들의 집계 | 0.563600 | 0.690300 | 1.2248048261178142 |

PA01 1.47 대비 **−16.679944%**, 조건 `1.47×0.8=1.176`과 1.0을 모두 넘습니다. `39c00c3` 과거 비교는 조건 미충족으로 수행하지 않았습니다. **첫 실행 0.908921은 미달**입니다. 집계 통과를 모든 실행 통과라고 쓰지 않습니다.

### 3.3. DSP find_peaks_96000

| 독립 실행 | R median | P median | P/R |
|---|---:|---:|---:|
| 1 | 0.986300 | 1.202200 | 1.2188989151373821 |
| 2 | 0.979500 | 0.925100 | 0.9444614599285349 |
| 3 | 0.982600 | 1.163700 | 1.1843069407693874 |
| median들의 집계 | 0.982600 | 1.163700 | 1.1843069407693874 |

PA02 1.346 대비 **−12.012857%**, 조건 `1.346×0.8=1.0768`과 1.0을 넘습니다. peaks만을 위한 과거 비교·bisect는 요구 조건에 해당하지 않습니다. 다만 FFT 때문에 실행한 과거 전체 DSP 벤치에 peaks도 포함되어 `e98a954`의 0.9672 / 1.0058 / 0.9405ms(집계 0.9672ms)를 얻었습니다. 현재 0.9826ms와의 차이만으로 회귀를 특정하지 않았습니다. **이번 2번 실행 0.944461, PA06 최초 0.922716의 미달은 보존합니다.**

### 3.4. DSP rfft_irfft_96000

| 독립 실행 | R median | P median | P/R |
|---|---:|---:|---:|
| 1 | 0.967200 | 2.120800 | 2.192721257237386 |
| 2 | 0.985300 | 1.490600 | 1.512838729321019 |
| 3 | 0.946700 | 1.549300 | 1.6365268828562374 |
| median들의 집계 | 0.967200 | 1.549300 | 1.6018403639371381 |

PA02 2.021 대비 **−20.740210%**로 `1.6168`보다 작아 과거 비교를 수행했습니다. 16:29~16:31의 현재 결과와 16:36~16:37의 `e98a954` 결과는 같은 시간대의 동일 머신 측정입니다.

| 비교 | 과거 Rust 1 / 2 / 3 | 과거 Rust 집계 | 현재 Rust 집계 | 판단 |
|---|---:|---:|---:|---|
| e98a954 FFT | 1.028600 / 1.152800 / 1.034400 | 1.034400 | 0.967200 | 현재 Rust 지연 증가 없음 |

이전 보고서의 Python은 2.0754ms, 이번 집계는 1.5493ms입니다. 과거 코드를 이번에 빌드한 Rust보다 현재 Rust가 빨라 **Rust 소스가 느려졌다는 증거는 없습니다.** 기록된 비율 감소에는 Python 시간 감소가 반영됩니다. 과거 당시 환경의 변동 원인까지 입증한 것은 아니며, Rust slowdown 조건이 없으므로 FFT bisect나 소스 수정은 하지 않았습니다.

### 3.5. Service resolve_recording_paths (FT)

| 독립 실행 | R median | FT P median | P/R |
|---|---:|---:|---:|
| 1 | 0.001301500 | 0.003030000 | 2.328082981175567 |
| 2 | 0.001201000 | 0.003153500 | 2.625728559533722 |
| 3 | 0.001206500 | 0.003197500 | 2.650227932034811 |
| median들의 집계 | 0.001206500 | 0.003153500 | 2.6137588064649813 |

PA04 5.124 대비 **−48.989875%**, 조건 `4.0992` 미만이므로 과거 비교가 필수였습니다. 최초 PA06 보고서가 지목한 `59d9ae3`은 현재 HEAD의 조상이 아닙니다. 그래도 요청된 커밋을 실제로 빌드하고 세 번 측정했습니다. 현재 이력에 있는 PA04 커밋은 `5307b6b`입니다. 두 커밋의 PA04 보고서·bench·`tests/bench_support/service.rs`는 동일함을 확인했지만 service 전체는 updater 변경 때문에 같지 않습니다.

| 위치와 커밋 | Rust median 1 | 2 | 3 | 세 median의 median |
|---|---:|---:|---:|---:|
| 본 트리 HEAD | 0.001301500 | 0.001201000 | 0.001206500 | 0.001206500 |
| scratch 59d9ae3(비조상) | 0.001427500 | 0.001464500 | 0.001443500 | 0.001443500 |
| scratch HEAD 최초 | 0.001620500 | 0.001627000 | 0.001644500 | 0.001627000 |
| scratch 5307b6b(조상 PA04) | 0.001460000 | 0.001442000 | 0.001453500 | 0.001453500 |
| scratch 부모 2afe55c(유효 bisect) | 0.001476000 | 0.001473500 | 0.001489000 | 0.001476000 |
| scratch HEAD 재확인 | 0.001391000 | 0.001492500 | 0.001387000 | 0.001391000 |

16:39~16:50의 과거·현재 scratch 비교에서 최초 HEAD 집계 0.001627ms는 과거보다 느렸습니다. 따라서 소스가 같다는 이유로 조사를 생략하지 않고 실제 `git bisect`를 실행했습니다.

1. 비조상 `59d9ae3`으로 시작하자 Git이 merge base `7ed262a6dae0b4d54f5e95f1011bcb3d38264bba` 검증을 요구했고 해당 위치에 service 벤치 파일이 없었습니다. 이 시도는 후보 판정에 쓰지 않고 reset했습니다.
2. 조상 `5307b6b`로 다시 시작했습니다. 최초 helper는 `git bisect run`의 cwd를 하위 로그 실행기에 전달하지 않아 **본 트리 `E:/Impulcifer`의 벤치**를 실행했습니다. 후보 31faf9f, 91152b5, 2afe55c에 붙인 세 차례 호출(각각 3회 측정)은 전부 무효입니다. 종료 코드 0도 이 오류를 없애지 않습니다.
3. helper가 `PA06B_CWD=os.getcwd()`를 전달하도록 로컬 실행기 호출을 고친 뒤 reset/start/run을 반복했습니다. 유효 후보는 같은 세 커밋이며 각각 실제 detached cwd에서 측정했습니다. `median > 0.00154ms`를 bad로 삼았습니다. 후보 호출은 무효 3+유효 3=6회입니다. 캐시를 재사용한 호출도 있어 **여섯 번 전체 재빌드했다고 쓰지 않습니다.**
4. 유효 후보 세 개가 모두 good여서 Git은 미리 bad로 지정한 `e62c3cd`를 첫 bad라고 출력했습니다. 이 커밋은 작업서 38줄만 추가했습니다. 부모 0.001476ms와 HEAD 재확인 0.001391ms를 비교하니 최초 slowdown이 재현되지 않았습니다. 이 결과를 인과적인 회귀 커밋으로 채택하지 않았습니다.

| 후보 호출 | 표기 커밋 | 세 Rust median(ms) | 집계 | 유효성 |
|---|---|---:|---:|---|
| 1 | 31faf9f | 0.001206 / 0.001231 / 0.0012035 | 0.001206 | 무효, 실제 main tree |
| 2 | 91152b5 | 0.0012285 / 0.001231 / 0.001195 | 0.0012285 | 무효, 실제 main tree |
| 3 | 2afe55c | 0.0011965 / 0.0012855 / 0.001195 | 0.0011965 | 무효, 실제 main tree |
| 4 | 31faf9f | 0.001445 / 0.001466 / 0.001471 | 0.001466 | 유효 good |
| 5 | 91152b5 | 0.0014535 / 0.001461 / 0.0014495 | 0.0014535 | 유효 good |
| 6 | 2afe55c | 0.001476 / 0.0014735 / 0.001489 | 0.001476 | 유효 good |

이전 보고서의 FT Python 0.006487ms는 이번 0.0031535ms보다 큽니다. 본 트리 Rust도 과거 보고서 0.001266ms에서 이번 0.0012065ms로 줄었습니다. 이 수치로 기록된 비율 감소의 상당 부분은 확인할 수 있습니다. 하지만 **scratch 최초 0.001627ms의 원인은 미확정**이므로 ‘Python만 달라졌음을 증명했다’고 결론 내리지 않습니다. Rust 소스 회귀는 재현되지 않았고 수정도 하지 않았습니다. worktree는 17:42에 제거했으며 17:43 확인에서 존재하지 않았습니다.

### 3.6. 1.5 미만 연산의 프로파일 해석

이번에는 샘플링 프로파일러나 새 축소 벤치를 실행하지 않았습니다. 기존 코드·trace에 근거한 비용 설명을 유지하되 새 측정으로 확정한 원인처럼 쓰지 않습니다.

- IO 읽기는 디코딩·전치·출력 할당, 32채널 입력의 scoped thread 생성 비용이 있습니다. sweep처럼 작은 입력에서는 파일 I/O·할당의 실행 간 차이가 큽니다. demo·float32 읽기와 전체 필드 파일명 파싱도 1.5 미만입니다.
- DSP SOS는 feedback에 따른 샘플 의존성, minimum-phase는 패리티용 전체 스펙트럼 처리, peaks/first peak는 순회·plateau 분기·결과 할당, 선형 spline은 질의별 검색·보간, expit은 scalar exp를 수행합니다.
- Service 일반 first-event 1.467904에는 설정 payload 생성·검증·스레드 시작·첫 poll 비용이 있습니다. 전체 BRIR 실행 시간과 다릅니다.
- 오디오에는 shared 40ms 버퍼, 초기화·시작·drain·종료·이벤트 전달 비용이 남아 있습니다. 개별 API trace median을 합쳐 동시 duplex wall time이라고 계산하지 않습니다.
- sys-win fresh COM 열거와 PortAudio 캐시는 같은 작업이 아닙니다. 새 SIMD·unsafe·f32 수치 구현·의존성·공개 시그니처 변경은 없으며 최적화 후의 새 golden 최대 오차도 산출하지 않았습니다.

## 4. 전체 파이프라인, 플롯과 메모리

| 시나리오 / Python | PA03 프로세스 비율 | PA06 1차 프로세스 비율 | PA06b 프로세스 비율 | PA06 1차 main/service | PA06b main/service | 판정 |
|---|---:|---:|---:|---:|---:|---|
| default / 일반 | 11.917014 | 11.307964 | 11.584635 | 8.789373 | 8.903480 | 통과 |
| default / FT | 5.469190 | 5.024980 | 5.022391 | 3.609704 | 3.502660 | 통과 |
| vbass / 일반 | 11.973251 | 11.445840 | 11.957855 | 8.805108 | 9.103734 | 통과 |
| vbass / FT | 5.155249 | 5.112934 | 5.138253 | 3.601658 | 3.582875 | 통과 |

아래는 `pipeline.log`의 전체 집계 행입니다. 시간은 ms, 메모리는 MiB입니다. root peak, largest workload process peak, sum historical peaks, concurrent tree RSS를 모두 보존했습니다. Python CLI의 in-process 0은 해당 타이머를 쓰지 않았다는 표기이지 실행 시간이 0이라는 뜻이 아닙니다.

```text
| scenario | side | mode | process median ms | process min ms | in-process median ms | root peak MiB | largest workload process peak MiB | sum historical peaks MiB | concurrent tree RSS MiB |
| default | rust | service | 1139.619500 | 1092.478900 | 1066.004900 | 246.859 | 246.859 | 246.859 | 241.172 |
| default | python314 | cli | 13202.075600 | 13199.389200 | 0.000000 | 494.711 | 494.711 | 1141.746 | 947.477 |
| default | python314 | inprocess | 11299.123100 | 11280.569800 | 9491.153800 | 495.594 | 495.594 | 995.504 | 795.973 |
| default | python314t | cli | 5723.614500 | 5721.705100 | 0.000000 | 4.316 | 521.152 | 525.461 | 525.406 |
| default | python314t | inprocess | 5701.975000 | 5694.795700 | 3733.853100 | 4.312 | 521.863 | 526.172 | 526.117 |
| vbass | rust | service | 1105.865900 | 1103.623900 | 1040.941600 | 234.102 | 234.102 | 234.102 | 231.656 |
| vbass | python314 | cli | 13223.784300 | 13176.782500 | 0.000000 | 494.895 | 494.895 | 1142.457 | 912.988 |
| vbass | python314 | inprocess | 11325.683900 | 11284.768700 | 9476.455400 | 495.684 | 495.684 | 995.863 | 762.332 |
| vbass | python314t | cli | 5682.218800 | 5678.533100 | 0.000000 | 4.309 | 520.961 | 525.270 | 525.215 |
| vbass | python314t | inprocess | 5720.622700 | 5693.067200 | 3729.564100 | 4.316 | 521.500 | 525.812 | 525.758 |
| scenario | comparator | Python CLI / Rust process | Python main / Rust service |
| default | python314 | 11.584635 | 8.903480 |
| default | python314t | 5.022391 | 3.502660 |
| vbass | python314 | 11.957855 | 9.103734 |
| vbass | python314t | 5.138253 | 3.582875 |
```

FT root 약 4.3MiB는 런처입니다. 이를 실제 Python workload 521MiB 대신 쓰지 않습니다. 과거 peak의 합은 같은 시점의 사용량이 아니고, 동시 트리 RSS도 공유 페이지를 중복 계산할 수 있습니다. Rust가 한 프로세스에서 rayon을 쓰고 일반 Python은 process pool, FT는 스레드를 쓰지만 인터프리터·할당기도 다르므로 메모리 차이의 원인을 스레드 하나로 단정하지 않습니다. 메모리 최적화는 하지 않았습니다.

**플롯 검증을 정정합니다.** 17:41:21의 `calculations.log`는 Python 두 환경에 각각 `16 False`를 출력했습니다. 출력 목록은 `plots\\headphones.png`처럼 역슬래시를 사용했는데 검사기는 슬래시 문자열과 그대로 비교했습니다. 17:41:48의 후속 검사는 `p.replace(chr(92),'/')`로 비교 문자를 맞춰 두 환경 모두 True임을 확인했습니다. 이는 새 측정이 아니라 이미 생성한 산출물 확인입니다. Rust 로그에도 `cli_plotting_results` 단계 12행이 있습니다.

```text
outputs ['README.md', 'headphone-responses.wav', 'hesuvi.wav', 'hrir.wav', 'plots\\headphones.png', 'plots\\results.png', 'responses.wav', 'room-responses.wav']
plots [('python314', True), ('python314t', True)]
RUST_PLOT_STAGE_ROWS 12
```

파이프라인 원시 자료 디렉터리는 `C:/Users/32170336/AppData/Local/Temp/impulcifer-pa03-baseline-cupfe9wj`입니다. 이 로컬 디렉터리만을 근거로 남기지 않도록 위 전체 시간·메모리 행과 부록의 실행 명령을 보고서에 포함했습니다. 새 그래프는 만들지 않았습니다.

## 5. 오디오 PA06 / PA05b / 이번 실행

### 5.1. 명시적 InputStream/OutputStream

| 연산 | PA06 일반 / FT | PA05b 일반 / FT | PA06b 일반 / FT | 이번 판정 |
|---|---:|---:|---:|---|
| 캐시 열거 | 75.650558 / 74.828996 | 78.547893 / 76.911877 | 77.003846 / 78.626923 | 통과 |
| duplex open/close | 1.273951 / 1.292799 | 1.363745 / 1.245404 | 1.280979 / 1.291691 | 통과 |
| headphones wall | 0.996738 / 0.996549 | 0.997478 / 0.997627 | 0.997030 / 0.997440 | 미달 |
| headphones overhead | 0.660448 / 0.640820 | 0.723585 / 0.739944 | 0.668897 / 0.714574 | 미달 |
| seven wall | 0.999532 / 0.999568 | 0.999639 / 0.999685 | 0.999621 / 0.999555 | 미달 |
| seven overhead | 0.719489 / 0.740656 | 0.777739 / 0.805918 | 0.771701 / 0.732015 | 미달 |
| 첫 샘플 전달 | 0.869886 / 0.890895 | 0.764189 / 0.711882 | 0.809602 / 0.845734 | 미달 |
| CPU | 2.333333 / 2.000000 | 3.000000 / 2.500000 | 2.000000 / 1.666667 | 1.0 이상, 계측 변동 미확정 |

### 5.2. 실제 core.recorder.play_and_record

PA06 production 비율은 보존한 첫 보고서의 raw Python 행을 당시 observer Rust 행으로 나눈 값입니다. PA05b도 해당 보고서의 observer 분모와 production 행을 사용합니다. 아래 PA06 열은 계산을 그대로 재현할 수 있도록 **정확한 분수**로 적었습니다. 괄호의 일반/FT 순서는 다른 표와 같습니다. 소수 비율이 이미 있는 PA05b·이번 값은 6자리 반올림입니다.

| 연산 | PA06 일반 / FT (P÷R) | PA05b 일반 / FT | PA06b 일반 / FT | 이번 판정 |
|---|---|---:|---:|---|
| 캐시 열거 | 2.0007÷0.0269 / 2.0817÷0.0269 | 77.068966 / 77.486590 | 77.100000 / 77.957692 | 통과 |
| duplex open/close | 230.2767÷179.0243 / 233.1205÷179.0243 | 1.273735 / 1.338634 | 1.267459 / 1.302388 | 통과 |
| headphones wall | 6202.1309÷6211.1332 / 6204.2565÷6211.1332 (약 0.998551 / 0.998893) | 0.998808 / 0.999423 | 0.999481 / 0.999594 | 미달 |
| headphones overhead | 50.672567÷59.674867 / 52.798167÷59.674867 | 0.869391 / 0.936721 | 0.942106 / 0.954757 | 미달 |
| seven wall | 43129.7026÷43132.0996 / 43128.7581÷43132.0996 (약 0.999944 / 0.999923) | 1.000164 / 1.000020 | 0.999955 / 1.000210 | 일반 미달, FT 통과 |
| seven overhead | 69.494267÷71.891267 / 68.549767÷71.891267 | 1.100785 / 1.012603 | 0.972617 / 1.126174 | 일반 미달, FT 통과 |
| 첫 샘플 전달 | 34.4411÷25.69865 / 36.2067÷25.69865 | 0.739952 / 1.131744 | 1.270103 / 1.265757 | 통과, 별도 callback 지연 |
| CPU | 62.5÷46.875 / 93.75÷46.875 | 2.500000 / 3.000000 | 2.333333 / 2.000000 | 1.0 이상 |

PA05b production 비율의 재계산 분모는 열거 0.0261, open 178.4842, headphones 6208.1062, overhead 56.647867, seven 43130.3044, overhead 70.096067, latency 31.0371, CPU 31.25ms입니다. Python 일반/FT 분자는 각각 2.0115/2.0224, 227.3416/238.9251, 6200.7075/6204.5216, 49.249167/53.063267, 43137.369/43131.1878, 77.160667/70.979467, 22.96595/35.12605, 78.125/93.75ms입니다. 이전 production 결과가 없다고 처리하지 않았습니다.

### 5.3. 원문 시각·카운터와 무결성 한계

- 이번 observer Rust headphones 6207.1271ms, 명시적 Python 6188.6950/6191.2378ms입니다. Rust가 각각 18.4321/15.8893ms 더 걸립니다. standalone Rust 6201.1429ms가 더 유리하더라도 표의 분모를 바꾸지 않았습니다.
- Observer Rust seven 43131.8612ms 대 명시적 Python 43115.5029/43112.6593ms입니다. 1.0 기준에 미달합니다. Production FT 43140.9019ms보다 빠르다는 사실만으로 오디오 전체를 통과 처리하지 않습니다.
- CABLE-A의 exclusive 거절 후 shared auto-convert, 48kHz stereo, 기본 4주기 1920프레임(40ms), 캡처 480프레임을 사용했습니다. PA05b의 MMCSS 승격이 포함된 소스를 그대로 측정했으며 버퍼 실험을 반복하지 않았습니다.
- 첫 샘플은 Rust 입력 session start 호출부터 비어 있지 않은 application read 반환, Python 입력 start부터 첫 callback입니다. 출력 신호가 실제 도착한 시간과 같지 않습니다. Production 표에서도 별도 입력 callback 측정입니다.
- CPU는 ACK로 측정 시작·끝을 맞춘 자식의 user+kernel 시간입니다. 이번 Rust 46.875ms는 15.625ms 단위 카운터 3틱이며 짧은 구간의 작은 변화는 정밀하게 해석할 수 없습니다.
- **독립 무결성 10회 시험을 새로 실행하지 않았습니다.** PA06b 지시대로 PA05b 최종 headphones 6/10, seven 8/10의 미달을 유지합니다. 이번 CABLE service 실기 테스트 1개 통과는 독립 동시 녹음 10/10 무손실 증거가 아닙니다. PA06의 공통 프레임 손실과 미확정 관찰, production convenience-stream 제약도 해결했다고 주장하지 않습니다.

## 6. 검증 결과와 perf.release 판정

| 요구·검증 | 이번 수치와 판정 |
|---|---|
| release 전체 빌드 | exit 0 |
| 기존 5개 크레이트 perf 및 Python 오라클 | 모두 종료, exit 0; 개별 시간 기준과 구분 |
| IO·DSP·service 전체 집계 ratio ≥ 1 | 충족; 최소 IO FT 파일명 1.141646, DSP peaks 1.184307, service 일반 first-event 1.467904 |
| 다섯 지정 연산의 세 독립 실행·집계 | 충족; 3절에 각 9개 숫자와 집계 보존 |
| 집계 미달/20% 감소의 조건부 과거 비교 | IO 2개·peaks는 조건 없음; FFT·service는 실행. Service bisect도 실행했으나 최초 변동의 원인은 미확정 |
| 파이프라인 default/vbass, 일반/FT, 플롯 포함 | 충족; process ratio 11.584635 / 5.022391 / 11.957855 / 5.138253 |
| 파이프라인 main/service | 충족; 8.903480 / 3.502660 / 9.103734 / 3.582875 |
| M5 전체 프로세스 메모리 측정 | 충족; 네 메모리 차원과 모든 모드 행 보존. 메모리 우열은 별도 통과 조건으로 만들지 않음 |
| CRLF 체크아웃 workspace | 충족; **379 passed / 0 failed / 10 ignored**, exit 0 |
| CRLF smoke | 충족; main.rs CRLF 248개, LF 바이트도 248개(모두 CRLF에 포함); `updater_plugin_registered_with_public_key ... ok` |
| policy / fmt / clippy | 충족; policy 6 passed, fmt exit 0, clippy `-D warnings` exit 0 |
| 기존 골든과 smoke | 충족; default/vbass parity, plots_do_not_change_wavs와 6개 bench smoke 통과 |
| updater 간헐 실패 재확인 | 지정 시험 연속 **10/10 통과**, 최초 PA06 실패의 원인 자체는 미확정 |
| CABLE-A service 실기 | **1 passed**, exit 0; 독립 10회 무결성과 별개 |
| 명시적 오디오 모든 ratio ≥ 1 | **미충족**; headphones 0.997030/0.997440, seven 0.999621/0.999555, overhead·latency도 미달 |
| production 오디오 모든 ratio ≥ 1 | **미충족**; headphones 0.999481/0.999594, seven 일반 0.999955, overhead 미달 |
| 독립 동시 녹음 무결성 | **미충족 유지**; PA05b headphones 6/10, seven 8/10, 재시험 없음 |
| 원인 규명·수정 완료 | **미충족**; service 최초 scratch 변동, 오디오 미달·CPU 계측 변동·무결성 문제 남음; source fix 없음 |
| perf.release / M5 승인 | **미통과**, features.toml 변경 없음 |

여기서 통과한 테스트는 측정 워커가 먼저 완료한 로그의 실제 결과입니다. 부모 세션은 로그를 검토했으며, 이후 독립 `cargo test`/clippy 연속 호출과 `cargo test` 재실행 시도는 승인 요구 때문에 **실행되지 않았습니다**. 부모가 다시 돌려 통과했다고 쓰지 않습니다. 이번 문서 작업에서도 테스트를 실행하지 않았습니다.

등록부 검증용 이름은 다음과 같이 제안할 수 있지만 M5 미통과이므로 상태를 바꾸지 않습니다.

- `impulcifer-service::bench_smoke_pipeline_demo`
- `impulcifer-io::bench_smoke_impulcifer_io`
- `impulcifer-dsp::bench_smoke_impulcifer_dsp`
- `impulcifer-service::bench_smoke_impulcifer_service`
- `impulcifer-audio-io::bench_smoke_impulcifer_audio_io`
- `impulcifer-sys-win::bench_smoke_impulcifer_sys_win`

## 부록 A. 이번 실행의 명령과 원시 증거

아래 부록은 무시된 로컬 로그만 가리키지 않도록 실제 argv, cwd/env, 시작·종료 시각, 원시 median/min과 테스트 요약을 보존합니다. 모든 시각은 **2026-09-08 +09:00**이며 이름 순서가 아니라 첫 JSON 줄의 start가 실행 순서입니다. 컴파일러의 반복적인 의존성 나열, 벤치의 중복 개별 표본, 파이프라인의 긴 반복 이벤트 JSON만 줄였습니다.

### A.1. 실제 argv를 재현하는 명령 목록

아래 PowerShell 명령은 로그 argv를 셸 문법으로 옮긴 것입니다. `M`은 본 트리 cwd `E:/Impulcifer`와 CARGO_TARGET_DIR 미설정, `S`는 detached cwd와 지정 target, `X`는 **본 트리 cwd이면서 지정 target인 무효 측정**입니다. 이 약호는 실행 기록표에만 씁니다. `Set-Location`과 환경 설정은 로그의 실행 조건을 재현하기 위한 문법이며, 측정 당시 별도 명령으로 실행했다는 주장이 아닙니다. 모든 실행에 `PYTHONIOENCODING=utf-8`, `PYTHONDONTWRITEBYTECODE=1`이 적용됐습니다. 명시하지 않은 RUSTFLAGS·CARGO_ENCODED_RUSTFLAGS·IMPULCIFER_PERF_DIR·IMPULCIFER_PA05_SHARED_PERIODS는 null입니다.

```powershell
# M 조건
Set-Location 'E:/Impulcifer'
$env:CARGO_TARGET_DIR = $null
$env:PYTHONIOENCODING = 'utf-8'
$env:PYTHONDONTWRITEBYTECODE = '1'
$env:RUSTFLAGS = $null
$env:CARGO_ENCODED_RUSTFLAGS = $null
$env:IMPULCIFER_PERF_DIR = $null
$env:IMPULCIFER_PA05_SHARED_PERIODS = $null
# S 조건
Set-Location 'C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect'
$env:CARGO_TARGET_DIR = 'C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect-target'
# X 조건(최초 잘못된 후보 호출의 실제 조건)
Set-Location 'E:/Impulcifer'
$env:CARGO_TARGET_DIR = 'C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect-target'
```

다음 번호는 아래 실행 기록의 ‘명령’ 열과 대응합니다. 동일 argv의 반복은 번호를 재사용하되 각 start/end를 따로 적었습니다.

```powershell
# C01
 git rev-parse HEAD
# C02
 cargo build --workspace --release
# C03
 cargo bench -p impulcifer-io --bench perf
# C04
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
# C05
 & C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
# C06
 cargo bench -p impulcifer-dsp --bench perf
# C07
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_dsp.py
# C08
 cargo bench -p impulcifer-service --bench perf
# C09
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
# C10
 & C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
# C11
 cargo bench -p impulcifer-dsp --bench perf --no-run
# C12
 cargo bench -p impulcifer-service --bench perf --no-run
# C13
 cargo bench -p impulcifer-service --bench perf -- --pipeline
# C14
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_pipeline.py
# C15
 py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
# C16
 & C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
# C17
 cargo bench -p impulcifer-audio-io --bench perf
# C18
 cargo bench -p impulcifer-sys-win --bench perf
# C19
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
# C20
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
# C21
 & C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
# C22
 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
# C23
 & C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
# C24
 cargo test --workspace
# C25
 cargo test -p impulcifer-policy
# C26
 cargo fmt --all -- --check
# C27
 cargo clippy --workspace --all-targets -- --no-deps -D warnings
# C28
 cargo test -p impulcifer-service --test updater legacy_executor_downloads_verifies_and_opens -- --exact --nocapture
# C29
 cargo test -p impulcifer-service --test recording -- --ignored recording_virtual_cable_end_to_end
# C30
 git status --porcelain
```

#### 본 측정과 검증의 시작·종료 기록

모든 행은 exit 0입니다. S·X의 빌드와 후보 측정은 다음 표에 따로 적었습니다.

| 로그명(.log) | 명령 / 조건 | start | END |
|---|---|---|---|
| starting-head | C01 M | 16:26:13.316875 | 16:26:13.332828 |
| release-build | C02 M | 16:26:28.415725 | 16:27:06.213599 |
| io-rust-standalone | C03 M | 16:27:19.263103 | 16:27:24.769354 |
| io-python-1 | C04 M | 16:27:35.716720 | 16:27:44.110579 |
| io-python-2 | C04 M | 16:27:52.636814 | 16:28:00.458412 |
| io-python-3 | C04 M | 16:28:07.098967 | 16:28:14.662898 |
| io-python-ft | C05 M | 16:28:26.291440 | 16:28:34.517927 |
| dsp-rust-1 | C06 M | 16:29:23.538704 | 16:29:33.821086 |
| dsp-python-1 | C07 M | 16:29:44.464696 | 16:30:09.859926 |
| dsp-rust-2 | C06 M | 16:30:18.113860 | 16:30:28.356907 |
| dsp-python-2 | C07 M | 16:30:35.408970 | 16:31:00.720515 |
| dsp-rust-3 | C06 M | 16:31:05.819322 | 16:31:16.029134 |
| dsp-python-3 | C07 M | 16:31:25.022769 | 16:31:50.352581 |
| service-rust-1 | C08 M | 16:32:36.434393 | 16:32:52.135301 |
| service-python | C09 M | 16:33:01.867527 | 16:33:19.821269 |
| service-python-ft-1 | C10 M | 16:33:39.106090 | 16:33:58.213572 |
| service-rust-2 | C08 M | 16:34:07.212581 | 16:34:14.228329 |
| service-python-ft-2 | C10 M | 16:34:24.751806 | 16:34:43.952140 |
| service-rust-3 | C08 M | 16:34:49.026147 | 16:34:56.226663 |
| service-python-ft-3 | C10 M | 16:35:02.789859 | 16:35:21.843079 |
| pipeline-rust | C13 M | 16:50:44.142248 | 16:50:57.344327 |
| pipeline | C14 M | 16:51:04.657394 | 16:56:13.908194 |
| environment | C15 M | 16:57:29.203454 | 16:57:29.981424 |
| environment-ft | C16 M | 16:57:40.365983 | 16:57:41.260999 |
| audio-rust-standalone | C17 M | 16:57:49.115676 | 17:03:00.959804 |
| sys-rust | C18 M | 17:03:09.713904 | 17:03:12.162050 |
| audio-rust-observed | C19 M | 17:03:20.239572 | 17:08:32.248937 |
| audio-python-explicit | C20 M | 17:08:40.365743 | 17:13:58.791316 |
| audio-python-ft-explicit | C21 M | 17:14:07.321589 | 17:19:26.038439 |
| audio-python-production | C22 M | 17:19:32.529447 | 17:24:51.270380 |
| audio-python-ft-production | C23 M | 17:25:00.794996 | 17:30:19.763024 |
| workspace-tests | C24 M | 17:30:57.810481 | 17:37:37.963914 |
| policy | C25 M | 17:37:45.566472 | 17:37:47.839154 |
| fmt | C26 M | 17:37:58.175448 | 17:37:58.847997 |
| clippy | C27 M | 17:38:06.418084 | 17:38:11.264670 |
| legacy-flake-1 | C28 M | 17:38:30.744089 | 17:38:37.192523 |
| legacy-flake-2 | C28 M | 17:38:37.281612 | 17:38:37.937565 |
| legacy-flake-3 | C28 M | 17:38:38.032418 | 17:38:38.704390 |
| legacy-flake-4 | C28 M | 17:38:38.790670 | 17:38:39.445278 |
| legacy-flake-5 | C28 M | 17:38:39.535467 | 17:38:42.140942 |
| legacy-flake-6 | C28 M | 17:38:42.227526 | 17:38:42.893322 |
| legacy-flake-7 | C28 M | 17:38:42.986598 | 17:38:43.694549 |
| legacy-flake-8 | C28 M | 17:38:43.784534 | 17:38:44.425324 |
| legacy-flake-9 | C28 M | 17:38:44.523184 | 17:38:45.154440 |
| legacy-flake-10 | C28 M | 17:38:45.243565 | 17:38:45.962424 |
| recording-hardware | C29 M | 17:39:09.549344 | 17:39:35.393989 |
| final-status | C30 M | 17:42:25.399728 | 17:42:25.437005 |
| final-head | C01 M | 17:42:50.648452 | 17:42:50.665014 |

#### 과거 빌드·재측정·무효/유효 bisect 후보

아래도 각 행 exit 0입니다. `X 무효` 행을 후보 커밋 성능으로 해석하면 안 됩니다.

| 로그명(.log) | 명령 / 조건 | start | END |
|---|---|---|---|
| old-dsp-build | C11 S | 16:36:24.135293 | 16:36:40.545371 |
| old-dsp-1 | C06 S | 16:36:54.877801 | 16:37:05.242837 |
| old-dsp-2 | C06 S | 16:37:15.567752 | 16:37:25.822189 |
| old-dsp-3 | C06 S | 16:37:33.132448 | 16:37:43.351746 |
| old-service-build | C12 S | 16:38:34.817986 | 16:39:17.199464 |
| old-service-1 | C08 S | 16:39:25.656022 | 16:39:32.965108 |
| old-service-2 | C08 S | 16:39:39.410379 | 16:39:46.742120 |
| old-service-3 | C08 S | 16:39:56.528918 | 16:40:03.869911 |
| current-scratch-service-build | C12 S | 16:40:59.362583 | 16:41:08.162689 |
| current-scratch-service-1 | C08 S | 16:41:16.786863 | 16:41:24.029658 |
| current-scratch-service-2 | C08 S | 16:41:35.099283 | 16:41:42.398561 |
| current-scratch-service-3 | C08 S | 16:41:50.369859 | 16:41:58.030361 |
| ancestor-service-1 | C08 S | 16:43:20.817477 | 16:43:34.211612 |
| ancestor-service-2 | C08 S | 16:43:44.984892 | 16:43:52.189015 |
| ancestor-service-3 | C08 S | 16:43:58.440805 | 16:44:05.708155 |
| bisect-1-31faf9f-1 | C08 X 무효 | 16:44:43.718828 | 16:44:56.520536 |
| bisect-1-31faf9f-2 | C08 X 무효 | 16:44:56.605277 | 16:45:03.795698 |
| bisect-1-31faf9f-3 | C08 X 무효 | 16:45:03.883940 | 16:45:10.984794 |
| bisect-2-91152b5-1 | C08 X 무효 | 16:45:11.565024 | 16:45:18.657464 |
| bisect-2-91152b5-2 | C08 X 무효 | 16:45:18.742129 | 16:45:25.878656 |
| bisect-2-91152b5-3 | C08 X 무효 | 16:45:25.961552 | 16:45:33.196490 |
| bisect-3-2afe55c-1 | C08 X 무효 | 16:45:33.790643 | 16:45:41.301960 |
| bisect-3-2afe55c-2 | C08 X 무효 | 16:45:41.388664 | 16:45:49.009935 |
| bisect-3-2afe55c-3 | C08 X 무효 | 16:45:49.094151 | 16:45:56.615457 |
| bisect-4-31faf9f-1 | C08 S | 16:46:53.689774 | 16:47:07.221894 |
| bisect-4-31faf9f-2 | C08 S | 16:47:07.309760 | 16:47:14.549219 |
| bisect-4-31faf9f-3 | C08 S | 16:47:14.634832 | 16:47:21.866882 |
| bisect-5-91152b5-1 | C08 S | 16:47:22.457541 | 16:47:29.671853 |
| bisect-5-91152b5-2 | C08 S | 16:47:29.755479 | 16:47:36.967323 |
| bisect-5-91152b5-3 | C08 S | 16:47:37.053431 | 16:47:44.646657 |
| bisect-6-2afe55c-1 | C08 S | 16:47:45.238834 | 16:47:59.123816 |
| bisect-6-2afe55c-2 | C08 S | 16:47:59.205576 | 16:48:06.461474 |
| bisect-6-2afe55c-3 | C08 S | 16:48:06.545740 | 16:48:13.749625 |
| candidate-confirm-1 | C08 S | 16:49:25.172696 | 16:49:32.971725 |
| candidate-confirm-2 | C08 S | 16:49:53.939840 | 16:50:01.224403 |
| candidate-confirm-3 | C08 S | 16:50:15.275329 | 16:50:22.554631 |

### A.2. Git 작업과 bisect의 실제 명령·출력

아래 블록은 M 조건입니다. 각 주석은 start → END, EXIT이며 실제 로그 argv를 그대로 셸 문법으로 썼습니다. 두 `git bisect run`만 S 조건을 적용합니다. reset은 bisect를 끝낸 동작이지 본 트리의 사용자 변경을 되돌린 동작이 아닙니다.

```powershell
# baseline-history 16:36:01.541542 -> 16:36:01.573650 EXIT=0
 git log --all '--format=%h%x20%T%x20%s' --max-count=4 59d9ae3 5307b6b
# add-dsp-worktree 16:36:12.777448 -> 16:36:14.806035 EXIT=0
 git worktree add --detach C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect e98a954
# dsp-worktree-check 16:37:57.450411 -> 16:37:57.713973 EXIT=0; stdout empty
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect status --porcelain --untracked-files=all
# service-baseline-verify 16:38:09.502413 -> 16:38:09.531014 EXIT=0
 git diff 59d9ae3 5307b6b -- crates/impulcifer-service docs/rust/perf/impulcifer-service.md tests/migration/bench_oracle_impulcifer_service.py
# switch-service-baseline 16:38:27.009877 -> 16:38:28.178365 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect switch --detach 59d9ae3
# service-worktree-check 16:40:18.238841 -> 16:40:18.586940 EXIT=0; stdout empty
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect status --porcelain --untracked-files=all
# switch-current-scratch 16:40:52.865186 -> 16:40:53.276959 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect switch --detach e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8
# service-bisect-history 16:42:09.541078 -> 16:42:09.560139 EXIT=0; stdout empty
 git log --oneline --ancestry-path 59d9ae3..e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8
# bisect-start 16:42:27.952004 -> 16:42:28.405316 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect start e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8 59d9ae3
# bisect-merge-base-files 16:42:47.506822 -> 16:42:47.524385 EXIT=0; bench files absent
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect ls-files crates/impulcifer-service/benches/perf.rs crates/impulcifer-service/tests/bench_support/service.rs
# bisect-reset-nonancestor 16:43:05.888214 -> 16:43:06.332663 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect reset
# switch-service-ancestor 16:43:13.482453 -> 16:43:13.858029 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect switch --detach 5307b6b
# bisect-ancestor-start 16:44:33.647891 -> 16:44:34.072993 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect start e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8 5307b6b
# S: bisect-run 16:44:43.499080 -> 16:45:56.668379 EXIT=0; INVALID cwd in child calls
 git bisect run py -3.14 E:/Impulcifer/target/pa06b_bisect.py
# M: invalid-bisect-log 16:46:25.640917 -> 16:46:25.657884 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect log
# invalid-bisect-reset 16:46:33.715066 -> 16:46:34.119856 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect reset
# bisect-corrected-start 16:46:43.335682 -> 16:46:43.740237 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect start e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8 5307b6b
# S: bisect-corrected-run 16:46:53.472387 -> 16:48:13.797830 EXIT=0
 git bisect run py -3.14 E:/Impulcifer/target/pa06b_bisect.py
# M: bisect-candidate-diff 16:48:33.524852 -> 16:48:33.545557 EXIT=0
 git show --stat --oneline e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8
# final-bisect-log 16:48:41.341189 -> 16:48:41.363486 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect log
# final-bisect-reset 16:48:58.519348 -> 16:48:58.906916 EXIT=0
 git -C C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect bisect reset e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8
# remove-owned-worktree 17:42:02.514037 -> 17:42:03.169336 EXIT=0; stdout empty
 git worktree remove --force C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect
```

비조상 시작의 실제 출력은 다음과 같습니다.

```text
Bisecting: a merge base must be tested
[7ed262a6dae0b4d54f5e95f1011bcb3d38264bba] feat(rust): 업데이터, check_for_updates·start_update·apply_pending_update와 설치 종류별 실행기 (P20)
```

유효 helper의 판정 출력과 Git의 마지막 출력도 보존합니다. `build`는 실행기의 누적 후보 호출 번호이며 전체 의존성 재빌드 횟수가 아닙니다.

```text
{"build": 4, "head": "31faf9fd176e4b07e489010c29e89b6ef82a7e2f", "rust_ms": [0.001445, 0.001466, 0.001471], "median": 0.001466, "threshold_ms": 0.00154, "bad": false}
{"build": 5, "head": "91152b58a0ce3bf87bc00d8bc544458614878362", "rust_ms": [0.0014535, 0.001461, 0.0014495], "median": 0.0014535, "threshold_ms": 0.00154, "bad": false}
{"build": 6, "head": "2afe55cbff72b9cf4a31d86ae8ec99e452933a12", "rust_ms": [0.001476, 0.0014735, 0.001489], "median": 0.001476, "threshold_ms": 0.00154, "bad": false}
e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8 is the first 'bad' commit
 .../rust/packets/PA06b-astra-perf-release-rerun.md | 38 ++++++++++++++++++++++
 1 file changed, 38 insertions(+)
 create mode 100644 docs/rust/packets/PA06b-astra-perf-release-rerun.md
bisect found first 'bad' commit
```

`invalid-bisect-log.log`와 `final-bisect-log.log`의 Git 명령·판정 본문은 동일했습니다. 전자는 잘못된 cwd에서 얻은 수치, 후자는 올바른 cwd의 수치로 판단했다는 차이가 있습니다. 아래는 두 로그에 공통으로 남은 Git 출력입니다. 사람이 후보 결과를 수동 good로 바꿨다는 뜻이 아니라 `git bisect run`이 기록한 판정입니다.

```text
# bad: [e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8] docs(rust): PA06b(출시 성능 감사 2차, 회귀 이분 탐색 필수) 작업서
# good: [5307b6b88d2a1e158ccdf5c0cd4f6210a593894a] perf(rust): 서비스 계층 성능 감사, IPC·작업 이벤트 9개 연산 전부 파이썬보다 빠름 (PA04)
git bisect start 'e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8' '5307b6b'
# good: [31faf9fd176e4b07e489010c29e89b6ef82a7e2f] perf(rust): 오디오 계층 성능 감사, 열거 캐시·동시 세션 초기화·이벤트 구동 캡처와 CABLE-A 무결성 실측 (PA05)
git bisect good 31faf9fd176e4b07e489010c29e89b6ef82a7e2f
# good: [91152b58a0ce3bf87bc00d8bc544458614878362] docs(rust): PA05b(실시간 스레드 승격과 작은 shared 버퍼로 오디오 wall time을 PortAudio 이하로) 작업서
git bisect good 91152b58a0ce3bf87bc00d8bc544458614878362
# good: [2afe55cbff72b9cf4a31d86ae8ec99e452933a12] perf(rust): WASAPI 세션 스레드 MMCSS 승격과 shared 버퍼 주기 실험, 7차 오디오 측정 (PA05b, 기준 미달)
git bisect good 2afe55cbff72b9cf4a31d86ae8ec99e452933a12
# first 'bad' commit: [e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8] docs(rust): PA06b(출시 성능 감사 2차, 회귀 이분 탐색 필수) 작업서
```

유효 후보의 비교 대상 원문 median/min(ms)도 보존합니다.

```text
bisect-4-31faf9f-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001445000 | 0.001369500 |
bisect-4-31faf9f-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001466000 | 0.001378000 |
bisect-4-31faf9f-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001471000 | 0.001385500 |
bisect-5-91152b5-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001453500 | 0.001375500 |
bisect-5-91152b5-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001461000 | 0.001372000 |
bisect-5-91152b5-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001449500 | 0.001382500 |
bisect-6-2afe55c-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001476000 | 0.001380500 |
bisect-6-2afe55c-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001473500 | 0.001372000 |
bisect-6-2afe55c-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001489000 | 0.001386500 |
```

이 ‘bad’ 출력은 원인 확정이 아닙니다. 최초 bad로 지정한 HEAD의 재측정은 0.001391ms로 good 범위였습니다. 잘못된 첫 bisect도 같은 문서 커밋을 가리켰지만 그 후보 측정은 main tree였으므로 전부 폐기했습니다. 소스를 고치는 대신 불안정한 실측 판정과 무효 실행을 보고합니다.

#### 무시된 실행기 재현에 필요한 정확한 helper

`target/pa06b_bisect.py`가 clone에 없으므로 최종 helper의 본문을 보존합니다. 과거 첫 시도는 `subprocess.run`의 `env=dict(os.environ, PA06B_CWD=os.getcwd())` 전달이 없었습니다. 아래 최종 코드를 첫 무효 실행에 사용했다고 주장하지 않습니다. 로컬 카운터 파일은 무효 실행 후 3, 유효 실행 후 6이었으며 성공 로그를 덮어쓰지 않았습니다.

```python
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys

root = Path('E:/Impulcifer/target/pa06b')
state = root / 'bisect-count.json'
count = json.loads(state.read_text()) if state.exists() else 0
if count >= 6:
    print('Six-build limit reached', flush=True)
    raise SystemExit(128)
count += 1
state.write_text(json.dumps(count))
runner = 'E:/Impulcifer/target/pa06b_run.py'
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
values = []
for i in range(1, 4):
    name = f'bisect-{count}-{head[:7]}-{i}'
    result = subprocess.run(['py', '-3.14', runner, name, 'cargo', 'bench', '-p', 'impulcifer-service', '--bench', 'perf'], env=dict(os.environ, PA06B_CWD=os.getcwd()))
    if result.returncode:
        raise SystemExit(125)
    lines = (root / (name + '.log')).read_text(encoding='utf-8').splitlines()
    matches = [float(line.split('|')[3]) for line in lines if line.startswith('| resolve_recording_paths |')]
    if len(matches) != 1:
        raise SystemExit(125)
    values.extend(matches)
median = statistics.median(values)
print(json.dumps({'build': count, 'head': head, 'rust_ms': values, 'median': median, 'threshold_ms': 0.00154, 'bad': median > 0.00154}), flush=True)
raise SystemExit(int(median > 0.00154))
```

하위 `target/pa06b_run.py`의 실제 본문입니다. 이 문서에 적은 C01~C30은 이 실행기가 호출한 자식 argv입니다.

```python
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path('E:/Impulcifer')
LOG = ROOT / 'target/pa06b'
LOG.mkdir(exist_ok=True)
name, *command = sys.argv[1:]
path = LOG / (name + '.log')
if path.exists():
    raise SystemExit('Refusing to overwrite ' + str(path))
env = dict(os.environ, PYTHONIOENCODING='utf-8', PYTHONDONTWRITEBYTECODE='1')
cwd = env.pop('PA06B_CWD', str(ROOT))
start = datetime.datetime.now().astimezone().isoformat()
with path.open('x', encoding='utf-8') as log:
    meta = {'start': start, 'cwd': cwd, 'argv': command, 'environment': {k: env.get(k) for k in ['CARGO_TARGET_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'IMPULCIFER_PERF_DIR', 'IMPULCIFER_PA05_SHARED_PERIODS']}}
    log.write(json.dumps(meta, ensure_ascii=False) + '\n')
    log.flush()
    print(json.dumps(meta, ensure_ascii=False), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, stdout=log, stderr=subprocess.STDOUT)
    end = datetime.datetime.now().astimezone().isoformat()
    log.write(f'\nEND={end} EXIT={result.returncode}\n')
text = path.read_text(encoding='utf-8')
for line in text.splitlines():
    if line.startswith(('|', 'END=', 'test result:', 'error:', 'FAILED', 'fatal:', 'Preparing worktree', 'HEAD is now')) or 'Finished `' in line:
        print(line, flush=True)
print('FULL_LOG=' + str(path), flush=True)
raise SystemExit(result.returncode)
```

### A.3. IO의 전체 원시 벤치 표

아래 압축 표의 각 셀은 원문 **median / min(ms)**입니다. R1/P1~R3/P3은 `io-python-1`~`3`에 함께 출력한 Rust/Python 값이며, RFT/PFT는 `io-python-ft`입니다. 반올림을 추가하지 않았습니다. 원문에 이미 있는 ratio 행은 2절·3절과 중복되어 여기서는 제외했습니다.

| op | size | R1 median/min | P1 median/min | R2 median/min | P2 median/min | R3 median/min | P3 median/min |
|---|---|---|---|---|---|---|---|
| write_pcm32_32tracks | 32x96000 | 11.251100 / 10.239300 | 24.447500 / 23.547100 | 11.823300 / 11.032300 | 24.988700 / 23.534900 | 10.838200 / 10.198700 | 24.001800 / 23.199700 |
| write_pcm32_30tracks | 30x96000 | 10.205900 / 9.425200 | 22.707300 / 21.753500 | 9.143600 / 8.944600 | 22.235200 / 21.791100 | 8.962600 / 8.642200 | 22.018300 / 21.758600 |
| write_pcm16_2tracks | 2x295000 | 1.797500 / 1.514000 | 5.680200 / 5.375000 | 1.504100 / 1.430100 | 6.034800 / 5.353800 | 1.519000 / 1.420000 | 5.477300 / 5.343200 |
| read_pcm32_32tracks | 32x96000 | 7.243700 / 6.259000 | 7.607900 / 7.243400 | 5.320300 / 5.055000 | 7.564300 / 6.992200 | 5.377900 / 5.146600 | 7.558900 / 7.237500 |
| read_bundled_sweep | 1x295270 | 0.751000 / 0.706000 | 0.682600 / 0.610500 | 0.472200 / 0.436900 | 0.690300 / 0.632600 | 0.563600 / 0.496600 | 0.690800 / 0.639100 |
| read_demo_recording | 2x878540 | 3.549900 / 3.180000 | 3.915300 / 3.673400 | 2.829800 / 2.535200 | 3.918700 / 3.661800 | 2.696500 / 2.478500 | 3.949900 / 3.630400 |
| read_float32_8tracks | 8x480000 | 7.930600 / 6.634900 | 9.521000 / 9.255800 | 6.958800 / 6.509500 | 9.629200 / 9.145100 | 6.850500 / 6.652400 | 9.510700 / 9.288600 |
| pcm32_round_trip | 30x96000 | 8.924800 / 8.542800 | 43.028800 / 41.004600 | 9.341300 / 8.615900 | 41.607000 / 41.105800 | 8.911500 / 8.436500 | 41.479100 / 40.574000 |
| sweep_file_name_parse | 10000 | 3.122600 / 3.068600 | 4.199400 / 4.038800 | 3.101600 / 3.039200 | 4.120500 / 3.997800 | 3.117300 / 3.032100 | 4.170800 / 4.003100 |

| op | size | standalone Rust median/min | RFT median/min | PFT median/min |
|---|---|---|---|---|
| write_pcm32_32tracks | 32x96000 | 11.047600 / 10.281500 | 10.660100 / 10.335400 | 23.362200 / 22.903200 |
| write_pcm32_30tracks | 30x96000 | 9.282600 / 8.783400 | 8.835400 / 8.574400 | 22.133600 / 21.717000 |
| write_pcm16_2tracks | 2x295000 | 1.625000 / 1.511300 | 1.575700 / 1.452300 | 5.565200 / 5.406700 |
| read_pcm32_32tracks | 32x96000 | 5.321300 / 5.043000 | 5.518600 / 4.917000 | 7.628000 / 7.325800 |
| read_bundled_sweep | 1x295270 | 0.609600 / 0.511300 | 0.467200 / 0.430300 | 0.710900 / 0.645500 |
| read_demo_recording | 2x878540 | 2.843600 / 2.535300 | 2.809700 / 2.523000 | 3.830800 / 3.687200 |
| read_float32_8tracks | 8x480000 | 6.676700 / 6.497500 | 6.771900 / 6.574100 | 9.308500 / 9.214500 |
| pcm32_round_trip | 30x96000 | 9.020800 / 8.626200 | 8.913200 / 8.364400 | 36.545900 / 35.465300 |
| sweep_file_name_parse | 10000 | 3.088200 / 3.035600 | 3.090800 / 3.050200 | 3.528600 / 3.459600 |

### A.4. DSP의 전체 원시 벤치 표

R1/P1~R3/P3은 `dsp-rust-N`과 `dsp-python-N`의 원문 median/min(ms)입니다.

| op | size | R1 median/min | P1 median/min | R2 median/min | P2 median/min | R3 median/min | P3 median/min |
|---|---|---|---|---|---|---|---|
| convolve_full_ir_fir | 96000 x 9600 | 2.321700 / 1.939500 | 4.218400 / 3.454100 | 2.288300 / 1.891400 | 3.979200 / 3.621000 | 2.314400 / 1.897100 | 3.833300 / 3.569900 |
| convolve_same_estimate | 391000 x 295000 | 14.189400 / 13.455200 | 28.015300 / 25.933100 | 14.615500 / 13.547000 | 26.861000 / 25.785500 | 14.370800 / 13.736000 | 27.214400 / 25.204100 |
| correlate_full_30ms | 8 x 1440 x 1440 | 0.120900 / 0.120600 | 1.192300 / 1.175700 | 0.122200 / 0.121500 | 1.199700 / 1.170900 | 0.121000 / 0.120800 | 1.215600 / 1.190200 |
| rfft_irfft_96000 | 96000 | 0.967200 / 0.920500 | 2.120800 / 1.953500 | 0.985300 / 0.930000 | 1.490600 / 1.455700 | 0.946700 / 0.926800 | 1.549300 / 1.422300 |
| magnitude_response_96000 | 96000 | 0.395800 / 0.393900 | 1.449800 / 1.367400 | 0.398400 / 0.389700 | 1.117400 / 1.026500 | 0.396400 / 0.392900 | 1.104800 / 1.050800 |
| butter8_sosfilt_96000 | 96000 | 0.649400 / 0.636900 | 1.002800 / 0.901200 | 0.667200 / 0.637700 | 0.933100 / 0.899500 | 0.652500 / 0.638900 | 0.929800 / 0.901000 |
| firwin2_19200 | 19200 taps / 9600 mesh | 1.125800 / 1.101500 | 2.399000 / 2.295700 | 1.139800 / 0.979800 | 2.668300 / 2.465100 | 1.184100 / 1.097500 | 2.386000 / 2.274300 |
| minimum_phase_19200 | 19200 | 0.855900 / 0.836700 | 1.070000 / 1.051000 | 0.889900 / 0.845000 | 1.065300 / 1.049600 | 0.848900 / 0.830600 | 1.085300 / 1.063600 |
| savgol_heavy_light_783 | 783 | 0.067700 / 0.067300 | 1.189900 / 1.167600 | 0.069700 / 0.069200 | 1.382900 / 1.229700 | 0.067200 / 0.067000 | 1.193000 / 1.148200 |
| find_peaks_96000 | 96000 | 0.986300 / 0.943900 | 1.202200 / 1.133700 | 0.979500 / 0.929800 | 0.925100 / 0.909100 | 0.982600 / 0.935300 | 1.163700 / 1.146100 |
| first_peak_index_96000 | 96000 | 1.340400 / 1.293500 | 1.829000 / 1.701100 | 1.262800 / 1.240200 | 2.006600 / 1.679200 | 1.343700 / 1.292900 | 1.970600 / 1.880000 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.073900 / 0.072700 | 0.105100 / 0.098200 | 0.074000 / 0.071800 | 0.123400 / 0.120500 | 0.072800 / 0.071900 | 0.102400 / 0.097800 |
| spline_k2_783 | 783 | 0.031800 / 0.031300 | 0.104700 / 0.084000 | 0.031300 / 0.031100 | 0.085300 / 0.084000 | 0.031500 / 0.031200 | 0.081600 / 0.081100 |
| spline_k3_783 | 783 | 0.040800 / 0.040500 | 0.107900 / 0.103100 | 0.040900 / 0.040600 | 0.107100 / 0.105100 | 0.040700 / 0.040400 | 0.101900 / 0.100500 |
| linregress_1000 | 1000 | 0.001800 / 0.001800 | 0.252200 / 0.233800 | 0.001800 / 0.001800 | 0.271500 / 0.238000 | 0.001800 / 0.001800 | 0.247400 / 0.227800 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 4.673100 / 4.411300 | 9.747700 / 9.307300 | 4.788900 / 4.411100 | 9.610100 / 8.777300 | 5.037400 / 4.448200 | 9.285100 / 9.021900 |
| resample_poly_96000_48k_44k1 | 96000 | 4.825400 / 4.660800 | 8.479200 / 8.339500 | 4.711000 / 4.661100 | 8.026500 / 7.896600 | 4.708500 / 4.640000 | 7.997100 / 7.909600 |
| nnresample_96000_48k_96k | 96000 | 624.503900 / 620.089200 | 1526.707500 / 1514.873100 | 623.815500 / 621.505900 | 1522.161000 / 1510.487000 | 624.011200 / 620.721000 | 1522.671900 / 1513.050300 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 4.049800 / 3.768000 | 11.856600 / 11.004200 | 3.934000 / 3.654800 | 12.134500 / 11.440000 | 3.948500 / 3.746400 | 12.729500 / 12.087600 |
| windows_32001 | 32001 / 19200 / 19200 | 0.612500 / 0.612000 | 1.282800 / 1.212400 | 0.619800 / 0.612000 | 1.221300 / 1.204600 | 0.612400 / 0.612000 | 1.219100 / 1.205600 |
| expit_783 | 783 | 0.002800 / 0.002700 | 0.003400 / 0.003300 | 0.002800 / 0.002700 | 0.003400 / 0.003300 | 0.002800 / 0.002600 | 0.003400 / 0.003300 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000010 / 0.000009 | 0.000270 / 0.000268 | 0.000009 / 0.000009 | 0.000270 / 0.000267 | 0.000010 / 0.000010 | 0.000271 / 0.000269 |

과거 전체 DSP 벤치 중 판단에 사용한 원문 행은 다음과 같습니다. 나머지 과거 연산은 새 승인 표의 분모로 쓰지 않았습니다.

```text
old-dsp-1.log
| rfft_irfft_96000 | 96000 | 1.028600 | 0.923700 |
| find_peaks_96000 | 96000 | 0.967200 | 0.920700 |
old-dsp-2.log
| rfft_irfft_96000 | 96000 | 1.152800 | 1.035400 |
| find_peaks_96000 | 96000 | 1.005800 | 0.955700 |
old-dsp-3.log
| rfft_irfft_96000 | 96000 | 1.034400 | 0.976000 |
| find_peaks_96000 | 96000 | 0.940500 | 0.908300 |
```

### A.5. Service의 전체 원시 벤치 표

모든 값은 batch 전체가 아닌 **per unit median/min(ms)**입니다. 일반 Python 열은 한 번, Rust·FT 열은 세 번의 독립 실행입니다.

| op | size | R1 median/min | 일반 P median/min | R2 median/min | R3 median/min |
|---|---|---|---|---|---|
| bootstrap | 200/batch; per unit | 0.241809000 / 0.239517000 | 1.031435000 / 1.004522500 | 0.246946500 / 0.242986500 | 0.245942000 / 0.241983500 |
| get_ui_settings | 200/batch; per unit | 0.147306000 / 0.145179500 | 0.374135000 / 0.368607000 | 0.150061500 / 0.144003000 | 0.146238500 / 0.143713000 |
| set_language_round_trip | 100/batch; per unit | 2.087608000 / 2.013470000 | 3.159274000 / 3.114301000 | 2.008806000 / 1.899527000 | 2.019909000 / 1.894047000 |
| resolve_recording_paths | 200/batch; per unit | 0.001301500 / 0.001208000 | 0.002932500 / 0.002883000 | 0.001201000 / 0.001195000 | 0.001206500 / 0.001195500 |
| start_brir_to_first_event | 5/batch; per unit | 0.328700000 / 0.317140000 | 0.482499995 / 0.467259996 | 0.324120000 / 0.320840000 | 0.340680000 / 0.320520000 |
| poll_job_drain | 5/batch; per unit | 1.100500000 / 1.048900000 | 4.780920001 / 4.640399991 | 0.991620000 / 0.837060000 | 1.024540000 / 0.770180000 |
| job_event_emit | 5/batch; per unit | 2.767760000 / 2.729180000 | 43.606619991 / 43.412499997 | 2.833680000 / 2.715900000 | 2.779000000 / 2.683020000 |
| detect_sweep | 20/batch; per unit | 0.213335000 / 0.202445000 | 0.576785000 / 0.544240000 | 0.223125000 / 0.199790000 | 0.213515000 / 0.200245000 |
| catalog_translate | 100000/batch; per unit | 0.000158517 / 0.000156527 | 0.000436355 / 0.000431605 | 0.000159184 / 0.000156928 | 0.000161624 / 0.000157617 |

| op | FT P1 median/min | FT P2 median/min | FT P3 median/min |
|---|---|---|---|
| bootstrap | 1.073741000 / 1.061004000 | 1.094072500 / 1.059718500 | 1.074343500 / 1.062470000 |
| get_ui_settings | 0.386837500 / 0.376926000 | 0.397472000 / 0.384414500 | 0.401604500 / 0.384645500 |
| set_language_round_trip | 3.255001000 / 3.179003000 | 3.256879000 / 3.188595000 | 3.314900000 / 3.163335000 |
| resolve_recording_paths | 0.003030000 / 0.002954000 | 0.003153500 / 0.003042000 | 0.003197500 / 0.003054000 |
| start_brir_to_first_event | 0.521320006 / 0.486399990 | 0.540499995 / 0.504080002 | 0.514519995 / 0.497960002 |
| poll_job_drain | 5.144019990 / 5.053019995 | 5.286139995 / 5.175779981 | 5.109019991 / 5.062160012 |
| job_event_emit | 46.853800002 / 46.269799996 | 47.157600010 / 46.585620003 | 47.274259990 / 46.702680009 |
| detect_sweep | 0.578569999 / 0.550570000 | 0.606525000 / 0.551410000 | 0.590085001 / 0.541300001 |
| catalog_translate | 0.000522791 / 0.000493702 | 0.000507583 / 0.000496482 | 0.000496877 / 0.000492883 |

과거·scratch·재확인에서 판단에 쓴 원문 행도 보존합니다.

```text
old-service-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001427500 | 0.001390500 |
old-service-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001464500 | 0.001399000 |
old-service-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001443500 | 0.001397000 |
current-scratch-service-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001620500 | 0.001557000 |
current-scratch-service-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001627000 | 0.001541000 |
current-scratch-service-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001644500 | 0.001557000 |
ancestor-service-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001460000 | 0.001368500 |
ancestor-service-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001442000 | 0.001372000 |
ancestor-service-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001453500 | 0.001393000 |
candidate-confirm-1.log
| resolve_recording_paths | 200/batch; per unit | 0.001391000 | 0.001377500 |
candidate-confirm-2.log
| resolve_recording_paths | 200/batch; per unit | 0.001492500 | 0.001382500 |
candidate-confirm-3.log
| resolve_recording_paths | 200/batch; per unit | 0.001387000 | 0.001374500 |
```

### A.6. Pipeline 추가 벤치와 오디오 전체 원문 표

메모리를 포함한 pipeline 오라클 표는 4절에 전부 보존했습니다. 별도 service pipeline 벤치는 아래와 같습니다. 오라클의 process/service 분모와 섞지 않습니다.

```text
pipeline-rust.log
| scenario | rust median ms | rust min ms |
| default | 1017.969100 | 1006.541300 |
| vbass | 1031.375000 | 1028.049800 |
```

다음은 오디오 로그의 표 행을 그대로 옮긴 것입니다. 헤더와 행 사이의 trace·진행 출력만 뺐습니다.

```text
audio-rust-standalone.log
| op | size | rust median ms | rust min ms |
| enumerate_backend | 20 calls/batch | 0.026100 | 0.026000 |
| open_close_session | 20 calls/batch | 176.193400 | 175.854100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6201.142900 | 6200.357500 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 49.684567 | 48.899167 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43129.105100 | 43128.621200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 68.896767 | 68.412867 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 32.490400 | 20.956900 |

audio-rust-observed.log
| op | size | rust median ms | rust min ms |
| enumerate_backend | 20 calls/batch | 0.026000 | 0.025700 |
| open_close_session | 20 calls/batch | 182.210900 | 176.457400 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6207.127100 | 6205.997600 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 55.668767 | 54.539267 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43131.861200 | 43131.631200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 71.652867 | 71.422867 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 27.355650 | 19.784800 |
| capture_loop_cpu | process user+kernel; 5 runs | 46.875000 | 15.625000 |

sys-rust.log
| op | size | rust median ms | rust min ms |
| enumerate_backend | 20 calls/batch | 267.252500 | 263.307200 |

audio-python-explicit.log
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.002100 | 1.982400 |
| open_close_session | 20 calls/batch | 233.408400 | 226.663600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6188.695000 | 6187.974000 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 37.236667 | 36.515667 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 15.625000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43115.502900 | 43112.289600 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 55.294567 | 52.081267 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.147200 | 17.293600 |

audio-python-ft-explicit.log
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.044300 | 2.019700 |
| open_close_session | 20 calls/batch | 235.360200 | 232.576600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6191.237800 | 6189.755500 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 39.779467 | 38.297167 |
| capture_loop_cpu | process user+kernel; 5 runs | 78.125000 | 15.625000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43112.659300 | 43111.913800 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 52.450967 | 51.705467 |
| first_sample_latency | input start to nonempty callback; 10 runs | 23.135600 | 14.375900 |

audio-python-production.log
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.004600 | 1.977600 |
| open_close_session | 20 calls/batch | 230.944800 | 224.974600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6203.904200 | 6200.797000 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 52.445867 | 49.338667 |
| capture_loop_cpu | process user+kernel; 5 runs | 109.375000 | 93.750000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43129.899100 | 43123.706600 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 69.690767 | 63.498267 |
| first_sample_latency | input start to nonempty callback; 10 runs | 34.744500 | 21.676600 |

audio-python-ft-production.log
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.026900 | 1.983100 |
| open_close_session | 20 calls/batch | 237.309200 | 233.285200 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6204.608500 | 6201.990100 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 53.150167 | 50.531767 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 78.125000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43140.901900 | 43133.928700 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 80.693567 | 73.720367 |
| first_sample_latency | input start to nonempty callback; 10 runs | 34.625600 | 21.485300 |
```

### A.7. 실제 빌드·fmt·clippy·policy 출력

시각·cwd/env·argv는 A.1에 있으므로 아래에는 출력 본문을 보존합니다. fmt의 본문은 비어 있었습니다.

```text
release-build.log
   Compiling audio_thread_priority v0.37.0
   Compiling impulcifer-sys-win v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-sys-win)
   Compiling impulcifer-audio-io v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-audio-io)
   Compiling impulcifer-service v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-service)
   Compiling impulcifer-cli v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-cli)
   Compiling impulcifer-app v3.0.0-alpha.0 (E:\Impulcifer\apps\impulcifer-app)
   Compiling impulcifer-python v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-python)
    Finished `release` profile [optimized] target(s) in 37.69s
END=2026-09-08T16:27:06.213599+09:00 EXIT=0

fmt.log
END=2026-09-08T17:37:58.847997+09:00 EXIT=0

clippy.log
    Checking audio_thread_priority v0.37.0
    Checking impulcifer-sys-win v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-sys-win)
    Checking impulcifer-audio-io v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-audio-io)
    Checking impulcifer-service v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-service)
    Checking impulcifer-cli v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-cli)
    Checking impulcifer-app v3.0.0-alpha.0 (E:\Impulcifer\apps\impulcifer-app)
    Checking impulcifer-python v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-python)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.66s
END=2026-09-08T17:38:11.264670+09:00 EXIT=0

policy.log
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.21s
     Running unittests src\lib.rs (target\debug\deps\impulcifer_policy-6f97dbb98fd5d3f8.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\gates.rs (target\debug\deps\gates-0c417439c6ccb24f.exe)

running 6 tests
test subprocess_scanner_checks_code_raw_text_and_exact_allowlist ... ok
test every_crate_root_forbids_unsafe ... ok
test canonical_features_registered ... ok
test no_shell_subprocesses ... ok
test no_unsafe_outside_budget ... ok
test implemented_features_have_existing_tests ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.94s

   Doc-tests impulcifer_policy

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
END=2026-09-08T17:37:47.839154+09:00 EXIT=0
```

### A.8. Workspace 전체 시험 요약

`workspace-tests.log`의 모든 시험 대상과 결과를 아래에 보존했습니다. 표의 passed/failed/ignored/measured/filtered 및 초는 실제 `test result:` 행을 옮긴 것입니다. 빈 unit/doc 시험도 포함하므로 합계를 검산할 수 있습니다. 로그 전체 808줄의 개별 성공 이름과 반복된 실행 파일 해시를 모두 전재하지는 않았습니다.

| crate / test target | passed | failed | ignored | measured | filtered | seconds |
|---|---:|---:|---:|---:|---:|---:|
| analysis lib | 0 | 0 | 0 | 0 | 0 | 0.00 |
| app main | 5 | 0 | 0 | 0 | 0 | 0.01 |
| app packaging | 3 | 0 | 1 | 0 | 0 | 7.80 |
| app smoke | 5 | 0 | 2 | 0 | 0 | 0.07 |
| audio-io lib | 11 | 0 | 0 | 0 | 0 | 0.00 |
| audio-io hardware | 0 | 0 | 2 | 0 | 0 | 0.00 |
| audio-io session_fake | 14 | 0 | 0 | 0 | 0 | 0.24 |
| cli lib | 2 | 0 | 0 | 0 | 0 | 0.00 |
| cli main | 0 | 0 | 0 | 0 | 0 | 0.00 |
| cli cli | 10 | 0 | 0 | 0 | 0 | 22.41 |
| dsp lib | 9 | 0 | 0 | 0 | 0 | 0.05 |
| dsp golden_batch1 | 19 | 0 | 0 | 0 | 0 | 0.08 |
| dsp golden_batch2 | 12 | 0 | 0 | 0 | 0 | 1.79 |
| dsp golden_batch3 | 7 | 0 | 0 | 0 | 0 | 11.67 |
| dsp golden_brir_objects | 15 | 0 | 0 | 0 | 0 | 12.35 |
| dsp golden_eqapo | 3 | 0 | 0 | 0 | 0 | 0.14 |
| dsp golden_fr | 12 | 0 | 0 | 0 | 0 | 0.22 |
| dsp golden_stages | 15 | 0 | 0 | 0 | 0 | 37.17 |
| dsp perf_smoke | 1 | 0 | 0 | 0 | 0 | 0.33 |
| dsp properties_batch1 | 7 | 0 | 0 | 0 | 0 | 0.01 |
| dsp properties_batch2 | 5 | 0 | 0 | 0 | 0 | 0.02 |
| dsp properties_batch3 | 7 | 0 | 0 | 0 | 0 | 0.87 |
| dsp properties_brir_objects | 11 | 0 | 0 | 0 | 0 | 0.00 |
| dsp properties_eqapo | 6 | 0 | 0 | 0 | 0 | 0.01 |
| dsp properties_fr | 6 | 0 | 0 | 0 | 0 | 0.02 |
| dsp properties_perf | 6 | 0 | 0 | 0 | 0 | 0.38 |
| dsp properties_stages | 9 | 0 | 0 | 0 | 0 | 0.01 |
| io lib | 15 | 0 | 0 | 0 | 0 | 0.98 |
| io brir_layout | 4 | 0 | 0 | 0 | 0 | 0.00 |
| io perf_smoke | 6 | 0 | 0 | 0 | 0 | 1.68 |
| jobs lib | 2 | 0 | 0 | 0 | 0 | 0.00 |
| jobs registry | 11 | 0 | 0 | 0 | 0 | 0.01 |
| plots lib | 0 | 0 | 0 | 0 | 0 | 0.00 |
| plots render | 4 | 0 | 0 | 0 | 0 | 3.18 |
| policy lib | 0 | 0 | 0 | 0 | 0 | 0.00 |
| policy gates | 6 | 0 | 0 | 0 | 0 | 1.92 |
| native lib | 0 | 0 | 0 | 0 | 0 | 0.00 |
| native config | 1 | 0 | 0 | 0 | 0 | 0.00 |
| service lib | 15 | 0 | 0 | 0 | 0 | 1.76 |
| service brir_inputs | 3 | 0 | 0 | 0 | 0 | 24.31 |
| service brir_ipc | 7 | 0 | 0 | 0 | 0 | 22.33 |
| service brir_optional | 3 | 0 | 0 | 0 | 0 | 32.33 |
| service brir_outputs | 3 | 0 | 0 | 0 | 0 | 21.25 |
| service brir_plots | 13 | 0 | 0 | 0 | 0 | 52.22 |
| service demo_parity | 2 | 0 | 0 | 0 | 0 | 21.88 |
| service ipc | 23 | 0 | 0 | 0 | 0 | 0.06 |
| service perf_smoke | 2 | 0 | 0 | 0 | 0 | 31.85 |
| service readme_trace | 1 | 0 | 0 | 0 | 0 | 46.08 |
| service recording | 13 | 0 | 1 | 0 | 0 | 3.91 |
| service recovery | 5 | 0 | 0 | 0 | 0 | 1.84 |
| service sweep_grid | 1 | 0 | 0 | 0 | 0 | 0.00 |
| service updater | 20 | 0 | 1 | 0 | 0 | 5.11 |
| sys-win lib | 15 | 0 | 0 | 0 | 0 | 0.00 |
| sys-win bench_smoke | 1 | 0 | 0 | 0 | 0 | 0.02 |
| sys-win hardware | 0 | 0 | 3 | 0 | 0 | 0.00 |
| types lib | 3 | 0 | 0 | 0 | 0 | 0.00 |
| analysis doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| audio-io doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| cli doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| dsp doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| io doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| jobs doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| plots doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| policy doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| native doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| service doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| sys-win doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| types doc | 0 | 0 | 0 | 0 | 0 | 0.00 |
| 합계 | 379 | 0 | 10 | 0 | 0 | 합산 시간으로 wall을 대체하지 않음 |

요구사항과 직접 관계있는 원문 성공 행입니다.

```text
test updater_plugin_registered_with_public_key ... ok
test bench_smoke_impulcifer_audio_io ... ok
test bench_smoke_impulcifer_dsp ... ok
test bench_smoke_impulcifer_io ... ok
test plots_do_not_change_wavs ... ok
test demo_brir_matches_python_within_budget ... ok
test demo_vbass_matches_python_within_budget ... ok
test bench_smoke_impulcifer_service ... ok
test bench_smoke_pipeline_demo ... ok
test legacy_executor_downloads_verifies_and_opens ... ok
test bench_smoke_impulcifer_sys_win ... ok
END=2026-09-08T17:37:37.963914+09:00 EXIT=0
```

### A.9. Updater 연속 시험과 CABLE 실기 원문

연속 실행기 `py -3.14 E:/Impulcifer/target/pa06b_flake.py`는 M 조건에서 17:38:30.615037에 시작해 17:38:45.979196에 exit 0으로 끝났습니다. 실제 10개 자식 argv와 시각은 A.1 C28에 전부 기록했습니다. 각 실행에서 `test legacy_executor_downloads_verifies_and_opens ... ok`를 출력했으며 아래는 모든 결과 행입니다.

```text
legacy-flake-1.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 2.08s
legacy-flake-2.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.19s
legacy-flake-3.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.24s
legacy-flake-4.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.22s
legacy-flake-5.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 2.17s
legacy-flake-6.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.22s
legacy-flake-7.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.26s
legacy-flake-8.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.19s
legacy-flake-9.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.18s
legacy-flake-10.log
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out; finished in 0.28s

recording-hardware.log
   Compiling impulcifer-service v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-service)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.42s
     Running tests\recording.rs (target\debug\deps\recording-ba2133967e0bd93e.exe)

running 1 test
test recording_virtual_cable_end_to_end ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 23.31s
END=2026-09-08T17:39:35.393989+09:00 EXIT=0
```

### A.10. 최종 확인과 플롯 검사 정정 명령

`calculations.log`는 M 조건에서 `py -3.14 E:/Impulcifer/target/pa06b_analyze.py`를 17:41:21.305344~17:41:21.410520(exit 0)에 실행한 결과입니다. 계산 결과의 다섯 집계는 3절과 동일합니다. 최초 플롯 검사 False도 4절에 보존했습니다. 아래 두 명령은 로그 첫 줄의 실제 `-c` 내용을 PowerShell 단일 인용 here-string으로 옮긴 것입니다. 모두 M 조건이며, 새로 실행한 명령이 아닙니다.

```powershell
# comparison-preflight 16:35:40.880608 -> 16:35:41.028198 EXIT=0
py -3.14 -c @'
from pathlib import Path; import subprocess; roots=[Path('C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect'),Path('C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect-target')]; [(print(str(p), 'exists=',p.exists()),print([str(x) for x in p.iterdir()] if p.exists() else [])) for p in roots]; subprocess.run(['git','worktree','list','--porcelain'],check=True); subprocess.run(['git','show','--no-patch','--format=fuller','59d9ae3'],check=True); subprocess.run(['git','log','--format=%h %s','--','docs/rust/perf/impulcifer-service.md'],check=True)
'@
# plot-and-cleanup-evidence 17:41:48.714389 -> 17:41:48.864027 EXIT=0
py -3.14 -c @'
import json,subprocess; from pathlib import Path; r=Path('E:/Impulcifer'); rows=json.loads(Path('C:/Users/32170336/AppData/Local/Temp/impulcifer-pa03-baseline-cupfe9wj/rows.json').read_text()); print('outputs',next(x['outputs'] for x in rows if x['side']=='python314')); print('plots',[(s,all({'plots/headphones.png','plots/results.png'} <= {p.replace(chr(92),'/') for p in x['outputs']} for x in rows if x['side']==s)) for s in ['python314','python314t']]); w='C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect'; subprocess.run(['git','-C',w,'status','--porcelain','--untracked-files=all','--ignored'],check=True); print('worktree top contents',[(p.name,p.is_dir()) for p in Path(w).iterdir()]); print('original report sha256',__import__('hashlib').sha256((r/'docs/rust/perf/release.md').read_bytes()).hexdigest()); subprocess.run(['git','check-ignore','target/pa06b','target/pa06b_run.py'],check=True)
'@
# audit-evidence-check 17:43:50.183645 -> 17:43:50.372255 EXIT=0
py -3.14 -c @'
from pathlib import Path; import subprocess,json,re; root=Path('E:/Impulcifer'); d=root/'target/pa06b'; print('HEAD',subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()); print('STATUS',subprocess.check_output(['git','status','--porcelain'],text=True)); print('DETACHED_TREE_EXISTS',Path('C:/Users/32170336/AppData/Local/Temp/impulcifer-bisect').exists()); print('EXITS',[(p.name,re.findall(r'END=.* EXIT=(-?[0-9]+)',p.read_text(encoding='utf-8'))[-1:]) for p in sorted(d.glob('*.log')) if p.name!='audit-evidence-check.log']); print('PARENT_CANDIDATE',subprocess.check_output(['git','diff','--name-only','e62c3cd^','e62c3cd'],text=True)); print('PA04_REPORT_IDENTICAL',subprocess.check_output(['git','diff','59d9ae3','5307b6b','--','docs/rust/perf/impulcifer-service.md','crates/impulcifer-service/benches/perf.rs','crates/impulcifer-service/tests/bench_support/service.rs'],text=True)==''); print('RUST_PLOT_STAGE_ROWS',sum('cli_plotting_results' in line for line in (d/'pipeline.log').read_text().splitlines())); print('LOG_BYTES',sum(p.stat().st_size for p in d.iterdir() if p.is_file()))
'@
```

후속 확인의 주요 원문 출력은 다음과 같습니다. STATUS가 비어 있었다는 것은 **문서 작성 전 측정 워커 종료 당시**의 상태입니다. 이 보고서 수정 뒤에도 clean이라고 주장하지 않습니다. EXITS 전체 목록은 각 로그의 마지막 값이 모두 0이었으며, 무효 bisect도 포함되므로 종료 코드와 유효성을 별도로 판단했습니다.

```text
MAIN CRLF 248 LF 248
original report sha256 a044cecac03facd2cac73dcc9d37560c5d8d50f64773e70d8dbcddd03458eebb
HEAD e62c3cd5d884ca522cb0b7d4a2a5bc68ebd9cec8
STATUS
DETACHED_TREE_EXISTS False
PARENT_CANDIDATE docs/rust/packets/PA06b-astra-perf-release-rerun.md
PA04_REPORT_IDENTICAL True
RUST_PLOT_STAGE_ROWS 12
LOG_BYTES 844648
```

원래 본문의 SHA-256은 위 로그에 기록한 **원래 파일 전체의 해시**입니다. 아래 보존 본문만 별도 해시로 다시 검사했다고 주장하지 않습니다. 편집은 원래 첫 제목 앞에 새 보고서를 삽입하고 그 앞에 First run 제목을 추가하는 방식으로 했으며, 기존 본문·원문 부록은 수정하지 않았습니다. 이 문서의 부록을 통해 측정 수치·명령·실패 처리·시험 집계는 clone에서도 확인할 수 있습니다. ignored WAV·각 표본의 장문 trace와 생성 시점의 임시 fixture 자체까지 영구 보존한 것은 아닙니다.

## First run (2026-09-08)

# PA06: 출시 전 성능 감사

2026-09-08. 대상 커밋은 `7bdba88091428d5ca63c28161ba3d20175fca553`이다. 기존 사용자 수정인 `docs/rust/packets/PA06-astra-perf-release.md`는 그대로 보존했다.

**판정: M5 미통과.** 플롯을 포함한 전체 파이프라인은 두 시나리오와 두 Python 인터프리터에서 시간 기준을 통과했다. 하지만 DSP 피크 검출의 최초 측정, 오디오 연산, 독립 동시 녹음의 무결성, 전체 테스트에 미해결 항목이 있다. 비율이 이전보다 20% 넘게 감소한 연산도 아래에 별도로 기록한다. 유리한 재측정값으로 최초 결과를 교체하지 않았다.

ASTRA 워커가 기존 하네스로 측정했고 부모 세션이 원문 표를 대조했다. 부모 세션의 전체 테스트 재실행에서도 실패가 재현됐다. 구현·벤치·오라클·골든·허용 오차·의존성·`features.toml`은 변경하지 않았다. 이 보고서와 기존 보고서 상단의 한 줄 링크만 추가했다. 커밋과 푸시는 하지 않았다.

## 1. 환경과 측정 조건

| 항목 | 측정 환경 |
|---|---|
| CPU | Intel Core i5-12600KF, 물리 10코어 / 논리 16코어 |
| OS | Windows 11 Education, 10.0.22621, x86-64 |
| Rust | rustc 1.97.0 (2d8144b78 2026-07-07), LLVM 22.1.6, MSVC |
| Cargo | 1.97.0, release/bench 프로필 |
| 일반 Python | CPython 3.14.5, `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe` |
| Free-threaded Python | CPython 3.14.7, GIL 비활성, `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe` |
| NumPy / SciPy | 두 인터프리터 모두 2.5.3 / 1.18.1 |
| soundfile / libsndfile | 0.14.0 / 1.2.2 |
| NumPy BLAS | OpenBLAS 0.3.34.106.0, ILP64, `NO_AFFINITY`, `MAX_THREADS=24` |
| SciPy BLAS | OpenBLAS 0.3.31.dev, LP64 |
| FFT | NumPy pocketfft / 설치된 SciPy duccfft |
| 스레드 환경 변수 | OMP_NUM_THREADS, MKL_NUM_THREADS, OPENBLAS_NUM_THREADS, BLIS_NUM_THREADS, NUMEXPR_NUM_THREADS, RAYON_NUM_THREADS 모두 미설정 |
| 추가 Rust 플래그 | RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS 미설정 |

**other processes were not queried; caller kept machine quiet.** 다른 프로세스를 조회하거나 종료하지 않았다. 모든 명령은 포그라운드에서 순차 실행했다. 메모리 측정은 기존 하네스가 실행한 자식과 그 후손에 한정했다. 기존 하네스의 psutil `memory_info().peak_wset`과 RSS 표본을 사용했다. 시스템 전체 프로세스 목록은 조회하지 않았다.

일부 기존 하네스는 SciPy FFT를 pocketfft라고 출력한다. 설치된 `_basic_backend.py`의 `_duccfft` 임포트와 달라 환경 설명을 위와 같이 정정했다. 하네스는 수정하지 않았다. 상세 `numpy.show_config()`와 SciPy 설정은 `E:/Impulcifer/target/pa06-environment-extra.log`, `pa06-environment-ft-extra.log`에 있다. 아래 원문 출력의 잘못된 백엔드 설명으로 실제 환경을 판단하면 안 된다.

### 반복과 비교 방법

- IO·DSP·service는 기존 입력 크기와 워밍업 3회, 측정 11회 규칙을 유지했다. IO 오라클은 같은 임시 fixture로 Rust도 실행하므로 각 Python 표에 대응하는 Rust 표를 사용했다. service는 기존 batch 크기와 연산당 시간을 그대로 사용했다.
- IO는 실제 sweep 1×295270, demo 2×878540, 합성 32×96000·30×96000·2×295000·8×480000 입력이다. DSP와 service의 크기는 부록 원문 표에 있다.
- 파이프라인은 기존 PA03 하네스대로 Rust 워밍업 1회와 측정 5회, 각 Python·실행 방식은 워밍업 1회와 측정 3회다. 플롯을 양쪽 모두 포함했다. default와 vbass를 각각 측정했다.
- 오디오는 기존 PA05 하네스의 워밍업과 연산별 반복을 유지했다. headphones 5회, seven-segment 3회, 첫 샘플 전달 10회, 열거·open/close는 20 calls/batch다. 독립 동시 녹음은 100~109번으로 10회 실행했다.
- `ratio = Python median / Rust median`. 1.0 미만은 미달이며 이전 비율보다 20% 넘게 감소해도 회귀 조사 대상이다. 표의 `일반 / FT`는 CPython 3.14.5 / 3.14.7t 순서다.
- 이전 IO 비율은 PA01 후반의 최신 3.14/FT 추가 측정을 사용했다. PA01 앞부분의 3.13 기준값과 섞지 않았다. 다른 보고서도 최신 결과를 사용했다. 오디오는 PA05의 sixth run과 비교했다.

## 2. 크레이트별 비교

### impulcifer-io

| 연산 | 이전 일반 / FT | 이번 일반 / FT | 판정 |
|---|---:|---:|---|
| write_pcm32_32tracks | 2.26 / 2.11 | 2.248870 / 2.002853 | 통과 |
| write_pcm32_30tracks | 2.63 / 2.47 | 2.520842 / 2.413090 | 통과 |
| write_pcm16_2tracks | 4.05 / 3.33 | 3.580682 / 3.748897 | 통과 |
| read_pcm32_32tracks | 1.70 / 1.29 | 1.279369 / 1.346057 | 일반 비율 20% 초과 감소 |
| read_bundled_sweep | 1.47 / 1.18 | 1.023646 / 1.224123 | 일반 비율 20% 초과 감소 |
| read_demo_recording | 1.26 / 1.49 | 1.372607 / 1.456057 | 통과 |
| read_float32_8tracks | 1.31 / 1.39 | 1.399126 / 1.406850 | 통과 |
| pcm32_round_trip | 4.79 / 4.49 | 4.640081 / 4.073999 | 통과 |
| sweep_file_name_parse | 1.29 / 1.28 | 1.340598 / 1.132295 | 통과, 비교 작업의 의미 차이 있음 |

파일명 비교는 Python의 실제 부분 regex 검색과 Rust의 전체 필드 검증이다. 동일한 파서의 속도 비교는 아니다. 읽기에서도 Python은 interleaved 배열의 transposed view를 반환하고 Rust는 독립 track 벡터를 반환한다. 원래 감사의 비교 방식을 바꾸지 않았다.

### impulcifer-dsp

| 연산 | 이전 | 이번 | 판정 |
|---|---:|---:|---|
| convolve_full_ir_fir | 1.800 | 1.877551 | 통과 |
| convolve_same_estimate | 1.840 | 1.896370 | 통과 |
| correlate_full_30ms | 9.867 | 9.859143 | 통과 |
| rfft_irfft_96000 | 2.021 | 1.563819 | 20% 초과 감소 |
| magnitude_response_96000 | 3.508 | 2.921079 | 통과 |
| butter8_sosfilt_96000 | 1.415 | 1.471577 | 통과 |
| firwin2_19200 | 2.187 | 2.252274 | 통과 |
| minimum_phase_19200 | 1.269 | 1.232030 | 통과 |
| savgol_heavy_light_783 | 17.858 | 18.174107 | 통과 |
| find_peaks_96000 | 1.346 | **0.922716** | **미달, 20% 초과 감소** |
| first_peak_index_96000 | 1.158 | 1.579252 | 통과 |
| spline_k1_783_to_4800 | 1.378 | 1.355769 | 통과 |
| spline_k2_783 | 2.685 | 2.648208 | 통과 |
| spline_k3_783 | 2.975 | 2.504926 | 통과 |
| linregress_1000 | 135.278 | 142.388889 | 통과 |
| nnresample_design_147_160 | 2.026 | 1.851836 | 통과 |
| resample_poly_96000_48k_44k1 | 1.693 | 1.713252 | 통과 |
| nnresample_96000_48k_96k | 2.427 | 2.437212 | 통과 |
| spectrogram_295000_4800 | 2.951 | 3.231426 | 통과 |
| windows_32001 | 1.988 | 2.068395 | 통과 |
| expit_783 | 1.138 | 1.172414 | 통과 |
| next_fast_len_1e6 | 27.700 | 30.444444 | 통과 |

DSP primitive 오라클은 일반 Python으로 실행했다. Python의 스레드 병렬 처리가 있는 전체 파이프라인과 service는 FT도 측정했다.

### impulcifer-service

| 연산 | 이전 일반 / FT | 이번 일반 / FT | 판정 |
|---|---:|---:|---|
| bootstrap | 4.149 / 4.315 | 4.138001 / 4.324591 | 통과 |
| get_ui_settings | 2.476 / 2.662 | 2.735937 / 2.938621 | 통과 |
| set_language_round_trip | 1.533 / 1.555 | 1.637455 / 1.727286 | 통과 |
| resolve_recording_paths | 2.274 / 5.124 | 2.215480 / 2.345780 | FT 비율 20% 초과 감소 |
| start_brir_to_first_event | 1.515 / 1.642 | 1.518266 / 1.612946 | 통과 |
| poll_job_drain | 4.459 / 4.869 | 4.598223 / 5.043069 | 통과 |
| job_event_emit | 15.595 / 16.578 | 14.535267 / 15.499733 | 통과 |
| detect_sweep | 2.647 / 2.919 | 2.393420 / 2.613097 | 통과 |
| catalog_translate | 2.794 / 3.118 | 2.694009 / 3.149894 | 통과 |

### impulcifer-audio-io

PA05 최신 표와 동일하게 명시적 Python `InputStream`/`OutputStream`을 비교 대상으로 삼았다. **이는 실제 `core.recorder.play_and_record`와 다르다.** 실제 함수도 일반·FT로 별도 측정했으며 부록에 함께 보존한다.

| 연산 | 이전 일반 / FT | 이번 일반 / FT | 판정 |
|---|---:|---:|---|
| 캐시 열거 | 75.828897 / 78.433460 | 75.650558 / 74.828996 | 통과 |
| duplex open/close | 1.283181 / 1.275104 | 1.273951 / 1.292799 | 통과 |
| headphones wall | 0.997324 / 0.997551 | **0.996738 / 0.996549** | 미달 |
| headphones overhead | 0.699578 / 0.725113 | **0.660448 / 0.640820** | 미달 |
| seven-segment wall | 0.999716 / 0.999599 | **0.999532 / 0.999568** | PA06의 1.0 기준 미달 |
| seven-segment overhead | 0.831658 / 0.761855 | **0.719489 / 0.740656** | 미달 |
| 첫 샘플 전달 | 0.698722 / 0.733974 | **0.869886 / 0.890895** | 미달 |
| CPU | 2.000000 / 2.500000 | 2.333333 / 2.000000 | 통과, 카운터 정밀도 제한 |

- headphones wall은 Rust 6211.1332 ms, Python 6190.8705 / 6189.6992 ms다. Rust가 0.3273% / 0.3463% 느려 PA05의 0.2% 허용 기준으로도 실패한다.
- seven-segment wall은 PA05의 0.2% 허용 기준에는 맞지만 PA06의 엄격한 1.0 기준에는 못 미친다. 기존 허용치를 PA06 통과에 적용하지 않았다.
- 첫 샘플 전달은 입력 세션 시작 호출부터 애플리케이션이 처음 데이터를 받는 시점까지다. 출력 신호의 실제 도착 지연과 같지 않다.
- CPU는 ACK로 측정 시작·종료를 맞춘 Rust 자식의 user+kernel 시간이다. Windows 카운터는 15.625 ms 단위여서 작은 차이를 정밀한 개선으로 해석하지 않았다.
- CABLE-A, exclusive 거절 후 shared auto-convert, 48 kHz stereo를 사용했다. 기존 float32 전송은 그대로이며 새 수치 구현을 추가하지 않았다. Rust 버퍼는 1920프레임(4주기, 40 ms), 캡처 패킷은 480프레임이다.

### impulcifer-sys-win

| 연산 | 이전 일반 / FT | 이번 일반 / FT | 판정 |
|---|---:|---:|---|
| fresh COM enumerate_backend 대 Python enumerate_devices | 0.007290 / 0.007540 | 0.007555 / 0.007473 | 1.0 미달, 동등 작업이 아닌 비교 |

Rust fresh 열거 20회 중앙값은 269.3437 ms다. Python PortAudio 캐시 조회와 fresh COM 열거는 작업이 달라 이를 같은 동작의 성능으로 판단하지 않는다. 실제 상위 audio-io의 캐시 열거는 별도로 측정했고 통과했다.

## 3. 전체 파이프라인과 메모리

플롯을 포함한 기존 release 실행 파일과 Python CLI를 비교했다. 별도 Python `main()` / Rust service의 in-process 시간도 측정했다. 프로세스 wall time에는 시작·종료가 포함된다.

| 시나리오·비교 대상 | 이전 프로세스 비율 | 이번 프로세스 비율 | 이번 main/service 비율 | 판정 |
|---|---:|---:|---:|---|
| default / 일반 | 11.917014 | 11.307964 | 8.789373 | 통과 |
| default / FT | 5.469190 | 5.024980 | 3.609704 | 통과 |
| vbass / 일반 | 11.973251 | 11.445840 | 8.805108 | 통과 |
| vbass / FT | 5.155249 | 5.112934 | 3.601658 | 통과 |

### CLI와 프로세스 측정

시간은 ms, 메모리는 MiB다. 메모리는 반복 중 최댓값이다. 과거 peak 합은 동시에 사용한 메모리가 아니다. 동시 트리 RSS는 살아 있는 측정 프로세스들의 RSS를 같은 표본 시점에 더한 값이다. 공유 페이지를 프로세스마다 중복 합산할 수 있으므로 물리 RAM의 순사용량과 같지 않다.

| 시나리오 | 실행 | 프로세스 중앙값 | 작업 프로세스 peak | 과거 peak 합 | 동시 트리 RSS |
|---|---|---:|---:|---:|---:|
| default | Rust | 1135.9600 | 246.215 | 246.215 | 224.297 |
| default | 일반 Python | 12845.3949 | 494.844 | 1142.430 | 945.551 |
| default | FT Python | 5708.1763 | 521.070 | 525.355 | 525.301 |
| vbass | Rust | 1121.0222 | 234.023 | 234.023 | 234.023 |
| vbass | 일반 Python | 12831.0407 | 494.855 | 1142.523 | 914.871 |
| vbass | FT Python | 5731.7120 | 521.031 | 525.344 | 525.289 |

FT 런처 자체의 약 4.3 MiB를 작업 프로세스의 메모리로 오인하지 않았다. FT의 동시 트리 RSS는 일반 Python보다 default 44.45%, vbass 42.58% 적었다. 인터프리터와 할당기 차이도 있으므로 이 차이를 전부 스레드 방식의 효과라고 단정하지 않는다. 메모리를 줄이는 최적화는 하지 않았다.

Rust 진행 이벤트 간격의 중앙값은 default/vbass 순서로 estimator 117/120, room 356/352, headphone 156/153, measurements 229/227, crop 109/105, EQ 18/16, normalize 26/26, result plot 15/14, final write 23/23 ms다. vbass 단계는 12 ms다. 각 함수를 따로 측정한 CPU 시간은 아니다.

전체 표와 실제 자식 명령은 `target/pa06-pipeline.log`, 추가 벤치는 `target/pa06-pipeline-rust.log`에 있다. 기존 하네스가 만든 원시 자료는 `C:/Users/32170336/AppData/Local/Temp/impulcifer-pa03-baseline-h7asxl_t/rows.json`과 `environment.json`이다.

## 4. 회귀 조사, 프로파일과 미해결 사항

원인 커밋을 특정하지 못했다. 측정상 기준 미달과 실제 코드 변경의 인과를 구분했다. 원인이 확인되지 않은 소스를 추측으로 최적화하지 않았다.

| 항목 | 조사와 재측정 | 남은 문제 |
|---|---|---|
| IO 32채널 읽기 | `wav.rs`는 PA01 `39c00c3` 이후 변경 없음. Rust는 이전보다 1.82% 증가, Python은 23.23% 감소. 별도 비율 1.466430 | 최초 20% 초과 감소의 원인 미확정 |
| IO mono 읽기 | 최초 Rust 0.7274 ms로 이전보다 45.68% 증가. 별도 Rust 0.5130 ms, 비율 1.348148 | 실행 간 변동이 커 최초 회귀 판정을 유지 |
| DSP FFT | PA02 `e98a954` 이후 관련 소스·벤치 변경 없음. rustfft/realfft/num-complex 버전 동일. 최초 Rust 시간은 2.20% 감소, Python은 24.32% 감소. 별도 Rust/Python 1.0176/2.0023 ms | 비교 대상의 속도 변화와 환경 영향을 분리하지 못함 |
| DSP peaks | 최초 Rust/Python 1.0028/0.9253 ms, 비율 0.922716. 별도 1.0692/1.3561 ms로 통과. 관련 소스·벤치 변경 없음 | 최초 미달 유지, 반복 측정의 안정성 확인 필요 |
| Service 경로 처리 | PA04 `59d9ae3` 이후 service 소스 변경 없음. Rust 1.54% 증가, FT Python 53.51% 감소. 이전 보고서에도 Python 변동 기록 있음 | 최초 20% 초과 감소 유지 |
| 오디오 | PA05 이후 구현 변경 없음. 이전에도 wall·overhead·latency 미달 | 신규 코드 회귀로 특정하지 못했고 기존 미달도 해결하지 않음 |

`git log`로 관련 이력을 검토했지만 원인 커밋을 찾지 못했다. **요청한 `/tmp` detached worktree의 원인 커밋·부모 비교는 수행하지 않았다.** 변경된 함수가 없다는 사실만으로 환경·컴파일·통합의 영향을 배제할 수 없다. 따라서 원인 규명까지 완료했다고 주장하지 않는다.

### 1.5 미만 연산의 코드 기반 프로파일

- IO 읽기에는 디코딩·전치·출력 할당이 있다. 32채널 읽기는 호출마다 scoped thread 4개를 만드는 비용도 있다. mono 입력은 작업이 작아 할당과 파일 I/O 변동이 비율에 영향을 준다. demo·float32 읽기와 파일명 파싱도 1.5 미만이다. 파일명 파서는 부분 regex보다 많은 필드를 검증하고 문자열을 소유한다.
- SOS 필터는 feedback 때문에 샘플 사이 계산 의존성이 있다. minimum-phase는 골든 패리티용 전체 스펙트럼 로그 처리와 Nyquist 계산이 남아 있다. 선형 spline은 질의별 이진 탐색과 보간, expit은 scalar exp 비용이 있다.
- 피크 검출은 plateau 분기 순회와 결과 Vec 확장, 양·음 두 번의 탐색을 수행한다. 이번에 수치 결과나 공개 시그니처를 바꾸지 않았다.
- 오디오 성공 세션 trace 320개의 `initialize_client` 중앙값은 5632 μs, enumerator 약 875 μs, client release 679.5 μs다. 두 세션을 동시에 여는 구현이므로 이 단계별 중앙값을 더해 한 쌍의 wall time으로 해석하면 안 된다.
- 오디오에는 40 ms buffering, 이벤트 전달, 시작·drain·종료 비용이 남는다. sys-win의 fresh 열거와 Python 캐시는 동일한 작업이 아니다. 샘플링 프로파일러를 설치하거나 실행하지 않았으며 위 내용은 코드 검토·기존 trace·제한된 크기의 재측정에 근거한다.

프로파일 로그는 `target/pa06-io-repeat.log`, `pa06-dsp-peaks-profile-rust.log`, `pa06-dsp-peaks-profile-python.log`, `pa06-dsp-fft-profile-rust.log`, `pa06-dsp-fft-profile-python.log`, `pa06-audio-open-profile.log`다. 구현 변경은 없으므로 최적화 전후의 새 golden max-error 값은 산출하지 않았다. 기존 골든은 그대로 실행했다.

### 독립 동시 녹음 10회

기존 `sixth_paired.py`를 period 4, run 100~109로 실행했다. 캡처 명령은 10회 모두 종료했지만 분석 결과를 모두 확정하지는 못했다.

- 8회는 쌍별 분석을 확정했다. 그중 100·102·106번에서 두 녹음에 공통으로 각각 480프레임 손실을 확인했다. 합계 1440프레임이다.
- 107번은 분석기 오류가 있었고 전체 소스 대비 RMS는 0.6971 / 0.6122다. 손실 원인은 미확정이다.
- 109번 Rust 녹음은 한 오프셋으로 소스 전체와 최대 오차 3.8142e-6 안에서 일치한다. 하지만 독립 관찰자에만 768프레임 삭제 후보가 있어 동시 관찰 증거가 불완전하다.
- 10회 모두 보고된 render underrun, 초기 패킷 이후 discontinuity, SILENT, packet index gap은 0이다. **카운터가 0이어도 파형 무결성을 보장하지 않는다.**
- 하네스의 `accepted=true`는 공통 외부 손실을 허용하는 분류 결과다. 무손실 판정이 아니며 **10/10 무결성 통과를 주장할 수 없다.**

원시 WAV·trace·JSON은 기존 무시 디렉터리 `crates/impulcifer-audio-io/tests/bench_support/`의 `sixth-p4-100`~`sixth-p4-109` 접두사 파일이다. 이 대용량 자료는 추적 파일에 추가하지 않았다.

## 5. 검증 결과와 등록부 제안

| 명령·검증 | 결과 |
|---|---|
| release workspace build | 0, 최초 한 번 실행 |
| IO·DSP·service·audio-io·sys-win perf bench | 모두 0 |
| service perf의 pipeline 모드 | 0 |
| 지정된 IO·DSP·service·pipeline Python 오라클 | 모두 0 |
| IO·service FT 오라클 | 모두 0 |
| 명시적 오디오 일반·FT 오라클 | 모두 0 |
| 실제 recorder 일반·FT 오라클 | 개별 재실행 모두 0 |
| Rust 오디오 CPU observer | 0 |
| 독립 캡처 100~109 | 명령 모두 0, 무결성은 위와 같이 미통과 |
| `cargo test --workspace` | 101, app 테스트에서 중단 |
| 워커 `cargo test --workspace --no-fail-fast` | 101, 372 passed / 2 failed / 10 ignored |
| 부모 `cargo test --workspace --no-fail-fast` | 101, 373 passed / 1 failed / 10 ignored |
| `cargo test -p impulcifer-policy` | 워커·부모 모두 0, 6 passed |
| CABLE-A service 녹음 실기 테스트 | 0, 1 passed |
| updater 실패 테스트 단독 재실행 | 0, 1 passed |
| `cargo fmt --all -- --check` | 워커·부모 모두 0 |
| workspace clippy `-D warnings` | 워커·부모 모두 0 |
| 부모 `git diff --check` | 0, LF/CRLF 변환 안내만 출력 |

첫 오디오 묶음 호출은 Rust·sys-win 벤치가 끝난 뒤 Python 실행 중 600초 제한으로 143을 반환했다. 그 Python 결과는 완료로 세지 않았고 별도 포그라운드 호출로 재실행해 종료 코드 0을 확인했다. 종료 코드 0은 벤치가 끝났다는 뜻이지 성능·무결성 기준 통과라는 뜻이 아니다.

부모가 직접 재현한 실패 원문은 다음과 같다.

```text
---- updater_plugin_registered_with_public_key stdout ----
thread 'updater_plugin_registered_with_public_key' (251872) panicked at apps\impulcifer-app\tests\smoke.rs:90:5:
assertion failed: source.contains("#[cfg(not(windows))]\n    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build())")

test result: FAILED. 4 passed; 1 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.06s
error: test failed, to rerun pass `-p impulcifer-app --test smoke`
```

`apps/impulcifer-app/src/main.rs`는 CRLF이며 테스트가 LF를 포함한 문자열을 찾는다. 워커는 CRLF를 LF로 정규화한 바이트에서 기대 문자열이 존재함을 확인했다. 부모 전체 재실행에서도 같은 실패가 재현됐다. 테스트는 이번 허용 범위 밖이므로 수정하지 않았다.

워커 전체 실행의 두 번째 실패는 `crates/impulcifer-service/tests/updater.rs:401`의 `legacy_executor_downloads_verifies_and_opens`였다. 기대한 상태 대신 `failed`를 받았다. 단독 재실행과 부모 전체 재실행은 통과했지만 최초 실패 원인은 미해결이다.

Demo default/vbass 골든, `plots_do_not_change_wavs`, 각 크레이트 smoke는 통과했다. 등록부에는 아래 실제 테스트를 제안하되, **성능과 전체 검증이 미통과이므로 `perf.release`를 implemented로 바꿀 근거는 없다.**

- `impulcifer-service::bench_smoke_pipeline_demo`
- `impulcifer-io::bench_smoke_impulcifer_io`
- `impulcifer-dsp::bench_smoke_impulcifer_dsp`
- `impulcifer-service::bench_smoke_impulcifer_service`
- `impulcifer-audio-io::bench_smoke_impulcifer_audio_io`
- `impulcifer-sys-win::bench_smoke_impulcifer_sys_win`

## 6. 실행 명령과 원문 자료

작업 디렉터리는 `E:/Impulcifer`다. 워커의 Cargo 호출에는 같은 workspace를 지정하는 `--manifest-path E:/Impulcifer/Cargo.toml`이 추가됐다. 다음은 요청한 명령이며 순서는 IO, DSP, service, pipeline, audio-io와 sys-win이다. 각 그룹에 기존 하네스의 추가 측정도 수행했다.

```powershell
cargo build --workspace --release
cargo bench -p impulcifer-io --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
cargo bench -p impulcifer-dsp --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_dsp.py
cargo bench -p impulcifer-service --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_pipeline.py
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
cargo test --workspace
cargo test -p impulcifer-policy
```

기존 하네스의 추가 측정에 대응하는 명령도 기록한다.

```powershell
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_io.py
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
cargo bench -p impulcifer-service --bench perf -- --pipeline
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py --period 4 --run 100
# 같은 명령의 --run 인자에 101부터 109까지 순서대로 사용했다.
cargo test --workspace --no-fail-fast
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
```

Rust CPU observer는 기존 오디오 오라클의 `--observe-rust`로 실행했다. 실제 빌드 산출물 이름을 포함한 실행 인자와 pipeline 하위 프로세스의 전체 인자는 원문 로그를 참고한다. 이 문서의 재현용 명령과 셸 호출 전체가 같은 것은 아니다. 프로파일용 임시 명령 전체는 이 문서에 보존하지 못했다.

주요 로그는 `E:/Impulcifer/target/` 아래에 있다. 무시된 로컬 산출물이므로 저장소를 clone한 다른 머신에는 없으며 영구 CI 아티팩트도 아니다. 측정 표 원문은 부록에도 보존한다.

| 그룹 | 파일 |
|---|---|
| IO | pa06-io-rust.log, pa06-io-python.log, pa06-io-python-ft.log |
| DSP | pa06-dsp-rust.log, pa06-dsp-python.log |
| Service | pa06-service-rust.log, pa06-service-python.log, pa06-service-python-ft.log |
| Pipeline | pa06-pipeline-rust.log, pa06-pipeline.log |
| Audio | pa06-audio-rust-standalone.log, pa06-audio-rust-observed.log, pa06-audio-python-explicit.log, pa06-audio-python-ft-explicit.log |
| 실제 recorder | pa06-audio-python-production-retry.log, pa06-audio-python-ft-production.log |
| Sys-win | pa06-sys-rust.log |
| 검증 | pa06-workspace-tests.log, pa06-workspace-all-tests.log, pa06-parent-workspace-tests.log, pa06-policy-tests.log, pa06-fmt.log, pa06-clippy.log |

## 7. 남은 작업

1. 최초 피크 검출 미달과 비율 감소 연산의 원인을 안정적으로 재현하고, 원인 커밋을 특정할 수 있으면 detached worktree 비교를 수행해야 한다. 이번에는 이 비교를 수행하지 않았다.
2. 오디오 wall·overhead·latency 미달과 동시 녹음의 공통 손실·미확정 2회를 해결해야 한다.
3. CRLF 의존 테스트를 별도 허용 범위에서 수정하고, updater의 간헐적 실패 원인을 확인한 뒤 전체 테스트가 통과해야 한다.
4. 프로파일용 명령 전체와 최초 release build 출력은 이 문서에 원문으로 붙이지 못했다. 벤치 표와 테스트 실패·요약은 보존했지만 모든 명령 출력의 완전한 전재는 아니다.

성능 감사의 실행 결과는 기록했으나 **출시 성능 승인과 M5 완료는 보류한다.**

## 부록 A. 측정 표 원문

아래에는 로그에서 표 행을 그대로 옮긴다. 원래 표 헤더가 로그 메시지 사이에 흩어진 파일도 있으며 수치를 반올림하거나 재측정값으로 대체하지 않는다.

### pa06-io-rust.log

```text
| op | size | rust median ms | rust min ms |
| write_pcm32_32tracks | 32x96000 | 10.767600 | 10.330500 |
| write_pcm32_30tracks | 30x96000 | 8.816500 | 8.663600 |
| write_pcm16_2tracks | 2x295000 | 1.616900 | 1.432600 |
| read_pcm32_32tracks | 32x96000 | 5.255500 | 5.018900 |
| read_bundled_sweep | 1x295270 | 0.512700 | 0.463900 |
| read_demo_recording | 2x878540 | 2.938000 | 2.648900 |
| read_float32_8tracks | 8x480000 | 6.768100 | 6.540400 |
| pcm32_round_trip | 30x96000 | 8.765500 | 8.607000 |
| sweep_file_name_parse | 10000 | 3.102300 | 3.051600 |
```

### pa06-io-python.log

```text
| op | size | rust median ms | rust min ms |
| write_pcm32_32tracks | 32x96000 | 10.689100 | 10.235100 |
| write_pcm32_30tracks | 30x96000 | 8.816300 | 8.639400 |
| write_pcm16_2tracks | 2x295000 | 1.556100 | 1.446800 |
| read_pcm32_32tracks | 32x96000 | 5.884700 | 5.045000 |
| read_bundled_sweep | 1x295270 | 0.727400 | 0.550600 |
| read_demo_recording | 2x878540 | 2.830600 | 2.510800 |
| read_float32_8tracks | 8x480000 | 6.708400 | 6.425100 |
| pcm32_round_trip | 30x96000 | 8.894500 | 8.620700 |
| sweep_file_name_parse | 10000 | 3.092800 | 3.046500 |
| op | size | python median ms | python min ms |
| write_pcm32_32tracks | 32x96000 | 24.038400 | 23.292700 |
| write_pcm32_30tracks | 30x96000 | 22.224500 | 21.532700 |
| write_pcm16_2tracks | 2x295000 | 5.571900 | 5.411200 |
| read_pcm32_32tracks | 32x96000 | 7.528700 | 7.325900 |
| read_bundled_sweep | 1x295270 | 0.744600 | 0.636800 |
| read_demo_recording | 2x878540 | 3.885300 | 3.736000 |
| read_float32_8tracks | 8x480000 | 9.385900 | 9.149000 |
| pcm32_round_trip | 30x96000 | 41.271200 | 40.392900 |
| sweep_file_name_parse | 10000 | 4.146200 | 3.993600 |
| op | python/rust median |
| write_pcm32_32tracks | 2.248870 |
| write_pcm32_30tracks | 2.520842 |
| write_pcm16_2tracks | 3.580682 |
| read_pcm32_32tracks | 1.279369 |
| read_bundled_sweep | 1.023646 |
| read_demo_recording | 1.372607 |
| read_float32_8tracks | 1.399126 |
| pcm32_round_trip | 4.640081 |
| sweep_file_name_parse | 1.340598 |
```

### pa06-io-python-ft.log

```text
| op | size | rust median ms | rust min ms |
| write_pcm32_32tracks | 32x96000 | 11.775300 | 11.442800 |
| write_pcm32_30tracks | 30x96000 | 9.221000 | 8.552900 |
| write_pcm16_2tracks | 2x295000 | 1.518900 | 1.441000 |
| read_pcm32_32tracks | 32x96000 | 5.697900 | 5.093600 |
| read_bundled_sweep | 1x295270 | 0.567100 | 0.441700 |
| read_demo_recording | 2x878540 | 2.820700 | 2.648500 |
| read_float32_8tracks | 8x480000 | 6.723600 | 6.545300 |
| pcm32_round_trip | 30x96000 | 9.044700 | 8.620700 |
| sweep_file_name_parse | 10000 | 3.108200 | 3.072600 |
| op | size | python median ms | python min ms |
| write_pcm32_32tracks | 32x96000 | 23.584200 | 23.086000 |
| write_pcm32_30tracks | 30x96000 | 22.251100 | 21.874800 |
| write_pcm16_2tracks | 2x295000 | 5.694200 | 5.434400 |
| read_pcm32_32tracks | 32x96000 | 7.669700 | 7.261900 |
| read_bundled_sweep | 1x295270 | 0.694200 | 0.643600 |
| read_demo_recording | 2x878540 | 4.107100 | 3.805100 |
| read_float32_8tracks | 8x480000 | 9.459100 | 9.126500 |
| pcm32_round_trip | 30x96000 | 36.848100 | 35.541500 |
| sweep_file_name_parse | 10000 | 3.519400 | 3.464400 |
| op | python/rust median |
| write_pcm32_32tracks | 2.002853 |
| write_pcm32_30tracks | 2.413090 |
| write_pcm16_2tracks | 3.748897 |
| read_pcm32_32tracks | 1.346057 |
| read_bundled_sweep | 1.224123 |
| read_demo_recording | 1.456057 |
| read_float32_8tracks | 1.406850 |
| pcm32_round_trip | 4.073999 |
| sweep_file_name_parse | 1.132295 |
```

### pa06-dsp-rust.log

```text
| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| convolve_full_ir_fir | 96000 x 9600 | 2.097200 | 1.932200 |
| convolve_same_estimate | 391000 x 295000 | 14.048000 | 13.514400 |
| correlate_full_30ms | 8 x 1440 x 1440 | 0.121400 | 0.121100 |
| rfft_irfft_96000 | 96000 | 1.004400 | 0.938600 |
| magnitude_response_96000 | 96000 | 0.392800 | 0.389500 |
| butter8_sosfilt_96000 | 96000 | 0.654400 | 0.419800 |
| firwin2_19200 | 19200 taps / 9600 mesh | 1.154300 | 1.100800 |
| minimum_phase_19200 | 19200 | 0.872300 | 0.836400 |
| savgol_heavy_light_783 | 783 | 0.067200 | 0.066300 |
| find_peaks_96000 | 96000 | 1.002800 | 0.968200 |
| first_peak_index_96000 | 96000 | 1.365900 | 1.291800 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.072800 | 0.071200 |
| spline_k2_783 | 783 | 0.030700 | 0.030600 |
| spline_k3_783 | 783 | 0.040600 | 0.040500 |
| linregress_1000 | 1000 | 0.001800 | 0.001800 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 5.020800 | 4.552100 |
| resample_poly_96000_48k_44k1 | 96000 | 4.711100 | 4.649100 |
| nnresample_96000_48k_96k | 96000 | 626.534500 | 620.331700 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 4.015100 | 3.734700 |
| windows_32001 | 32001 / 19200 / 19200 | 0.617000 | 0.612000 |
| expit_783 | 783 | 0.002900 | 0.002600 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000009 | 0.000009 |
```

### pa06-dsp-python.log

```text
| op | size | python median ms | python min ms |
|---|---|---:|---:|
| convolve_full_ir_fir | 96000 x 9600 | 3.937600 | 3.495400 |
| convolve_same_estimate | 391000 x 295000 | 26.640200 | 25.614600 |
| correlate_full_30ms | 8 x 1440 x 1440 | 1.196900 | 1.178000 |
| rfft_irfft_96000 | 96000 | 1.570700 | 1.478200 |
| magnitude_response_96000 | 96000 | 1.147400 | 1.048500 |
| butter8_sosfilt_96000 | 96000 | 0.963000 | 0.892300 |
| firwin2_19200 | 19200 taps / 9600 mesh | 2.599800 | 2.415300 |
| minimum_phase_19200 | 19200 | 1.074700 | 1.044800 |
| savgol_heavy_light_783 | 783 | 1.221300 | 1.172100 |
| find_peaks_96000 | 96000 | 0.925300 | 0.916400 |
| first_peak_index_96000 | 96000 | 2.157100 | 1.815100 |
| spline_k1_783_to_4800 | 783 to 4800 | 0.098700 | 0.098000 |
| spline_k2_783 | 783 | 0.081300 | 0.081000 |
| spline_k3_783 | 783 | 0.101700 | 0.100600 |
| linregress_1000 | 1000 | 0.256300 | 0.234900 |
| nnresample_design_147_160 | 32001 / FFT 524288 | 9.297700 | 8.708400 |
| resample_poly_96000_48k_44k1 | 96000 | 8.071300 | 7.890300 |
| nnresample_96000_48k_96k | 96000 | 1526.997700 | 1518.476700 |
| spectrogram_295000_4800 | 295000 / 4800 / overlap 3349 | 12.974500 | 12.323800 |
| windows_32001 | 32001 / 19200 / 19200 | 1.276200 | 1.215700 |
| expit_783 | 783 | 0.003400 | 0.003400 |
| next_fast_len_1e6 | 1000000 (real + legacy) | 0.000274 | 0.000268 |
```

### pa06-service-rust.log

```text
| op | size | rust median ms | rust min ms |
| bootstrap | 200/batch; per unit | 0.254766500 | 0.248967500 |
| get_ui_settings | 200/batch; per unit | 0.150710000 | 0.147283500 |
| set_language_round_trip | 100/batch; per unit | 1.959715000 | 1.906111000 |
| resolve_recording_paths | 200/batch; per unit | 0.001285500 | 0.001225000 |
| start_brir_to_first_event | 5/batch; per unit | 0.331220000 | 0.306860000 |
| poll_job_drain | 5/batch; per unit | 1.024400000 | 0.841040000 |
| job_event_emit | 5/batch; per unit | 2.990920000 | 2.735800000 |
| detect_sweep | 20/batch; per unit | 0.224785000 | 0.217095000 |
| catalog_translate | 100000/batch; per unit | 0.000159753 | 0.000157192 |
```

### pa06-service-python.log

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.054224000 | 1.038691500 |
| get_ui_settings | 200/batch; per unit | 0.412333000 | 0.406429000 |
| set_language_round_trip | 100/batch; per unit | 3.208946000 | 3.164395000 |
| resolve_recording_paths | 200/batch; per unit | 0.002848000 | 0.002762500 |
| start_brir_to_first_event | 5/batch; per unit | 0.502880005 | 0.440099998 |
| poll_job_drain | 5/batch; per unit | 4.710419988 | 4.560499999 |
| job_event_emit | 5/batch; per unit | 43.473819998 | 42.869979993 |
| detect_sweep | 20/batch; per unit | 0.538004999 | 0.520520000 |
| catalog_translate | 100000/batch; per unit | 0.000430376 | 0.000428727 |
```

### pa06-service-python-ft.log

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.101761000 | 1.087274000 |
| get_ui_settings | 200/batch; per unit | 0.442879500 | 0.420324000 |
| set_language_round_trip | 100/batch; per unit | 3.384988000 | 3.236628000 |
| resolve_recording_paths | 200/batch; per unit | 0.003015500 | 0.002979000 |
| start_brir_to_first_event | 5/batch; per unit | 0.534240005 | 0.504939997 |
| poll_job_drain | 5/batch; per unit | 5.166120012 | 5.056680000 |
| job_event_emit | 5/batch; per unit | 46.358459990 | 45.691279991 |
| detect_sweep | 20/batch; per unit | 0.587385001 | 0.542859999 |
| catalog_translate | 100000/batch; per unit | 0.000503205 | 0.000487853 |
```

### pa06-pipeline-rust.log

```text
| scenario | rust median ms | rust min ms |
| default | 1030.955500 | 1020.029200 |
| vbass | 1054.058000 | 1032.601800 |
```

### pa06-pipeline.log

```text
| scenario | side | mode | process median ms | process min ms | in-process median ms | root peak MiB | largest workload process peak MiB | sum historical peaks MiB | concurrent tree RSS MiB |
| default | rust | service | 1135.960000 | 1115.647600 | 1052.865600 | 246.215 | 246.215 | 246.215 | 224.297 |
| default | python314 | cli | 12845.394900 | 12827.495900 | 0.000000 | 494.844 | 494.844 | 1142.430 | 945.551 |
| default | python314 | inprocess | 11005.964700 | 10989.267400 | 9254.028000 | 494.898 | 494.898 | 995.500 | 796.094 |
| default | python314t | cli | 5708.176300 | 5680.420000 | 0.000000 | 4.309 | 521.070 | 525.355 | 525.301 |
| default | python314t | inprocess | 5731.890700 | 5690.103700 | 3800.532700 | 4.312 | 522.074 | 526.383 | 526.328 |
| vbass | rust | service | 1121.022200 | 1118.377900 | 1048.391900 | 234.023 | 234.023 | 234.023 | 234.023 |
| vbass | python314 | cli | 12831.040700 | 12739.208900 | 0.000000 | 494.855 | 494.855 | 1142.523 | 914.871 |
| vbass | python314 | inprocess | 10964.271400 | 10939.268100 | 9231.203700 | 495.266 | 495.266 | 995.406 | 763.512 |
| vbass | python314t | cli | 5731.712000 | 5708.932900 | 0.000000 | 4.312 | 521.031 | 525.344 | 525.289 |
| vbass | python314t | inprocess | 5726.024600 | 5658.674100 | 3775.948900 | 4.312 | 521.215 | 525.527 | 525.473 |
| scenario | comparator | Python CLI / Rust process | Python main / Rust service |
| default | python314 | 11.307964 | 8.789373 |
| default | python314t | 5.024980 | 3.609704 |
| vbass | python314 | 11.445840 | 8.805108 |
| vbass | python314t | 5.112934 | 3.601658 |
```

### pa06-audio-rust-standalone.log

```text
| op | size | rust median ms | rust min ms |
| enumerate_backend | 20 calls/batch | 0.026100 | 0.025900 |
| open_close_session | 20 calls/batch | 179.636300 | 175.307800 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6208.279200 | 6201.356600 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 56.820867 | 49.898267 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43129.730900 | 43127.586900 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 69.522567 | 67.378567 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.752200 | 20.552600 |
```

### pa06-audio-rust-observed.log

```text
| op | size | rust median ms | rust min ms |
| enumerate_backend | 20 calls/batch | 0.026900 | 0.026600 |
| open_close_session | 20 calls/batch | 179.024300 | 176.007600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6211.133200 | 6206.625200 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 59.674867 | 55.166867 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43132.099600 | 43131.564400 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 71.891267 | 71.356067 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 25.698650 | 19.721300 |
| capture_loop_cpu | process user+kernel; 5 runs | 46.875000 | 31.250000 |
```

### pa06-sys-rust.log

```text
| op | size | rust median ms | rust min ms |
| enumerate_backend | 20 calls/batch | 269.343700 | 267.987200 |
```

### pa06-audio-python-explicit.log

```text
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.035000 | 1.960700 |
| open_close_session | 20 calls/batch | 228.068100 | 224.959600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6190.870500 | 6188.772300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 39.412167 | 37.313967 |
| capture_loop_cpu | process user+kernel; 5 runs | 109.375000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43111.933300 | 43111.853900 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 51.724967 | 51.645567 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.354900 | 13.432700 |
```

### pa06-audio-python-ft-explicit.log

```text
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.012900 | 1.974200 |
| open_close_session | 20 calls/batch | 231.442400 | 227.955000 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6189.699200 | 6188.008500 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 38.240867 | 36.550167 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 15.625000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43113.455000 | 43113.069500 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 53.246667 | 52.861167 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.894800 | 16.934100 |
```

### pa06-audio-python-production-retry.log

```text
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.000700 | 1.956500 |
| open_close_session | 20 calls/batch | 230.276700 | 226.047600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6202.130900 | 6200.743900 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 50.672567 | 49.285567 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 62.500000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43129.702600 | 43129.573200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 69.494267 | 69.364867 |
| first_sample_latency | input start to nonempty callback; 10 runs | 34.441100 | 21.978200 |
```

### pa06-audio-python-ft-production.log

```text
| op | size | python median ms | python min ms |
| enumerate_devices | 20 calls/batch | 2.081700 | 1.975800 |
| open_close_session | 20 calls/batch | 233.120500 | 227.930500 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6204.256500 | 6201.434700 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 52.798167 | 49.976367 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43128.758100 | 43120.509100 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 68.549767 | 60.300767 |
| first_sample_latency | input start to nonempty callback; 10 runs | 36.206700 | 32.954000 |
```
