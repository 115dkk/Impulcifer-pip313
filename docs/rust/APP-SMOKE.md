# Tauri 앱의 로컬 M3 검사

## 실행

Windows, WebView2 런타임, Python 3.14가 필요합니다. 기본 검사는 CDP, Playwright, Pillow 없이 실행합니다. 계약 테스트는 기존 Node.js를 사용합니다. 하드웨어 검사에서는 Windows PowerShell과 기본 제공 UI Automation 어셈블리로 실제 WebView2 확인 창의 확인 버튼을 누릅니다.

모든 명령은 포그라운드로 실행합니다.

```powershell
cargo build -p impulcifer-app --release
cargo test -p impulcifer-app
py -3.14 E:/Impulcifer/tests/app_smoke/smoke.py --exe E:/Impulcifer/target/release/impulcifer-app.exe --hardware
cargo test -p impulcifer-app -- --ignored app_smoke_windows
```

`app_smoke_windows`는 기존 호출 계약 때문에 이름을 유지하지만 **기본 앱 내부 드라이버를 실행**합니다. 비하드웨어 검사이므로 CABLE-A 검사 증거를 덮어쓰지 않습니다. `--hardware`는 [HARDWARE.md](HARDWARE.md)의 `CABLE-A Input(VB-Audio Cable A)` 출력과 `CABLE-A Output(VB-Audio Cable A)` 입력만 선택합니다. 다른 장치를 대신 선택하지 않습니다.

CDP는 선택 사항입니다.

```powershell
py -3.14 -m pip install playwright
py -3.14 E:/Impulcifer/tests/app_smoke/smoke.py --exe E:/Impulcifer/target/release/impulcifer-app.exe --cdp
```

Playwright 브라우저는 다운로드하지 않습니다. `--cdp`에서는 기존 Playwright UI 검사를 사용합니다. 이 머신의 런타임에서는 아래에 기록한 CDP 장애 때문에 기본 검사를 사용해야 합니다.

## 기본 검사의 동작

