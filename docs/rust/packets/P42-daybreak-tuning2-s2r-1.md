# P42: virtual room tuning 2, bundle 1 of ADR 0015 (dip limit, low-end protection, extra bands, residual guard)

Repository worktree: `E:\Impulcifer\.worktrees\s2r1` (Git Bash: `/e/Impulcifer/.worktrees/s2r1`), branch
`claude/tuning2-s2r-1`. Work only inside it. Read first:

- `E:\Impulcifer\.worktrees\s2r1\CLAUDE.md`, section "3.x Rust 워크스페이스": `forbid(unsafe_code)`, no new
  dependencies, the `features.toml` gate, `EXTENSION_FIELD_NAMES` / `EXTENSION_OPTIONS`, brir-3x byte identity,
  "DSP를 병렬화할 때는 map만" (everything here is sequential f64).
- `E:\Impulcifer\.worktrees\s2r1\docs\rust\ROOM_CORRECTION.md`, section "Virtual room tuning v2": **the
  specification**. The parts marked 3.13.0 are what you build: "Options" (three new rows and the
  `room_tuning2_bands` paragraph), "Correction band" ("Band set"), "Design curve" (steps 3, 4, 6, "Rolloff",
  "Limited dips"), "PEQ fit" (the non-finite residual and "Reach"), "Low-end protection in the fits", "IIR-only
  mode", "FIR modes" (steps 1, 3, 4, 6, 7, 8 and the phase-score grid), "Diagnostics" (the three log keys),
  "Reference acceptance" (the `s2r_*` family). When this packet and the spec differ, the spec wins.
- `E:\Impulcifer\.worktrees\s2r1\docs\adr\0015-secs2r-improvements-into-tuning2.md` (Korean; the decision).
  Bundle 1 is item 4.1 of that ADR: D56, D47 (main part), D48, D54. Nothing from the other items enters this
  branch, not even an option name.
- `E:\Impulcifer\.worktrees\s2r1\tests\migration\README-secs2.md`: the fixture encoding and the gate
  methodology (`MEASURE` lines, budgets ×1.5) that the new `s2r_*` family reuses.

Clean room: the three options reproduce behaviour of a GPL program. You implement them from the specification
and the fixtures only. Do not search for, open, or ask for that program's source, and do not look for its
repository on this machine. If the spec is ambiguous, say so in the report and implement the reading the
fixtures support.

## Fixtures

`E:\Impulcifer\.worktrees\s2r1\tests\migration\goldens\s2r_*.json` with `.f64` sidecars, encoded exactly like
the `s2_*` family (`README-secs2.md`, "Encoding"; `s2r_bins.json` lists the stored bins and equals
`s2_bins.json`). Every fixture is `{inputs, outputs, meta}`; `meta.weights` gives the position weights the
oracle used (`[5, 1]` for the demo cases: left ear is its listening position with weight 5), `meta.libm`
names the C library the oracle ran on, and `meta.settings` the settings. Families:

