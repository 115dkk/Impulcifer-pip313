# P56: catalog for the room measurement's subwoofer (ADR 0017)

Repository root: the worktree you are started in. Read first `CLAUDE.md` (3.x workspace section: the overlay
catalog rules) and `docs/adr/0017-room-subwoofer-lfe.md`, `docs/rust/LFE.md` ("Logs and README").

## Task

Write real sentences in all nine `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json` for
the keys below, with the same placeholders in every file. Seven keys exist with English placeholder text in all
nine files (the backend added them); replace them with translations. `room_file_subwoofer` exists with a sentence
that is now wrong in every language; rewrite it. The English column is the meaning to carry: plain interface
language, no marketing tone, no exclamation marks. Match the vocabulary of the neighbouring strings of each file
(how that language already says subwoofer, LFE, room measurement, room correction, in-ear, speaker).

Korean follows the maintainer's rules: 합니다체 for descriptions, no em dash, no sentence ending in a colon before
a list, no 번역투 (`~의 경우`, `~될 수 있다`, `활용하다`, `경로`, `탐색하다`, `창` for anything but a window,
`약하다`/`절반` for truth values, `X에 가깝다`); single quotes for terms; no emoji. Read the neighbouring Korean
strings first.

| Key | Where | English meaning |
| --- | --- | --- |
| `room_file_subwoofer` (rewrite) | room measurement table, subwoofer row | A subwoofer. Assigned to LFE, it corrects the LFE track, and makes it when no subwoofer was measured in the ears. |
| `cli_room_lfe_created` | job log | {slot}: the LFE track is made from the room measurement's subwoofer, level matched on {count} speakers |
| `cli_room_lfe_level_spread` | job log, warning | {slot}: the level difference between the in-ear and the room measurements varies by {spread} dB across speakers; they may have been measured at different seats |
| `cli_room_lfe_no_level` | job log, warning | {slot}: the room measurement shares only {count} speakers with the BRIR measurements (3 are needed), so no LFE track is made from its subwoofer |
| `cli_room_lfe_correction` | job log | {slot}: corrected with the room measurement's subwoofer, {low}–{high} Hz |
| `cli_room_lfe_uncorrected` | job log | {slot}: the room measurement has no subwoofer for this LFE track, so it is not corrected |
| `cli_lfe_source_measured_corrected` | log and README, inside a sentence ("the LFE tracks use {source}") | the measured subwoofer, corrected with the room measurement |
| `cli_lfe_source_room` | same | the room measurement's subwoofer |

Suggested Korean (use unless a neighbouring string forces another word): `room_file_subwoofer` '서브우퍼입니다.
LFE에 배정하면 LFE 트랙을 보정하는 데 쓰고, 귓속에서 잰 서브우퍼가 없으면 이 측정으로 LFE 트랙을 만듭니다.',
`cli_lfe_source_measured_corrected` '룸 측정으로 보정한 측정 서브우퍼', `cli_lfe_source_room` '룸 측정의 서브우퍼'.
The two `cli_lfe_source_*` texts are inserted into `cli_info_lfe_filled` / `cli_readme_lfe_filled`; read those
sentences in each language and make the inserted phrase fit grammatically.

Edit with exact string replacement; keep the files' formatting and key order; do not touch other keys.

## Allowed files

`crates/impulcifer-service/locales/*.json` only.

## Verification (foreground; report the tail of each)

```bash
cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
```

Run them separately. Report the key table as written (key, English, Korean) and the command tails. Do not commit.
