# A06e (ASTRA): two catalogue keys for the room correction tooltips

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Same rules and style as `docs/rust/packets/A06d-astra-tuning-processor-catalog.md` (read it): only the nine files `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`; do not run `cargo` or `npm`. Foreground only.

The long descriptions under the room correction controls move into tooltips (an info button next to the label). Two keys are needed. First check whether the nine overlay files or the 2.x catalogues under `i18n/locales/` (read-only) already have a generic "More information" / "Help" key used as an accessible button name; if one exists, do not add `button_more_info` and report its name instead.

| key | English | where |
| --- | --- | --- |
| `button_more_info` | `More information` | accessible name of the info button (screen readers; not visible) |
| `tooltip_room_tuning_phase_schroeder` | `Checked: only the room's bass timing up to the Schroeder frequency is corrected. Unchecked: timing is corrected over the full band, where the room measurements at both ear positions agree.` | tooltip of the Stable skin's checkbox "Correct timing only up to the Schroeder frequency" |

Terms as in the existing entries (Korean: 슈레더 주파수, 룸 측정, 시간 응답; 합니다체).

## Verification (foreground; paste the output)

Python check in your own scratch location: all files parse; every other key unchanged; one key set across the nine files (165 + 2 = 167, or 166 if `button_more_info` was not needed); English equals the table; no empty value; no non-English value equals the English one.

## Report

(1) Korean text; (2) whether a generic key already existed; (3) check output; (4) `git status --porcelain`.
