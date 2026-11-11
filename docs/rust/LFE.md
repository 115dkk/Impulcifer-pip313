# LFE tracks

The canonical specification of the LFE tracks of the format files (`layout_files`). ADR 0013 added the synthetic
fills (`lfe_mode`), ADR 0016 the subwoofer measured at the ears, ADR 0017 the subwoofer of the room measurement,
the shared timing rule, the LFE band limit and the LFE room correction, ADR 0018 the sum of two subwoofers for a
format with one LFE channel. `hesuvi.wav`, `hrir.wav`,
`responses.wav`, the TrueHD layouts, JamesDSP and Hangloose never carry an LFE track.

With no `LFE`/`LFE2` in any measurement map (folder, single recording, BRIR file, room file), every output is
byte-identical to 3.18.

## Sources and priority

For each slot S in `LFE`, `LFE2`, in this order:

1. **In-ear.** A pair named S read with the BRIR measurements (`brir_recording_speakers`, `brir_measurement_map`;
   `docs/rust/SINGLE_RECORDING.md`). Label `measured`, or `measured_corrected` when a room correction for S exists
   (see "Room correction").
2. **Room.** Otherwise, a room pair named S (below) and an available level transfer: the room pair scaled by the
   level transfer. Label `room`. A room-derived track is always corrected.
3. Otherwise no measured track for S.

A format file's LFE target S takes, in this order: the sum of both measured tracks when the format has one LFE
channel (next section), the measured track S, the other slot's measured track (`LFE` falls back to `LFE2` and the
other way round), the `lfe_mode` synthesis (`none`, `center`, `all`, unchanged). The source reported for the
target is the label of the track actually used.

## One LFE channel, two subwoofers (3.20.0, ADR 0018)

When a format file has exactly one of the targets `LFE`/`LFE2` (every format but NHK 22.2 and DTS:X Pro 30.2,
which have both) and measured tracks exist for both slots, each with both ears, that target's track is the sum
of the two tracks: for each ear, `out[n] = LFE[n] + LFE2[n]`. The tracks summed are the finished ones that
`fill_output` receives (onset placed, band-limited, equalised and corrected each on its own, the speakers' gain,
resampled, length-matched), so the two subwoofers arrive together and no gain is applied to the sum. The two
tracks must have the same rate and length (an error otherwise). The reported source is
`sum:<label of LFE>+<label of LFE2>`. A format with both targets keeps the tracks apart.

