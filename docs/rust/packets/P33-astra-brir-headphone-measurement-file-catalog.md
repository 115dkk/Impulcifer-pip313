# P33: catalog strings for BRIR and headphone measurements from a REW or Dirac Live file

Repository: `/home/user/Impulcifer-pip313`. Read `/home/user/Impulcifer-pip313/CLAUDE.md` (the bullet
"3.x 문자열은 오버레이 카탈로그에 쓴다"), `/home/user/Impulcifer-pip313/docs/adr/0011-brir-and-headphone-measurements-from-rew-and-dirac.md`
and the previous packet `/home/user/Impulcifer-pip313/docs/rust/packets/P31-astra-room-measurement-file-catalog.md`
(its keys are already in the catalogs; reuse their terminology in each language).

## Context

The room correction section already accepts one REW measurement file (`.mdat`) or Dirac Live project
(`.liveproject`) and shows a table that assigns measurements to Impulcifer speaker codes and ear positions. The
same files now also replace two other inputs of the BRIR processing screen:

- **BRIR measurements**: the binaural recordings of each speaker at both ears, normally the speaker recordings
  (`FL,FR.wav` and so on) in the recording folder. A new row in the Input Files card picks either those
  recordings or a REW / Dirac Live file in which a binaural microphone measured each speaker at each ear. The
  results are still written to the recording folder. Each speaker needs one measurement per ear. Dirac Live has
  one microphone input, so the left and right ear are measured at two different Dirac positions.
- **Headphone measurement**: normally `headphones.wav` (the user's recording of the headphones through the
  binaural microphones). A new row in the Headphone Compensation section picks either that recording or a file;
  with a file the user chooses which measurement is the left earcup at the left ear and which the right earcup
  at the right ear.

The job log also reports the time difference between the two ears of each speaker (interaural time difference)
and warns when it is impossible for a head (over 1 ms) or zero for every side speaker.

Speaker codes, file names, `BRIR`, `REW`, `Dirac Live`, `.mdat`, `.liveproject`, `headphones.wav` stay
untranslated. Do not edit the screen (`apps/impulcifer-app/ui/`) or any Rust code.

## Keys

Add every key below to all nine overlay catalogs
`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`, right after the existing
`room_file_*` / `cli_room_measurement_file_*` keys, with the same placeholders in every language. The English
column is the source text; the Korean column is a draft to make natural.

