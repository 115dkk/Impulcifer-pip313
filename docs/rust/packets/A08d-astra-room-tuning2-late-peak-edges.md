# A08d (ASTRA): virtual room tuning 2 — late peaks at the band edges

Work in `/home/user/Impulcifer-pip313` on the checked-out branch (do not switch branches, do not commit, do not stash). All earlier rules apply: clean room (no SECS source or binary, nothing under `/tmp/claude-0/`, do not open or run `tests/migration/export_goldens_secs2.py`), numpy/scipy sources are fine, faer sequential and without `std`, every command in the foreground, no delegation.

Use `export CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false`.

## What changed

The specification's "Late peaks" definition (`docs/rust/ROOM_CORRECTION.md`, Shared numerical primitives) was ambiguous and has been corrected; read it again. The 20–160 Hz mask multiplies the summed spectrum over the whole `rfftfreq(4096, 1/600)` grid, and `find_peaks` runs on that masked array. The bins outside the band are zero, so the first in-band bin and the last in-band bin (159.96 Hz) are local maxima whenever the sum falls away from, or rises toward, the band edge. `primitives::late_peaks` searches only inside the band, so it can never return those two bins. That is exactly why the two guard values were named exceptions (mixed/100/low40 BL and FC trial 0): on those inputs the after-spectrum still rises at 159.96 Hz, the oracle counts that bin among its six peaks, and so it never reaches the 44 Hz peak our port took sixth.

## Task

1. Change `late_peaks` to the corrected definition. Its plateau handling must stay that of `scipy.signal.find_peaks`, applied to the full masked array (the zero region outside the band is a plateau of minima, never a peak). Every caller keeps its own filters: MIMO band-change extra bands, the tail-change modes, and the RC long-mode analysis (`lo ≤ p ≤ min(hi, 160)`).
2. Add a primitive test `late_peaks_include_band_edges`. Feed synthetic signals whose late spectrum rises steadily through 160 Hz and falls steadily from 20 Hz. The edge bins must come back as peaks, and an interior-only spectrum must behave as before. Compare against `scipy.signal.find_peaks` on the masked array, computed in the test from numbers you derive, or hard-coded from a scipy run you document.
3. Remove the named exceptions in `golden_secs2_mimo.rs`. All 85 guard values must now agree to 1e-9 absolute under the specification's rule. Keep `mimo_guard_hypotheses` only if it still adds information; otherwise reduce it to the single rule and its count.
4. Re-run every RC and MIMO gate (`golden_secs2_rc`, `golden_secs2_primitives`, `golden_secs2_mimo`, `room_tuning2_rc`, `room_tuning2_apply`). The RC long-mode bins must still be exact. Report the end-to-end guard-step counts per setting next to the worst oracle variant. Then set `GUARD_STEP_ALLOWANCE` in `room_tuning2_apply.rs` to the smallest value that passes (0 if no setting exceeds its worst variant), and keep its comment accurate.
5. Update the MIMO rows of `tests/migration/README-secs2.md` and the guard-hypothesis section of `docs/rust/perf/room-tuning2.md` (resolved: band-edge peaks). Do not edit `docs/rust/ROOM_CORRECTION.md`.

Allowed files: `crates/impulcifer-dsp/src/stages/room_tuning2/primitives.rs`, `crates/impulcifer-dsp/tests/{golden_secs2_primitives.rs,golden_secs2_mimo.rs,room_tuning2_apply.rs}`, `tests/migration/README-secs2.md`, `docs/rust/perf/room-tuning2.md`.

## Verification (foreground; paste tails and MEASURE lines)

```
cd /home/user/Impulcifer-pip313
cargo fmt --all -- --check
cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-policy
cargo test -p impulcifer-dsp --no-fail-fast
SECS2_ASSUME_FOREIGN_LIBM=1 cargo test -p impulcifer-dsp --no-fail-fast --test golden_secs2_mimo --test golden_secs2_primitives --test room_tuning2_apply
cargo test -p impulcifer-service --test room_tuning2
git status --porcelain
```

## Report

(1) the change to `late_peaks`; (2) the guard table (85 values, worst difference); (3) end-to-end guard-step counts per setting vs every oracle variant, and the allowance set; (4) RC gates after the change (long-mode bins, end-to-end distribution vs the envelope); (5) `test result:` lines; (6) `git status --porcelain`. Do not end your turn before the commands complete.
