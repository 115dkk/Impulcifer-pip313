# Static plots (P23)

The PNG renderer is a paper-light reading view, not a matplotlib screenshot replica.
It borrows the service's prepared arrays and never mutates the pipeline.
Only the reflection display derives an envelope and peaks inside the plots crate.
Existing public types, function signatures, filenames, `padded_range`, and
`headphones_limits` remain available. `PanelLimits::synchronize` retains its exact
union semantics. PNG output uses the bundled DejaVu Sans and has no system-font lookup.

## Tokens and anatomy

`crates/impulcifer-plots/src/theme.rs` is the sole palette and geometry definition.

| Token | Value |
|---|---|
| Canvas / panel | `#fbfbfc` / `#ffffff` |
| Ink / muted text | `#1b1f24` / `#6b7280` |
| Major / minor grid | `#e5e7eb` / `#f1f3f5` |
| Zero / target | `#9ca3af` |
| Left / right ear | `#2563eb` / `#dc2626` |
| Both ears / difference | `#111827` |
| Correction | `#7c3aed` |
| Guide / positive / negative fill | `#fef3c7` / `#dbeafe` / `#fee2e2` |
| Raw / smoothed curves | 35% alpha, 1 px / opaque, 3 px |
| Title / subtitle / axis title | 34 / 22 / 22 px |
| Tick / legend / annotation | 18 / 20 / 20 px |
| Outer margin / panel gap | 64 / 48 px |
| Single / six-panel / overlay PNG | 1600×1000 / 2400×1350 / 1600×900 |

Annotations use white rounded labels with a one-pixel major-grid border. Measured
boxes use a 16 px inset and flip anchors at edges. Overlay peak labels start on
opposite sides below their peaks; intersecting boxes stack vertically with leaders.
Reflection labels occupy a reserved right column, outside the curve area, in time
order with thin leaders. Text
always uses ink or muted text, not categorical colors. The shared helpers in
`src/drawing.rs` measure and wrap titles and legends, map coordinates, draw
consistent dash lengths, and draw axes and signed fills. Frequency tick labels
alternate rows where needed so the 18 px text does not collide. Level ticks use 6 dB for spans through 48 dB, 12 dB through 96 dB, and
24 dB beyond. Zero remains explicit; no extra near-zero tick is squeezed in.

The six band keys are Sub-bass 20–60, Bass 60–250, Low mids 250–500,
Mids 500–2k, Presence 2k–6k, and Air 6k–20k. Their strip uses equal-width
**label cells**, not a claim of equal frequency widths. Frequency data uses a
log axis with ticks at 20, 50, 100, 200, 500, 1k, 2k, 5k, 10k, and 20k Hz.
Level bounds are padded and rounded outward to multiples of 6 dB. Time is in
milliseconds, except the recorded sweep in seconds. Waveform amplitude is in
full-scale sample units (FS), with three significant digits for amplitude summaries.
Linear tick **steps** are 1, 2, or 5 times a power of ten; bounds round outward to
cover the supplied range. Tick precision follows the step, including scientific
notation, so distinct ticks remain distinct and nonzero ticks never print as zero.
The overlay retains its fixed 0.5 ms step. Every Y axis measures every actual tick
string with the embedded font before reserving its label column; the rotated title
sits outside that column with token padding. This includes dB axes, the spectrogram
frequency axis, and the full-response inset. A padded gap separates the band strip
from the upper ticks.

Left is solid; right is short-dashed. Correction uses long dashes (left/common)
and dots (right); target uses gray long dashes. Legends reproduce the actual dash pattern and use plain names.
The fixed categorical colors fail the all-pairs palette validator: blue/purple
has deutan ΔE 0.4 and normal-vision ΔE 12.4. Owner tokens are unchanged. Dashes,
legends, peak labels and numerical footers supplement color; this is not a claim
that the palette itself passes the color-separation gate.

## Layouts and summaries

- **Results** uses two stacked panels. The top compares raw and smoothed ears.
  The bottom shows smoothed left minus right, blue fill above zero and red below,
  with a ±3 dB guide. The footer uses the arithmetic mean of supplied frequency
  samples from 200 Hz through 8 kHz, inclusive, and the largest absolute signed
  difference from 40 Hz through 16 kHz (inclusive). The latter is directly annotated.
- **Headphones** without supplied correction is unchanged: title
  `Your headphones as measured`, subtitle
  `Impulcifer flattens this in the final equalization; the flatter the bold line, the less it has to do.`,
  raw faint, smoothed thick, target dashed gray, two footer lines, and no purple.
  With correction for both ears, the title is
  `Your headphones and the correction applied` and the subtitle is
  `Bold lines show the measurement, grey dashes the target, and purple dashes the equalization Impulcifer applied to the front speakers.`
  Purple long dashes are `Correction applied · left`; purple dots are
  `Correction applied · right`. These are the front pair's limited, applied FIR
  correction curves (front-left speaker's left ear and front-right speaker's right
  ear), handed over by the service in `FrCurve.equalization` after the equalize
  stage, on the measurement frequency grid. They are not an unlimited inverse
  manufactured by the renderer. Both must be present with matching finite arrays;
  one missing ear or a wrong length returns `PlotError::Invalid`.
  The correction view unions the unchanged `headphones_limits` result with the
  padded correction range across both ears, rounding outward to multiples of 6 dB.
  The service smooths a measurement clone with 1/3 and 1/5 octave windows,
  transitioning at 20,000–23,999 Hz. Raw samples and target remain unchanged,
  preserving the headphone raw golden. The first footer computes each ear's signed
  arithmetic mean of smoothed minus target samples from 100 Hz through 10 kHz,
  inclusive. The second selects the greatest absolute deviation across both ears
  from 40 Hz through 16 kHz, inclusive, retaining its sign and frequency. Raw is
  used only when smoothing is absent; missing targets yield `unavailable`. With
  correction, a third footer reports the supplied samples' signed minimum, maximum,
  and largest absolute value across both ears in 40 Hz–16 kHz, inclusive:
  `Correction applied 40 Hz–16 kHz: −6.2 … +4.8 dB, largest −6.2 dB at 3.4 kHz`.
  No samples in that band yields
  `Correction applied 40 Hz–16 kHz: unavailable (no samples in band)`.
  Legacy gain arguments remain accepted for API compatibility but do not supply
  summaries. Size, margins, band strip, fonts, and tokens are unchanged.
