# Rust로 짤 때 unsafe는 피할 수 없는가, 피할 수 없다면 어떻게 가두는가 (2026-09-07)

ASTRA 워커 2기의 보고서(`report-10-rust-unsafe-inventory.md` 인벤토리, `report-11-rust-unsafe-quarantine.md` 격리 아키텍처)를 Claude가 읽고 종합한 문서입니다. 대상 스택은 앞선 결정대로 Rust 코어 + Tauri 2, Windows는 wasapi-rs, macOS/Linux는 CPAL입니다. 표지는 [V] 검증(출처 또는 소스 확인), [I] 추론, [U] 미확인입니다. 두 보고서 모두 실제 Rust 워크스페이스를 빌드하지는 않았습니다.

## 1. 결론

1. **[I] 우리가 손수 쓰는 unsafe는 0으로 갈 수 있습니다.** 필요한 모든 기능에 안전한 호출 표면이 있습니다. WASAPI 열거·렌더·캡처(wasapi 0.24.0), CoreAudio/ALSA(cpal 0.18.2), FFT(RustFFT 6.4.1/RealFFT 3.5.0), 배열(ndarray 0.17.2/nalgebra 0.35.0), 병렬(rayon 1.11), WAV(hound 3.5.1 또는 자체 안전 writer), ffmpeg 실행(std::process), Tauri 명령·테마·다이얼로그·업데이터, PyO3 0.27/0.29 + numpy(소유 복사 경로), Velopack 1.2.0 전부입니다. 따라서 **애플리케이션 크레이트 전부에 `#![forbid(unsafe_code)]`**를 걸 수 있습니다.
2. **[V] 그러나 "실행 파일에 unsafe가 없다"는 뜻은 아닙니다.** wasapi-rs `api.rs` 한 파일에만 unsafe 블록이 99개 있고(소스 텍스트 집계), PyO3 매크로는 `unsafe extern "C"` 내보내기를 생성하며, Tauri 자체(`app.rs`)와 wry/tao, RustFFT의 SIMD 커널, rayon 내부가 모두 unsafe를 씁니다. 우리가 통제하는 것은 손수 쓰는 unsafe이고, 나머지는 검토·고정·감사 대상입니다.
3. **[V] 진짜 위험은 lint가 잡지 못하는 곳, 즉 '안전해 보이는 함수의 불건전성'에 있습니다.** 인벤토리가 두 건을 소스에서 찾아냈고 Claude가 원본으로 재확인했습니다(2절).
4. **[I] 격리 방식은 Tauri와 같습니다.** 플랫폼 unsafe는 leaf 크레이트 하나(`impulcifer-sys-win`)에만 허용하되, 초기 허용량은 0이고, wasapi-rs가 못 하는 네이티브 호출이 실제로 증명될 때만 개별 승인으로 열어 줍니다. 그 크레이트의 공개 API에는 포인터·COM 인터페이스·unsafe 트레이트가 하나도 나오지 않습니다.
5. **[I] "Tauri처럼 가둔다"는 검토와 소유권의 경계이지 샌드박스가 아닙니다.** 크레이트 경계는 메모리 격리를 주지 않습니다. 의존성 안의 unsafe가 불건전하면 바깥의 안전한 크레이트가 그것을 고칠 수 없습니다. 크래시 격리가 필요하면 프로세스 경계를 따로 설계해야 합니다.

## 2. lint가 못 잡는 위험: 소스에서 확인된 불건전성

