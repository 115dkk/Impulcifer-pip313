# P36: fill unmeasured speakers and the LFE channel in the output files (ADR 0013)

Repository: `/home/user/Impulcifer-pip313`. Read first:

- `/home/user/Impulcifer-pip313/CLAUDE.md` (3.x workspace section: `forbid(unsafe_code)`, no new dependencies,
  `features.toml`, `EXTENSION_FIELD_NAMES`/`EXTENSION_OPTIONS`, brir-3x integrity, "DSP를 병렬화할 때는 map만").
- `/home/user/Impulcifer-pip313/docs/adr/0013-missing-speakers-and-lfe-channel.md` (the decision; Korean). This
  packet restates everything you need in English.

## What to build

Two new 3.x extension settings that change only what the output files contain:

| field | values | default | CLI |
| --- | --- | --- | --- |
| `missing_speakers` | `silent`, `neighbor` | `silent` | `--missing_speakers` |
| `lfe_mode` | `none`, `center`, `all` | `none` | `--lfe_mode` |

With the defaults every output file must stay byte-identical to today (the brir-3x workflow compares SHA-256 of
all 26 scenarios against the base commit). Follow the existing `layout_files` / `vbass_mode` pattern in
`crates/impulcifer-types/src/config.rs` (field, default, `EXTENSION_FIELD_NAMES` (25 → 27), validation with an
exact error message listing the allowed values, `from_kwargs`), and in `crates/impulcifer-cli/src/options.rs`
(`EXTENSION_OPTIONS` with choices and help text). Check how the service validates `start_brir` requests
(`crates/impulcifer-service/src/brir/validation.rs`) and add the two fields where the other extension strings
are handled.

## Where the outputs are assembled

`crates/impulcifer-dsp/src/pipeline.rs`, stage `StageKey::WriteBrirs`: `output_tracks(&hrir, &HEXADECAGONAL_TRACK_ORDER,
..)` (hrir.wav), `output_tracks(&hrir, &HESUVI_TRACK_ORDER, ..)` (hesuvi.wav) and `layout_tracks(&hrir, layout)` for
every layout file (`LayoutOutput { layout, tracks, missing }`). Tracks are found by name (`"FL-left"`), and a name
with no IR is a silent row. LFE channels of a layout use the slot names `LFE` / `LFE2` (`layout.track_names()`).

Put the new logic in a new module `crates/impulcifer-dsp/src/stages/fill.rs` and call it from `WriteBrirs`. Build
one output copy of the HRIR for each file kind and stack from it; never modify `hrir` itself (later stages and
`responses.wav` must not see filled channels).

### `missing_speakers = "neighbor"`

Which slots are filled:

- `hesuvi.wav` and `hrir.wav`: only the speakers in the first `base_channel_count(order)` tracks of
  `HESUVI_TRACK_ORDER` / `HEXADECAGONAL_TRACK_ORDER` (`impulcifer_types::constants::base_channel_count`). Never
  the 3.x extension slots after them.
- Every layout file that is written: every non-LFE channel slot of the layout.
- No other output (TrueHD layouts, JamesDSP, Hangloose, `responses.wav`) changes.

How a missing slot `S` is filled (sources are only measured speakers, never a slot filled earlier). Positions come
from `impulcifer_types::layouts::SLOTS` (`azimuth`, `elevation` in degrees, ITU: positive azimuth is left;
`layer`). Angular distance = the angle between the two unit vectors built from azimuth and elevation. Side = 
`impulcifer_types::constants::speaker_side`.

1. `S` on the left or right side: candidates are measured slots with the same side and the same `layer`. Take the
   smallest angular distance. Ties (|difference| < 1e-9): if `|azimuth(S)| < 90` take the candidate with the
   smaller `|azimuth|`, otherwise the larger; a remaining tie takes the earlier slot in `SLOTS` order. Copy both
   ears' IR of the chosen slot unchanged.
2. `S` in the centre (`Side::Center`): candidates are symmetric measured pairs on the same layer (a left slot L and
   a right slot R with `azimuth(R) == -azimuth(L)` and equal elevation). Take the pair whose left member has the
   smallest angular distance to `S` (same tie rules). Each ear of `S` = (L's ear + R's ear) / √2, sample by sample.
   With no pair, take the nearest measured centre slot on the same layer (copy).
3. No candidate on the same layer: leave `S` silent.

Expected choices (put them in a test): with the demo set FL FR FC BL BR SL SR measured, WL → FL (tie between FL and
SL at 30°, front rule) and WR → FR; with only FL FR BL BR measured, SL → BL, SR → BR, FC → (FL+FR)/√2; with
FL FR SL SR measured, BL → SL; with TFL TFR measured, TBL → TFL and TSL → TFL; with no top slot measured, TFL stays
silent; BC with BL BR measured → (BL+BR)/√2.

### `lfe_mode`

Applies only to layout files (their `LFE` / `LFE2` channels; both get the same IRs). `hesuvi.wav` and `hrir.wav`
have no LFE tracks and do not change.

