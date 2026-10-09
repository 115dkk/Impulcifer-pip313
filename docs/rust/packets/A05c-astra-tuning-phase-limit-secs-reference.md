# A05c (ASTRA): virtual room tuning as a processor emulation, independent of room EQ v2

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This reworks the virtual room tuning that revision 2 of `docs/rust/packets/A05-astra-virtual-room-tuning.md` put in the working tree (`crates/impulcifer-dsp/src/stages/room_tuning.rs`, the tuning branch of `room_v2`, the hook in `crates/impulcifer-dsp/src/pipeline.rs`, `crates/impulcifer-dsp/tests/room_tuning.rs`, config/CLI/validation). Read that packet, `docs/rust/ROOM_CORRECTION.md`, `docs/adr/0004-room-correction-v2.md`, `docs/adr/0005-virtual-room-tuning.md` and `CLAUDE.md` (section "3.x Rust 워크스페이스") first. Rules: `#![forbid(unsafe_code)]`, no new dependencies, no shell subprocesses, every command in the foreground. If a build runs out of disk, delete `target/debug/incremental` and retry.

**Clean-room rule (unchanged).** This mode follows SECS, a community room-correction tool whose source has no licence. Do not search for, open or read any third-party room-correction source, and do not open anything under `/tmp/claude-0/`. Implement only from this packet and the repository. The reference numbers in `tests/migration/goldens/room_tuning_secs_reference.json` were produced outside this task by running SECS as a black box.

Do not touch `apps/**`, `crates/impulcifer-service/locales/**` (other workers write the screen and the catalogue at the same time; the log keys you must use are listed below), `Cargo.toml`/`Cargo.lock` versions, `CHANGELOG.md`, `README.md`, `.github/**`, `docs/adr/**`, the Python 2.x tree.

## What changes and why

The owner rejected revision 2's magnitude: it borrowed room EQ v2's gain rule (range, Schroeder, max boost, two-point combination) and applied it through the equalize path. Room EQ (`room_mode = "eq"`) and virtual room tuning (`room_mode = "tuning"`) are two different members of one feature family. Tuning emulates a room-correction processor in each speaker's feed while the BRIRs were recorded, and **all** of it follows that processor (SECS): magnitude, phase, delay, level matching. Nothing of v2 runs in tuning mode. The acceptance rule: on the demo, our tuning must reach the same result as SECS within stated tolerances. See ADR 0005.

The model is unchanged: one filter `F` per speaker, computed from the omni room measurements at the ear positions, convolved identically into both ears of that speaker (`F * BRIR` is what the recording would have been with the processor running; ITD/ILD/IPD untouched).

## Config, CLI, validation

| field | type / default | valid | notes |
| --- | --- | --- | --- |
| `room_mode` | `"eq"` | `eq`, `tuning` | unchanged |
| `room_tuning_delay` | string spec, default `"10"` | `"auto"` or a number 2–20 (ms) | was f64; JSON accepts a number or a string (a number is stored as its decimal string, like `channel_balance`). Parse into `enum TuningDelay { Auto, Ms(f64) }` |
| `room_tuning_phase_limit` | string spec, default `"full"` | `"off"`, `"schroeder"`, `"full"`, or a number 300–20000 (Hz) | JSON number or string. `enum PhaseLimit { Off, Schroeder, Full, Hz(f64) }`; a number ≥ 0.45·fs is `Full` |
| `room_tuning_max_boost` | f64, 6 | 0–12 dB | new |
| `room_tuning_curtain` | f64, 300 | 100–5000 Hz | new |
| `room_tuning_level_match` | bool, true | | new |

