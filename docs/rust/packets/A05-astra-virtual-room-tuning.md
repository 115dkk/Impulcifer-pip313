# A05 (ASTRA): virtual room tuning — a second room correction mode with excess-phase correction

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스"), `docs/rust/ROOM_CORRECTION.md` and `docs/adr/0004-room-correction-v2.md` first. Rules: `#![forbid(unsafe_code)]`, no new dependencies, no shell subprocesses, run every command in the foreground, never in the background. ALSA development files are installed. Disk is limited: if a build runs out of space, delete `target/debug/incremental` and retry.

**Clean-room rule.** This mode is inspired by a community room-correction tool for real speakers whose source has no licence. Do not search for, open or read any third-party room-correction source, and do not open anything under `/tmp/claude-0/`. Implement only from this packet and the repository.

**Another worker edits `apps/**` at the same time. Never open it for writing.** Do not touch `crates/impulcifer-service/locales/**`, `Cargo.toml`/`Cargo.lock` versions, `CHANGELOG.md`, `README.md`, `.github/**`, the Python 2.x tree.

## What and why

Room correction now has two modes. `room_mode = "eq"` (default) is today's v2 magnitude correction, unchanged and bit-identical. `room_mode = "tuning"` ("virtual room tuning") keeps the same v2 magnitude correction and adds a correction of the BRIR's **excess phase** at low frequencies. A real room can only have its excess phase (modal ringing, speaker crossover group delay, early-reflection interference) corrected at the measurement microphone; the BRIR is that single recorded point and the listener never leaves it, so the correction holds exactly there. That is the whole idea of the mode.

The phase information is taken from **the in-ear BRIR itself**, not from the omni room measurement: the omni file was recorded at a different point (about 9 cm away, head centre) in another session, and in a modal field a few centimetres can cross a nodal plane and flip the sign of a mode, which would double its ringing. A microphone capsule is minimum-phase, so the in-ear excess phase needs no microphone calibration. Consequence: tuning works even without room measurement files (then it corrects phase only).

## Config, CLI, validation

Two more 3.x-only fields, appended to `EXTENSION_FIELD_NAMES` (now six) and to the CLI's `EXTENSION_OPTIONS`, accepted everywhere the other four are (service validation, `from_kwargs`, Python filter, `brir_defaults`):

| field | type | default | valid | CLI |
| --- | --- | --- | --- | --- |
| `room_mode` | `String` | `"eq"` | `eq`, `tuning` | `--room_mode {eq,tuning}`: `Room correction mode. "eq" corrects the magnitude only; "tuning" also corrects the low-frequency excess phase of each BRIR (virtual room tuning) and delays every channel by --room_tuning_delay.` |
| `room_tuning_delay` | `f64` | `10.0` | ms, `5 ≤ d ≤ 20` | `--room_tuning_delay MS`: `Common delay in milliseconds that virtual room tuning adds to every channel to hold its pre-ringing. Default 10.` |

`room_mode = "tuning"` with `room_range = "legacy"` is rejected (service: invalid-request error naming `room_mode`; CLI: exit 2) — tuning builds on the v2 ranges. `ProcessingConfig::oracle_defaults()` keeps `room_mode = "eq"`.

## Where it runs

Inside the existing `StageKey::Equalize` branch of `crates/impulcifer-dsp/src/pipeline.rs`, right after `equalize_hrir(..)` and before `observer.on_equalized`, when `config.do_room_correction && config.room_mode == "tuning"`. Do **not** add a stage row or a progress step: `golden_stage_table_matches_python` and the progress totals must not change. Put the algorithm in a new module `crates/impulcifer-dsp/src/stages/room_tuning.rs` with a public entry point, e.g.

```rust
pub struct TuningOptions { pub delay_ms: f64, pub f_ph: f64, pub vbass_crossover: Option<f64>, pub rolloff_knees: Vec<(String, Side, f64)> }
pub struct TuningReport { pub f_ph: f64, pub delay_samples: usize, pub speakers: Vec<SpeakerTuning> }
pub struct SpeakerTuning { pub speaker: String, pub corrected_share: f64, pub pre_echo_db: f64 }
pub fn tune_room_phase(hrir: &mut Hrir, fs: u32, options: &TuningOptions) -> Result<TuningReport, DspError>;
```

Measure the excess phase after `equalize_hrir`: the magnitude FIR is minimum-phase and does not change excess phase, so this equals measuring before it and needs no copy of the BRIR.

`f_ph = min(f_S, 300 Hz)`, where `f_S` comes from the v2 room diagnostics when the room stage ran (`RoomCorrection.diagnostics.schroeder.freq`), otherwise from `room_v2::estimate_schroeder` on the BRIR's own IRs with the config's `room_volume` / `schroeder_freq`. Never use the extreme range's 10 kHz upper bound. `rolloff_knees` come from the v2 diagnostics (specific ears by speaker/side, the generic `room.wav` knee for speakers without specific files); when there is no room data, run `room_v2::detect_rolloff` on each BRIR ear's own centred magnitude response with its own `room_v2::snr_db`.

