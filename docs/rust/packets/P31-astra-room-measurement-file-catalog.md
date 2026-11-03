# P31: catalog strings for room measurements from a REW or Dirac Live file

Repository: `/home/user/Impulcifer-pip313`. Read `/home/user/Impulcifer-pip313/CLAUDE.md` (the bullet
"3.x 문자열은 오버레이 카탈로그에 쓴다") and `/home/user/Impulcifer-pip313/docs/adr/0010-room-measurements-from-rew-and-dirac.md` first.

## Context

The BRIR processing screen's Room Correction section gets a new first row: room measurements come either from
the recording folder's `room-*.wav` files (as before) or from one REW measurement file (`.mdat`) or Dirac Live
project (`.liveproject`). With a file, the screen shows what the file holds and a table that assigns each REW
measurement (or each Dirac speaker) to an Impulcifer speaker code (FL, FR, ...) and an ear position. A room
measurement here is a measurement microphone's response at an ear position of the listening seat, not a
binaural recording. Dirac Live measures the same speakers at several microphone positions (position 1 is its
main listening position); the user picks which position stands for the left ear and which for the right ear.
Speaker codes, file names, `REW`, `Dirac Live`, `.mdat`, `.liveproject` and `room-*.wav` stay untranslated.

When the file cannot be read, the screen shows `room_file_error` and keeps the reader's English message under
a collapsed `room_file_error_details` disclosure.

Do not edit the screen (`apps/impulcifer-app/ui/`) or any Rust code.

## Keys

Add every key below to all nine overlay catalogs
`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`, grouped after the existing room
correction keys (`label_room_mode` and its neighbours), with the same placeholders in every language. The
English column is the source text; the Korean column is a draft to make natural.

| key | English | Korean draft |
| --- | --- | --- |
| `label_room_measurements` | Room measurements: | 룸 측정: |
| `option_room_measurements_folder` | room-*.wav in the recording folder | 녹음 폴더의 room-*.wav |
| `option_room_measurements_file` | A REW or Dirac Live file | REW 또는 Dirac Live 파일 |
| `label_room_measurement_file` | Measurement file: | 측정 파일: |
| `room_file_summary_rew` | {count} measurements with an impulse response, {rate} Hz | 임펄스 응답이 있는 측정 {count}개, {rate} Hz |
| `room_file_summary_dirac` | Project {title}: {speakers} speakers, {positions} positions measured, {rate} Hz | 프로젝트 {title}: 스피커 {speakers}개, 측정한 위치 {positions}곳, {rate} Hz |
| `room_file_microphone` | Microphone: {name} | 마이크: {name} |
| `room_file_microphone_calibrated` | Microphone: {name}. The project includes its calibration, which is used when no Mic Calibration file is set. | 마이크: {name}. 프로젝트에 이 마이크의 보정 곡선이 들어 있어, 마이크 보정 파일을 정하지 않으면 이것을 씁니다. |
| `label_room_file_left_position` | Left ear position: | 왼쪽 귀 위치: |
| `label_room_file_right_position` | Right ear position: | 오른쪽 귀 위치: |
| `option_room_file_position` | Position {n} | 위치 {n} |
| `room_file_col_measurement` | Measurement | 측정 |
| `room_file_col_project_speaker` | Speaker in the project | 프로젝트의 스피커 |
| `room_file_col_speaker` | Speaker | 스피커 |
| `room_file_col_ear` | Ear position | 귀 위치 |
| `option_room_file_unused` | Not used | 쓰지 않음 |
| `option_room_file_both` | Both ears | 양쪽 귀 |
| `option_room_file_left` | Left ear | 왼쪽 귀 |
| `option_room_file_right` | Right ear | 오른쪽 귀 |
| `room_file_skipped` | No impulse response, so it cannot be used | 임펄스 응답이 없어 쓸 수 없습니다 |
| `room_file_subwoofer` | Subwoofer. The BRIR has no subwoofer channel, so it is used only if you pick a speaker for it. | 서브우퍼입니다. BRIR에는 서브우퍼 채널이 없으므로, 스피커를 골라 줄 때만 씁니다. |
| `hint_room_file` | Only the measurements assigned here are used. The folder's room-*.wav files are not read. | 여기서 정한 측정만 씁니다. 폴더의 room-*.wav는 읽지 않습니다. |
| `room_file_choose` | Choose a .mdat or .liveproject file. | .mdat 또는 .liveproject 파일을 고르세요. |
| `room_file_missing` | The file does not exist. Check the path. | 파일이 없습니다. 경로를 확인하세요. |
| `room_file_error` | This file cannot be read. It may be damaged or not a REW or Dirac Live file. Choose another file. | 이 파일을 읽을 수 없습니다. 파일이 손상됐거나 REW 또는 Dirac Live 파일이 아닐 수 있습니다. 다른 파일을 고르세요. |
| `room_file_error_details` | Details | 자세히 |
| `room_file_duplicate` | An ear position of a speaker has more than one measurement. Change the rows marked in red. | 한 스피커의 같은 귀 위치에 측정이 둘 이상 정해져 있습니다. 빨갛게 표시한 줄을 고치세요. |
| `room_file_none_assigned` | No measurement is assigned to a speaker yet. | 아직 스피커에 정한 측정이 없습니다. |
| `cli_room_measurement_file_loaded` | Room measurements: {count} responses from {file} ({format}) | 룸 측정: {file}({format})에서 응답 {count}개 |
| `cli_room_measurement_file_replaces_folder` | The folder's {count} room-*.wav files are not used because a room measurement file is set | 룸 측정 파일을 정했으므로 폴더의 room-*.wav {count}개는 쓰지 않습니다 |
| `cli_room_measurement_file_mic_calibration` | Using the microphone calibration stored in the Dirac Live project | Dirac Live 프로젝트에 저장된 마이크 보정을 씁니다 |

Rules: read each language's existing room-correction and EQ strings (`label_room_mode`, `eq_*`, `option_*` in
the overlay, and `label_mic_calibration` in the 2.x catalog `i18n/locales/<lang>.json` for reference only) and
match their register, terminology and punctuation (how each language already says "measurement", "speaker",
"ear position", "calibration", "recording folder"). Labels ending in a colon keep it in the style the language
already uses. Plain, short sentences: no exclamation marks, no marketing tone, no em dash, no physical
metaphors. The Korean column is a draft; fix anything that reads unnatural. Do not edit `i18n/locales/`.

## Verification (foreground)

```bash
cd /home/user/Impulcifer-pip313
python3 -c "import json,glob; [json.load(open(f, encoding='utf-8')) for f in glob.glob('crates/impulcifer-service/locales/*.json')]; print('json ok')"
python3 - <<'PY'
import json, glob
sets = {f: set(json.load(open(f, encoding='utf-8'))) for f in glob.glob('crates/impulcifer-service/locales/*.json')}
first = next(iter(sets.values()))
print('same key sets' if all(s == first for s in sets.values()) else {f: sorted(first ^ s) for f, s in sets.items()})
PY
```

A Rust build is running in the same tree; do not run cargo. Do not commit. End your turn only after reporting.

## Report

The final text of all keys in all nine languages (a table), and the output of both commands.
