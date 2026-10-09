# A04 (ASTRA): catalogue entries for room correction v2

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스", the overlay catalogue rule) and `/home/user/Impulcifer-pip313/docs/adr/0003-3x-frontend-fork-and-overlay.md` item 2 first. Run every command in the foreground.

**Another worker is editing `crates/**/src`, `crates/**/tests`, `features.toml` and `docs/rust/ROOM_CORRECTION.md` at the same time, so do not run `cargo` (its tree is mid-edit) and do not open anything outside the files below for writing.**

Allowed files: the nine overlay catalogues `crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`. Nothing else (not `i18n/locales/`, which is 2.x; not `apps/`).

## What to add

Twenty-four keys, in all nine files. The English text below is final (it is also the inline fallback in `apps/impulcifer-app/ui/index.html`); translate it into the other eight languages. Placeholders are `{name}` and must appear unchanged, each exactly once, in every language.

### Screen (room correction section and virtual bass section)

| key | English |
| --- | --- |
| `label_room_range` | Correction range: |
| `option_room_range_modes` | Room modes only |
| `option_room_range_schroeder` | Up to the Schroeder frequency |
| `option_room_range_extreme` | Up to 10 kHz |
| `option_room_range_legacy` | Legacy (2.x) |
| `tooltip_room_range_modes` | Cuts room-mode resonances and leaves the bass balance as it is; the safest choice. |
| `tooltip_room_range_schroeder` | Makes the response flat up to the Schroeder frequency, including dips a real room EQ cannot fill, because the virtual speaker never moves and the measured point is the listening point. |
| `tooltip_room_range_extreme` | Corrects up to the Schroeder frequency and also applies a smoothed tone correction to the speaker and room up to 10 kHz; above about 400 Hz this is a broad tone adjustment, not an exact correction. |
| `tooltip_room_range_legacy` | The 2.x behaviour, and the only range that uses Specific Limit and Generic Limit. |
| `label_room_volume` | Room volume (m³): |
| `label_schroeder_freq` | Schroeder frequency (Hz): |
| `label_room_max_boost` | Max boost (dB): |
| `placeholder_schroeder_freq` | auto |
| `tooltip_vbass_room_handoff` | Room correction leaves everything below the crossover to virtual bass, except with the Legacy (2.x) range. |

"Specific Limit" and "Generic Limit" in `tooltip_room_range_legacy` must match how each language already translates `label_specific_limit` and `label_generic_limit` (look them up in `i18n/locales/<lang>.json`, read-only, without the trailing colon and "(Hz)"). "Legacy (2.x)" inside `tooltip_vbass_room_handoff` must match your `option_room_range_legacy`.

### Processing log (written by the service while a BRIR is built)

| key | English |
| --- | --- |
| `cli_room_range` | Room correction range: {range}, up to {f_hi} Hz |
| `cli_room_schroeder` | Schroeder frequency {freq} Hz (reverberation time {t60} s, room volume {volume} m³) |
| `cli_room_schroeder_assumed` | Schroeder frequency {freq} Hz (reverberation time {t60} s, room volume assumed to be 50 m³) |
| `cli_room_schroeder_override` | Schroeder frequency {freq} Hz, as set |
| `cli_room_schroeder_fallback` | Could not measure the reverberation time; using a Schroeder frequency of {freq} Hz |
| `cli_room_rolloff` | {speaker} {side}: the speaker rolls off below {freq} Hz, so nothing is boosted there |
| `cli_room_rolloff_snr` | {speaker} {side}: the measurement is too noisy below {freq} Hz, so nothing is boosted there |
| `cli_room_snr_unavailable` | {speaker} {side}: the room measurement is shorter than one second, so boosts are not limited by its noise level |
| `cli_room_selfcheck_warning` | {speaker} {side}: {rms} dB RMS error remains between {lo} and {hi} Hz after room correction; check the room measurement |
| `cli_room_vbass_handoff` | Room correction leaves the range below {freq} Hz to virtual bass |

`{range}` is a machine value (`modes`, `schroeder`, `extreme`), `{speaker}` a speaker code such as `FL` or `room.wav`, `{side}` one of `left`, `right`, `both`. Keep them as placeholders; do not translate around them in a way that assumes a grammatical gender or number.

## Style

Match the voice each file already uses (read the existing entries of that file first). In Korean that is the plain 합니다체 of the current `ko.json` overlay. The sentences are instructions and facts for someone setting up audio processing: plain, calm, no exclamation marks, no marketing or emotive words, no ornamental phrasing, technical terms the audio community in that language actually uses (for Korean: 룸 모드, 슈레더 주파수, 롤오프, 가상 베이스, 크로스오버, 잔향 시간). Keep labels short enough for a 150 px label column (the English is the length budget; a label may wrap to two lines, as "Crossover Frequency (Hz)" already does). Keep the trailing colon on `label_` keys where the language's existing labels have one.

## Verification (foreground; paste the output)

Write a small Python check in your own scratch location (not in the repo) and run it with `python3`:
- every file parses as JSON and keeps its existing keys and values unchanged (compare with `git show HEAD:<path>`);
- all nine files have exactly the same key set;
- for every key, the set of `{placeholders}` is identical across the nine files;
- the 24 keys above are present in all nine;
- no value is empty, and no non-English value is identical to the English one except `placeholder_schroeder_freq` where a language keeps "auto" on purpose (say which).

Then `git status --porcelain` and `git diff --stat`.

## Report format

(1) the Korean text of all 24 keys; (2) for each other language, any key where you made a non-obvious choice and why; (3) the check output; (4) `git status --porcelain`. Do not end your turn before the check has run.
