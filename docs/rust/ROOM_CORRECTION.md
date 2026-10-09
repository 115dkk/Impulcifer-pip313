# Room correction v2

The Rust room stage defaults to `schroeder`. The Python 2.x implementation is unchanged. `room_range=legacy` dispatches to the original room calculations, including their full-Hann limit mask, and produces `RoomTerm::LegacyError`. Oracle tests explicitly select legacy; they do not regenerate goldens or relax tolerances.

## Configuration

| Option | Default | Accepted values |
| --- | --- | --- |
| `room_range` | `schroeder` | `legacy`, `modes`, `schroeder`, `extreme` |
| `room_volume` | unset | cubic metres, greater than 0 and at most 10000 |
| `schroeder_freq` | unset | override in Hz, 50–1000 inclusive |
| `room_max_boost` | 12 | dB, 0–24 inclusive |

`room_mode` defaults to `eq` (`eq` or `tuning`). Tuning is independent of the range and accepts `legacy`; see the processor specification below.

These are extension fields, not additions to the frozen 33-field 2.x list. CLI defaults are suppressed: omitting the flags preserves the old kwargs object. The service bootstrap includes all ten extension defaults. Service validation returns `INVALID_REQUEST` with `details.field`; CLI validation exits 2. `specific_limit` and `generic_limit` apply only in legacy mode.

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
2.20–2.71 dB): no weak warnings. The requested pre-echo formula still warns on
SL-left at the tuning hook: −67.97 → −26.66 dB. The other 13 channels do not
warn. The demo no-pre-echo-warning assertion remains red; thresholds and the
filter have not been retuned to hide this remaining acceptance discrepancy.

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

## Why not invert everything?

A recorded BRIR is fixed at the measured point, so usable low-frequency nulls can be filled in a way that ordinary loudspeaker room EQ cannot safely maintain across listener positions. That does not make a signal below the measurement rolloff usable: it is mostly unavailable measurement energy or noise, not information to amplify. The headphone can reproduce low bass, but the room recording cannot establish its correction there. Virtual bass supplies synthetic bass instead, so room modes must not be imposed onto it.

Above the room-mode/Schroeder region (often a few hundred Hz, not a universal fixed 400 Hz cutoff), interference varies rapidly in space. The half-wavelength spatial period is about 57 cm at 300 Hz and 17 cm at 1 kHz. SNR and increasingly broad smoothing therefore restrict boosts. The omni microphone has no head shadow: its inter-ear differences at higher frequencies must not create artificial binaural level differences. Extreme mode's final diotic blend enforces this while retaining broad tonal correction.
