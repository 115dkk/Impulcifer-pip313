# A09 (ASTRA): virtual room tuning v2 — config, CLI, service, registry

Work in `/home/user/wt-a09`, a git worktree of the repository on branch `wt-a09` (do not switch branches, do not commit, do not stash; do not touch `/home/user/Impulcifer-pip313`, where another worker is finishing the MIMO guard and gates in `crates/impulcifer-dsp` (A08b) at the same time). This wires the library you built in A07/A08 (`crates/impulcifer-dsp/src/stages/room_tuning2/`) into the product. Same rules as A07/A08 (clean-room: no SECS source, nothing under `/tmp/claude-0/`). Specification: `docs/rust/ROOM_CORRECTION.md` "Virtual room tuning v2" (Options, Measurements and spectra, Channel level, Correction band, Diagnostics), ADR 0008, and `CLAUDE.md` "3.x Rust 워크스페이스" (extension fields go to `EXTENSION_FIELD_NAMES`/`EXTENSION_OPTIONS`, never to the 2.x parity lists; every feature in `features.toml`). Every command in the foreground. Do not delegate to another agent.

Already in the tree and not yours to change, but your wiring must serve them exactly:
- The 3.x screen (`apps/impulcifer-app/ui/app.js`, `ipc.d.ts`) sends `room_mode: "tuning2"`, `room_tuning2_mode` (one of the four names, or the latency as a number), `room_tuning2_max_boost`, `room_tuning2_low`, `room_tuning2_high` (`"auto"` or a number), `room_tuning_level_match`, `room_tuning2_mimo`, and `room_tuning2_fsiso`/`room_tuning2_mimo_low` (`"auto"` or a number) only when MIMO is on; `room_volume`/`schroeder_freq` only when the high end is `auto`. It reads the defaults of all seven fields from `brir_defaults` under these names. Run `cd apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts` and report the tail.
- `.github/scripts/brir_3x_integrity.py` runs the release CLI with `--room_mode=tuning2`, `--room_tuning2_mode=zero|linear`, `--room_tuning2_mimo=true --room_tuning2_fsiso=120`, and `--vbass --room_mode=tuning2`. Those exact spellings must parse.
- The 3.x overlay catalogue (`crates/impulcifer-service/locales/*.json`) already has the log keys below in all nine languages; `cargo test -p impulcifer-service --lib settings --test ui_catalog` must pass.

Allowed files: `crates/impulcifer-types/**`, `crates/impulcifer-cli/**`, `crates/impulcifer-service/src/**` and `crates/impulcifer-service/tests/**` (not `locales/`), `crates/impulcifer-python/**` only where it lists config fields or defaults, `crates/impulcifer-policy/**` (canonical names), `features.toml`, in `crates/impulcifer-dsp` only `room_tuning2/mod.rs` to move `Tuning2Mode` into `impulcifer-types` (re-export it from the dsp module so existing paths keep working); anything else the wiring needs from the dsp crate goes into the report instead (A08b owns those files right now). Use `export CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false`; if the disk fills, delete this worktree's `target/debug` and rerun. Do not touch `apps/**`, `crates/impulcifer-service/locales/**` (another packet writes the catalogue — use exactly the keys below), `.github/**`, versions, `CHANGELOG.md`, `README.md`, `docs/**`, the 2.x Python tree.

## Config (impulcifer-types)

`room_mode` accepts `eq`, `tuning`, `tuning2` (validation message names all three). New extension fields (defaults; validation rejects out-of-range values naming the field):

| Field | Type | Default | Valid |
| --- | --- | --- | --- |
| `room_tuning2_mode` | string (string or number in JSON) | `"mixed"` | `zero`, `mixed`, `linear`, `long`, or 30–200 |
| `room_tuning2_max_boost` | f64 | 10 | 0–10 |
| `room_tuning2_low` | f64 | 20 | 10–200 |
| `room_tuning2_high` | string (string or number) | `"auto"` | `auto` or 100–20000, and above `room_tuning2_low` |
| `room_tuning2_mimo` | bool | false | |
| `room_tuning2_fsiso` | f64 | 150 | 50–150 |
| `room_tuning2_mimo_low` | string (string or number) | `"auto"` | `auto` or 15–150 |

