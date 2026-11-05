# P39: catalog strings for the unmeasured speaker and LFE options (ADR 0013)

Repository: `/home/user/Impulcifer-pip313` (branch `ccr-083fe5e3-isoctv`). Work only inside it. Read `CLAUDE.md`
first (bullet "3.x 문자열은 오버레이 카탈로그에 쓴다") and `docs/adr/0013-missing-speakers-and-lfe-channel.md`, which
says what the two options do.

## Context

The synthesis tab's output options gain two selects:

- "Unmeasured speakers": leave the tracks of speakers that were not measured silent, or copy the nearest measured
  speaker on the same side and layer (a missing center is made from the symmetric pair beside it). Height channels
  are never filled from ear-level speakers.
- "LFE channel" (only the channel-order files such as `atmos_9.1.6.wav` have LFE tracks): silent, copy the center
  speaker (or the FL+FR phantom), or synthesize the bass of every measured speaker below 120 Hz (power average,
  minimum phase). The reason for the last one: averaging directions evens out the room's peaks and dips at any one
  position, the way several subwoofers do.

The job log and the result folder's README then say which channels were filled and from where.

## Keys

All sixteen already exist in all nine overlay catalogs `crates/impulcifer-service/locales/*.json` with the English
text. Replace the text in the eight other languages; keep English as it is.

UI (select labels end with a colon like their neighbours `label_layout_files` etc.):

| key | English |
| --- | --- |
| `label_missing_speakers` | Unmeasured speakers: |
| `option_missing_speakers_silent` | Silent tracks |
| `option_missing_speakers_neighbor` | Copy the nearest measured speaker |
| `hint_missing_speakers` | For hesuvi.wav, hrir.wav and the channel-order files. The nearest measured speaker on the same side and at the same height is copied, and a missing center speaker is made from the pair beside it. Height channels are never filled from ear-level speakers. |
| `label_lfe_mode` | LFE channel: |
| `option_lfe_mode_none` | Silent |
| `option_lfe_mode_center` | Copy the center speaker |
| `option_lfe_mode_all` | Bass from every direction |
| `hint_lfe_mode_none` | The LFE tracks of the channel-order files stay silent. hesuvi.wav and hrir.wav have no LFE track. |
| `hint_lfe_mode_center` | The LFE tracks use the center speaker, or the two front speakers when the center was not measured. |
| `hint_lfe_mode_all` | The LFE tracks are made from the bass of every measured speaker below 120 Hz. Averaging the directions evens out the peaks and dips of any one position, as several subwoofers do in a room. |

Log and README (`cli_` prefix):

| key | English |
| --- | --- |
| `cli_info_filled_channels` | {file}: unmeasured channels are filled from the nearest measured speaker ({list}) |
| `cli_info_lfe_filled` | {file}: the LFE tracks use {source} |
| `cli_readme_filled_channels` | Unmeasured channels filled from the nearest measured speaker: {list} |
| `cli_readme_lfe_filled` | The LFE tracks use {source}. |
| `cli_lfe_source_all` | the low frequencies of every measured speaker (below 120 Hz) |

Placeholders: `{file}` is a file name (`hesuvi.wav`, `atmos_9.1.6.wav`); `{list}` is a list such as
`SL ← FL, SR ← FR, FC ← FL+FR`; `{source}` is either a speaker code (`FC`) or a code pair (`FL+FR`) or the
translated `cli_lfe_source_all` text. So the sentence around `{source}` must read correctly with a bare code and
with that phrase: write `cli_lfe_source_all` as a noun phrase that fits there. A particle or inflection right
after a placeholder must work for any value (in Korean, prefer a construction that needs no 을/를 or 이/가 after
`{source}`, or the form that reads for both a code and the phrase).

Wording: use each language's existing words for "speaker", "center", "track", "channel", "height", "LFE" and
"measured" (read `label_layout_files`, `option_layout_files_*`, `hint_layout_files_*`, `cli_readme_layouts_*`,
`cli_success_layout_file` and the speaker descriptions in each overlay, and the 2.x catalog
`i18n/locales/<lang>.json`; do not edit the 2.x catalog). Keep file names, `LFE`, `120 Hz` and speaker codes as they
are. Plain sentences: no exclamation marks, no em dash, no marketing tone. Do not translate the subwoofer comparison
into a promise; it explains why the option exists.

## Verification (foreground)

```bash
cd /home/user/Impulcifer-pip313
python3 - <<'PY'
import json, glob, re
files = sorted(glob.glob('crates/impulcifer-service/locales/*.json'))
data = {f: json.load(open(f, encoding='utf-8')) for f in files}
first = set(data[files[0]])
print('same key sets' if all(set(d) == first for d in data.values()) else {f: sorted(first ^ set(d)) for f, d in data.items()})
ph = lambda s: sorted(re.findall(r'\{[a-z_]+\}', s))
bad = [(f, k) for f, d in data.items() for k in d if ph(d[k]) != ph(data[files[0]][k])]
print('placeholders ok' if not bad else bad)
keys = ['label_missing_speakers','option_missing_speakers_silent','option_missing_speakers_neighbor','hint_missing_speakers','label_lfe_mode','option_lfe_mode_none','option_lfe_mode_center','option_lfe_mode_all','hint_lfe_mode_none','hint_lfe_mode_center','hint_lfe_mode_all','cli_info_filled_channels','cli_info_lfe_filled','cli_readme_filled_channels','cli_readme_lfe_filled','cli_lfe_source_all']
en = data['crates/impulcifer-service/locales/en.json']
left = [(f, k) for f, d in data.items() if not f.endswith('en.json') for k in keys if d[k] == en[k]]
print('all translated' if not left else left)
PY
git diff --stat
```

Edit only the sixteen values in the eight non-English overlay catalogs. Do not run cargo, do not edit any other file,
do not commit, and do not start other agents. End your turn after reporting the sixteen keys in all nine languages
(a table) and the command output.
