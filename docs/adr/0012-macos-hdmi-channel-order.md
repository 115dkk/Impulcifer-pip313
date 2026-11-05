# ADR 0012: macOS의 HDMI 출력은 리시버의 채널 순서로 sweep을 낸다

- **상태:** 결정됨 (2026-11-05)
- **결정자:** 유지보수자 (115dkk)
- **관련:** ADR 0007(몰입형 포맷의 채널 대응), ADR 0009(Atmos 9.1 계열의 채널 순서)

## 맥락

Apple 실리콘 맥에 AV 리시버를 HDMI로 연결해 3.9.0을 쓴 테스터가 맥에서만 생기는 문제 넷을 보고했다.

1. 마이크 권한 문구(`NSMicrophoneUsageDescription`)가 앱에 없었다. 이 문구가 없는 앱에는 macOS가 권한을 묻지 않으며, 입력으로 0만 들어온다. 녹음은 성공으로 끝나고, 처리 단계에서야 쓸 수 없는 녹음이었음이 드러난다.
2. 녹음 버튼이 반응하지 않았다. 화면이 녹음 전에 `window.confirm`으로 설정을 확인하는데, WKWebView는 호스트가 대화상자를 구현하지 않으면 `confirm`에 바로 false를 돌려주고 `alert`는 보여 주지 않는다. Tauri가 쓰는 wry 0.55는 macOS에서 파일 선택 창만 구현한다.
3. 7.1 sweep의 센터 트랙이 서브우퍼에서 나왔다. sweep 트랙은 Windows(WAVE) 순서 `FL FR FC LFE BL BR SL SR`로 쓰인다. Windows의 HDMI 드라이버는 이 순서를 HDMI 슬롯으로 다시 배치하지만, 맥의 HDMI·DisplayPort 출력은 HDMI 슬롯(CEA-861)을 그대로 장치 채널로 보인다. 3번이 LFE, 4번이 FC, 5·6번이 서라운드, 7·8번이 서라운드 백이다. 그래서 FC와 LFE가 바뀌고, BL·BR과 SL·SR도 서로 바뀐다. Apple 개발자 포럼과 DCP-o-Matic 포럼에도 Apple 실리콘 HDMI 출력이 LFE를 3번, 센터를 4번에 둔다는 보고가 여럿 있다.
4. 앱에 서명과 공증이 없었다. 서명되지 않은 번들을 내려받으면 Gatekeeper는 '손상되었다'고 알리며 '그래도 열기'를 주지 않는다.

## 결정

1. **녹음은 장치 채널 순서를 고른다.** 녹음 요청에 `channel_order`(`auto`, `standard`, `hdmi`)를 둔다. `standard`는 지금의 순서다. `hdmi`는 앞의 여덟 장치 채널을 CEA-861 슬롯에 맞춘다. FC와 LFE를 바꾸고, 2.x 7.1 계열(5.1, 7.1, 7.1.4, 7.1.6)은 5~8번을 `SL SR BL BR`로 둔다. 9.1 계열은 5~8번이 이미 그 순서라(ADR 0009) FC와 LFE만 바꾼다. 아홉 번째 채널부터는 그대로다. 통합 기기(Aggregate Device)의 뒤쪽 장치로 나가는 채널이기 때문이다. 13.1, 22.2, 24.1.10, 30.2는 HDMI 순서를 두지 않는다. 이 포맷들의 앞 여덟 채널에 대응하는 CEA-861 배치가 하나로 정해지지 않는다.
2. **`auto`는 macOS의 HDMI·DisplayPort 출력에서만 `hdmi`다.** 출력의 연결 방식은 `system_profiler -json SPAudioDataType`의 `coreaudio_device_transport`로 알아낸다. CoreAudio의 채널 레이아웃(`kAudioDevicePropertyPreferredChannelLayout`)을 직접 읽으면 더 정확하지만 unsafe 호출이 필요하다. 모든 크레이트가 `forbid(unsafe_code)`이고 새 의존성도 넣지 않으므로 그 길은 쓰지 않는다. Windows와 Linux에서 `auto`는 언제나 `standard`다. 장치 목록은 장치마다 `hdmi`를 알리고, 녹음은 고른 순서를 로그와 결과(`channel_order`)에 남긴다. 화면은 macOS에서만 선택 칸과 그 순서의 장치 채널 표를 보인다.
3. **무음만 녹음되면 오류로 알린다.** 녹음한 모든 채널의 모든 샘플이 정확히 0이면 녹음 검사는 그 사실 하나만 오류로 알린다. macOS에서는 마이크 권한을 허용하는 곳을 함께 알린다. 파일은 지금처럼 저장한다.
4. **번들은 마이크 사용을 밝힌다.** `Info.plist`에 `NSMicrophoneUsageDescription`을, `Entitlements.plist`에 `com.apple.security.device.audio-input`을 둔다. 공증에 필요한 hardened runtime은 이 권한 없이 마이크를 막는다.
5. **화면은 자체 대화상자로 묻는다.** `window.confirm`, `window.alert`, `window.prompt`를 쓰지 않고 모든 플랫폼에서 같은 대화상자(`askDialog`)를 쓴다. 테스트가 화면 코드에 이 호출이 없음을 검사한다.
6. **서명은 임시 서명(ad hoc)으로 시작한다.** `signingIdentity`는 `"-"`다. 이렇게 서명한 번들은 Gatekeeper가 '손상되었다' 대신 '확인할 수 없는 개발자'로 막고, 시스템 설정의 '개인정보 보호 및 보안'에서 '그래도 열기'로 열 수 있다. 공증에는 유료 Apple Developer 계정이 필요하다. 저장소에 Developer ID 비밀값(`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`)이 있으면 릴리스가 같은 빌드를 그 인증서로 서명하고 공증한다. 릴리스의 macOS 잡은 번들의 서명, 마이크 문구, 권한을 검사한다.

## 결과

- 맥의 HDMI 출력으로 5.1·7.1 sweep을 낼 때 센터가 서브우퍼로, 사이드가 백으로 가지 않는다. 연결 방식을 알 수 없는 출력(통합 기기 등)에서는 사용자가 `hdmi`를 골라야 한다.
- `system_profiler`의 장치 이름과 cpal의 장치 이름이 다르면 `auto`는 `standard`로 남는다. 그때는 로그와 장치 채널 표로 확인하고 직접 고른다.
- Linux의 ALSA·PipeWire가 다채널 스트림을 어떤 순서로 내는지는 확인하지 않았다. 보고가 오면 같은 선택에 순서를 더한다.
- 임시 서명은 코드가 바뀔 때마다 서명 해시가 바뀌므로, 업데이트한 뒤 macOS가 마이크 권한을 다시 물을 수 있다. Developer ID로 서명하면 이 문제가 없다.
- 다시 제안하지 말 것: CoreAudio를 직접 부르려고 unsafe를 허용하는 것, `window.confirm`으로 되돌리는 것.
