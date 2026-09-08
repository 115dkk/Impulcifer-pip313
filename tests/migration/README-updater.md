# P20 업데이터 검증

2026-09-08, Windows에서 검증했습니다. 네트워크 요청을 수행하는 일반 테스트는 로컬 TCP 서버만 사용합니다. 실제 배포 피드에 접속하는 테스트는 `#[ignore]`이며 설치된 앱과 `IMPULCIFER_LIVE_UPDATE_TEST=1`이 필요합니다.

## 구현 파일

- `crates/impulcifer-service/src/update/{mod,check,install_kind,legacy,velopack}.rs`
- `crates/impulcifer-service/src/lib.rs`
- `crates/impulcifer-service/Cargo.toml`
- `crates/impulcifer-service/tests/{updater,ipc}.rs`
- `apps/impulcifer-app/src/{main,updater}.rs`
- `apps/impulcifer-app/Cargo.toml`
- `apps/impulcifer-app/tauri.conf.json`의 `plugins.updater`
- `apps/impulcifer-app/capabilities/default.json`
- `apps/impulcifer-app/tests/smoke.rs`
- `Cargo.lock`
- `tests/migration/export_goldens_updater.py`
- `tests/migration/goldens/p20_updater.json`
- 이 문서

다른 워커가 수정 중인 오디오 크레이트와 성능 감사 파일은 P20 변경 목록에 포함하지 않습니다. `features.toml`, 동결된 프론트엔드와 i18n 카탈로그도 수정하지 않았습니다.

## 의존성

| 직접 추가한 의존성 | lockfile 버전 | 라이선스 |
|---|---|---|
| velopack | 1.2.0 | MIT |
| tauri-plugin-updater | 2.11.0 | Apache-2.0 OR MIT |
| ureq | 3.4.1 | MIT OR Apache-2.0 |
| pep440_rs | 0.7.3 | Apache-2.0 OR BSD-2-Clause (Apache-2.0 선택 가능) |
| sha2 | 0.10.9 | MIT OR Apache-2.0 |
| tempfile | 3.27.0 | MIT OR Apache-2.0 |
| url | 2.5.8 | MIT OR Apache-2.0 |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 |

구현 워커는 docs.rs에서 SDK API를, crates.io에서 버전과 라이선스를 확인했습니다. `velopack`은 Windows 대상 의존성에만, `tauri-plugin-updater`는 앱 크레이트에만 추가했습니다. 직접 지정한 `ureq`는 기본 기능을 끄고 `rustls`만 켭니다. Windows에서는 Velopack의 의존성 때문에 gzip 등의 ureq 기본 기능도 합쳐지지만, 확인한 ureq 기능 목록에 native-TLS/OpenSSL은 없습니다. Windows 앱은 Tauri 업데이터 플러그인을 등록하지 않습니다.

