# Single recording file and measured LFE

Specification for ADR 0016. Two things are specified here: the third source of BRIR measurements (one WAV that
holds every speaker's sweep in a row) and the handling of a measured subwoofer as the LFE channel. With none of
the new settings given and no `LFE` in a measurement map, every output is byte-identical to 3.17.

## Configuration

Extension fields (`EXTENSION_FIELD_NAMES`, `EXTENSION_OPTIONS`), all optional strings:

| Field | CLI | Meaning |
| --- | --- | --- |
| `brir_recording_file` | `--brir_recording_file` | The recording: a WAV whose first two tracks are the left and the right ear. |
| `brir_recording_speakers` | `--brir_recording_speakers` | What each sweep of the recording is, in playback order. |
| `brir_recording_sweep` | `--brir_recording_sweep` | The sweep that was played, as a WAV (first track). Empty: the job's test signal. |

Validation (`INVALID_REQUEST` with `details.field`; the CLI exits 2):

- `brir_recording_file` and `brir_measurement_file` cannot both be set.
- `brir_recording_speakers` is required with `brir_recording_file`, and `brir_recording_speakers` or
  `brir_recording_sweep` without `brir_recording_file` is an error.
- The list is comma separated; entries are trimmed. Each entry is a slot code (`layouts::SLOTS`), `LFE`, `LFE2` or
  `X` (a sweep to skip). Codes are case sensitive. An empty entry, an unknown code, a code listed twice (`X` may
  repeat) and a list without `FL` are errors. The messages name the offending entry.

With `brir_recording_file` the folder's speaker recordings are not read (log `cli_brir_recording_replaces_folder`
with their count when there are any), as with `brir_measurement_file`. Every other input and all outputs use the
recording folder.

## Without `brir_recording_sweep`: the folder method

The file's first two tracks are read exactly as a folder recording whose name is the list: the same call
(`Hrir::open_recording_samples(estimator, fs, tracks, names, None, 2.0)`), so for a two-track file the result is
byte-identical to renaming it to `<list>.wav` and putting it in an otherwise empty folder. Later tracks are
ignored (a folder recording would take each further pair of tracks as further speakers), and a file with fewer
than two tracks is an error, as it is with a sweep. `X` entries are skipped as they
are in `FC,X.wav`. `LFE` and `LFE2` entries are ingested like speakers and then moved to the LFE list (see
"Measured LFE").

Test signal `auto` (or unset) and no `test.wav` in the folder: the sweep length is detected from this file with
the per-file rule of folder detection (`sweep_grid`), using the number of list entries as the sweep count. A
high-confidence result that is not the default sweep generates that sweep; otherwise the bundled default is used.
The folder itself is not scanned.

After ingestion every listed speaker that is not `X` must have both ears. Otherwise the job fails with
`BrirError::Unsupported` and a message that states how many of the listed sweeps were read, and that a recording
made with another sweep needs `brir_recording_sweep`.

## With `brir_recording_sweep`: locate and divide

All steps are sequential and deterministic (no parallel reduction). `fs` is the recording's sampling rate; the
sweep file must have the same rate (`BrirError::Unsupported` otherwise). Only the first two tracks of the
recording are used; fewer than two is an error.

### Reference sweep

1. `x` = the sweep file's first track. `k = floor(0.05 * fs)`.
2. `a[n] = mean(|x[n..n+k]|)` for `n = 0 ..= len(x) - k` (a prefix sum; an `x` shorter than `k` is an error).
3. Active windows are those with `a[n] > 0.01 * max(a)`. `r = x[first .. last + k]`. `len(r) < 0.5 * fs` is an
   error ("not a sweep").

`r` is used as stored, without level normalisation: a loopback recording of the file deconvolves to a unit
impulse, as it does with the Farina inverse filter.

### Spectrum, sweep type and band

1. `N` = the smallest power of two that is at least `len(recording) + len(r)`. `X = rfft(r, N)`,
   `P[k] = |X[k]|^2`, `f[k] = k * fs / N`.