| Family | What is frozen | How the test feeds it |
| --- | --- | --- |
| `s2r_libm_probe` | 400 x values and their `ln`, `exp(x/1000)`, `log10`, `log2`, `sqrt`, `x^0.5`, `sin`, `cos` on the oracle's machine | compute the same with Rust `f64`; all equal → this platform has the oracle's libm (exact gates), otherwise the 1e-9 gates apply |
| `s2r_rolloff_<spk>` | per speaker: `power`, `db` (at bins), `weights`, `reference`, `threshold`, `rolloff_hz` or `null`, `crossing` | rebuild P from the demo omni files with the SECS position policy (weight 5 on position 0) as `golden_secs2_rc.rs` does; compare `power` and `db` at the bins (precondition), then the scalars |
| `s2r_rolloff_synthetic_{hp40,flat}` | full-resolution `power` (one position, weight 1) and the same outputs | feed the stored power directly |
| `s2r_clamp_<case>` | bands before/after the centre clamp | direct |
| `s2r_limit_<case>` | `peq`, `fixed` (complex, the bins below the rolloff, full), `rolloff_hz`, `outcome_in`; outputs `peq`, `outcome`, `limited`, `worst_before`, `worst_after` | direct: run `Limit` and `Limited` on the stored inputs |
| `s2r_dc_<case>` | FIR before/after the DC correction, `applied`, `excess` | direct |
| `s2r_dip_<spk>`, `s2r_dip_synthetic_{narrow8,broad6}` | `power`, `sm`, `target`, `level_db`, `corr_before`; `env`, `depth`, `fill`, `corr_after`, `limited`, `unlimited`, `notes` | demo cases: rebuild P as above and check `power`/`sm`/`corr_before` at the bins first; synthetic: feed the stored full-resolution power |
| `s2r_mask_<spk>` | `corr`, `rolloff_hz`, `max_boost_db`, `nb`, `lm`, `bands`; `design_db` | direct on the stored `corr` at the bins is not possible (the masks need every bin), so rebuild the design curve from the demo with the SECS policy up to step 2, check it at the bins, then apply steps 4 to 6 and compare `design_db` |
| `s2r_band_set_<case>`, `s2r_masks_<case>` | band-set merging; `magnitude_mask`, `phase_fade`, `phase_error_band`, `band_grid`, `fit_reach` | direct |
| `s2r_residual_{finite,nonfinite}` | the fit residual on a 900-point grid, with one non-finite band in the second case | direct through the residual function of the PEQ fit |
| `s2r_rc_<spk>_<off,dip,protect,bands,all>` | the oracle's end-to-end channel correction: `design_db`, `peq`, `fir`, `response`, `phase_strength`, `notes`, `rolloff_hz` | run the whole RC with the SECS policy; `design_db` is gated, the rest is report-only (see "Gates") |

If a family the exporter produced is missing from this table, or a fixture holds a field this table does not
name, use it as its `meta` describes and say so in the report. `meta.settings` of the `s2r_rc_*` cases lists
every switch of the oracle's own configuration, including ones that are not part of this bundle (all `false`
or empty there); ignore those names, they are not options of Impulcifer. The oracle ran on Windows
(`meta.libm = "msvcrt"`), so on a Linux or macOS CI runner `s2r_libm_probe` will not match and the 1e-9
gates apply there; on this machine it should match exactly.

## What to build

### Configuration (`crates/impulcifer-types/src/config.rs`)

Three fields on `ProcessingConfig`, all extension fields appended to `EXTENSION_FIELD_NAMES` (29 → 32):

```rust
pub room_tuning2_dip_limit: bool,      // default false
pub room_tuning2_low_protect: bool,    // default false
pub room_tuning2_bands: String,        // default "" (none)
```

Parsing and validation per the spec's "Options": `room_tuning2_bands` accepts the empty string or up to four
`lo-hi` pairs separated by commas, whitespace around a pair ignored, both numbers finite with
`20 ≤ lo < hi ≤ 20000`; store the string as given (trimmed), expose the parsed pairs through a method
`ProcessingConfig::room_tuning2_bands() -> Vec<(f64, f64)>` (sorted by lo, not merged). Errors name the field
like the other tuning2 fields. `from_kwargs` / JSON: the booleans accept JSON booleans (and the strings the
other booleans accept, if the existing code accepts any); the bands field accepts a string. Update the two
tests that assert `EXTENSION_FIELD_NAMES.len() == 29` (`crates/impulcifer-types/tests/itd_config.rs`,
`crates/impulcifer-types/tests/output_fill_config.rs`) to 32 and extend
`crates/impulcifer-types/tests/tuning2_config.rs` (`config_room_tuning2_defaults_and_validation`) with the
three fields: defaults, accepted forms, every rejection named above, and that the parsed pairs come back
sorted.

Service validation (`crates/impulcifer-service/src/brir/validation.rs`): the same rules for the JSON request,
rejecting with `INVALID_REQUEST` naming the field, as the existing tuning2 fields do. Extend the service's
`config_room_tuning2_defaults_and_validation` test.

