# Impulcifer 재작성 스택 조사 종합 (2026-09-07)

조사 방식은 다음과 같습니다. ASTRA(gpt-6-astra-high) 직접 워커 9기를 병렬로 돌려 각각 한 차원씩 맡겼고, 모든 주장에 [V] 검증(출처 URL 또는 저장소 줄 번호), [I] 추론, [U] 미확인 표지를 붙이게 했습니다. 이 문서는 그 9개 보고서(`report-01` ~ `report-09`)를 Claude가 읽고 종합한 것입니다. 작업서는 `brief-*.md`에 그대로 남겨 두었습니다. 조사 기준 저장소 리비전은 `0144dcc`(2.13.3)입니다.

| 보고서 | 차원 |
|---|---|
| report-01-audio-io | 언어별 오디오 I/O 라이브러리, ASIO, WASAPI 8채널 초과 |
| report-02-dsp | scipy 프리미티브 전수 대응표, 최소위상·firwin2·nnresample 알고리즘, 패리티 검증안 |
| report-03-rust-tauri | S1 심층 |
| report-04-cpp-electron | S2 심층 |
| report-05-go-wails | S3 심층 |
| report-06-alternatives | 제시되지 않은 스택 9종 |
| report-07-packaging | 패키징·업데이터·CI·PyPI 연속성 |
| report-08-precedents | 유사 도구 선례와 셸별 실제 앱 |
| report-09-migration | 모듈별 이식 난이도, 마이그레이션 전략, 마일스톤 |

## 1. 결론

상위 3개 스택은 다음과 같습니다.

1. **Rust 코어 + Tauri 2 셸** (S1을 수정한 형태). 웹 UI와 i18n을 그대로 살리고, PyO3/maturin으로 PyPI를 유지하며, Windows 업데이터는 Velopack Rust SDK로 그대로 둡니다. 오디오는 CPAL이 아니라 PortAudio를 얇은 FFI로 감쌉니다.
2. **C++ 코어 + Electron 셸(사이드카 프로세스)** (S2를 수정한 형태). DSP 생태계는 네 언어 중 C++가 가장 두텁고(pocketfft, Eigen, Ceres, Iir1), 유지보수자는 이미 EqualizerAPO-XT로 C++/MSVC/Velopack 파이프라인을 운영하고 있습니다. 네이티브 애드온 대신 별도 프로세스로 붙여 Electron ABI 결합을 피합니다. 같은 C++ 코어를 Tauri 뒤에 두는 조합도 성립합니다.
3. **C# (.NET 10) + Avalonia 11 + Velopack** (제시되지 않은 스택 중 1위). Velopack이 .NET 태생이라 Windows 패키징이 가장 단순하고, MathNet에 유계 LM이 실제로 있습니다. 대신 웹 UI 2,960줄을 버리고 네이티브 UI를 새로 그려야 하며 PyPI는 사실상 포기합니다.