- **Equalization** uses the same grammar, with a single common panel or left/right
  panels on a shared dB scale. A missing ear has an empty-state explanation. Both
  inputs absent still means no EQ file, preserving the service PNG-set contract.
- **Room** compares thin measurements, bold smoothed response, and a dashed target.
  Target-relative error is a faint signed fill. Peak/dip footers use supplied
  smoothed error, or the difference of supplied smoothed and target arrays. A
  missing error/target is reported as unavailable, not as zero. All extrema and
  response-range summaries use 40 Hz–16 kHz and name that band.
- **Six-panel sheets** are two rows of three, ordered recording, impulse, decay,
  spectrogram, frequency response, and first reflections. The impulse highlights the first
  30 ms over the faint full response. The decay panel plots supplied squared and
  moving-average series. `plot_ir_panels_with_noise_floor` adds the service's already
  computed Lundeby floor without changing `IrPanels` or `plot_ir_panels`. It locates
  the first downward crossing after the average's maximum, linearly interpolates
  the crossing time, and labels it in whole milliseconds. If no crossing exists it says so. Existing
  callers without the floor receive an explicit unavailable statement.
- **Spectrogram** uses the existing frequency-major matrix, log frequency, time in
  ms, bundled `magma.rgb`, and a fixed −80..0 dB color bar. Bin edges come from
  neighboring sample centers. Values are clipped only for color mapping. The `dB`
  title sits below the color bar with the theme's 48 px panel gap, away from the
  band strip. The frequency tick column uses the same measured layout helper.
- **First reflections** uses subtitle `Direct sound at 0 ms; the echoes that follow, relative to it.`
  and only the IR, never the sweep spectrogram. A 0.2 ms
  peak-hold envelope of absolute amplitude bridges carrier zero crossings. The
  first substantial local peak (at least 0.12589 of global maximum), or the
  midpoint of its plateau, defines time zero and the dB reference. The held
  direct peak and its falling envelope form the direct lobe; a new rise after
  that fall can qualify at any later time, including below 1 ms. No fixed
  direct-search window absorbs separate early echoes. The panel spans 0–25 ms.
  Candidates must exceed −30 dB re direct and have more than 6 dB prominence
  on both sides within 1 ms. Up to three strongest candidates, separated by at
  least 0.5 ms, are labelled in time order. The footer independently reports the
  earliest qualifying echo before selecting those three labels, even when the
  first echo is weaker than all three labelled echoes. These are display heuristics, not a new acoustic measurement. No eligible
  echo produces the explicit no-echo footer. The supplied global-peak-relative
  Lundeby floor is converted to the direct reference; callers without it get an
  explicitly labelled RMS estimate from the final 10% of the IR. Silence/empty
  IR produces an empty state. Legacy waterfall data remains accepted but unused.
  Sheet titles expand every speaker code from `core/constants.py`, plus X and
  LFE, with the ear and pre/post/room stage inferred from the unchanged PNG path.
- **Interaural overlay** uses one origin, the earlier of the two supplied peak
  indices, and a fixed −1..+5 ms detail window with 0.5 ms ticks. No peak is recomputed. A horizontal
  arrow and labels identify the delay, with a faint full-response inset. The delay
  uses two decimals as in the packet's 0.31 ms example; other summaries use one.

Missing optional data produces meaningful empty states. Invalid frequency grids,
array lengths, nonfinite values, matrix shapes, sample rates and overlay peaks
return `PlotError::Invalid`. File errors remain errors. Service cancellation checks
and synchronization calls are unchanged. The existing six-slot limits still retain
P19 meanings for compatibility; slot 5's legacy waterfall limits cannot be used as
time/dB limits for the replacement direct-relative reflection chart.

## Adding a chart

1. Keep service computations and public data contracts separate from rendering.
2. Add a chart composition in `src/charts.rs`, using the shared helpers and tokens.
   Use an existing size; include a plain title, explanatory subtitle, units, legend
   and summaries derived from the displayed samples. Do not manufacture missing data.
3. Validate arrays before mapping coordinates. Use the established cancellation and
   synchronized-axis preparation when rendering batches.
4. Add a synthetic render test for file existence, exact size, canvas and title ink,
   plus optional-data and malformed-input coverage. Add it to the ignored `gallery`
   test. Keep old gallery artifacts or select a fresh output directory.
5. Run fmt, focused clippy, plot tests, ignored gallery, unchanged service plot tests
   and policy tests. Review the synthetic and temporary-copy demo PNGs visually.

`cargo test -p impulcifer-plots --test render -- --ignored gallery` writes under
`target/plots-gallery/synthetic`; if occupied, it creates a unique run subdirectory.
For real input, copy `data/demo` to a unique temporary directory and run
`cargo run -p impulcifer-cli --release -- --dir_path <copy> --plot` (an explicit
`--test_signal` may select the bundled sweep). Copy the freshly generated `plots`
tree to `target/plots-gallery/demo`. Never run the output-producing CLI on source data.

The policy registry still names `png_outputs_have_matplotlib_sizes`; P23 uses
`png_outputs_have_p23_sizes`. Updating `features.toml` remains outside scope.
