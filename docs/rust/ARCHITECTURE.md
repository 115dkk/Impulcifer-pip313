# Impulcifer 3.x 아키텍처 (Rust 코어 + Tauri 2)

이 문서가 3.x 구조의 정본입니다. 결정 근거는 ADR 0002와 `docs/research/rewrite-stack-2026-09/`에 있습니다. 여기 적힌 크레이트 경계, 타입 이름, 함수 시그니처, 오류 정책은 워커 작업서가 그대로 따라야 하는 계약입니다.

## 1. 원칙

1. **IPC 불변.** 2.x의 `application/impulcifer_service.py`가 정의한 메서드 23개, 응답 봉투, 잡 폴링 모델을 그대로 옮깁니다. 프론트엔드 파일은 한 줄도 바꾸지 않습니다.
2. **f64 DSP, scipy 의미론 보존.** 프리미티브는 scipy와 같은 이름·의미로 구현하고 스테이지별 골든으로 검증합니다.
3. **unsafe 0.** 모든 크레이트 루트에 `#![forbid(unsafe_code)]`. 예외 후보는 `impulcifer-sys-win` 하나이며 초기 허용 0입니다.
4. **WASAPI 단일 경로.** ASIO 없음, DirectSound/MME 없음.
5. **기능마다 테스트.** `features.toml`에 없는 기능은 존재하지 않는 것이고, 구현됐다고 표시된 기능에 검증 테스트가 없으면 CI가 떨어집니다.
6. **2.x는 오라클.** 파이썬 구현이 골든을 내보내고 Rust가 허용오차로 맞춥니다.

## 2. 워크스페이스

```text
Cargo.toml                      워크스페이스 루트 (lints, 공용 의존성 버전)
rust-toolchain.toml             1.97.0 고정
features.toml                   기능 등록부 (게이트의 입력)
crates/
  impulcifer-types/             설정·상수·IPC 봉투·잡 모델·오디오 트레이트. 의존성 없음(serde만)
  impulcifer-dsp/               f64 프리미티브와 파이프라인 스테이지
  impulcifer-io/                WAV 읽기/쓰기, ffmpeg 프로세스, CSV/TXT
  impulcifer-analysis/          분석 배열(FR/IR/ILD/IPD/IACC/EDC/spectrogram), PNG, 오프라인 HTML
  impulcifer-audio-io/          백엔드 선택과 측정 세션 오케스트레이션, cpal 백엔드(macOS/Linux)
  impulcifer-sys-win/           wasapi-rs 백엔드(Windows). unsafe 예외 후보. Daybreak 전담
  impulcifer-jobs/              잡 레지스트리, seq 저널, 취소 토큰
  impulcifer-service/           IPC 메서드 23개의 구현. 호스트 기능은 HostAdapter 트레이트로 주입
  impulcifer-cli/               헤드리스 CLI (2.x `impulcifer` 옵션 호환)
  impulcifer-python/            PyO3 모듈 + maturin (wheel)
  impulcifer-policy/            게이트 테스트만 있는 크레이트 (unsafe 스캔, forbid 검사, features.toml 검증)
apps/
  impulcifer-app/               Tauri 2 앱. 커맨드 1개 + 폴리필 초기화 스크립트 + 다이얼로그/테마/업데이터
apps/impulcifer-app/ui/         3.x 프론트엔드 (frontendDist; ADR 0003에서 webview_ui/를 분기)
crates/impulcifer-service/locales/  3.x 문자열 오버레이 (2.x i18n/locales 위에 겹침, 아홉 언어 한 키 집합)
webview_ui/, i18n/locales/      2.x 소유. 3.x 작업에서 수정하지 않음
```

의존 방향은 아래로만 흐릅니다. `sys-win`은 `types`와 `wasapi`에만 의존하고 `audio-io`·`service`·`app`을 모릅니다. `dsp`와 `python`은 `sys-win`을 모릅니다. `cli`는 Tauri를 링크하지 않습니다.

