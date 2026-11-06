# Interaural alignment: ITD report, correction and the left/right balance check

Specification for ADR 0014. The Rust pipeline measures the interaural time
difference (ITD) of every speaker, reports it, and optionally corrects it.
The report also carries the left/right level balance of the recording chain
and, with headphone compensation, the balance of the final BRIR it predicts.
With `itd_mode = off` (the default) every output stays byte-identical.

Sign conventions, used everywhere in this document, in the code, in the job
result and in the catalog texts:

- **ITD**: positive when the sound reaches the left ear first (a left speaker
  has a positive ITD). Microseconds in reports, milliseconds in this text.
- **Azimuth**: ITU, positive to the left (`impulcifer_types::layouts::SLOTS`).
- **Yaw**: positive when the head is turned to the left, so every speaker's
  relative azimuth is `azimuth − yaw`.
- **Level differences**: left minus right, in dB.

## Configuration

| Option | Default | Accepted values |
| --- | --- | --- |
| `itd_mode` | `off` | `off`, `symmetric`, `model` |
| `head_radius` | unset | centimetres, 5–15 inclusive; unset means estimated |

Both are 3.x extension fields (`EXTENSION_FIELD_NAMES`, `EXTENSION_OPTIONS`,
`CANONICAL_CONFIG`), CLI `--itd_mode` and `--head_radius` with suppressed
defaults, validated like `missing_speakers` (service `INVALID_REQUEST` with
`details.field`, CLI exit 2). `head_radius` is read in both modes (the model
ITD column and the yaw estimate use it) but only `model` corrects towards it.

## The ITD estimate (one implementation)

`impulcifer_dsp::itd::estimate(left: &[f64], right: &[f64], fs: u32, max_lag_ms: f64) -> Option<ItdEstimate>`
with `ItdEstimate { itd_us: f64, inverted: bool }`. It is the function that
`crates/impulcifer-service/src/recording/checks.rs::estimate_itd_from_ir`
holds today, moved without changing a number; the recording checks call it
with `max_lag_ms = 1.0` and the pipeline with `1.5`.

1. Both ears through a 2nd-order Butterworth low-pass at 1.5 kHz (`filters::butter`,
   `sosfilt`), as today.
2. Direct arrival `p`: the first sample of either filtered ear whose magnitude
   reaches half of the larger ear's maximum (`direct_arrival`, as today).
3. Window `[p − 2 ms, p + 3 ms]` (clamped to the data).
4. For every integer lag in `[−L, L]`, `L = round(max_lag_ms · fs / 1000)`:
   `c(lag) = Σ left[i] · right[i + lag]` over the window (indices outside the
   window skipped, as today). Take the lag with the largest `|c|`.
5. Parabolic interpolation on `|c|` at the three lags around the maximum, as
   today; `itd_us = (lag + fraction) · 10⁶ / fs`; `inverted = c(lag) < 0`.

Positive `itd_us` means the right ear is later, i.e. the left ear hears the
sound first.

## Band levels

`impulcifer_dsp::itd::band_level_db(ir: &[f64], fs: u32, lo_hz: f64, hi_hz: f64) -> Option<f64>`:
`10·log10` of the mean of `|rfft(ir, n)|²` over the bins with `lo ≤ f ≤ hi`,
`n = next_fast_len(ir.len())` (`fft::next_fast_len`), `None` when the band
holds no bin or the mean is 0. No windowing.

Per speaker and ear the report uses two bands:

- **Low-frequency level**, 30–120 Hz, on the whole IR as it is at the analysis
  point (uncropped tail). The head does not shadow this band, so the two ears
  must agree; what remains of their difference is the recording chain (input
  gains, microphone sensitivity).
- **High-frequency level**, 4–12 kHz, on the first 100 ms of the IR (direct
  sound and early reflections).

`lf_ild_db` and `hf_ild_db` are left minus right of those levels.

## Pairs and centre slots

From `SLOTS`: a **mirror pair** is two measured slots with
`azimuth_R = −azimuth_L`, equal elevation, `azimuth_L > 0`, `|azimuth| ≠ 180`.
A **centre slot** is a measured slot whose lateral angle is below 5°
(`lateral = asin(|sin azimuth| · cos elevation)` in degrees; FC, BC, TC, HFC,
HBC, DFC). Every other measured slot is **lateral**, paired or not.

