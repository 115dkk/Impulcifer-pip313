# P23 (ASTRA): plots for people, not for matplotlib

The 3.x plots (`crates/impulcifer-plots`, P19) reproduce the 2.x matplotlib figures: same panels, same sizes, same colours. The owner decided on 2026-09-08 that the pictures do not have to match Python; they have to be readable by someone who is not an audio engineer. This packet replaces the rendering with a designed visual system. The numbers behind the pictures do not change: the service still prepares the same series and panels (`crates/impulcifer-service/src/brir/plots.rs`), the golden series tests in `crates/impulcifer-service/tests/brir_plots.rs` stay exactly as they are, the PNG file names and directories stay, cancellation checks stay.

Work in `E:/Impulcifer` (read `CLAUDE.md`, section "3.x Rust 워크스페이스", and `docs/rust/ARCHITECTURE.md` first). Rules: `#![forbid(unsafe_code)]`, no new runtime dependencies beyond `plotters` (already present) unless a font or colormap asset is added under `crates/impulcifer-plots/assets/` with its licence file; run every command in the foreground and never in the background; do not query or terminate processes; do not touch `features.toml`, `CHANGELOG.md`, the Python tree or `webview_ui/`.

## Design direction (fixed; do not redesign the direction, do design within it)

**Tone.** Paper, not terminal. One light style for every picture (the PNGs are opened in Explorer, chat apps and forums, not only inside the app). Big type, few lines, one message per chart, a sentence that says what the reader is looking at, and the two or three numbers that matter written on the picture.

**Tokens** (define them once in `crates/impulcifer-plots/src/theme.rs` and use nothing else):
- canvas `#fbfbfc`, panel `#ffffff`, ink `#1b1f24`, muted text `#6b7280`, grid major `#e5e7eb`, grid minor `#f1f3f5`, zero line `#9ca3af`
- left ear `#2563eb`, right ear `#dc2626`, both/sum `#111827`, target/reference `#9ca3af` (dashed), correction `#7c3aed`, guide band fill `#fef3c7`, positive fill `#dbeafe`, negative fill `#fee2e2`
- raw curves: the ear colour at 35 percent alpha, 1 px; smoothed curves: the ear colour, 3 px
- spectrogram colormap: the bundled `magma.rgb`, dB range clipped to −80..0
- type: the bundled DejaVu Sans; title 34 px, subtitle 22 px muted, axis title 22 px, tick labels 18 px, legend 20 px, annotation 20 px on a white rounded label with a 1 px `grid major` border
- sizes: single charts 1600×1000; the six-panel sheet 2400×1350 (2 rows × 3); the interaural overlay 1600×900. Margins 64 px, panel gap 48 px.

**Axes.** Frequency axes are logarithmic with ticks at 20, 50, 100, 200, 500, 1k, 2k, 5k, 10k, 20k, labelled `20 Hz … 20 kHz`; along the top edge of every frequency chart a muted band strip: `Sub-bass 20–60`, `Bass 60–250`, `Low mids 250–500`, `Mids 500–2k`, `Presence 2k–6k`, `Air 6k–20k` (thin separators, small caps). Level axes in dB with major ticks every 6 dB, the zero line drawn in `zero line`, the range from the data padded to whole multiples of 6 dB. Time axes in ms (or s for the sweep recording). Every axis has a title with a unit.

**Every chart** has: a title in plain words, a one-line subtitle (what this shows), a legend with plain names (`Left ear`, `Right ear`, `Left minus right`, `Target`, `Correction applied`), and a footer line with the key numbers computed from the plotted series (see per-chart lists). Numbers are rounded to one decimal, signed where sign matters, with units.

## Per-chart specification