- 임시 `USERPROFILE`, `HOME`, WebView2 데이터 폴더를 사용합니다. 원래 설정이나 데모 폴더는 수정하지 않습니다.
- 셸은 시작할 때 `IMPULCIFER_APP_SMOKE==1`을 한 번 확인합니다. 이때만 보고 파일, 드라이버, 매개변수, 프로필, 선택적인 CDP 포트를 읽습니다. 일반 실행에서는 `smoke_report`도 다른 메서드처럼 서비스로 전달합니다.
- 초기 스크립트 순서는 관찰기, 기존 브리지, 드라이버입니다. `IMPULCIFER_APP_SMOKE_DRIVER`는 절대경로여야 하며 시작할 때 한 번 읽습니다. `IMPULCIFER_APP_SMOKE_PARAMS` 파일은 serde_json으로 파싱한 뒤 JSON 리터럴로 주입합니다.
- `pywebview_api`는 여전히 유일한 Tauri 커맨드입니다. smoke 모드의 `smoke_report`만 셸에서 처리합니다. Mutex로 보호한 파일에 `args[0]`의 JSON과 줄바꿈을 기록하고 `{"ok":true,"data":null}`을 돌려줍니다. 인자·파일·확인 실패는 오류 봉투로 반환합니다.
- 화면 검사점의 보고에는 정수 `checkpoint`가 있습니다. 셸은 줄을 기록하고 파일 잠금을 해제한 뒤 `IMPULCIFER_APP_SMOKE_ACK_DIR/<번호>.json`을 최대 30초 기다립니다. Python은 화면과 디스크를 검사하고 임시 파일을 원자적으로 교체해 확인을 전달합니다. 따라서 한국어 설정 파일을 읽기 전에 영어로 돌아가거나 캡처 전에 다른 화면으로 이동하지 않습니다. 네트워크 요청이나 추가 IPC 메서드는 없습니다.
- Python은 NDJSON을 `done`까지 읽습니다. 전체 제한은 300초, BRIR과 각 작업의 UI 대기는 120초입니다. 실패한 단계도 진단과 화면을 남기며 독립적인 검사는 계속합니다.
- `driver.js`는 DOM의 input/change/click과 스크롤만으로 기존 화면을 조작합니다. 서비스 호출을 대신 만들거나 성공 응답을 주입하지 않습니다.
- 첫 실행 언어 창에서 영어를 선택하고 Studio를 선택합니다. 실제 bootstrap 성공, 최초 실행 플래그, 처리 화면과 활성화된 생성 버튼, `rf-share-mode`와 `auto` 옵션을 검사합니다.
- `DemoCopy`와 같은 명시적인 입력 목록만 임시 폴더에 복사합니다. `auto` 신호로 BRIR을 생성하고 완료된 체크리스트 10개, `hesuvi.wav`, 비어 있지 않은 `README.md`를 확인합니다.
- 두 번째 폴더에는 생성한 `hesuvi.wav`만 복사합니다. Studio의 계획 응답 후 복원 버튼이 활성화될 때까지 기다립니다. 복원 UI의 성공 제목, `recovery-ledger`의 `hrir.wav`가 `created`인지, 새 16채널 `hrir.wav`, 원본 WAV의 바이트 보존을 각각 검사합니다. BRIR 실패 시 가짜 복원 입력을 만들지 않습니다.
- 설정에서 한국어 `언어 선택:`을 확인합니다. Python이 실제 `.impulcifer/settings.json`의 `language=ko`를 확인한 뒤에만 영어로 돌아갑니다. 2.x 카탈로그와 3.x 오버레이를 합친 두 언어의 문자열을 저장 파일과 비교합니다.
- 정보 화면에서 업데이트 버튼을 눌러 `INTERNAL_ERROR: check_for_updates not implemented` 표시와 정확한 봉투를 비교합니다. 업데이트 설치 성공을 뜻하지 않습니다. 이 봉투를 pageerror 예외로 취급하지 않습니다.
- 하드웨어 검사에서는 실제 헤드폰 확인 창을 사용합니다. 앱 PID로 찾은 HWND의 접근성 트리에서 헤드폰 확인 문구와 확인 버튼 하나를 확인한 뒤 InvokePattern으로 누릅니다. 서비스가 만든 `headphones.wav`의 RIFF 청크를 Python이 직접 읽어 2채널, 48000Hz, 비어 있지 않은 data를 요구합니다. 음향 품질이나 골든 패리티 판정은 별도입니다.
- Win32 `EnumWindows`와 `GetWindowThreadProcessId`로 앱 창을 찾고 `PrintWindow`로 캡처합니다. ctypes의 HWND/HDC 등은 포인터 크기 시그니처를 선언합니다. GetDIBits 전에 비트맵을 DC에서 해제하고 zlib로 PNG를 기록합니다. 단색 캡처는 실패입니다.
- 관찰기는 일반 console 메시지도 기록하며 pageerror, unhandled rejection, resource-error와 서비스 오류 봉투를 구분합니다. `smoke_report` 자체는 IPC 기록에서 제외하여 재귀 기록을 막습니다. 매 검사점과 `done`에 이벤트를 보존합니다. 예상하지 못한 페이지·리소스·console 오류가 있으면 전체 결과는 실패입니다. 성공한 console.assert는 오류로 판정하지 않습니다.
- 종료 시 자신이 시작한 PID의 프로세스 트리만 종료하고 기다린 뒤 임시 폴더를 정리합니다. 검사 전후 `apps/impulcifer-app/ui/`, `i18n/`, `crates/impulcifer-service/locales/`의 파일별 SHA-256도 비교합니다.

## 셸에서 고친 결함

1. 동결된 HTML은 `frontendDist` 밖의 `logo/pulse-32.png`, `logo/pulse-128.png`를 참조합니다. 첫 실행에서 두 리소스 오류를 확인했습니다. 셸의 자산 응답 훅에서 기존 로고 파일의 내장 바이트를 같은 URL로 제공하도록 수정했습니다. HTML과 번들 설정은 바꾸지 않았습니다.
2. Tauri dialog 플러그인은 `window.confirm`을 비동기 함수로 바꿉니다. 기존 UI는 boolean을 기대하므로 Promise를 참으로 판정하여 확인 전에 녹음을 시작했습니다. 셸은 플러그인의 Rust 초기화와 파일 대화상자 API를 유지하되 JS 재정의만 제외합니다. WebView2 본래의 동기식 확인 창을 실제로 표시하고 누르는 검사가 통과했습니다.
3. 최초 구현의 로컬 HTTP 확인 요청은 WebView2에서 `Failed to fetch`로 실패했습니다. 보안 설정을 바꾸지 않고 파일 확인 방식으로 교체했습니다.