## The algorithm (per speaker; both ears get the same filter)

Notation: N = next power of two ≥ max(65536, 2 × IR length) at 48 kHz (scale with fs so the bin spacing stays ≤ 0.75 Hz). `S_w` = fractional-octave boxcar smoothing **on the linear FFT bin grid**: for bin frequency f, average over bins in `[f·2^(−w/2), f·2^(w/2)]` (implement with prefix sums; complex or real input; DC and Nyquist bins pass through). Write this helper; do not reuse the log-grid `FrequencyResponse` smoothing, which assumes a log-spaced grid. `hh(f; a→b)` = half-Hann in log2 f from 1 at a to 0 at b.

1. **Per ear excess phase.** Take the ear's IR, find its peak `p` with `peaks::first_peak_index(data, 0, None, 0.12589)` (the finder decay/VB use), FFT at N, compute the minimum-phase spectrum `H_min` from `|H|` by the real-cepstrum method (fold the cepstrum: keep c[0], double c[1..N/2), keep c[N/2], zero the rest; exponentiate the FFT), and `E = (H / H_min) · exp(+j·2π·f·p/fs)` (removes the bulk delay so ITD does not enter E). Guard `|H_min|` with a floor of 1e-12.
2. **Combine the ears.** `w = S_{1/3}(|H|)` per ear (linear magnitude). `C = w_L·E_L + w_R·E_R`; `A0 = conj(C / |C|)` (identity where `|C|` < 1e-12).
3. **Gates**, each a real weight in [0, 1] on the bin grid:
   - agreement: `Δφ = arg(E_L · conj(E_R))`; weight 1 for `|Δφ| ≤ 60°`, 0 for `|Δφ| ≥ 120°`, linear in between (a mode with a node between the ears is left alone; a per-ear correction would change IPD/IACC);
   - local level: `L = 20·log10(S_{1/6}|H|) − 20·log10(S_1|H|)`, averaged over the two ears; weight 1 at `L ≥ −6 dB`, 0 at `L ≤ −12 dB`, linear in dB between;
   - SNR: `room_v2::snr_db` of each ear interpolated to the bins (use the min of the two ears); weight 0 at ≤ 20 dB, 1 at ≥ 30 dB; if unavailable for either ear, the weight is 0 everywhere (no correction without a noise estimate) and the report says so;
   - rolloff: 0 below the speaker's knee (the higher of the two ears' knees), 1 above;
   - virtual bass: when on, `room_v2::vbass_handoff_mask` evaluated at the bin frequencies;
   - band: 1 below `f_ph/√2`, `hh(f; f_ph/√2 → f_ph)`, 0 above `f_ph`; also 0 below 15 Hz.
   `W = S_{1/6}(product of the gates)`.
4. **Frequency-dependent resolution.** `A1` = complex smoothing of `A0` with `S_{1/12}` below 100 Hz, `S_{1/6}` 100–200 Hz, `S_{1/3}` above 200 Hz (blend the three smoothed spectra with complementary weights that switch over a quarter octave at 100 and 200 Hz), then normalise to unit magnitude.
5. **Blend toward identity.** `A2 = W·A1 + (1 − W)`, normalise to unit magnitude.
6. **Pre-ringing control.** `a = IFFT(A2)` (real, centred at sample 0; negative time wraps to the end). Let `D = round(delay_ms·fs/1000)`. Make three windowed copies whose pre-side (negative time) is a half-Hann of length `D`, `D/2`, `D/4` (falling from 1 at t = 0 to 0), and whose post-side is a half-Hann of 100 ms; FFT each; blend with complementary weights (the `D` copy below 100 Hz, `D/2` for 100–200 Hz, `D/4` above 200 Hz, quarter-octave transitions); normalise to unit magnitude; IFFT; apply the `D` pre-window and 100 ms post-window once more; normalise again in frequency; rotate so time zero lands at sample `D` and truncate to `D + 100 ms` samples. This is the speaker's all-pass FIR `g`.
7. **Apply.** Convolve `g` (full) into both ears of the speaker. A speaker with only one ear present, or whose `W` is zero everywhere, gets a pure delay of `D` samples instead, so **every channel moves by exactly D** and inter-channel timing is untouched.
8. **Report.** `corrected_share` = fraction of bins in `[20 Hz, f_ph]` with `W ≥ 0.5`; `pre_echo_db` = max over the two ears of `20·log10(max |h| before peak − 0.5 ms / |h(peak)|)` on the tuned IR.

Do not port anything else (no magnitude curtain, no broad tone correction, no peak crushing, no delay auto-selection, no left/right delay matching, no low-/zero-latency variants): v2 owns magnitude, and inter-channel delays are the spatial cues `crop_and_align` set.

## Service and logs

The service passes `room_mode`, `room_tuning_delay`, the v2 diagnostics and the VB crossover into the pipeline (follow how `vbass_crossover` already reaches the room stage). Log with `events.log` (another worker writes the catalogue; use exactly these keys and placeholders):

