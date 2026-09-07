# Windows 오디오 API 선택: ASIO와 DirectSound/MME 폴백이 필요한가 (2026-09-07)

질문은 두 가지였습니다. Windows에서 ASIO 출력이나 DirectSound/MME 폴백을 지원할 이유가 있는가, 없다면 WASAPI 단일 경로로 가도 되는가. 결론은 다음과 같습니다. **지금 시점에 ASIO를 지원할 이유는 없고, DS/MME 폴백도 이유가 없습니다.** WASAPI 단일 경로로 가되, exclusive 모드 우선과 shared+auto-convert 폴백, 그리고 채널 수를 엔드포인트 네이티브 채널 수로 검증하는 두 조건이 붙습니다. ASIO는 12채널 이상 프로 인터페이스 수요가 실제로 나타날 때 옵션 백엔드로 붙일 수 있도록 어댑터 경계만 남겨 둡니다.

표지는 [V] 검증(출처 또는 실측), [I] 추론, [U] 미확인입니다.

## 1. 현재 제품의 실제 상태

| 항목 | 사실 |
|---|---|
| 번들 PortAudio의 호스트 API | [V 실측] sounddevice 0.5.5, PortAudio V19.7.0-devel. Windows에서 MME, DirectSound, WASAPI, WDM-KS 4종. **ASIO 없음**. 기본 호스트 API 인덱스 0 = MME |
| 코드의 폴백 순서 | [V] `core/recorder.py:238` DirectSound → MME → WASAPI. CTk 레코더 탭 기본값 "Windows DirectSound"(`gui/tabs/recorder_tab.py:86`) |
| WASAPI 설정 사용 | [V] `WasapiSettings`, `auto_convert`, `exclusive`가 코드 어디에도 없음 |
| 이슈 트래커 | [V] 이 저장소와 업스트림 jaakkopasanen/Impulcifer 모두 "ASIO", "WASAPI" 검색 결과 0건 |
| 업스트림 측정 가이드 | [V] 7.1 소비자 세팅에서 VLC로 재생하고 Audacity로 녹음, 추천 인터페이스는 Behringer UMC202HD(2출력) |

즉 ASIO 사용자는 지금까지 존재할 수 없었고, 요구도 기록된 바 없습니다.

## 2. DirectSound/MME가 기본이 된 진짜 이유

같은 머신에서 각 호스트 API의 기본 출력 장치에 44.1/48/96 kHz를 열 수 있는지 `sd.check_output_settings`로 실측했습니다(믹서 레이트 48 kHz).

| 호스트 API | 44.1 kHz | 48 kHz | 96 kHz |
|---|---|---|---|
| MME | OK | OK | OK |
| DirectSound | OK | OK | OK |
| WASAPI shared, 옵션 없음(현재 코드 상태) | **실패** (-9997) | OK | **실패** (-9997) |
| WASAPI shared + `auto_convert` | OK | OK | OK |
| WASAPI exclusive | OK | OK | OK |
| WDM-KS | OK | OK | OK |

[V] PortAudio의 WASAPI shared 모드는 `paWinWasapiAutoConvert`(AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | SRC_DEFAULT_QUALITY, 2019년 4월 추가) 없이는 믹서 레이트 하나만 받습니다. 원본 Impulcifer 코드는 그 이전 관행대로 DS/MME에 리샘플링을 맡겼고, 이 포크도 그대로입니다. 따라서 DS/MME 폴백은 WASAPI에 없는 기능이 아니라 **한 줄짜리 옵션의 부재를 메우는 우회로**입니다.

[V] Microsoft 문서에 따르면 Vista 이후 DirectSound와 waveOut/waveIn은 WASAPI 세션 위의 에뮬레이션이며, 레거시 인터페이스의 한계 때문에 엔드포인트 기능의 일부만 접근할 수 있습니다. DS/MME가 오히려 손해인 점은 다음과 같습니다.

- [V 실측] MME는 장치 이름을 31자에서 자릅니다(`'CABLE Input(VB-Audio Virtual Ca'` 대 WASAPI의 `'CABLE Input(VB-Audio Virtual Cable)'`). Wave API 구조체 한계입니다.
- [V] 공유 엔진을 거치므로 APO(Equalizer APO 자체, 라우드니스 이퀄라이제이션, 마이크 잡음 억제·AGC 같은 향상 기능)가 스윕과 마이크 입력에 개입합니다. Microsoft 저지연 오디오 문서와 dechamps/APO 노트에 따르면 exclusive 모드는 오디오 엔진과 APO를 우회합니다.
- [I] 리샘플링·믹싱 단계가 하나 더 끼어듭니다. 측정 신호에는 없을수록 좋습니다.

