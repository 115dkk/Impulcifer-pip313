# P23c (ASTRA): plot redesign, third pass (label collisions, nice ticks, the headphones chart)

Continue the uncommitted P23/P23b work in `E:/Impulcifer` (keep it; do not reset). Same rules and allowed files as `P23b-astra-plot-redesign-review.md`. The owner reviewed the second gallery; the layout and wording are now right, what remains is collisions and one chart that says something misleading. Fix all six.

1. **Amplitude axes: nice ticks and a label column that fits.** Tick labels such as `0.0468`, `-0.00727`, `-0.00314` are printed over the axis title `Amplitude (FS)` (recorded sweep panel, impulse response panel, the overlay). Two rules: (a) ticks are placed at round values (1, 2 or 5 times a power of ten) that span the data range, so an axis reads `0.02 0.01 0 -0.01 -0.02`, never `0.0244 0.0118 -0.00727`; (b) the label column is measured from the widest tick label of that axis and the axis title is placed outside it, so nothing overlaps whatever the digits are. Apply to every axis, including the inset in the overlay.
2. **Headphones chart: drop the "requested correction" curves.** The unsmoothed, unlimited inverse (`-7.6 … +34.1 dB`) is not what is applied and misleads. Show the measured headphones only: raw faint, smoothed thick, the target dashed grey; title `Your headphones as measured`, subtitle `Impulcifer flattens this in the final equalization; the flatter the bold line, the less it has to do.`; footer `Average deviation from target 100 Hz–10 kHz: left +0.0 dB, right +1.5 dB` and `Largest deviation (40 Hz–16 kHz): +7.2 dB at 4.9 kHz`. Remove the equalization/error plumbing added in P23b if nothing else uses it; the `headphones raw` golden stays. Note in `docs/rust/PLOTS.md` that the applied correction curve can be added once the pipeline passes it to the plot stage.
3. **Overlay peak labels collide.** `Left peak` is hidden behind `Right peak`. Place the two labels on opposite sides of their peaks (left label to the left and below, right label to the right and below) and, if their boxes would still intersect, stack them vertically with leader lines.
4. **First-reflections annotations collide.** `+3.2 ms, −10 dB`, `+4.0 ms, −10 dB` and `+6.4 ms, −12 dB` are printed over each other and over the curve. Stack the annotation boxes in a column at the right side of the panel (top to bottom in time order) with thin leader lines to their points; shorten the panel title to `First reflections` and move the explanation to the subtitle `Direct sound at 0 ms; the echoes that follow, relative to it.` so the title fits one line.
5. **Spectrogram colour bar title collides with the band strip.** `dB` is printed over `AIR 6k–20k`. Put the colour bar title under the bar (or as `Level (dB)` beside it) with the panel gap from the theme, and keep the band strip clear of it.
6. **Re-check every panel for text over text or text over the frame** after the fixes (results, headphones, eq single and split, room, both sheet kinds, overlay), then re-render the demo gallery and the synthetic gallery. Update `docs/rust/PLOTS.md` for the tick rule and the label column.

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
Re-render `target/plots-gallery/demo/` as before. Report: one line per item, the `test result:` lines, `git status --porcelain`, anything undone. Do not end your turn before the commands complete.