2. `Q[k] = P[k] * f[k]` (flat inside the band of a logarithmic sweep).
3. `q_ref` = the median of `Q` over bins with `200 <= f <= 2000`.
4. On the grid `g[j] = 10 * 2^(j/24)` Hz, for every `j >= 0` with `g[j] <= fs/2`:
   `q[j]` = the mean of `Q` over bins with `g[j] * 2^(-1/12) <= f < g[j] * 2^(1/12)` (the bins that exist: the
   last bands are cut at Nyquist); a band without bins has `q[j] = 0`.
5. Sweep type: every `q[j]` with `200 <= g[j] <= 2000` must lie within ±3 dB of `q_ref`
   (`10*log10(q[j]/q_ref)`). Otherwise the file is not a logarithmic sweep that covers 200–2000 Hz and the job
   fails with that message.
6. Band: start at the grid point nearest to 632.46 Hz and extend down and up while
   `q[j] >= q_ref * 10^(-1)`. `f_lo` and `f_hi` are the lowest and the highest grid frequencies reached.
7. Band window `W[k]`, a half-Hann in log2 frequency over a third of an octave outside each edge:
   - `f = 0`: 0.
   - `t_lo = clamp(3 * log2(f / (f_lo * 2^(-1/3))), 0, 1)`, `t_hi = clamp(3 * log2(f_hi * 2^(1/3) / f), 0, 1)`.
   - `W = 0.5 * (1 - cos(pi * t_lo)) * 0.5 * (1 - cos(pi * t_hi))`.
8. Regularisation `eps[k] = 1e-6 * q_ref / max(f[k], f[1])`: 60 dB under the sweep's own pink trend.

Checked on a Dirac Live sweep the maintainer captured: the active part is found, `Q` over 200–2000 Hz stays far
inside the ±3 dB limit, and the band reaches from the grid start to just under Nyquist.

### Sweep positions

1. For each ear `e`: `Y_e = rfft(y_e, N)`, `c_e = irfft(Y_e * conj(X), N)`.
2. `E[n] = c_L[n]^2 + c_R[n]^2` for `0 <= n < len(recording)`.
3. Greedy picks: take the first index of the maximum of `E`; the first pick's value is `E_top`. Stop when the
   maximum is below `1e-3 * E_top` (30 dB). Otherwise record the index and set `E` to zero on
   `[i - D, i + D]` with `D = floor(0.9 * len(r))`, then repeat.
4. Sort the picks. Their number must equal the number of list entries (`X` included). Otherwise the job fails
   with a message that gives both numbers and the pick times in seconds (two decimals).
5. Two picks closer than `len(r)` are an error (overlapping sweeps).

The count is never forced to the list: with one entry too few, the weakest sweep (usually a subwoofer) would drop
out silently and every later speaker would shift.

Checked on two recordings of the maintainer: every sweep was picked, and the gap between the weakest sweep (a
subwoofer) and the next candidate was well over the 30 dB threshold on both sides.

### Impulse responses

1. `d_e = irfft(W * Y_e * conj(X) / (P + eps), N)`, once per ear for the whole recording. Never divide a
   segment per sweep: the truncated next sweep smears into the response.
2. `pre = min(floor(0.1 * fs), first pick)`.
3. `T` = the smallest of: every spacing between consecutive picks minus `floor(0.25 * len(r))` (the next sweep's
   harmonic distortion arrives that early), `N - last pick`, and `floor(3.0 * fs)`. `T < floor(0.2 * fs)` is an
   error (sweeps too close).
4. Entry `i` with pick `p_i` that is not `X`: both ears' responses are `d_e[p_i - pre .. p_i + T]` with
   `recording: None`.
5. The HRIR has the recording's rate and is resampled to the job's rate when they differ, as a measurement file
   is.

The job's estimator is not used for these responses. As with `brir_measurement_file`, test signal `auto` does
not scan for a sweep and uses the bundled default (it still serves the headphone recording and the tail fade).

