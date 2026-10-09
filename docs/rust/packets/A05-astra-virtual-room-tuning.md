# A05 (ASTRA): virtual room tuning — revision 2

> **Revision 2 replaces revision 1 entirely.** Revision 1 took the excess phase from the in-ear BRIR. The owner's goal is different and is what this revision implements: emulate a room-correction processor (Dirac-like) that ran in the speaker feed while the BRIRs were recorded. The working tree holds a partial implementation of revision 1 (stopped midway): config fields, CLI options, validation, a pipeline hook, `crates/impulcifer-dsp/src/stages/room_tuning.rs`, `crates/impulcifer-dsp/tests/room_tuning.rs`, edits to `docs/rust/ROOM_CORRECTION.md`. Keep what still applies (the two config fields and their CLI/validation, the all-pass construction, bin-grid smoothing, pre-ringing windows, the common delay), change the rest to this revision, and delete tests and docs that encode revision 1 behaviour.

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스"), `docs/rust/ROOM_CORRECTION.md` and `docs/adr/0004-room-correction-v2.md`. Rules: `#![forbid(unsafe_code)]`, no new dependencies, no shell subprocesses, every command in the foreground. ALSA development files are installed. If a build runs out of disk, delete `target/debug/incremental` and retry.

**Clean-room rule.** This mode is inspired by a community room-correction tool whose source has no licence. Do not search for, open or read any third-party room-correction source, and do not open anything under `/tmp/claude-0/`. Implement only from this packet and the repository.

Do not touch `apps/**`, `crates/impulcifer-service/locales/**` (another worker writes the catalogue at the same time), `Cargo.toml`/`Cargo.lock` versions, `CHANGELOG.md`, `README.md`, `.github/**`, the Python 2.x tree.

## The model

The in-ear BRIR is the response from a speaker's feed to the ear: speaker plus room error, seen through the head and ear. The omni measurement-microphone files exist to measure the **room error**, the way a room-correction processor measures it. A processor filter F inserted in a speaker's feed during the recording would have produced exactly `F * BRIR` at both ears (LTI filters commute). So virtual room tuning computes, per **speaker**, the filter such a processor would have computed from the omni data, and convolves the **same** filter into **both ears** of that speaker. The result is "the BRIR recorded with the processor running": room error corrected, head/ear cues untouched (no per-ear filter, so ITD/ILD/IPD do not change). The headphone-to-ear response is separate and stays with headphone compensation.

The two ear-position files `room-<SPK>-left.wav` / `room-<SPK>-right.wav` are a two-point measurement at the right points: below 300 Hz the head-absent pressure at the ear position equals what the in-ear mic saw within about 1 dB / 10°. `room.wav` is one point at head centre, usually of one speaker, shared by all speakers: it may drive magnitude only, never phase.

`room_mode = "eq"` (default) stays exactly as today (per-ear v2 gain, bit-identical). `room_mode = "tuning"` uses this model. `eq` keeps the virtual-only advantage of per-ear node filling; `tuning` deliberately gives it up because a speaker-feed filter cannot fill a node at one ear without raising the other.

## Config, CLI, validation (keep from revision 1)

`room_mode` (`"eq"` | `"tuning"`, default `"eq"`) and `room_tuning_delay` (ms, default 10, valid 5–20), in `EXTENSION_FIELD_NAMES`, `EXTENSION_OPTIONS`, service validation, `from_kwargs`, the Python filter and `brir_defaults`. `tuning` with `room_range = "legacy"` is rejected (service: invalid request naming `room_mode`; CLI: exit 2). CLI help for `--room_mode`: `Room correction mode. "eq" corrects the frequency response at each ear position; "tuning" applies, per speaker, the correction a room-correction processor in the speaker feed would have applied during the recording (frequency response, and low-frequency timing when both ear positions were measured) and delays every channel by --room_tuning_delay.` Help for `--room_tuning_delay` as in revision 1.

## Where it runs

- **Room stage** (`room_v2::room_correction`, tuning branch): computes per speaker the magnitude gain G (below) and, when both ear-position files exist, the excess-phase all-pass spectrum. Return them in `RoomCorrection` (add a field, e.g. `tuning: Option<TuningPlan>` holding per-speaker all-pass FIRs or spectra, the delay, `f_ph`, diagnostics). The `RoomFrs` entries for both ears of a speaker carry the **same** G (term `Gain`), so the existing equalize path applies it unchanged.
- **Equalize branch** of `crates/impulcifer-dsp/src/pipeline.rs`: after `equalize_hrir(..)`, apply the all-pass FIRs (same FIR to both ears) and the common delay. No new stage row or progress step (`golden_stage_table_matches_python` and progress totals unchanged).