| 대상 | 내용 | 확인 | 대응 |
|---|---|---|---|
| wasapi-rs 0.24.0 `WaveFormat::parse(&WAVEFORMATEX)` | `pub fn`(안전 시그니처)인데 `wFormatTag`가 EXTENSIBLE이고 `cbSize`가 충분하면 `&WAVEFORMATEX`를 `*const WAVEFORMATEXTENSIBLE`로 캐스팅해 `ptr::read`합니다. 호출자가 단독 `WAVEFORMATEX` 값을 넘기면 할당 바깥을 읽습니다. 헤더 필드가 뒤에 저장 공간이 있음을 증명하지 못하므로 불건전한 안전 API입니다 | [V] Claude가 `waveformat.rs` 원본으로 재확인 | 어댑터에서 이 함수 사용 금지. 길이 검사가 있는 `parse_from_blob_bytes`, 검증된 `WaveFormat::new`만 사용. 업스트림에 시그니처 변경(unsafe 또는 길이 제한 입력) 요청 |
| wasapi-rs 0.24.0 캡처 `read_from_device` 두 변형 | `GetBuffer` 뒤 `AUDCLNT_BUFFERFLAGS_SILENT`를 확인하기 전에 반환 포인터로 슬라이스를 만들어 복사하고, 플래그는 복사 뒤 `BufferInfo`로만 돌려줍니다. Microsoft는 SILENT일 때 데이터 값을 무시하라고 합니다 | [V] `api.rs` 원본으로 재확인. [U] Windows가 SILENT에서 null을 돌려준다는 문서는 없어 UB 여부는 조건부 | 신호 의미로는 우리 쪽에서 SILENT 패킷을 0으로 채워야 하고, 메모리 의미로는 업스트림이 슬라이스 구성 전에 분기해야 합니다. 채택 전 재현 조사 필요 |
| coreaudio-rs 0.14.2 `Handle::from_ptr` | 안전 함수가 임의 포인터를 저장하고 안전 `get()`이 검사 없이 역참조합니다. `from_ptr(null_mut()).get()`이 전부 안전한 Rust로 표현됩니다 | [V] 보고서 10의 소스 진단. [U] 일반 CPAL 빌더 경유로 도달하는지는 미확인 | CPAL의 타입 빌더만 쓰고 coreaudio 원시 헬퍼를 직접 쓰지 않음 |
| Tauri `Manager::unmanage` | deprecated 안전 함수인데 문서 스스로 dangling reference를 경고 | [V] | 사용 금지, `Mutex<Option<T>>`로 대체 |

이 표가 이 조사의 가장 중요한 교훈입니다. `#![forbid(unsafe_code)]`는 우리가 unsafe를 쓰지 않았다는 것만 보증하고, 우리가 부르는 안전한 함수가 건전한지는 보증하지 않습니다. 그래서 의존성의 unsafe와 그 불변식을 지키는 안전 코드까지 검토 대상에 넣어야 합니다.

## 3. unsafe가 새어들 수 있는 지점과 안전한 대안

| 새어드는 경로 | 이유 | 대안 |
|---|---|---|
| windows-rs로 COM/DWM/MMCSS 직접 호출 | 모든 COM 메서드가 `unsafe fn` | wasapi-rs 안전 표면 우선, Tauri `set_theme`으로 다크 타이틀바(tao가 내부에서 DwmSetWindowAttribute 호출), 부족하면 업스트림 수정, 그래도 안 되면 leaf 예외 |
| COM 래퍼에 `unsafe impl Send/Sync` | 아파트먼트 규칙이 스레드 이동을 금지 | 오디오 전용 OS 스레드에서 생성·사용·해제, 스레드 간에는 ID·소유 버퍼·결과만 전달 |
| 위 표의 불건전 안전 API 사용 | lint가 못 잡음 | 어댑터에서 명시적 금지 목록 유지 |
| memmap2 파일 매핑 | 외부 파일 변경·절단이 메모리를 무효화하므로 `Mmap::map`이 unsafe | 수백 MB는 `std::fs::read`나 스트리밍 읽기로 충분 |
| NumPy 원시/미초기화/차용 배열(`PyArray::new`, `borrow_from_array`, `PyArrayMethods::as_slice`) | 초기화·별칭·수명 의무 | `PyReadonlyArray` 가드 → 복사 → `Python::detach`로 소유 데이터만 계산 → `from_vec`/`from_slice`로 새 배열 반환 |
| 포인터 SIMD, `get_unchecked`, `assume_init`, `set_len` | 경계·정렬·CPU 기능·초기화 | `Vec<f64>`와 안전 슬라이스, RustFFT 플래너의 런타임 디스패치 재사용, 측정된 병목에만 `wide`/`pulp` 안전 API |
| 링버퍼 원시 커밋 API | 초기화된 슬롯과 공개 순서 증명 필요 | rtrb 0.3.5 이상 또는 ringbuf 0.4.8의 안전 push/pop만 |
| `static mut`, 전역 환경변수 변경(2024 에디션에서 `set_var`가 unsafe), Unix `pre_exec` | 전역 별칭·프로세스 런타임 계약 | OnceLock/Arc/Mutex/원자값, `Command::env` |
| 공유 메모리 IPC, 원시 콜백 | 수명·피어 변경·FFI unwind | Tauri JSON/바이너리 응답, stdio |

## 4. 격리 아키텍처

### 4.1 워크스페이스와 unsafe 예산

