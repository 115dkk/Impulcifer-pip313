# A03 (ASTRA): room correction v2 — ranges, Schroeder, rolloff, virtual-bass hand-off

> **Revision 2.** A first run stopped before editing on two contradictions; both are fixed here. (1) The rolloff slope is measured in dB versus `log2 f`, where a high-pass rolloff is a *positive* slope: the condition is now `s ≥ +9`, and the rolloff point is redefined (section 4). (2) The Schroeder-range test overrides `f_S` to 400 Hz so that 210 Hz lies in the full-strength region; the modes test uses the same override. Nothing else changed.

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스") and `/home/user/Impulcifer-pip313/docs/rust/ARCHITECTURE.md` first. Rules: `#![forbid(unsafe_code)]`, no new dependencies, no shell subprocesses, run every command in the foreground and never in the background, do not query or terminate processes. When you are unsure how an existing helper behaves (smoothing, `decay_times`, `filters::butter`, the catalog), read its source before using it.

**Other workers edit `apps/**` and `crates/impulcifer-service/locales/**` at the same time. Never open those for writing.** Also do not touch `Cargo.toml`'s version, `Cargo.lock`'s workspace versions, `CHANGELOG.md`, `README.md`, `.github/**`, `i18n/**` or the Python 2.x tree (`core/`, `autoeq/`, `impulcifer.py`, `gui/`, `application/`, `infra/`, `updater/`, `tests/`).

## Why

Today room correction (`crates/impulcifer-dsp/src/stages/room.rs`, a bit-exact port of 2.x `core/room_correction.py`) turns an omni measurement-mic recording at the ear position into an error curve `measured − target`, zeroes it above `specific_limit`/`generic_limit` with a mask that has a defect (a full Hann over `[limit/2, limit]`: the mask drops to 0 just above `limit/2`, rises to 1 at `limit/√2` and falls to 0 at `limit`), sums it with the headphone-compensation and custom-EQ errors, smooths the sum (`smoothen_heavy_light`: max of 1/6- and 1/3-oct, then 1/3-oct), inverts it with a +40 dB cap and convolves a minimum-phase FIR into each ear's BRIR. It boosts the speaker's low-frequency rolloff (a 45 Hz 4th-order high-pass gets +28 dB at 20 Hz), and when virtual bass is on it imposes the inverse room modes onto the synthetic flat bass, because the virtual-bass stage runs before the equalize stage.

The owner's rules for v2:
1. Derive from the omni room response the EQ that a real room cannot take but a recorded BRIR can (the listener never leaves the measured point, so nulls can be filled), and apply it to the person's BRIR, aiming at flat.
2. Three ranges: room modes / Schroeder / extreme.
3. Detect the low-frequency rolloff automatically and do not boost it. (Physics: the headphone can play it; the measurement has no usable signal there.)
4. With virtual bass on, the room EQ does not act below the virtual-bass crossover.

The decisions below are fixed (owner + advisor review). Implement exactly; where a number is given, use it.

## Compatibility

- `room_range = "legacy"` is today's behaviour, bit-exact. Every test that compares 3.x output against a 2.x oracle golden with room correction active must pin `legacy` (see "Tests"). No WAV, README, golden series or FIR of a legacy run may change.
- The new default is `room_range = "schroeder"`. 2.x is not changed.
- In the three v2 ranges `specific_limit` and `generic_limit` are ignored (document this in their CLI help by appending ` Used only with --room_range legacy.`).

## Config (crates/impulcifer-types/src/config.rs)

Add four 3.x-only fields to `ProcessingConfig` (serde default):

| field | type | default | valid |
| --- | --- | --- | --- |
| `room_range` | `String` | `"schroeder"` | `legacy`, `modes`, `schroeder`, `extreme` |
| `room_volume` | `Option<f64>` | `None` | m³, `0 < v ≤ 10000` |
| `schroeder_freq` | `Option<f64>` | `None` | Hz override, `50 ≤ f ≤ 1000` |
| `room_max_boost` | `f64` | `12.0` | dB, `0 ≤ b ≤ 24` |

