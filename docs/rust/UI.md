# 3.x 앱 화면

## 파일과 문자열

3.x 화면은 `apps/impulcifer-app/ui/`에 있습니다. `index.html`, `app.js`, `styles.css`와 로고·글꼴은 Tauri의 `frontendDist` 안에서 제공하며, 프레임워크나 번들러는 사용하지 않습니다. 글꼴은 기존 Pretendard와 JetBrains Mono를 그대로 쓰고 라이선스도 함께 포함합니다.

`webview_ui/`와 `i18n/locales/`는 2.x 파일입니다. 3.x 변경에서는 수정하지 않습니다. 새 문자열과 기존 문구의 수정은 `crates/impulcifer-service/locales/<언어>.json` 아홉 파일에 넣습니다. 서비스와 화면 검사 도구는 2.x 영어, 2.x 해당 언어, 오버레이 영어, 오버레이 해당 언어 순서로 합칩니다. 같은 키가 있으면 뒤의 값이 이깁니다.

모든 오버레이는 키 집합과 `{placeholder}` 집합이 같아야 합니다. 키를 바꾸지 않고 문구만 번역하며, 페이지에서는 `t()`와 `fmt()`로 표시합니다. 실행 정보의 선택적인 값이 없으면 해당 행은 표시하지 않습니다. 설정의 폴더 표시는 `get_system_info().paths`를 사용하고, 설정 파일의 열기 버튼은 부모 폴더를 `open_path`에 전달합니다.

## 타입을 검사하는 IPC

`apps/impulcifer-app/ui/ipc.d.ts`가 24개 IPC 메서드의 위치 인자, 요청, 응답, 작업 스냅샷과 이벤트를 선언합니다. `Envelope<T>`는 `ok`로 구분하는 합집합입니다. `ok`를 확인한 뒤 성공의 `data` 또는 실패의 `error`를 사용합니다. 작업은 `kind`, 이벤트는 실제 전송 필드인 `type`으로 구분합니다. 초기 스크립트와 스모크 드라이버가 쓰는 `Window` 속성도 이 파일에서 선언합니다.

JS에서는 JSDoc으로 선언을 참조합니다. `$()`는 필수 HTML 요소를 찾고, `el(id, HTMLInputElement)` 같은 호출은 실제 DOM 생성자를 검사해 정확한 요소 타입을 반환합니다. `unknown`은 확인하지 않은 전송 값에만 사용합니다. `any`, 타입 검사 생략 주석, 느슨한 컴파일러 설정은 사용하지 않습니다.

IPC 메서드를 추가할 때는 다음 항목을 함께 수정합니다.

1. Rust의 `IpcMethod`, 서비스 구현, 정책 목록, `features.toml`에 메서드를 등록하고 검증 테스트를 연결합니다.
2. `ipc.d.ts`에 요청·응답 구조와 `IpcApi`의 위치 인자 시그니처를 적습니다. 브리지 프록시는 같은 이름을 전달하므로 메서드별 분기를 만들지 않습니다.
3. 페이지에서 실패 봉투를 처리하고, 필요한 문자열을 아홉 오버레이에 추가합니다.
4. 갤러리의 모의 IPC와 필요한 스모크 검사를 갱신한 뒤 타입·계약·카탈로그 검사를 실행합니다.

## Studio와 Stable

두 스킨은 Pulse Studio의 색상 토큰과 카드·필드·칩을 공유하며 밝은 테마와 어두운 테마를 지원합니다. Studio는 사이드바와 작업 진행 카드를 사용합니다. Stable은 상단 탭과 기존 작업 모달을 유지합니다. 스킨·테마·언어 선택의 ID는 `sf-skin`, `sf-theme`, `sf-language`입니다. 3.x에는 CustomTkinter 선택 항목이 없습니다.

녹음 화면의 `rf-share-mode`는 bootstrap의 `capabilities.share_modes`로 채웁니다. `auto`만 있으면 선택을 비활성화하고 Windows WASAPI에서만 고정 모드를 지원한다는 안내를 표시합니다. `rf-host-api`는 요청에서 유지하되 호스트 API가 하나 이하이면 행을 숨깁니다. 고정 모드를 거부한 오류는 상태와 로그에 표시하고 Stable에서는 알림도 띄웁니다. 성공하면 실제 출력·입력 모드를 표시합니다.

## Studio 복원 목록

`recovery-inventory`는 폴더와 두 옵션을 마지막으로 바꾼 뒤 300ms 후 `plan_output_recovery`를 호출합니다. 각 요청에 증가하는 번호를 부여하고 오래된 응답은 무시합니다. 같은 값의 `input`과 `change`는 중복 요청하지 않습니다. 다시 확인하는 동안 기존 목록의 높이는 유지하되 내용은 숨겨, 포커스 이동 중 아래 체크박스가 움직이지 않도록 합니다.

| `data-state` | 표시와 실행 버튼 |
|---|---|
| `empty` | 폴더 선택 안내, 실행 불가 |
| `planning` | 파일 확인 중 안내, 실행 불가 |
| `ready` | 원본·샘플레이트·길이·스피커·파일 목록, 실행 가능 |
| `nothing` | 요청한 파일이 모두 있다는 안내, 실행 불가 |
| `error` | 오류 코드와 메시지, 오류색 테두리, 실행 불가 |

`recovery-ledger`는 `existing_files`와 `planned_files`의 실제 파일만 표시합니다. 상대 파일명과 `present`, `planned`, `created` 상태를 각각 `data-file`, `data-status`에 기록합니다. 기존 파일은 그대로 유지하고, 실행으로 만든 파일의 상태만 `created`로 바꿉니다. 실행 도중 폴더나 옵션을 바꾸었다면 이전 실행 결과를 새 목록에 적용하지 않습니다.

세 카드와 기존 결과 ID는 유지합니다. 성공 시 요약과 출력 폴더 열기 버튼을 표시합니다. Stable은 목록을 숨기고 계획 IPC를 호출하지 않습니다. Stable의 복원 버튼은 계획 상태 때문에 비활성화하지 않으며 입력 검증은 서비스가 담당합니다. 두 스킨 모두 다른 작업이 실행 중이면 새 작업을 시작하지 못합니다.

## 검증과 갤러리

저장소 루트에서 실행합니다. 모든 명령은 포그라운드로 실행합니다.

```powershell
npm --prefix apps/impulcifer-app run typecheck
npm --prefix apps/impulcifer-app run test:contracts
cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-policy -- app_scripts_opt_into_type_checking
cargo test -p impulcifer-app --test smoke
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out target/ui-gallery
```

갤러리는 설치된 Playwright Chromium으로 모의 IPC 응답을 주입합니다. 실제 앱, 장치 녹음, 업데이트 실행은 하지 않습니다. 시나리오마다 초기 스크립트 하나를 사용하고, 페이지·콘솔·리소스 오류가 있거나 PNG 수가 64개와 다르면 실패합니다.

- 두 스킨 × 두 테마 × 영어·한국어 × 설정·정보·녹음 화면 24장
- Studio 복원의 다섯 계획 상태와 실행 성공 × 두 테마 × 두 언어 24장
- 녹음 모드 비활성화·고정 모드 거부 × 두 스킨 × 두 테마 × 두 언어 16장

추가 동작 검사에서는 300ms 지연 호출, 오래된 응답 무시, 옵션 변경 후 재확인, Stable에서 계획 미호출, 설정 부모 폴더 열기, 언어 변경, 녹음 확인의 자리표시자, 접근 모드 성공·오류 표시를 확인합니다. 실제 앱 검사는 `docs/rust/APP-SMOKE.md`를 따릅니다.