Parse `room_tuning2_mode` with `Tuning2Mode::parse` (move it to `impulcifer-types` if the dsp crate should depend on it from there, like `TuningDelay`/`PhaseLimit`; keep one definition). `tuning2` with `room_range = "legacy"` is accepted (the range is inert, as for v1). `ProcessingConfig::oracle_defaults()` and every 2.x parity list stay as they are.

## CLI (impulcifer-cli)

Options `--room_tuning2_mode`, `--room_tuning2_max_boost`, `--room_tuning2_low`, `--room_tuning2_high`, `--room_tuning2_mimo` (`true|false`, like `--room_tuning_level_match`), `--room_tuning2_fsiso`, `--room_tuning2_mimo_low` in `EXTENSION_OPTIONS`, defaults suppressed like the other extension options; `--room_mode` choices gain `tuning2`. Help texts (English, plain):

- `--room_mode`: append `"tuning2" applies, per speaker, the room correction of SECS 2 (Speaker Gallery): a PEQ and FIR in front of each speaker that corrects its bass response and part of the room's bass timing, and optionally MIMO supports from the other speakers that cancel the late bass energy at the ears; every channel starts later by the mode's latency.`
- `--room_tuning2_mode`: `Filter structure and latency of "tuning2": zero (PEQ only, no delay), mixed (8 ms, default), linear (28 ms), long (200 ms), or a latency of 30–200 ms.`
- `--room_tuning2_max_boost`: `Largest boost of "tuning2" in dB (0–10, default 10).`
- `--room_tuning2_low` / `--room_tuning2_high`: `Lower / upper end of the "tuning2" correction band in Hz. The upper end defaults to auto: the Schroeder frequency from --schroeder_freq or --room_volume, at most 500 Hz.`
- `--room_tuning2_mimo`: `Add MIMO supports: low-frequency copies of each speaker's signal sent to the other speakers to cancel the late bass energy at the ears (default false).`
- `--room_tuning2_fsiso` / `--room_tuning2_mimo_low`: `Upper end of the MIMO supports in Hz (50–150, default 150)` / `Lower end of the MIMO supports in Hz, or auto (default).`

## Service (impulcifer-service)