## Magnitude per speaker

Run the v2 gain rule (range, caps, rolloff, SNR cap, upper fade, LR8 virtual-bass mask; no diotic blend needed) on each ear-position file as today, giving `G_L`, `G_R`. Also run it on the **power-averaged** error `e_p = 10·log10((10^(e_L/10) + 10^(e_R/10)) / 2)` giving `G_p`. Per bin: if `G_p < 0` (a cut), `G = G_p`; otherwise `G = min(max(G_L, 0), max(G_R, 0))` (a boost only as far as both points agree). SNR cap uses the min of the two files' SNR; the rolloff is the higher of the two knees. Speakers with only `room.wav` data get the generic v2 gain (magnitude only). Speakers with no room data get no correction.

## Excess phase per speaker (only when both ear-position files exist)

Same algorithm as revision 1, with the **omni ear-position IRs** (as split by the room stage, after `crop_head`, before tail cropping) as the two inputs instead of the two BRIR ears:

1. Per file: peak `p` via `peaks::first_peak_index(data, 0, None, 0.12589)`; FFT at N (power of two ≥ max(65536, 2·len) at 48 kHz, bin spacing ≤ 0.75 Hz at any fs); minimum-phase spectrum from the real cepstrum; `E = (H / H_min) · exp(+j·2π·f·p/fs)`. Subtract the omni mic calibration from |H| first if present (it does not change E; it matters for the level gate).
2. `C = w_L·E_L + w_R·E_R` with `w = S_{1/3}(|H|)` per file; `A0 = conj(C/|C|)`.
3. **Remove the residual linear phase** of `A0` below `f_ph`: least-squares fit of the unwrapped phase vs frequency over bins with weight `W ≥ 0.5` (weights from step 4), subtract the fitted line, so the all-pass carries zero mean in-band delay and speakers do not get different low-vs-high offsets.
4. Gates (multiply, then `S_{1/6}`): two-point agreement (1 at |Δφ| ≤ 60°, 0 at ≥ 120°), local level (1 at ≥ −6 dB, 0 at ≤ −12 dB relative to the 1-oct local level, mean of the two files), SNR from the omni files' `room_v2::snr_db` (min of the two; 0 at ≤ 20 dB, 1 at ≥ 30 dB; unavailable → weight 0 and report it), rolloff (0 below the higher knee), virtual bass (`vbass_handoff_mask`), band (1 below `f_ph/√2`, half-Hann to 0 at `f_ph`, 0 below 15 Hz). `f_ph = min(f_S, 300 Hz)` with `f_S` from the room stage's Schroeder estimate.
5. Band-dependent complex smoothing (1/12 oct < 100 Hz, 1/6 oct 100–200 Hz, 1/3 oct > 200 Hz, quarter-octave blends), unit-magnitude normalisation, blend toward identity by `W`, normalise.
6. Pre-ringing windows (pre-side half-Hann D, D/2, D/4 by band; post-side 100 ms), renormalise, rotate time zero to sample D, truncate to D + 100 ms: the speaker's FIR `g`.

## Delay and application

Apply `g` to both ears of each speaker that has one; every other channel (generic-only, no room data, fully gated) gets a pure delay of D samples, so every channel moves by exactly D. If **no** speaker received any phase correction (every `W` < 0.1 everywhere), add no delay at all and log it.

## Session check (warning only)

The omni files must belong to this BRIR session's geometry. For each speaker with both ear-position files and a lateral position (BRIR interaural arrival difference > 0.2 ms by the −18 dB peak finder): warn if the sign of the omni files' arrival difference (right − left) differs from the BRIR's (swapped files), or if the omni arrival time differs from the BRIR's by more than 1 ms (different listening spot or chain).

## Logs (another worker writes the catalogue; use exactly these)