The lateral angle `θ` of a slot, in radians, is
`asin(|sin azimuth| · cos elevation)`.

## The spherical head model

Woodworth: `itd_model(θ) = (r / c) · (θ + sin θ)` with `c = 343 m/s` and `r`
the head radius in metres; the sign is the sign of the azimuth (left
positive), and centre slots are 0. Its derivative with respect to the
azimuth, needed for the yaw estimate, is

```
d itd / d azimuth = (r / c) · (1 + cos θ) · (cos azimuth · cos elevation) / cos θ
```

(`cos θ` is never 0 for a lateral slot with elevation; for an ear-level
slot at ±90° the derivative is 0 and that pair is skipped below).

## The analysis

`impulcifer_dsp::itd::analyze(hrir: &Hrir, head_radius_cm: Option<f64>, max_lag_ms: f64) -> AlignmentReport`
runs on the HRIR at the analysis point. Measured speakers are the slots with
both ears present.

1. Per speaker: `measured_us = estimate(left, right, fs, max_lag_ms)`
   (`None` when the estimate fails; the speaker is then left out of every
   statistic below), `lf_ild_db`, `hf_ild_db`.
2. **Input balance** `input_balance_db`: the median of `lf_ild_db` over the
   measured speakers (lower median for an even count); `None` with no
   speakers.
3. **Common offset** `common_offset_us`: the mean of `measured_us` over the
   centre slots; `None` without a centre slot. `o` below is that value or 0.
4. **Head radius**: `head_radius_cm` when given (`source = "given"`).
   Otherwise, from the mirror pair with the largest lateral angle `θ_max`
   whose both members have an estimate: `t = (|ITD_L − o| + |ITD_R − o|) / 2`
   in seconds, `r = c · t / (θ_max + sin θ_max)`, clamped to 5–15 cm
   (`source = "estimated"`). With no such pair, 8.75 cm (`source = "default"`).
5. Per speaker `model_us = itd_model(θ)` with that radius and the slot's sign.
6. Per mirror pair: `sum_us = ITD_L + ITD_R` as measured (0 for a symmetric
   head facing the speakers with no common offset), and when
   `|d itd / d azimuth| ≥ 0.05 ms/rad` for the left member,
   `yaw_deg = −((sum − 2·o) / 2) / (d itd / d azimuth)` in degrees (the sum
   is in seconds for this formula and the derivative in s/rad; the common
   offset enters both members, so `2·o` is taken out before the yaw is
   read); otherwise `yaw_deg = None`. The sign follows the convention above: a yaw to the
   left makes the front-left speaker's ITD smaller and the front-right
   speaker's larger, so a negative front-pair sum is a positive yaw.
7. **Yaw** `yaw_deg`: the median of the pairs' `yaw_deg` (both front and back
   pairs enter with their own sign of derivative, so a real yaw adds up and
   a common offset cancels); `None` without a pair.
8. **High-frequency asymmetry** `hf_asymmetry_db`: the mean over mirror pairs
   of `((hf_ild_L − b) + (hf_ild_R − b)) / 2` with `b = input_balance_db`
   (0 when `None`); positive when the left ear is brighter. `None` without a
   pair.

Everything is sequential f64; no rayon.

## Correction

`impulcifer_dsp::itd::correct(hrir: &mut Hrir, report: &AlignmentReport, mode: ItdMode) -> Vec<Correction>`
with `Correction { speaker, target_us, applied_us, ear: Ear, skipped: bool }`.
`mode = Off` returns an empty list and touches nothing.

Targets per speaker (`o` = common offset or 0):

- `symmetric`: centre slots → 0. A mirror pair → `+t` for the left member and
  `−t` for the right with `t = (|ITD_L − o| + |ITD_R − o|) / 2`. An unpaired
  lateral slot → `ITD − o`.
- `model`: centre slots → 0; every lateral slot → `model_us` with the report's
  radius.