- **Inputs** (`brir/inputs.rs`): for `tuning2`, ingest the room files as for `tuning` (keep the per-recording pairs for the geometry check; `room.wav` ignored with `cli_room_tuning_room_wav_ignored`), and build the plan with `room_correction_tuning2` from the **uncropped** split IRs, the room target (same target object v1 receives) and the mic calibration. Options from the config; `RcOptions.high` from `room_tuning2_high`, `schroeder` from `schroeder_freq`, `room_volume` from `room_volume`, policy `Symmetric`; `level_match` from `room_tuning_level_match`; MIMO options when `room_tuning2_mimo`. Every other place that tests `room_mode == "tuning"` (virtual bass's room gains and room-target rule, any `!= "tuning"` filter) must treat `tuning2` as a tuning mode: list each place you changed in the report.
- **Logs** (`brir/run.rs`, from `on_room_tuned2`), exact keys and arguments:

| level | key | args |
| --- | --- | --- |
| info | `cli_room_tuning_no_data` | — (no measured speaker) |
| info | `cli_room_tuning2` | `mode` (string: `zero`/`mixed`/`linear`/`long` or the ms value as an integer string), `delay` (int ms), `low` (int Hz), `high` (int Hz) |
| info | `cli_room_tuning_single_point` | `speaker` |
| warning | `cli_room_tuning_level_inconsistent` | `speaker`, `db` (1 decimal string) |
| warning | `cli_room_tuning_mismatch` | `speaker` |
| warning | `cli_room_tuning_weak` | `speaker`, `db` (as v1) |
| warning | `cli_room_tuning2_preecho` | `speaker`, `db` (1 decimal string), same rule as v1's pre-echo warning |
| info | `cli_room_tuning2_mimo` | `speaker`, `supports` (int), `db` (1 decimal string, support level) |
| info | `cli_room_tuning2_mimo_off` | `speaker` |

  No v1 key that v1 alone produces (`cli_room_tuning`, `cli_room_tuning_off`, `cli_room_tuning_preecho`) appears in a `tuning2` run, and no `cli_room_tuning*` key appears in an `eq` run.
- **README** (`brir/outputs.rs`): a section `## Virtual room tuning v2` with the mode, latency (ms), band (Hz) and its source (manual / Schroeder override / room volume / default), then a table per speaker: points, level (dB), trim (dB) and source, PEQ bands, phase strength, phase resolution (1/24 or 1/6 oct, `—` in `zero`), long modes (Hz list or `—`), representativeness RMS, LF EDT and median excess GD before/after (information), weak; the per-ear pre-echo table as v1; with MIMO a table per input: supports (speaker names), support level (dB), guard (`0 dB`, `−3 dB` … or `off`), band and tail change (dB). End with the attribution line from the specification.
- **Bootstrap / defaults / IPC**: every place that publishes extension defaults (bootstrap, `brir_defaults`, the Python filter if any) includes the seven fields.

## Registry (features.toml, policy)

New features, `status = "implemented"` with existing test names (the policy test fails on a missing test): `config.room_tuning2_mode`, `config.room_tuning2_max_boost`, `config.room_tuning2_low`, `config.room_tuning2_high`, `config.room_tuning2_mimo`, `config.room_tuning2_fsiso`, `config.room_tuning2_mimo_low`, `room.virtual_tuning2` (RC + application tests from A07/A08 and the service demo test), `room.tuning2_mimo` (MIMO tests), `room.tuning2_secs2_reference` (the two SECS reference tests and `trf_matches_scipy`), and `perf.room-tuning2` pointing at the performance report test or bench smoke used by other `perf.*` entries (follow their pattern). Update `config.room_mode`'s tests if needed. Canonical config names in the policy crate.

## Tests (names fixed)

- types/service: `config_room_tuning2_defaults_and_validation` (defaults, every boundary in and out, string/number JSON forms, `tuning2` + `legacy` accepted).
- CLI: `cli_room_tuning2_options_parse_and_validate` (every option, suppressed defaults, invalid values exit 2).
- service (`tests/room_v2.rs` or a new `tests/room_tuning2.rs`): `room_tuning2_runs_on_the_demo` (default `mixed`: finite outputs; `cli_room_tuning2` with delay 8, low 20, high 500; every channel of `hesuvi.wav` starts 8 ms (384 samples) later than the `eq` run's by the −18 dB direct peak within ±5 samples; no mismatch/weak/pre-echo warning; README section present; normalisation gain within 6 dB of the `eq` run), `room_tuning2_modes_on_the_demo` (`zero`: no delay; `linear`: 28 ms; `60`: 58 ms), `room_tuning2_mimo_on_the_demo` (MIMO on: `cli_room_tuning2_mimo` / `_off` logged for every measured speaker, README MIMO table, late LF energy at the ears lower than MIMO off for every input with supports — report the dB), `room_tuning2_ignores_v1_options` (changing `room_tuning_delay`, `room_tuning_curtain`, `room_tuning_phase_limit`, `room_tuning_max_boost` does not change the `tuning2` output bytes), `room_tuning2_level_match_off` (all trims 0 in the README).
- Make sure every existing test still passes; `eq` and `tuning` outputs unchanged.

## Verification (foreground; paste tails)

```
cd /home/user/wt-a09
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-cli -p impulcifer-policy --no-fail-fast
cargo test -p impulcifer-service --no-fail-fast
cargo test -p impulcifer-service --lib settings --test ui_catalog
(cd apps/impulcifer-app && npm ci && npm run typecheck && npm run test:contracts)
cargo build -p impulcifer-cli --release && for t in 1 4; do RAYON_NUM_THREADS=$t target/release/impulcifer --dir_path=data/demo --test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav --room_mode=tuning2 --room_tuning2_mimo=true --room_tuning2_fsiso=120 >/dev/null && sha256sum data/demo/hesuvi.wav; git checkout -- data/demo; done   # the two hashes must be equal (thread-count free)
cargo run -p impulcifer-cli --release -- --dir_path=data/demo --test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav --room_mode tuning2 --room_tuning2_mimo true   # paste the log lines and the README section; then restore data/demo with `git checkout -- data/demo` and `git status`
git status --porcelain
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails here because the container runs as root; it must be the only failure.

## Report format

(1) fields, options, defaults as landed; (2) every `"tuning"` check you extended to `tuning2`; (3) the demo run: log lines, README section, timings (whole CLI run with and without MIMO, `eq` for comparison); (4) `test result:` lines; (5) `git status --porcelain`; (6) anything undone. Do not end your turn before the commands complete.
