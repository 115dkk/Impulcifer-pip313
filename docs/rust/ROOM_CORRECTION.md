# Room correction v2

The Rust room stage defaults to `schroeder`. The Python 2.x implementation is unchanged. `room_range=legacy` dispatches to the original room calculations, including their full-Hann limit mask, and produces `RoomTerm::LegacyError`. Oracle tests explicitly select legacy; they do not regenerate goldens or relax tolerances.

## Configuration

| Option | Default | Accepted values |
| --- | --- | --- |
| `room_range` | `schroeder` | `legacy`, `modes`, `schroeder`, `extreme` |
| `room_volume` | unset | cubic metres, greater than 0 and at most 10000 |
| `schroeder_freq` | unset | override in Hz, 50–1000 inclusive |
| `room_max_boost` | 12 | dB, 0–24 inclusive |

These are extension fields, not additions to the frozen 33-field 2.x list. CLI defaults are suppressed: omitting the flags preserves the old kwargs object. The service bootstrap includes all four defaults. Service validation returns `INVALID_REQUEST` with `details.field`; CLI validation exits 2. `specific_limit` and `generic_limit` apply only in legacy mode.

## Measurement preparation

Calculations use the existing logarithmic grid: 10 Hz to Nyquist, ratio 1.01. Microphone calibration is subtracted first. Specific responses reuse the first response's 100 Hz–10 kHz centring gain. Generic tracks are centred individually and use the existing average/conservative combination, with the correction limit set to zero. The unmasked error is measured minus target. Positive gain means boost.

Specific SNR is computed after head cropping but before tail cropping. Generic SNR uses the split IRs. With `L = min(length/4, fs)`, the first and last L samples give signal and noise spectra. Both use identical rectangular windows, the existing IR-to-response mapping, and constant 1/6-octave smoothing. Their dB difference is SNR. Rectangular windows avoid suppressing the direct impulse near the start. IRs shorter than one second have unavailable SNR, skip this cap and generate a warning. Exact silent segments use a finite −600 dB floor. Generic correction uses the minimum SNR of its tracks; if any track lacks SNR, the combined SNR cap is unavailable.

## Schroeder estimate

Use all specific IRs, or generic IRs if no specific measurements exist. For octave centres 125, 250 and 500 Hz, apply order-4 Butterworth high-pass at `centre/sqrt(2)` and low-pass at `centre*sqrt(2)`. Cutoffs passed to `butter` are divided by Nyquist. Skip a band whose upper edge reaches Nyquist.

The existing `decay_times` helper's `rt30` and `rt20` are elapsed times for 30 and 20 dB, not extrapolated T60. Multiply by 2 and 3 respectively, preferring rt30; retain T60 estimates in 0.05–3.0 seconds and take their median.

- Override: clamp to 50–1000 Hz; source `Override` (takes precedence).
- Known T60 and supplied volume V: `2000*sqrt(T60/V)`, clamped to 80–500 Hz; source `Volume`.
- Known T60 without V: assume 50 m³ and clamp to 120–300 Hz; source `AssumedVolume`.
- No usable T60 and no override: 200 Hz; source `Fallback`.

## Low-frequency rolloff

Smooth the calibrated, centred measured response by 1/6 octave. Scan candidate knees from 160 Hz down to 20 Hz in 1/12-octave steps. Fit dB versus log2 frequency in the lower octave `[fc/2,fc]`. A high-pass slope is positive.

Accept the highest candidate with slope at least +9 dB/octave whose lower-frequency samples are all at least 6 dB below its level. Only samples with SNR at least 10 dB participate when SNR is available. Require at least one participating sample; an empty set is not evidence of a rolloff. The candidate level uses the grid sample immediately below fc.

The plateau is the median over `[knee,2*knee]`. The rolloff is the highest grid frequency at or below the knee with response at most plateau−6 dB, or the knee if none exists. Enforce `knee >= rolloff*2^(1/4)`. This is source `Slope`.

If slope detection fails, smooth SNR by 1/3 octave. A contiguous sub-10-dB region starting at the lowest grid frequency yields a rolloff at its upper edge, capped at 160 Hz, and a knee at rolloff·sqrt(2), source `Snr`. Otherwise there is no rolloff. Generic measurement rolloff is the highest detected rolloff across its split IRs.

Reference checks cover order-2 50 Hz and order-4 45 Hz analog high-pass responses, with and without `8/(1+(f/100)^2)` dB boundary gain, and an isolated −12 dB, 1/6-octave dip at 35 Hz. The dip alone must not be classified as rolloff.

## Ranges and gain

Constant fractional-octave smoothing uses the existing quadratic Savitzky–Golay implementation.

- **Modes:** upper frequency `f_hi=f_S/sqrt(2)`. Preserve the bass balance and remove resonances. The narrow view is 1/24 octave; boosts use a further 1/6-octave smoothing of the resonance error.
- **Schroeder:** `f_hi=f_S`. Error is the full measured-minus-target curve. Cuts use 1/12 octave and boosts 1/6 octave.
- **Extreme:** `f_hi=10000 Hz`. Below f_S use Schroeder smoothing. Above it, interpolate log2 smoothing width versus log2 frequency through `(f_S,1/12)`, `(1000,1/3)`, `(4000,1)`, constant above 4 kHz. Precompute widths 1/12, 1/6, 1/3, 1/2 and 1 octave and interpolate adjacent curves in log2 width. If f_S is 1000 Hz, the 1 kHz upper-band anchor wins above the boundary.

### Modes specification conflict