- [Velopack API](https://docs.rs/velopack/1.2.0/velopack/)
- [Tauri updater API](https://docs.rs/tauri-plugin-updater/2.11.0/tauri_plugin_updater/)
- [ureq API](https://docs.rs/ureq/3.4.1/ureq/)

## IPC 계약

각 응답에는 기존 공통 성공/실패 envelope가 적용됩니다.

| IPC | 2.x의 필드 | Rust의 필드 |
|---|---|---|
| `check_for_updates` | `update_available`, `current_version`, `latest_version`, `download_url`, `release_notes`, `release_url` | 동일 |
| `start_update` 즉시 응답 | `job` (`job_id`, `kind`, `status`, `cancellable`, `result`, `error`) | 동일, `kind=update`, `cancellable=false` |
| 업데이트 작업의 성공 결과 | `status_key`, `status_default`, `title_key`, `title_default`, `message_key`, `message_default`, `progress`, `requires_restart` | 동일 |
| 진행 이벤트의 내용 | `progress`, `message` | 동일 |
| `apply_pending_update` | `restarting` | 동일 |

| 설치 종류 | 실행 방식 | 재시작 필요 |
|---|---|---|
| Windows `velopack` | SDK로 확인·다운로드한 뒤 apply 요청 때 SDK에 설치와 재시작을 맡김 | true |
| macOS/Linux `tauri` | HostAdapter가 플러그인으로 확인·다운로드하고 apply 요청 때 설치·재시작 | true |
| `dev` 및 미일치 번들 | 임시 디렉터리에 다운로드하고 SHA256SUMS 검증 후 host로 열기. AppImage 교체 및 실패 시 다운로드 파일 열기도 지원 | false |

다운로드 검증 파일이 404이면 2.x와 같이 다운로드를 허용합니다. 검증 파일이 존재하지만 파일명이 없거나 해시가 다르면 실행하지 않습니다. 다른 HTTP 오류도 실패로 처리합니다.

작업서의 `update_downloaded`는 실제 2.x 실행 코드와 카탈로그에 없으므로 추가하지 않았습니다. 기존 `update_downloading`, `update_installing`, `update_opening_installer` 및 `Downloading: NN%` 문구를 사용합니다. 요청 검증에는 2.x의 문자열 변환 동작도 반영했습니다.

다운로드가 실패하면 직전 성공 작업의 staged action을 유지합니다. apply 실행 시 action을 소비하므로 적용에 실패한 뒤 다시 시도하려면 다운로드 작업부터 실행해야 합니다. 이 동작은 테스트로 고정했습니다.

## 골든 생성

```sh
py -3.14 E:/Impulcifer/tests/migration/export_goldens_updater.py
```

2.x `UpdateChecker`를 사용해 30개 릴리스, 2개 Windows 설치 디렉터리 구성, 13개 요청 검증 envelope, 2개 실행 결과를 생성합니다. macOS 번들과 Linux APPIMAGE 판정은 Rust 테스트에서 별도로 검증합니다. 테스트에서는 프로세스 환경을 변경하지 않고 설치 판정 함수와 `UpdateOptions`에 입력을 전달합니다.

Rust 3.x는 정규화한 기본 버전이 같으면 PEP 440의 사전 출시 부분만 비교합니다. 정식 버전은 모든 사전 출시 버전보다 나중이며, post/dev/local 부분은 비교하지 않습니다. 기본 버전이 다르거나 전체 문자열을 PEP 440 버전으로 해석할 수 없으면 2.x 판정 방식을 유지합니다. 이 규칙 때문에 골든 30쌍 중 `3.0.0-alpha.0 -> v3.0.0-rc1`만 2.x의 `false`와 달리 `true`입니다. 골든 JSON은 2.x 결과 그대로 두고 Rust 테스트에서 이 차이를 명시합니다.

## 최종 검증 출력

아래는 구현 워커의 보고 후 부모 세션이 일곱 명령을 모두 포그라운드로 다시 실행한 출력입니다. 모든 명령의 종료 코드는 0입니다. 구현 워커가 앞서 보고했던 다른 워커의 포맷 문제는 이 재실행 시점에는 없었습니다.

원본 로그 디렉터리는 `C:/Users/32170336/AppData/Local/Temp/impulcifer-p20-parent-gates-xkgmjnaf/`이며, 명령 순서대로 `1.log`부터 `7.log`까지 저장했습니다.

### 1. Python 골든

```text
P20 offline goldens: 30 releases, 2 layouts, 13 requests, 2 executor results
Wrote E:/Impulcifer/tests/migration/goldens/p20_updater.json
```

### 2. `cargo fmt --all -- --check`

출력 없이 종료 코드 0으로 끝났습니다.

### 3. `cargo clippy -p impulcifer-service -p impulcifer-app --all-targets -- --no-deps -D warnings`

```text
    Checking impulcifer-service v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-service)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.99s
```

### 4. `cargo test -p impulcifer-service --test updater --test ipc`

```text
   Compiling impulcifer-service v3.0.0-alpha.0 (E:\Impulcifer\crates\impulcifer-service)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.29s
     Running tests\ipc.rs (target\debug\deps\ipc-836f3da690b969f4.exe)

running 23 tests
test argument_types_defaults_null_and_unknown_methods ... ok
test ipc_get_system_info_shape ... ok
test ipc_cancel_job_shape ... ok
test ipc_select_file_shape ... ok
test invalid_sweeps_and_naming_edges ... ok
test ipc_select_directory_shape ... ok
test ipc_list_audio_devices_shape ... ok
test ipc_open_url_shape ... ok
test ipc_resolve_recording_paths_shape ... ok
test ipc_open_path_shape ... ok
test ipc_poll_job_shape ... ok
test resolve_recording_paths_matches_python_examples ... ok
test backend_errors_panics_and_empty_enumeration ... ok
test host_failures_and_panics_do_not_escape_boundary ... ok
test ipc_bootstrap_shape ... ok
test ipc_get_ui_settings_shape ... ok
test ipc_updater_error_and_job_shapes ... ok
test ipc_set_frontend_shape ... ok
test ipc_set_skin_shape ... ok
test ipc_set_theme_shape ... ok
test settings_persist_preserve_other_keys_and_reload_python_preferences ... ok
test missing_or_invalid_settings_and_write_failures_keep_python_first_run_semantics ... ok
test ipc_set_language_shape ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests\updater.rs (target\debug\deps\updater-8fa86da58bf7edf3.exe)

running 18 tests
test no_update_hides_download_and_notes ... ok
test velopack_check_against_live_feed ... ignored, requires installed Velopack build and IMPULCIFER_LIVE_UPDATE_TEST=1
test golden_asset_selection_and_version_compare_match_python ... ok
test apply_pending_update_without_stage_is_invalid_request ... ok
test updates_are_non_cancellable_and_busy_jobs_are_rejected ... ok
test failed_download_preserves_previous_stage_and_apply_failure_consumes_it ... ok
test latest_successful_update_replaces_stage ... ok
test velopack_kind_selected_on_windows_with_update_exe ... ok
test golden_install_kind_matches_python ... ok
test start_update_validation_matches_python ... ok
test tauri_kind_delegates_to_host ... ok
test check_for_updates_reads_github_latest_from_mock_server ... ok
test installer_http_failure_never_opens_or_stages ... ok
test legacy_appimage_replaces_running_file_and_falls_back_when_missing ... ok
test legacy_executor_downloads_verifies_and_opens ... ok
test check_failures_and_timeout_are_retryable ... ok
test checksum_404_preserves_legacy_compatibility_and_open_failure_is_reported ... ok
test checksum_failures_discard_download_and_never_open ... ok

test result: ok. 17 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 8.69s
```

### 5. `cargo test -p impulcifer-app`

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.64s
     Running unittests src\main.rs (target\debug\deps\impulcifer_app-cd40f8dfda0fe05b.exe)

running 2 tests
test dialog::tests::native_confirmation_contract ... ok
test smoke::tests::smoke_report_contract ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\smoke.rs (target\debug\deps\smoke-e67947657cdbb5cd.exe)

running 7 tests
test app_smoke_hardware_windows ... ignored, local M3 hardware: CABLE-A virtual cable pair; see docs/rust/APP-SMOKE.md
test app_smoke_windows ... ignored, local M3: release app + WebView2 + Python 3.14; see docs/rust/APP-SMOKE.md
test smoke_report_contract ... ok
test updater_plugin_registered_with_public_key ... ok
test startup_observer_contract ... ok
test in_app_driver_failure_contract ... ok
test bridge_script_contract ... ok

test result: ok. 5 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

### 6. `cargo build -p impulcifer-app --release`

```text
    Finished `release` profile [optimized] target(s) in 0.65s
```

### 7. `cargo test -p impulcifer-policy`

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.19s
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

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s

   Doc-tests impulcifer_policy

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## 패키징에서 수행할 일

### Windows Velopack

- 기존 2.x와 동일한 피드 `https://github.com/115dkk/Impulcifer-pip313/releases/latest/download`를 사용합니다.
- 채널은 `win`, manifest 이름은 `releases.win.json`입니다.
- 실제 설치 디렉터리에서 SDK의 설치 정보 판독, 패키지 호환성, 다운로드, 적용과 재시작을 검증해야 합니다.
- `velopack_check_against_live_feed`는 설치된 빌드에서 명시적으로 실행해야 합니다. 이번 로컬 검증에서는 실행하지 않았습니다.

### macOS/Linux Tauri

- 릴리스 자산으로 서명한 플랫폼별 업데이트 파일과 `latest.json`을 발행해야 합니다.
- 설정한 endpoint는 `https://github.com/115dkk/Impulcifer-pip313/releases/latest/download/latest.json`입니다.
- CI secret 이름은 `TAURI_SIGNING_PRIVATE_KEY`입니다. 개인 키를 로그·저장소·릴리스 자산에 포함하면 안 됩니다.
- Tauri CLI로 키 쌍을 OS 임시 디렉터리에 생성했습니다. 개인 키의 위치는 `C:/Users/32170336/AppData/Local/Temp/impulcifer-p20-signing-6mu4rvjg/updater.key`입니다. 개인 키 내용은 이 문서에 기록하지 않습니다. 이 파일은 임시 저장물이므로 패키징 담당자가 안전하게 보관하거나 새 키로 교체해야 합니다. 교체하면 설정의 공개 키도 함께 바꿔야 합니다.
- `tauri.conf.json`에는 공개 키만 추가했습니다.
- macOS/Linux의 실제 번들 설치와 재시작은 아직 검증하지 않았습니다. 이번 Windows 빌드는 호스트 어댑터의 Rust API 타입 검증까지 수행합니다.

## features.toml 등록 제안

이번 작업에서는 등록부를 수정하지 않았습니다.

| 등록 항목 | 존재하는 검증 테스트 |
|---|---|
| `ipc.check_for_updates` | `impulcifer-service::check_for_updates_reads_github_latest_from_mock_server`, `impulcifer-service::check_failures_and_timeout_are_retryable` |
| `ipc.start_update` | `impulcifer-service::start_update_validation_matches_python`, `impulcifer-service::ipc_updater_error_and_job_shapes` |
| `ipc.apply_pending_update` | `impulcifer-service::apply_pending_update_without_stage_is_invalid_request`, `impulcifer-service::tauri_kind_delegates_to_host` |
| `updater.install_kind` | `impulcifer-service::golden_install_kind_matches_python` |
| `updater.velopack` | `impulcifer-service::velopack_kind_selected_on_windows_with_update_exe` (설치 판정만 검증하므로 실제 SDK 검증을 대체하지 않음) |
| `updater.tauri` | `impulcifer-service::tauri_kind_delegates_to_host`, `impulcifer-app::updater_plugin_registered_with_public_key` |
| `updater.legacy` | `impulcifer-service::legacy_executor_downloads_verifies_and_opens`, `impulcifer-service::checksum_failures_discard_download_and_never_open`, `impulcifer-service::legacy_appimage_replaces_running_file_and_falls_back_when_missing` |

P20은 로컬 구현·계약 검증을 마쳤습니다. 실제 설치 업데이트 검증, 피드 발행, CI secret 등록과 성능 감사 완료를 주장하지 않습니다. 커밋·푸시·CI 실행도 하지 않았습니다.