CLI (`crates/impulcifer-cli/src/options.rs`, `EXTENSION_OPTIONS`): `--room_tuning2_dip_limit true|false`,
`--room_tuning2_low_protect true|false`, `--room_tuning2_bands LO-HI[,LO-HI...]`, help text in the style of
the neighbouring tuning2 options. Extend `cli_room_tuning2_options_parse_and_validate` in
`crates/impulcifer-cli/tests/cli.rs`.

Policy gate: `CANONICAL_CONFIG` in `crates/impulcifer-policy/tests/gates.rs` lists every config field
(`[&str; 52]` → 55). The Python module (`crates/impulcifer-python/src/module.rs`) iterates the name lists and
needs no change; run its Rust tests anyway.

### DSP (`crates/impulcifer-dsp/src/stages/room_tuning2/`)

Carry the three options into `Tuning2Options` / `RcOptions` (`apply.rs`, `mod.rs`) and implement the spec:

- **Band set** (`mod.rs`): `CorrectionBand` gains the merged set (`Vec<(f64, f64)>`, primary first after
  sorting); the phase-score grid and the long-mode peak filter use it as the spec says. With no extras the set
  is `[(lo, hi)]` and every existing code path must take exactly the branch it takes today.
- **Design curve** (`rc.rs`): steps 3 (dip limit), 4 (low-end protection), 6 (band fade over the set), the
  rolloff, and the limited-dip report. Keep the unlimited curve alongside only when the dip limit is on.
- **Low-end protection in the fits**: a new module `protect.rs` with `rolloff`, `clamp`, `limit`, `limited`
  and the FIR DC correction, called from `rc.rs` exactly where the spec's "Where" paragraph and "FIR modes"
  steps 1, 4, 6, 7, 8 put them. `limit` returns the outcome (reduced band indices, cut count) so the post-FIR
  call can continue it.
- **Reach** and the **fit floor** (`peq.rs`, `rc.rs`): the residual's non-finite rule (always on), the reach
  band list for the zero-mode fits with extras, and `lo = max(…, f_r)`.
- **Reports**: `Speaker2Report` (`apply.rs`) gains the rolloff (Hz or none), the limit outcome (reduced count,
  cut count, limited), the limited dips (up to six `(hz, depth, fill)`), and the merged band set. The README
  writer (`crates/impulcifer-dsp/src/stages/readme.rs`) and the service log
  (`crates/impulcifer-service/src/brir/run.rs`, where `cli_room_tuning2_*` is logged today) emit the three
  keys of the spec's "Diagnostics": `cli_room_tuning2_low_protect` `{speaker, rolloff, reduced, cuts, limited}`
  (`rolloff` one decimal or `none`; `limited` as the catalog's yes/no words if a pattern exists, else
  `true`/`false`), `cli_room_tuning2_dip_limit` `{speaker, count, hz, depth, fill}`, `cli_room_tuning2_bands`
  `{bands}` (`lo-hi` pairs joined by `, `). Add these keys and the README line keys you need to **all nine**
  catalogs `crates/impulcifer-service/locales/<lang>.json` with English text and identical placeholders
  (translation is a later packet); `cargo test -p impulcifer-service --lib settings --test ui_catalog` must
  pass.

Byte identity: with the three options at their defaults every existing test, every `s2_*` gate and the
`room_tuning_secs2_*` references must pass unchanged, and the demo outputs must be byte-identical to
`origin/master` (brir-3x checks this in CI; run the local comparison described in
`.github/scripts/brir_3x_integrity.py` for the default tuning2 scenario if the script supports a local run,
and report the hashes).

### Tests and gates

- `crates/impulcifer-dsp/tests/golden_secs2r.rs`: one test per family, reading fixtures through
  `golden_secs2_helpers.rs` (`fixtures("s2r_…")`, `Errors`, `MEASURE` output). Budgets: on a platform where
  `s2r_libm_probe` matches Rust's `f64` exactly, scalars, bands, indices, outcomes and masks are exact and
  curves are relative 1e-12 (absolute floor 1e-12); otherwise relative 1e-9. The bisection in `Limit` and
  the `worst` choices are integer decisions: compare them exactly on the oracle's libm and report-only
  elsewhere, as the `s2_*` family does for its fit traces. `s2r_rc_*`: `design_db` within the curve budget,
  `rolloff_hz` and `notes` exact; `peq`, `fir`, `response`, `phase_strength` report-only (print the maximum
  differences), because the oracle's PEQ refinement diverges beyond its first evaluations like SECS 2's.
