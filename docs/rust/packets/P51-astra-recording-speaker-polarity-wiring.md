# P51: wiring and catalog for the recording check's speaker polarity (ADR 0015 bundle 5)

Repository worktree: `E:\Impulcifer\.worktrees\s2r3` (Git Bash: `/e/Impulcifer/.worktrees/s2r3`), branch
`claude/tuning2-s2r-5`. Work only inside it. P50 (backend) is applied on this branch: a speaker take's
recording result now carries `measurements[].direct = {fs, left, right}` (the two direct-sound windows, a few
hundred numbers each) when the take qualifies, the recording request accepts the same object as an optional
`reference_measurements[].direct`, and two warnings, `record_check_speaker_polarity`
(`{speaker}`, `{compared}`, `{opposite}`, `{confidence}` as a string) and `record_check_speaker_polarity_split`
(`{group_a}`, `{group_b}`, `{confidence}`, all strings), were added to the nine catalogs with English text.
There is no new control on the screen. You connect the reference data and write the words.

Read first:

- `E:\Impulcifer\.worktrees\s2r3\CLAUDE.md`, "3.x Rust 워크스페이스": the overlay catalogs (nine files, same key
  sets and placeholders), `// @ts-check` and `apps/impulcifer-app/ui/ipc.d.ts`,
  `npm run typecheck && npm run test:contracts`.
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\ALIGNMENT.md`, "Recording checks", subsection "Speaker polarity
  (3.17.0)" (what the check does and which takes take part).
- `E:\Impulcifer\.worktrees\s2r3\docs\rust\packets\P49-astra-tuning2-s2r-4-catalog-and-wiring.md`, the previous
  wiring packet: the same conventions apply.

## 1. Wiring (`apps/impulcifer-app/ui/app.js`, `apps/impulcifer-app/ui/ipc.d.ts`)

- `rememberMeasurements`: keep each speaker measurement's `direct` with the take (normalize a missing one to
  `null`, the way `lf_ild_db` is normalized). A retake still replaces the speaker's entry.
- `gatherRecordingPayload`: every `reference_measurements` entry gains `direct: item.direct ?? null`. Do not
  send `direct` for anything else. The reference list is per folder and per app session, as today.
- `ipc.d.ts`: a `DirectSound { fs: number; left: number[]; right: number[] }` type;
  `RecordingMeasurement.direct?: DirectSound | null` and the reference entry's `direct?: DirectSound | null`,
  with a one-line comment in the style of the neighbouring lines.
- `checkMessage` needs no change: `confidence`, `group_a` and `group_b` arrive as strings and pass through;
  `compared` and `opposite` are integers. Confirm this by reading it.

## 2. Catalog (`crates/impulcifer-service/locales/{en,ko,ja,de,es,fr,ru,zh_CN,zh_TW}.json`)

Translate the two keys P50 added in the eight non-English files (find them with
`git diff origin/master -- crates/impulcifer-service/locales/en.json`). Keep every placeholder exactly as in
English, in every language. You may improve the English (same placeholders). Meaning:

- `record_check_speaker_polarity`: compared at the ear on its own side with the other speakers recorded in this
  folder, this speaker's direct sound has the opposite sign to most of them (`{opposite}` of `{compared}`
  comparisons; `{confidence}` is the median correlation strength, 0 to 1). Its + and − wires are probably
  swapped at the speaker or the amplifier; fix the wiring and record it again. It is a warning, not a verdict
  on the speaker.
- `record_check_speaker_polarity_split`: two groups of speakers have opposite signs and there is no third
  speaker to show which group is reversed; check the wiring of both groups.

Korean rules (the maintainer reads these): plain sentences, no translationese, no emoji, no em-dash; units with
a space before them. Say what the user should check; do not restate the English word order. '극성' is the word
for polarity; '배선' or '+/− 연결' for the wiring. Other languages: natural, technical register, same facts.

Gate: `cargo test -p impulcifer-service --lib settings` and `cargo test -p impulcifer-service --test ui_catalog`,
run separately (in one command the `settings` filter also filters out every `ui_catalog` test), and
`cargo test -p impulcifer-service --test recording_checks`. Do not touch `i18n/locales/` (2.x) or `webview_ui/`.

## 3. Gallery (`apps/impulcifer-app/tests/ui_gallery.py`)

- The mock recording result's speaker measurements gain a small `direct` (for example `{fs: 48000, left: [..],
  right: [..]}` with a handful of numbers; the screen only stores and returns it).
- In the existing reference checks, assert that the next request's `reference_measurements` carry each
  speaker's `direct` unchanged, that a measurement without `direct` sends `null`, and that the headphone and
  other-folder cases behave as they do today.
- Show the two new warnings once: make the mock return `record_check_speaker_polarity` (speaker `SL`,
  `compared: 3`, `opposite: 3`, `confidence: "0.78"`) and `record_check_speaker_polarity_split` (`group_a: "FL"`,
  `group_b: "FC"`, `confidence: "1.00"`) for one guided step or recorder take, assert the rendered texts contain
  `3`, `0.78` and `FL`, and add one capture pair named in the pattern of the existing captures (for example
  `recorder-polarity-studio-{en,ko}-dark`) showing them.

## Verification (foreground, before reporting)

```
cd /e/Impulcifer/.worktrees/s2r3/apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts
cd /e/Impulcifer/.worktrees/s2r3 && cargo test -p impulcifer-service --lib settings
cargo test -p impulcifer-service --test ui_catalog
cargo test -p impulcifer-service --test recording_checks
py -3.14 apps/impulcifer-app/tests/ui_gallery.py --out "C:\Users\32170336\AppData\Local\Temp\claude\b7093525e1d0c75f\7bcd40cd-140a-5f35-a38f-646c326a8bcc\scratchpad\ui-gallery-s2r5"
```

(Read the gallery script's usage first; the checks run before the captures, so exit 0 means the checks
passed.)

## Report (then end your turn)

1. Files changed, one line each.
2. The final English and Korean text of both keys, and the files you translated.
3. The commands' final lines; the gallery's PNG count and the names of the new captures.
4. Anything ambiguous that you resolved, and how.
