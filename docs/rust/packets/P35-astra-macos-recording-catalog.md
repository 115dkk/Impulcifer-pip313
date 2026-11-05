# P35: catalog strings for the macOS recording fixes

Repository: `/home/user/Impulcifer-pip313`. Read `/home/user/Impulcifer-pip313/CLAUDE.md` (bullet "3.x 문자열은
오버레이 카탈로그에 쓴다") first.

## Context

A tester recorded on an Apple silicon Mac with an AV receiver on HDMI. Two fixes need text:

- **Channel order.** On a Mac an HDMI or DisplayPort output takes its channels in the receiver's slot order: channel
  3 is LFE and channel 4 is FC (the Windows order, which the sweep tracks use, has FC on 3 and LFE on 4). The
  recorder gets a "Channel order" choice (Auto / Standard / HDMI), shown on macOS only for multichannel layouts.
  Auto uses HDMI when the chosen output is HDMI or DisplayPort. The job log names the order used.
- **Silent input.** macOS gives an app without microphone access a stream of exact zeros. The recording check now
  reports an input that recorded only digital silence.

Speaker codes (`FC`, `LFE`), `HDMI`, `DisplayPort`, `Impulcifer` and the placeholder `{channels}` stay as they are.
`{channels}` is a list such as `1 FL, 2 FR, 3 LFE, 4 FC, 5 SL, 6 SR, 7 BL, 8 BR`.

## Keys

Add every key to all nine overlay catalogs `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`
(put the `label_`/`option_`/`hint_` keys next to the existing recorder keys such as `label_sweep_channel_map`, and
the `record_check_` / `recording_` keys next to the existing `record_check_*` and `recording_share_mode_opened`).

| key | English | Korean draft |
| --- | --- | --- |
| `label_channel_order` | Channel order: | 채널 순서: |
| `option_channel_order_auto` | Auto | 자동 |
| `option_channel_order_standard` | Standard: FC on 3, LFE on 4 | 표준: 3번 FC, 4번 LFE |
| `option_channel_order_hdmi` | HDMI: LFE on 3, FC on 4 | HDMI: 3번 LFE, 4번 FC |
| `hint_channel_order` | A Mac sends the channels of an HDMI or DisplayPort output in the receiver's order. If a sweep plays from the wrong speaker, compare the device channels with Configure Speakers in Audio MIDI Setup. | 맥에서는 HDMI·DisplayPort 출력의 채널이 리시버의 순서로 나갑니다. sweep이 다른 스피커에서 나오면 Audio MIDI 설정의 스피커 구성과 장치 채널을 비교해 보세요. |
| `hint_channel_order_auto_hdmi` | This output is HDMI or DisplayPort, so the HDMI order is used. | 이 출력은 HDMI나 DisplayPort이므로 HDMI 순서를 씁니다. |
| `hint_channel_order_auto_standard` | This output is not HDMI or DisplayPort, so the standard order is used. For an aggregate device whose first channels are an HDMI output, choose HDMI. | 이 출력은 HDMI나 DisplayPort가 아니므로 표준 순서를 씁니다. 앞쪽 채널이 HDMI 출력인 통합 기기라면 HDMI를 고르세요. |
| `recording_channel_order_hdmi` | HDMI channel order: {channels} | HDMI 채널 순서: {channels} |
| `recording_channel_order_standard` | Device channel order: {channels} | 장치 채널 순서: {channels} |
| `record_check_input_silent` | The input recorded only digital silence. Check that the input device is the binaural microphone and that it is not muted. | 입력에 디지털 무음만 녹음됐습니다. 입력 장치가 바이노럴 마이크인지, 음소거돼 있지 않은지 확인하세요. |
| `record_check_input_silent_macos` | The input recorded only digital silence. On macOS an app without microphone access receives only silence. Allow Impulcifer in System Settings > Privacy & Security > Microphone, then quit and reopen Impulcifer and record again. | 입력에 디지털 무음만 녹음됐습니다. macOS에서 마이크 접근 권한이 없는 앱에는 무음만 들어옵니다. 시스템 설정 > 개인정보 보호 및 보안 > 마이크에서 Impulcifer를 허용한 뒤, Impulcifer를 종료했다가 다시 열고 다시 녹음하세요. |

## macOS names

Use the names Apple's macOS uses in each language for System Settings > Privacy & Security > Microphone, Audio MIDI
Setup, its Configure Speakers button, and Aggregate Device. Starting points (verify any you are unsure of against
Apple's localized macOS User Guide or Audio MIDI Setup User Guide on support.apple.com with WebFetch):

| lang | System Settings > Privacy & Security > Microphone | Audio MIDI Setup | Configure Speakers | Aggregate Device |
| --- | --- | --- | --- | --- |
| ko | 시스템 설정 > 개인정보 보호 및 보안 > 마이크 | Audio MIDI 설정 | 스피커 구성 | 통합 기기 |
| ja | システム設定 > プライバシーとセキュリティ > マイク | Audio MIDI設定 | スピーカーを構成 | 機器セット |
| de | Systemeinstellungen > Datenschutz & Sicherheit > Mikrofon | Audio-MIDI-Setup | Lautsprecher konfigurieren | Gerätesammlung |
| es | Ajustes del Sistema > Privacidad y seguridad > Micrófono | Configuración de Audio MIDI | Configurar altavoces | Dispositivo agregado |
| fr | Réglages Système > Confidentialité et sécurité > Micro | Configuration audio et MIDI | Configurer les haut-parleurs | Périphérique agrégé |
| ru | Системные настройки > Конфиденциальность и безопасность > Микрофон | Настройка Audio-MIDI | Настроить динамики | Агрегированное устройство |
| zh_CN | 系统设置 > 隐私与安全性 > 麦克风 | 音频 MIDI 设置 | 配置扬声器 | 聚集设备 |
| zh_TW | 系統設定 > 隱私權與安全性 > 麥克風 | 音訊 MIDI 設定 | 設定揚聲器 | 聚集裝置 |

Match each language's existing recorder strings for "sweep", "speaker", "device channel", "output", "input
device", "binaural microphone", "recording" (read `label_sweep_channel_map`, `hint_sweep_layout_codes`,
`record_check_*` in each overlay and the 2.x catalog `i18n/locales/<lang>.json`; do not edit the 2.x catalog).
Plain, short sentences: no exclamation marks, no em dash, no marketing tone, and no full sentence that ends in a
colon followed by a list. Labels ending in a colon keep the colon style the language already uses. The Korean
column is a draft; make it natural.

## Verification (foreground)

```bash
cd /home/user/Impulcifer-pip313
python3 - <<'PY'
import json, glob, re
files = sorted(glob.glob('crates/impulcifer-service/locales/*.json'))
data = {f: json.load(open(f, encoding='utf-8')) for f in files}
first = set(data[files[0]])
print('same key sets' if all(set(d) == first for d in data.values()) else {f: sorted(first ^ set(d)) for f, d in data.items()})
ph = lambda s: sorted(re.findall(r'\{[a-z_]+\}', s))
bad = [(f, k) for f, d in data.items() for k in d if ph(d[k]) != ph(data[files[0]][k])]
print('placeholders ok' if not bad else bad)
PY
```

A Rust build is running in the same tree; do not run cargo. Do not edit any other file. Do not commit. End your turn
only after reporting the final text of all eleven keys in all nine languages (a table) and the command output.