`FIELD_NAMES` stays the 33 2.x names. Add `pub const EXTENSION_FIELD_NAMES: [&str; 4]` (the four above, in this order) and make every consumer that filters by `FIELD_NAMES` accept both lists: `ProcessingConfig::from_kwargs`, `impulcifer-service/src/brir/validation.rs` (unknown-field check), `impulcifer-python/src/module.rs` (kwargs filter). Add `ProcessingConfig::oracle_defaults()` = `default()` with `room_range = "legacy"`, documented as "the 2.x dataclass defaults; use in tests that compare against 2.x oracle output".

Validation: the service rejects out-of-range values with the same error shape it uses for other invalid BRIR fields (look at how it reports e.g. a bad `fr_combination_method` or `vbass_polarity` and follow it, including the field name in details). The CLI rejects them like its other argparse-style errors (exit 2).

## CLI (crates/impulcifer-cli/src/options.rs)

Four new options in a separate `pub static EXTENSION_OPTIONS` (not in `OPTIONS`, so `golden_cli_options_match_python`, which zips `OPTIONS` with the 2.x argparse dump, keeps passing): `--room_range {legacy,modes,schroeder,extreme}`, `--room_volume FLOAT`, `--schroeder_freq FLOAT`, `--room_max_boost FLOAT`, all with suppressed defaults so an argv without them yields exactly today's kwargs (the twelve `p13_parse_*` goldens must still pass). Parsing and `--help` cover both slices. Help texts (English, one or two sentences each):
- `--room_range`: `Room correction range. "modes" corrects room-mode resonances and keeps the bass balance, "schroeder" (default) flattens everything up to the Schroeder frequency, "extreme" also applies a smoothed tone correction up to 10 kHz, "legacy" is the 2.x behaviour.`
- `--room_volume`: `Room volume in cubic metres, used to compute the Schroeder frequency. Without it 50 m³ is assumed.`
- `--schroeder_freq`: `Schroeder frequency in Hz. Overrides the estimate from the room measurements.`
- `--room_max_boost`: `Largest boost in dB that room correction may apply with speaker-ear specific room measurements. Default 12.`

## DSP (crates/impulcifer-dsp)