Application, per speaker with an estimate: `Δ = target − measured`
(microseconds). If `|Δ| > 1500`, skip (`skipped = true`, nothing changes).
Otherwise delay one ear by `|Δ|` and never advance: `Δ > 0` delays the right
ear, `Δ < 0` delays the left ear, `|Δ| < 0.5` (half a microsecond) does
nothing. A delayed ear's whole IR moves, direct sound and reflections alike.

`impulcifer_dsp::itd::fractional_delay(x: &[f64], fs: u32, delay_us: f64) -> Vec<f64>`,
`delay_us ≥ 0`, returns a vector of the same length: `d = delay_us · fs / 10⁶`
samples, `m = next_fast_len(x.len() + ceil(d) + 1)`, `X = rfft(x zero-padded to m)`,
`X[k] *= exp(−j · 2π · k · d / m)` for every bin, `irfft(·, m)` and the first
`x.len()` samples. Integer `d` must reproduce `ImpulseResponse::shift(d)` to
within 1e-9 of the peak (a test).

After the correction and after `align_ipsilateral_all` and
`align_onset_groups_peak_leftref` have run, every speaker is measured again
with the same estimator: `after_us`. The result for a corrected speaker must be
within 3 µs of `target_us` (a test on the demo set); the FC branch of
`align_ipsilateral_all` finds lag 0 once FC's ITD is 0, so it moves nothing.

## Where it runs

`StageKey::CropAndAlign` in `crates/impulcifer-dsp/src/pipeline.rs` becomes

1. `crop_heads(head_ms)`
2. `analyze(&hrir, config.head_radius, 1.5)` → `report` (measured values)
3. `correct(&mut hrir, &report, mode)` when `itd_mode ≠ off`
4. `align_ipsilateral_all`, `align_onset_groups_peak_leftref` (unchanged)
5. the `after_us` measurement into the report, then
   `observer.on_alignment(&report, &corrections)`
6. `crop_tails`, then the room tunings as today.

Step 2 measures the recording's own ITDs, before the integer alignment of
the centre slot. With `off`, steps 2 and 5 only read the HRIR.

## The pairing check (headphone compensation)