```text
app ─┬─> service ─┬─> jobs ──> types
     │            ├─> dsp ───> types
     │            ├─> audio-io ─┬─> types
     │            │             ├─> sys-win ──> types, wasapi      [windows]
     │            │             └─> cpal                            [macos, linux]
     │            ├─> io, analysis ──> types
     │            └─> types
     └─> tauri, tauri-plugin-{dialog,opener,updater}, velopack
cli ──> service(NoopHost), dsp, io, analysis, types
python ──> dsp, io, analysis, types            (audio-io 미포함)
policy ──> (없음; 파일 시스템만 읽음)
```

## 3. 데이터 모델 (`impulcifer-types`)

### 3.1 ProcessingConfig

2.x `core/pipeline.py`의 `ProcessingConfig` 필드 33개를 같은 이름·같은 기본값으로 옮깁니다. `#[serde(default)]`로 부분 JSON을 받고, `from_kwargs`는 모르는 키를 무시합니다(2.x와 동일). 기본값의 정본은 이 구조체 하나입니다. CLI 옵션과 서비스의 `brir_defaults`는 여기서 파생됩니다.

| 필드 | 타입 | 기본값 |
|---|---|---|
| dir_path, test_signal, room_target, room_mic_calibration, headphone_compensation_file | Option<String> | None |
| fs | Option<u32> | None |
| plot, interactive_plots | bool | false |
| channel_balance | Option<String> | None |
| decay | Option<DecaySpec> (숫자 또는 채널별 맵) | None |
| target_level | Option<f64> | None |
| fr_combination_method | String | "average" |
| specific_limit, generic_limit | f64 | 400, 300 |
| bass_boost_gain, bass_boost_fc, bass_boost_q | f64 | 0.0, 105, 0.76 |
| tilt | f64 | 0.0 |
| do_room_correction, do_headphone_compensation, do_equalization | bool | true |
| remove_silent_channels | bool | false |
| head_ms | f64 | 1.0 |
| jamesdsp, hangloose | bool | false |
| microphone_deviation_correction | bool | false |
| mic_deviation_strength | f64 | 0.7 |
| mic_deviation_debug_plots | bool | false |
| output_truehd_layouts | bool | false |
| vbass | bool | false |
| vbass_freq | u32 | 250 |
| vbass_hp | f64 | 15.0 |
| vbass_polarity | String | "auto" |

### 3.2 IPC 봉투와 메서드

- 성공: `{"ok": true, "data": <object>}`
- 실패: `{"ok": false, "error": {"code": <ErrorCode>, "message": <string>, "details": <object>, "retryable": <bool>}}`
- `ErrorCode`는 2.x 정본 11개입니다. INVALID_REQUEST, FILE_NOT_FOUND, INTERNAL_ERROR, UPDATE_FAILED, OUTPUT_MISSING, JOB_NOT_FOUND, DEVICE_ERROR, CONFIRMATION_REQUIRED, UPDATE_CHECK_FAILED, JOB_NOT_CANCELLABLE, JOB_BUSY.
- `IpcMethod`는 24개입니다. bootstrap, list_audio_devices, start_recording, start_brir, start_output_recovery, plan_output_recovery, poll_job, cancel_job, get_ui_settings, set_language, set_theme, set_skin, set_frontend, get_system_info, resolve_recording_paths, detect_sweep, generate_sweep_set, open_path, check_for_updates, start_update, apply_pending_update, select_file, select_directory, open_url.
- 인자는 pywebview와 같은 위치 인자 배열로 옵니다. `poll_job(job_id, after_seq=0)`처럼 기본값이 있는 인자는 생략 가능해야 합니다.
- 서비스 메서드는 예외를 던지지 않고 항상 봉투를 반환합니다. Tauri 커맨드도 `Result`가 아니라 `serde_json::Value`를 반환합니다.

### 3.3 잡 모델