| 크레이트 | 책임 | 손수 쓰는 unsafe 예산 |
|---|---|---|
| `impulcifer-types` | 검증된 설정·채널 레이아웃·오류 타입 | 0, forbid |
| `impulcifer-dsp` | f64 scipy 호환 프리미티브, RustFFT, ndarray/nalgebra, rayon | 0, forbid. 자체 SIMD 금지 |
| `impulcifer-io` | WAV/CSV/TXT, ffmpeg 프로세스, 안전 바이트 파싱 | 0, forbid |
| `impulcifer-analysis` | 분석 배열, plotters PNG, 오프라인 Plotly | 0, forbid |
| `impulcifer-audio-io` | 백엔드 선택, 측정 세션 오케스트레이션, macOS/Linux는 cpal | 0, forbid |
| `impulcifer-sys-win` | Windows 엔드포인트·세션 구현(wasapi-rs), 승인된 windows-rs 직접 호출 | **유일한 예외 후보**. 초기 허용 0, 항목별 승인, 검토 트립와이어 12블록/120줄 |
| `impulcifer-jobs` | seq 저널, 취소 토큰, 유계 큐 | 0, forbid |
| `impulcifer-service` | JSON 안전 애플리케이션 연산 | 0, forbid |
| `impulcifer-app` | Tauri 명령, 다이얼로그/테마/opener, 업데이터 어댑터 | 0, forbid(build.rs 포함) |
| `impulcifer-cli` | 헤드리스 CLI | 0, forbid |
| `impulcifer-python` | PyO3/maturin, 소유 입출력 경계 | 손수 쓰는 것 0, forbid. 매크로 생성 코드는 별도 검토 |
| `impulcifer-policy` | CI에서 메타데이터·구문 파싱으로 정책 검사 | 0, forbid |

```text
existing vanilla UI + JSON catalogues
                 |
         impulcifer-app (Tauri)
                 |
        impulcifer-service <---------- impulcifer-cli
                 |
          impulcifer-jobs
           /           \
impulcifer-dsp      impulcifer-audio-io
     ^              /              \
     |      [Windows only]      [macOS/Linux only]
impulcifer-python   impulcifer-sys-win       cpal
                       |
                     wasapi ---> windows/windows-core
```

leaf 안에서도 공개 unsafe 함수 0, `unsafe impl Send/Sync` 0, `static mut` 0, transmute 0, SIMD/어셈블리 0, 내보내는 원시 포인터 0입니다. 트립와이어를 넘기거나 새 범주가 필요하면 ADR 승인이 선행됩니다. 범용 `unsafe-utils` 크레이트는 만들지 않습니다. Python이나 SIMD용 unsafe가 정말 필요해지면 Windows 크레이트에 끼워 넣지 말고 별도 도메인 leaf를 새로 결정합니다.

### 4.2 leaf 공개 API 규칙

공개 타입은 소유 값, 검증된 설정, 일반 슬라이스, enum, `Result`뿐입니다. `OutputSession::play_to_completion(&mut self, PlaybackBuffer, &CancelToken) -> Result<PlaybackReport>`, `CaptureSession::read_into(&mut self, &mut [f32], &CancelToken) -> Result<CaptureRead>` 같은 형태이고 두 세션 타입은 의도적으로 `!Send + !Sync`입니다. HWND, COM 인터페이스, 네이티브 포인터, 원시 버퍼를 받는 콜백은 이 경계를 넘지 않습니다. `set_dark_titlebar(hwnd: usize)` 같은 "정수 핸들을 받는 안전 함수"는 만들지 않습니다.

### 4.3 COM 소유권 규칙

- Tauri 메인(STA) 아파트먼트는 건드리지 않습니다. 오디오 전용 OS 스레드마다 MTA를 초기화하고, 열거자·장치·AudioClient·서비스·이벤트를 그 스레드에서 만들고 쓰고 해제합니다.
- `CoInitializeEx`는 S_FALSE까지 포함해 균형을 맞추고, 초기화 실패 뒤에는 `deinitialize`를 부르지 않습니다. 네이티브 인터페이스를 전부 drop한 뒤에 아파트먼트 가드를 놓습니다. wasapi-rs는 RAII 가드를 제공하지 않으므로 우리 어댑터가 `PhantomData<Rc<()>>`로 `!Send`를 강제한 비공개 가드를 둡니다.
- 캡처 패킷의 `GetBuffer`/`ReleaseBuffer`는 같은 스레드에서 짝을 이루고, 패킷 borrow는 어댑터 밖으로 나가지 않습니다. SILENT면 데이터를 읽지 않고 0으로 채웁니다.
- 컴파일 오류를 없애려고 `unsafe impl Send`를 추가하지 않습니다. async 실행기 위에서 스레드를 옮겨 다니는 작업에 아파트먼트 종속 상태를 들고 있지 않습니다.
- `mem::forget`은 안전하므로 Drop이 항상 실행된다고 가정하지 않습니다. 값이 누수되면 자원은 새도 use-after-free는 생기지 않게 설계합니다.