1. `plots/results.png` (`plot_results`): two stacked panels. Top: "How each ear hears the room": smoothed left and right (thick), raw (faint). Bottom: "Balance: left minus right": the difference of the smoothed curves as a filled area (blue above zero, red below) with a ±3 dB guide band; footer: `Average level difference 200 Hz–8 kHz: +0.4 dB (left louder)` and `Largest difference: +2.1 dB at 3.4 kHz`.
2. `plots/headphones.png` (`plot_headphones`): "Your headphones and the correction applied": both ears' smoothed responses, raw faint, the correction curve dashed in `correction`, the target dashed grey; footer: `Correction range: −6.2 … +4.8 dB`.
3. `plots/eq.png` (`plot_eq`): "Equalization": same grammar as 2 for the single and the split (left/right) forms.
4. `plots/room/*.png` and `room.png` (`plot_generic_room` and the room panels): "Room: measured against the target": measured thin, smoothed thick, target dashed, error as a faint filled area; footer: `Largest room peak: +6.2 dB at 120 Hz`, `Largest dip: −4.0 dB at 210 Hz`.
5. The six-panel sheet (`plot_ir_panels`, pre/post per speaker and side): (1) "Test sweep as recorded" waveform in s, subdued; (2) "Impulse response" in ms, first 30 ms emphasised with the full length faint behind it; (3) "Decay: how fast the room's sound dies away": the existing decay series in dB over ms with the noise-floor line and a `Tail length ≈ 210 ms` label at the point where the average decay reaches the floor; (4) "Sound over time by frequency": the spectrogram with the magma colormap, log frequency axis, dB colour bar; (5) "Frequency response" smoothed thick, raw faint (room correction sheets use target/smoothed/error as today); (6) "Decay by band": replace the waterfall with three decay lines derived from the spectrogram matrix already in `IrPanels` (mean dB over time in `Bass 20–250`, `Mids 250–2k`, `Treble 2k–20k`), no new DSP. Keep `PanelLimits::synchronize` so pre and post sheets share axes.
6. `plots/interaural_overlay/<speaker>_interaural_overlay.png` (`plot_interaural_overlay`): "Which ear hears the speaker first": left and right impulse responses overlaid on −1 … +5 ms around the earlier peak, faint full range in a small inset, an arrow between the two peaks with `0.31 ms: left ear first`; the peaks are the `left_peak`/`right_peak` the service passes (do not recompute them).

## Tests and review material
- Replace `png_outputs_have_matplotlib_sizes` with the new size contract, update `png_is_not_blank` and the fixed-colour checks to the new tokens, keep the invalid-input tests, and add one test per chart kind that renders synthetic data and asserts: file exists, expected size, background token present, ink present (title text renders), no panic on empty optional panels.
- Add an ignored gallery test `gallery` in `crates/impulcifer-plots/tests/render.rs` that writes one of every chart with synthetic data to `E:/Impulcifer/target/plots-gallery/synthetic/`.
- Render the real demo plots: copy `E:/Impulcifer/data/demo` to a temp directory and run the 3.x CLI there with plots (`cargo run -p impulcifer-cli --release -- --dir_path <copy> --plot`, check `crates/impulcifer-cli` for the exact flags), then copy the whole `plots/` tree to `E:/Impulcifer/target/plots-gallery/demo/`. List every PNG path in the report; the owner reviews the pictures themselves.
- `crates/impulcifer-service/tests/brir_plots.rs` must pass unchanged. `tests/migration/README-plots.md` gets a short "Rendering (P23)" paragraph: the series contract is unchanged, the pictures are designed, sizes and tokens listed. Write the visual system down in `docs/rust/PLOTS.md` (tokens, per-chart layouts, how to add a chart).

## Allowed files
`crates/impulcifer-plots/**` (sources, tests, assets with licences), `crates/impulcifer-service/src/brir/plots.rs` only if a chart needs one more already-computed number passed in (prefer computing annotations inside the plots crate from the series), `docs/rust/PLOTS.md` (new), `tests/migration/README-plots.md` (one paragraph). Nothing else.

## Verification (foreground, paste output)
```
cd E:/Impulcifer
cargo fmt --all -- --check
cargo clippy -p impulcifer-plots -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-plots
cargo test -p impulcifer-plots --test render -- --ignored gallery
cargo test -p impulcifer-service --test brir_plots
cargo test -p impulcifer-policy
git status --porcelain
```

## Report format
(1) the token and layout decisions you made inside the direction; (2) the full list of PNG paths under `target/plots-gallery/` (synthetic and demo); (3) the pasted `test result:` lines and `git status --porcelain`; (4) anything you could not do and why. Do not end your turn before the commands complete.
