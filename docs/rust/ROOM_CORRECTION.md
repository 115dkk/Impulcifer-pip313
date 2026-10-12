# Room correction v2

The Rust room stage defaults to `schroeder`. The Python 2.x implementation is unchanged. `room_range=legacy` dispatches to the original room calculations, including their full-Hann limit mask, and produces `RoomTerm::LegacyError`. Oracle tests explicitly select legacy; they do not regenerate goldens or relax tolerances.

## Configuration

| Option | Default | Accepted values |
| --- | --- | --- |
| `room_range` | `schroeder` | `legacy`, `modes`, `schroeder`, `extreme` |
| `room_volume` | unset | cubic metres, greater than 0 and at most 10000 |
| `schroeder_freq` | unset | override in Hz, 50–1000 inclusive |
| `room_max_boost` | 12 | dB, 0–24 inclusive |

`room_mode` defaults to `eq` (`eq`, `tuning` or `tuning2`). Both tunings are independent of the range and accept `legacy`; see the processor specifications below (v1 under "Virtual room tuning", v2 under "Virtual room tuning v2").

These are extension fields, not additions to the frozen 33-field 2.x list. CLI defaults are suppressed: omitting the flags preserves the old kwargs object. The service bootstrap includes all eleven extension defaults (the eleventh, `vbass_mode`, is described under "Virtual bass v2"). Service validation returns `INVALID_REQUEST` with `details.field`; CLI validation exits 2. `specific_limit` and `generic_limit` apply only in legacy mode.

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

For extreme, blend final left/right gains per speaker toward their mean using a rising half-Hann from 500 to 700 Hz. Above 700 Hz they are identical. This happens after caps, upper fade and virtual-bass hand-off, not before smoothing. The blend must not undo a per-ear limit: each ear's final boost limit is its boost cap after the rolloff and SNR limits, times the upper fade and the hand-off mask, and the mean is first lowered to the smaller of the two ears' limits (`min(mean, limit_L, limit_R)`; cuts are unaffected because the cut caps are the same for both ears). Each blended gain is then a mix of two values within that ear's limit, so both ears stay within their own limits, and above 700 Hz both equal the limited mean. Without this step an ear whose SNR allows no boost received half of the other ear's boost.

## Virtual-bass hand-off

The room gain is multiplied by the magnitude response of the exact LR8 high-pass used by virtual bass: order-4 Butterworth high-pass duplicated twice. The shared SOS constructor leaves virtual-bass synthesis unchanged. Below crossover/sqrt(2) the mask is explicitly zero. At crossover times `2^(-1/4),1,2^(1/4),sqrt(2),2`, the mask is approximately 0.20, 0.50, 0.80, 0.94 and 0.996 (test tolerance 0.02). Invalid or Nyquist-exceeding cutoffs return the identity mask, matching virtual bass's existing early-return behaviour at Nyquist.

## Virtual bass v2

ADR 0006. `vbass_mode` is `auto` (default), `manual` or `legacy`. Legacy runs the unchanged 2.x function (`apply_virtual_bass`); oracle tests select it through `ProcessingConfig::oracle_defaults()`. Manual uses `vbass_freq`; `from_kwargs` selects manual when the kwargs carry `vbass_freq` without `vbass_mode`. Auto and manual share everything below except where the crossover comes from.

### Crossover

For every speaker with both ears, the cropped responses (as after `CropAndAlign`; `crop_heads` and `crop_tails` suffice) go through `magnitude_to_frequency_response` and are power-averaged per grid point, `10·log10((10^(L/10)+10^(R/10))/2)`. The average goes to `detect_rolloff` without SNR (the slope search of "Low-frequency rolloff" above). The automatic crossover is twice the highest rolloff (`freq`, the point 6 dB under the plateau), rounded to whole hertz and clamped to 30–500 Hz. With no rolloff at all, auto applies nothing and logs `cli_vbass_auto_none`. Manual warns (`cli_vbass_below_rolloff`) when its crossover is below the highest rolloff times √2.

The pair average is what makes the detection usable on in-ear responses: per ear, a room-mode dip can pass for a rolloff, and the other ear usually fills it. The demo resolves to 109 Hz (FL, 6 dB down at 54 Hz). One octave above a 2nd–4th order rolloff's −6 dB point the speaker is within 0.3–1.2 dB, so the high-passed measurement is intact at the crossover.

The service resolves the crossover before the room stage, because the hand-off mask needs it: when virtual bass is on and not legacy, it opens the measurements first, crops a copy and resolves the plan there. Progress steps keep their order.

### Synthesis

The unit synthetic bass, the interaural shelf, the ITD, the head delay and the LR8 high-pass of the measured response are the legacy ones. What changes:

| Step | Rule |
| --- | --- |
| Level band | B = [f_c, min(4 f_c, 0.45 fs)] |
| Level | For each speaker, the ear that receives the unshelved copy (left for left-side speakers, otherwise right): `magnitude_to_frequency_response`, plus the room EQ v2 gain of that ear when room EQ v2 runs (`RoomTerm::Gain`, hand-off mask included), smoothed at 1/6 octave, median over B. The gain is the median over speakers minus the same median of the unit bass before its low-pass. |
| Room target | Only with room correction on, in a v2 range or tuning, and a target that is not flat: a minimum-phase filter of `T(min(f, 2 f_c)) − T_B` on a 2n FFT, with T_B the target's median over B. |
| Alignment | One delay d ∈ [0, fs/f_c) samples for every speaker: the argmax of the cross-correlation between the high-passed measurement and the synthetic bass, summed over all ears, restricted to f_c/√2–f_c·√2. |
| Output | high-passed measurement + synthetic bass delayed by head + d (and ± ITD on the far ear). |

Sums run sequentially in speaker order, left then right ear.

Demo measurements with the release CLI (`--no_room_correction`, `responses.wav`; "legacy" is `--vbass_mode legacy`):

| Quantity | Legacy | v2 |
| --- | --- | --- |
| Level reference spread across ears at 250 Hz | 11 dB (one FFT bin) | median over two octaves |
| Synthetic level against the direct ears' median over B, median, f_c = 83 / 120 / 250 Hz | −1.1 / 0.0 / +1.0 dB | −0.4 / +0.4 / +0.1 dB |
| Shift of the synthetic bass from a +10 dB, Q 4 resonance at 280 Hz filtered into every recording (f_c = 250 Hz) | +5.4 dB | +1.2 dB; 0 dB when room EQ removes it (synthetic test) |
| Crossover cancellation against the branches' power sum, median (worst), f_c = 83 / 120 / 250 Hz | −1.2 (−4.3) / −6.5 (−10.0) / −10.8 (−13.3) dB | −0.3 (−0.7) / −2.0 (−5.9) / −0.3 (−1.6) dB |

With 14 ears the demo averages the single bin's spread away; a stereo measurement averages four.

A delay per speaker would line up each speaker better, but two speakers playing the same mono bass would then cancel each other's synthetic bass; one delay keeps them coherent as in 2.x.

### Logs

| Level | Key | Arguments |
| --- | --- | --- |
| info | cli_vbass_auto | freq, speaker, rolloff (integer Hz) |
| warning | cli_vbass_auto_none | — |
| warning | cli_vbass_below_rolloff | freq, speaker, rolloff |
| info | cli_vbass_match | lo, hi (integer Hz), delay (ms, 1 decimal) |
| info | cli_vbass_room_target | — |

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

## Virtual room tuning

### Model and placement

`room_mode=eq` is the unchanged per-ear room EQ v2. `room_mode=tuning` instead
reproduces a room-correction processor in each speaker feed during recording:
`F * BRIR`. One magnitude-and-phase FIR per speaker is applied identically to its
two ears. Before finite output cropping, this preserves the interaural transfer
ratio, hence the head/ear ILD and IPD. Headphone correction remains separate.
Tuning never executes v2's gain, range, boost, rolloff or LR8 handoff rules.
`room_range` and `room_max_boost` are inert, including `room_range=legacy`.
`room_volume` and `schroeder_freq` matter only for the Schroeder phase stop.