### 4.4 린트와 정책 검사기

```toml
# workspace Cargo.toml
[workspace.package]
edition = "2024"
rust-version = "1.98"

[workspace.lints.rust]
unsafe_code = "forbid"
unsafe_op_in_unsafe_fn = "deny"

[workspace.lints.clippy]
undocumented_unsafe_blocks = "deny"
multiple_unsafe_ops_per_block = "deny"
missing_safety_doc = "deny"
```

일반 멤버는 `[lints] workspace = true`로 상속하고(Cargo 1.74 이후, 멤버가 명시적으로 옵트인해야 함), 크레이트 루트마다 `#![forbid(unsafe_code)]`와 `#![deny(unsafe_op_in_unsafe_fn)]`를 중복으로 둡니다. leaf만 `forbid`를 상속하지 않되 나머지 세 clippy 린트는 deny입니다. `--cap-lints`와 `--force-warn`이 forbid를 낮출 수 있으므로 `.cargo/config*`, build.rs, CI 환경의 rustc 플래그를 `impulcifer-policy`가 검사합니다. 이 검사기는 `cargo metadata`로 모든 타깃 루트(lib/bin/build/test/example/bench)를 열거하고, 첫 번째 당사자 Rust/TOML을 파싱해 unsafe 블록·함수·impl·extern·unsafe 속성을 세며, cfg로 숨긴 코드도 잡고, 플랫폼 의존성 직접 사용 권한(windows/wasapi는 leaf만, cpal은 audio-io만, PyO3는 python만, Tauri는 app만)을 강제합니다. 검사기 자체와 워크플로·툴체인 파일·감사 원장은 유지보수자 검토 대상이며, AI 에이전트가 스스로 승인할 수 없습니다.

## 5. 도구 판정

| 도구 | 검증된 상태 | 판정 |
|---|---|---|
| rustc `unsafe_code = forbid` | 의존성에는 전이되지 않음. 매크로 확장 코드는 진단에서 제외 | 필수 |
| Cargo workspace lints | 1.74 이후, 멤버 옵트인 필요 | 필수 |
| clippy `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `missing_safety_doc` | 앞 둘은 restriction(기본 off) | leaf에서 deny |
| cargo-deny 0.20.2 (2026-07-09) | advisories/licenses/bans/sources | 필수 게이트 |
| cargo-audit 0.22.2 (2026-06-05) | RustSec 검사 | 주간·릴리스 |
| cargo-vet 0.10.2 (2026-01-13) | 감사 원장. 기본 `safe-to-deploy`는 unsafe 전수 검토를 요구하지 않음 | 원장으로 채택하되 `impulcifer-soundness-reviewed` 커스텀 기준을 추가. 에이전트는 인증·면제 불가 |
| cargo-crev 0.27.1 | 서명된 리뷰, 신뢰망 | 선택 |
| cargo-geiger 0.13.0 (2025-08-31) | 구문 기반 unsafe 집계. `cargo install --locked`가 RUSTSEC-2025-0024에 걸린 yanked crossbeam-channel을 고르는 이슈 #564 미해결 | 자문용 인벤토리. 권한 있는 릴리스 잡에 설치하지 않음 |
| Miri (nightly) | 실행된 순수 Rust 경로의 UB·레이스 검출. FFI/COM 실행 불가, Windows 지원 제한 | 순수 코드와 모델 테스트에만. COM 검증 아님 |
| ASan/TSan (nightly `-Zsanitizer`, Linux x86_64) | std 재빌드 필요. TSan은 원자 fence·어셈블리 가시성 한계 | 주간 별도 잡, 필수 게이트 승격 전 실측 |
| Kani | 유계 검증 하니스 | 정수 길이·라우팅·상태 헬퍼에 소규모 선택 적용 |
| Verus | 지원 부분집합, 활발히 변경 중 | 초기 게이트에서 제외 |
| Rust 1.98.1 (2026-09-03) | 1.98.0의 vtable 생성 오컴파일 수정 | 첫 후보 툴체인으로 고정, edition 2024 |

edition 2024에서 `unsafe_op_in_unsafe_fn`은 warn 기본이므로 명시적으로 deny합니다. `unsafe extern` 블록과 `#[unsafe(no_mangle)]`은 leaf 밖에서 금지합니다. `std::env::set_var`는 2024에서 unsafe이므로 자식 프로세스 환경은 `Command::env`로만 설정합니다.

