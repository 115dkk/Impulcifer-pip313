# 실기 측정 기록 (Windows / WASAPI 백엔드)

`impulcifer-sys-win`의 `hardware_probe` 예제로 실제 장치에서 측정한 결과입니다. 정책 결정(exclusive 우선, shared+auto-convert 폴백, float32 전송)이 실제 장치에서 어떻게 귀결되는지 기록합니다. 새 장치나 새 백엔드 버전을 측정하면 이 문서에 절을 추가합니다.

## 2026-09-07, 개발 머신 (Windows 11 22621, wasapi-rs 0.24.0, 워크스페이스 커밋 a0cce2b + P01)

### 엔드포인트

| 방향 | 이름 | 채널 | mix Hz | 기본 |
|---|---|---:|---:|---|
| Output | 스피커(Steam Streaming Speakers) | 2 | 48000 | |
| Output | CABLE In 16ch(VB-Audio Virtual Cable) | 16 | 48000 | |
| Output | Line(Realphones System-Wide) | 2 | 44100 | |
| Output | 스피커(TOPPING USB DAC) | 2 | 48000 | |
| Output | 스피커(Steam Streaming Microphone) | 2 | 48000 | |
| Output | CABLE Input(VB-Audio Virtual Cable) | 8 | 48000 | 기본 |
| Output | CABLE-B Input(VB-Audio Cable B) | 2 | 48000 | |
| Output | CABLE-A Input(VB-Audio Cable A) | 8 | 48000 | |
| Input | CABLE Output(VB-Audio Virtual Cable) | 16 | 48000 | 기본 |
| Input | CABLE-B Output(VB-Audio Cable B) | 2 | 48000 | |
| Input | 마이크(Steam Streaming Microphone) | 1 | 44100 | |
| Input | CABLE-A Output(VB-Audio Cable A) | 2 | 48000 | |

### 프로브 매트릭스 (44.1/48/96 kHz × 2/8/16ch × exclusive/shared)

- **Exclusive float32는 거의 모든 엔드포인트가 거부했습니다.** `is_supported_exclusive_with_quirks`가 "Could not find a compatible format"을 돌려줍니다. 유일한 예외는 Line(Realphones System-Wide)로 44.1/48/96 kHz 모두 exclusive float32를 그대로 받았습니다. VB-Cable 계열, TOPPING USB DAC, Steam 가상 장치는 전부 거부입니다.
- **Shared + auto-convert는 모든 엔드포인트, 모든 레이트, mix 채널 수 이하의 모든 채널 수에서 지원**으로 판정됐습니다. mix 레이트와 다른 요청(44.1k/96k on 48k 엔진)은 "auto-convert enabled; engine mix format is 48000 Hz" 상세로 지원됩니다.
- 채널 수가 mix format을 넘는 요청은 백엔드가 열기 전에 `UnsupportedFormat`으로 거부합니다(조용한 다운믹스 없음).

전체 표는 `cargo run -p impulcifer-sys-win --example hardware_probe`로 재생성할 수 있습니다.

### 2026-09-08, 독립 녹음으로 확인한 공통 오디오 처리 중 누락

PA05 6차 측정에서는 Rust 재생마다 별도의 Python `sd.InputStream`을 동시에 실행했습니다. 4주기(실제 버퍼 1920프레임) 시험 8에서 두 녹음 모두 소스 `[25920,26400)`의 480프레임을 잃었습니다. 시작 시차를 맞춘 2078890프레임은 비트 단위로 같았고, 제출한 렌더 데이터의 체크섬과 소스 전체의 연속 제출도 확인했습니다. 해당 누락 지점의 캡처 패킷에는 SILENT나 discontinuity 표시가 없었습니다. 이처럼 서로 독립적인 두 녹음에서 같은 소스 구간이 사라졌다면 그 시험의 누락을 Rust 캡처에만 생긴 결함으로 볼 수 없습니다. 하지만 Windows 오디오 엔진과 VB-Cable 중 어느 쪽에서 누락됐는지는 확인하지 못했고, CPU 부하가 원인이라는 증거도 없습니다. 관찰용 녹음이나 정렬 분석이 실패한 다른 시험은 통과로 계산하지 않습니다. 원본과 추적 기록은 `crates/impulcifer-audio-io/tests/bench_support/sixth-p4-08-*`에 보존했습니다.

### 재생 + 캡처 (2스트림 세션)

| 출력 → 입력 | 레이트 | 결과 |
|---|---:|---|
| CABLE-A Input(8ch) ch1 → CABLE-A Output(2ch) | 48000 | **성공.** 두 방향 모두 exclusive 거부 후 shared+auto-convert. 제출 48000 프레임, 드레인 48000, 캡처 96000 프레임. 캡처 ch1 RMS -26.02 dBFS (1초짜리 -20 dBFS 사인을 2초 창에서 재면 -20 -3.01 -3.01 = -26.02 dBFS로 기대값과 일치), ch0 -inf. underrun 1, discontinuity 1(시작 직후), SILENT 패킷 0 |
| CABLE In 16ch ch3 → CABLE-A Output | 48000 | 성공(경로가 다른 케이블이라 캡처는 무음, 정상). underrun 0, discontinuity 1 |
| CABLE In 16ch → CABLE Output(16ch) | 48000, 44100, 96000 | **실패, 환경 문제.** 입력 초기화가 0x8889000A(AUDCLNT_E_DEVICE_IN_USE). 같은 엔드포인트를 sounddevice/PortAudio로 열어도 `Invalid device [-9996]`이므로(2ch auto-convert, 16ch 네이티브 모두) 이 캡처 엔드포인트를 다른 프로세스가 독점 중이거나 VB-Cable 상태 문제입니다. 백엔드 결함이 아닙니다 |

### 결론과 후속

1. **exclusive-first 정책은 이 머신에서 매번 폴백으로 끝납니다.** 비용은 프로브 한 번(밀리초)이라 유지하되, float32 exclusive를 받는 장치가 드물다는 점을 기록합니다. 후속 결정 사항: exclusive에서 int16/int24 전송을 허용할지(측정 무결성상 APO 우회 이득 vs 형식 변환 부담).
2. **shared + auto-convert가 실질 기본 경로**이며 44.1/48/96 kHz 전부 동작합니다. 2.x가 DirectSound에 기대던 이유(레이트 변환)를 WASAPI 안에서 해결했습니다.
3. 시작 직후 underrun/discontinuity 1회는 재생 시작 전 버퍼 프라이밍 순서를 점검할 항목입니다(`audio-io` 세션 패킷 P02에서 "입력 준비 확인 → 출력 시작" 순서와 함께 다룸).
4. 16채널 캡처 엔드포인트가 막힌 원인은 별도로 확인합니다(다른 앱의 독점 점유 여부). 백엔드 코드 변경 없음.