The room stage prepares `RoomCorrection.tuning` and returns an empty `RoomFrs`:
equalize receives no room term. Its 5 Hz FIR resolution would blur the processor's
1/24-octave LF work. The plan is applied at the end of `CropAndAlign`, after
`crop_heads`, `align_ipsilateral_all`, `align_onset_groups_peak_leftref` and
`crop_tails`, before `VirtualBass`. There is no stage row or progress increment.
Later analysis sees the processed BRIR; virtual bass replaces the processed LF
band without requiring another tuning gate. Keep each convolved response's old
length plus D samples and drop the remainder, fading the new last 5 ms with a
falling half-Hann. Unmeasured speakers receive only the common delay, not a trim.
No ear-position measurements means no tuning and no delay.

### Options

| Field / CLI option | Default | Values |
| --- | --- | --- |
| `room_tuning_delay` | `"10"` | `auto` or 2–20 ms |
| `room_tuning_phase_limit` | `"full"` | `off`, `schroeder`, `full`, or 300–20000 Hz |
| `room_tuning_max_boost` | 6 | 0–12 dB |
| `room_tuning_curtain` | 300 | 100–5000 Hz |
| `room_tuning_level_match` | true | true / false |

Delay and phase-limit JSON accept strings or numbers; numeric values are stored
as decimal strings. They parse into `TuningDelay::{Auto,Ms}` and
`PhaseLimit::{Off,Schroeder,Full,Hz}`. A numeric stop at or above 0.45 fs is full.
`off` retains magnitude and optional speaker-level correction, but has no phase
inverse, pre/post magnitude windows or added delay. CLI spelling uses `--` plus
the field name; level matching explicitly takes `true|false`. Validation names
the offending field; these are extension options, not 2.x parity fields.

### Measurements and common-clock provenance

Use speaker-specific omni ear-position measurements, split by the room stage,
`crop_head(1 ms)`, then the first N=fs samples (zero-pad shorter inputs). FFT
length is exactly N, so bin spacing is 1 Hz, not a power-of-two approximation.
Two positions are two points; one position runs the entire single-point chain
and logs `cli_room_tuning_single_point`. Subtract microphone calibration only
from magnitude. `room.wav` is never used: it is usually one point of one speaker;
log `cli_room_tuning_room_wav_ignored` once if present. With no specific data log
`cli_room_tuning_no_data` and leave every channel unchanged.

Retain uncropped split IRs and recording membership for auto-delay and the
omni-only geometry check. Different files have independent latencies: never
compare their absolute arrivals, and never compare omni arrivals with in-ear
BRIR arrivals (head diffraction legitimately changes the latter). For a symmetric
pair in `IPSILATERAL_PAIRS` except FC, recorded in the same omni file, arrival is
`argmax(abs(h))`. At the left position the left speaker must arrive first; at the
right position the right speaker must arrive first. A reversed ordering exceeding
0.05 ms warns `cli_room_tuning_mismatch` for both speakers. Non-symmetric pairs
(BL/SL and SR/BR on the demo) and single-speaker files are not checked. These
warnings do not disable processing. The demo FL/FR differences are +0.125 ms
at left and −0.250 ms at right: neither warns.

### Shared numerical primitives

Let f be FFT-bin frequency and k positive bin index, k=1…N/2. Fractional-octave
boxcar `S_b(X)` uses width
`w=f_k*(2^(1/(2b))-2^(-1/(2b)))`,
`m=max(1,floor(w/(2*delta_f)))`, and the mean over the half-open range
`[max(1,k-m),min(N/2+1,k+m+1))`, implemented with cumulative sums. DC is unchanged.
Negative frequencies are the conjugate mirror (real mirror for real data); even-N
Nyquist is real when transformed to real time. The same primitive smooths complex
phase vectors and real linear magnitudes.

Define `L(fc)=1/(1+(abs(f)/fc)^4)` and the five weights
`w0=L(100)`, `w1=(1-L(100))*L(150)`, `w2=(1-L(150))*L(200)`,
`w3=(1-L(200))*L(300)`, `w4=1-L(300)`.
They are used exactly as written, without renormalizing their sum.
`B(X)=sum(w_i*S_bi(X))`; Normal denominators are `(24,12,6,3,1)`,
Low denominators `(12,6,3,2,1)`. Both reside in one constant table.

Minimum phase of A: IFFT the real spectrum `ln(A+1e-12)`, apply the real-cepstrum
lifter `[1,2,…,2,1 at N/2,0,…,0]`, then `exp(FFT(cepstrum))`.
Quantiles use linear interpolation between sorted samples.

### Excess-phase inverse

Per point, p is the index of maximum absolute h, and
`E_j=H_j*exp(+j*2*pi*f*p_j/fs)/H_min,j`, with minimum phase from the raw magnitude.
The calibrated magnitude is used for weighting and level regularisation:
`r_j=clip((S_3(abs(H_j))/mean_(100<f<10000)(S_3(abs(H_j)))-0.177)/0.5,0,1)`.
For one point `X=conj(E_1)`, `G=r_1`. For two points,
`C=S_3(abs(H_1))*E_1+S_3(abs(H_2))*E_2`, `X=conj(C/abs(C))`,
`G=min(r_1,r_2)*agreement`. Agreement is 1 through 60 degrees of excess-phase
difference, linear to 0 at 120 degrees.

Full applies no phase-band gate. Schroeder sets `f_ph=min(f_S,300)`; a numeric
stop sets `f_ph=x`. Multiply G by 1 below `f_ph/sqrt(2)`, then a falling half-Hann
in frequency to zero at `f_ph`. There is no SNR floor: the attempted safety factor
bound on the demo and was removed, not retuned. For a single point at full phase,
G is exactly the processor's r. No rolloff, local-level, LR8, or residual
linear-phase-fit gate exists.

Blend `X=G*X+(1-G)`, smooth with complex Normal B, normalize `X/=abs(X)`, and take
real IFFT x. For chosen delay D in ms:
`pr_low=clip(D,2,25)`, `pr_mid=min(10,pr_low/2)`, `pr_high=min(5,pr_low/4)`.
Circular time is `t=((n+N/2) mod N)-N/2`. W(pre) equals 1 at t>=0, a half-Hann
`0.5*(1+cos(pi*t/P))` at -P<=t<0, and zero earlier (P in samples). Post-side is
open. Form
`H_ex=(w0+w1)*FFT(x*W(pr_low))+(w2+w3)*FFT(x*W(pr_mid))+w4*FFT(x*W(pr_high))`,
then normalize `H_ex/=abs(H_ex)`. Near-zero complex vectors become identity.

### Magnitude, tone and peaks

The one magnitude spectrum is the spatial power average
`abs(H)=sqrt((abs(H_1)^2+abs(H_2)^2)/2)`, or the single-point magnitude.

1. `M=B(abs(H))` using Normal. Below 15 Hz replace M by
   `min(M,max(M over 15–20 Hz))`.
2. `ref=percentile_35(raw abs(H),100<=f<10000)`. The lower endpoint is
   included, as in the black-box frequency grid (its integer bins carry a tiny
   positive epsilon). Input FFT magnitudes and their smoothing cumulative sums
   use float32; subsequent complex/tone arithmetic uses float64. This boundary
   matters for exact cutoff crossings, not just the last decimal of a spectrum.
3. `M_dB=20*log10(M)`. LF reference is its mean over 1–10 kHz. The first bin
   in 10–1000 Hz at least reference−6 dB is lc, clipped 10–300 Hz, with 300 Hz
   fallback. HF reference is `20*log10(percentile_95(M over 2–22.05 kHz))`.
   The last bin there at least reference−10 dB is hc, clipped 6–20 kHz, with
   16 kHz fallback. Report both.