A literal `S_(1/24)(e) - S_1(e)` with the existing quadratic smoother cuts the specified +8 dB Q=5 peak at 60 Hz on a +6 dB shelf by only 2.91 dB, conflicting with the required cut of at least 6 dB. To retain the stated purpose and numerical acceptance test, modes uses a robust median bass baseline over `[f/2,2*f]` instead of quadratic `S_1`. It subtracts this baseline from the 1/24-octave view. The test checks the Q=5 peak and a change at 30 Hz no larger than 1.5 dB. This is a deliberate deviation, not a claim of equivalence to the written smoothing equation.

Before caps, `G=-c` for positive cut view c; otherwise `G=max(-b,0)` for boost view b.

| Cap | Value |
| --- | --- |
| Cuts below f_S | 24 dB |
| Extreme cuts above f_S to 1 kHz | 12 dB |
| Extreme cuts above 1 kHz | 6 dB |
| Specific boosts up to f_S | max_boost |
| Specific boosts f_S to 1 kHz | min(6,max_boost) |
| Specific boosts above 1 kHz | min(3,max_boost) |
| Modes specific boosts | min(6,max_boost) everywhere |
| Generic average boosts | 0 (cut only) |
| Generic conservative boosts | min(3,max_boost) |
| Generic boosts above f_S/sqrt(2) | fade to 0 |

Every cap step is interpolated linearly in log2 frequency across `[f0*2^(-1/4), f0*2^(1/4)]`. The generic zero-boost boundary uses this same continuous transition; zero is reached at its upper transition edge, rather than an abrupt step at f0.

The rolloff multiplier affects only boosts: zero below rolloff, a rising half-Hann up to the knee, then one. SNR further limits boosts to `max(0,SNR−20)` dB. Cuts remain available below rolloff.

The upper fade is a falling half-Hann from `f_hi/sqrt(2)` to f_hi, except extreme uses 5–10 kHz. The log-frequency half-Hann is `0.5*(1+cos(pi*t))`, with t clamped to [0,1].

For extreme, blend final left/right gains per speaker toward their mean using a rising half-Hann from 500 to 700 Hz. Above 700 Hz they are identical, including the effect of different SNR caps. This happens after caps, upper fade and virtual-bass hand-off, not before smoothing.

## Virtual-bass hand-off

The room gain is multiplied by the magnitude response of the exact LR8 high-pass used by virtual bass: order-4 Butterworth high-pass duplicated twice. The shared SOS constructor leaves virtual-bass synthesis unchanged. Below crossover/sqrt(2) the mask is explicitly zero. At crossover times `2^(-1/4),1,2^(1/4),sqrt(2),2`, the mask is approximately 0.20, 0.50, 0.80, 0.94 and 0.996 (test tolerance 0.02). Invalid or Nyquist-exceeding cutoffs return the identity mask, matching virtual bass's existing early-return behaviour at Nyquist.

## Equalization, plots and self-check

V2 returns `RoomTerm::Gain`: each entry has centred measured raw response, target, equalization G, and error −G. Legacy retains its original error entries. In the equalize stage, v2 room gain bypasses heavy/light error smoothing and is subtracted from both `error` and `error_smoothed` immediately before `equalize`. The global 40 dB ceiling remains unchanged. Room plots continue to consume the error representation; no plotting crate was changed.

Residual is `x+G` (mode resonance error for modes, original error otherwise), smoothed by 1/6 octave. Compute RMS across logarithmic grid points from `max(20,knee,crossover*sqrt(2))` to `f_hi/sqrt(2)`. Omit absent terms from the lower bound and skip an empty interval. A result above 3 dB emits a warning. Extreme residuals use the final diotic gains.

## Logs

No new keys are emitted by legacy. Placeholder formatting is done by the service and translations remain catalog-owned.

| Level | Key | Arguments |
| --- | --- | --- |
| info | cli_room_range | range, f_hi (integer Hz) |
| info | cli_room_schroeder | freq (integer Hz), t60 (2 decimals), volume (1 decimal) |
| info | cli_room_schroeder_assumed | freq, t60 |
| info | cli_room_schroeder_override | freq |
| warning | cli_room_schroeder_fallback | freq |
| info | cli_room_rolloff | speaker, side, freq |
| info | cli_room_rolloff_snr | speaker, side, freq |
| warning | cli_room_snr_unavailable | speaker, side |
| warning | cli_room_selfcheck_warning | speaker, side, rms (1 decimal), lo, hi (integer Hz) |
| info | cli_room_vbass_handoff | freq |

Sides are `left`, `right` or `both`; generic diagnostics use speaker `room.wav` and side `both`.

## Why not invert everything?

A recorded BRIR is fixed at the measured point, so usable low-frequency nulls can be filled in a way that ordinary loudspeaker room EQ cannot safely maintain across listener positions. That does not make a signal below the measurement rolloff usable: it is mostly unavailable measurement energy or noise, not information to amplify. The headphone can reproduce low bass, but the room recording cannot establish its correction there. Virtual bass supplies synthetic bass instead, so room modes must not be imposed onto it.

Above the room-mode/Schroeder region (often a few hundred Hz, not a universal fixed 400 Hz cutoff), interference varies rapidly in space. The half-wavelength spatial period is about 57 cm at 300 Hz and 17 cm at 1 kHz. SNR and increasingly broad smoothing therefore restrict boosts. The omni microphone has no head shadow: its inter-ear differences at higher frequencies must not create artificial binaural level differences. Extreme mode's final diotic blend enforces this while retaining broad tonal correction.
