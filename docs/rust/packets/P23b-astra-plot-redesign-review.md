# P23b (ASTRA): plot redesign, second pass after the owner's review of the gallery

Continue the uncommitted P23 work in `E:/Impulcifer` (`git status` shows the P23 files; keep them, do not reset). Same rules as `P23-astra-plot-redesign.md`. The owner looked at `target/plots-gallery/demo/{results,headphones}.png`, `post/FL-left.png`, `room/FL-left.png` and `interaural_overlay/FL_interaural_overlay.png`; the direction is right, the items below are what still stands between the pictures and a reader who is not an engineer. Fix all of them.

## Findings
1. **Level axes are too dense.** `results.png` shows `6 / 0 / -6` crammed at the top of the upper panel. Rule: 6 dB ticks while the range is up to 48 dB, 12 dB up to 96 dB, 24 dB beyond; the zero line stays. Apply to every level axis (the decay panel spans 200 dB and gets 24 dB ticks).
2. **Numbers must be in the meaningful band.** `Largest difference: +9.4 dB at 19.8 kHz` reports noise at the band edge. Every peak/dip/largest-difference footer searches 40 Hz to 16 kHz only and says so (`Largest difference (40 Hz–16 kHz): …`); the average keeps 200 Hz–8 kHz. The room sheet's `Largest dip: −30.7 dB at 20.1 Hz` is the same problem.
3. **The headphones chart must show the correction.** The service passes the raw curves only. In `crates/impulcifer-service/src/brir/plots.rs::headphones` pass the `FrequencyResponse` values the pipeline already computed at the headphone-compensation stage (smoothed, `equalization`, `target`) into the `FrCurve`; if the objects at that point do not carry them, compute `smoothen` on a clone inside the service with the same window parameters the 2.x headphone plot used (`core/plotting/hrir_plotter.py`, `plot_headphones`) and pass the equalization from the compensation result. The picture then shows smoothed thick, raw faint, correction dashed purple, target dashed grey, and the footer `Correction range: −6.2 … +4.8 dB` from real numbers. The `headphones raw` golden comparison in `crates/impulcifer-service/tests/brir_plots.rs` must keep passing unchanged.
4. **Small numbers are rounded to nothing.** `Peak amplitude: 0.0 FS` and impulse-response tick labels reading `0.00 0.00 0.00`. Format amplitudes with three significant digits (`0.0138 FS`) and, where the axis range is below 0.1, use significant digits for tick labels too (or dBFS: `−37 dBFS`). Never print a value that rounds to zero when the data is not zero.
5. **The "Decay by band" panel is wrong.** It is computed from the spectrogram of the *recorded sweep*, so it shows the sweep passing through the bands, not the room decaying. Replace panel 6 with **"First reflections: the direct sound and the echoes that follow"**: the envelope of |IR| in dB relative to the direct-sound peak over 0 … 25 ms (peak at 0 ms), the noise floor as a dashed line, and the three strongest later peaks that rise more than 6 dB above their surroundings and above −30 dB re direct, each labelled `+3.2 ms, −12 dB`. Envelope and peak picking live in the plots crate on the IR the sheet already has; no new DSP elsewhere. Footer: `First echo: +3.2 ms (−12 dB)` or `No echo above −30 dB within 25 ms`. Drop the band-decay code and its dB-axis synchronisation problem with it.
6. **Sheet titles in plain words.** `FL-left` becomes `Front left speaker → left ear · after processing` (`pre` sheets: `· as recorded`, room sheets: `· room correction`). Speaker codes: FL Front left, FR Front right, FC Center, BL Back left, BR Back right, SL Side left, SR Side right, X reference microphone (check `core/constants.py` for the full list). Keep the file names exactly as they are.
7. **Overlay wording and ticks.** Replace `Supplied peak times: left 1.0 ms; right 1.2 ms` with `Direct sound reaches the left ear at 1.0 ms and the right ear at 1.2 ms`. Time ticks at 0.5 ms steps (−1, −0.5, 0, 0.5 …), not −1.0/0.2/1.4.
8. **Labels must not touch the frame.** `Tail length ≈ 650.3 ms` overlaps the panel's top-right frame; anchor annotation boxes inside the plot area with the 16 px margin from `theme.rs`, flipping to the other side when they would leave the panel. Tail length with no decimals (`≈ 650 ms`).
9. **Size expectations in the service tests.** Update the three PNG size assertions in `crates/impulcifer-service/tests/brir_plots.rs` (lines near 301, 386, 430) to the P23 sizes; change nothing else in that file (the series goldens stay).
10. **Colour distinctness test.** With panel 6 replaced, re-run the fixed-colour check; if blue/purple still fail it, keep the tokens and make the test assert the tokens that actually appear in each chart kind.

## Allowed files
Those of P23 plus `crates/impulcifer-service/tests/brir_plots.rs` (size numbers only) and `crates/impulcifer-service/src/brir/plots.rs` (headphones data, finding 3). Not `features.toml` (the owner renames the registry test reference), not `CHANGELOG.md`.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
cargo fmt --all -- --check
cargo clippy -p impulcifer-plots -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-plots
cargo test -p impulcifer-plots --test render -- --ignored gallery
cargo test -p impulcifer-service --test brir_plots
git status --porcelain
```
Re-render the demo gallery exactly as in P23 (temp copy of `data/demo`, release CLI with plots, copy `plots/` to `target/plots-gallery/demo/`, overwriting the previous one).

## Report format
(1) one line per finding with what changed; (2) the pasted `test result:` lines and `git status --porcelain`; (3) the demo gallery paths; (4) anything undone and why. Do not end your turn before the commands complete.