- `JobKind`: recording, brir, output_recovery, update.
- `JobStatus`: running, cancel_requested, succeeded, failed, cancelled. 종료 상태는 뒤의 셋입니다.
- `JobEvent { seq: u64, kind: progress | log | status, payload: Value }`. seq는 잡마다 1부터 단조 증가합니다.
- `poll_job(job_id, after_seq)`는 `seq > after_seq`인 이벤트와 스냅샷, `next_seq`를 돌려줍니다. 보관 이벤트는 2,000개로 제한합니다(2.x `_MAX_JOB_EVENTS`).
- 동시에 활성인 잡은 하나입니다(2.x와 동일). 둘째 요청은 JOB_BUSY입니다.
- 녹음 잡은 취소 불가(`cancellable: false`)로 시작합니다. 안전한 stop/drain이 실기에서 증명되면 그때 엽니다.

### 3.4 오디오 트레이트

`types::audio`에 두어 `sys-win`과 `audio-io`가 같은 계약을 봅니다.

```rust
pub enum Direction { Input, Output }
pub enum ShareMode { Exclusive, SharedAutoConvert }   // cpal 백엔드는 무시
pub struct Endpoint { id, name, host_api, max_input_channels, max_output_channels, default_samplerate, is_default_input, is_default_output }
pub struct StreamSpec { sample_rate: u32, channels: u16 }
pub struct CancelToken(Arc<AtomicBool>)

pub trait AudioBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError>;
    fn probe(&self, ep: &Endpoint, dir: Direction, spec: StreamSpec, mode: ShareMode) -> Result<ProbeResult, AudioError>;
    fn open_output(&self, ep: &Endpoint, spec: StreamSpec, mode: ShareMode) -> Result<Box<dyn OutputSession>, AudioError>;
    fn open_input(&self, ep: &Endpoint, spec: StreamSpec, mode: ShareMode) -> Result<Box<dyn InputSession>, AudioError>;
}
pub trait OutputSession {   // !Send. 만든 스레드에서만 쓴다
    fn play_to_completion(&mut self, interleaved: &[f32], cancel: &CancelToken) -> Result<PlaybackReport, AudioError>;
}
pub trait InputSession {    // !Send
    fn start(&mut self) -> Result<(), AudioError>;
    fn read_into(&mut self, dst: &mut [f32], cancel: &CancelToken) -> Result<CaptureRead, AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
}
```

측정 세션 계약은 2.x `core/recorder.py::play_and_record`와 같습니다. 입력 스트림을 먼저 열고 준비 확인 → 출력 재생을 끝까지 하고 드레인 → 입력 정지 → 두 스레드 조인. 세션 객체는 스레드 간에 넘기지 않고, 스레드 사이에는 엔드포인트 ID·설정·소유 버퍼·결과만 오갑니다. Windows에서는 각 오디오 스레드가 자기 MTA를 초기화하고 자기 COM 객체를 그 스레드에서 해제합니다.

Windows에서 `share_mode=auto`는 exclusive를 먼저 시도하고 `UnsupportedFormat`이면 shared + auto-convert로 다시 시도하지만, 고정 `share_mode`는 실패해도 다른 모드로 다시 시도하지 않습니다. 채널 수는 엔드포인트 mix format과 같게 열고 트랙 배치는 우리가 합니다. wasapi-rs의 `WaveFormat::parse`와 `Device::from_raw`는 쓰지 않습니다. SILENT 패킷은 0으로 채웁니다.

## 4. 스레딩

- Tauri 메인 스레드는 셸만 담당합니다. 커맨드는 검증 후 잡 레지스트리에 넘기고 즉시 반환합니다.
- 잡은 전용 OS 스레드에서 돌고, 스피커별 병렬은 rayon 풀을 씁니다. 결과는 스피커 순서대로 모으고 전역 축약은 직렬로 합니다(부동소수점 결정성).
- 오디오 스트림은 방향마다 전용 OS 스레드입니다.
- 취소는 `CancelToken`(AtomicBool)을 스테이지 경계, 스피커 경계, 긴 반복 안에서 확인하는 협조적 취소입니다. `cancel_requested`는 워커가 실제로 멈춘 뒤에야 `cancelled`가 됩니다.

