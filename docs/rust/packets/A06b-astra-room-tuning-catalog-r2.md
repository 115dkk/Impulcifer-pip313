# A06b (ASTRA): catalogue update for virtual room tuning, revision 2

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Same rules, style and allowed files as `docs/rust/packets/A06-astra-room-tuning-catalog.md` (read it): only the nine files `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`; another worker edits `crates/**/src`, `crates/**/tests`, `features.toml` and `docs/rust/**` at the same time, so do not run `cargo`. Foreground only.

The meaning of "Virtual room tuning" changed: it now applies, per speaker, the correction a room-correction processor in the speaker feed would have applied while the BRIRs were recorded (computed from the room measurement files, the same filter for both ears). The low-frequency timing part needs room measurements at both ear positions.

## Change two existing keys (all nine files)

| key | new English |
| --- | --- |
| `tooltip_room_mode_tuning` | Applies to each speaker the correction a room-correction processor would have made during the recording: frequency response, and bass timing up to the Schroeder frequency (300 Hz at most), which shortens room-mode ringing. Bass timing needs room measurements at both ear positions. The output starts later by the tuning delay. |
| `cli_room_tuning_no_snr` | {speaker}: the room measurement is shorter than one second, so its bass timing is not corrected |

Rewrite every translation of these two to the new meaning (do not patch the old sentences).

## Add four keys (all nine files)

| key | English |
| --- | --- |
| `cli_room_tuning_magnitude_only` | {speaker}: no room measurements at both ear positions, so only the frequency response is corrected |
| `cli_room_tuning_no_phase` | No speaker has room measurements at both ear positions, so bass timing is not corrected and no delay is added |
| `cli_room_tuning_mismatch` | {speaker}: the room measurements do not match this recording's left/right arrival order or listening position; check that they belong to this recording |
| `cli_room_tuning_weak` | {speaker}: bass timing improved less than expected; the room measurements may not represent the ear positions |

Terms must match the existing entries (Korean: 가상 룸 튜닝, 튜닝 지연, 룸 측정, 슈레더 주파수, 룸 모드, 주파수 응답; 합니다체; effect rather than mechanism).

## Verification (foreground; paste the output)

Python check in your own scratch location: all files parse; every key other than the two changed ones is unchanged vs `git diff`'s base (`git show HEAD:<path>` is the committed state; the working tree already has the earlier A06 additions, which are not committed yet, so compare against a copy of the files taken before you edit); all nine files share one key set (147 + 4 = 151); placeholders identical per key across files; English equals the tables exactly; no empty value; no non-English new or changed value equals the English one.

## Report

(1) Korean text of the six keys; (2) non-obvious choices; (3) check output; (4) `git status --porcelain`.