| key | English | Korean draft |
| --- | --- | --- |
| `label_brir_measurements` | BRIR measurements: | BRIR 측정: |
| `option_brir_measurements_folder` | Speaker recordings in the recording folder | 녹음 폴더의 스피커 녹음 |
| `hint_brir_file` | Only the measurements assigned here are used. The folder's speaker recordings are not read, and the results are still written to the recording folder. | 여기서 정한 측정만 씁니다. 폴더의 스피커 녹음은 읽지 않고, 결과 파일은 지금처럼 녹음 폴더에 저장합니다. |
| `hint_brir_file_dirac` | Dirac Live measures one ear per position. The job log shows each speaker's time difference between the ears; a warning there means the two positions were measured with different delays. | Dirac Live는 위치마다 한쪽 귀만 잽니다. 작업 기록에 스피커마다 두 귀의 시간차가 나오며, 경고가 있으면 두 위치를 서로 다른 지연으로 잰 것입니다. |
| `brir_file_incomplete` | Some speakers have a measurement for only one ear ({speakers}). Assign the missing ear or set the row to Not used. | 한쪽 귀의 측정만 있는 스피커가 있습니다({speakers}). 빠진 귀를 정하거나 그 줄을 쓰지 않음으로 바꾸세요. |
| `brir_file_same_position` | The left and right ear use the same position. Choose the two positions where each ear was measured. | 왼쪽 귀와 오른쪽 귀가 같은 위치를 씁니다. 두 귀를 각각 잰 위치를 고르세요. |
| `brir_file_one_position` | This project has only one measured position. A BRIR needs the two ears measured at two positions. | 이 프로젝트에는 측정한 위치가 하나뿐입니다. BRIR에는 두 귀를 두 위치에서 따로 잰 측정이 필요합니다. |
| `brir_file_needs_fl` | FL is required, because the other speakers are aligned to it. | FL이 있어야 합니다. 다른 스피커를 FL에 맞춰 정렬하기 때문입니다. |
| `label_headphone_measurements` | Headphone measurement: | 헤드폰 측정: |
| `option_headphone_measurements_folder` | A recording (headphones.wav) | 녹음 파일(headphones.wav) |
| `label_headphone_file_left` | Left earcup at the left ear: | 왼쪽 귀에서 잰 왼쪽 이어컵: |
| `label_headphone_file_right` | Right earcup at the right ear: | 오른쪽 귀에서 잰 오른쪽 이어컵: |
| `option_headphone_file_none` | Choose a measurement | 측정 고르기 |
| `option_measurement_at_position` | {name}, position {n} | {name}, 위치 {n} |
| `headphone_file_incomplete` | Choose a measurement for both earcups. | 두 이어컵의 측정을 모두 고르세요. |
| `headphone_file_same` | Both earcups use the same measurement. Choose a different one for each. | 두 이어컵이 같은 측정을 씁니다. 서로 다른 측정을 고르세요. |
| `hint_headphone_file` | The folder's headphones.wav is not read. | 폴더의 headphones.wav는 읽지 않습니다. |
| `cli_brir_measurement_file_loaded` | BRIR measurements: {count} speakers from {file} ({format}) | BRIR 측정: {file}({format})에서 스피커 {count}개 |
| `cli_brir_measurement_file_replaces_folder` | The folder's {count} speaker recordings are not used because a BRIR measurement file is set | BRIR 측정 파일을 정했으므로 폴더의 스피커 녹음 {count}개는 쓰지 않습니다 |
| `cli_brir_measurement_file_itd` | Time difference between the ears per speaker (ms, right minus left) {list} | 스피커별 두 귀 시간차(ms, 오른쪽에서 왼쪽을 뺀 값) {list} |
| `cli_brir_measurement_file_itd_large` | {speaker}: the ears are {ms} ms apart, more than a head allows. The two ears may have been measured with different delays. | {speaker}의 두 귀 시간차는 {ms} ms로, 머리가 낼 수 있는 값보다 큽니다. 두 귀를 서로 다른 지연으로 잰 것일 수 있습니다. |
| `cli_brir_measurement_file_itd_zero` | Every side speaker reaches both ears at the same moment. If the ears were measured separately without a timing reference, the time difference between them is lost. | 모든 옆쪽 스피커의 소리가 두 귀에 같은 순간에 닿았습니다. 타이밍 기준 없이 두 귀를 따로 쟀다면 두 귀의 시간차가 사라진 것입니다. |
| `cli_headphone_measurement_file_loaded` | Headphone measurement from {file} ({format}), left {left}, right {right} | 헤드폰 측정 {file}({format}), 왼쪽 {left}, 오른쪽 {right} |
| `cli_headphone_measurement_file_replaces_folder` | {file} in the folder is not used because a headphone measurement file is set | 헤드폰 측정 파일을 정했으므로 폴더의 {file} 파일은 쓰지 않습니다 |

`{list}` is a list such as `FL +0.26, FR -0.25`; `{ms}` is a signed number such as `+9.38`; `{speakers}` is a
comma-separated list of speaker codes; `{name}` is a measurement or speaker name from the file.

Rules: read each language's existing `room_file_*`, `cli_room_measurement_file_*`, `label_headphone_file` and
the headphone compensation strings first, and match their register, terminology and punctuation (how each
language already says "measurement", "speaker", "ear", "earcup", "recording folder", "job log"). Labels ending
in a colon keep it in the style the language already uses. Plain, short sentences: no exclamation marks, no
marketing tone, no em dash, no physical metaphors, and no full sentence that ends in a colon followed by a list.
The Korean column is a draft; fix anything that reads unnatural (a Korean particle right after `{file}`,
`{speaker}` or `{name}` must work for any value; prefer wording that needs no particle there). Do not edit
`i18n/locales/`.

## Verification (foreground)

```bash
cd /home/user/Impulcifer-pip313
python3 -c "import json,glob; [json.load(open(f, encoding='utf-8')) for f in glob.glob('crates/impulcifer-service/locales/*.json')]; print('json ok')"
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

A Rust build is running in the same tree; do not run cargo. Do not commit. End your turn only after reporting.

## Report

The final text of all new keys in all nine languages (a table), and the output of both commands.
