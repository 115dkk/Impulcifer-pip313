# ADR 0003: 3.x 프론트엔드는 `apps/impulcifer-app/ui/`에서 따로 자란다

- **상태:** 결정됨 (2026-09-12)
- **결정자:** 유지보수자 (115dkk)
- **선행:** ADR 0002 3항("프론트엔드는 `webview_ui/`를 그대로 쓴다")

## 맥락

ADR 0002는 재작성 기간 동안 화면을 바꾸지 않기 위해 `webview_ui/`와 i18n 카탈로그를 동결했다. 첫 사전 출시(v3.0.0-alpha.0) 뒤 유지보수자가 알파를 벗어나기 전에 화면을 러스트판의 실정에 맞게 고치라고 지시했다. CTk를 고르는 설정 항목을 없애고, Python을 가리키는 표기를 걷어내고, 녹음 장치 접근 방식(독점·공유)을 직접 고르게 하고, Studio 스킨의 출력 복원 화면을 다시 만들고, 바닐라 JS에 타입 검사를 붙이는 일이다. 이 모두는 2.x가 쓰는 파일을 건드리면 2.x 출하물이 같이 바뀐다. 2.x는 오라클이자 기존 사용자의 안정판이므로 그대로 두어야 한다.

## 결정

1. **프론트엔드를 분기한다.** `webview_ui/`(index.html, app.js, styles.css)와 로고 두 장을 `apps/impulcifer-app/ui/`로 복사했고, Tauri의 `frontendDist`는 이 폴더를 가리킨다. 이후 3.x 화면 작업은 이 폴더에서만 한다. `webview_ui/`와 `i18n/locales/`는 2.x 소유로 남고 3.x 작업에서 수정하지 않는다. 릴리스 게이트(`release_gate.py`)는 `apps/*`를 제외하므로 3.x 화면 변경이 2.x 발행을 일으키지 않는다.
2. **3.x 문자열은 오버레이 카탈로그에 둔다.** `crates/impulcifer-service/locales/<언어>.json` 아홉 파일이 2.x 카탈로그 위에 겹쳐진다(합치는 순서: 2.x en, 2.x 해당 언어, 오버레이 en, 오버레이 해당 언어). 같은 키가 있으면 오버레이가 이긴다. 아홉 파일은 키 집합과 자리표시자 집합이 같아야 하며(`overlay_catalogues_share_one_key_set_and_placeholders`), 3.x 화면이 요구하는 모든 키는 모든 언어에서 풀려야 한다(`ui_keys_resolve_in_every_language`). 예전의 `EXTRA_STRINGS` 상수는 이 파일들로 옮겼다.
3. **IPC 표면은 3.x가 소유한다.** 커맨드는 여전히 `pywebview_api(method, args)` 하나이고 봉투 형식과 잡 모델은 2.x 그대로다. 다만 메서드 집합과 응답 모양은 이제 3.x 화면의 필요에 따라 더할 수 있다(예: `plan_output_recovery`, `get_system_info`의 러스트판 모양). 새 메서드는 `IpcMethod`, 정책 게이트의 정본 목록, `features.toml`에 함께 등록한다.
4. **바닐라 JS에 타입 검사를 건다.** 페이지 스크립트, 초기화 스크립트, 스모크 드라이버는 `// @ts-check`로 시작하고 `apps/impulcifer-app/tsconfig.json`(`checkJs`, `strict`)이 전부를 포함한다. IPC 표면은 `ui/ipc.d.ts`에 선언해 호출을 검사한다. `rust.yml`의 `js` 잡이 고정된 TypeScript로 `tsc`와 노드 계약 테스트를 돌리고, 정책 게이트(`app_scripts_opt_into_type_checking`)가 옵트인 누락을 실패로 만든다. 번들러나 TypeScript 소스 파일은 도입하지 않는다.
5. **Windows 릴리스 빌드는 GUI 서브시스템이다.** `main.rs`의 `windows_subsystem = "windows"` 속성으로 콘솔 창을 붙이지 않는다. 릴리스 워크플로가 패킹된 exe의 PE 서브시스템을 검사한다.

## 결과

- ADR 0002 3항은 이 결정으로 대체된다. 2항(IPC 형식)과 나머지 항은 유효하다.
- 2.x의 `webview_ui/`는 동결 상태로 남고 2.x 릴리스에만 들어간다. 두 프론트엔드 사이의 동기화는 하지 않는다.
- 2.x 카탈로그에 키를 더하는 일은 2.x 작업이다. 3.x 문자열은 오버레이에만 쓴다.
- 이 결정을 되돌리려면 새 ADR이 필요하다.
