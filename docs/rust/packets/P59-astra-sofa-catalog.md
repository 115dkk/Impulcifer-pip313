# P59: catalog for the SOFA export (ADR 0019)

Repository: `/home/user/Impulcifer-pip313` (branch `ccr-083fe5e3-isoctv`). Work only inside it. Read first
`CLAUDE.md` (3.x workspace section: the overlay catalog rules), `docs/adr/0019-sofa-export.md` and
`docs/rust/SOFA.md` ("Files", "Content", "Headphone split").

Another worker is editing Rust files in `crates/impulcifer-io/` at the same time. Do not touch, build or format
anything outside the files allowed below, and do not run `cargo fmt`.

## Task

Write real sentences in all nine `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json` for
the twelve keys below, with the same placeholders in every file.

- The six UI keys (`checkbox_sofa` … `studio_adv_sofa_split`) already exist in all nine files, right after
  `hint_lfe_mode_all`, with English text. Keep the English in `en.json` unless it is wrong; replace the English in the
  other eight files with translations.
- The six `cli_` keys do not exist yet. Add them in all nine files directly after `studio_adv_sofa_split`, in the
  order of the table. The backend will log them with exactly these placeholders.

The English column is the meaning to carry: plain interface language, no marketing tone, no exclamation marks.
File names (`brir.sofa`, `brir-no-headphone-eq.sofa`, `headphone-eq.wav`, `hrir.wav`), `SOFA`,
`SimpleFreeFieldHRIR`, `LFE` and `EQ` stay as they are. Match the vocabulary of the neighbouring strings of each file
(how that language already says headphone compensation, speaker, track, left ear, save, player, LFE).

Korean follows the maintainer's rules: 합니다체 for descriptions, no em dash, no sentence ending in a colon before
a list, no 번역투 (`~의 경우`, `~될 수 있다`, `활용하다`, `경로`, `탐색하다`, `창` for anything but a window,
`약하다`/`절반` for truth values, `X에 가깝다`); single quotes for terms; no emoji. Read the neighbouring Korean
strings first (`hint_lfe_mode_*`, `hint_layout_files_*`, `cli_success_layout_file`, `cli_readme_layouts_*`).

| Key | Where | English meaning |
| --- | --- | --- |
| `checkbox_sofa` | output files, checkbox | SOFA file (brir.sofa) |
| `tooltip_sofa` | tooltip of that checkbox | The same BRIRs as hrir.wav in the SOFA format (SimpleFreeFieldHRIR), for players that read SOFA. Each speaker is stored at its nominal direction, 2 m away. The LFE is not included. |
| `checkbox_sofa_split_headphone_eq` | checkbox nested under the SOFA checkbox | Also save it without headphone compensation, plus the headphone EQ |
| `tooltip_sofa_split_headphone_eq` | its tooltip | Also saves brir-no-headphone-eq.sofa and headphone-eq.wav (left ear, right ear), so the player can apply the headphone EQ itself. brir.sofa is saved as well. Needs headphone compensation. |
| `tooltip_sofa_split_locked` | tooltip while that checkbox is disabled | Turn on the SOFA file and headphone compensation first. |
| `studio_adv_sofa_split` | short label in a grid of output files, next to the file names | SOFA + headphone EQ |
| `cli_success_sofa` | job log | Saved the BRIR as a SOFA file ({count} speakers): {path} |
| `cli_success_sofa_split` | job log | Saved the BRIR without headphone compensation as a SOFA file: {path}, and the headphone EQ: {eq} |
| `cli_info_sofa_split_skipped` | job log | This job does not use headphone compensation, so brir-no-headphone-eq.sofa and headphone-eq.wav were not saved. |
| `cli_readme_sofa_title` | README section title | SOFA |
| `cli_readme_sofa_note` | README paragraph | brir.sofa holds the BRIRs of hrir.wav in the SOFA format (SimpleFreeFieldHRIR): one measurement per speaker at its nominal direction, 2 m away. LFE is not included. |
| `cli_readme_sofa_split_note` | README paragraph | brir-no-headphone-eq.sofa holds the same BRIRs without headphone compensation. Apply headphone-eq.wav (track 1 left ear, track 2 right ear) in the player to get brir.sofa. |

Suggested Korean (use unless a neighbouring string forces another word):

- `checkbox_sofa` 'SOFA 파일(brir.sofa)'
- `tooltip_sofa` 'SOFA를 읽는 플레이어를 위해 hrir.wav와 같은 BRIR을 SOFA 형식(SimpleFreeFieldHRIR)으로 저장합니다. 스피커마다 공칭 방향, 2 m 거리에 둡니다. LFE는 넣지 않습니다.'
- `checkbox_sofa_split_headphone_eq` '헤드폰 보정을 뺀 파일과 헤드폰 EQ도 저장'
- `tooltip_sofa_split_headphone_eq` 'brir-no-headphone-eq.sofa와 headphone-eq.wav(왼쪽 귀, 오른쪽 귀)도 저장해 플레이어가 헤드폰 EQ를 직접 걸 수 있게 합니다. brir.sofa도 함께 저장합니다. 헤드폰 보정을 켜야 고를 수 있습니다.'
- `tooltip_sofa_split_locked` 'SOFA 파일과 헤드폰 보정을 먼저 켜세요.'
- `studio_adv_sofa_split` 'SOFA + 헤드폰 EQ'
- `cli_success_sofa` 'BRIR을 SOFA 파일로 저장했습니다(스피커 {count}개): {path}'
- `cli_success_sofa_split` '헤드폰 보정을 뺀 BRIR을 SOFA 파일로 저장했습니다: {path}, 헤드폰 EQ: {eq}'
- `cli_info_sofa_split_skipped` '이 작업은 헤드폰 보정을 쓰지 않으므로 brir-no-headphone-eq.sofa와 headphone-eq.wav를 저장하지 않았습니다.'
- `cli_readme_sofa_title` 'SOFA'
- `cli_readme_sofa_note` 'brir.sofa에는 hrir.wav의 BRIR이 SOFA 형식(SimpleFreeFieldHRIR)으로 들어 있습니다. 스피커마다 측정 하나를 공칭 방향, 2 m 거리에 두었고 LFE는 넣지 않았습니다.'
- `cli_readme_sofa_split_note` 'brir-no-headphone-eq.sofa에는 같은 BRIR이 헤드폰 보정 없이 들어 있습니다. 플레이어에서 headphone-eq.wav(1번 트랙 왼쪽 귀, 2번 트랙 오른쪽 귀)를 걸면 brir.sofa와 같아집니다.'

The `studio_adv_sofa_split` label sits in a narrow grid cell: keep it as short as the English in every language.

Edit with exact string replacement; keep the files' formatting (two-space indent, UTF-8, no escaping of non-ASCII)
and key order; do not touch other keys.

## Allowed files

`crates/impulcifer-service/locales/*.json` only.

## Verification (foreground; report the tail of each)

```bash
cd /home/user/Impulcifer-pip313
export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0
for f in crates/impulcifer-service/locales/*.json; do python3 -I -c "import json,sys; json.load(open(sys.argv[1], encoding='utf-8'))" "$f" || echo "BAD $f"; done
cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
```

Run the two cargo commands separately, in the foreground, and end your turn only after reporting the key table as
written (key, English, Korean, and the Japanese for `studio_adv_sofa_split`) and the command tails. If a cargo
command fails because the other worker's Rust files do not compile at that moment, wait a minute and run it again;
do not edit those files. Do not commit. Do not start other agents.
