# A03b (ASTRA): room correction v2 — audit follow-ups

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `/home/user/Impulcifer-pip313/CLAUDE.md` (section "3.x Rust 워크스페이스") and `/home/user/Impulcifer-pip313/docs/rust/ROOM_CORRECTION.md` first; the implementation landed from packet `docs/rust/packets/A03-astra-room-correction-v2.md`. Rules: `#![forbid(unsafe_code)]`, no new dependencies, no shell subprocesses, run every command in the foreground, never in the background. ALSA development files are installed, so default-feature builds work.

Do not touch `apps/**`, `crates/impulcifer-service/locales/**`, `Cargo.toml`/`Cargo.lock` versions, `CHANGELOG.md`, `README.md`, `.github/**`, the Python 2.x tree. No change may alter any legacy output, golden, WAV or FIR, and no v2 numerical behaviour changes except where item 1 says so (it changes a plot only).

An independent audit of the landed code found the following. Fix each.

## 1. The generic-room plot must show the correction that was applied

`crates/impulcifer-service/src/brir/plots.rs` `generic_room()` always recomputes the legacy curve with `calculate_generic_room_correction(.., config.generic_limit)`. In the v2 ranges that is not what was applied. Change it so that for `room_range != "legacy"` the plot uses the generic entry the room stage produced (`RoomCorrection.frs`, `term == Gain`; the generic `FrequencyResponse` is the one pushed for speakers without specific files, its `error` is `−G`), and keep today's recomputation unchanged for legacy (the legacy plot bytes must not change; `brir_plots.rs` pins them). Pass what you need from `inputs.rs` (the call site is near line 299); keep `generic_room`'s behaviour for legacy byte-identical. If a v2 run has `room.wav` but every speaker also has specific files, no generic entry was pushed and the generic correction was applied to no speaker: skip `room.png` in that case (no legacy recomputation for v2) and say so in a one-line comment.

Test (service): `room_v2_generic_plot_uses_the_applied_gain` — a folder with only `room.wav` (build it the way existing generic-room tests in `crates/impulcifer-service/tests/` do), `plot = true`, default range: `plots/room/room.png` exists, and the curve handed to the plot equals `−G` of the applied generic entry. If the curve is not observable from outside, factor the curve selection into a small `pub(crate)` function and unit-test it in `plots.rs`.

## 2. Prove that the three numeric options reach the room stage

`features.toml` registers `config.room_volume`, `config.schroeder_freq`, `config.room_max_boost` with tests that would still pass if `inputs.rs` dropped the assignment. Add one service test, `room_v2_numeric_options_reach_the_room_stage`, that runs the room stage on the demo three times and asserts from the logs and/or the returned diagnostics:
- `room_volume = 100` → `cli_room_schroeder` is logged with `volume` `"100.0"` (and not the assumed key);
- `schroeder_freq = 400` → `cli_room_schroeder_override` with `freq` 400, and `cli_room_range` `f_hi` 400 for `schroeder`;
- `room_max_boost = 0` → no applied room gain is positive anywhere (inspect the applied entries or the equalization curves the pipeline returns; use whichever the existing tests already reach).
Reuse the existing helper in `crates/impulcifer-service/tests/room_v2.rs`; if running only the input-loading stage is possible (as `run_demo` may already do), prefer it to a full pipeline run. Register the test on those three features.

## 3. Test the diotic rule through the production path

`room_v2_extreme_is_diotic_above_700_hz` calls `correction_gain` and `blend_diotic` itself, so it would pass if `room_v2::room_correction` dropped or reordered the blend. Add `room_v2_extreme_room_correction_is_diotic_above_700_hz` that goes through `room::room_correction(..)` with `range = Extreme` on inputs where the two ears of one speaker differ (the demo's specific measurements do; or synthetic IRs with different per-ear responses and different noise levels so the SNR caps differ) and asserts `G_left == G_right` (1e-9) above 700 Hz and that they differ somewhere below 500 Hz. Register it on `room.v2_ranges`.

## 4. Legacy must log no room key at all

`crates/impulcifer-service/tests/room_v2.rs` asserts only that `cli_room_range` is absent for legacy. Assert that no logged key starts with `cli_room_` (note `cli_running_room_correction` does not match that prefix and must still be logged).

## 5. Tighten two boundary tests

- `room_v2_schroeder_estimate_sources_and_clamps`: add a short-decay and a long-decay synthetic case whose unclamped assumed-volume result falls below 120 Hz and above 300 Hz, and assert the clamps (and the same for the volume source's 80/500 Hz clamps).
- `crates/impulcifer-cli/tests/cli.rs`: the `--room_range` help check can pass from the legacy-only suffix of the limit options. Assert each `EXTENSION_OPTIONS` entry's flag and exact help sentence appear in `--help` output, and that `--room_range` is accepted by the parser with each of the four values.

## Verification (foreground; paste the tail of each)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy -p impulcifer-python --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-types -p impulcifer-dsp -p impulcifer-service -p impulcifer-cli -p impulcifer-policy --no-fail-fast
git status --porcelain
```

`brir_outputs::output_readonly_file_failure_preserves_existing_bytes` fails in this container because it runs as root (read-only permissions are ignored); that one failure is expected and must be the only one.

## Report format

(1) per item, what changed (files, functions) and the new test names; (2) the pasted `test result:` lines and the list of failures; (3) `git status --porcelain`; (4) anything you could not do. Do not end your turn before the commands complete.