## 5. 파이프라인 (`impulcifer-dsp::pipeline`)

2.x `core/pipeline.py::_stage_table`의 26개 스테이지를 같은 순서·같은 게이트·같은 진행률 스텝 수로 옮깁니다. 스테이지 키는 `StageKey` enum(estimator, room_correction, headphone_compensation, equalization_files, target, open_measurements, plot_pre, crop_and_align, virtual_bass, mic_deviation_skipped, mic_deviation, write_responses, equalize, decay, channel_balance, normalize, write_readme, plot_post, plot_results, plot_additional, interactive_plots, resample, write_brirs, truehd_layouts, jamesdsp, hangloose)이고, 진행률 이벤트는 이 키를 그대로 실어 보냅니다(2.x가 로거 콜백에 원본 키를 넘기는 것과 같음).

출력 형식은 2.x와 같이 PCM_32 정수입니다. float32 출력은 별도 결정 전까지 넣지 않습니다. 트랙 순서 상수(HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER 등)는 `core/constants.py`를 그대로 옮기고 파이썬 덤프와 비교하는 테스트로 고정합니다.

## 6. DSP 프리미티브 (`impulcifer-dsp`)

모두 f64, `&[f64]`/`Vec<f64>` 기반이며 scipy 함수와 1:1입니다. 세부 의미(경계 처리, 홀수 길이, 정규화)는 `docs/research/rewrite-stack-2026-09/report-02-dsp.md` 2.4절이 정본입니다.

| 모듈 | 함수 | scipy 대응 |
|---|---|---|
| fft | rfft, irfft(n), fft, ifft, next_fast_len_legacy(2·3·5), next_fast_len | numpy.fft, scipy.fftpack/fft.next_fast_len |
| conv | convolve(mode), correlate(mode), correlation_lags | scipy.signal |
| filters | butter(order, wn, btype) -> Sos, sosfilt, tf2sos, rbj_peaking/shelf | scipy.signal, autoeq.biquad |
| fir | firwin2, minimum_phase(n_fft, half) | scipy.signal |
| smoothing | savgol_filter(window, polyorder, mode=interp) | scipy.signal |
| peaks | find_peaks(height) 플래토 중앙값 규칙 | scipy.signal |
| windows | hann(sym), hamming(sym), kaiser(beta), get_window | scipy.signal.windows |
| interp | Spline::new(x, y, k=1/2/3) FITPACK not-a-knot, ext=0 외삽 | InterpolatedUnivariateSpline |
| resample | nnresample_design(up, down) 32001탭 널-온-나이퀴스트, resample_poly(taps) | nnresample, resample_poly |
| stats | linregress, expit, uniform_filter2d(size 3, constant 0) | scipy.stats, special, ndimage |
| spectrogram | spectrogram(nperseg, noverlap, periodic hann) | scipy.signal |

## 7. 서비스와 호스트 (`impulcifer-service`, `apps/impulcifer-app`)

```rust
pub trait HostAdapter: Send + Sync {
    fn select_file(&self, kind: &str) -> Option<String>;
    fn select_directory(&self) -> Option<String>;
    fn open_path(&self, path: &str) -> Result<(), String>;
    fn open_url(&self, url: &str) -> Result<(), String>;
    fn apply_title_theme(&self, theme: &str);
}
pub struct ImpulciferService { /* settings, jobs, backend, host */ }
impl ImpulciferService {
    pub fn call(&self, method: IpcMethod, args: Vec<Value>) -> Value;  // 항상 봉투
}
```

Tauri 앱은 커맨드 하나만 노출합니다.