당시 동결된 UI에서는 언어를 바꾸면 runtime 표시가 연결 대기로 바뀌었고 헤드폰 확인 문구의 `{play_file}` 등 자리표시자를 채우지 않았습니다. 2026-09-12의 3.x UI 수정에서는 서비스 연결 상태를 유지하고, 스피커·헤드폰 확인 모두 실제 요청 값과 장치 접근 방식으로 문구를 채웁니다. 아래 2026-09-08 기록의 업데이트 미구현 결과는 당시 실행에 해당합니다.

## 증거 파일

실행마다 `E:/Impulcifer/target/app-smoke/{hardware,nonhardware}/<날짜-시간-PID>/`를 새로 만듭니다. 각 모드의 `latest.json`이 마지막 요약을 가리킵니다. 요청한 고정 위치 `E:/Impulcifer/target/app-smoke/summary.json`과 PNG에도 마지막 실행 결과를 복사합니다.

| 파일 | 내용 |
|---|---|
| `summary.json` | 단계별 결과·시간, 독립적인 파일 검사, 확인 창, 실행 파일 SHA-256, 전체 결과 |
| `report.ndjson`, `events.json` | 원본 드라이버 보고와 console/pageerror/resource/IPC 관찰 |
| `params.json`, `acks/` | 실행 매개변수와 원자적인 검사점 확인 파일 |
| `app.log`, `cleanup.log`, `confirmation.log` | 앱, 소유한 프로세스 종료, 실제 접근성 확인 기록 |
| `frozen-before.json`, `frozen-after.json` | 동결 대상의 파일별 해시 |
| `first-run.png`, `bootstrap.png`, `brir.png`, `recovery.png` | 첫 실행 창, 처리 준비, BRIR 완료, 복원 완료 |
| `settings-ko.png`, `settings-en.png`, `settings.png`, `updates.png` | 언어별 설정과 업데이트 오류 표시 |
| `recording-ready.png`, `recording.png` | CABLE-A 선택과 녹음 완료(하드웨어 검사만) |
| `hesuvi.wav`, `README.md`, `recovered-hrir.wav`, `headphones.wav` | Python이 확인한 실제 출력 |
| `settings-ko.json`, `settings-en.json` | 전환 시점의 실제 설정 파일 |

## 2026-09-08 두 번째 실행 결과

**기본 앱 내부 검사와 하드웨어 검사가 모두 통과했습니다.** 스크린샷을 열어 녹음 완료, 한국어 설정, 업데이트 오류 화면도 확인했습니다.

- 구현 워커가 검사한 뒤 부모 세션에서 디프를 검토하고 요청한 일곱 명령을 다시 실행했습니다. 하네스의 고정 `events.json` 저장을 보완한 뒤 하드웨어 검사와 ignored 검사도 다시 통과했습니다.
- 최종 하드웨어 증거 폴더는 `E:/Impulcifer/target/app-smoke/hardware/20260908-042812-100940/`입니다. 전체 24.161초, 스크린샷 10개입니다.
- 최종 ignored 테스트의 비하드웨어 증거 폴더는 `E:/Impulcifer/target/app-smoke/nonhardware/20260908-042842-119552/`입니다. 전체 4.431초, 스크린샷 8개입니다.
- 두 실행 모두 페이지·리소스·console 오류는 0개이며 동결 대상 22개 파일의 전후 해시가 같습니다.
- `hesuvi.wav`는 14채널, 복원 `hrir.wav`는 16채널이고 둘 다 48000Hz·24000프레임입니다. `headphones.wav`는 2채널·48000Hz·878540프레임입니다.

| 단계 | 하드웨어 결과 / 초 | 비하드웨어 결과 / 초 |
|---|---|---|
| bootstrap·영어 첫 실행·Studio | ok / 0.558 | ok / 0.536 |
| BRIR(auto) | ok / 0.793 | ok / 0.801 |
| 복원 | ok / 0.287 | ok / 0.296 |
| 한국어 저장 후 영어 복귀 | ok / 0.682 | ok / 0.673 |
| 업데이트 오류 표시 | ok / 0.058 | ok / 0.058 |
| CABLE-A 헤드폰 녹음 | ok / 19.256 | skipped / 0 |
| 종료·임시 폴더 정리 | ok / 0.256 | ok / 0.179 |

단계 시간에는 해당 단계 내부의 언어 확인 대기가 포함되며 마지막 화면 캡처 대기는 별도입니다. 전체 시간은 준비·캡처·종료를 모두 포함합니다.

부모 세션은 `E:/Impulcifer`에서 명령을 실행했습니다. 모든 원문 로그는 `E:/Impulcifer/target/app-smoke/`에 있습니다. 앞선 워커의 `second-*.log`도 보존했습니다.