**Go + Wails(S3)는 DSP 언어로는 탈락**입니다. gonum에 유계 잔차 최적화기가 없고(이슈 #1725 미해결), 최소위상·firwin2·nnresample 등을 전부 손으로 써야 하며, PortAudio 때문에 어차피 cgo가 필요해 '순수 Go 단일 바이너리' 장점이 사라집니다. Wails v3는 beta.17(2026-09-06)이고 GA 마일스톤은 9월 15일입니다. Velopack Go SDK는 '계획 중'입니다. 네이티브 코어 위의 셸로만 쓸 수 있는데, 그 자리라면 Tauri가 더 낫습니다.

**이행 경로는 빅뱅이 아니라 스트랭글러입니다.** 네이티브 코어를 먼저 PyO3(Rust) 또는 nanobind(C++)로 파이썬 앱 뒤에 붙이고, 현재 파이썬 구현을 오라클로 삼아 스테이지별 골든 테스트로 패리티를 확보한 뒤, 마지막에 pywebview를 Tauri로 바꿉니다. 보고서 06과 09가 독립적으로 같은 결론을 냈습니다.

## 2. 스택과 무관하게 확정된 사실

### 2.1 진짜 비용은 셸이 아니라 scipy 의미론입니다

9개 보고서가 예외 없이 1순위 위험으로 지목한 것은 수치 의미론의 이식입니다. 대응표(report-02)는 20개 프리미티브 계열 중 언어를 막론하고 11~13개를 손으로 써야 한다고 판정했습니다.

| 언어 | 손수 구현(C) | 라이브러리 있으나 위험(R) | 호환 계층 예상 줄수 |
|---|---:|---:|---:|
| C++ | 13 | 5 | 2,000~4,000 |
| Rust | 12 | 6 | 1,800~3,800 |
| C# | 11 | 6 | 2,000~4,200 |
| Go | 13 | 5 | 2,800~5,200 |

특히 다음 항목은 라이브러리 이름만 보고 대체할 수 없습니다.

- **유계 최소제곱(scipy `least_squares` TRF)**. 어느 언어에도 드롭인이 없습니다. Ceres(C++)는 유계 LM/Dogleg, MathNet(C#)은 sin 변환 유계 LM, Rust `basin`은 반사 스텝이 없는 Coleman-Li 변종, `trust-region-least-squares`는 이름과 달리 무계입니다. 다만 **이 최적화기는 기본 BRIR 파이프라인에서 호출되지 않습니다**(core/, impulcifer.py, application/에 호출자 없음, Claude가 grep으로 재확인). AutoEQ 공개 API에만 해당하므로 첫 마일스톤에서 제외할 수 있습니다.
- **nnresample**. 단순 `resample_poly`가 아닙니다. 32,001탭 Kaiser(beta 5.65326), 2^19 FFT로 Nyquist 널 탐색 후 재설계, 특정 prepad/trim 규칙을 따릅니다. libsamplerate·soxr·rubato는 '좋은 리샘플러'이지 같은 계약이 아닙니다.
- **minimum_phase**. scipy 1.17.1 소스 기준으로 `1e-7*min(A[A>0])` 바닥, 짝수 n_fft에서 중간 리프터 0, `half=True`의 제곱근 크기 목표 등을 그대로 옮겨야 합니다. AutoEQ는 `n_fft=len(ir)`을 명시하므로 기본값과 다릅니다.
- **FITPACK 스플라인**. k=1 외에 **k=2가 실제로 쓰입니다**(클리핑 꺾임 복원). k=3은 not-a-knot이지 natural cubic이 아닙니다. 외삽은 다항식 연장입니다.
- **savgol_filter의 `interp` 경계 모드**를 1,000회 반복하므로 경계 처리 차이가 증폭됩니다.
- 레거시 `fftpack.next_fast_len`(2·3·5 smooth)과 `fft.next_fast_len`이 둘 다 쓰입니다.

### 2.2 작업서의 오류 두 가지

조사 중 워커들이 제 작업서의 전제를 두 군데 바로잡았습니다.

- 현재 WAV 출력은 **PCM_32 정수**이지 IEEE float32가 아닙니다(`core/audio_io.py:82-97`, Claude가 재확인). R3의 float 출력은 새 형식 요구이며, 패리티 게이트와 분리해서 결정해야 합니다.
- 테스트 파일은 56개가 아니라 `test_*.py` 53개이고, i18n 키는 264개가 아니라 433개입니다(CLAUDE.md의 264 기술이 낡았습니다).

### 2.3 오디오 I/O는 언어와 무관하게 PortAudio입니다

- CPAL 0.18.2는 Windows에서 WASAPI만 지원하고 MME/DirectSound가 없으며 채널 마스크를 공개하지 않습니다. 현재 코드의 DirectSound→MME→WASAPI 폴백을 재현할 수 없습니다.
- PortAudio ASIO 구현은 스트림을 하나만 엽니다(`openAsioDeviceIndex` 가드). 같은 인터페이스에서 ASIO 재생과 녹음을 하려면 듀플렉스 예외가 필요합니다.
- ASIO SDK 2.3.4(2025-10-15)는 GPLv3 또는 독점 라이선스 양자택일입니다. MIT 프로젝트가 ASIO를 켠 바이너리를 배포하려면 명시적 결정이 필요합니다.
- SDL3와 SDL3-CS는 8채널까지만 변환합니다. Oto는 스테레오 출력 전용입니다. Java Sound 표준 공급자는 2채널/16비트입니다. 모두 탈락입니다.
- 7.1.6의 TSL/TSR에 대응하는 WAVEFORMATEXTENSIBLE 스피커 비트가 없습니다. 물리 출력 매핑을 따로 설계해야 합니다.
- 부수 발견입니다. sounddevice 문서상 편의 함수 `play()`와 `rec()`는 서로를 중단시킵니다. 현재 `core/recorder.py`의 스레드 분리 설계가 문서화된 계약과 어긋나므로, 새 구현은 명시적 InputStream/OutputStream 소유자를 둬야 합니다.

### 2.4 패키징과 업데이터

| 항목 | 확인된 사실 |
|---|---|
| Velopack 1.2.0 | C#, Rust, C/C++, JS/Electron, **Python** SDK 완료. Go는 '계획 중'. `vpk`는 언어 무관. macOS(.pkg)와 Linux(AppImage) 지원 |
| Tauri 2.11.5 | AppImage가 WebKitGTK를 동봉함(시스템 WebKit 필수 아님). 업데이터 플러그인 2.11.0, 서명 필수, 델타 없음 |
| Wails | v2.15.0 안정, v3.0.0-beta.17 프리릴리스. v3에는 내장 업데이터 있음. Linux는 v3부터 GTK4/WebKitGTK 6.0 기본 |
| Electron 44.2.0 | Windows 런타임 ZIP 158.2MB. 현재 Impulcifer v2.14.0 Windows Setup은 216.2MB |
| WebView2 | Windows 11 내장, Windows 10은 대부분 설치. evergreen bootstrapper 권장, 고정 런타임은 250MB 초과 |
| PyO3 0.29 | **3.13t 지원 제거**. cibuildwheel 4.0도 3.13t 제거. abi3는 3.14t를 덮지 않음 |
| nanobind 3.0.1 | Python 3.10 이상 요구(현재 프로젝트는 3.9 지원) |
| 라이선스 | JUCE 8/9는 AGPLv3 또는 상용. Qt Charts·QCustomPlot은 GPL/상용. FFTW GPL. pocketfft BSD-3 |

### 2.5 프론트엔드 재사용

`webview_ui/app.js`는 브릿지 접근이 `api()` 한 곳(58행)에 모여 있고, 그 밖에 `pywebviewready` 이벤트와 준비 검사(933, 1486행), Python/GIL 정보 페이지(1216~1244행), CTk/WebView 전환 설정(1370~1373행)만 호스트 종속입니다. Tauri는 `withGlobalTauri` + 정적 `frontendDist`로 번들러 없이 이 파일들을 그대로 쓸 수 있고, Wails v3는 `-b` 옵션으로 런타임을 동봉한 JS 바인딩을 생성하며, Electron은 preload/contextBridge 어댑터 하나면 됩니다. 어느 웹 셸이든 3천 줄 중 어댑터 수십 줄만 바뀝니다.

### 2.6 패리티 전략

SHA-256 동일성은 다른 언어 사이에서 성립하지 않습니다. 보고서 02·03·04·09가 제안한 게이트는 다음과 같습니다(측정 전 초기값이지 확정 임계값이 아닙니다).

| 비교 대상 | 초기 제안 게이트 |
|---|---|
| 순수 프리미티브(FFT 왕복, 계수) | `abs(a-b) <= 1e-12 + 1e-10*abs(ref)`, 모양과 비유한값 정책은 정확히 일치 |
| 스테이지 전달 함수 | 유의 대역에서 0.01dB 이하, 깊은 널은 절대 오차로 |
| 정렬·피크·크롭·레이아웃 | 정수 결정 정확 일치(±2샘플 전역 허용 금지) |
| 최종 BRIR(양자화 전) | 정규화 RMS 1e-6 이하, 1/12옥타브 밴드 0.01dB 이하, ITD 정수 오프셋 0, ILD 0.01dB, IPD 0.1도 |
| PEQ 최적화기 | 파라미터 동일성 대신 적합 응답 비교. RMS 추가 오차 0.05dB, 최대 곡선 차 0.2dB에서 시작 |

핵심은 파이썬 오라클을 고정(커밋·의존성 버전·CPU·스레드)하고 스테이지마다 float64 골든 배열을 내보낸 뒤, 새 구현의 각 프리미티브에 **파이썬 스테이지의 입력**을 먹여 첫 발산 지점을 찾는 것입니다. SHA 게이트는 파이썬끼리의 회귀 검사로 남겨 두고, 알려진 결함(1샘플 지연, 0.02dB 게인, natural spline 등)을 주입해 새 게이트가 실제로 잡는지 확인한 뒤에만 교차 언어 해시 비교를 은퇴시킵니다.

## 3. 후보 전체 판정

| 후보 | 판정 | 결정적 근거 |
|---|---|---|
| S1 Rust + Tauri 2 | **채택(1위)** | 웹 UI·i18n 보존, PyO3 wheel, Velopack Rust SDK. CPAL 대신 PortAudio FFI 필요 |
| S2 C++ + Electron | **채택(2위)** | DSP 생태계 최상, XT 경험. 애드온 대신 사이드카. 158MB 런타임은 현재 216MB 대비 감당 가능 |
| S3 Go + Wails | **탈락(DSP 언어로)** | gonum 유계 솔버 없음, cgo 불가피, v3 베타, Velopack Go 미완 |
| A C# + Avalonia | **채택(3위)** | Velopack 태생, MathNet 유계 LM, ScottPlot. 웹 UI 폐기, PyPI 포기 |
| B C++ + JUCE 8 | 조건부 | AGPLv3/상용 결정 선행. FFT가 float 전용이라 DSP 대체 불가. WebView 릴레이는 유효 |
| C C++ + Qt 6 | 차점 | Open Sound Meter 선례, XT 경험. LGPL 동적 링크, Charts는 GPL. 웹 UI 폐기 |
| D Rust 네이티브 GUI(egui) | 예비 | 웹뷰 제거 대가로 UI 전면 재작성. iced·GPUI·Dioxus는 탈락 |
| E Flutter + Rust | 예비 | 데스크톱 성숙하나 Dart/코드생성 층이 추가되고 UI 폐기 |
| F Kotlin Compose | 먼 예비 | REW가 Java라는 사실은 가능성의 증거이지 우위의 증거가 아님 |
| G 브라우저/WASM | 측정은 탈락, 처리 전용은 별도 제품 | 호스트 API 열거와 ffmpeg 실행이 브라우저에 없음 |
| H Python 유지 + 사이드카 | **이행 전략으로 채택** | 목표 스택이 아니라 1위·2위로 가는 길 |
| I Zig/Nim/Swift/D | 탈락 | 자산 재사용 0, 생태계 부담만 추가 |

## 4. 상위 3개 스택과 아키텍처

### 4.1 1위. Rust 코어 + Tauri 2 셸

```text
crates/
  impulcifer-dsp/        f64 알고리즘, ProcessingConfig 정본, 스테이지 테이블
                         (RustFFT/RealFFT, ndarray 또는 nalgebra, 자체 호환 계층:
                          firwin2·minimum_phase·FITPACK k1/k2/k3·savgol interp·
                          nnresample 재설계·SOS·find_peaks)
  impulcifer-audio-io/   PortAudio를 얇은 C ABI로 감싼 어댑터. 호스트 API 열거,
                         독립 출력 16ch + 입력 2ch 스트림, ASIO는 듀플렉스 예외
  impulcifer-analysis/   FR/IR/ILD/IPD/IACC/spectrogram 배열 + PNG(plotters)
                         + 오프라인 HTML(동봉 Plotly.js). WAV 트랙 순서·헤더 정책
  impulcifer-jobs/       job 스냅샷, seq 저널, AtomicBool 취소 토큰, rayon 스피커 병렬
  impulcifer-app/        Tauri 바이너리. #[tauri::command] 22개가 기존 {ok,data|error}
                         봉투를 그대로 반환. 대용량 배열은 tauri::ipc::Response 바이너리
  impulcifer-cli/        Tauri 의존 없는 독립 실행 파일(헤드리스 배치)
  impulcifer-python/     PyO3 cdylib + maturin. `pip install impulcifer-py313` 유지
webview_ui/              기존 HTML/CSS/JS 그대로. api() 어댑터 + 준비 검사만 교체
i18n/locales/            433키 × 9언어 JSON 그대로
```

- 업데이트는 Windows에서 Velopack Rust SDK(`VelopackApp::build().run()`을 Tauri 초기화 전에)로 기존 팩 ID·피드·설치 경로를 유지하고, macOS/AppImage는 Tauri 업데이터를 씁니다. 한 설치에 업데이터 둘을 두지 않습니다.
- Linux는 Tauri AppImage(WebKitGTK 동봉, Ubuntu 22.04 빌드 베이스)를 '최선 노력' 매트릭스로 공개하고 CLI/tarball을 폴백으로 둡니다.
- Windows는 WebView2 evergreen bootstrapper, 부재 시 안내.
- PyPI wheel은 3.13t를 포기하거나 구 PyO3 라인을 따로 유지해야 합니다(PyO3 0.29에서 제거).
- 위험 1순위는 scipy 의미론, 2순위는 16-out/2-in 하드웨어 미검증, 3순위는 혼자서 DSP·플롯·wheel·오디오 FFI·설치기를 동시에 바꾸는 범위 폭발입니다.

### 4.2 2위. C++ 코어 + Electron 셸(사이드카)

```text
cpp/
  src/dsp/               double 전용. pocketfft(BSD-3, 임의 길이), Eigen 5, Iir1(MIT),
                         Ceres 2.2(유계 LM, TRF 대체는 '알고리즘 변경'으로 선언)
  src/pipeline/          스테이지 테이블, 결정론적 기본값
  src/audio/             PortAudio 직접 호출 + libsndfile. vcpkg 기본 포트는 Linux에서
                         ALSA가 꺼져 있으므로 오버레이 포트 필수
  src/analysis/          분석 배열 사양. PNG는 Cairo/Pango 또는 렌더러에서 캡처
  src/service/           `impulcifer-core --serve-stdio`: JSON-RPC 2.0 줄 단위 프레임,
                         stdout은 프로토콜 전용, stderr는 로그
  src/cli/               동일 코어의 콘솔 실행 파일
bindings/python/         nanobind + scikit-build-core (Python 3.10+; 3.9는 별도 정책)
desktop/
  main.cjs               child_process.spawn(코어, shell:false), 다이얼로그, Velopack JS SDK
  preload.cjs            contextBridge로 22개 메서드만 노출. nodeIntegration off, sandbox on
webview_ui/              기존 파일 그대로. window.impulcifer 어댑터
```

- 애드온을 쓰지 않는 이유는 Electron ABI 재빌드, V8 메모리 케이지, 워커 스레드 안의 동기 호출이 취소를 막는 문제 때문입니다. 별도 프로세스는 코어가 죽어도 셸이 살아남습니다.
- naudiodon은 2021년 이후 정지 상태이므로 오디오는 전부 C++ 쪽에 둡니다.
- 크기 논리("파이썬도 무거웠다")는 다운로드 크기에서만 성립합니다. RAM·기동 시간은 같은 머신에서 측정하기 전까지 미확인입니다.
- 같은 C++ 코어를 Tauri 사이드카로 붙이는 조합(보고서 09가 제안)도 유효합니다. Electron을 고르는 실질적 이유는 세 플랫폼에서 렌더러가 하나(Chromium)라는 점뿐이며, 그 대가로 8주 주기 Chromium 보안 릴리스를 따라가야 합니다.
- 유지보수자의 EqualizerAPO-XT(C++ / Qt 6.10.1 / MSVC / Velopack / GitHub Actions, 2.51.0)가 이 스택의 실측 선례입니다. 다만 XT의 provisioning 스크립트에 기록된 aqt 추출 경쟁과 vcpkg 드리프트가 보여주듯, 언어를 바꿔도 빌드 드리프트는 사라지지 않고 종류만 바뀝니다.

### 4.3 3위. C# (.NET 10 LTS) + Avalonia 11 + Velopack

```text
Impulcifer.Core/         double 배열. MathNet(FFT Bluestein·QR·유계 LM),
                         NWaves는 double 컴포넌트만 선별(Fft64는 2^n 제한이라 부적합),
                         나머지는 자체 호환 계층
Impulcifer.Audio/        PortAudio P/Invoke 자체 유지(PortAudioSharp2는 콜백 필수·
                         blocking read/write 없음). NAudio 3는 macOS 백엔드 없음
Impulcifer.Analysis/     분석 배열 + ScottPlot 5 오프스크린 PNG + 동봉 JS HTML 리포트
Impulcifer.Desktop/      Avalonia 네이티브 UI 전면 신규(웹 UI 폐기). 433키 카탈로그 재사용
Impulcifer.Cli/          Avalonia 미참조 콘솔 프로젝트
Impulcifer.Native/       (선택) NativeAOT C ABI 내보내기 → ctypes 래퍼 wheel
```

- 배포는 self-contained JIT 우선, NativeAOT는 검증 뒤 선택입니다. Velopack C# API가 가장 성숙하고 `VelopackApp.Build().Run()` 한 줄로 기존 설치 ID를 잇습니다.
- Avalonia 공식 WebView는 Accelerate 상용 컴포넌트이고 Linux는 다이얼로그 형태라, 웹 UI를 살리려고 이 스택을 고르면 장점이 사라집니다.
- PyPI는 pythonnet(FT 미지원, 런타임 필요) 아니면 NativeAOT C ABI뿐이라 사실상 CLI 래퍼 수준입니다.
- 이 스택이 3위인 이유는 '가장 단정한 단일 애플리케이션'이 되기 때문이지 DSP 이식이 쉬워서가 아닙니다. 웹 UI를 버릴 각오가 있을 때만 의미가 있습니다.

## 5. 권장 이행 전략과 마일스톤

보고서 09의 추정으로 C++ 코어 16~28주, Rust 코어 18~32주(에이전트 보조 기준 한 주는 25~35시간의 사양·검토·검증)입니다. 실측이 아니라 계획 사전값입니다.

| 마일스톤 | 시기 | 종료 조건 |
|---|---|---|
| M0 계약 동결 | 1~2주 | 파이썬 오라클 커밋·환경 고정, 5개 무결성 시나리오 골든 매니페스트, PCM_32/float 정책 결정 |
| M1 수치·빌드 파일럿(정지/진행 결정) | 2~4주 | C++/nanobind와 Rust/PyO3가 같은 픽스처(FFT→savgol→minimum_phase FIR)를 통과, 3 OS 빌드, 16-out/2-in 하드웨어 프로브 |
| M2 프리미티브 라이브러리 | 4~8주 | FFT/convolution/스플라인/SOS/WAV 레이아웃/복구 계약 통과, CLI와 Python 어댑터가 코어 공유 |
| M3 BRIR 경로 | 8~14주 | estimator, HRIR/decay, room/headphone EQ, virtual bass, mic correction, resampling 전 스테이지 게이트 통과. 플롯은 파이썬이 계속 렌더 |
| M4 분석·렌더·서비스·오디오 | 12~22주 | PNG/HTML 데이터 테스트, job/취소/이벤트, 장치 매트릭스, 셸 없는 CLI 완주 |
| M5 셸과 설치 업데이트 전환 | 18~28주+ | 기존 웹 UI가 Tauri 어댑터로 동작, 기존 Velopack 설치에서 후보 버전으로 업그레이드 성공 |
| M6 데스크톱 Python 의존 제거 | M3~M5 이후 | 네이티브 데스크톱/CLI에 Python 과학 스택 불필요, 2.x LTS/PyPI 정책 명시 |

첫 세 가지 산출물은 다음과 같습니다.

1. `migration-contract-v1` 사양과 골든 매니페스트. 기준 SHA·환경, 스테이지 배열, 5개 CLI 시나리오, 허용오차 예산, `tests/migration/` 프로토콜.
2. 이중 백엔드 차등 실행기와 비교 파일럿 하나. 같은 픽스처를 Python과 C++/Rust에 통과시켜 첫 발산 지점을 보고. 코어 언어는 이 결과와 의존성 감사로 정합니다.
3. 파이썬 API 뒤에서 옵트인으로 릴리스 가능한 네이티브 슬라이스 하나. 배열 프리미티브와 출력 레이아웃 복구부터 시작해 검증된 FIR 경로를 연결. 기본 BRIR 동작은 게이트 통과 전까지 불변.

M1이 실제 정지/진행 결정입니다. Rust가 검증되지 않은 최적화기·FIR 스택을 요구하면 C++로 가거나 그 부분만 파이썬을 더 오래 유지하고, C++ wheel/빌드 비용이 수치 이득보다 커지면 Rust로 갑니다. 헬로월드 설치기 스크린샷으로 스택을 고르지 않습니다.

## 6. 유지보수자가 정해야 할 것

1. 대상 하드웨어. 어떤 인터페이스/AVR이 12/14/16 물리 채널을 어느 호스트 API·샘플레이트로 노출해야 하는지. ASIO 동일 장치 듀플렉스 예외를 허용할지.
2. ASIO SDK 배포 조건. GPLv3 결합 배포를 받아들일지, 독점 계약을 맺을지, ASIO를 빼고 WASAPI만 갈지.
3. 출력 형식. PCM_32 유지, float32 옵션 추가, 기본값 변경 중 하나.
4. PyPI 범위. `pip install` CLI만 유지할지, 기존 클래스/함수 API까지 보존할지. 3.9와 3.13t 지원을 언제 끊을지.
5. 허용오차 승인. 제안된 0.01dB/ITD 0샘플 등을 실측 뒤 프로젝트 정책으로 확정.
6. Linux 지원 수준. AppImage 최선 노력 매트릭스로 갈지, 피드에서 Linux를 어떻게 분리할지(구 클라이언트는 접미사로 자산을 고르므로 조용히 빼면 안 됨).
7. 코드 서명. Azure Artifact Signing은 개인의 경우 미국/캐나다 거주자만 가능. 서명 없이 갈지 대안을 찾을지.
8. 기존 알려진 결함의 처리. 룸 보정 마스크의 전체 Hann, mic correction의 same/full 불일치, nnresample 동일 레이트 실패 등을 패리티 작업 중 '고치지 않고' 옮길지 의도적으로 수정할지.