- Synthetic unit tests of your own for each rule where a fixture would be thin: an extra band that merges with
  the primary, four extras out of order, the `lo ≥ hi` rejection, a rolloff that lands exactly on the 20 Hz
  endpoint, a `Limit` call that excludes a bin, a DC correction below the threshold.
- `tests/migration/README-secs2r.md`: families, encoding (refer to `README-secs2.md`), and the measured
  budgets table with the `MEASURE` maxima you observed, in the style of `README-secs2.md` "Gates".
- `features.toml`: `config.room_tuning2_dip_limit`, `config.room_tuning2_low_protect`,
  `config.room_tuning2_bands` (kind `config`, tests = the three config/CLI tests above) and
  `room.tuning2_dip_limit`, `room.tuning2_low_protect`, `room.tuning2_bands` (kind `stage`, tests = the
  matching `golden_secs2r` tests and one service test that runs the demo with the option on and finds the log
  key). `cargo test -p impulcifer-policy` must pass.
- brir-3x: add three scenarios to `.github/scripts/brir_3x_integrity.py` tagged `"3.13.0"`:
  `room_tuning2_dip_limit` (`--room_mode=tuning2 --room_tuning2_dip_limit=true`), `room_tuning2_low_protect`
  (`--room_mode=tuning2 --room_tuning2_low_protect=true`), `room_tuning2_bands`
  (`--room_mode=tuning2 --room_tuning2_bands=1000-3000`).
- README: three rows in the CLI option table next to the other `--room_tuning2_*` rows, Korean, same style.
- Version: `3.13.0` in `Cargo.toml` (`[workspace.package].version`), `apps/impulcifer-app/tauri.conf.json`
  and `Cargo.lock` (run a build so cargo rewrites the lock). CHANGELOG: a new entry at the top

  ```
  ## 3.13.0 - <today>
  ### 가상 룸 튜닝 2의 딥 제한, 저역 보호, 추가 보정 구간

  #### ⭐ 새로운 기능 / 개선
  - **…**: …
  ```

  in Korean, one bullet per option plus one for the always-on residual guard, saying that the defaults keep
  the output byte-identical and that the behaviour comes from SECS 2R through the clean-room method of
  ADR 0015. Keep the category heading's emoji exactly as above.

Do not touch: `apps/impulcifer-app/ui/*` (the screen and `ipc.d.ts` are a later packet), `webview_ui/`,
`i18n/locales/`, `CLAUDE.md`, the ADR, `docs/rust/ROOM_CORRECTION.md` (if the spec needs a correction, say
so in the report instead of editing it), the `s2_*` fixtures.

## Verification (foreground, before reporting; `env -u HTTPS_PROXY -u HTTP_PROXY` in front of cargo if a
command fails on the network)

```
cd /e/Impulcifer/.worktrees/s2r1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-cli -p impulcifer-policy
cargo test -p impulcifer-dsp
cargo test -p impulcifer-service
cargo test -p impulcifer-python
```

Then the brir-3x local comparison for `room_tuning2` (default) between `origin/master` and this branch if
the script supports it, and a run of each new scenario to show it completes.

## Report (in this order, then end your turn)

1. Files changed with a one-line purpose each.
2. The `MEASURE` table: every family's observed maximum and the gate you set, and whether this machine has
   the oracle's libm.
3. The `s2r_rc_*` report-only maxima (per setting: `peq` band-wise, `fir`, `response` in dB, `phase_strength`).
4. Byte identity: the hashes or the test names that prove the defaults are unchanged.
5. The exact commands run and their final lines.
6. Open questions and anything you could not do, with the reason.
