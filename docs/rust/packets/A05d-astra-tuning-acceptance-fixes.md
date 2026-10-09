# A05d (ASTRA): virtual room tuning — acceptance fixes after A05c

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). This continues `docs/rust/packets/A05c-astra-tuning-phase-limit-secs-reference.md`, which you implemented in the working tree (read your own code and that packet again; the A05c rules, clean-room rule and forbidden files all still apply). Every command in the foreground; run `cargo test` per crate and, for `impulcifer-service`, per test target.

The reference comparison already passes closely (magnitude ≤ 0.004 dB and excess group delay ≤ 0.003 ms above 300 Hz on all 14 entries). These are the remaining decisions and fixes.

## 1. Delete the SNR floor

It bound on the demo (factors 0.68–0.91 for FR, SL, SR, BR); SECS has no such gate, and the A05c rule was "if it binds on the demo, delete it rather than tune it". Remove it from the phase gate entirely (and any SNR plumbing that only served it). Re-run the reference comparison: the 20–300 Hz differences are expected to shrink because `G` is now exactly SECS's `r` for one point. Report the new numbers in the same tables.

## 2. Replace the session check with an omni-only geometry check

Comparing the omni within-recording arrival difference with the BRIR's is wrong: in the ear the contralateral speaker's sound goes around the head, so the two legitimately differ by 0.3–0.6 ms (the demo shows exactly that). New rule, using only the omni files: for every **symmetric** pair (`IPSILATERAL_PAIRS` minus `FC`) recorded **in the same room file**, with arrival = index of max |h| in that recording, at the left ear position the left speaker must arrive first and at the right ear position the right speaker first, whenever the difference exceeds 0.05 ms. A reversed order warns `cli_room_tuning_mismatch` for both speakers of the pair (swapped left/right files or wrong speaker order). Pairs that are not symmetric (BL/SL, SR/BR on the demo) and single-speaker files are not checked. The demo must not warn. `room_tuning_detects_swapped_files`: swapping `room-FL,FR-left.wav` and `room-FL,FR-right.wav` warns for FL and FR.

## 3. Auto delay: use SECS's own criterion

Your coherence score picked 2 ms on the demo; SECS picks 10 ms for the same pair. Replace it with SECS's criterion, evaluated per **two-speaker room file** (any two speakers recorded together, symmetric or not, both ear positions), and check it against `tests/migration/goldens/room_tuning_secs_auto_delay.json` (SECS's own choices per file: 10, 10, 7, 10, 10, 10 ms; its `ir.construction` says how SECS's input was built — build ours the same way for this test).

Per file and candidate D (2, 3, … 10 ms), with A = first speaker, B = second speaker of the file, N = 48000 samples of each from the common start:

1. `F_A`, `F_B` = the single-point chain of each speaker at delay D (full length N, before the final crop), no level matching, `phase_limit = full`, settings as configured.
2. Level: `F_B ← F_B · rms_A / rms_B`, where `rms_X` = RMS of `h_X` over `[p_X − w, p_X + w)` with `w = round(0.005·fs)`, `p_X` = index of max |h_X|.
3. Arrival alignment for scoring only: shift the filter of the earlier speaker right by `(max(p_A, p_B) − p_X)` samples (zero-fill, no wrap).
4. Crop both to `T = round(0.51·fs)` samples starting at `q − D_s`, where `q` = the sample index of the largest absolute value over both shifted filters (zero-fill outside).
5. `Y = H_A·FFT_N(crop_A) + H_B·FFT_N(crop_B)` with `H_X = FFT_N(h_X)` (uncropped input).
6. `after_dB = 20·log10(B(|Y|) + 1e-12)` (five-band magnitude smoothing, Normal); `target_dB = 20·log10(ref_A·target_A)` (speaker A's processor target including its `ref`).
7. Band: `f_low = max(10, (lc_A + lc_B)/2)` ≤ f ≤ 300 Hz; weights `w(f) = 1 + clip((300 − f)/(300 − f_low), 0, 1)`; `e = after_dB − (target_dB + 6)`; `abs = Σ w|e| / Σ w`; `mean = Σ w e / Σ w`; `shape = Σ w|e − mean| / Σ w`; **metric = abs + 0.35·shape** (lower is better).

Per file: the D with the smallest metric (ascending candidates, strict `<`, so ties keep the smaller D) must equal the golden. Run level (`room_tuning_delay = auto`): the D with the smallest metric **summed over every two-speaker room file and both ear positions**; no such file → 10 ms. This scoring path uses its own common-start IRs, not the room stage's per-speaker `crop_head` IRs. Report the per-file metric curves and the run-level sum on the demo. Test `room_tuning_auto_delay_matches_secs` (per-file choices equal the golden).

## 4. Timing assertions

The FIR design origin is exactly `D_s` for every speaker (assert on the FIRs). On the tuned BRIR, peak picking moves by a few samples because the tuning changes the waveform near the peak (you measured 478/481 for 480): the demo test asserts every channel's peak shift within `D_s ± round(0.1 ms·fs)` and every channel without room data shifts by exactly `D_s`; the synthetic test keeps the exact assertion where it holds. Explain this in the test comment.

## 5. Off versus full

The 0.216 dB synthetic difference comes from the post windows on the minimum-phase inverse that only the full mode applies (as SECS does). Keep the behaviour; set the bound to 0.35 dB with that comment.

## 6. A robust low-frequency diagnostic for the BRIR

"BRIR excess group delay RMS" is dominated by near-null spikes (23–42 ms RMS on the demo) and is not a usable measure. Replace it in the diagnostics, readme and the `cli_room_tuning_weak` rule with:
- **LF decay**: per 1/3-octave band at 40, 50, 63, 80, 100, 125, 160 Hz (4th-order Butterworth band-pass, zero-phase), the early decay time EDT (Schroeder backward integral, 0 to −10 dB, × 6) of each ear, averaged over the two ears; per speaker the median over the bands, before and after tuning.
- **Median excess group delay**: median |excess group delay| over 30–300 Hz from the 1/6-octave complex-smoothed excess phase, before and after.
- `cli_room_tuning_weak` when the median EDT does not drop by at least 10 % **and** the median excess group delay does not drop by at least 20 %. Report all numbers on the demo and the synthetic fixture; if the rule fires for every demo speaker, keep it but report it prominently (it then means the demo's room files do not represent its BRIRs well, which the omni/in-ear magnitude RMS you measured, 1.6–3.5 dB, already hints at).

## 7. Small items

- Restore a dedicated two-point agreement-gate regression test (`room_tuning_gates_disagreeing_points`: points whose excess phase differs by > 120° in a band give G = 0 there and the FIR equals identity there within 0.1 ms excess group delay).
- `room_tuning_level_match_uses_median`: the formula is right (omni levels 0, −3, −10 dB → trims −3, 0, +6 clipped); keep it and fix the packet's example in the docs.
- Update `docs/rust/ROOM_CORRECTION.md` for 1–6 (no SNR floor; the geometry check; the auto-delay criterion with the per-file golden; the timing assertion; the new diagnostics) and add the SECS auto-delay golden to the reference-check subsection.

## Verification and report

As in A05c (fmt, clippy on the six crates with `-D warnings`, tests per crate/target in the foreground, `git status --porcelain`; the root-permission test is the only allowed failure). Report: (1) changes; (2) the reference comparison tables again (all bands, median and worst, per file/speaker); (3) auto delay per file (metric curves, chosen vs golden) and run level; (4) demo diagnostics per speaker (trim, pre-echo, LF EDT before/after, median excess GD before/after, weak or not, mismatch or not, peak shifts); (5) `test result:` lines; (6) anything undone. Do not end your turn before the commands complete.