## 6. CI 게이트 (요약)

| 잡 | 빈도 | 내용 |
|---|---|---|
| policy-format | 매 PR | `cargo fmt --check`, `cargo run -p impulcifer-policy -- check` |
| stable-native | 매 PR, 3 OS | `cargo clippy --workspace --all-targets -- --no-deps -D warnings`, `cargo test --workspace` |
| feature-contracts | 매 PR | dsp/cli `--no-default-features`, app `--features desktop`, dsp `--features scalar` |
| dependency-policy | 매 PR + 일간 | `cargo deny check`, `cargo vet check` |
| windows-leaf | leaf/audio/의존성/컴파일러 변경 시 | `cargo test -p impulcifer-sys-win`, `-p impulcifer-audio-io` |
| miri-models | dsp/state/leaf 모델 변경 시 + 주간 | 고정 nightly로 jobs·dsp(scalar)·sys-win(model-tests) |
| asan-core / tsan-jobs | 주간 + 관련 변경 시 | Linux, std 재빌드, 별도 target dir |
| unsafe-inventory | 의존성/leaf PR + 주간, 자문 | 검증된 geiger 바이너리로 JSON 인벤토리 비교 |
| python-boundary | Python/의존성/컴파일러 변경 시 | maturin 빌드 → pip 설치 → 경계 테스트, FT 인터프리터 포함 |
| numerical-goldens | dsp/컴파일러/의존성 변경 시 | 스테이지별 f64 골든, WAV 계약 |
| hardware-release | 오디오 백엔드 변경 릴리스 전 | 통제된 실기 러너 또는 서명된 수동 수락 |

전체 명령과 예상 소요는 `report-11` §3.4에 있습니다. Miri와 sanitizer 인프라 장애는 조용한 continue-on-error가 아니라 소유자와 만료일이 있는 명시적 면제로 처리합니다.

## 7. 정책 한 페이지 (CONTRIBUTING/CLAUDE.md용 초안)

원문은 `report-11` §3.6에 있고 요지는 다음과 같습니다.

- 목표. 모든 애플리케이션 코드는 안전한 Rust이다. `impulcifer-sys-win`만 손수 쓰는 unsafe 예외 후보이며 승인된 연산 0개로 시작한다. 의존성·매크로·std·OS에 unsafe가 없다는 주장은 아니다.
- 기본. 다른 모든 라이브러리·바이너리·build.rs·테스트·예제는 `#![forbid(unsafe_code)]`와 `unsafe_op_in_unsafe_fn` deny를 쓰고 워크스페이스 린트를 상속한다. 린트 캡, cfg/매크로/생성 파일 뒤에 숨기기, 두 번째 예외 크레이트는 금지.
- unsafe를 추가하기 전에. 충족되지 않는 요구를 밝히고, 선택한 안전 API가 왜 안 되는지 보이고, 정확한 업스트림/네이티브 계약을 링크하고, 가장 작은 안전 인터페이스와 불변식 장부를 제안하고, 유지보수자 승인을 받는다. DWM, SIMD, 프로세스 환경 변경, zero-copy NumPy 차용, 복사한 네이티브 예제는 자동 예외가 아니다.
- leaf 계약. 공개 API는 소유 값·검증된 설정·일반 슬라이스·enum·Result만. 공개 원시 포인터/COM 인터페이스/unsafe 함수·트레이트/`static mut`/transmute/수동 Send·Sync 없음. 네이티브 객체는 소유 워커에서 생성·사용·해제하고 COM 초기화는 모든 종속 객체보다 오래 산다.
- 증명과 정리. 비공개 unsafe 함수는 `# Safety`로 호출자 의무를, 각 unsafe 블록은 바로 앞 `// SAFETY:`로 길이·정렬·초기화·별칭·출처·스레드·수명을 증명한다. "Windows API라서", "예제와 같아서", "포인터가 유효해서"는 증명이 아니다.
- 예산. 각 unsafe 지점은 승인된 사유 ID를 가진다. 12블록 또는 120줄 초과는 카운터 면제가 아니라 새 아키텍처 검토를 요구한다.
- Python과 워커. detach/Rayon 작업에는 독립적으로 소유된 Rust 버퍼와 설정만 들어간다. Python 토큰, Bound 값, 차용한 NumPy 메모리, COM 인터페이스, SendWrapper는 금지. 복사는 복사 중 동시 변경을 안전하게 만들지 않으므로 free-threaded 경계의 기본은 불변/직렬화된 입력이다.
- 의존성. 잠금 파일과 기능 집합을 검토 상태로 유지한다. 에이전트는 감사 증거를 준비할 수 있지만 cargo-vet 인증, 면제 추가, 신뢰 발행자 변경, advisory 억제를 유지보수자 승인 없이 할 수 없다.