## 3. WASAPI만으로 무엇이 되고 무엇이 안 되는가

### 8채널 이하 (mono, stereo, 5.1, 7.1)

[V] HDMI LPCM은 8채널이 상한이므로 AVR 경유 7.1까지는 어떤 API로도 8채널이 전부이고, WASAPI로 충분합니다. 프로 인터페이스도 벤더 설정으로 8채널 WDM 장치를 만들 수 있습니다. Focusrite Scarlett 18i20/Clarett+는 Windows 소리 패널의 "구성"에서 5.1/7.1을 고르고 Focusrite Control에서 Windows 채널을 노출하며, RME USB.IO는 WDM 장치당 최대 8채널, MOTU Pro Audio 드라이버는 WDM 단일 장치로 최대 24채널을 제공합니다.

### 12·14·16채널 (7.1.4, 7.1.6)

- [V] HDMI로는 불가능합니다. Atmos 높이 채널은 TrueHD/MAT 비트스트림으로만 전달되며, 현재 코드도 Atmos 오브젝트 마스터를 거부하고 "Impulcifer가 생성한 다채널 WAV 스윕을 쓰라"고 안내합니다(`core/recorder.py:367-385`). 그 WAV를 재생하려면 12개 이상의 개별 출력을 가진 인터페이스가 필요합니다.
- [V] 프로 인터페이스의 WDM 노출은 벤더별입니다. MOTU는 24채널 WDM이 가능하고, RME는 8채널 단위 장치로 쪼개지며, Focusrite는 스테레오 쌍 단위입니다. **ASIO가 '범용' 경로인 것은 오직 이 조합, 즉 12채널 이상을 한 스트림으로 열어야 하는 RME/Focusrite류 인터페이스뿐입니다.**
- [V 실측] 16채널 WASAPI 엔드포인트(VB-Cable 16ch)에서 exclusive와 shared+auto_convert 모두 16채널 열기에 성공했습니다. 드라이버가 다채널 엔드포인트를 주면 WASAPI로 열립니다.

### auto-convert의 안전성

[V 실측] shared+auto_convert는 엔드포인트보다 **적은** 채널은 받아들이지만(2ch → 16ch 엔드포인트 OK), **많은** 채널은 거부합니다(스테레오 엔드포인트에 8ch/16ch 요청은 "Invalid number of channels"). 따라서 auto-convert가 7.1 스윕을 스테레오로 조용히 접어 버리는 일은 PortAudio 수준에서 일어나지 않습니다. 다만 [I] 적은 채널을 열면 Windows 매트릭서가 업믹스할 수 있으므로, 앱은 항상 엔드포인트의 전체 채널 수로 열고 트랙 배치를 직접 해야 합니다.

## 4. ASIO를 지금 넣지 말아야 할 비용

| 비용 | 근거 |
|---|---|
| 라이선스 | [V] ASIO SDK 2.3.4(2025-10-15)는 GPLv3 또는 독점 계약 양자택일. MIT 프로젝트가 ASIO를 켠 바이너리를 배포하려면 명시적 결정 필요 |
| 단일 스트림 제약 | [V] PortAudio ASIO는 스트림을 하나만 엽니다. 같은 인터페이스로 재생과 녹음을 하려면 현재의 2스트림 설계 대신 듀플렉스 예외를 둬야 합니다 |
| 빌드 | [V] CPAL의 asio 기능은 C++ 컴파일러, LLVM/bindgen, SDK 다운로드가 필요. PortAudio도 SDK 경로를 주고 다시 빌드해야 합니다 |
| 수요 | [V] 이슈 0건, 현재 제품 미지원, 업스트림 가이드는 소비자 7.1 |

## 5. 권고 설계 (WASAPI 단일 경로)