4. Tracking `t=clip(t_low+t_high+t_noise,0,1)`, where
   `t_low=clip((1.15*lc-f)/(0.15*lc),0,1)`,
   `t_high=clip((f-0.8*hc)/(0.2*hc),0,1)`,
   `thr=mean(M over 200–1000 Hz)*10^(-30/20)` and
   `t_noise=clip((2*thr-M)/thr,0,1)` below 200 Hz, zero above.
5. `natural=M/ref`, `env=min(1,natural)`,
   `flat=(f^4/(f^4+1))*(20000^4/(f^4+20000^4))`,
   `target=env^t*flat^(1-t)`. Above 1 kHz cap target to flat. Multiply a configured
   room target in linear amplitude after subtracting its mean dB over 100 Hz–10 kHz.
   Existing bass boost and tilt remain in the downstream target stage.
6. `inv=clip(target/natural,0,10^(max_boost/20))`; copy bin 1 into DC.
7. Curtain c: `fade=clip((f-c)/(min(2c,fs/2-1)-c),0,1)`;
   `inv=inv*(1-fade)+fade`.
8. `m=IFFT(minimum_phase(inv))`. With phase enabled,
   `Hm=FFT(m*W2(pr_low,500 ms))*(1-fade)+FFT(m*W2(pr_low,10 ms))*fade`.
   W2 also half-Hann-windows the post side to zero at its stated duration.
   With off, `Hm=FFT(m)` without windows.
9. `D_s=round(D*fs/1000)`;
   `initial=H_ex*Hm*exp(-j*2*pi*f*D_s/fs)`. With off use `initial=Hm`, D=0.
10. `sim=abs(H)*abs(initial)`, `tonal=B_Low(sim)`,
    `macro=clip(ref*target/tonal,0.5,2)`.
    Where macro>1, replace by `1+(macro-1)*clip((20000-f)/10000,0,1)`.
    Then `macro=S_1(macro)`.
11. `res=sim*macro/(ref*target)`, `peaks=S_48(max(res,1))`,
    `crush=(1/peaks)^0.6`, `cw=clip((500-f)/200,0,1)`,
    `crush=(1-cw)+crush*cw`.
12. `post=macro*crush`, copy bin 1 into DC, then
    `F=real(IFFT(initial*minimum_phase(post)))`, length N, design zero at D_s.

### Level matching, auto delay and final FIR

