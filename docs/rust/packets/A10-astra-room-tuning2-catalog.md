# A10 (ASTRA): catalogue entries for virtual room tuning 2

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스", the overlay catalogue rule) and the previous catalogue packets `docs/rust/packets/A04-astra-room-v2-catalog.md` and `A06-astra-room-tuning-catalog.md` (same rules, same style section) first. Run every command in the foreground.

**Another worker edits `crates/**/src`, `crates/**/tests`, `features.toml` and `docs/**` at the same time: do not run `cargo` and do not open anything outside the files below for writing.**

Allowed files: the nine overlay catalogues `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`.

Background, for the meaning only: "Virtual room tuning 2" (ADR 0008, `docs/adr/0008-virtual-room-tuning-v2.md`) applies, per speaker, the room correction of SECS 2, a program by the Speaker Gallery community: a PEQ (parametric EQ) and an FIR filter in front of each speaker that correct its bass response and, within the filter's latency, part of the room's bass timing. MIMO adds, to each speaker, low-frequency copies played by the other speakers, which cancel the bass that lingers at the ears. "SECS 2", "PEQ", "FIR" and "MIMO" stay as they are in every language.

## Add 27 keys in all nine files

English is final; translate into the other eight. Placeholders `{name}` unchanged, each exactly once.

### Screen (room correction section)

| key | English |
| --- | --- |
| `option_room_mode_tuning2` | Virtual room tuning 2 |
| `tooltip_room_mode_tuning2` | Applies to each speaker the room correction of SECS 2 (Speaker Gallery): a PEQ and an FIR in front of the speaker that correct its bass response and, within the filter's latency, part of the room's bass timing. Speaker levels are matched to the quietest one. Both ears get the same correction. Uses the room measurements at the ear positions; room.wav is not used. The output starts later by the filter's latency. |
| `label_room_tuning2_mode` | Filter: |
| `option_room_tuning2_mode_zero` | PEQ only (no latency) |
| `option_room_tuning2_mode_mixed` | Low latency (8 ms) |
| `option_room_tuning2_mode_linear` | Standard (28 ms) |
| `option_room_tuning2_mode_long` | Long filter (200 ms) |
| `option_room_tuning2_mode_custom` | Custom latency |
| `tooltip_room_tuning2_mode_zero` | Corrects the frequency response with PEQ only. No latency is added and the timing of the bass stays as recorded. |
| `tooltip_room_tuning2_mode_mixed` | PEQ and an FIR with 8 ms latency. The FIR corrects part of the room's bass timing within that latency. |
| `tooltip_room_tuning2_mode_linear` | PEQ and an FIR with 28 ms latency, which corrects more of the room's bass timing. |
| `tooltip_room_tuning2_mode_long` | PEQ and a long FIR with 200 ms latency, which corrects the most bass timing. Every channel starts 200 ms later. |
| `tooltip_room_tuning2_mode_custom` | PEQ and an FIR with the latency you enter (30–200 ms). A longer latency corrects more of the bass timing. |
| `label_room_tuning2_latency` | Latency (ms): |
| `label_room_tuning2_low` | Correct from (Hz): |
| `label_room_tuning2_high` | Correct up to (Hz): |
| `tooltip_room_tuning2_high` | Auto uses the Schroeder frequency from the room volume or the Schroeder frequency below, at most 500 Hz. Without either it is 500 Hz. |
| `placeholder_room_tuning2_auto` | auto |
| `tooltip_room_tuning2_level_match` | Lowers every speaker to the level of the quietest one, as SECS 2 does. Turn it off to keep the levels as recorded. |
| `checkbox_room_tuning2_mimo` | Cancel late bass with the other speakers (MIMO) |
| `tooltip_room_tuning2_mimo` | Adds to each speaker low-frequency copies played by the other speakers, designed to cancel the bass that lingers at the ears after the first 60 ms. Works only below the upper limit, and a check keeps every band from getting worse. |
| `label_room_tuning2_fsiso` | MIMO up to (Hz): |
| `label_room_tuning2_mimo_low` | MIMO from (Hz): |

### Processing log

| key | English |
| --- | --- |
| `cli_room_tuning2` | Virtual room tuning 2 from {low} to {high} Hz; every channel starts {delay} ms later |
| `cli_room_tuning2_preecho` | {speaker}: pre-echo reaches {db} dB before the direct sound; a filter with less latency reduces it |
| `cli_room_tuning2_mimo` | {speaker}: {supports} other speakers cancel its late bass (MIMO), at {db} dB |
| `cli_room_tuning2_mimo_off` | {speaker}: MIMO would make a bass band worse, so this speaker gets no MIMO |

## Style

As in A04/A06: the voice each file already uses (Korean: 합니다체 for sentences, noun phrases for labels; no 해요체; no -아/어 주다·드리다·두다 on the program's actions), plain and calm, effect rather than mechanism, no exclamation marks or marketing words, nothing decorative. Terms must match what the files already use for the same things ("room measurement", "Schroeder frequency", "frequency response", "pre-echo", "direct sound", "speaker levels", the existing "Max boost" label). For Korean: 가상 룸 튜닝 2 as the option name (parallel to the existing 가상 룸 튜닝), 지연 for latency, 늦은 저음 for late bass, 필터 for Filter; labels keep the trailing colon where the language's existing labels have one, and fit a 150 px label column (may wrap to two lines). The four option labels for the filter should read as a set (same pattern in each language).

## Verification (foreground; paste the output)

A Python check in your own scratch location (not the repo), run with `python3`:
- every file parses; all existing keys and values are unchanged (compare with `git show HEAD:<path>`);
- all nine files have the same key set (previous count + 27);
- per key, the placeholder set is identical across the nine files;
- the English values equal the tables above exactly;
- no value is empty; no non-English new value equals the English one except where a term stays as is by the rule above (report each).

Then `git status --porcelain` and `git diff --stat -- crates/impulcifer-service/locales`.

## Report format

(1) the Korean text of the 27 keys; (2) non-obvious choices per language; (3) the check output; (4) `git status --porcelain`. Do not end your turn before the check has run.
