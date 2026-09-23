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

# Interactive report (P24)

`--interactive_plots` writes `interactive_plots/interactive_summary.html`, the 3.x
counterpart of the 2.x Bokeh summary (`core/pipeline.py::_stage_interactive_plots`).
With `--plot`, the ILD, IPD, IACC and EDC panels are also written one per file as
`plots/{ild,ipd,iacc,etc}/<name>_analysis.html`, the 2.x
`_save_bokeh_analysis_plots` names. Both are side outputs: the WAVs and README are
byte-identical with and without them (`interactive_report_leaves_wavs_and_pngs_unchanged`).
The pipeline hands the HRIR to `StageObserver::on_plot` at the 2.x position, after
`plot_additional` and before `resample`, so a `--fs` run still shows the measured rate.

## Numbers

`impulcifer-analysis::model` ports `core/plotting/analysis.py` (`octave_bands`,
`band_interaural_level_difference`, `band_interaural_phase_difference`,
`energy_decay_curve_db`, `interaural_cross_correlation`) and the data half of the
Bokeh generators (`build_report`, `result_overview`). It borrows the HRIR and does
no I/O. Details that matter:

- The band spectra pad to `scipy.fft.next_fast_len(n)` with SciPy's default
  `real=False`, the 2,3,5,7,11-smooth size (`fft::next_fast_len_complex`), not the
  `real=True` size the rest of the port uses. ILD and IPD share one transform pair.
- IACF is SciPy's `correlate(left, right, "full")` evaluated directly at the lags
  inside `round(1 ms * fs)` (a later right ear peaks at a negative lag).
- The result overview smooths with `1/3, 1/5, 20000, max(20001, int(fs/2 - 1))`,
  unlike the PNG results chart's fixed 23999 (identical at 48 kHz).
- As in 2.x: overlay, ILD, IPD and IACC need both ears; EDC takes either ear; NaN
  bands are dropped from the bars; silent pairs get no IACC chart; unequal response
  lengths lose only the result overview (`cli_warning_interactive_plot_error`); no
  panel at all writes nothing (`cli_warning_no_interactive`).
- Charts follow the canonical speaker order (FL, FR, FC, BL, BR, SL, SR, then
  others), like README.md; 2.x used file-read order.

`tests/migration/export_goldens_interactive.py` calls the unmodified 2.x functions
and generators on deterministic signals (impulses, pure delays, level differences,
polarity inversion, silence, sub-1e-12 energy, unequal and prime lengths, 20-sample
and 1-sample responses, 8/44.1/48 kHz; noise from `default_rng(1234)`) and writes
five `p24_interactive_*.json` files under 200 kB. Measured against them: ILD
2.6e-14 dB, IPD 6.3e-13° (compared on the circle, because a cross spectrum on the
negative real axis may land on either side of ±180°), IACF 2.4e-15, EDC 7.1e-15 dB,
overview 1.7e-12 dB; band edges bit-exact, lags, FFT lengths, labels and chart order
exact. The gates are 1e-9 (dB, degrees), 1e-12 (IACF) and the P09
`1e-9 + 1e-11·|ref|` dB for the overview.

## Document

`impulcifer-analysis::html` writes one self-contained file: inline CSS
(`assets/report.css`), an inline vanilla-JS renderer (`assets/report.js`, passes
`tsc --checkJs --strict`), a small JSON manifest (tabs, chart specs, bar values;
`<`, `>` and `&` escaped) and one `<script type="application/octet-stream">` per tab
holding base64 little-endian `f32` samples. Series address the blob by
`[offset, length]`; evenly sampled series carry `x = (i + start) * step` instead of
an x array, and the overview's five curves share one frequency grid. The model
stays `f64`; the page rounds to `f32` once (7 significant digits). There is no
`http(s)://`, `src=`, `<link>`, `@import`, `url(` or `fetch` in the document
(`interactive_summary_is_offline_and_complete`), and the fonts are the system UI
stack. The page `<title>` is `Interactive Plot Summary`; single-panel pages use the
registry title (`ILD`, `IPD`, `IACC`, `EDC`). Report text is English, like 2.x.

| Tab | Charts | Default view |
|---|---|---|
| Interaural Overlay | per speaker, left solid / right dashed, peak markers, right-minus-left peak delay | −5…30 ms, level fitted to that window |
| ILD | per speaker, octave bars labelled by centre, 2.x band string on hover | 0 dB included |
| IPD | per speaker, octave bars | −180…180°, 45° ticks |
| IACC | per speaker, IACF with the 2.x `Max: x.xx at y.yyms` legend and a peak marker | ±1.1 ms |
| EDC | per speaker, left solid / right dashed | 0…200 ms, −80…0 dB |
| Result Overview | raw (35 %) and smoothed ears; the smoothed L−R difference below with signed fill and ±3 dB guides; x linked | 20 Hz…20 kHz, log |

