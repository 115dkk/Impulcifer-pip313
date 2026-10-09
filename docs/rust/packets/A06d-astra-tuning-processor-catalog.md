# A06d (ASTRA): catalogue for virtual room tuning as a processor emulation

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Same rules, style and allowed files as `docs/rust/packets/A06-astra-room-tuning-catalog.md`, `A06b-…-r2.md` and `A06c-…-catalog.md` (read them): only the nine files `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`. Other workers edit `crates/**/src`, `crates/**/tests`, `features.toml`, `docs/**` and `apps/**` at the same time: do not run `cargo` or `npm`. Foreground only.

Virtual room tuning changed (see `docs/adr/0005-virtual-room-tuning.md`, readable): it now follows the room-correction processor completely. Per speaker it corrects the frequency response in detail up to the "curtain" frequency (default 300 Hz, fading out over the next octave), applies a broad tone correction above it, matches the speakers' levels, and corrects timing (by default over the full band; options: off, up to the Schroeder frequency, or up to a chosen frequency). It uses only the room measurements at the ear positions; `room.wav` is not used in this mode. The delay can be chosen automatically. Room EQ is the other mode and is unchanged.

## Change three existing keys (rewrite every translation to the new meaning)

| key | new English |
| --- | --- |
| `tooltip_room_mode_eq` | `Corrects the frequency response at each ear position; the timing of the bass stays as recorded.` |
| `tooltip_room_mode_tuning` | `Applies to each speaker the correction a room-correction processor would have made during the recording: a detailed frequency response correction in the low frequencies, a broad tone correction above them, speaker levels and timing. Both ears get the same correction. Uses the room measurements at the ear positions; room.wav is not used. The output starts later by the tuning delay.` |
| `tooltip_room_tuning_phase_limit` | `Up to the Schroeder frequency, the room's bass timing is corrected. Higher limits also correct the speaker's own timing, for example around its crossover, where the room measurements at both ear positions agree. Off corrects the frequency response only and adds no delay.` |

## Add eleven keys

| key | English | where |
| --- | --- | --- |
| `label_room_tuning_curtain` | `Detailed correction up to (Hz):` | label, 150 px column, may wrap |
| `tooltip_room_tuning_curtain` | `Above this frequency only a broad tone correction is applied; the detailed correction fades out over the next octave.` | hint |
| `checkbox_room_tuning_level_match` | `Match speaker levels` | checkbox |
| `tooltip_room_tuning_level_match` | `Matches the speakers' levels at the listening position within ±6 dB, as a room-correction processor does. Turn it off to keep the levels as recorded.` | hint |
| `checkbox_room_tuning_delay_auto` | `Choose automatically` | checkbox next to the tuning delay input |
| `option_room_tuning_phase_off` | `Off` | slider value text for the first stop (short) |
| `cli_room_tuning_off` | `Virtual room tuning without timing correction: frequency response and speaker levels only, no delay added` | log |
| `cli_room_tuning_single_point` | `{speaker}: room measurement at one ear position only; it is used for the whole speaker` | log |
| `cli_room_tuning_room_wav_ignored` | `room.wav is not used by virtual room tuning; it uses the room measurements at the ear positions` | log |
| `cli_room_tuning_no_data` | `No speaker has room measurements at the ear positions, so virtual room tuning is not applied` | log |
| `cli_room_tuning_level_inconsistent` | `{speaker}: the room measurement level differs from the recording by {db} dB, so this speaker's level is not matched` | log |

## Remove three keys (all nine files)

`cli_room_tuning_magnitude_only`, `cli_room_tuning_no_phase`, `cli_room_tuning_no_snr` (the processor no longer has those cases).

Terms must match the existing entries (Korean: 가상 룸 튜닝, 룸 EQ, 튜닝 지연, 룸 측정, 슈레더 주파수, 주파수 응답, 시간 응답; the word "curtain" never appears on screen, Korean calls the detailed correction 정밀 보정; 합니다체 for sentences; labels as noun phrases with the trailing colon where the language's labels have one). Keep `room.wav` as a file name in every language.

## Verification (foreground; paste the output)

Python check in your own scratch location: all files parse; every key not named above is unchanged (compare with a copy of the nine files taken before you edit); all nine files share one key set (157 − 3 + 11 = 165); placeholders identical per key across files (`{speaker}`, `{db}` exactly once where the English has them); English equals the tables exactly; no empty value; no non-English new or changed value equals the English one (list deliberate exceptions).

## Report

(1) Korean text of the fourteen new or changed keys; (2) non-obvious choices; (3) check output; (4) `git status --porcelain`.