Log (info) `cli_brir_recording_loaded` with `file`, `count` (speakers read, LFE included), `sweeps` (found),
`seconds` (sweep length, two decimals), `low` and `high` (band edges in Hz, rounded). Then the per-speaker time
differences and the 1 ms and all-zero warnings of the measurement-file path (`log_itd`), for the speakers only.

Checked by a manual run on the maintainer's recording (not a committed fixture; its numbers are not published):
every sweep was found, the speakers and both subwoofers were read, and the time differences taken from the
largest sample of each ear agreed with an independent analysis. The job log's list is `log_itd`'s, which takes
the first peak at least 0.126 of the largest in each ear, so it can read differently from a largest-sample
figure for the same responses.

### Errors

A failure a user can cause names its cause, with the numbers in the IPC error's `details` so the screen can
say it in the page language; the CLI prints the English sentence with the numbers.

| Cause | IPC message | `details` |
| --- | --- | --- |
| The number of sweeps found differs from the list (or none was found) | `The recording does not hold the listed number of sweeps.` | `path`, `found`, `listed`, `starts_seconds` |
| Without a sweep file, fewer listed sweeps could be read than listed | `The test signal does not match the recording file.` | `path`, `read`, `listed` |
| The sweep file has no active part, is too short, or is not a logarithmic sweep over 200–2000 Hz | `The sweep file is not a logarithmic sweep.` | `path` |
| The sweep file and the recording have different sampling rates | `The sweep file and the recording have different sampling rates.` | `path`, `sweep_fs`, `recording_fs` |

## Measured LFE

`LFE` and `LFE2` are not slots (`layouts::SLOTS` stays at 40). They are accepted where measurements are assigned:

- the list of a single recording;
- `brir_measurement_map` (`LFE.left=ID,LFE.right=ID`, same for `LFE2`); both ears are required like any slot.
  `room_measurement_map` and `headphone_measurement_map` reject them.
- Automatic BRIR assignment of a measurement file: Dirac speakers flagged as subwoofers go to `LFE`, then `LFE2`,
  in project order (a third is left unassigned); a REW title with a subwoofer word and an ear word goes to `LFE`,
  or to `LFE2` when the word ends in `2`. The canonical map lists `LFE` and `LFE2` after the slots.

The service hands them to the pipeline apart from the speakers (`PipelineInputs.lfe`, empty by default). They
take no part in the FL requirement, the speaker count, virtual-bass planning or the ITD log.

Processing, in pipeline order:

| Stage | Measured LFE |
| --- | --- |
| Head crop | Cropped like a speaker with no speaker delay: start at the earlier ear's peak (`peak_index(0, None, 0.12589)`) minus `head_ms`, the same Hann fade-in. |
| ITD report and correction, speaker alignment | Not applied. |
| Tail crop | Truncated (or zero-padded) to the speakers' tail length, with the same fade-out window. |
| Room correction (`eq`, `tuning`, `tuning2`), virtual bass, microphone deviation | Not applied. |
| Equalisation | The headphone, EQ and target terms of the same stage; no room term. Runs whenever the stage runs. |
| Decay, channel balance | Not applied. |
| Normalisation | The gain computed from the speakers is applied; the LFE does not enter the computation. |
| Resampling | Resampled with the speakers, then the speakers' second gain is applied. |

Output:

- Format files (`layout_files`): an `LFE`/`LFE2` track uses the measured channel of that name; if only one of the
  two was measured, both tracks use it. A measured channel takes precedence over `lfe_mode`. `filled` reports the
  source as `measured`.
- `hesuvi.wav`, `hrir.wav`, `responses.wav`, the TrueHD layouts, JamesDSP and Hangloose are unchanged.
- The job log and the README name the measured LFE with the existing LFE lines (`cli_info_lfe_filled`,
  `cli_readme_lfe_filled`) and the source text `cli_lfe_source_measured`.

## Deliberately out of scope

- Linear sweeps, MLS and other signals.
- Room correction of the LFE, and using a room measurement's subwoofer (an omnidirectional microphone) for the
  LFE track.
- Recovering the LFE from `responses.wav`.