- All in `EXTENSION_FIELD_NAMES` and CLI `EXTENSION_OPTIONS` (`--room_tuning_delay`, `--room_tuning_phase_limit`, `--room_tuning_max_boost`, `--room_tuning_curtain`, `--room_tuning_level_match`; for the boolean follow the CLI's existing convention for a default-true boolean if there is one, otherwise take a value `true|false`). Service validation (`invalid_request` naming the field), `from_kwargs`, the Python filter, `brir_defaults`, `features.toml`, the policy gate's `CANONICAL_CONFIG`.
- **Remove** the rejection of `tuning` with `room_range = "legacy"`: `room_range`, `room_max_boost` are inert in tuning mode (they belong to room EQ). `room_volume` and `schroeder_freq` stay meaningful in tuning only for the Schroeder estimate used by `phase_limit = "schroeder"`.
- CLI help: `--room_mode`: `Room correction mode. "eq" corrects the frequency response at each ear position. "tuning" applies, per speaker, the correction a room-correction processor in the speaker feed would have applied during the recording (frequency response, speaker level and timing; the same filter for both ears) and delays every channel by --room_tuning_delay. --room_range and --room_max_boost apply to "eq" only.` Write short help lines for the five tuning options in the same style ("Used only with --room_mode tuning.").

## Where it runs

- **Room stage** (`room_v2::room_correction`, tuning branch): build the per-speaker FIRs and diagnostics into `RoomCorrection.tuning` (a `TuningPlan`). In tuning mode the `RoomFrs` passed to equalize must hold **no room term** (equalize runs as if room correction were off; its 5 Hz FIR resolution would blur this mode's 1/24-octave low-frequency work).
- **Pipeline**: apply the plan at the end of the `CropAndAlign` branch (after `crop_heads`, `align_ipsilateral_all`, `align_onset_groups_peak_leftref`, `crop_tails`), i.e. before `VirtualBass`, so every later analysis sees `F * BRIR` exactly as the processor would have produced it. Remove the revision 2 hook from the `Equalize` branch. No new stage row or progress step (`golden_stage_table_matches_python` and progress totals unchanged). After convolving, keep each IR at its previous length plus D samples (the correction shortens decays; the extra convolution tail is dropped) with a 5 ms half-Hann fade-out at the new end.
- **Timing invariant**: every speaker's FIR has its design time zero at exactly D samples (see "Final FIR"), so every channel moves by exactly D and the inter-speaker timing set by the alignment stages is untouched. Impulcifer's alignment stages already do what the processor's inter-channel delay matching does (distance compensation); do **not** add any per-speaker shift.
- No virtual-bass gate is needed any more (the virtual bass stage replaces that band after the tuning). Remove the LR8 gate from the tuning path.

## Inputs per speaker

The omni ear-position IRs of that speaker (`room-<SPK>-left.wav`, `room-<SPK>-right.wav`), as split by the room stage, `crop_head(1 ms)` applied, then the first `fs` samples (zero-padded if shorter): analysis length **N = fs** (48000 at 48 kHz; FFT at that length). Subtract the room mic calibration from |H| if one is given (magnitude only).

- Two files → two points. One file → single point, the full chain below on that point; log `cli_room_tuning_single_point`.
- `room.wav` is **not used** in tuning mode (it is one point of usually one speaker; the chain is speaker-specific). If it is present log `cli_room_tuning_room_wav_ignored` once.
- No speaker with an ear-position file → no tuning at all, no delay; log `cli_room_tuning_no_data`.

## The processor (per speaker)

Notation: `f` bin frequencies of the N-point FFT, `k = 1 … N/2` positive bins.

**Octave smoothing `S_b(X)`** (fraction 1/b, works on complex or real): for bin k, `w = f_k·(2^(1/(2b)) − 2^(−1/(2b)))`, `m = max(1, floor(w / Δf / 2))`, average of `X` over bins `[max(1, k − m), min(N/2 + 1, k + m + 1))` (half-open; computed with a cumulative sum); bin 0 unchanged; the negative half filled as the conjugate mirror (real mirror for real data); at even N the Nyquist bin is made real.

**Five bands**: `L(fc) = 1 / (1 + (f/fc)^4)`; weights `w0 = L(100)`, `w1 = (1 − L(100))·L(150)`, `w2 = (1 − L(150))·L(200)`, `w3 = (1 − L(200))·L(300)`, `w4 = 1 − L(300)` (use |f|). Five-band smoothing `B(X) = Σ w_i·S_{b_i}(X)` with the **Normal** set `b = (24, 12, 6, 3, 1)`; the **Low** set `(12, 6, 3, 2, 1)` is used once below. Keep both sets in one constant table.

**Minimum phase** of a magnitude `A`: real cepstrum of `ln(A + 1e-12)`, lifter `[1, 2 … 2, 1 (at N/2), 0 …]`, `exp(FFT(...))`.

### Magnitude spectrum of the speaker

`|H| = sqrt((|H_1|² + |H_2|²) / 2)` for two points (spatial power average), `|H_1|` for one. Everything in "Magnitude" uses this one spectrum.

### Phase (excess-phase inverse), unless `phase_limit = off`

Per point j: `p_j` = index of max |h_j|; `E_j = H_j · exp(+j·2π·f·p_j/fs) / H_min,j` (H_min,j = minimum phase of |H_j|); level regularisation `r_j = clip((S_3(|H_j|) / mean_{100 Hz < f < 10 kHz} S_3(|H_j|) − 0.177) / 0.5, 0, 1)`.

- One point: `X = conj(E_1)`, `G = r_1`.
- Two points: `C = S_3(|H_1|)·E_1 + S_3(|H_2|)·E_2`, `X = conj(C/|C|)`, `G = min(r_1, r_2) · a` where `a` is revision 2's two-point agreement gate (1 at |Δφ| ≤ 60°, 0 at ≥ 120°).
- Band gate by `phase_limit`: `full` → none; `schroeder` → `f_ph = min(f_S, 300)`; `Hz(x)` → `f_ph = x`; for both, 1 below `f_ph/√2`, half-Hann to 0 at `f_ph`. Multiply into G.
- SNR floor (safety only): when `room_v2::snr_db` is available for every point, multiply by 0 at ≤ 10 dB, 1 at ≥ 20 dB (min over points); when it is not available, no SNR factor and no log. This must not bind on the demo (the demo test asserts it); if it does, report instead of tuning it.
- No rolloff gate, no local-level gate, no residual linear-phase fit (revision 2 had them; SECS has none: `r` already fades where the level is low, and per-point peak alignment removes the bulk delay).
- `X ← G·X + (1 − G)`, then `X ← B(X)` (complex, Normal), `X ← X/|X|`, `x = IFFT(X)` (real part).
- Pre-ringing windows, with D the tuning delay in ms: `pr_low = clip(D, 2, 25)`, `pr_mid = min(10, pr_low/2)`, `pr_high = min(5, pr_low/4)`. Window `W(pre)` on the circular time axis `t = ((n + N/2) mod N) − N/2`: 1 at t = 0 and for all t > 0 (post side open), half-Hann `0.5·(1 + cos(π·t/P))` for `−P ≤ t < 0` with P = pre in samples, 0 before. `H_ex = (w0 + w1)·FFT(x·W(pr_low)) + (w2 + w3)·FFT(x·W(pr_mid)) + w4·FFT(x·W(pr_high))`, then `H_ex ← H_ex/|H_ex|`.

### Magnitude

1. `M = B(|H|)` (real, Normal); for f < 15 Hz, `M = min(M, max(M over 15–20 Hz))`.
2. `ref` = 35th percentile (linear interpolation) of raw `|H|` over 100 Hz < f < 10 kHz.
3. Low cutoff: `M_dB = 20·log10(M)`; reference = mean of `M_dB` over 1–10 kHz; the first f in 10–1000 Hz with `M_dB ≥ reference − 6` (clipped to 10–300 Hz; 300 if none). High cutoff: over 2–22.05 kHz, reference = 20·log10 of the 95th percentile of M there; the last f with `M_dB ≥ reference − 10` (clipped to 6–20 kHz; 16 kHz if none). Report both per speaker.
4. Track weight `t = clip(t_low + t_high + t_noise, 0, 1)`: `t_low = clip((1.15·lc − f)/(0.15·lc), 0, 1)`; `t_high = clip((f − 0.8·hc)/(0.2·hc), 0, 1)`; `t_noise`: `thr = mean(M over 200–1000 Hz)·10^(−30/20)`, `clip((2·thr − M)/thr, 0, 1)`, set to 0 for f ≥ 200 Hz.
5. `natural = M/ref`; `env = min(1, natural)`; `flat = (f⁴/(f⁴ + 1))·(20000⁴/(f⁴ + 20000⁴))`; `target = env^t · flat^(1−t)`; above 1 kHz `target = min(target, flat)`. If a room target curve is configured, multiply `target` by it (linear, after removing its mean dB over 100 Hz–10 kHz).
6. `inv = clip(target/natural, 0, 10^(max_boost/20))`; `inv[0] = inv[1]`.
7. Curtain c: `fade = clip((f − c)/(min(2c, fs/2 − 1) − c), 0, 1)`; `inv ← inv·(1 − fade) + fade`.
8. `m = IFFT(minimum phase of inv)`. Unless `phase_limit = off`: `Hm = FFT(m·W2(pr_low, 500 ms))·(1 − fade) + FFT(m·W2(pr_low, 10 ms))·fade`, where `W2(pre, post)` is `W(pre)` with the post side also half-Hann over `post`. With `off`: `Hm = FFT(m)` (no windows).
9. `initial = H_ex · Hm · exp(−j·2π·f·D_s/fs)` with `D_s = round(D·fs/1000)`; with `off`: `initial = Hm` and D = 0.
10. Tone layer: `sim = |H|·|initial|`; `tonal = B_Low(sim)` (real); `macro = clip(ref·target/tonal, 0.5, 2.0)`; where `macro > 1`: `macro ← 1 + (macro − 1)·clip((20000 − f)/10000, 0, 1)`; `macro ← S_1(macro)` (real).
11. Peak reduction: `res = sim·macro/(ref·target)`; `peaks = S_48(max(res, 1))` (real); `crush = (1/peaks)^0.6`; `cw = clip((500 − f)/200, 0, 1)`; `crush ← (1 − cw) + crush·cw`.
12. `post = macro·crush`, `post[0] = post[1]`; `F = IFFT(initial · minimum phase of post)` (real part), length N, design time zero at `D_s`.

### Level matching (`room_tuning_level_match`, default on)

Per speaker with room data, `trim_i = median_j(ref_j)/ref_i` over the speakers with room data, clipped to ±6 dB, multiplied into F (same scalar for both ears). Cross-check: the in-ear BRIR power level of each speaker (both ears summed, 100–500 Hz, after alignment) gives an independent level ratio to the same median set; if the omni ratio and the in-ear ratio differ by more than 2 dB for a speaker, skip that speaker's trim and log `cli_room_tuning_level_inconsistent` (speaker, db = the difference, 1 decimal). Speakers without room data are untouched. Report per speaker: trim dB and source (`omni` / `skipped`).

### Delay (`room_tuning_delay`)

- A number: that D for every speaker.
- `auto`: one D for the whole run from 2, 3, … 10 ms. For each candidate build every speaker's F, then score: for every symmetric speaker pair present (`IPSILATERAL_PAIRS` minus the self-pair `FC`) and every ear position where both speakers of the pair were measured **in the same recording file** (pairs split over different files have no common time reference and do not score; on the demo only FL/FR scores), take the two speakers' IRs from that recording with their **common** time reference (both cropped by the same amount, 1 ms before the earlier peak; the room stage's per-speaker `crop_head` destroys this, so keep the uncropped split for this), `Y = H_A·F_A + H_B·F_B`, coherence `q(f) = |Y| / (|H_A·F_A| + |H_B·F_B|)`; score of that pair and position = weighted mean of q over `max(10 Hz, mean low cutoff of the two) ≤ f ≤ 300 Hz` with weights `w(f) = 1 + clip((300 − f)/(300 − f_low), 0, 1)` (2 at the low end, 1 at 300 Hz). Run score = sum over pairs and positions. Pick the highest; ties (within 1e-9) → the smaller D. No pairs → 10 ms. Log the chosen D in `cli_room_tuning`.

### Final FIR

Take `F` from index 0 (design time zero is at `D_s`, the pre-ringing lies in `[0, D_s)`), length `T = round(0.51·fs)`, fade the last 5 ms with `(1 − n/len)²`. This is the FIR convolved into both ears.

## Three problems revision 2 left open (fix them here)

1. **The filter barely corrected its own measurement.** On the synthetic case revision 2 reduced the omni excess group delay RMS by only 2.27 % (required ≥ 50 %). The chain above replaces the parts that are the likely causes (100 ms post window on the all-pass, residual linear-phase fit, rolloff and local-level gates), but do not assume: measure it again, and if `F` applied to the measurement it was built from (single point, `phase_limit = full`) does not reduce that point's 1/6-octave excess group delay RMS over 30–300 Hz (bins where G ≥ 0.9) by ≥ 50 %, find the step that is wrong (sign of the conjugate, peak alignment, window side, normalisation order) before going on. The reference test must agree with the same conclusion.
2. **The session check compared absolute arrival times across separate recordings.** Every room file is its own recording with its own playback/recording latency, so arrival times from different files cannot be compared (on the demo it flagged five of seven speakers: e.g. SR omni right−left +77 samples, 1.6 ms, more than any interaural delay). Replace it: compare only arrivals **within one recording**. For a room file holding two speakers (e.g. `room-FL,FR-left.wav`) take the arrival difference between its two speakers, and compare it with the same two speakers' arrival difference in the BRIR recording that holds both (same ear, before `crop_heads`). Warn with `cli_room_tuning_mismatch` (per speaker of that pair) when the signs differ while both magnitudes exceed 0.1 ms, or when they differ by more than 0.5 ms. Files with one speaker (FC) and pairs not recorded together in both measurements are not checked. The demo must not warn; report the numbers per pair.
3. **The demo BRIR's low-frequency excess group delay got worse** in revision 2 (e.g. FL 48.96 → 54.36 ms RMS, BL 27.09 → 34.26). Measure it again after this rework, and also report per speaker how well the omni ear-position measurement represents the in-ear recording below 300 Hz: the RMS difference in dB between the omni magnitude at an ear position and the BRIR magnitude of the same ear (both 1/6-octave smoothed, level-matched over 100–300 Hz), 30–300 Hz. That tells whether the demo's room files come from the same session and position as its BRIRs; do not tune anything to it.

## Logs (the catalogue worker writes these keys; use exactly these)

| level | key | args |
| --- | --- | --- |
| info | `cli_room_tuning` | `freq` (int Hz: f_ph, `round(fs/2)` for full; 0 for off), `delay` (int ms) |
| info | `cli_room_tuning_off` | — (phase_limit off: frequency response and level only, no delay) |
| info | `cli_room_tuning_single_point` | `speaker` |
| info | `cli_room_tuning_room_wav_ignored` | — |
| info | `cli_room_tuning_no_data` | — |
| warning | `cli_room_tuning_level_inconsistent` | `speaker`, `db` |
| warning | `cli_room_tuning_preecho` | `speaker`, `db` (as revision 2) |
| warning | `cli_room_tuning_mismatch` | `speaker` (as revision 2) |
| warning | `cli_room_tuning_weak` | `speaker` (as revision 2) |

Stop using `cli_room_tuning_magnitude_only`, `cli_room_tuning_no_phase`, `cli_room_tuning_no_snr` (the catalogue worker removes them). Readme (WriteReadme stage): a "Virtual room tuning" section with the chosen delay, the phase limit, and per speaker: points used, low/high cutoff, level trim and source, pre-echo dB, BRIR excess group delay before/after below 300 Hz. In tuning mode do not draw the room EQ per-ear room gain curves in the room plots (they do not apply); if the plotting code makes a per-speaker tuning plot easy (|F| and excess group delay before/after), add it, otherwise report it as undone.

## Tests (DSP in `crates/impulcifer-dsp/tests/room_tuning.rs` unless noted; retire revision 2 tests that encode its magnitude rule, the local-level/rolloff gates or the residual fit, and say which)

- `room_tuning_excess_part_is_all_pass`: |H_ex| = 1 ± 1e-6 after normalisation; `room_tuning_uses_one_filter_per_speaker` (both ears bit-identical FIRs; interaural ratio unchanged to 1e-9).
- `room_tuning_magnitude_follows_rolloff`: synthetic 4th-order 45 Hz high-pass speaker plus a +8 dB mode at 70 Hz and a −15 dB dip at 120 Hz: no boost below the low cutoff, the mode cut, the dip boost ≤ max_boost, |inv| → 1 above 2c.
- `room_tuning_tone_layer_is_bounded`: a 1/1-octave −10 dB shelf above 2 kHz is corrected by at most +6 dB; boosts fade to 0 at 20 kHz.
- `room_tuning_two_points_use_power_average`: a −20 dB node at one point and 0 dB at the other is treated as −3 dB (boost ≤ 3 dB plus tone-layer effect stated in the test).
- `room_tuning_keeps_speaker_timing`: every channel's direct peak moves by exactly D_s (tuned vs untuned) and inter-speaker peak offsets are unchanged; `room_tuning_off_adds_no_delay`: with `off`, no shift, and |F_off| vs |F_full| within the measured bound (report it; Fable expects ≤ 0.1 dB).
- `room_tuning_reduces_room_excess_group_delay` and `room_tuning_pre_echo_is_buried` (as revision 2, full band).
- `room_tuning_level_match_uses_median`: three speakers with omni levels 0, −3, −10 dB → trims −3, 0 (median speaker), +6 (clipped); `room_tuning_level_match_skips_inconsistent` (omni ratio 3 dB off the in-ear ratio → skipped, logged).
- `room_tuning_auto_delay_is_one_value`: result in 2..=10, equals the arg-max of the reported scores, identical on a second run, the same for every channel.
- `room_tuning_detects_swapped_files`: swapping a two-speaker pair's left/right ear-position files triggers the new within-recording check.
- **`room_tuning_matches_secs_reference`** (may be its own file `tests/room_tuning_secs.rs`): for every file and speaker in the golden, build the input exactly as described in its `ir` field (room stage split of `data/demo/<file>` with the demo estimator `data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`, `crop_head(1 ms)`, first 48000 samples), run the single-point chain with `phase_limit = full`, `curtain = 300`, `max_boost = 6`, D = 10 ms, no level matching, no target, no calibration, and take **F before the final crop** (N = 48000; the reference FIRs were not cropped either). Derive on our F exactly as the golden's `derived` field says: `mag_db = 20·log10(max(|FFT(F)|, 1e-12))`; `E = H/H_min·exp(+j·2π·f·0.010)`; `excess_gd_ms = −d(unwrap(angle(E)))/d(2πf)·1000` (numerical gradient over positive bins 1 … N/2 − 1); both smoothed onto `grid_hz` as the mean over bins in `[g·2^(−1/24), g·2^(+1/24)]` (linear interpolation if no bin falls inside). Compare on **all 250 grid points**:
  - magnitude, both curves minus their own mean over 100 Hz–10 kHz; bands 20–100, 100–300, 300–600, 600 Hz–10 kHz, 10–16 kHz: RMS and max |Δ| dB;
  - excess group delay where our G ≥ 0.9; bands 20–40 (report only), 40–100, 100–300, 300–1000, 1000–5000 Hz: RMS, median and max |Δ| ms and the kept fraction;
  - low and high cutoff vs the golden's `low_cutoff_hz` / `high_cutoff_hz`: ratio in octaves.
  Print per file/speaker and aggregate (median, max over the 14), and a table for FL in `room-FL,FR-left.wav` at 30, 40, 50, 63, 80, 100, 125, 160, 200, 250, 300, 500, 1000, 2000, 5000, 10000 Hz (golden vs ours: mag relative, excess GD, G). Assert **provisional** bounds = measured max × 1.5 rounded up to 0.1 dB / 0.1 ms, each a named constant at the top with `// provisional: set by the owner after review`; cutoffs within 1/12 octave. If any band's measured max exceeds 3 dB or 5 ms, do not just widen: find the step that differs from this packet's description and report it.
- Service (`crates/impulcifer-service/tests/room_v2.rs`): `room_tuning_runs_on_the_demo` (finite outputs; `cli_room_tuning` with delay 10; every channel exactly 10 ms later than the `eq` run; report the normalisation gain change; no `cli_room_tuning_mismatch`; SNR floor not binding), `room_tuning_auto_delay_on_the_demo` (chosen D in 2..10, logged), `room_tuning_accepts_legacy_range` (runs, same output as with `schroeder` since the field is inert), `room_tuning_off_on_the_demo` (no delay), and an `eq` run logs no `cli_room_tuning*` key. CLI: `cli_room_v2_options_parse_and_validate` covers all tuning options (valid, out of range, `auto`, `off`, `schroeder`, `full`, NaN).
- `features.toml`: `config.room_tuning_delay`, `config.room_tuning_phase_limit`, `config.room_tuning_max_boost`, `config.room_tuning_curtain`, `config.room_tuning_level_match`, `room.virtual_tuning`, `room.tuning_level_match`, `room.tuning_auto_delay`, `room.tuning_secs_reference`, all `implemented` with their tests.

All existing tests, goldens and A03/A03b tests pass; the `eq` mode output is bit-identical to before this packet.

## Docs

Rewrite the "Virtual room tuning" section of `docs/rust/ROOM_CORRECTION.md` to this packet: the model, independence from room EQ, the hook position, the inputs (two points, one point, room.wav not used), every formula and number above, level matching with the cross-check, the delay (fixed / auto), the phase limit stops, the final FIR, the timing invariant (and why the processor's inter-channel delay matching is already done by the alignment stages while left/right ear matching would destroy the ITD), the reference check (what is compared, bands, provisional bounds, which SECS behaviour is deliberately not reproduced: its delay auto-selection range above 10 ms is not offered, its per-pair shift, its stereo gain pre-scale, and its low-latency output, which leaves a near-empty channel on three of the four demo pairs), and the attribution line: "Virtual room tuning reproduces the correction method of SECS by 한플 (DCinside speaker minor gallery, 2026). Impulcifer's implementation is independent; SECS was used only as a black box to produce the reference numbers."

## Verification (foreground; paste tails)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy -p impulcifer-python --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy --no-fail-fast
git status --porcelain
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails here because the container runs as root; it must be the only failure. A single command longer than 10 minutes is moved to the background by your shell tool, so run `cargo test` per crate and, for `impulcifer-service`, per test target (`--lib`, `--test room_v2`, …), each in the foreground. `ui_catalog` may fail only on keys the catalogue worker has not added yet; list them.

## Report format

(1) API as landed; (2) interpretations with the choice made; (3) the reference comparison (aggregate, per file/speaker, the FL table, the provisional bounds); (4) other new tests with measured numbers (demo: chosen auto D, per speaker points used, cutoffs, trims and source, pre-echo, BRIR excess GD change, normalisation gain eq vs tuning); (5) retired tests; (6) `test result:` lines and failures; (7) `git status --porcelain`; (8) anything undone. Do not end your turn before the commands complete.