Colours are the P23 tokens (left `#2563eb`, right `#dc2626`, difference `#111827`,
positive/negative fills `#dbeafe`/`#fee2e2`, grid `#e5e7eb`, muted `#6b7280`) with a
`prefers-color-scheme: dark` set (left `#60a5fa`, right `#f87171`, ink `#e6e8eb` on
`#161a21` panels). Two columns from 900 px, one below; the tab bar scrolls on its
own and the page never scrolls sideways.

## Performance design

2.x embeds every array as JSON-described float64 columns (time and value for each
line), lays everything out with `sizing_mode="scale_both"`, and redraws every glyph
of a plot on each pan or zoom step. The 3.x page instead:

1. **Decodes lazily.** Only the opened tab's blob is decoded (`Uint8Array.fromBase64`
   where available, `atob` otherwise) and its DOM built; the source text is then
   removed. Opening the report touches the overlay payload only.
2. **Decimates per pixel column.** Each redraw walks the visible index range once and
   keeps first, minimum, maximum and last per device-pixel column (M4), so a path
   has at most four vertices per column however long the response. Evenly sampled
   series on a linear axis map index to pixel with one multiply-add.
3. **Coalesces frames.** Wheel, drag and keyboard input only update the view; one
   `requestAnimationFrame` redraws every dirty chart. The hover crosshair, dots and
   readout live on a second canvas, so moving the pointer never redraws data.
4. **Skips what is not seen.** `IntersectionObserver` (200 px margin) keeps
   off-screen charts from drawing until they scroll in (and `beforeprint` draws them
   all); `ResizeObserver` and `devicePixelRatio` (capped at 3) size the canvases.
5. **Bounds navigation.** Zoom-out and panning stop 5 % beyond the data (x); linked
   overview charts share their x range.

Interaction: drag pans, wheel zooms time or frequency around the pointer,
Shift/Ctrl + wheel zooms the level axis, double-click resets, legend entries toggle
curves, focused charts take arrows, `+`/`−` and `0`, tabs take arrows/Home/End and
remember themselves in the URL hash. There is no Bokeh toolbar (box zoom, save).

## Measurements (demo, 2026-09-23)

Same Linux container, headless Chromium 141 (Playwright 1.56, 1440×900, DPR 1).
The 2.x page is the `--interactive_plots --plot` output of 2.14.2 (this tree, Python 3.13) on a copy of `data/demo`;
its CDN script was served from the installed `bokeh` 3.9.2 package so neither page
touched the network. Interaction: the first overlay chart zoomed out to the whole
700 ms response (both ears, 33 599 samples each), then 60 wheel steps and a 60-step
drag, each followed by two animation frames.

| | 2.x Bokeh | 3.x report |
|---|---:|---:|
| `interactive_summary.html` | 11.33 MB + 1.27 MB BokehJS (CDN) | 5.12 MB, self-contained |
| `plots/etc/etc_analysis.html` | 5.06 MB | 2.56 MB |
| `plots/{ild,ipd,iacc}` pages | 39–43 kB (+ CDN) | 53–58 kB |
| First charts on screen | 4.41 s | 0.26–0.40 s |
| Wheel zoom: input to second frame, median / p95 | 199 / 358 ms | 66.7 / 68.9 ms |
| Wheel zoom: longest frame; long tasks | 817 ms; 128 (10.8 s) | 16.8 ms; 0 |
| Drag pan: median frame interval; long tasks | 41.7 ms; 316 (22.2 s) | 16.7 ms; 0 |
| JS heap after interaction | 115 MB | 6.4 MB |

The 3.x input-to-second-frame time is two 16.7 ms frames plus the harness's
round trips; no frame was dropped. One full-length overlay redraw took 1.54 ms of
script on average; decoding the overlay tab's 1.9 MB of samples took 6.7 ms. On
the Rust side the stage adds about 50 ms to a 1.75 s release demo run
(`build_report` 36 ms, `summary_html` 15 ms, one thread). Phone width (390 px)
gives one column and no horizontal scroll. The renderer passes
`tsc --checkJs --strict` (TypeScript 5.9.3).

## Changing the report

Numbers belong in `model.rs` with a 2.x oracle (`export_goldens_interactive.py`);
`html.rs` only arranges them. A chart is a manifest entry (`kind` line or bar,
axis labels, units, default view, series `[offset, length]` into the tab blob)
and needs no JavaScript unless it introduces a new mark. Keep the page offline
(`interactive_summary_is_offline_and_complete` rejects network references), keep
series in the blob rather than in JSON, and check both themes and a narrow window.