Keep `stages/room.rs` (legacy) bit-exact. Put v2 in a new `stages/room_v2.rs` and dispatch from `room::room_correction`. Public surface (names fixed, add fields if you need them):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRange { Legacy, Modes, Schroeder, Extreme }
impl RoomRange { pub fn parse(s: &str) -> Option<Self>; pub fn as_str(self) -> &'static str; }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomTerm { LegacyError, Gain }

#[derive(Clone, Debug)]
pub struct RoomFrs { pub term: RoomTerm, pub entries: Vec<(String, Side, FrequencyResponse)> }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchroederSource { Override, Volume, AssumedVolume, Fallback }
#[derive(Clone, Debug)]
pub struct SchroederEstimate { pub freq: f64, pub t60: Option<f64>, pub volume: Option<f64>, pub source: SchroederSource }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RolloffSource { Slope, Snr }
#[derive(Clone, Copy, Debug)]
pub struct Rolloff { pub freq: f64, pub knee: f64, pub source: RolloffSource }
#[derive(Clone, Debug)]
pub struct EarDiagnostics {
    pub speaker: String,              // "room.wav" for the generic measurement
    pub side: Option<Side>,           // None for the generic measurement
    pub rolloff: Option<Rolloff>,
    pub snr_available: bool,
    pub residual_rms_db: Option<f64>,
}
#[derive(Clone, Debug)]
pub struct RoomDiagnostics { pub range: RoomRange, pub schroeder: SchroederEstimate, pub f_hi: f64, pub vbass_crossover: Option<f64>, pub ears: Vec<EarDiagnostics> }

// RoomCorrectionOptions gains: range, room_volume, schroeder_freq, max_boost_db, vbass_crossover: Option<f64>
// RoomCorrection gains: diagnostics: Option<RoomDiagnostics> (None for legacy)

pub fn estimate_schroeder(irs: &[&ImpulseResponse], volume: Option<f64>, override_hz: Option<f64>) -> SchroederEstimate;
pub fn snr_db(ir: &ImpulseResponse, frequency: &[f64]) -> Option<Vec<f64>>;
pub fn detect_rolloff(frequency: &[f64], measured_db: &[f64], snr_db: Option<&[f64]>) -> Option<Rolloff>;
pub fn vbass_handoff_mask(frequency: &[f64], crossover: f64, fs: u32) -> Vec<f64>;
```

`RoomFrs` changes from a tuple struct to the struct above; update every user (`equalize.rs`, `pipeline.rs`, service, plots). Legacy produces `term = LegacyError` with exactly today's entries.

Notation: the frequency grid is the existing 1.01-ratio grid from 10 Hz to fs/2. `S_w(x)` is constant-width fractional-octave smoothing of width `w` octaves on that grid (reuse the existing fractional-octave smoothing; expose a `pub(crate)` helper over a plain slice if needed). A *half-Hann fade from f_a (1) to f_b (0)* is `h(f) = 0.5·(1 + cos(π·t))`, `t = clamp(log2(f/f_a) / log2(f_b/f_a), 0, 1)`. Wherever this spec gives a cap that steps at `f0`, make it continuous by interpolating linearly in `log2 f` over `[f0·2^(−1/4), f0·2^(1/4)]`.

### 1. Error per ear

Exactly as legacy up to the error: mic calibration subtracted, specific measurements centred with the first file's 100 Hz–10 kHz gain reused for all, generic tracks centred each and combined by `fr_combination_method` (reuse the legacy functions with `limit = 0` and take the unmasked combined `error`). `e(f) = measured − target`. Positive `e` means too loud; the room gain `G` below is in dB with + = boost.

### 2. SNR

`snr_db(ir, frequency)`: the IR must be the measurement before tail cropping (specific: after `crop_head`, before `crop_tails`; generic: as split). Let `L = min(len/4, fs)` samples. Signal segment: the first `L` samples. Noise segment: the last `L` samples. Magnitude spectra of both with the same length and window, mapped to `frequency` the way the code maps an IR to a response, each smoothed `S_{1/6}`; `SNR = signal − noise` in dB. Return `None` when the IR is shorter than one second (`len < fs`). When `None`, the SNR cap is skipped for that ear and a warning is logged (see Logs).

### 3. Schroeder frequency

`estimate_schroeder`:
- `T60`: for each IR and each octave band centred at 125, 250 and 500 Hz, filter the IR with `butter(4, f/√2, highpass)` then `butter(4, f·√2, lowpass)` (`filters::butter` + `sosfilt`), take `decay_times(..).rt30`, falling back to `rt20`. Keep values in `[0.05, 3.0]` s. `T60` = median of the kept values; `None` if none.
- Override given → `freq = clamp(override, 50, 1000)`, source `Override`.
- Volume given and `T60` known → `freq = clamp(2000·√(T60/V), 80, 500)`, source `Volume`.
- No volume, `T60` known → `freq = clamp(2000·√(T60/50), 120, 300)`, source `AssumedVolume`.
- `T60` unknown (and no override) → `freq = 200`, source `Fallback`.
Use all specific IRs, or the generic IRs when there are no specific ones.

### 4. Rolloff detector

`detect_rolloff(frequency, measured_db, snr)` with `measured_db` = mic-calibrated, centred response:
1. `M = S_{1/6}(measured_db)`.
2. For `f_c` from 160 Hz downward in 1/12-octave steps (down to 20 Hz): `s(f_c)` = least-squares slope of `M` versus `log2 f` over the grid points in `[f_c/2, f_c]` (dB/oct). A high-pass rolloff makes this slope **positive** (the response falls toward low frequencies).
3. The knee `f_knee` = the first (highest) `f_c` with `s(f_c) ≥ +9` AND `M(f') ≤ M(f_c) − 6` for every grid `f' ≤ f_c/2` where `SNR(f') ≥ 10` (all of them when SNR is `None`). Then the plateau `P` = median of `M` over grid points in `[f_knee, 2·f_knee]`, and `f_roll` = the highest grid frequency `≤ f_knee` with `M ≤ P − 6` (the in-room −6 dB point; `f_knee` itself if none). Enforce `f_knee ≥ f_roll·2^(1/4)`. Source `Slope`.
4. Otherwise, if SNR is available: smooth it `S_{1/3}`; if the region starting at the lowest grid frequency where it stays below 10 dB is non-empty, `f_roll` = its upper edge (capped at 160 Hz) and `f_knee = f_roll·√2`. Source `Snr`.
5. Otherwise `None`.
Specific: per ear. Generic: run it per split IR and take the one with the highest `f_roll`.
Reference values from the advisor's simulation of this exact rule (1/6-oct smoothing, no SNR): 2nd-order 50 Hz high-pass → knee 50 Hz, `f_roll` 35 Hz; 4th-order 45 Hz → knee 67 Hz, `f_roll` 39 Hz; adding +8 dB of boundary gain below 100 Hz lowers `f_roll` to about 30 and 35 Hz; a flat response with a −12 dB, 1/6-oct dip at 35 Hz → `None`.

### 5. Ranges and the gain rule

`f_S` from section 3. `f_hi`: modes `f_S/√2`, schroeder `f_S`, extreme `10000`.

Tier input `x` and the two smoothed views `c` (for cuts) and `b` (for boosts):
- **modes**: `x = S_{1/24}(e) − S_1(e)` (resonances only; the bass balance is left alone). `c = x`, `b = S_{1/6}(x)`.
- **schroeder**: `x = e`. `c = S_{1/12}(x)`, `b = S_{1/6}(x)`.
- **extreme**: for specific measurements, above 500 Hz blend each ear's `e` with the mean of the same speaker's left and right `e`: `x = (1−α)·e_ear + α·mean(e_L, e_R)` with `α` rising 0→1 as `1 − h` of a half-Hann fade from 500 Hz to 700 Hz (diotic above 700 Hz: the omni mic has no head shadow, so per-ear differences there are not ILD). Generic: `x = e`. Below `f_S`: `c = S_{1/12}(x)`, `b = S_{1/6}(x)`. Above `f_S`: `c = b = S_{w(f)}(x)` where `log2 w` is piecewise linear in `log2 f` through `(f_S, 1/12)`, `(1000, 1/3)`, `(4000, 1)` and constant above 4 kHz; compute `S_w` for `w ∈ {1/12, 1/6, 1/3, 1/2, 1}` and interpolate per bin linearly in `log2 w` between the two neighbours.

Gain before caps: `G = −c` where `c > 0`; else `G = max(−b, 0)`.

Caps (apply to the magnitude of cuts and boosts respectively, `max_boost = room_max_boost`):
- Cut cap: 24 dB below `f_S`; extreme above `f_S`: 12 dB up to 1 kHz, 6 dB above 1 kHz.
- Boost cap, specific measurements: `max_boost` up to `f_S`; `min(6, max_boost)` from `f_S` to 1 kHz; `min(3, max_boost)` above 1 kHz. Modes range: `min(6, max_boost)` everywhere.
- Boost cap, generic measurement: `average` → 0 (cut only); `conservative` → `min(3, max_boost)`; and 0 above `f_S/√2` in every range.
- Rolloff: boost cap × `r(f)`, `r = 0` below `f_roll`, `r = 1 − h` of a half-Hann fade from `f_roll` to `f_knee` (0 → 1), 1 above `f_knee`. Cuts are untouched below the rolloff.
- SNR: boost ≤ `max(0, SNR(f) − 20)`.

Then multiply `G` by the upper fade and, with virtual bass on, by the hand-off mask:
- Upper fade: half-Hann from `f_hi/√2` (1) to `f_hi` (0); extreme: from 5 kHz (1) to 10 kHz (0).
- `vbass_handoff_mask(frequency, f_c, fs)`: `|H(f)|` of the virtual-bass high-pass section exactly as `virtual_bass.rs` builds it (`butter(4, f_c/nyq, highpass)` applied twice, i.e. LR8 high-pass), evaluated at each grid frequency, and 0 below `f_c/√2`. Reference values: `f_c·2^{−1/4, 0, 1/4, 1/2, 1}` → 0.20, 0.50, 0.80, 0.94, 0.996 (±0.02). Expose the SOS construction from `virtual_bass.rs` as a `pub(crate)` helper rather than duplicating it; `apply_virtual_bass` output must not change.

Self-check: residual `r = x + G` (modes: on `x`; others on `e`) smoothed `S_{1/6}`, RMS over grid points in `[lo, f_hi/√2]` where `lo = max(20, f_knee if any, f_c·√2 if virtual bass)`; skip when the interval is empty. Store in `residual_rms_db`; above 3 dB, log a warning.

Each v2 entry in `RoomFrs` is a `FrequencyResponse` on the grid with `raw` = measured (centred), `target` = target, `equalization` = `G`, `error = −G` (so the existing room plot, which shows the smoothed error, shows the correction actually applied). `term = Gain`.

### 6. Equalize stage (stages/equalize.rs)

`EqInputs.room_frs` keeps its role. For `LegacyError`, nothing changes. For `Gain`, the room entry is not added to `fr.error` before smoothing; after `fr.smoothen_heavy_light()` and before `fr.equalize(..)`, do `error_smoothed[i] -= G[i]` and `error[i] -= G[i]`. (Adding it before would let the heavy/light smoothing make filled dips shallow again.) The global `max_gain: 40.0` stays.

### 7. Service (crates/impulcifer-service)

`brir/inputs.rs` builds `RoomCorrectionOptions` from the config (`vbass_crossover = config.vbass.then(config.vbass_freq as f64)`), passes the uncropped IRs needed for SNR, and logs with `events.log(level, key, args)` (the catalog translates keys; the locale files are written by another worker, so only use these keys and placeholders):

| level | key | args |
| --- | --- | --- |
| info | `cli_room_range` | `range` (string), `f_hi` (integer Hz) |
| info | `cli_room_schroeder` | `freq` (int Hz), `t60` (string, 2 decimals), `volume` (string, 1 decimal) |
| info | `cli_room_schroeder_assumed` | `freq`, `t60` |
| info | `cli_room_schroeder_override` | `freq` |
| warning | `cli_room_schroeder_fallback` | `freq` |
| info | `cli_room_rolloff` | `speaker`, `side` (`left`/`right`/`both`), `freq` (int Hz) |
| info | `cli_room_rolloff_snr` | `speaker`, `side`, `freq` |
| warning | `cli_room_snr_unavailable` | `speaker`, `side` |
| warning | `cli_room_selfcheck_warning` | `speaker`, `side`, `rms` (string, 1 decimal), `lo`, `hi` (int Hz) |
| info | `cli_room_vbass_handoff` | `freq` (int Hz) |

Nothing is logged by these keys for `legacy`. `brir_defaults` (bootstrap) must carry the four new fields with their defaults. The room plot must keep working for both terms (`plots.rs` reads `room.frs` — adapt to the new struct only; do not change `impulcifer-plots`).

## Feature registry and gates

- `features.toml`: `config.room_range`, `config.room_volume`, `config.schroeder_freq`, `config.room_max_boost` (kind `config`), and `room.v2_ranges`, `room.schroeder_estimate`, `room.rolloff_detection`, `room.vbass_handoff`, `room.self_check` (use the existing kind that fits DSP behaviour; read the file). All `status = "implemented"` with real test names. Update the notes of `config.specific_limit`, `config.generic_limit` and `stage.room_correction` to say legacy-only / v2 dispatch.
- `crates/impulcifer-policy/tests/gates.rs`: add the four names to `CANONICAL_CONFIG`.

## Tests

Pin legacy: every test that compares output with a 2.x oracle golden and runs room correction (search `ProcessingConfig::default()` and `ProcessingConfig {` in `crates/*/tests` and `crates/*/src` test modules, and the service/CLI demo parity runs) switches to `ProcessingConfig::oracle_defaults()` or sets `room_range: "legacy".into()`. Tests asserting that 3.x defaults equal the 2.x dataclass compare only `FIELD_NAMES`. Do not loosen any tolerance.

New tests (names fixed; put DSP ones in `crates/impulcifer-dsp/tests/room_v2.rs`):
- `room_v2_rolloff_detects_sealed_and_ported_speakers`: synthetic `measured_db` from an analog 2nd-order 50 Hz and 4th-order 45 Hz Butterworth high-pass on the grid → `f_roll` within ¼ octave of the high-pass's own −6 dB frequency (compute it in the test) and `f_knee` in `[corner, corner·2^(3/4)]`; the same two plus +8 dB of boundary gain below 100 Hz (`8/(1+(f/100)²)`) → detected, with `f_roll` lower than without the boundary gain; a flat response with only a −12 dB, 1/6-oct-wide Gaussian dip at 35 Hz, with and without the boundary gain → `None`.
- `room_v2_schroeder_estimate_sources_and_clamps`: synthetic exponentially decaying noise IRs (seeded deterministic PRNG, T60 0.4 s, fs 48000, 2 s) → `T60` within 15 %; the four sources and their clamps; override wins.
- `room_v2_schroeder_range_corrects_modes_and_fills_specific_dips`: with `f_S` overridden to 400 Hz (full strength up to 283 Hz), an error with +8 dB at 60 Hz, −10 dB at 120 Hz and +6 dB at 210 Hz (Lorentzian peaks, Q 4–6) and no rolloff → `G(60) ≈ −8`, `G(120) ≈ +10` (cap 12), `G(210) ≈ −6` (the legacy mask would notch here), each ±1 dB; `G = 0` at and above 400 Hz.
- `room_v2_modes_range_keeps_bass_balance`: with `f_S` overridden to 400 Hz, an error that is a broad +6 dB shelf below 80 Hz plus a +8 dB, Q 5 peak at 60 Hz → the peak is cut by ≥ 6 dB, the shelf at 30 Hz is changed by ≤ 1.5 dB.
- `room_v2_rolloff_is_never_boosted`: the 45 Hz 4th-order response from the first test, schroeder range with `f_S` 400 Hz → `G ≤ 0.01` below `f_roll` everywhere, and `G` between `f_roll` and `f_knee` no larger than the ramp allows.
- `room_v2_snr_caps_boosts`: SNR 25 dB at a −10 dB dip → boost ≤ 5 dB.
- `room_v2_generic_average_is_cut_only`.
- `room_v2_extreme_is_diotic_above_700_hz`: left/right errors that differ by ±4 dB at 2 kHz → `G_left == G_right` (1e-9) above 700 Hz; above 10 kHz `G = 0`.
- `room_v2_vbass_handoff_mask_follows_lr8`: the reference values above, 0 below `f_c/√2`.
- `room_v2_gain_is_added_after_heavy_light_smoothing`: an `EqInputs` with only a `Gain` room entry of +10 dB over a 1/6-oct-wide dip at 120 Hz → the resulting `equalization` at 120 Hz is within 0.5 dB of +10.
- `room_v2_self_check_warns_when_residual_is_large`.
- `room_v2_legacy_range_is_bit_exact`: legacy dispatch yields exactly the old `RoomFrs` (`term == LegacyError`) on the demo's specific measurements.
- Service: `config_room_range_defaults_and_validation` (defaults in `brir_defaults`; each invalid value rejected with the field named), `demo_default_room_range_runs_schroeder` (default config on the demo end to end: finite outputs, `cli_room_schroeder*` and `cli_room_range` logged, `hesuvi.wav` differs from the legacy run), `room_v2_with_vbass_logs_handoff`.
- CLI: `cli_room_v2_options_parse_and_validate` (each flag reaches kwargs; bad choice / out of range exits 2; no flag → kwargs unchanged).

## Docs

Write `docs/rust/ROOM_CORRECTION.md`: what each range does, every number above as implemented, the logs, the legacy dispatch, and why boosts stop at the rolloff and above ~400 Hz (SNR; spatial period of interference nulls λ/2 = 57 cm at 300 Hz but 17 cm at 1 kHz; no head shadow in the omni mic). Plain English, no marketing tone.

## Verification (foreground, paste the tail of each)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy
cargo check -p impulcifer-python
git status --porcelain
```

If a command runs longer than your tool allows, split it per crate; never background it.

## Report format

(1) the public API as landed (signatures); (2) for each section 2–6 one paragraph on any place you had to interpret the spec, with the choice made; (3) the list of tests switched to legacy; (4) new test names, one line each; (5) the pasted `test result:` lines and `git status --porcelain`; (6) the demo run's logged Schroeder frequency, T60, rolloff and knee per ear and residual RMS per ear; (7) anything undone. Do not end your turn before the commands complete. If you find another contradiction, choose the reading that keeps the stated purpose of the rule, implement it, and list it under (2) instead of stopping.
