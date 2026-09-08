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