```rust
#[tauri::command]
fn pywebview_api(state: State<AppState>, method: String, args: Vec<Value>) -> Value
```

초기화 스크립트(`apps/impulcifer-app/src/bridge.js`)가 `window.pywebview = { api: Proxy }`를 만들어 각 메서드 호출을 `invoke("pywebview_api", { method, args })`로 넘기고, DOMContentLoaded 뒤에 `pywebviewready`를 발생시킵니다. `withGlobalTauri: true`, `frontendDist: ui`(`apps/impulcifer-app/ui`). 페이지 스크립트는 `// @ts-check`와 `ui/ipc.d.ts`의 IPC 선언으로 `tsc --checkJs`를 통과해야 합니다(`rust.yml`의 `js` 잡). 다이얼로그·open_path·open_url·타이틀바 테마는 앱 크레이트의 `TauriHost`가 구현합니다. `get_system_info`는 버전, 설치 종류, 운영체제, CPU 수, Rust 런타임, 설정·데이터 경로, 업데이트 채널을 반환하며 Tauri 호스트에서는 셸과 WebView 버전도 포함합니다. 업데이터는 Windows에서 Velopack Rust SDK, 그 밖은 Tauri 업데이터이며 한 설치에 둘을 함께 두지 않습니다.

## 8. CLI와 Python

- `impulcifer-cli`는 2.x `impulcifer --help`의 옵션 이름과 기본값을 그대로 제공합니다(clap). 옵션 목록은 ProcessingConfig 필드에서 파생되며 `features.toml`의 `config.*` 항목과 1:1입니다.
- `impulcifer-python`은 `impulcifer_native` 모듈로 `run(config: dict) -> dict`와 프리미티브 일부를 노출합니다. 입력은 소유 복사(`PyReadonlyArray` → `to_vec`), 긴 계산은 `Python::detach`, 출력은 `from_vec`. free-threaded 지원은 별도 계약 뒤에 선언합니다.

## 9. 게이트

1. **`features.toml`.** 기능마다 `id`, `kind`(ipc, config, stage, output, audio, cli, python), `status`(planned, implemented), `tests`(`crate-name::test_fn` 목록). 게이트 테스트(`crates/impulcifer-policy/tests/gates.rs`)는 (a) ipc 23개, config 33개, stage 26개가 전부 등록됐는지, (b) `implemented`인 기능의 테스트가 하나 이상 있고 그 이름의 `#[test] fn`이 해당 크레이트 소스에 실제로 있는지, (c) 모든 크레이트 루트에 `#![forbid(unsafe_code)]`가 있는지, (d) `crates/`·`apps/` 아래에 `unsafe` 토큰이 없는지(`impulcifer-sys-win`은 `unsafe-budget.toml`의 허용치, 초기 0)를 검사합니다.
2. **골든 테스트.** `tests/migration/export_goldens.py`(2.x)가 데모 측정과 합성 픽스처의 스테이지별 f64 배열을 `tests/migration/goldens/`에 내보내고, Rust 테스트가 report-02의 허용오차로 비교합니다. 정수 결정(피크 인덱스, 크롭 길이, 채널 순서)은 정확히 일치해야 합니다. 픽스처 예산은 작업서당 12 MB(200 kB를 넘는 배열은 `.f64`; 예외는 작업서에 명시), 저장소 전체 150 MB이며, 전체 배열은 단계 검증에 꼭 필요한 트랙에만 두고 나머지는 처음·끝 256개 표본과 max/argmax/RMS 통계로 고정합니다.
3. **하드웨어 프로브.** `impulcifer-sys-win`의 `examples/hardware_probe.rs`로 실제 장치에서 exclusive/shared, 44.1/48/96 kHz, 2/8/16채널 열기와 1초 재생·캡처를 실측하고 결과를 `docs/rust/HARDWARE.md`에 기록합니다.
4. **CI.** `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`(정책 게이트 포함), `cargo deny check`. 나머지 잡(Miri, sanitizer, geiger)은 report-11 §3.4에 따라 뒤에 붙입니다.
5. **성능 감사.** 크레이트가 착륙하면 ASTRA가 `docs/rust/packets/PA-astra-perf-audit.md`(공통 방법)와 `PAnn-astra-perf-<crate>.md`(연산 목록)로 같은 머신에서 2.x와 비교 측정합니다. 상대는 최신 CPython(3.14.5, 2.x 의존성 설치본; 2.x가 스레드로 병렬화하는 연산은 free-threaded 3.14t도)에서 도는 2.x이지 PATH의 기본 파이썬이 아닙니다. 정확성 골든의 오라클 환경(`tests/migration/README.md`)과는 별개입니다. `crates/<crate>/benches/perf.rs`(release, 11회 중앙값, 추가 의존성 없음) 대 `tests/migration/bench_oracle_<crate>.py`, 파이프라인이 실제로 처리하는 크기로, 모든 연산에서 `python/rust >= 1.0`이어야 하고 골든은 그대로 통과해야 합니다. 보고서는 `docs/rust/perf/<crate>.md`, 등록부에는 `perf.<crate>`가 `bench_smoke_<crate>` 테스트를 인용합니다. 파이프라인 전체(M2)와 출시 전(M5)에 한 번씩 더 돌고, 그때는 데모 BRIR 생성의 최대 상주 메모리도 함께 잽니다(Rust rayon 스레드 대 2.x 프로세스 풀 대 3.14t 스레드; 최적화 대상이 아니라 사용자가 궁금해한 측정 항목). 기능의 `implemented` 등록은 패리티 게이트이고 성능 감사는 `perf.<crate>` 항목(초기 `planned`)으로 따로 추적합니다. 크레이트는 그 항목이 `implemented`가 될 때까지 '감사 전'이며, `perf.pipeline-demo`는 M2, `perf.release`는 M5의 종료 조건입니다. 파이썬보다 느린 포팅은 완료가 아닙니다.

