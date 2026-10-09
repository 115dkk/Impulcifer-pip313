# A06 (ASTRA): catalogue entries for virtual room tuning

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스", the overlay catalogue rule) and the previous catalogue packet `docs/rust/packets/A04-astra-room-v2-catalog.md` (same rules, same style section) first. Run every command in the foreground.

**Another worker edits `crates/**/src`, `crates/**/tests`, `features.toml` and `docs/rust/**` at the same time: do not run `cargo` and do not open anything outside the files below for writing.**

Allowed files: the nine overlay catalogues `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`.

## Add nine keys in all nine files

English is final; translate into the other eight. Placeholders `{name}` unchanged, each exactly once.

### Screen (room correction section)

| key | English |
| --- | --- |
| `label_room_mode` | Correction mode: |
| `option_room_mode_eq` | Room EQ |
| `option_room_mode_tuning` | Virtual room tuning |
| `tooltip_room_mode_eq` | Corrects the frequency response only; the timing of the bass stays as recorded. |
| `tooltip_room_mode_tuning` | Corrects the frequency response and also the timing of the bass up to the Schroeder frequency (300 Hz at most), which shortens room-mode ringing. The output starts later by the tuning delay; without room measurement files, only the timing is corrected. |
| `label_room_tuning_delay` | Tuning delay (ms): |

### Processing log

| key | English |
| --- | --- |
| `cli_room_tuning` | Virtual room tuning up to {freq} Hz; every channel starts {delay} ms later |
| `cli_room_tuning_no_snr` | {speaker}: the measurement is shorter than one second, so its bass timing is not corrected |
| `cli_room_tuning_preecho` | {speaker}: pre-echo reaches {db} dB before the direct sound; a shorter tuning delay reduces it |

## Change one existing key in all nine files

`tooltip_room_range_schroeder` drops its last clause (it explained the mechanism, not the effect). New English: `Makes the response flat up to the Schroeder frequency, including dips a real room EQ cannot fill.` Shorten every translation the same way (remove the clause about the virtual speaker not moving and the measured point being the listening point), keeping the rest of each translation as it is.

## Style

As in A04: the voice each file already uses (Korean: 합니다체 for sentences, noun phrases for labels; no 해요체; no -아/어 주다·드리다·두다 on the program's actions), plain and calm, effect rather than mechanism, no exclamation marks or marketing words. Terms must match what the files already use for the same things: "Schroeder frequency", "room mode", "room measurement", "frequency response", "virtual bass", and "Room EQ" / "Virtual room tuning" must be used consistently between the two option labels and the two tooltips. For Korean use 룸 EQ and 가상 룸 튜닝 as the option names, 튜닝 지연 for the delay. Labels keep the trailing colon where the language's existing labels have one, and fit a 150 px label column (may wrap to two lines).

## Verification (foreground; paste the output)

A Python check in your own scratch location (not the repo), run with `python3`:
- every file parses; all existing keys and values are unchanged except `tooltip_room_range_schroeder` (compare with `git show HEAD:<path>`);
- all nine files have the same key set (138 + 9 = 147 keys);
- per key, the placeholder set is identical across the nine files;
- the English values equal the tables above exactly;
- no value is empty; no non-English new value equals the English one (report any deliberate exception).

Then `git status --porcelain` and `git diff --stat -- crates/impulcifer-service/locales`.

## Report format

(1) the Korean text of the ten keys (nine new plus the changed one); (2) non-obvious choices per language; (3) the check output; (4) `git status --porcelain`. Do not end your turn before the check has run.