1. **출력 스트림**은 exclusive 모드를 먼저 시도합니다. 엔진·APO 우회, 네이티브 레이트, 드라이버가 받는 채널 수 그대로입니다. 실패하면 shared + `AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | SRC_DEFAULT_QUALITY`로 폴백합니다.
2. **입력 스트림**도 같은 정책입니다. [I] exclusive로 마이크 향상 APO(잡음 억제, AGC)를 우회하는 것이 측정 무결성에 유리합니다.
3. **채널 수 검증**은 엔드포인트의 mix format 채널 수(PortAudio `max_output_channels`)와 정확히 같게 엽니다. Windows 매트릭서에 채널 배치를 맡기지 않습니다. 부족하면 지금처럼 자르지 말고 오류로 알립니다.
4. **장치 식별**은 WASAPI 전체 이름과 엔드포인트 ID를 씁니다. 31자 잘림과 인덱스 변동 문제가 사라집니다.
5. **12채널 이상**은 우선 "WDM 다채널 엔드포인트를 제공하는 인터페이스(MOTU류)"로 지원 범위를 문서화합니다. [I, 미검증] 더 나은 대안은 장치 분할 라우팅입니다. 스윕은 스피커별로 순차 재생되고 정렬은 스윕 감지가 복원하므로, 스피커 그룹을 서로 다른 출력 장치(예: 8채널 엔드포인트 두 개)에 배정하면 16채널 단일 스트림이 필요 없어집니다. 이 방식이면 ASIO 없이도 RME/Focusrite류로 7.1.4를 덮을 수 있습니다. 장치 간 클럭 드리프트가 한 스윕(약 6초) 안에서 문제되는지 실측이 필요합니다.
6. **어댑터 경계**를 남겨 두어, 위 5번으로도 안 되는 요구가 실제로 접수되면 ASIO를 옵션 백엔드로 추가합니다. 그때 라이선스 결정과 듀플렉스 예외를 함께 처리합니다.

## 6. 2.x에 바로 적용할 수 있는 작은 개선

[I] 재작성과 무관하게, 현재 파이썬 코드에서 `sd.default.extra_settings = sd.WasapiSettings(auto_convert=True)`를 기본으로 두고 폴백 순서를 WASAPI 우선으로 바꾸면 DS/MME 의존이 사라집니다. 녹음 경로만 바뀌고 BRIR DSP는 건드리지 않으므로 무결성 해시 게이트에는 영향이 없습니다. exclusive 기본 적용은 다른 앱의 소리를 막고 일부 드라이버에서 실패하므로 옵션으로 두는 편이 안전합니다.

## 출처

- Microsoft, Interoperability with Legacy Audio APIs: https://learn.microsoft.com/windows/win32/coreaudio/interoperability-with-legacy-audio-apis
- Microsoft, Low Latency Audio(exclusive 모드가 엔진을 우회): https://learn.microsoft.com/windows-hardware/drivers/audio/low-latency-audio
- dechamps, Windows APO 노트: https://github.com/dechamps/APO
- PortAudio `pa_win_wasapi.h`(paWinWasapiAutoConvert): https://www.portaudio.com/docs/v19-doxydocs/pa__win__wasapi_8h.html
- NAudio 이슈 #819(AUTOCONVERTPCM 필요성): https://github.com/naudio/NAudio/issues/819
- python-sounddevice, WasapiSettings/AsioSettings: https://python-sounddevice.readthedocs.io/en/latest/api/platform-specific-settings.html
- PortAudio 메일링 리스트, MME 장치 이름 잘림: https://portaudio.music.columbia.narkive.com/fyHpcZW3/mme-device-names-are-truncated
- Focusrite, Windows 서라운드 구성: https://support.focusrite.com/hc/en-gb/articles/360013865959-Configuring-an-interface-for-Surround-Sound-on-Windows
- Focusrite, 비ASIO 앱용 Windows 채널 노출: https://support.focusrite.com/hc/en-gb/articles/360010711620-How-can-I-select-different-inputs-outputs-in-non-ASIO-apps-on-Windows-
- MOTU Pro Audio Driver Read Me(WDM 24채널): https://cdn-data.motu.com/downloads/audio/AVB/docs/MOTU%20Pro%20Audio%20Driver%20Read%20Me.pdf
- RME USB.IO 매뉴얼(WDM 장치당 8채널): https://rme-audio.de/downloads/usb_io_e.pdf
- HDMI LPCM 8채널 상한과 Atmos 비트스트림: https://cinemaconfig.com/reference/lpcm , https://cinemaconfig.com/reference/dolby-truehd
- Steinberg ASIO SDK 라이선스: https://www.steinberg.net/developers/asiosdk-open/
- PortAudio ASIO 단일 스트림 가드: https://raw.githubusercontent.com/PortAudio/portaudio/master/src/hostapi/asio/pa_asio.cpp
- 업스트림 측정 가이드: https://github.com/jaakkopasanen/Impulcifer/wiki/Measurements
- 자세한 라이브러리별 근거는 같은 폴더의 `report-01-audio-io.md`