| level | key | args |
| --- | --- | --- |
| info | `cli_room_tuning` | `freq` (int Hz, f_ph), `delay` (int ms) |
| info | `cli_room_tuning_magnitude_only` | `speaker` |
| info | `cli_room_tuning_no_phase` | — |
| warning | `cli_room_tuning_no_snr` | `speaker` |
| warning | `cli_room_tuning_preecho` | `speaker`, `db` (string, 1 decimal), when > −30 dB on the BRIR |
| warning | `cli_room_tuning_mismatch` | `speaker` |
| warning | `cli_room_tuning_weak` | `speaker` (the BRIR's excess group delay below f_ph dropped by < 30 % RMS: the emulation is correct but the omni files may not represent the ears) |

`cli_room_tuning_magnitude_only` is logged once per speaker that has room data but not both ear-position files. Nothing is logged by these keys in `eq` mode.

## Feature registry

`config.room_mode`, `config.room_tuning_delay`, `room.virtual_tuning`, all `implemented` with the tests below; both config names in `CANONICAL_CONFIG`.

## Tests (DSP in `crates/impulcifer-dsp/tests/room_tuning.rs`, names fixed)

Synthetic setup: two omni ear-position IRs per speaker that share a 2nd-order all-pass at 80 Hz (Q 0.7) and a −6 dB reflection at 6 ms (the room error), plus seeded noise at −70 dB, 1.5 s, fs 48000; a BRIR pair built from the same room error convolved with two different "head" filters (different ITD 0.3 ms and a mild ILD shelf).

- `room_tuning_filter_is_all_pass`: |g| = 0 ± 0.2 dB, 20 Hz–fs/2.
- `room_tuning_uses_one_filter_per_speaker`: both ears got bit-identical FIRs; the interaural difference spectrum (left/right ratio) of the tuned BRIR equals the untuned one to 1e-9 relative.
- `room_tuning_magnitude_combines_two_points`: a −20 dB node at one ear position and 0 dB at the other → no boost there; a peak common to both → cut fully; a peak at one point only → cut about 3 dB (power average).
- `room_tuning_reduces_room_excess_group_delay`: applying `g` to the omni IRs reduces their 1/6-oct excess group delay RMS below f_ph by ≥ 50 % and nowhere raises it by > 1 ms; T20 of 1/3-oct bands 40–160 Hz not up by > 10 %.
- `room_tuning_moves_every_peak_by_the_delay`: every channel (with filter, pure delay, generic-only) moves by exactly D; and `room_tuning_adds_no_delay_without_phase` (generic-only input → no delay, `cli_room_tuning_no_phase` condition reported).
- `room_tuning_pre_echo_is_buried`: on the tuned BRIR, max |h| before peak − 0.5 ms ≤ −30 dB re peak; LF energy in `[peak − D, peak − 0.5 ms]` ≤ −20 dB vs `[peak, peak + 20 ms]`.
- `room_tuning_keeps_magnitude`: tuned BRIR 1/12-oct magnitude vs the same tuning run without the all-pass (magnitude only), |Δ| ≤ 0.3 dB, 20 Hz–20 kHz.
- `room_tuning_gates_disagreeing_points` and `room_tuning_skips_without_snr` as in revision 1, on the omni inputs.
- `room_tuning_detects_swapped_files`: swapping the left/right omni files of a lateral speaker triggers the mismatch condition.
- Service (`crates/impulcifer-service/tests/room_v2.rs`): `room_tuning_runs_on_the_demo` (demo has specific ear-position room files: finite outputs, `cli_room_tuning` with `delay` 10, every channel in `hesuvi.wav` exactly 10 ms later than the `eq` run, normalisation gain within 3 dB of the `eq` run, no `cli_room_tuning_mismatch`), `room_tuning_rejects_legacy_range`, and an `eq` run logs no `cli_room_tuning*` key.
- CLI: `cli_room_v2_options_parse_and_validate` covers both options and the legacy rejection.

All existing tests, goldens and A03/A03b tests pass unchanged.

## Docs

`docs/rust/ROOM_CORRECTION.md` section "Virtual room tuning": the model (processor in the speaker feed, `F * BRIR`, one filter per speaker), why the ear-position files may drive phase and `room.wav` may not, the magnitude combination rule, every number, the delay, the session check, what was deliberately not brought over (no curtain inverse, no broad ±6 dB tone layer, no peak crushing, no delay auto-selection, no left/right or inter-channel delay matching, no low-/zero-latency variants), and the attribution line: "The idea of correcting a speaker's low-frequency excess phase with band-dependent resolution and band-dependent pre-ringing windows follows SECS by 한플 (DCinside speaker minor gallery, 2026); Impulcifer's implementation is independent and applies the processor's filter to the BRIR instead of a speaker feed."

## Verification (foreground; paste tails)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy -p impulcifer-python --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy --no-fail-fast
git status --porcelain
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails here because the container runs as root; it must be the only failure. Split long commands per crate or test target if needed, still in the foreground.

## Report format

(1) public API as landed; (2) interpretations with the choice made; (3) new tests with their measured numbers (omni excess group delay RMS before/after, T20 per band before/after; demo: f_ph, per speaker whether phase was corrected and its weight share, pre-echo, BRIR excess group-delay change, normalisation gain eq vs tuning); (4) `test result:` lines and failures; (5) `git status --porcelain`; (6) anything undone. Do not end your turn before the commands complete.
