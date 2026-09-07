# ADR 0002: 3.x는 Rust 코어 + Tauri 2 셸로 재작성한다

- **상태:** 결정됨 (2026-09-07)
- **결정자:** 유지보수자 (115dkk)
- **근거 자료:** `docs/research/rewrite-stack-2026-09/` (스택 조사 종합 00~03, ASTRA 보고서 11편)

## 맥락

2.x는 Python(numpy/scipy) + pywebview + Nuitka 스탠드얼론이다. Nuitka/pywebview 패키징 드리프트가 반복되고, 유지보수자가 Python 배포 기계를 더 이상 떠안고 싶지 않다. 2026-09-07 조사에서 Rust+Tauri, C+++Electron, Go+Wails, C#/Avalonia 등을 비교했고 Rust 코어 + Tauri 2가 1위였다. 이후 Windows 오디오 API와 Rust unsafe 조사가 뒤따랐다.

## 결정

1. **언어와 셸.** Rust 코어(f64 DSP, 오디오 I/O, 잡 모델, 서비스)와 Tauri 2 셸. 아키텍처는 `docs/rust/ARCHITECTURE.md`가 정본이다.
2. **IPC는 2.x의 것을 그대로 쓴다.** `application/impulcifer_service.py`의 메서드 이름 23개, `{ok, data}` / `{ok: false, error: {code, message, details, retryable}}` 봉투, `poll_job(job_id, after_seq)` 시퀀스 커서 모델을 바꾸지 않는다. Tauri 커맨드는 `pywebview_api(method, args)` 하나이고, 초기화 스크립트가 `window.pywebview.api`를 폴리필해 `pywebviewready`를 발생시킨다. 조사 보고서가 제안한 Python 사이드카나 IPC 재설계는 채택하지 않는다.
3. **프론트엔드는 `webview_ui/`를 그대로 쓴다.** `index.html`, `app.js`, `styles.css`와 i18n 카탈로그를 수정하지 않는다. 화면 변화가 없기 때문이다.
4. **Windows 오디오는 WASAPI만 쓴다.** ASIO는 지원하지 않는다. DirectSound/MME 폴백도 없다. 백엔드는 wasapi-rs(exclusive 우선, shared+auto-convert 폴백), macOS/Linux는 cpal이다. PortAudio는 버린다.
5. **PyPI를 유지한다.** PyO3 + maturin으로 DSP 코어를 wheel로 내고 CLI 진입점을 보존한다.
6. **손수 쓰는 unsafe는 0이 기본이다.** 모든 크레이트가 `#![forbid(unsafe_code)]`로 시작한다. 예외 후보는 `impulcifer-sys-win` 하나뿐이며, 예외를 여는 것은 개별 승인 사항이다.
7. **wasapi-rs의 불건전한 안전 API(`WaveFormat::parse`, `Device::from_raw`)는 어댑터에서 쓰지 않는다.** 업스트림 이슈는 올리지 않는다. 저장소에 AI 작성 이슈를 허용하는 명시적 분위기가 없기 때문이다(README·CONTRIBUTING·이슈 템플릿에 관련 언급 없음, 2026-09-07 확인).
8. **기능 게이트.** 루트 `features.toml`에 모든 기능(IPC 메서드, 설정 필드, 파이프라인 스테이지, 출력물, 오디오 세션 동작)을 등록하고, `impulcifer-policy`의 게이트 테스트가 등록 누락과 검증 테스트 누락을 실패로 만든다. 구현된 기능에 검증 테스트가 하나라도 없으면 CI가 실패한다.
9. **작업 분배.** unsafe 단(`impulcifer-sys-win`)과 오디오 백엔드는 Daybreak Blue 워커가, 나머지 크레이트는 ASTRA 워커가 작업서 단위로 구현한다. 아키텍처·작업서·검토·게이트·커밋은 Claude가 쥔다.
10. **장치 문제는 실측한다.** `impulcifer-sys-win`의 `hardware_probe` 예제로 이 머신의 실제 장치에서 exclusive/shared, 44.1/48/96 kHz, 2/8/16채널을 재보고 결과를 문서에 남긴다.
11. **2.x는 오라클이다.** 파이썬 구현은 스테이지별 f64 골든을 내보내는 기준으로 남고, Rust 구현은 허용오차 게이트로 비교한다. 교차 언어 SHA-256 동일성은 요구하지 않는다.

## 결과

- 저장소 루트에 Cargo 워크스페이스(`Cargo.toml`, `crates/*`, `apps/impulcifer-app`)가 추가된다. Python 2.x 트리는 그대로 유지되며 릴리스 게이트의 경로 판정에 Rust 경로가 추가되기 전까지는 출하물에 영향을 주지 않는다.
- ADR 0001(네이티브 프론트엔드는 네이티브답게)은 유효하다. CTk는 2.x에 동결 상태로 남고 3.x 제품에는 포함되지 않는다.
- 이 결정을 되돌리려면 새 ADR이 필요하다.