When `do_headphone_compensation` is on, the service measures the headphone
recording's balance on the compensation it already holds
(`HeadphoneCompensation { left, right }`, both `FrequencyResponse` on the same
grid; `right.raw` already carries the left's centring shift, so the difference
is the recording's): `headphone_balance_db` = the mean of
`left.raw − right.raw` over the grid points with 200 Hz ≤ f ≤ 2 kHz.

The final BRIR's balance is the speaker chain's divided by the headphone
chain's: `predicted_balance_db = input_balance_db − headphone_balance_db`.
The service logs `cli_itd_pairing` (info) with both numbers and the
prediction, and `cli_itd_pairing_warning` (warning) instead when
`|predicted_balance_db| ≥ 2`. Without headphone compensation both headphone
values are `None` and nothing is logged about them.

## What the service reports

`BrirEvents::on_alignment` logs, in this order:

| key | level | placeholders |
| --- | --- | --- |
| `cli_itd_report` | info | `{yaw}` (°, one decimal, `?` when None), `{offset_us}` (integer), `{balance}` (dB, one decimal) |
| `cli_itd_pairing` / `cli_itd_pairing_warning` | info / warning | `{speaker_db}`, `{headphone_db}`, `{predicted_db}` (one decimal) |
| `cli_itd_head_turned` | warning, only when `|yaw_deg| ≥ 3` | `{yaw}` |
| `cli_itd_corrected` | info, only with corrections | `{mode}`, `{count}`, `{max_us}` (integer, the largest `|applied_us|`) |
| `cli_itd_skipped` | warning, per skipped speaker | `{speaker}`, `{us}` |

The BRIR job result gains `alignment` beside `output_path`:

```json
"alignment": {
  "mode": "symmetric",
  "head_radius_cm": 11.2,
  "head_radius_source": "estimated",
  "yaw_deg": 4.4,
  "common_offset_us": -20,
  "input_balance_db": -6.5,
  "hf_asymmetry_db": 2.1,
  "headphone_balance_db": 7.0,
  "predicted_balance_db": -13.5,
  "speakers": [
    {"speaker": "FL", "measured_us": 288, "model_us": 231, "target_us": 298,
     "after_us": 298, "applied_us": 10, "lf_ild_db": -7.3, "skipped": false}
  ],
  "pairs": [
    {"left": "FL", "right": "FR", "sum_us": -21, "yaw_deg": 0.9}
  ]
}
```

Microsecond values are rounded to integers, dB and degrees to one decimal,
`None` is JSON `null`, `target_us`/`applied_us` are `null` with `off`,
`applied_us` is 0 for a skipped speaker, and `skipped` is false with `off`.
`model_us`, `head_radius_cm`, `head_radius_source` and `sum_us` are always
present. Speakers follow `SPEAKER_NAMES` order.

The result folder's README gets a section "Interaural time differences"
(`cli_readme_itd_title`) with one line per speaker
(`cli_readme_itd_row`: `{speaker}`, `{measured_us}`, `{model_us}`, `{after_us}`)
and the summary lines `cli_readme_itd_summary` (`{yaw}`, `{offset_us}`,
`{balance}`, `{radius_cm}`) and, with headphone compensation,
`cli_readme_itd_pairing` (`{speaker_db}`, `{headphone_db}`, `{predicted_db}`).

## Recording checks

`check_recording` (service) gains, per take and speaker, `lf_ild_db` (the same
band level on the deconvolved IRs it already has; `None` without an estimator
or with a silent ear) in `Measurement` and `ReferenceMeasurement` (an optional
field; the request validator accepts it).

- **Speaker takes**: `record_check_input_balance` (warning, once per take, on
  the first speaker) when the median of `lf_ild_db` over the take's usable
  speakers (snr ≥ 6 dB, both ears, not wrong side) has `|·| ≥ 3 dB`; values
  `{db_diff}` (one decimal, signed, left minus right).
- **Headphone takes** (`Mode::Headphones`, segments FL = left playback and
  FR = right playback): `headphone_balance_db` = the 200 Hz–2 kHz level of the
  FL segment's left-ear IR minus the FR segment's right-ear IR. With
  reference measurements that carry `lf_ild_db` (the folder's earlier speaker
  takes), `predicted = median(reference lf_ild_db) − headphone_balance_db`;
  `record_check_headphone_balance` (warning) when `|predicted| ≥ 2 dB`, values
  `{db_diff}` (the headphone balance) and `{predicted_db}`. The UI therefore
  sends `reference_measurements` for headphone recordings too (today it
  drops them for that mode).

The existing checks and their numbers do not change.

## Registration

- `crates/impulcifer-types/src/config.rs`: fields, defaults, `EXTENSION_FIELD_NAMES`
  (27 → 29), validation messages, `from_kwargs`; the length assert in
  `crates/impulcifer-types/tests/output_fill_config.rs`.
- `crates/impulcifer-cli/src/options.rs` (`EXTENSION_OPTIONS`), `--info`.
- `crates/impulcifer-policy/tests/gates.rs` (`CANONICAL_CONFIG`), `features.toml`
  (`config.itd_mode`, `config.head_radius`, `alignment.itd_report`,
  `alignment.itd_correction`, `recording.input_balance_check`).
- Service: `brir/validation.rs`, `brir/run.rs` (observer, pairing check,
  result), `brir/outputs.rs` (README), `recording/checks.rs`,
  `recording/request.rs`, `lib.rs` (`brir_defaults`).
- Catalogs: the nine overlay files, keys above plus the UI keys of the
  synthesis options and the result table.
- UI: `apps/impulcifer-app/ui/ipc.d.ts` (`ProcessingRequest`, `BrirDefaults`,
  the job result type, `ReferenceMeasurement.lf_ild_db`), `app.js`, gallery
  mocks in `apps/impulcifer-app/tests/ui_gallery.py`.
- `.github/scripts/brir_3x_integrity.py`: scenarios `itd_symmetric`
  (`--itd_mode=symmetric`) and `itd_model` (`--itd_mode=model --head_radius=8.75`),
  since 3.12.0.