| level | key | args |
| --- | --- | --- |
| info | `cli_room_tuning` | `freq` (int Hz, f_ph), `delay` (int ms) |
| warning | `cli_room_tuning_no_snr` | `speaker` |
| warning | `cli_room_tuning_preecho` | `speaker`, `db` (string, 1 decimal) — when `pre_echo_db > −30` |

Nothing is logged by these keys in `eq` mode.

## Feature registry

`features.toml`: `config.room_mode`, `config.room_tuning_delay` (kind `config`) and `room.virtual_tuning` (the kind the other `room.*` entries use), all `implemented` with the tests below. Add the two config names to `CANONICAL_CONFIG` in `crates/impulcifer-policy/tests/gates.rs`.

## Tests (names fixed; DSP ones in `crates/impulcifer-dsp/tests/room_tuning.rs`)

Build a synthetic stereo BRIR pair for the DSP tests: a band-limited impulse per ear (different ITD, 0.3 ms), plus a 2nd-order all-pass at 80 Hz (Q 0.7, the crossover-like excess phase) and a decaying reflection at 6 ms with gain −6 dB (comb-like excess phase), plus seeded noise at −70 dB so the SNR gate opens; 1.5 s long, fs 48000.

- `room_tuning_filter_is_all_pass`: `|G(f)|` = 0 ± 0.2 dB from 20 Hz to fs/2 for every speaker filter.
- `room_tuning_keeps_magnitude`: 1/12-oct magnitude of every tuned ear vs the untuned ear (both after the same magnitude EQ), |Δ| ≤ 0.3 dB, 20 Hz–20 kHz.
- `room_tuning_keeps_interaural_cues`: both ears of a speaker got bit-identical filters; the cross-correlation lag of the two ears after a 700 Hz low-pass is unchanged (0 samples).
- `room_tuning_moves_every_peak_by_the_delay`: the `first_peak_index(.., 0.12589)` of every channel (including a single-ear speaker and a fully gated one) moves by exactly `D` samples; pairwise peak differences unchanged.
- `room_tuning_pre_echo_is_buried`: max |h| before peak − 0.5 ms ≤ −30 dB re peak; energy below f_ph in `[peak − D, peak − 0.5 ms]` ≤ −20 dB relative to `[peak, peak + 20 ms]`.
- `room_tuning_reduces_excess_group_delay`: on the synthetic pair, the RMS of the 1/6-oct smoothed excess group delay below f_ph (bins with W ≥ 0.9) drops by ≥ 50 % and rises nowhere by more than 1 ms; T20 of 1/3-oct band-filtered IRs at 40, 50, 63, 80, 100, 125, 160 Hz is at most 10 % above the untuned value in every band.
- `room_tuning_gates_disagreeing_ears`: if the two ears' excess phases differ by 180° in a band, `W` is 0 there.
- `room_tuning_skips_without_snr`: an IR shorter than one second gets a pure delay and the report flags it.
- Service (`crates/impulcifer-service/tests/room_v2.rs`): `room_tuning_runs_on_the_demo` (demo, `room_mode = "tuning"`: finite outputs, `cli_room_tuning` logged with `delay` 10, every channel's peak in `hesuvi.wav` is 10 ms later than in the `eq` run ± 0 samples, normalisation gain differs from the `eq` run by ≤ 3 dB — read it from the README or the logs the existing tests already read; and `room_tuning_rejects_legacy_range`; and that an `eq` run logs no `cli_room_tuning*` key.
- CLI: extend `cli_room_v2_options_parse_and_validate` with the two new options and the legacy rejection.

Every existing test, including all goldens and the A03/A03b tests, must pass unchanged: `eq` mode is bit-identical.

## Docs

Add a section "Virtual room tuning" to `docs/rust/ROOM_CORRECTION.md`: what it does, why the phase comes from the BRIR and not the room file, every number above, the delay and its effect on latency, what was deliberately not brought over, and this attribution line: "The idea of correcting a speaker's low-frequency excess phase with band-dependent resolution and band-dependent pre-ringing windows follows SECS by 한플 (DCinside speaker minor gallery, 2026); Impulcifer's implementation is independent and works on the BRIR rather than on a speaker feed."

## Verification (foreground; paste the tail of each)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy -p impulcifer-python --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy --no-fail-fast
git status --porcelain
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails here because the container runs as root; it must be the only failure. If one command exceeds your tool's time limit, split it per crate or per test target, still in the foreground.

## Report format

(1) public API as landed; (2) any place you had to interpret this packet, with the choice made; (3) new test names, one line each, with the measured numbers for the effect test (excess group delay RMS before/after, T20 per band before/after) and the demo run (f_ph, corrected share and pre-echo per speaker, normalisation gain eq vs tuning); (4) pasted `test result:` lines and failures; (5) `git status --porcelain`; (6) anything undone. Do not end your turn before the commands complete.
