# P27 (ASTRA): catalog strings for the immersive-layout screens, and the Korean register of P24's strings

Workspace: `/home/user/Impulcifer-pip313`. Run every command in the foreground; do not end your turn before the verification has completed and the report is written.

Read first: `docs/adr/0007-immersive-layouts.md` (what the feature does), `crates/impulcifer-service/locales/en.json` and `ko.json` (the 3.x overlay catalog; nine files share one key set), `crates/impulcifer-service/tests/ui_catalog.rs`, and the screens that use the new keys: `apps/impulcifer-app/ui/index.html` (search `rf-layout-row`, `bf-layout-files`, `ba-layout-files`) and `apps/impulcifer-app/ui/app.js` (`renderLayoutMap`, `useLayoutSpeakers`, `layoutFilesValue`). Do not edit `apps/` or anything outside the nine locale files.

## 1. Add eight screen keys to all nine files
`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`, same keys, same placeholders (none of these has placeholders). English (use as written unless you find an error):

| Key | English | Where it appears |
| --- | --- | --- |
| `label_sweep_channel_map` | `Device channels:` | Recorder, label of the list that maps each output channel of an immersive layout (e.g. 22.2) to the speaker code it plays: `1 FL → WL`, `7 FLc → FL` … |
| `hint_sweep_layout_codes` | `Type the code after each arrow into Speakers. Format labels can name other positions: channel 1 of 22.2 (FL, 60°) is WL here.` | Under that list. "Speakers" is the label of the speaker-list field (`label_sweep_speakers`); use the same word your language's `label_sweep_speakers` uses (look it up in `i18n/locales/<lang>.json`). Keep `FL`, `WL`, `22.2`, `60°` as they are. |
| `button_sweep_use_layout` | `Use every speaker in this layout` | Link-style button that fills the speaker field with every speaker code of the selected layout. |
| `label_layout_files` | `Channel-order files:` | Output options, label of a select. These files hold the BRIR again in a format's official channel order (`nhk_22.2.wav`, `auro_13.1.wav`, …). |
| `option_layout_files_auto` | `When every speaker is measured` | Option of that select (the default). |
| `option_layout_files_none` | `Off` | Option of that select. |
| `label_layout_files_force` | `Write even with missing speakers:` | Label above four checkboxes, one per format (Auro-3D 13.1, NHK 22.2, Dolby Atmos 24.1.10, DTS:X Pro 30.2). |
| `hint_layout_files` | `Tracks of missing speakers are silent.` | Hint under those checkboxes. |

Format names, file names, speaker codes and format labels are never translated.

## 2. Korean register
The Korean catalog states facts in 합니다체 and gives instructions with -세요 (see the existing `ko.json` entries). P24 wrote its eleven Korean entries in 해요체 by mistake: `cli_success_layout_file`, `cli_warning_layout_partial`, `cli_info_layout_missing`, `cli_readme_layouts_title`, `cli_readme_layouts_note`, `cli_readme_layouts_atmos_order`, `cli_readme_layouts_dts_order`, `cli_readme_layouts_partial`, `cli_readme_layouts_header_tracks`, `cli_readme_layouts_header_channel`, `cli_readme_layouts_header_speaker`. Rewrite those that end in 해요체 into 합니다체, keep the placeholders exactly, and write the eight new Korean keys in the same register. Plain, literal Korean: no translationese (no "~을 의미합니다", no needless passive), no decorative words. Suggested Korean for the new keys (improve only if wrong):
`장치 채널:` · `화살표 뒤의 코드를 스피커 칸에 입력하세요. 포맷의 라벨은 다른 위치를 가리킬 수 있습니다. 22.2의 1번 채널(FL, 60°)은 여기서 WL입니다.` · `이 배치의 스피커 모두 쓰기` · `채널 순서 파일:` · `모든 스피커를 측정했을 때` · `끔` · `스피커가 빠져도 만들 포맷:` · `빠진 스피커의 트랙은 무음입니다.`
(Check that the Korean `label_sweep_speakers` really is `스피커`-something and match it in the hint.)

## Verification (foreground; paste the tails)
```
cd /home/user/Impulcifer-pip313
python3 -c "import json,glob; [json.load(open(f,encoding='utf-8')) for f in glob.glob('crates/impulcifer-service/locales/*.json')]; print('json ok')"
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-service --lib settings
```

## Report
(1) the eight keys with their text in every language as a table; (2) the eleven Korean P24 entries before → after; (3) the verification tails verbatim.