Room pairs exist only when `do_room_correction` is on and the room measurement comes from `room_measurement_file`
(the folder's `room-*.wav` names have no subwoofer).

## Room measurement assignment

- `parse_map_for(.., Purpose::Room)` accepts `LFE` and `LFE2` with the room forms: `LFE=ID` (both ear
  positions), `LFE.left=ID`, `LFE.right=ID`. `Purpose::Headphones` still rejects them.
- Automatic Dirac room assignment: speakers flagged as subwoofers, in project order, take `LFE` then `LFE2` (a
  third is left unassigned) at the room position the speakers use (position 1, else the lowest), for both ear
  positions, when that measurement exists and is usable.
- Automatic REW room assignment: a measurement title with a subwoofer token (`is_subwoofer_name`) takes `LFE`, or
  `LFE2` when the token ends in `2`. An ear word gives that ear position; without one, both. A slot and side
  already taken is skipped, as for speakers.
- `format_map_for(.., Purpose::Room)` lists `LFE`, then `LFE2`, after the slots, in the room form (`LFE=ID` when
  both sides use one id).
- `inspect_room_measurements` with `purpose: room` reports a subwoofer speaker's automatic `slot` (`LFE`/`LFE2`)
  as it does for `brir`. `headphones` keeps `null`.
- `load_room` returns the speakers and the LFE pairs apart. Room correction (all modes) sees the speakers only.
  An LFE pair assigned on one side only uses that measurement for both sides.

## Onset

The onset of an impulse response `h` (one ear):

1. `p` = `peaks::first_peak_index(h, 0, None, 0.12589)` (the rule the speaker alignment uses).
2. `o` = `p`; while `o > 0` and `|h[o-1]| >= 0.1 * |h[p]|`, decrease `o`.

The onset of a pair is the smaller of its two ears' onsets.

## Level transfer

Used only to make a room-derived track. Inputs: the opened BRIR speakers (uncropped, as opened), the room
speakers (uncropped, resampled to the job's rate, before room correction modifies them) and the prepared
microphone calibration (`room::prepare_mic_calibration`, or none).

For every speaker slot that the BRIR has with both ears and the room has (a room speaker with one side counts for
both), and for each ear e:

1. `B` = the BRIR ear e, `R` = the room response for ear e.
2. Segments: each starts at its own onset minus `round(0.005 * fs)` samples (clamped at 0) and is
   `min(fs / 2, samples left)` long.
3. Spectra: `fr::magnitude_to_frequency_response` of each segment, on the standard grid (10 Hz to Nyquist, ratio
   1.01). Subtract the calibration from the room spectrum. Smooth both by 1/6 octave (`room_v2::smooth`).
4. Level: `L = 10 * log10(mean of 10^(dB/10) over the grid points with 100 <= f <= 250)`.
5. `d[s][e] = L_B - L_R`.

`K_e` = the median of `d[·][e]` (`room_v2::median`). `spread_e` = the 75th minus the 25th percentile of
`d[·][e]` (linear interpolation between closest ranks, NumPy's default). The transfer is available when at least
3 speakers took part. The room-derived pair is the room pair with ear e multiplied by `10^(K_e / 20)`, made in the
service before the pipeline runs.

## Room correction

Computed when room correction runs, for every room pair S and each ear e, from the room response of that ear
(uncropped, resampled, as loaded):

1. Segment from the onset minus 5 ms (clamped at 0) to the end. `fr` = its `magnitude_to_frequency_response`;
   subtract the calibration from `fr.raw`.
2. `snr` = `room_v2::snr_db(response, grid)` (unavailable below one second, as for room EQ).
3. Low end: `r` = `room_v2::detect_rolloff(grid, fr.raw, snr)`. The rolloff used has `freq = max(r.freq, 16)` and
   `knee = max(r.knee, 20)`; without `r` it is `freq = 16`, `knee = 20`. `lo = knee`.
4. High end: `s3` = `fr.raw` smoothed by 1/3 octave. `P` = the median of `s3` over `30 <= f <= 80`. `hi` = the
   lowest grid frequency `f` with `40 <= f <= 120` such that `s3(g) <= P - 6` at every grid point `g` in
   `[f, 2f]`; if there is none, `hi = 120`.
5. Error: `fr.compensate(room_target, &CompensateOptions::default())` (error = raw − target, the target centred at
   1 kHz). `c` = the median of the error smoothed by 1/6 octave over `lo <= f <= hi / sqrt(2)` (0 if there is no
   grid point). Subtract `c` from `fr.error` and `fr.error_smoothed`.
6. Gain: `room_v2::correction_gain(&mut fr, snr, Some(rolloff), hi, false, fs, &options)` with
   `options = RoomCorrectionOptions { range: Schroeder, max_boost_db: 6.0, vbass_crossover: None, ..Default }`.
   With these options cuts reach 24 dB (1/12-octave view), boosts 6 dB (1/6-octave view), no boost below the
   rolloff (half-Hann rise to the knee), and everything fades to zero over `[hi / sqrt(2), hi]` and is zero above.

The pipeline receives, per slot S that has a track and a room pair, and per ear, a `RoomFrs { term: Gain }` entry
named S whose `equalization` is the gain above, minus the calibration at every grid point when the track of S is
room-derived. An in-ear track without a room pair of its slot gets no entry (logged when room correction runs).

## Pipeline

`PipelineInputs.lfe` is an `LfeInputs { pairs, labels, correction }`: the tracks (pairs named `LFE`/`LFE2`, both
ears), a label per slot (`measured`, `measured_corrected`, `room`) and the correction entries (`Option<RoomFrs>`,
term `Gain`). The default is empty. Stages:

| Stage | LFE |
| --- | --- |
| Crop and align | After the speakers' alignment (`align_onset_groups_peak_leftref`, `itd::measure_after`): `t` = the first peak (rule above) of FL left. Each pair is moved so that its onset lands at `t`, both ears by the same amount, with `ImpulseResponse::shift` (the length is kept: zeros enter at the start when delayed, at the end when advanced). The first `round(head_ms * fs / 1000)` samples get the rising half of a Hann window of twice that length, as `Hrir::crop_heads` does. Then the speakers' tail length (`crop_tails_to`), then the band limit. |
| Band limit | Each ear filtered twice with `filters::butter(2, 120 / (fs / 2), Lowpass)` through `filters::sosfilt`: the 4th-order Linkwitz-Riley low-pass at 120 Hz. Always, whatever else is enabled. |
| ITD report and correction, speaker alignment, room correction of the speakers, virtual bass, microphone deviation, decay, channel balance | Not applied. |
| Equalisation | `equalize_hrir` on the LFE pairs with `room_frs = correction`, and the same headphone, EQ, target and rate as the speakers. Runs whenever the stage runs. |
| Normalisation and resampling | As 3.18: the speakers' gain, resampled with the speakers, the speakers' second gain. |
| Output | `fill_output` takes the pairs and their labels; the reported source is the label of the pair used. |

`Hrir::crop_heads_without_speaker_delay` (3.18) is removed.

## Logs and README

| Key | Level | Arguments | When |
| --- | --- | --- | --- |
| `cli_room_lfe_created` | info | `slot`, `count` | a room-derived track is made |
| `cli_room_lfe_level_spread` | warning | `slot`, `spread` (dB, one decimal, the larger ear) | `spread > 3` |
| `cli_room_lfe_no_level` | warning | `slot`, `count` | a room pair without an in-ear track, fewer than 3 shared speakers |
| `cli_room_lfe_correction` | info | `slot`, `low`, `high` (Hz, rounded; the lower `lo` and the higher `hi` of the two ears) | a correction entry is made |
| `cli_room_lfe_uncorrected` | info | `slot` | room correction runs and an in-ear track has no room pair |

The existing `cli_info_lfe_filled` and `cli_readme_lfe_filled` show the source with `cli_lfe_source_measured`
(`measured`), `cli_lfe_source_measured_corrected` (`measured_corrected`) or `cli_lfe_source_room` (`room`). A
summed track (`sum:a+b`) shows `cli_lfe_source_sum` with `first` and `second` set to the phrases of `a` and `b`.

## Out of scope

- Restoring the arrival-time difference between two subwoofers, correcting the sum again, or scaling the sum
  (ADR 0018).
- Other LFE low-pass frequencies than 120 Hz.
- Subwoofers in the folder's `room-*.wav` recordings.
- Using the room measurement's time axis to place the subwoofer later than the speakers (ADR 0017).