For measured speakers use `trim_i=median(ref_j)/ref_i`, clipped to ±6 dB.
Compare its un-clipped dB ratio with the independently median-referenced in-ear
BRIR level (both ears' power summed in 100–500 Hz, after alignment). If they differ
by >2 dB, skip this speaker's trim and log `cli_room_tuning_level_inconsistent`
with the signed difference to one decimal place. Otherwise apply the scalar to
the shared F. Report trim and `omni` / `skipped` source. For levels 0,−3,−10 dB,
the median rule yields −3,0,+6 dB (the last trim is clipped).

A fixed delay applies to every speaker. Auto tries each integer 2…10 ms using
SECS's absolute-plus-shape target-error criterion. Every two-speaker room file
is eligible, symmetric or not, at both ear positions (six demo files). Keep file
order: A is its first speaker and B its second. From the uncropped split, start
both IRs `round(0.001*fs)` before the earlier maximum-absolute peak; take N=fs
samples and zero-pad as needed. This path never uses per-speaker head crops.

For each candidate D:
1. Build independent single-point full-length F_A and F_B, `phase_limit=full`,
   no level match, with configured boost, curtain, calibration and target.
2. Scale F_B by RMS_A/RMS_B, measured in each input's `[p-w,p+w)` clipped to
   available samples, `w=round(0.005*fs)`, p the maximum-absolute peak.
3. Shift the earlier speaker's filter right by `max(p_A,p_B)-p_X`, zero-fill,
   no wrapping. Find q, the largest absolute value over both shifted filters.
4. Crop both to `round(0.51*fs)` samples from `q-int((D/1000)*fs)`, zero-filling
   outside. Preserve this scoring-only float conversion/truncation: 9 ms at
   48 kHz gives 431 samples, whereas the production FIR origin remains the
   rounded 432. No pre-window or minimum-phase-window change is needed.
5. Form `Y=H_A*FFT_N(crop_A)+H_B*FFT_N(crop_B)` with uncropped common-start H.
6. `after_dB=20*log10(B(abs(Y))+1e-12)` (Normal),
   `target_dB=20*log10(ref_A*target_A)`.
7. Over `f_low=max(10,(lc_A+lc_B)/2) < f <= 300 Hz` use (lower edge open,
   reproducing the black-box cutoffs' positive sub-bin epsilon)
   `w=1+clip((300-f)/(300-f_low),0,1)` and `e=after_dB-(target_dB+6)`.
   `abs=sum(w*abs(e))/sum(w)`, `mean=sum(w*e)/sum(w)`,
   `shape=sum(w*abs(e-mean))/sum(w)`, metric=`abs+0.35*shape`.

Choose the smallest summed metric over all eligible files/positions. Candidates
are ascending with strict `<`, so ties keep the smaller D. No eligible file
means 10 ms. Pair-specific level/arrival matching and joint cropping exist only
inside scoring; the production filters retain their shared design origin.

Final FIR starts at index zero, length `round(0.51*fs)`, with the final 5 ms faded
by `(1-n/len)^2`. Its design origin stays at D_s, with pre-ringing in [0,D_s).
No per-speaker or per-ear shift is added. Impulcifer's existing alignment already
performs the processor's speaker distance compensation; matching the two ears
would destroy ITD. **A common design origin is not proof that the largest or
threshold-selected waveform peak moves exactly D_s:** frequency-dependent
filtering can change which local peak is selected. Tests assert the FIR design
origin exactly, demo peaks within D_s ± round(0.1 ms*fs), and unmeasured channels
at exactly D_s. Synthetic peaks retain the exact assertion where it holds.

### Diagnostics and reference acceptance

Log `cli_room_tuning` with integer phase ceiling (Nyquist for full, zero for off)
and chosen delay in ms. Off additionally logs `cli_room_tuning_off`. Single point,
ignored generic file and no-data logs are described above. Warn with
`cli_room_tuning_mismatch` for geometry checks. Pre-echo uses the direct sound
`first_peak_index(data,0,None,0.12589)`, not the largest reflection: maximum
absolute sample in `[0,direct-round(2 ms*fs)]`, relative to the direct sample.
The last 2 ms before the direct sound are excluded on purpose: a full-band
mixed-phase correction applied at the ear always leaves −20 to −45 dB there.
SECS's own single-point filters applied to the demo BRIRs measure −22 to −47 dB
0.5 ms before the direct sound and −33 to −54 dB 2 ms before it; Impulcifer's
two-point filters measure −22 to −35 dB and −33 to −39 dB. That part is heard as
a softer onset, not as an echo.
Measure before/after the tuning convolution separately for each ear, but refer
both ears of a speaker to the larger of the two direct levels (before and after
respectively): the pre-echo of a source is heard against its louder direct
sound, and the head-shadowed contralateral direct sound would otherwise inflate
the ratio (demo SL right ear: −29.3 dB against its own direct sound). Warn with
`cli_room_tuning_preecho` only if tuned > −30 dB **and** tuned >= untuned + 6 dB
for that same ear; never compare one ear with the other ear's baseline.

`cli_room_tuning_weak` now measures representativeness: for each available ear,
1/6-octave smooth the omni and in-ear dB magnitudes, level-match over
100–300 Hz, and compute RMS difference over 30–300 Hz. Warn only if the mean
over measured ears exceeds 4 dB. No old magnitude-only/no-phase/no-SNR keys
remain. EQ emits no tuning keys.

LF decay uses 1/3-octave bands centred at 40, 50, 63, 80, 100, 125 and 160 Hz.
Each is a fourth-order Butterworth prototype transformed to a band-pass and
applied zero-phase (squared digital magnitude, padded FFT to prevent wrapping).
From the direct peak, form the Schroeder backward energy integral; fit dB versus
time from 0 to −10 dB and extrapolate the slope to −60 dB (EDT, ×6). Average
left/right EDT in each band, then take the median of seven bands per speaker.
The second diagnostic is median absolute excess GD over 30–300 Hz, after
1/6-octave complex smoothing of the peak-aligned excess spectrum; average the
two ears' medians. Both EDT and median excess GD are **information only**:
SECS itself can increase them at its own measurement point, so neither judges
whether the room files represent the ears.
README output includes delay, phase limit, points, cutoffs, trim/source,
per-ear untuned/tuned pre-echo and warning status, representativeness RMS,
both before/after informational diagnostics and weak status. EQ per-ear room plots are not
drawn in tuning mode; a dedicated tuning plot is not implemented.

The clean-room reference is `tests/migration/goldens/room_tuning_secs_reference.json`.
For each of its 14 file/speaker entries use the demo estimator, head crop 1 ms,
48000 samples, full phase, 10 ms, max boost 6, curtain 300, no trim, target or mic
calibration. Compare the **uncropped** N-sample FIR. Magnitude is
`20*log10(max(abs(FFT(F)),1e-12))`. Excess spectrum is
`FFT(F)/F_min*exp(+j*2*pi*f*0.010)`; numerical gradient of its unwrapped phase over
positive bins 1…N/2−1 gives excess GD in ms. At all 250 grid points average bins
in `[g*2^(-1/24),g*2^(1/24)]`, interpolating linearly where none falls inside.
Subtract each magnitude curve's own 100 Hz–10 kHz mean. Magnitude bands are
20–100,100–300,300–600,600–10000,10000–16000 Hz; GD bands (only G>=0.9) are
20–40 (report only),40–100,100–300,300–1000,1000–5000 Hz. Report RMS, maximum,
and GD median/kept share. Cutoffs must agree within 1/12 octave.

The golden's four-decimal frequency labels must not be used as averaging
boundaries: reconstruct `15*2^(i/24)` and verify it rounds to each label. Otherwise
isolated LF bins are included/excluded incorrectly (the apparent ~0.5 dB / 2.9 ms
outliers were comparison-grid errors, not filter errors).
Current provisional bounds are magnitude `[0.01;5]` dB and GD `[0.01;5]` ms:
measured maximum × 1.5, rounded up to 0.01, with a 0.01 minimum. Maximum errors
are 0.000534 dB and 0.000149 ms; the first GD band remains report-only. Named
constants retain the provisional comment and require owner review.

`room_tuning_matches_secs_stages` checks three independent single-point entries
against `room_tuning_secs_stages.json`, using the production chain's optional
stage capture: peak/ref, cutoffs, r, smoothed M, t, target, inverse, windowed
minimum phase, macro, crush, final magnitude. Scalar tolerance is 1e-4 relative;
cutoffs match exact integer bins (ignoring serialized ~1e-12 Hz roundoff), spectra
0.01 dB, r/t 1e-3 absolute. All pass; largest spectral discrepancy is 0.000025 dB.
Single-point synthetic excess GD RMS improves 3.1624→0.9404 ms (70.3%).
Off/full differs by 0.2158 dB because only full applies the SECS post windows
to the minimum-phase inverse; the accepted synthetic bound is 0.35 dB.
The demo has no geometry mismatch; final peaks shift 478/481 samples for the
480-sample design origin, within the accepted ±5 samples.

The demo's per-ear representativeness RMS is 1.59–3.45 dB (speaker means
2.20–2.71 dB): no weak warnings. No ear warns about pre-echo either, and
`room_tuning_runs_on_the_demo` asserts both. The loudest tuned pre-echo is
SL-left: −31.49 dB at the default 10 ms and −33.43 dB with auto delay (4 ms),
below the −30 dB threshold; every other ear is at most −35.68 dB at 10 ms
(3.2.1 release CLI). An earlier draft of this section recorded an SL-left
warning at −26.66 dB from a previous version of the measurement; it no longer
applies, and neither the thresholds nor the filter were retuned for it.

On the demo the information-only diagnostics rise for every speaker: at 10 ms
the LF EDT goes from 452–493 ms to 531–602 ms and the median excess GD from
3.1–5.1 ms to 4.6–7.3 ms (FL: 461 → 580 ms, 3.85 → 6.34 ms). Tuning reproduces
the processor's correction at the room measurement points; it does not promise
a shorter bass decay or a smaller excess GD in the binaural result, and a
false `weak` says only that the room measurements represent the ears.

Auto-delay has a separate black-box golden:
`tests/migration/goldens/room_tuning_secs_auto_delay.json`. Its `ir.construction`
defines the common-start inputs, not the per-speaker crop used above. The
`room_tuning_auto_delay_matches_secs` test prints all nine per-file metrics and
the run-level sums, asserts every score within 0.002, and asserts all six chosen
Ds exactly. Maximum score discrepancy is 0.000008. FL/FR left/right, BL/SL right,
and SR/BR left/right choose 10 ms; BL/SL left chooses 7 ms. The former D-dependent
drift came from including the lower scoring-band endpoint; the remaining 9 ms
error came from scoring-crop sample truncation. Flat/no-calibration run-level
sum selects 10 ms. The demo's configured room target and calibration select
4 ms (sum 20.532675); checked separately by the service auto-delay test.

Deliberately not reproduced from SECS: auto-selection above 10 ms, per-pair time
shifts, stereo gain pre-scaling, and low-latency output (which leaves a near-empty
channel on three of the four demo pairs in the owner's black-box observation).

Virtual room tuning reproduces the correction method of SECS by 한플 (DCinside speaker minor gallery, 2026). Impulcifer's implementation is independent; SECS was used only as a black box to produce the reference numbers.

## Virtual room tuning v2

`room_mode=tuning2` reproduces the channel correction (RC) of SECS 2 and, as an
option, its MIMO low-frequency supports. It is a third member of the room
family, next to `eq` (ADR 0004) and `tuning` (v1, SECS 1.41, ADR 0005), and
shares no rule with either. The decisions are recorded in ADR 0008. This
section is the clean-room specification: the implementation was written from
it, and SECS 2 (GPL-3.0) was run only as a black box to produce the reference
numbers in `tests/migration/goldens/room_tuning_secs2_*.json`.

### Model and placement

SECS 2 designs, per loudspeaker, one correction filter in front of the
loudspeaker (minimum-phase PEQ plus an FIR that also undoes part of the room's
excess phase), and optionally adds to each input low-pass "support" copies of
that input sent to other loudspeakers, chosen to cancel the late
low-frequency energy at the measurement positions. A processor in the speaker
feed during recording commutes with every later linear path, so the tuned
BRIR of measured speaker i at ear e is

`BRIR'_i,e = g_i * F_i ⊛ BRIR_i,e + Σ_j Q_ij ⊛ BRIR_j,e`

with F_i the correction filter, g_i its level trim and Q_ij the support from
input i to speaker j (MIMO only). BRIR_j,e are the untuned responses. F_i is
designed from the speaker's omni ear-position measurements, exactly as v1
uses them; the supports are designed on the aligned in-ear BRIRs, because
only they share one time base across recordings and because the ears are the
exact reproduction points.

Placement is v1's: the plan is prepared by the room stage, which returns an
empty `RoomFrs` (equalize gets no room term), and applied at the end of
`CropAndAlign` after `crop_tails`, before `VirtualBass`. No stage row, no
progress increment. `room_range`, `room_max_boost` and every v1 option
(`room_tuning_delay`, `room_tuning_phase_limit`, `room_tuning_max_boost`,
`room_tuning_curtain`) are inert in this mode. `room_tuning_level_match`,
`room_volume` and `schroeder_freq` apply as described below.

### Options

| Field / CLI option | Default | Values |
| --- | --- | --- |
| `room_tuning2_mode` | `"mixed"` | `zero`, `mixed`, `linear`, `long`, or 30–200 (ms, SECS's ultra mode) |
| `room_tuning2_max_boost` | 10 | 0–10 dB |
| `room_tuning2_low` | 20 | 10–200 Hz |
| `room_tuning2_high` | `"auto"` | `auto` or 100–20000 Hz |
| `room_tuning2_mimo` | false | true / false |
| `room_tuning2_fsiso` | 150 | 50–150 Hz |
| `room_tuning2_mimo_low` | `"auto"` | `auto` or 15–150 Hz |

`room_tuning2_mode`, `room_tuning2_high` and `room_tuning2_mimo_low` accept
strings or numbers in JSON and are stored as decimal strings, like v1's
delay. Values outside the ranges are rejected naming the field (SECS clamps;
Impulcifer validates). These are extension fields and CLI extension options.

Modes (FIR taps at 48 kHz, PEQ bands, lift = design delay, phase resolution β):

| Mode | Taps | PEQ | Lift (ms) | β |
| --- | --- | --- | --- | --- |
| `zero` | none | 32 | 0 | — |
| `mixed` | 2048 | 16 | 8 | 6 |
| `linear` | 2048 | 16 | 28 | 24 or 6 (chosen, below) |
| `long` | 32768 | 16 | 200 | 6 |
| x ms (30–200) | 2048·2^k | 16 | x − 2 | 24 or 6 (chosen) |

For x ms, start at 2048 taps and double while taps < 16384 and
`0.7*taps/48 < x-2` (60 ms → 4096, 200 ms → 16384). In every mode the lift is
capped at `0.7*taps/fs*1000` ms; the listed modes never reach the cap at
48 kHz.

Other sample rates keep times and frequencies: the analysis length is
`N = 2^17*fs/48000` rounded to the nearest even integer and the tap count is
`round(taps*fs/48000)`. Every millisecond quantity becomes a sample count by
truncation, `int(ms*fs/1000)`, as SECS does. Only 48 kHz is checked against
SECS. Sample rates below 44.1 kHz are rejected.

### Measurements and spectra

For each speaker the positions are its ear-position omni measurements in the
order left, right (one or two). Use the room stage's uncropped split IR
(`open_recording_samples(…, 2.0)`); there is no head crop and no length cut.
`room.wav` is never used (log `cli_room_tuning_room_wav_ignored` once if
present). No ear-position measurement for any speaker: log
`cli_room_tuning_no_data` and leave the BRIR unchanged. One position: log
`cli_room_tuning_single_point` and run the single-position chain.

The grid is `f_k = k*fs/N`, k = 0…N/2, computed as `numpy.fft.rfftfreq(N, 1/fs)`;
Δ = f_1 − f_0. For an IR h:

- onset o: the first index at or before the first maximum of |h| with
  `|h| ≥ 0.1*max|h|`;
- RC window: `a = max(0, o − int(0.002*fs))`, `w = h[a : a+int(0.5*fs)]`
  (shorter if h ends), the last `K = int(0.2*len(w))` samples multiplied by
  `0.5*(1+cos(π*k/K))`, k = 0…K−1;
- spectrum `H_p = rfft_N(w)*exp(−j*2π*f*(a−o)/fs)`: every position is placed
  on its own onset, so its window starts 2 ms before time zero. SECS places
  all positions on the listening position's onset (the earliest onset of all
  its speakers there). The chain removes each position's delay before using
  its phase, so only the long-mode analysis sees where a response sits in
  time; it must see each position as SECS sees its listening position, and
  positions recorded in different files have unrelated latencies. (In a SECS
  run of several speakers, one shared origin moves every speaker but the
  earliest away from −2 ms, which changes their long-mode analysis: such a run
  is a reference for levels only.)

Microphone calibration, when configured, divides every H by the minimum-phase
response whose magnitude is `10^(c(f)/20)`, c interpolated linearly in Hz with
end values held. (SECS receives calibrated responses from REW; this is not part
of its chain.) The room target T(f), when configured, is the target file
interpolated the same way; without one T = 0. Bass boost and tilt stay in the
downstream target stage, as in v1.

### Position policy

SECS weights its listening position 5× and runs three steps on it alone. Both
ears are listening positions here, so the weights are equal (w_p = 1) and each
single-position step runs on every position and combines as stated where it
appears: the long-mode mask is the union, the phase-resolution score is the
worst position, the Schroeder decay pools all positions. With one position the
chain is SECS's exactly. The implementation also carries SECS's own policy
(listening position p0 with weight w, all other weights 1, the three steps on
p0 only) for the reference tests; it is not an option.

### Shared numerical primitives

- **Smoothing** `S_b(X)`, b a scalar or one value per bin: `h_k = 2^(0.5/b_k)`,
  `lo_k = clip(floor((f_k/h_k)/Δ), 0, N/2)`, `hi_k = clip(ceil((f_k*h_k)/Δ), 0, N/2)`,
  then `hi_k = max(hi_k, lo_k+1)`; `S = (C[hi+1] − C[lo]) / (hi+1−lo)` with C the
  sequential float64 cumulative sum, C[0] = 0. Complex input: real and
  imaginary parts separately. It is not v1's primitive.
- **Variable resolution** `b(f) = 24*(6/24)^t`,
  `t = clip((log10(max(f,1e-3)) − log10 200)/(log10 2000 − log10 200), 0, 1)`:
  1/24 octave below 200 Hz, 1/6 octave above 2 kHz.
- **Minimum phase** `MP(A)`: `L = ln(max(A, 1e-8))`, `c = irfft_N(L)`, fold to
  `[c_0, 2c_1 … 2c_(N/2−1), c_(N/2), 0 …]`, `MP = exp(rfft_N(fold))`. (v1 uses a
  different floor.)
- **Peaking (RBJ)** `P(f; f0, Q, G)`: `A = 10^(G/40)`, `w0 = 2π*f0/fs`,
  `α = sin(w0)/(2Q)`, numerator `(1+αA, −2cos w0, 1−αA)`, denominator
  `(1+α/A, −2cos w0, 1−α/A)`, `z = exp(−j*2π*f/fs)`,
  `P = (b0 + b1*z + b2*z*z)/(a0 + a1*z + a2*z*z)`. A PEQ is the product of its
  bands in order, starting from 1.
- **HP4** `(s²/(s²+√2 s+1))²`, `s = j*f/fc`.
- **Low rate** `LR(X, n)`: `fl = rfftfreq(2n, 1/600)`;
  `X_l = interp(fl, f, Re X) + j*interp(fl, f, Im X)`;
  `X_l *= clip((285 − fl)/60, 0, 1)`; `x = irfft_(2n)(X_l)[:n]`.
- **Late peaks** `LP(xs, count)`: for each x, `k = first index with |x| > 0.1*max|x|`,
  zero x before `k+90`, add `|rfft_4096|²`; on `rfftfreq(4096, 1/600)` multiply
  the sum by the 20–160 Hz mask over the whole grid (0 outside, the sum inside)
  and take the local maxima of that masked array (`scipy.signal.find_peaks`
  without conditions). Because the bins outside the band are zero, the first
  bin at or above 20 Hz and the last bin at or below 160 Hz (159.96 Hz) are
  maxima whenever the sum falls away from, or rises toward, the band edge;
  searching only inside the band would miss them. Such an edge bin is an
  artifact of the masking, not a room mode, but it is kept: the MIMO guard's
  modes and the RC's long-mode list depend on it, and on the demo it decides
  two guard values. Sort by height descending (stable), keep `count`, return
  their frequencies.
- **Mode decay** `T60(x, fc, half)` at 600 Hz: band `[max(fc−half, 1), min(fc+half, 270)]`,
  order-2 Butterworth band-pass SOS; prepend 1200 zeros, filter the reversed
  signal and reverse again, square → e; from `s = argmax e`, the reversed
  cumulative sum EDC; none if `EDC_0 ≤ 0`; `D = 10*log10(max(EDC/EDC_0, 1e-30))`;
  i5 / i25 the first indices with D ≤ −5 / ≤ −25 (0 if none); none if
  `i25 ≤ i5+2`; least-squares slope of D over `t = (i5 … i25−1)/600`;
  `T60 = −60/slope` if the slope is negative, else none.
- Conventions where the chain uses them: inverse real FFTs discard the
  imaginary parts of the DC and Nyquist bins; `interp` holds end values;
  `geomspace` has exact end points; `gradient` is `numpy.gradient` with a
  coordinate array and first-order edges; `unwrap` uses period 2π; medians
  average the two middle values; `round(x, 1)` is Python's; Butterworth SOS and
  filtering follow `scipy.signal.butter(..., output='sos')` and `sosfilt`.

### Channel level

`P_i = Σ_p w_p |H_p|² / Σ_p w_p`.
`L_i = median over 300 ≤ f ≤ 3000 of (10*log10(P_i + 1e-30) − T(f))`.
SECS cuts every channel to the quietest: `g_i = min_j L_j − L_i` dB over the
measured speakers. With `room_tuning_level_match=false` every g_i is 0.
Impulcifer keeps v1's consistency check: with d_i the omni level minus the
median omni level, and e_i the aligned in-ear BRIR level (both ears' power
summed over 100–500 Hz, dB) minus its median, a speaker with
`|d_i − e_i| > 2 dB` gets g_i = 0, is left out of the minimum, and logs
`cli_room_tuning_level_inconsistent` with the signed difference to one decimal.
The correction filter itself never depends on g_i or on the other speakers.

### Correction band

`lo = room_tuning2_low`. `hi = room_tuning2_high` when numeric. Automatic:
`f_S = schroeder_freq` when set; otherwise, with `room_volume = V` set, SECS's
estimate: for every omni IR (position policy: all positions; SECS: the
listening position) and fc ∈ {500, 1000} Hz, filter the whole uncropped IR with
an order-4 Butterworth band-pass `[fc/√2, fc*√2]` (SOS), take the reversed
cumulative energy E, `D = 10*log10(E/(E_0+1e-30) + 1e-30)`, i5 / i25 the first
indices with D < −5 / < −25 (0 if none), and keep `3*(i25−i5)/fs` when
`i25 > i5 > 0`. `T60 = median`, `f_S = 2000*sqrt(T60/V)`. Then `hi = 500` when
there is no f_S or `f_S > 500`, else `max(f_S, 100)`.

### Design curve

For each measured speaker. The channel trim does not enter the design (in SECS
it cancels): the "comb" response at position p is `comb_p = H_p` and L is the
speaker's own level L_i.

1. `S = 10*log10(S_b(f)(P) + 1e-30)`, `S3 = 10*log10(S_3(P) + 1e-30)`,
   `S = max(S, S3 − 3)` — narrow dips are filled by at most 3 dB.
2. `U = clip(T(max(f,1)) + L − S, −25, cap)` with `cap = max_boost`, or
   `min(2, max_boost)` above 16 kHz.
3. Long-mode limit: `U = min(U, 3)` inside the long-mode mask.
4. Band fade: `ℓ = log2(max(f,1e-3))`, `e = min(hi*√2, 0.99*fs/2)`,
   `U0 = U * clip((ℓ − log2(lo/√2))/0.5, 0, 1) * clip((log2 e − ℓ)/max(log2(e/hi), 1e-3), 0, 1)`.
   U0 is the design curve.

Long-mode mask of one position's comb response X: `x = LR(X, 300)`; peaks
`LP([x], 8)` within `lo ≤ p ≤ min(hi, 160)`; `T60(x, p, 1.5)` for each, dropping
none and zero; fewer than two left → empty. With `r` their median, mark the bins
`p*2^(−1/24) ≤ f ≤ p*2^(1/24)` of every peak with `T60 ≥ max(0.3, 1.3*r)`.
SECS reports the marked peaks in its notes; Impulcifer lists them in the README.
SECS's "no boost at excess group-delay peaks" mask is off by default and is not
reproduced.

### PEQ fit

`Fit(g, c, n, lo=20, hi=500, boost, fmax)` fits n peaking bands to the dB curve c
on the log grid g. Every call uses a ring limit τ = 30 ms with
`Qring(f0, G) = π*f0*τ/1000 / 10^(G/40)`.

Greedy start, at most n times: `cur = 20*log10(|PEQ(g; bands)| + 1e-12)`;
`r = c − cur` inside `[lo, hi]`, 0 outside; k the first `argmax|r|`; stop when
`|r_k| < 0.5`; `G = clip(r_k, −25, boost)`; widen `i0 ≤ k ≤ i1` while the next
neighbour has `|r| ≥ |r_k|/2` and the same sign as r_k;
`Q = clip(g_k / max(g_i1 − g_i0, 0.02*g_k), 0.5, 15)`, then
`Q = max(min(Q, Qring(g_k, G)), 0.31)`; append `(g_k, Q, G)`. No band → empty
result.

Refinement over `(ln f0, ln Q, G)` per band. Bounds start at
`(ln 15, ln 0.3, −25)` … `(ln f_top, ln 20, boost)`, `f_top = fmax` if given, else
`max(2000, 1.2*hi)`. Each band then narrows from its greedy values (f0, Q, G):
`ln f0 ∈ [max(lower, ln(f0/1.2)), max(min(upper, ln(1.2 f0)), lower' + 1e-3)]`;
for `G > 0` the gain range is `[0, min(G+3, boost)]`, else its upper bound is
`min(G/2, −0.25)` (lower stays −25); the upper ln Q becomes
`ln(max(min(e^upper, Qring(f0/1.2, G_upper)), 0.31))`. The start point is the
greedy result clipped to `[lower + 1e-6, upper − 1e-6]`. Residual:
`ω*(20*log10(|Π_bands P(g; e^lnf, e^lnQ, G)| + 1e-12) − c)`, ω = 1 in `[lo, hi]`,
0.15 elsewhere. Minimize half its squared norm with the semantics of
`scipy.optimize.least_squares` (scipy 1.18.1): `method='trf'`, `jac='2-point'`
(default step), bounds above, `x_scale=1`, linear loss, `ftol = xtol = gtol = 1e-8`,
`tr_solver='exact'`, `max_nfev=150`. The iteration often stops at max_nfev, so
the trust-region path is part of the result: port `trf_bounds`, its helpers and
the bounded two-point difference from scipy (BSD-3, credited in
THIRD_PARTY_LICENSES). Output `(e^lnf, min(e^lnQ, Qring(e^lnf, G)), G)` per band,
in order.

### IIR-only mode (`zero`)

`g = geomspace(15, 20000, 900)`, `c = interp(g, f, U0)`.
`B_lo = Fit(g, c·[g ≤ 500], 16, lo = max(15, lo/1.4), hi = 400)`;
`rest = c − 20*log10(|PEQ(g; B_lo)| + 1e-12)`;
`B_hi = Fit(g, rest·[g ≥ 300], 16, lo = 400, hi = min(20000, hi))`.
`F = PEQ(f; B_lo ++ B_hi)`, no FIR, no lift, phase strength 0 and no phase
resolution (SECS reports a one-tap unit FIR and its default resolution 6;
both mean "none"). (`·[cond]` keeps
the value where the condition holds and puts 0 elsewhere.)

### FIR modes

Let `g = geomspace(15, 20000, 700)`, `f_F = 1.5*fs/taps`,
`ramp(f) = clip((f − f_F/2)/(f_F/2), 0, 1)`, `f_e = hi*√2`, λ = lift in seconds.

1. **First PEQ.** `B1 = Fit(g, interp(g, f, U0), 16, fmax = hi)`, `P1 = PEQ(f; B1)`,
   `U1 = (U0 − 20*log10(|P1| + 1e-12)) * ramp`; `M = MP(10^(U1/20))`.
2. **Excess phase common to the positions.** For each position:
   `R = comb_p*P1*M`; `t = argmax|irfft_N(R)|/fs`; `R *= exp(+j*2π*f*t)`;
   `E = R/(MP(sqrt(S_b(f)(|R|²)) + 1e-12) + 1e-30)`; `V += w_p*E/(|E| + 1e-30)`.
   `V /= Σw_p`; `φ = unwrap(angle(nan_to_num(S_β(V))))`.
3. **Delay budget.** `γ = clip((log2 f_e − log2 max(f,1e-3))/0.5, 0, 1)`, and
   `γ = clip(f/15, 0, 1)` below 15 Hz.
   `τ = −gradient(φ, 2π*max(f, Δ))`, non-finite values set to 0, `τ = S_β(τ)`;
   `x = (τ − median(τ over 3–6 kHz)) * γ`.
4. **Realize(s, M′):** `adv = clip(x, −0.005, λ*s)`,
   `ψ = −2π*cumsum(λ − adv)*Δ`, `h = irfft_N(M′*exp(jψ))[:taps]`, the last
   `K = int(0.1*taps)` samples multiplied by `0.5*(1+cos(π*k/K))`,
   `F = rfft_N(h)`; error `20*log10(|F|+1e-12) − 20*log10(|M′|+1e-12)` over
   `max(20, f_F) ≤ f ≤ f_e`; return h, F, its RMS and maximum absolute value.
5. **Strength.** `(h, F, rms0, max0) = Realize(0, M)`, `s* = 0`. For s in
   0.9, 0.7, 0.5, 0.3 take the first Realize(s, M) with
   `rms ≤ max(0.5, rms0+0.3)` and `max ≤ max(2, max0+1)`; s* = s. s* is the
   reported phase strength.
6. **Second PEQ.** `B2 = Fit(g, interp(g, f, U0 − 20*log10(|F|+1e-12)), 16, fmax = hi)`,
   `P2 = PEQ(f; B2)`.
7. **Broad touch-up**, at most two rounds. ρ: bins with
   `max(lo,20) ≤ f ≤ hi`. `err(F′)`: with
   `e = 10*log10(S_24(|F′*P2|²) + 1e-30) − U0` on ρ, `sqrt(mean((e − median e)²))`.
   Round: `r = ρ·(U0 − 20*log10(|F*P2| + 1e-12))`; `ζ = S_3(r)*ramp*ρ`;
   `M2 = MP(|M|*10^(ζ/20))`; `(h2, F2) = Realize(s*, M2)`; stop if
   `err(F2) ≥ err(F)`, else `h, F, M ← h2, F2, M2`.
8. The correction is `F_total = F*P2`: FIR h and PEQ B2.

**Phase resolution (linear, x ms).** Run steps 1–8 with β = 24 and with β = 6.
Score each by `GD − median(GD)`'s maximum, where
`GD = GroupDelay(comb_p*F_total)` on `geomspace(max(lo,20), hi, 1200)` and the
position policy takes the worst position. Keep the lower score, β = 24 on a tie.
`GroupDelay(X)`: `t = argmax|irfft_N(X)|/fs`, `Xs = S_24(X*exp(j*2π*f*t))`,
`c = interp(grid, f, Re Xs) + j*interp(grid, f, Im Xs)`,
`GD = −gradient(unwrap(angle(c)), 2π*grid)*1000` ms.

### MIMO supports

Run after the RC, on the aligned in-ear BRIRs before any tuning. Positions are
the left and right ear, equally weighted. Inputs are the measured speakers (they
have an RC), in speaker order; every speaker with a BRIR may support.

- **Long spectra.** As the RC window but 2 s long with a 10% taper:
  `w = h[a : a+2*fs]`, last `int(0.1*len(w))` samples half-Hann tapered;
  `B = rfft_N(w)*exp(−j*2π*f*(a−b)/fs)`, b the earliest left-ear onset over all
  speakers (any common b gives the same supports).
- **Support bands.** For each speaker: `D = 10*log10(S_3(mean_e |B_e|²) + 1e-30)`,
  `ref = median D over 200–1000 Hz`, `start = min(200, Fsiso)`. Walking
  `geomspace(start, 15, 200)` (D interpolated), `low` is the first point below
  `ref − 10` (15 if none), raised to at least 50; walking
  `geomspace(start, Fsiso, 200)`, `high` is the first point below `ref − 10`
  (Fsiso if none). The band is `(round(low,1), round(high,1))` if
  `low < 0.8*min(high, Fsiso)`, else the speaker does not support. A configured
  `room_tuning2_mimo_low` raises low to at least itself and drops the band when
  that low is not below `0.8*high`.
- **Paths at 600 Hz.** `n = 720`. Input `x_e = LR(B_e,i * 10^(g_i/20) * F_total,i, n)`;
  support path `G_e,j = B_e,j * HP4(f; low_j)`, `γ_e,j = LR(G_e,j, n)`;
  `ℓ = firwin(31, Fsiso, fs=600)` (Hamming, unit DC gain). Candidates C for input
  i: the supporting speakers except i, R = |C|.
- **Least squares.** Per ear: `k_e = first index with |x_e| > 0.1*max|x_e|`,
  late window `L_e[t] = 1` for `t ≥ k_e + 36` (60 ms), else 0. For each
  candidate `c_e,j = (γ_e,j ⊛ ℓ)[:n]` and the n × 180 convolution matrix
  `C(c)[t, k] = c[t−k]` (t ≥ k). Stack `A_e = [C(c_e,1) … C(c_e,R)]`. Rows: the
  late blocks `L_e·A_e` for left then right, then the early blocks
  `√10·(1−L_e)·A_e`; right side `−[L_left·x_left; L_right·x_right; 0; 0]`.
  Solve `(AᵀA + λI) u = Aᵀb`. The solver is free as long as it solves this
  system: the normal matrix has Toeplitz-like blocks that may be built directly,
  and any accurate symmetric factorization may be used.
- **Strength by bisection.** Split u into R blocks of 180 taps; support FIRs
  `s_j = ℓ ⊛ u_j` (210 taps); `ratio = Σ_j ||s_j||² / max(||LR(F_total,i, n)||², 1e-30)`.
  With `σ = mean(diag AᵀA)`, start `lo = −8`, `hi = 4` (log10 of λ/σ) and the
  solution at hi; 18 times take `mid = (lo+hi)/2`; if `ratio(σ*10^mid) > 10^(−1.2)`
  set lo = mid, else set hi = mid and keep that solution.
- **Guard.** Try scales 1, then −3, −6, −9, −12 dB. With
  `y_e = x_e + Σ_j (γ_e,j ⊛ scale*s_j)[:n]`, accept the first scale whose band
  change and tail change are both at most 1 dB; none accepted → this input has
  no supports (`cli_room_tuning2_mimo_off`).
  *Band change:* on `rfftfreq(1440, 1/600)`, bands `[e_m, e_(m+1))` between
  consecutive edges `e = 20*2^a`, a the elements of
  `numpy.arange(0, log2(Fsiso/20) + 1e-9, 1/3)` (20–127 Hz for 150 Hz), plus `[p − 1.5, p + 1.5)` for each p in `LP({x_e}, 6)`. Sum the band energy of
  `|rfft_1440(L_e*x_e)|²` (before) and `|rfft_1440(L_e*y_e)|²` (after) over both
  ears; the change is the largest `10*log10((after + 1e-30)/before)` over bands
  with energy before (−99 if none).
  *Tail change:* modes = sorted set of `round(p, 1)` over `LP({x_e}, 6)` and
  `LP({y_e}, 6)`. For each mode, order-2 Butterworth band-pass
  `[max(p−1.5, 1), p+1.5]` at 600 Hz; a = Σ_e Σ_(t ≥ k_e+180) of the filtered
  x_e squared, b the same for y_e (same k_e). Over modes with `a > 0` and
  `a ≥ 0.01*max a`, the largest `10*log10((b + 1e-30)/a)` (−99 if none).
- **Result.** `s_ij = scale*s_j`, level `10*log10(max(ratio*scale², 1e-30))` dB,
  logged per input as `cli_room_tuning2_mimo`.

SECS realizes these supports as IIR chains fitted to the FIR result (its
low-latency DSP cannot run long FIRs); that fit dominated its run time. The
BRIR has no tap or latency limit, so the FIR supports are used directly. SECS's
three-position minimum does not apply either: it exists because a loudspeaker
room must be sampled to generalize, while the BRIR is fixed at the ears.

### Applying to the BRIR

`D_s = round(lift*fs/1000)` (0 for `zero`). For measured speaker i, ear e:
`f_i = irfft_N(F_total,i)`; for each support
`q_ij = irfft_N(HP4(f; low_j) * S_ij(f) * clip((285 − f)/60, 0, 1))` with
`S_ij(f) = Σ_n s_ij[n]*exp(−j*2π*f*n/600)`. The tuned response is the first
`len(BRIR_i,e) + D_s` samples of the linear convolution
`10^(g_i/20)*(BRIR_i,e ⊛ f_i) + Σ_j BRIR_j,e ⊛ q_ij` (all BRIR_j,e untuned), with
the last 5 ms faded by a falling half-Hann. Unmeasured speakers are only shifted
by D_s. The frequency-domain taper reproduces the band limit the design saw;
its small non-causal part wraps to the end of q_ij and falls outside the kept
samples.

### Diagnostics

Log `cli_room_tuning2` once with mode, latency (ms), lo and hi (Hz, rounded).
v1's geometry check (`cli_room_tuning_mismatch`), representativeness
(`cli_room_tuning_weak`), pre-echo and the information-only LF EDT and median
excess group delay apply unchanged to the v2 result (with MIMO included). The
pre-echo warning uses its own key, `cli_room_tuning2_preecho`, because the cure
is a lower-latency mode rather than a shorter delay. Per input with MIMO log
`cli_room_tuning2_mimo` (speaker, number of supports, level in dB to one
decimal) or `cli_room_tuning2_mimo_off`. The README section lists mode, latency, band
and its source, per speaker points, level, trim and source, PEQ band count,
phase strength and resolution, long modes, representativeness, pre-echo,
EDT and excess group delay before/after, and with MIMO each input's supports,
level and guard scale.

### Reference acceptance

The port is gated like the 2.x migration: `tests/migration/export_goldens_secs2.py`
runs SECS 2 on fixed inputs and freezes every function and stage boundary as an
`s2_*` fixture (primitives, every PEQ fit with its full trust-region trace,
each correction run's intermediate spectra and FIR realizations, MIMO paths,
normal equations, bisection, guard and supports). `tests/migration/README-secs2.md`
documents the families, the encoding and the measured budgets. The end-to-end
references below sit on top of them.

The oracle runs with NumPy's SIMD dispatch limited to the X86-64-v2 baseline, so
its `log10`/`exp` are the C library's like Rust's. That matters: SECS 2 itself,
on the same data, changes its corrections by up to 2.66 dB, and 7–9 of 61
channels by more than 1 dB, when only the last bits of some operations change
the way another computer changes them (NumPy's AVX-512 kernels, OpenBLAS kernels
of older CPUs, LAPACK's other SVD driver): its PEQ refinement usually stops at
the evaluation cap and its path is determined only for the first 1–15
evaluations. Equivalence is therefore stage by stage on the oracle's own inputs
(fit traces up to the oracle's own horizon), and end to end within the oracle's
own spread across computers (`tests/migration/README-secs2.md`, "SECS against
itself").

MIMO follows the same split. Fed the oracle's own RC outputs (`x` per ear and
the direct correction energy), the paths, normal equations, bisection, guard
and supports reproduce it to floating-point noise (supports within 6e-13
relative). Two guard values (the first tail change of BL and FC in the
`mixed`/100 Hz/40 Hz setting) first disagreed: the port read "the sum masked
to 20–160 Hz" as a search restricted to that band, while the oracle multiplies
the whole grid by the mask, so its 159.96 Hz band-edge bin is a peak (see
"Late peaks" above, now stated explicitly). With the edge bin, the
oracle's own late-peak function reproduces both recorded values exactly.
End to end, the tail change is a maximum over a top-six peak set and so is
discontinuous: a slightly different RC can move a near-threshold input by one
guard step (3 dB of support level). The oracle's own variants do so for 1–3 of
7 inputs per setting; the allowance over the worst variant is recorded next to
`room_tuning2_matches_secs2_mimo`.

`tests/migration/goldens/room_tuning_secs2_rc.json` holds SECS's correction
responses (|F_total| dB and phase with the lift removed, at FFT bins of
N = 131072) for 62 runs on the demo omni files: every file/speaker as one
position (`mixed`), every speaker with both ears in all five modes (60 ms for
ultra), both ears with SECS's own listening-position weight 5, the demo room
target, an automatic band from V = 40 m³, a manual 30–300 Hz band, a 6 dB
boost limit, and all seven speakers in one run for the channel levels. Each
entry stores SECS's PEQ bands, every intermediate PEQ fit, phase strength and
resolution and, for one-position entries, the design curve. Tests use SECS's
position policy (listening position 1, its weight) to compare.
`tests/migration/goldens/room_tuning_secs2_mimo.json` holds SECS's support
bands, support FIRs, levels and guard results for three settings on the
aligned demo BRIRs, after an RC on the same BRIRs. Bounds are set from the
measured agreement and recorded with the tests.

### Deliberately not reproduced

Subwoofer alignment (BA), subwoofer bundles and DBA, two-way subwoofers,
bass-management crossovers and their PEQ, the subwoofer bus, GD cuts and the
no-boost mask (both off by default in SECS), the IIR realization of the
supports, the per-input support limit (a DSP budget), SECS's arrival-delay
alignment (Impulcifer's alignment already places the speakers),
REW/CamillaDSP export, and SECS's environment knobs (fixed at their defaults:
ring 30 ms, common-phase positions, long-mode boost 3 dB, guard 1 dB, 180
support taps).

Virtual room tuning v2 reproduces the correction method of SECS 2 by the
Speaker Gallery (DCinside), released under GPL-3.0. Impulcifer's implementation
is independent and contains no SECS code; SECS 2 was used only as a black box
to produce the reference numbers.

## Why not invert everything?

A recorded BRIR is fixed at the measured point, so usable low-frequency nulls can be filled in a way that ordinary loudspeaker room EQ cannot safely maintain across listener positions. That does not make a signal below the measurement rolloff usable: it is mostly unavailable measurement energy or noise, not information to amplify. The headphone can reproduce low bass, but the room recording cannot establish its correction there. Virtual bass supplies synthetic bass instead, so room modes must not be imposed onto it.

Above the room-mode/Schroeder region (often a few hundred Hz, not a universal fixed 400 Hz cutoff), interference varies rapidly in space. The half-wavelength spatial period is about 57 cm at 300 Hz and 17 cm at 1 kHz. SNR and increasingly broad smoothing therefore restrict boosts. The omni microphone has no head shadow: its inter-ear differences at higher frequencies must not create artificial binaural level differences. Extreme mode's final diotic blend enforces this while retaining broad tonal correction.