- `none`: silent, as today.
- `center`: FC's two ears unchanged. FC not measured: (FL+FR)/√2 per ear. Neither: silent.
- `all`: one IR per ear built from every measured speaker (filled slots excluded):
  1. `n` = the common IR length L (all IRs have the same length at this stage; return an error if not),
     `nfft` = next power of two ≥ 2·L.
  2. For each ear e: `P_e[k] = mean over measured speakers s of |rfft(ir_{s,e}, nfft)[k]|²`, `M_e = sqrt(P_e)`.
  3. Multiply `M_e[k]` by the magnitude of a 4th-order Linkwitz-Riley low-pass at 120 Hz, i.e. `1 / (1 + (f/120)^4)`
     (the squared magnitude of a 2nd-order Butterworth, which is the LR4 magnitude).
  4. Minimum phase from that magnitude: use an existing routine if one fits (look at
     `crates/impulcifer-dsp/src/stages/room_tuning.rs::minimum_phase` and `crates/impulcifer-dsp/src/fir.rs`;
     read what they return and do not change them), otherwise the real cepstrum method on `nfft` points. Take the
     first L samples.
  5. One delay for both ears: `d` = the median (lower median for an even count) of the peak index
     (`argmax |x|`) over every measured speaker's left and right IR. Shift the minimum-phase IR right by `d` samples
     (zeros in front, truncate to L).
  Deterministic and sequential (no rayon reduction).

### Reporting

- `LayoutOutput` gains `filled: Vec<(&'static str, String)>` (slot, source description) and `missing` keeps only the
  channels that stay silent. The service's existing `cli_warning_layout_partial` / `cli_readme_layouts_partial`
  then only name truly silent channels.
- Source descriptions are plain codes: `"BL"`, `"FL+FR"` for a phantom pair, `"FC"`, and `"all"` for `lfe_mode = all`.
- Service (`crates/impulcifer-service/src/brir/outputs.rs`): for each written file with filled channels log
  `info` `cli_info_filled_channels` with `{"file":..., "list": "SL ← BL, SR ← BR"}` and, for LFE,
  `cli_info_lfe_filled` with `{"file":..., "source": <description>}`. In the README's layout section and in a new
  short section for hesuvi.wav/hrir.wav, list the filled channels with `cli_readme_filled_channels`
  (`{list}`), and LFE with `cli_readme_lfe_filled` (`{source}`). For the `all` source, the README and log use the
  catalog text `cli_lfe_source_all` instead of the code `all`.
- Add these keys to all nine overlay catalogs `crates/impulcifer-service/locales/*.json` with the English text in
  every file (a translator replaces the eight other languages later):
  - `cli_info_filled_channels`: `{file}: unmeasured channels are filled from the nearest measured speaker ({list})`
  - `cli_info_lfe_filled`: `{file}: the LFE tracks use {source}`
  - `cli_readme_filled_channels`: `Unmeasured channels filled from the nearest measured speaker: {list}`
  - `cli_readme_lfe_filled`: `The LFE tracks use {source}.`
  - `cli_lfe_source_all`: `the low frequencies of every measured speaker (below 120 Hz)`

## Tests (must exist and pass; add them to `features.toml`)

- `impulcifer-dsp`: `neighbor_fill_picks_the_nearest_slot_on_the_same_side_and_layer` (the table of expected choices
  above, including the silent top case and the phantom values), `fill_never_uses_a_filled_slot_as_source`,
  `lfe_center_copies_fc_or_the_front_phantom`, `lfe_all_is_the_power_average_below_the_crossover` (synthetic
  speakers: check the magnitude at 40 Hz within 0.5 dB of the power average, at 1 kHz at least 60 dB down, the peak
  of the result after `d`, identical result on repeated calls), `defaults_leave_every_output_unchanged` (run the
  WriteBrirs assembly with defaults and with `missing_speakers=neighbor`/`lfe_mode=all` on an HRIR where nothing
  is missing and no layout has LFE: identical tracks).
- `impulcifer-types`: config defaults and validation messages for both fields; `EXTENSION_FIELD_NAMES` length.
- `impulcifer-cli`: both options parse, reject an invalid value, and appear in `--help`.
- `impulcifer-service`: a BRIR job on `data/demo` with `layout_files: "9.1"`, `missing_speakers: "neighbor"`,
  `lfe_mode: "center"` writes `atmos_9.1.wav` (check the real file name in `layouts.rs`) whose WL tracks equal the
  FL tracks and whose LFE tracks equal the FC tracks, logs `cli_info_filled_channels` and `cli_info_lfe_filled`,
  and the README names the filled channels; the same job with defaults keeps WL and LFE silent.
- `features.toml`: `output.missing_speakers` and `output.lfe_mode` (`kind = "output"`, `status = "implemented"`,
  real test paths), and add the two config fields wherever the other extension config fields are registered
  (look for `config.layout_files`).

## Allowed files

`crates/impulcifer-types/src/config.rs` (+ its tests), `crates/impulcifer-cli/src/options.rs` (+ tests),
`crates/impulcifer-dsp/src/stages/fill.rs` (new), `crates/impulcifer-dsp/src/stages/mod.rs`,
`crates/impulcifer-dsp/src/pipeline.rs`, `crates/impulcifer-service/src/brir/outputs.rs`,
`crates/impulcifer-service/src/brir/validation.rs`, `crates/impulcifer-service/locales/*.json` (only the five keys),
service/cli/dsp/types `tests/` files, `features.toml`. Do not touch `apps/`, `.github/`, docs, `Cargo.toml`,
`Cargo.lock`, the 2.x tree, or any golden fixture. If a parity test compares the 2.x field list, the new fields
belong to the extension lists, not to `FIELD_NAMES`/`OPTIONS`.

## Verification (foreground; report the tail of each)

```bash
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-cli
cargo test -p impulcifer-service --lib
cargo test -p impulcifer-service --test brir_outputs --test brir_inputs --test brir_ipc --test config_parity --test immersive_layouts --test ui_catalog
cargo test -p impulcifer-policy
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails in this container because it runs as
root; ignore only that one. Use `CARGO_PROFILE_DEV_DEBUG=0` if the disk runs low. Run every command in the
foreground and end your turn only after reporting: the files changed, the public signatures, the test names and the
command outputs. Do not commit. Do not use a type alias or any other trick to get past a policy gate; if a gate
blocks the design, stop and report it.