## 8. 비용과 편익 (1인 유지보수자 + 코딩 에이전트)

- 검토 부담. 알고리즘·서비스 작업 대부분은 안전한 Rust와 의미 테스트로 검토할 수 있고, 불변식을 바꾸는 네이티브 패치만 집중 검토합니다. 줄어드는 것은 변하는 첫 번째 당사자 unsafe 표면이지 전체 신뢰 코드가 아닙니다.
- 에이전트 실패 양상. 에이전트는 원시 포인터/COM/SIMD 예제를 복사하거나 async 바운드를 맞추려 `unsafe impl Send`를 넣는 경향이 있습니다. forbid는 그런 패치를 즉시 실패시키며, 작업서는 린트 면제 대신 소유 데이터·동일 스레드 재설계를 요구해야 합니다.
- 런타임 비용. 버퍼 소유·복사·직렬화·스레드 메시지가 메모리와 시간을 더 쓸 수 있지만 저지연 요구가 없으므로 측정 전에는 최적화하지 않습니다.
- 빌드 비용. leaf 크레이트 하나는 Tauri/PyO3 컴파일에 비해 미미합니다. Miri, std 재빌드 sanitizer, 다중 OS/인터프리터 잡, 감사 도구 콜드 빌드가 실제 추가 비용입니다.
- 잔여 위험. 업스트림 래퍼·드라이버·매크로가 여전히 불건전할 수 있고 프로세스는 여전히 죽을 수 있습니다.

## 9. 열린 질문

1. **[U] 완전한 인벤토리 뒤에도 손수 쓰는 unsafe가 실제로 필요한가.** wasapi-rs가 주요 네이티브 연산을, Tauri가 테마를, PyO3/numpy가 소유 변환을, RustFFT가 SIMD를 제공하므로 leaf 예약은 조건부입니다. 12개 연산이 필요하다는 증거는 없습니다.
2. **[U] wasapi-rs 업스트림 수정 시점.** `WaveFormat::parse`와 SILENT 처리에 대해 이슈나 패치를 올리지 않았습니다. 채택 전 우리가 올리거나 최소 패치를 벤더링해야 합니다.
3. **[U] SILENT 패킷의 네이티브 메모리 계약.** 실제 하드웨어가 비어 있지 않은 SILENT 패킷에 null이나 미초기화 저장소를 주는지 재현이 필요합니다.
4. **[U] 실제 forbid 빌드.** Tauri/PyO3 매크로, edition 2024, limited-API, free-threaded Python, 3 OS 조합을 실제로 컴파일한 적이 없습니다. M1 파일럿에서 확인해야 합니다.
5. **[U] numpy free-threaded 입력 계약.** 차용 가드는 임의의 Python/네이티브 쓰기를 배제하지 못합니다. 초기 경계는 불변 bytes + 메타데이터로 두고, ndarray 편의 API는 별도 계약과 FT 테스트 뒤에 엽니다.
6. **[U] cargo-geiger 0.13.0이 Rust 1.98.1에서 동작하는지.** 마지막 확인된 수정은 1.89 호환입니다.
7. **[U] Miri/ASan/TSan용 날짜 고정 nightly와 Kani 버전.** 파일럿에서 고릅니다.
8. **[U] hound 3.5.1의 유지보수 응답성.** 마지막 발행이 2023-09-25이고, 30/32트랙 기본 채널 마스크가 0x3FFFF로 의미가 없으므로 자체 안전 RIFF writer가 유력합니다.

## 출처

두 보고서의 출처 목록(report-10은 55개 그룹, report-11은 76개)을 그대로 따릅니다. Claude가 직접 재확인한 것은 다음 둘입니다.

- wasapi-rs v0.24.0 `src/waveformat.rs`: https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/waveformat.rs
- wasapi-rs v0.24.0 `src/api.rs` 캡처 read 경로: https://raw.githubusercontent.com/HEnquist/wasapi-rs/v0.24.0/src/api.rs