## 10. 작업 분배

- **Daybreak Blue:** `impulcifer-sys-win`(wasapi 백엔드, hardware_probe). unsafe가 필요해지면 그것도 Daybreak만 쓴다.
- **ASTRA:** `impulcifer-audio-io`(cpal 백엔드, 세션 오케스트레이션), `impulcifer-types`, `impulcifer-dsp`, `impulcifer-io`, `impulcifer-analysis`, `impulcifer-jobs`, `impulcifer-service`, `impulcifer-cli`, `impulcifer-python`, `apps/impulcifer-app`, `tests/migration/export_goldens.py`.
- **Claude:** 이 문서, 작업서, 스켈레톤과 시그니처, `features.toml`, 게이트 테스트, 디프 검토, 게이트 재실행, 커밋과 PR.

## 11. 마일스톤

| 단계 | 내용 | 종료 조건 |
|---|---|---|
| M0 | 스켈레톤 컴파일, 게이트 테스트, features.toml 등록 | `cargo test --workspace` 통과 |
| M1 | 하드웨어 프로브 실측, DSP 프리미티브 1차(fft/conv/filters/windows)와 골든 | 실측 문서, 골든 테스트 통과 |
| M2 | 파이프라인 전 스테이지 패리티(데모 5 시나리오) | 최종 BRIR 허용오차 게이트 통과, 데모 BRIR 생성 시간이 `python impulcifer.py` 이하(성능 감사) |
| M3 | 서비스·잡·Tauri 앱이 기존 webview_ui로 완주 | 녹음·BRIR·복원이 UI에서 동작 |
| M4 | CLI, Python wheel | 2.x 옵션 호환, `pip install` 후 `run()` |
| M5 | 패키징(Velopack/Tauri 업데이터), 기존 설치에서 업그레이드 | 설치·업데이트 실측 설치·업데이트 실측, 최종 성능 감사(전 크레이트 `perf.*` 등록) |
