# WASAPI 단일 경로라면 PortAudio 대신 CPAL로 가도 되는가 (2026-09-07)

결론은 "가도 되지만, CPAL 하나로는 R1을 못 채우고 wasapi-rs를 곁들여야 한다"입니다. 그리고 C++ vtable 문제는 CPAL의 문제도, PortAudio의 문제도 아니라 **ASIO 하나의 문제**입니다. WASAPI만 쓰면 그 문제가 통째로 사라집니다.

표지는 [V] 검증(출처 또는 실측), [I] 추론, [U] 미확인입니다.

## 1. CPAL은 무엇으로 만들어져 있는가

[V] cpal v0.18.2의 `Cargo.toml` 기준입니다.

| 플랫폼 | 의존성 | 성격 |
|---|---|---|
| Windows | `windows`, `windows-core` 0.62 | 순수 Rust COM 바인딩(windows-rs). C 라이브러리 링크 없음 |
| Windows, `asio` 기능 켤 때만 | `asio-sys` 0.4.0 | **C++ 바인딩**. LLVM/Clang+bindgen 필요, Steinberg SDK 다운로드 |
| macOS | `coreaudio-rs`, `objc2-*` | Rust 바인딩. Apple 프레임워크(C/ObjC ABI)를 호출 |
| Linux | `alsa` → `alsa-sys` | C 라이브러리 libasound를 C ABI로 링크. 빌드 시 헤더 필요 |
| 선택 | `jack`, `pulseaudio`, `pipewire` | C 라이브러리 바인딩 |

즉 CPAL은 **Rust + C ABI**로만 이루어져 있고, C++ 컴파일러가 필요한 곳은 `asio` 기능 하나뿐입니다. WASAPI 단일 경로를 택하면 CPAL 트리에서 C++는 0이 됩니다.

## 2. 왜 vtable 문제는 ASIO에만 있는가

- [V] **WASAPI는 COM입니다.** COM 인터페이스는 vtable 포인터가 첫 필드이고 메서드는 stdcall인, 언어 중립을 목표로 설계된 C 호환 ABI입니다. windows-rs는 이것을 네이티브로 소비하며 C++ 컴파일러가 개입하지 않습니다. CPAL의 WASAPI 호스트가 순수 Rust인 이유입니다.
- [V] **ASIO는 COM이 아닙니다.** Steinberg SDK의 `IASIO`는 호출 규약을 명시하지 않은 C++ 추상 클래스라 MSVC에서 `thiscall`(this를 ECX로)이 됩니다. Borland/GCC는 물론 다른 언어에서도 그대로 호출하면 크래시가 나서, PortAudio와 RtAudio는 어셈블리로 쓴 `IASIOThiscallResolver` 어댑터를 실어 나릅니다. CPAL의 `asio-sys`도 같은 이유로 bindgen으로 C++ 심을 생성합니다. **이것이 정확히 "Rust는 C++ vtable에서 피똥 싼다"에 해당하는 사례이고, WASAPI에는 해당 사항이 없습니다.**
- [V] **PortAudio 자체는 Rust에 vtable 문제를 주지 않습니다.** PortAudio는 C 라이브러리이고 공개 API는 C ABI입니다. ASIO 호스트(`pa_asio.cpp`)만 내부적으로 C++이며 C API 뒤에 숨어 있습니다. Rust에서 PortAudio를 쓸 때의 실제 비용은 다른 데 있습니다. C 라이브러리를 플랫폼마다 빌드·고정·배포해야 하고, 래퍼 `rust-portaudio` 0.8.0(2024-10)은 유지보수 모드에 lifetime/UAF 보고가 있으며, 래퍼가 host-specific stream info를 항상 null로 넘겨 WASAPI exclusive·auto-convert·채널 마스크를 쓰려면 래퍼를 직접 확장해야 합니다(report-01 §2.4).

## 3. CPAL의 WASAPI 호스트가 실제로 하는 일

[V] cpal v0.18.2 `src/host/wasapi/device.rs` 기준입니다.

