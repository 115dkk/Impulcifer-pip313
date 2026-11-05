# P38: catalog strings for recording cancel and outputs with too few channels

Repository (a git worktree): `/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/wt-macsign`.
Work only inside it. Read its `CLAUDE.md` first (bullet "3.x 문자열은 오버레이 카탈로그에 쓴다").

## Context

- A running recording can now be cancelled. Studio shows a "Cancel recording" button in the recorder's page header;
  a cancelled recording keeps no file.
- When the output device has fewer channels than the chosen layout (an 8-channel HDMI receiver with 7.1.4, which
  needs 12), the recording is not blocked: it plays the channels the device has. Before the recording, the recorder
  shows a warning under the layout and asks for confirmation; afterwards the job log names what did not play.

Placeholders: `{device}` is the output device name (e.g. `DENON-AVR`), `{available}` and `{required}` are channel
counts, `{first}` and `{last}` are 1-based device channel numbers (e.g. 9 and 12), `{missing}` is a list of speaker
codes such as `TFL, TFR, TBL, TBR`, `{layout}` is a layout name such as `7.1.4` or `NHK 22.2`.

## Keys

All four already exist in all nine overlay catalogs `crates/impulcifer-service/locales/*.json` with the English text.
Replace the text in the eight other languages; keep English as it is.

| key | English | Korean draft |
| --- | --- | --- |
| `warning_output_channels_short` | {device} has {available} output channels, but {layout} needs {required}. Channels {first}–{last} ({missing}) will not play, so the recording will be missing those speakers. | {device}의 출력은 {available}채널인데 {layout}에는 {required}채널이 필요합니다. {first}~{last}번 채널({missing})은 재생되지 않으므로, 녹음에서 그 스피커가 빠집니다. |
| `recording_output_channels_short` | {device} has {available} output channels, so channels {first}–{last} ({missing}) were not played. The recording is missing those speakers. | {device}의 출력은 {available}채널이라 {first}~{last}번 채널({missing})은 재생하지 않았습니다. 이 녹음에는 그 스피커가 빠져 있습니다. |
| `recording_status_cancelled` | Recording cancelled. Nothing was saved. | 녹음을 취소했습니다. 저장한 파일은 없습니다. |
| `button_cancel_recording` | Cancel recording | 녹음 취소 |

Use each language's existing words for "output", "channel", "speaker", "recording" and "cancel" (read
`button_cancel`, `recording_status_*`, `record_check_*`, `label_sweep_channel_map` in each overlay and in the 2.x
catalog `i18n/locales/<lang>.json`; do not edit the 2.x catalog). A particle or inflection right after `{device}`
or `{layout}` must work for any value. Use the range punctuation each language normally uses between two numbers.
Plain sentences: no exclamation marks, no em dash, no marketing tone.

## Verification (foreground)

```bash
cd /tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/wt-macsign
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

Do not run cargo, do not edit any other file, do not commit, and do not start other agents. End your turn after
reporting the four keys in all nine languages (a table) and the command output.