| 명령 | 결과 | 부모 세션 원문 로그 |
|---|---|---|
| `cargo build -p impulcifer-app --release` | exit 0 | `parent-build.log` |
| `cargo fmt --all -- --check` | exit 1, 서비스 `src/brir/inputs.rs:207`의 포맷 차이 | `parent-fmt.log` |
| `cargo clippy -p impulcifer-app --all-targets -- --no-deps -D warnings` | exit 0 | `parent-clippy.log` |
| `cargo test -p impulcifer-app` | 6 passed, 0 failed, 1 ignored | `parent-tests.log` |
| `py -3.14 …/smoke.py --exe …/impulcifer-app.exe --hardware` | exit 0, `SMOKE_OK True` | `parent-hardware.log` |
| `cargo test -p impulcifer-app -- --ignored app_smoke_windows` | 1 passed, exit 0, `SMOKE_OK True` | `parent-ignored.log` |
| `cargo test -p impulcifer-policy` | 5 passed, 1 failed | `parent-policy.log` |

`ruff check E:/Impulcifer/tests/app_smoke --output-format=concise`와 수정한 앱 Rust 파일만 대상으로 한 `rustfmt --edition 2024 --check`는 통과했습니다. Python 구문 검사도 통과했습니다.

정책 실패는 서비스 테스트 `optional_dsp_stages_and_plot_placeholders_complete`를 등록부의 출력 기능 5개가 참조하지만 현재 이름으로 찾지 못하기 때문입니다. 허용 범위 밖이므로 수정하지 않았습니다. 중간에 서비스의 `plot_generic_room` 미정의로 앱 테스트 컴파일이 실패했지만 최종 앱 테스트와 Clippy는 통과했습니다. 부모 세션의 재검증에서도 포맷 검사와 정책 검사는 실패했습니다. 해당 파일의 담당자가 수정한 뒤 두 명령을 다시 실행해야 합니다. 부모 세션의 `py -3.14 -m ruff`는 해당 Python에 모듈이 없어 실패했지만, 설치된 `ruff check tests/app_smoke --output-format=concise`는 통과했습니다.

### CDP를 기본 검사에서 제외한 이유

첫 실행의 측정에서는 WebView2 Runtime `152.0.4191.66` 프로세스 명령행에 `--remote-debugging-port`와 임시 프로필이 있었지만 listener와 `DevToolsActivePort`가 없었습니다. 같은 머신의 Microsoft Edge 152 자체는 포트를 열었습니다. 비승격 실행에서도 WebView2만 실패했습니다. WebView2 150 이후 원격 디버깅 변경은 [WebView2Feedback 5640](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5640)에서 다룹니다. 이 문서는 특정 런타임 결함이라고 단정하지 않습니다.

이전 CDP 실패 증거는 `E:/Impulcifer/target/app-smoke/hardware/20260908-033754-314936/summary.json`, `E:/Impulcifer/target/app-smoke/nonhardware/20260908-033827-280996/summary.json`에 남아 있습니다. 이번에는 `--cdp`를 재실행하지 않았습니다. CDP가 열렸다고 보고하지 않습니다.

## 등록부에 제안할 테스트

`features.toml`은 수정하지 않았습니다.

| 기능 | 제안 테스트 |
|---|---|
| `app.pywebview_polyfill` | `impulcifer-app::bridge_script_contract` |
| smoke 보고 opt-in | `impulcifer-app::smoke_report_contract`(소스 게이트와 실제 helper의 동시 기록·전달 테스트) |
| 초기 관찰기 | `impulcifer-app::startup_observer_contract` |
| 드라이버 실패 진단 | `impulcifer-app::in_app_driver_failure_contract` |
| 동기식 확인 창 | `impulcifer-app::native_confirmation_contract`와 하드웨어 실기 증거 |
| `app.smoke_brir`, `app.smoke_recovery`, `app.smoke_settings` | `impulcifer-app::app_smoke_windows` |
| `app.smoke_recording` | `smoke.py --hardware` 실기 증거. Rust ignored hook은 비하드웨어이므로 이 hook만으로 녹음 등록을 대신하면 안 됩니다. |

## API 참고

- [Tauri WebviewWindowBuilder](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)
- [Win32 PrintWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-printwindow)
- [Win32 GetDIBits](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-getdibits)
- [Playwright WebView2](https://playwright.dev/python/docs/webview2)