| 항목 | 상태 |
|---|---|
| 공유/독점 | **공유 모드만**. `AUDCLNT_SHAREMODE_SHARED` 고정, exclusive 코드 경로 없음. 이슈 #106(2016)과 #459(2020)가 아직 열려 있음 |
| 출력 auto-convert | 있음. `AUTOCONVERTPCM | SRC_DEFAULT_QUALITY`로 Initialize |
| 입력 auto-convert | **없음**. 입력은 `EVENTCALLBACK`만. 0.17.2에서 입력에도 켰다가(#1097) Windows 11 24H2 통신용 엔드포인트 무음 회귀(#1200)가 나서 #1201로 되돌림 |
| 채널 수 | mix format의 채널 수 하나만 보고("기본 채널 수만 지원된다고 가정") |
| 채널 마스크 | `KSAUDIO_SPEAKER_DIRECTOUT` 고정, 공개 API로 바꿀 수 없음 |
| I/O 모델 | 콜백 전용. "버퍼를 끝까지 재생하고 드레인" 동작은 직접 만들어야 함 |
| 알려진 문제 | COM STA/열거자 lifetime 접근 위반 #1302, 장치 재라우팅 #1339 |

이 표를 오늘 PortAudio로 실측한 결과에 대입하면 문제가 보입니다. 이 머신의 16채널 가상 케이블 입력은 2채널 캡처에 auto-convert가 있어야 열렸고(공유 모드 "Invalid number of channels"), 44.1k/96k 스윕은 출력 auto-convert가 있어야 열렸습니다. CPAL은 출력 쪽은 되지만 **입력 쪽은 사용자가 Windows 소리 설정에서 마이크를 스윕과 같은 레이트·채널 수로 맞춰 두지 않으면 열리지 않습니다.** 그리고 exclusive가 없으니 APO 우회와 네이티브 형식 비트 정확 재생이 불가능합니다.

## 4. 판정

| 질문 | 답 |
|---|---|
| WASAPI 단일 경로에서 PortAudio를 버려도 되는가 | [I] 됩니다. C 라이브러리 빌드·배포와 유지보수 모드 래퍼를 떠안을 이유가 없습니다 |
| CPAL 하나로 충분한가 | [I] 아닙니다. 입력 형식 변환 부재와 exclusive 부재 때문에 R1의 실측 사례를 못 엽니다 |
| C++ vtable 위험이 있는가 | [V] WASAPI 경로에는 없습니다. ASIO를 넣는 순간 생깁니다 |
| 그러면 무엇으로 | [I] Windows는 **wasapi-rs**, macOS/Linux는 **CPAL**. 둘 다 순수 Rust(+C ABI)입니다 |

[V] wasapi-rs 0.24.0(HEnquist, MIT, 2026-08-12)은 `initialize_client(wavefmt, direction, stream_mode)`에서 `ShareMode::Shared/Exclusive`, `StreamMode::EventsShared/PollingShared`의 `autoconvert` 플래그, `WaveFormat`의 채널 마스크, `get_mixformat()`, `is_supported()`, exclusive용 `is_supported_exclusive_with_quirks()`를 제공하고 캡처와 렌더를 모두 지원합니다. 01 문서의 권고 설계(exclusive 우선, shared+auto-convert 폴백, 엔드포인트 채널 수 검증, 전체 이름 식별)를 그대로 구현할 수 있는 API입니다. 이슈 #62(24비트 폴백), #63(exclusive 프로브)은 테스트로 덮을 항목입니다.

## 5. 권고 아키텍처

```text
impulcifer-audio-io/
  trait AudioBackend {
      enumerate() -> Vec<Endpoint>            // 전체 이름 + 엔드포인트 ID + 방향 + 채널 수 + mix 레이트
      probe(endpoint, fs, channels, mode)     // Exclusive / SharedAutoConvert 각각 판정
      open_output(endpoint, fs, channels, mode) -> OutputSession   // 버퍼 전체 재생 + 드레인 완료를 블로킹으로 감쌈
      open_input(endpoint, fs, channels, mode) -> InputSession     // 준비 확인 후 시작, 프레임 수·xrun 카운터
  }
  backend_wasapi.rs   // wasapi-rs: exclusive 우선 → 실패 시 shared+autoconvert. 채널 수는 mix format과 같게. DIRECTOUT 마스크
  backend_cpal.rs     // macOS CoreAudio, Linux ALSA. 콜백 위에 원자 커서 + 완료 신호로 블로킹 세션 구성
  session.rs          // 입력 먼저 시작 → 출력 재생 → 출력 드레인 → 입력 조인. 현재 recorder.py의 2스트림 계약 유지
```

- ASIO는 이 trait의 세 번째 백엔드 자리로만 남깁니다. 넣기로 하면 그때 C++ 심(asio-sys 또는 자체 thiscall 어댑터)과 SDK 라이선스, 단일 스트림 제약(듀플렉스 예외)을 함께 들여옵니다. 그 전까지는 이 크레이트에 C++가 없습니다.
- [I] CPAL을 Windows에도 쓰고 싶다면 CPAL 상류에 exclusive 모드를 기여하는 길도 있지만, 2016년부터 열려 있는 요청이라 일정에 넣을 수 없습니다.
- [U] wasapi-rs와 CPAL의 macOS/Linux 경로 모두 16채널 출력·2채널 입력 실기 검증은 하지 않았습니다. 01 문서의 하드웨어 게이트가 그대로 적용됩니다.

## 출처

- cpal v0.18.2 Cargo.toml: https://raw.githubusercontent.com/RustAudio/cpal/v0.18.2/Cargo.toml
- cpal v0.18.2 WASAPI device.rs: https://raw.githubusercontent.com/RustAudio/cpal/v0.18.2/src/host/wasapi/device.rs
- cpal exclusive 모드 요청 #106, #459: https://github.com/RustAudio/cpal/issues/106 , https://github.com/RustAudio/cpal/issues/459
- cpal 입력 auto-convert 회귀 #1200(#1097 도입, #1201 수정): https://github.com/RustAudio/cpal/issues/1200
- cpal COM/열거자 #1302, 재라우팅 #1339: https://github.com/RustAudio/cpal/issues/1302 , https://github.com/RustAudio/cpal/issues/1339
- IASIOThiscallResolver(Ross Bencina): http://www.rossbencina.com/code/iasio-thiscall-resolver
- PortAudio ASIO-README(thiscall 문제): https://github.com/EddieRingle/portaudio/blob/master/src/hostapi/asio/ASIO-README.txt
- RtAudio의 동일 심: https://github.com/thestk/rtaudio/blob/master/include/iasiothiscallresolver.cpp
- wasapi-rs 0.24.0 AudioClient: https://docs.rs/wasapi/latest/wasapi/struct.AudioClient.html
- rust-portaudio 유지보수 모드와 lifetime 보고: https://github.com/RustAudio/rust-portaudio , https://github.com/RustAudio/rust-portaudio/issues/196
- 라이브러리별 상세 근거: 같은 폴더의 `report-01-audio-io.md`
