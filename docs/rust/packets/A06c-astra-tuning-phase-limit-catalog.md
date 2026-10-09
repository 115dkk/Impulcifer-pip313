# A06c (ASTRA): catalogue entries for the tuning phase limit

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Same rules, style and allowed files as `docs/rust/packets/A06-astra-room-tuning-catalog.md` and `docs/rust/packets/A06b-astra-room-tuning-catalog-r2.md` (read both): only the nine files `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`. Other workers edit `crates/**/src`, `crates/**/tests`, `features.toml`, `docs/rust/**` and `apps/**` at the same time, so do not run `cargo` or `npm`. Foreground only.

Virtual room tuning gets a new setting: how high the timing correction reaches. By default it stops at the Schroeder frequency (300 Hz at most), where the room's bass timing is corrected. Higher limits also correct the speaker's own timing (for example around its crossover), but only where the room measurements at both ear positions agree. The Stable skin shows a checkbox (checked = Schroeder only, unchecked = full band); the Studio skin shows a slider with the stops Schroeder, 500 Hz, 1 kHz, 2 kHz, 5 kHz, 10 kHz, full band.

## Add six keys (all nine files)

| key | English | where |
| --- | --- | --- |
| `label_room_tuning_phase_limit` | `Timing correction up to:` | Studio, label of the slider (150 px label column, may wrap) |
| `checkbox_room_tuning_phase_schroeder` | `Correct timing only up to the Schroeder frequency` | Stable, checkbox text |
| `option_room_tuning_phase_schroeder` | `Schroeder` | slider value text for the first stop (short: shown next to the slider, like "500 Hz") |
| `option_room_tuning_phase_full` | `Full band` | slider value text for the last stop |
| `studio_adv_unit_khz` | `kHz` | unit symbol, like the existing `studio_adv_unit_hz` (Russian uses кГц the way it uses Гц there) |
| `tooltip_room_tuning_phase_limit` | `Up to the Schroeder frequency, the room's bass timing is corrected. Higher limits also correct the speaker's own timing, for example around its crossover, where the room measurements at both ear positions agree.` | hint under both controls |

Terms must match the existing entries (Korean: 슈레더 주파수, 룸 측정, 가상 룸 튜닝; 합니다체 for sentences; labels as noun phrases with the trailing colon where the language's labels have one). `option_room_tuning_phase_schroeder` is the short name of the frequency as each language would print it on a slider (Korean `슈레더`, Japanese `シュレーダー`, and so on), not a sentence.

## Verification (foreground; paste the output)

Python check in your own scratch location: all files parse; every existing key and value is unchanged (compare with a copy of the nine files taken before you edit; the working tree has uncommitted earlier additions); all nine files share one key set (151 + 6 = 157); placeholders identical per key across files (none of the six has a placeholder); English equals the table exactly; no empty value; no non-English value equals the English one, except where the unit symbol is the same in that language (`kHz`), which you list.

## Report

(1) Korean text of the six keys; (2) non-obvious choices; (3) check output; (4) `git status --porcelain`.
