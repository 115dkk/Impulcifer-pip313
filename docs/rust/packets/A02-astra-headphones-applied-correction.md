# A02 (ASTRA): the headphones chart shows the correction that was applied

Work in `E:/Impulcifer` on the checked-out branch (do not switch branches, do not commit, do not stash). Read `E:/Impulcifer/CLAUDE.md` (section "3.x Rust 워크스페이스"), `E:/Impulcifer/docs/rust/PLOTS.md` and the P23 packet `E:/Impulcifer/docs/rust/packets/P23-astra-plot-redesign.md` (the visual system: tokens, grammar, sizes) first. Rules: `#![forbid(unsafe_code)]`, no new dependencies, run every command in the foreground and never in the background, do not query or terminate processes, do not touch `features.toml`, `CHANGELOG.md`, the service crate, the Python tree or `apps/`.

**A second worker (Daybreak, packet DB02) edits `crates/impulcifer-dsp/**`, `crates/impulcifer-service/**`, `crates/impulcifer-audio-io/**`, `crates/impulcifer-cli/**`, `crates/impulcifer-python/**`, `crates/impulcifer-policy/**` in the same tree at the same time.** Never open those for writing. Its service change fills `FrCurve.equalization` for the two headphone curves after the equalize stage; your chart draws it. The contract between you is only the existing `FrCurve` struct and the unchanged `plot_headphones` signature.

## What the chart is for

`plots/headphones.png` (`plot_headphones` in `crates/impulcifer-plots/src/charts.rs`) shows the user's own headphone measurement against the flat target. P23 left a hole on purpose: "The applied correction curve can be added once the pipeline passes it to the plot stage; the unlimited inverse of headphone error is not an applied filter." The pipeline now passes it. The curve in `FrCurve.equalization` is the limited correction Impulcifer actually turned into the FIR for the front-left speaker's left ear and the front-right speaker's right ear (the same measurement positions the headphone compensation came from). It is on the same grid as `frequency`.

## Design direction (fixed; design within it)

- `plot_headphones(path, left, right, gain_left_db, gain_right_db)` keeps its signature. When both `left.equalization` and `right.equalization` are empty the chart is exactly what it is today (title `Your headphones as measured`, the same subtitle, the same two footer lines, no purple anywhere). When they are present (non-empty, same length as `frequency`; validate like `validate_curve` and return `PlotError::Invalid` when only one ear has one or the lengths differ):
  - title `Your headphones and the correction applied`; subtitle in the P23 voice, one sentence, saying the bold lines are the measurement, the grey dashes the target, and the purple dashes the equalization Impulcifer applied to the front speakers.
  - the correction lines use the `correction` token (`#7c3aed`), left with `Dash::Long`, right with `Dash::Dots`, the split-ear grammar of the equalization chart (`correction_lines`); legend names `Correction applied · left` and `Correction applied · right`.
  - the panel's dB range is the union of the existing `headphones_limits` result and the correction's padded range (whole multiples of 6 dB, as `padded_range` does). `headphones_limits` itself must not change: the service golden `golden_headphones_series_match_python` pins its output.
  - a third footer line, computed from the supplied correction over 40 Hz–16 kHz inclusive, both ears together: `Correction applied 40 Hz–16 kHz: −6.2 … +4.8 dB, largest −6.2 dB at 3.4 kHz` (range as min … max with signs, then the largest absolute value with its sign and frequency, one decimal, the frequency formatted as the existing footers format frequencies). `unavailable` when the band holds no samples.
- Sizes, margins, band strip, fonts, colours: unchanged (1600×1000). No new tokens.

## Files
`crates/impulcifer-plots/src/charts.rs`, `crates/impulcifer-plots/src/lib.rs` (only if a helper must move), `crates/impulcifer-plots/tests/render.rs`, `docs/rust/PLOTS.md` (rewrite the Headphones bullet: what is shown with and without the correction, the footer, the fact that the curve is the front pair's applied FIR curve handed over by the service after the equalize stage).

## Tests (`crates/impulcifer-plots/tests/render.rs`)
- Keep the existing token test as it is for the case without correction: `headphones` with empty `equalization` must not contain the purple token (`headphones must never show an inverse correction`).
- Add `headphones_chart_draws_the_applied_correction_when_supplied`: build the two curves as the existing tests do, fill `equalization` on both (different shapes for left and right so both dash styles are exercised), render, decode, and assert the purple token is present, the blue/red/target tokens are still present, the size is 1600×1000 and `png_is_not_blank` holds; assert the mismatch cases (only one ear, wrong length) return `PlotError::Invalid`.
- Add `headphones_correction_footer_reports_range_and_largest_value`: if the footer text is not reachable from the PNG, expose the footer function `pub` (or `pub(crate)` with a unit test in `charts.rs`) and pin the three cases: a curve with a clear minimum and maximum inside 40 Hz–16 kHz, a curve whose extremes lie outside that band and must be ignored, and an empty band giving `unavailable`.
- `png_outputs_have_p23_sizes`, `chart_kinds_use_their_actual_tokens` and every other existing test stay green.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
cargo fmt -p impulcifer-plots -- --check
cargo clippy -p impulcifer-plots --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-plots
git status --porcelain
```
Also render one example each way to `E:/Impulcifer/target/a02/` (a small `#[test]`-free helper is not needed: reuse the test's curves through `cargo test -p impulcifer-plots -- --nocapture` and copy the two PNGs from the temp dir the tests use, or write them from the new test with `IMPULCIFER_A02_OUT` set) so the caller can look at them; give the two paths in the report.

## Report format
(1) the titles, subtitle and footer wording as landed; (2) the y-range rule and the proof that `headphones_limits` did not change; (3) the new test names with one line each; (4) the pasted `test result:` lines and `git status --porcelain`; (5) the two PNG paths; (6) anything undone. Do not end your turn before the commands complete.
